// Copyright 2016 Bruno Medeiros
//
// Licensed under the Apache License, Version 2.0 
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>. 
// This file may not be copied, modified, or distributed
// except according to those terms.


// WARNING: Rust newbie code ahead (-_-)'


#![allow(non_upper_case_globals)]

use util::core::*;

use serde;
use serde_json;

use serde_json::Map;
use serde_json::Value;
use serde_json::builder::ObjectBuilder;

use std::io;
use std::collections::HashMap;
use std::result::Result;

use service_util::ServiceError;
use service_util::ServiceHandler;
use service_util::Provider;

use json_util::*;


/* ----------------- JSON RPC ----------------- */

#[derive(Debug, PartialEq)]
pub enum RpcId { Number(u64), String(String), }

#[derive(Debug, PartialEq)]
/// A JSON RPC request, version 2.0
pub struct JsonRpcRequest {
	// ommited jsonrpc field, must be "2.0"
	//pub jsonrpc : String, 
	pub id : Option<RpcId>,
	pub method : String,
	pub params : Map<String, Value>,
}

/// A JSON RPC response, version 2.0
/// Only one of 'result' or 'error' is defined
#[derive(Debug, PartialEq)]
pub struct JsonRpcResponse {
	pub id : Option<RpcId>,
//	pub result : Option<Value>,
//	pub error: Option<JsonRpcError>,
	pub result_or_error: JsonRpcResult_Or_Error,
}

#[derive(Debug, PartialEq)]
pub enum JsonRpcResult_Or_Error {
	Result(Value),
	Error(JsonRpcError)
}

#[derive(Debug, PartialEq)]
pub struct JsonRpcError {
	pub code : i64,
	pub message : String,
	pub data : Option<Value>,
}

impl JsonRpcError {
	
	pub fn new(code: i64, message: String) -> JsonRpcError {
		JsonRpcError { code : code, message : message, data : None }
	}
	
}

impl JsonRpcResponse {
	
	pub fn new_result(id: Option<RpcId>, result: Value) -> JsonRpcResponse {
		JsonRpcResponse { id : id, result_or_error : JsonRpcResult_Or_Error::Result(result) }
	}
	
	pub fn new_error(id: Option<RpcId>, error: JsonRpcError) -> JsonRpcResponse {
		JsonRpcResponse { id : id, result_or_error : JsonRpcResult_Or_Error::Error(error) }
	}
	
}

/* -----------------  ----------------- */

pub fn error_JSON_RPC_ParseError() -> JsonRpcError { 
	JsonRpcError::new(-32700, "Invalid JSON was received by the server.".to_string())
}
pub fn error_JSON_RPC_InvalidRequest() -> JsonRpcError { 
	JsonRpcError::new(-32600, "The JSON sent is not a valid Request object.".to_string())
}
pub fn error_JSON_RPC_MethodNotFound() -> JsonRpcError { 
	JsonRpcError::new(-32601, "The method does not exist / is not available.".to_string())
}
pub fn error_JSON_RPC_InvalidParams() -> JsonRpcError { 
	JsonRpcError::new(-32602, "Invalid method parameter(s).".to_string())
}
pub fn error_JSON_RPC_InternalError() -> JsonRpcError { 
	JsonRpcError::new(-32603, "Internal JSON-RPC error.".to_string())
}



pub type JsonRpcResult<T> = Result<T, JsonRpcError>;

struct JsonRequestDeserializerHelper {
	
}

impl JsonDeserializerHelper<JsonRpcError> for JsonRequestDeserializerHelper {
	
	fn new_request_deserialization_error(&self) -> JsonRpcError {
		return error_JSON_RPC_InvalidRequest();
	}
	
}

impl serde::Serialize for RpcId {
	fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
		where S: serde::Serializer,
	{
		match self {
			&RpcId::Number(number) => serializer.serialize_u64(number), 
			&RpcId::String(ref string) => serializer.serialize_str(string),
		}
	}
}


// TODO: review code below, probably a way to shorten this
impl RpcId {
	pub fn to_value(&self) -> Value {
		serde_json::to_value(&self)
	}
}
impl JsonRpcRequest {
	
	pub fn new(id_number : u64, method : String, params : Map<String, Value>) -> JsonRpcRequest {
		JsonRpcRequest { 	
			id : Some(RpcId::Number(id_number)),
			method : method,
			params : params,
		} 
	}
	
	pub fn to_value(&self) -> Value {
		serde_json::to_value(&self)
	}
}



impl serde::Serialize for JsonRpcRequest {
	fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
		where S: serde::Serializer
	{
		// TODO: need to investigate if elem_count = 4 is actually valid when id is missing
		// serializing to JSON seems to not be a problem, but there might be other issues
		let elem_count = 4;
		let mut state = try!(serializer.serialize_struct("JsonRpcRequest", elem_count)); 
		{
			try!(serializer.serialize_struct_elt(&mut state, "jsonrpc", "2.0"));
			if let Some(ref id) = self.id {
				try!(serializer.serialize_struct_elt(&mut state, "id", id));
			}
			try!(serializer.serialize_struct_elt(&mut state, "method", &self.method));
			try!(serializer.serialize_struct_elt(&mut state, "params", &self.params));
		}
		serializer.serialize_struct_end(state)
	}
}

impl serde::Serialize for JsonRpcResponse {
	fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
		where S: serde::Serializer
	{
		let elem_count = 2;
		let mut state = try!(serializer.serialize_struct("JsonRpcRequest", elem_count)); {
			
			try!(serializer.serialize_struct_elt(&mut state, "jsonrpc", "2.0"));
			match self.result_or_error {
				//FIXME: test
				JsonRpcResult_Or_Error::Result(ref value) => {
					try!(serializer.serialize_struct_elt(&mut state, "result", &value));
				}
				JsonRpcResult_Or_Error::Error(ref json_rpc_error) => {
					//FIXME todo
//					try!(serializer.serialize_struct_elt(&mut state, "result", &json_rpc_error)); 
				}
			}
		}
		serializer.serialize_struct_end(state)
	}
}

/* -----------------  ----------------- */


pub fn parse_jsonrpc_request(message: &str) -> JsonRpcResult<JsonRpcRequest> {
	let mut json_result : Value = match 
		serde_json::from_str(message) 
	{
		Ok(ok) => { ok } 
		Err(error) => { 
			return Err(error_JSON_RPC_ParseError());
		}
	};
	
	parse_jsonrpc_request_json(&mut json_result)
}

pub fn parse_jsonrpc_request_json(request_json: &mut Value) -> JsonRpcResult<JsonRpcRequest> {
	
	let mut json_request_map : &mut Map<String, Value> =
	match request_json {
		&mut Value::Object(ref mut map) => map ,
		_ => { return Err(error_JSON_RPC_InvalidRequest()) },
	};
	parse_jsonrpc_request_jsonObject(&mut json_request_map)
}

pub fn parse_jsonrpc_request_jsonObject(mut request_map: &mut Map<String, Value>) -> JsonRpcResult<JsonRpcRequest> {
	
	let mut helper = JsonRequestDeserializerHelper { };
	
	let jsonrpc = try!(helper.obtain_String(&mut request_map, "jsonrpc"));
	if jsonrpc != "2.0" {
		return Err(error_JSON_RPC_InvalidRequest())
	}
	let id = try!(parse_jsonrpc_request_id(request_map.remove("id")));
	let method = try!(helper.obtain_String(&mut request_map, "method"));
	let params = try!(helper.obtain_Map_or(&mut request_map, "params", &|| new_object()));
	
	let jsonrpc_request = JsonRpcRequest { id : id, method : method, params : params}; 
	
	Ok(jsonrpc_request)
}

pub fn parse_jsonrpc_request_id(id: Option<Value>) -> JsonRpcResult<Option<RpcId>> {
	let id : Value = match id {
		None => return Ok(None),
		Some(id) => id,
	};
	match id {
		Value::I64(number) => Ok(Some(RpcId::Number(number as u64))), // FIXME truncation
		Value::U64(number) => Ok(Some(RpcId::Number(number))),
		Value::String(string) => Ok(Some(RpcId::String(string))),
		Value::Null => Ok(None),
		_ => Err(error_JSON_RPC_InvalidRequest()),
	}
}



impl JsonRpcError {
	
	pub fn to_string(&self) -> String {
		let value = ObjectBuilder::new()
			.insert("code", self.code)
			.insert("message", &self.message)
			.build()
		;
		// TODO: test
		return serde_json::to_string(&value).unwrap();
	}
	
	pub fn write_out(&self, out: &mut io::Write) -> io::Result<()> {
		try!(out.write_all(self.to_string().as_bytes()));
		Ok(())
	}
	
}


/* -----------------  ----------------- */

pub type DispatcherFn = Fn(&mut io::Write, Map<String, Value>);

pub struct JsonRpcDispatcher<'a> {
	pub dispatcher_map : HashMap<String, Box<DispatcherFn>>,
	pub output : &'a mut io::Write,
}

impl<'a> JsonRpcDispatcher<'a> {
	
	pub fn new(output : &'a mut io::Write) -> JsonRpcDispatcher<'a> {
		JsonRpcDispatcher { dispatcher_map : HashMap::new() , output : output }
	}
	
	pub fn read_incoming_messages<PROVIDER : Provider<String, GError>>(&mut self, mut input: PROVIDER ) -> GResult<()> {
		loop {
			let message = try!(input.obtain_next());
			
			match self.process_message(&message) {
				Ok(_) => {  } 
				Err(error) => {
					try!(error.write_out(self.output));
					// TODO log 
//					try!(output.write_fmt(format_args!("Error parsing message: "))); 
				}
			};
		}
	}
	
	pub fn process_message(&mut self, message: &str) -> JsonRpcResult<()> {
		
		let rpc_request = try!(parse_jsonrpc_request(message));
		
		try!(self.dispatch(rpc_request));
		
		Ok(())
	}
	
	pub fn add_notification<METHOD_PARAMS>(
		&mut self,
		method : (&'static str, Box<Fn(METHOD_PARAMS)>)
	)
		where 
		METHOD_PARAMS: serde::Deserialize + 'static, // FIXME review
	{
		let method_name: String = method.0.to_string();
		let method_fn: Box<Fn(METHOD_PARAMS)> = method.1;
		
		let handler_fn : Box<DispatcherFn> = Box::new(move |_json_rpc_handler, params_map| { 
			let params : Result<METHOD_PARAMS, _> = serde_json::from_value(Value::Object(params_map));
			let params : METHOD_PARAMS = params.unwrap(); /* FIXME: */
			method_fn(params);
		});
		
		self.dispatcher_map.insert(method_name, handler_fn);
	}
	
	pub fn add_request<METHOD_PARAMS, METHOD_RESULT, METHOD_ERROR_DATA>(
		&mut self,
		method : (&'static str, Box<ServiceHandler<METHOD_PARAMS, METHOD_RESULT, METHOD_ERROR_DATA>>)
	)
		where 
		METHOD_PARAMS: serde::Deserialize + 'static, // FIXME review why 'static
		METHOD_RESULT: serde::Serialize + 'static,
		METHOD_ERROR_DATA: serde::Serialize + 'static,
	{
		let method_name: String = method.0.to_string();
		let method_fn: Box<ServiceHandler<METHOD_PARAMS, METHOD_RESULT, METHOD_ERROR_DATA>> = method.1;
		
		let handler_fn : Box<DispatcherFn> = Box::new(move |writer : &mut io::Write, params_map| {
			Self::handle_request(writer, params_map, method_fn.as_ref()); 
		});
		
		self.dispatcher_map.insert(method_name, handler_fn);
	}
	
	pub fn handle_request<WRITE, METHOD_PARAMS, METHOD_RESULT, METHOD_ERROR_DATA>(
		writer: WRITE, 
		params_map: Map<String, Value>,
		method_fn: &ServiceHandler<METHOD_PARAMS, METHOD_RESULT, METHOD_ERROR_DATA>
	) 
		where 
		WRITE: io::Write,
		METHOD_PARAMS: serde::Deserialize + 'static, // FIXME review why 'static
		METHOD_RESULT: serde::Serialize + 'static,
		METHOD_ERROR_DATA: serde::Serialize + 'static,
	{
		// FIXME: TODO asynchronous handling
		let result_or_error = Self::handle_request2(params_map, method_fn);
		
		let json_response = JsonRpcResponse { 
			id : Some(RpcId::Number(1)), // FIXME: ID
			result_or_error : result_or_error, 
		};
		
		// FIXME: review this intermediate step
		let mut writer : Box<io::Write> = Box::new(writer);
		// FIXME: result
		serde_json::to_writer(&mut writer, &json_response);
		// TODO: log
	}
	
	pub fn handle_request2<METHOD_PARAMS, METHOD_RESULT, METHOD_ERROR_DATA>(
		params_map: Map<String, Value>,
		method_fn: &ServiceHandler<METHOD_PARAMS, METHOD_RESULT, METHOD_ERROR_DATA>
	) -> JsonRpcResult_Or_Error
		where 
		METHOD_PARAMS: serde::Deserialize + 'static, // FIXME review why 'static
		METHOD_RESULT: serde::Serialize + 'static,
		METHOD_ERROR_DATA: serde::Serialize + 'static,
	{
		let params_result : Result<METHOD_PARAMS, _> = serde_json::from_value(Value::Object(params_map));
		
		let params = 
		if let Ok(params) = params_result {
			params
		} else {
			return JsonRpcResult_Or_Error::Error(error_JSON_RPC_InvalidParams());
		};
		
		let result = method_fn(params);
		
		match result {
			Ok(ret) => {
				let ret = serde_json::to_value(&ret);
				return JsonRpcResult_Or_Error::Result(ret); 
			} 
			Err(error) => {
				let error : ServiceError<METHOD_ERROR_DATA> = error; // FIXME cleanup syntax
				let json_rpc_error = JsonRpcError { 
					code : error.code as i64, // FIXME review truncation
					message : error.message,
					data : Some(serde_json::to_value(&error.data)),
				};
				
				return JsonRpcResult_Or_Error::Error(json_rpc_error);
			}
		}
	}
	
	pub fn dispatch(&mut self, request: JsonRpcRequest) -> JsonRpcResult<()> {
		
		if let Some(dispatcher_fn) = self.dispatcher_map.get(&request.method) 
		{
			dispatcher_fn(&mut self.output, request.params);
			Ok(())
		} else {
			Err(error_JSON_RPC_MethodNotFound())
		}
	}
	
}

/* ----------------- Test ----------------- */

#[test]
fn parse_jsonrpc_request_json_Test() {
	
	let sample_params = unwrap_object(ObjectBuilder::new()
		.insert("param", "2.0")
		.insert("foo", 123)
	);
	
	// Test invalid JSON
	assert_eq!(parse_jsonrpc_request("{" ).unwrap_err(), error_JSON_RPC_ParseError());
	
	// Test invalid JsonRpcRequest
	let mut invalid_request = ObjectBuilder::new()
		.insert("jsonrpc", "2.0")
		.insert("id", 1)
		.insert("params", sample_params.clone())
		.build();
	
	let result = parse_jsonrpc_request_json(&mut invalid_request).unwrap_err();
	assert_eq!(result, error_JSON_RPC_InvalidRequest());
	
	// Test invalid JsonRpcRequest 2 - jsonrpc 1.0
	let mut invalid_request = ObjectBuilder::new()
		.insert("jsonrpc", "1.0")
		.insert("id", 1)
		.insert("method", "my method")
		.insert("params", sample_params.clone())
		.build();
	
	let result = parse_jsonrpc_request_json(&mut invalid_request).unwrap_err();
	assert_eq!(result, error_JSON_RPC_InvalidRequest());
	
	// Test basic JsonRpcRequest
	let request = JsonRpcRequest { 
		id : Some(RpcId::Number(1)), 
		method: "myMethod".to_string(), 
		params: sample_params.clone() 
	}; 
	
	let result = parse_jsonrpc_request_json(&mut request.to_value()).unwrap();
	assert_eq!(request, result);
	
	// Test basic JsonRpcRequest, no params
	let mut request = ObjectBuilder::new()
		.insert("jsonrpc", "2.0")
		.insert("id", 1)
		.insert("method", "myMethod")
		.build();
	
	let result = parse_jsonrpc_request_json(&mut request).unwrap();
	assert_eq!(result, JsonRpcRequest { 
			id : Some(RpcId::Number(1)), 
			method : "myMethod".to_string(), 
			params : unwrap_object(ObjectBuilder::new())
	});
	
}

#[test]
fn test_JsonRpcDispatcher() {
	
	use std::collections::BTreeMap;
	
	let mut output : Vec<u8> = vec![];
	let mut rpc = JsonRpcDispatcher::new(&mut output);
	
	let request = JsonRpcRequest::new(1, "my_method".to_string(), BTreeMap::new()); 
	assert_eq!(rpc.dispatch(request), Err(error_JSON_RPC_MethodNotFound()));
	
	
	let handler : Box<Fn(Vec<u32>) -> Result<String, ServiceError<()>>> = Box::new(move |params| {
		let params : Vec<u32> = params;
		let len : usize = params.len();
		let ret : String = len.to_string();
		Ok(ret)
	});
	rpc.add_request(("my_method", handler));
	
	let request = JsonRpcRequest::new(1, "my_method".to_string(), BTreeMap::new());
	assert_eq!(rpc.dispatch(request), Err(error_JSON_RPC_InvalidParams()));
}
