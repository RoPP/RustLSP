// Copyright 2016 Bruno Medeiros
//
// Licensed under the Apache License, Version 2.0 
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>. 
// This file may not be copied, modified, or distributed
// except according to those terms.


#![allow(non_upper_case_globals)]
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]

#[macro_use] extern crate log;
extern crate serde_json;
extern crate serde;

extern crate melnorme_util as util;

pub mod json_util;
pub mod jsonrpc_objects;
pub mod service_util;
pub mod output_agent;

/* -----------------  ----------------- */

use util::core::*;

use std::collections::HashMap;
use std::result::Result;

use std::sync::Arc;
use std::sync::Mutex;

use service_util::ServiceError;
use service_util::ServiceResult;
use service_util::MessageReader;

use jsonrpc_objects::*;


/* -----------------  Endpoint  ----------------- */

use output_agent::OutputAgent;
use output_agent::OutputAgentTask;
use output_agent::AgentRunnable;


/// A JSON-RPC Server-role than can receive requests.
/// TODO: Client role (send requests)
pub struct Endpoint {
	pub request_handler : Box<RequestHandler>,
	output_agent : Arc<Mutex<OutputAgent>>,
}

impl Endpoint {
	
	pub fn start<AGENT_RUNNER>(agent_runner: AGENT_RUNNER, request_handler: Box<RequestHandler>) 
		-> Endpoint
	where 
		AGENT_RUNNER : AgentRunnable,
		AGENT_RUNNER : Send + 'static,
	{
		let output_agent = OutputAgent::start(agent_runner);
		Self::start_with_output_agent(output_agent, request_handler)
	}
	
	pub fn start_with_output_agent(output_agent: OutputAgent, request_handler: Box<RequestHandler>) 
		-> Endpoint
	{
		Endpoint { request_handler: request_handler, output_agent : newArcMutex(output_agent) }
	}
	
	pub fn is_shutdown(& self) -> bool {
		self.output_agent.lock().unwrap().is_shutdown()
	}
	
	pub fn shutdown(&mut self) {
		self.output_agent.lock().unwrap().shutdown_and_join();
	}
	
	pub fn handle_message(&mut self, message: &str) {
		match parse_jsonrpc_request(message) {
			Ok(rpc_request) => { 
				self.handle_request(rpc_request);
			} 
			Err(error) => {
				// If we can't parse JsonRpcRequest, send an error response with null id
				let id = RpcId::Null;
				submit_write_task(&mut self.output_agent, JsonRpcResponse::new_error(id, error).to_message()); 
			}
		}
	}
	
	pub fn handle_request(&mut self, request: JsonRpcRequest) {
		let output_agent = self.output_agent.clone();
		
		let on_response = new(move |response: Option<JsonRpcResponse>| {
			if let Some(response) = response {
				submit_write_task(&output_agent, response.to_message()); 
			} else {
				let method_name = ""; // TODO
				info!("JSON-RPC notification complete. {:?}", method_name);
			} 
		});
		
		let completable = ResponseCompletable::new(request.id, on_response);
		self.request_handler.handle_request(&request.method, request.params, completable); 
	}
	
}

/* -----------------  ----------------- */

pub type EndpointHandle = Arc<Mutex<Endpoint>>;

pub fn run_message_read_loop<MSG_READER : ?Sized>(endpoint: Arc<Mutex<Endpoint>>, input: &mut MSG_READER) 
	-> GResult<()>
where
	MSG_READER : MessageReader
{
	loop {
		let message = match input.read_next() {
			Ok(ok) => { ok } 
			Err(error) => { 
				let mut endpoint = endpoint.lock().unwrap();
				endpoint.shutdown();
				return Err(error);
			}
		};
		
		let mut endpoint = endpoint.lock().unwrap();
		endpoint.handle_message(&message);
	}
}

/* ----------------- Response handling ----------------- */

pub trait RequestHandler {
	
	fn handle_request(
		&mut self, request_method: &str, request_params: RequestParams, completable: ResponseCompletable
	);
}

pub struct ResponseCompletable {
	completion_flag: FinishedFlag,
	id: Option<RpcId>,
	on_response: Box<FnMut(Option<JsonRpcResponse>) + Send>,
}

impl ResponseCompletable {
	
	pub fn new(id: Option<RpcId>, on_response: Box<FnMut(Option<JsonRpcResponse>) + Send>) -> ResponseCompletable {
		ResponseCompletable { 
			completion_flag : FinishedFlag(false), id : id, on_response: on_response
		}
	}
	
	pub fn complete(mut self, rpc_result: Option<ResponseResult>) {
		self.completion_flag.finish();
		
		// From the spec: `A Notification is a Request object without an "id" member.`
		if let Some(rpc_result) = rpc_result {
			
			let response =
			if let Some(id) = self.id {
				JsonRpcResponse{ id : id, result_or_error : rpc_result }
			} else {
				JsonRpcResponse::new_error(RpcId::Null, 
					error_JSON_RPC_InvalidRequest("Property `id` not provided for request."))
			};
			
			(self.on_response)(Some(response));
		} else {
			(self.on_response)(None)
		}
	}
	
	pub fn complete_with_error(self, error: RpcError) {
		self.complete(Some(ResponseResult::Error(error)));
	}
	
	pub fn sync_handle_request<PARAMS, RET, RET_ERROR, METHOD>(
		self, params: RequestParams, method_fn: METHOD
	) 
	where 
		PARAMS : serde::Deserialize, 
		RET : serde::Serialize, 
		RET_ERROR : serde::Serialize ,
		METHOD : FnOnce(PARAMS) -> ServiceResult<RET, RET_ERROR>,
	{
		let method_fn = move |params| Some(method_fn(params));
		let result = invoke_method_with_fn(params, method_fn);
		self.complete(result);
	}
	
	pub fn sync_handle_notification<PARAMS, METHOD>(
		self, params: RequestParams, method_fn: METHOD
	) 
	where 
		PARAMS : serde::Deserialize, 
		METHOD : FnOnce(PARAMS),
	{
		let method_fn = move |params| { method_fn(params); None };
		let result = invoke_method_with_fn::<_, (), (), _>(params, method_fn);
		self.complete(result);
	}
	
}

pub fn invoke_method_with_fn<PARAMS, RET, RET_ERROR, METHOD>(
	params: RequestParams,
	method_fn: METHOD
) -> Option<ResponseResult>
	where 
	PARAMS : serde::Deserialize, 
	RET : serde::Serialize, 
	RET_ERROR : serde::Serialize,
	METHOD : FnOnce(PARAMS) -> Option<ServiceResult<RET, RET_ERROR>>
{
	let params_value = params.into_value();
	
	let params_result : Result<PARAMS, _> = serde_json::from_value(params_value);
	
	let result = 
	match params_result {
		Ok(params) => { 
			method_fn(params) 
		} 
		Err(error) => { 
			return Some(ResponseResult::Error(error_JSON_RPC_InvalidParams(error)));
		}
	};
	
	let result = 
	if let Some(result) = result {
		result
	} else {
		return None;
	};
	
	match result {
		Ok(ret) => {
			let ret = serde_json::to_value(&ret);
			return Some(ResponseResult::Result(ret)); 
		} 
		Err(error) => {
			let error : ServiceError<RET_ERROR> = error; // FIXME cleanup syntax
			let json_rpc_error = RpcError { 
				code : error.code as i64, // FIXME review truncation
				message : error.message,
				data : Some(serde_json::to_value(&error.data)),
			};
			
			return Some(ResponseResult::Error(json_rpc_error));
		}
	}
}
	
	
pub fn submit_write_task(output_agent: &Arc<Mutex<OutputAgent>>, rpc_message: JsonRpcMessage) {
	
	let write_task : OutputAgentTask = Box::new(move |mut response_handler| {
		info!("JSON-RPC message: {:?}", rpc_message);
		
		let response_str = serde_json::to_string(&rpc_message).unwrap_or_else(|error| -> String { 
			panic!("Failed to serialize to JSON object: {}", error);
		});
		
		let write_res = response_handler.write_message(&response_str);
		if let Err(error) = write_res {
			// FIXME handle output stream write error by shutting down
			error!("Error writing JSON-RPC message: {}", error);
		};
	});
	
	let res = {
		output_agent.lock().unwrap().try_submit_task(write_task)
	}; 
	// If res is error, panic here, outside of thread lock
	res.expect("Output agent is shutdown or thread panicked!");
}

/* -----------------  Request sending  ----------------- */

impl Endpoint {
	
	// TODO
//	pub fn send_request<
//		PARAMS : serde::Serialize, 
//		RET: serde::Deserialize,
//		RET_ERROR : serde::Deserialize,
//	>(&mut self, method_name: &str, params: PARAMS) -> GResult<Future<ServiceResult<RET, RET_ERROR>>> {
//		let id = None; // FIXME
//			
//		self.do_send_request(id, method_name, params)
//	}
	
	pub fn do_send_request<
		PARAMS : serde::Serialize, 
		RET: serde::Deserialize,
		RET_ERROR : serde::Deserialize,
	>(&mut self, id: Option<RpcId>, method_name: &str, params: PARAMS) 
    	-> GResult<Future<ServiceResult<RET, RET_ERROR>>> 
	{
		let params_value = serde_json::to_value(&params);
		let params = try!(jsonrpc_objects::parse_jsonrpc_params(params_value));
		
		let rpc_request = JsonRpcRequest { id: id, method : method_name.into(), params : params };
		
		let future = Future(None);
		submit_write_task(&self.output_agent, JsonRpcMessage::Request(rpc_request));
		
		Ok(future)
	}
	
	pub fn send_notification<
		PARAMS : serde::Serialize, 
	>(&mut self, method_name: &str, params: PARAMS) -> GResult<()> {
		let id = None;
		
		let future: Future<ServiceResult<(), ()>> = try!(self.do_send_request(id, method_name, params));
		future.complete(Ok(()));
		Ok(())
	}
	
}

// FIXME: use upcoming futures API, this is just a mock ATM
pub struct Future<T>(Option<T>); 

impl<T> Future<T> {
	pub fn is_completed(&self) -> bool {
		// TODO
		true
	}
	
	pub fn complete(&self, _result: T) {
	}
}

/* -----------------  MapRequestHandler  ----------------- */

pub type RpcMethodHandler = Fn(RequestParams, ResponseCompletable);

pub struct MapRequestHandler {
	pub method_handlers : HashMap<String, Box<RpcMethodHandler>>,
}

impl MapRequestHandler {
	
	pub fn new() -> MapRequestHandler {
		 MapRequestHandler { method_handlers : HashMap::new() }
	}
	
	pub fn add_notification<
		PARAMS : serde::Deserialize + 'static,
	>(
		&mut self,
		method_name: &'static str, 
		method_fn: Box<Fn(PARAMS)>
	) {
		let req_handler : Box<RpcMethodHandler> = new(move |params, completable| {
			completable.sync_handle_notification(params, &*method_fn);
		});
		self.add_rpc_handler(method_name, req_handler);
	}
	
	pub fn add_request<
		PARAMS : serde::Deserialize + 'static, 
		RET : serde::Serialize + 'static, 
		RET_ERROR : serde::Serialize + 'static
	>(
		&mut self,
		method_name: &'static str, 
		method_fn: Box<Fn(PARAMS) -> ServiceResult<RET, RET_ERROR>>
	) {
		let req_handler : Box<RpcMethodHandler> = new(move |params, completable| {
			completable.sync_handle_request(params, &*method_fn);
		});
		self.add_rpc_handler(method_name, req_handler);
	}
	
	pub fn add_rpc_handler(
		&mut self,
		method_name: &'static str,
		method_handler: Box<RpcMethodHandler>
	) {
		self.method_handlers.insert(method_name.to_string(), method_handler);
	}
	
	fn do_invoke_method(
		&mut self, 
		method_name: &str, 
		completable: ResponseCompletable,
		request_params: RequestParams,
	) {
		if let Some(method_fn) = self.method_handlers.get(method_name) 
		{
			let method_fn : &Box<RpcMethodHandler> = method_fn;
			method_fn(request_params, completable);
		} else {
			completable.complete_with_error(error_JSON_RPC_MethodNotFound());
		};
	}
	
}

impl RequestHandler for MapRequestHandler {
	
	fn handle_request(&mut self, request_method: &str, request_params: RequestParams, 
		completable: ResponseCompletable) 
	{
		self.do_invoke_method(request_method, completable, request_params);
	}
	
}



/* ----------------- Tests ----------------- */

mod tests_sample_types;

#[cfg(test)]
mod tests_ {
	
	use super::*;
	use util::core::*;
	use util::tests::*;
	use tests_sample_types::*;
	use std::thread;
	
	use service_util::{ServiceResult, ServiceError};
	use jsonrpc_objects::*;
	use jsonrpc_objects::tests::*;
	
	use json_util::JsonObject;
	use output_agent::IoWriteHandler;
	use output_agent::OutputAgent;
	use serde_json::Value;
	use serde_json;
	
	pub fn sample_fn(params: Point) -> ServiceResult<String, ()> {
		let x_str : String = params.x.to_string();
		let y_str : String = params.y.to_string();
		Ok(x_str + &y_str)
	}
	pub fn new_sample_params(x: i32, y: i32) -> Point {
		Point { x : x, y : y }
	}
	pub fn no_params_method(_params: ()) -> Result<String, ServiceError<()>> {
		Ok("okay".into())
	}
	
	pub fn check_request(result: ResponseResult, expected: ResponseResult) {
		if let ResponseResult::Error(ref error) = result {
			
			if let ResponseResult::Error(expected_error) = expected {
				check_error(error.clone(), expected_error.clone());
				return;
			}
			
		}
		
		assert_equal(&result, &expected);
	}
	
	pub fn async_method(request_params: RequestParams, completable: ResponseCompletable) {
		thread::spawn(move || {
			completable.sync_handle_request(request_params, sample_fn);
        });
	}
		
	fn invoke_method<FN>(
		req_handler: &mut RequestHandler, 
		method_name: &str, 
		request_params: RequestParams, 
		mut and_then: FN
	) 
	where 
		FN : FnMut(Option<ResponseResult>) + 'static + Send
	{
		let on_response : Box<FnMut(Option<JsonRpcResponse>) + Send> = new(move |response: Option<JsonRpcResponse>| {
			and_then(response.and_then(|e| Some(e.result_or_error)));
		});
		
		let completable = ResponseCompletable::new(Some(RpcId::Number(123)), on_response);
		req_handler.handle_request(method_name, request_params, completable);
	}
	
	#[test]
	fn test_Endpoint() {
		
		{
			// Test handle unknown method
			let mut request_handler = MapRequestHandler::new();
			
			let request = JsonRpcRequest::new(1, "my_method".to_string(), JsonObject::new());
			invoke_method(&mut request_handler, &request.method, request.params,
				|result| 
				check_request(result.unwrap(), ResponseResult::Error(error_JSON_RPC_MethodNotFound())) 
			);
		}
		
		let mut request_handler = MapRequestHandler::new();
		request_handler.add_request("my_method", Box::new(sample_fn));
		request_handler.add_rpc_handler("async_method", Box::new(async_method));
		
		// test with invalid params = "{}" 
		let request = JsonRpcRequest::new(1, "my_method".to_string(), JsonObject::new());
		invoke_method(&mut request_handler, &request.method, request.params, 
			|result| 
			check_request(result.unwrap(), ResponseResult::Error(error_JSON_RPC_InvalidParams(r#"missing field "x""#)))
		);
		
		// test with valid params
		let params_value = match serde_json::to_value(&new_sample_params(10, 20)) {
			Value::Object(object) => object, 
			_ => panic!("Not serialized into Object") 
		};
		let request = JsonRpcRequest::new(1, "my_method".to_string(), params_value);
		invoke_method(&mut request_handler, &request.method, request.params.clone(),
			|result| 
			assert_equal(result.unwrap(), ResponseResult::Result(
				Value::String("1020".to_string())
			))
		);
		
		
		// Test valid request with params = "null"
		request_handler.add_request("no_params_method", Box::new(no_params_method));
		
		let id1 = Some(RpcId::Number(1));
		let request = JsonRpcRequest { id : id1, method : "no_params_method".into(), params : RequestParams::None, };
		invoke_method(&mut request_handler, &request.method, request.params.clone(), 
			|result| 
			assert_equal(result.unwrap(), ResponseResult::Result(
				Value::String("okay".to_string())
			))
		);
		
		// --- Endpoint:
		let output = vec![];
		let output_agent = OutputAgent::start_with_provider(move || IoWriteHandler(output));
		let mut rpc = Endpoint::start_with_output_agent(output_agent, new(request_handler));
		
		// Test ResponseCompletable - missing id for notification method
		let completable = ResponseCompletable::new(None, new(|_| {}));
		completable.complete(None);
		
		// Test ResponseCompletable - missing id for regular method
		let completable = ResponseCompletable::new(None, new(|_| {}));
		completable.complete(Some(ResponseResult::Result(Value::String("1020".to_string()))));
		
		// test again using handle_request
		// TODO review this code
		let request = JsonRpcRequest { 	
			id : None,
			method : "my_method".into(),
			params : request.params.clone(),
		}; 
		rpc.handle_request(request);
		
		
		let params = new_sample_params(123, 66);
		rpc.send_notification("my_method", params.clone()).unwrap();
		
		rpc.send_notification("async_method", params.clone()).unwrap();
		
		rpc.shutdown();
	}
	
}