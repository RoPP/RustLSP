// Copyright 2016 Bruno Medeiros
//
// Licensed under the Apache License, Version 2.0 
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>. 
// This file may not be copied, modified, or distributed
// except according to those terms.


extern crate serde_json;
extern crate serde;


use std::fmt;
use serde_json::Value;

use util::core::GResult;
use util::core::ErrorMessage;
use service_util::ServiceResult;
use json_util::*;


/* ----------------- JSON-RPC 2.0 object types ----------------- */

#[derive(Debug, PartialEq, Clone)]
pub enum RpcId { Number(u64), String(String), Null, }

#[derive(Debug, PartialEq, Clone)]
/// A JSON RPC request, version 2.0
pub struct JsonRpcRequest {
	// ommited jsonrpc field, must be "2.0" when serialized
	//pub jsonrpc : String, 
	pub id : Option<RpcId>,
	pub method : String,
	pub params : RequestParams,
}

#[derive(Debug, PartialEq, Clone)]
pub enum RequestParams {
	Object(JsonObject),
	Array(Vec<Value>),
	None,
}


/// A JSON RPC response, version 2.0
/// Only one of 'result' or 'error' is defined
#[derive(Debug, PartialEq, Clone)]
pub struct JsonRpcResponse {
	// Rpc id. Note: spec requires key `id` to be present
	pub id : RpcId, 
	// field `result` or field `error`:
	pub result_or_error: ResponseResult,
}

#[derive(Debug, PartialEq, Clone)]
pub enum ResponseResult {
	Result(Value),
	Error(RpcError)
}

#[derive(Debug, PartialEq, Clone)]
pub struct RpcError {
	pub code : i64,
	pub message : String,
	pub data : Option<Value>,
}

/* ----------------- Impls ----------------- */

impl JsonRpcRequest {
	
	pub fn new(id_number : u64, method : String, params : JsonObject) -> JsonRpcRequest {
		JsonRpcRequest { 	
			id : Some(RpcId::Number(id_number)),
			method : method,
			params : RequestParams::Object(params),
		} 
	}
	
	pub fn to_message(self) -> JsonRpcMessage {
		JsonRpcMessage::Request(self)
	}
	
}


impl JsonRpcResponse {
	
	pub fn new_result(id: RpcId, result: Value) -> JsonRpcResponse {
		JsonRpcResponse { id : id, result_or_error : ResponseResult::Result(result) }
	}
	
	pub fn new_error(id: RpcId, error: RpcError) -> JsonRpcResponse {
		JsonRpcResponse { id : id, result_or_error : ResponseResult::Error(error) }
	}
	
	pub fn to_message(self) -> JsonRpcMessage {
		JsonRpcMessage::Response(self)
	}
	
}

impl RpcError {
	
	pub fn new(code: i64, message: String) -> RpcError {
		RpcError { code : code, message : message, data : None }
	}
	
}

pub fn error_JSON_RPC_ParseError<T: fmt::Display>(error: T) -> RpcError { 
	RpcError::new(-32700, format!("Invalid JSON was received by the server: {}", error).to_string())
}
pub fn error_JSON_RPC_InvalidRequest<T: fmt::Display>(error: T) -> RpcError { 
	RpcError::new(-32600, format!("The JSON sent is not a valid Request object: {}", error).to_string())
}
pub fn error_JSON_RPC_MethodNotFound() -> RpcError { 
	RpcError::new(-32601, "The method does not exist / is not available.".to_string())
}
pub fn error_JSON_RPC_InvalidParams<T: fmt::Display>(error: T) -> RpcError { 
	RpcError::new(-32602, format!("Invalid method parameter(s): {}", error).to_string())
}
pub fn error_JSON_RPC_InternalError() -> RpcError { 
	RpcError::new(-32603, "Internal JSON-RPC error.".to_string())
}


impl ResponseResult {
	
	pub fn from_service_result<RET, RET_ERROR>(svc_result : ServiceResult<RET, RET_ERROR>) -> ResponseResult 
	where 
		RET : serde::Serialize, 
		RET_ERROR : serde::Serialize ,
	{
		match svc_result {
			Ok(ret) => {
				let ret = serde_json::to_value(&ret);
				ResponseResult::Result(ret) 
			} 
			Err(error) => {
				let code : u32 = error.code;
				let json_rpc_error = RpcError { 
					code : code as i64, // Safe convertion. TODO: use TryFrom when it's stable
					message : error.message,
					data : Some(serde_json::to_value(&error.data)),
				};
				
				ResponseResult::Error(json_rpc_error)
			}
		}
	}
	
}

/* -----------------  ----------------- */

impl serde::Serialize for RpcId {
	fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
		where S: serde::Serializer,
	{
		match *self {
			RpcId::Null => serializer.serialize_none(),
			RpcId::Number(number) => serializer.serialize_u64(number), 
			RpcId::String(ref string) => serializer.serialize_str(string),
		}
	}
}


impl serde::Serialize for RequestParams {
	fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
		where S: serde::Serializer
	{
		match *self {
			RequestParams::Object(ref object) => object.serialize(serializer),
			RequestParams::Array(ref array) => array.serialize(serializer),
			RequestParams::None => serializer.serialize_none(),
		}
	}
}

impl RequestParams {
	
	pub fn into_value(self) -> Value {
		// Note, we could use serde_json::to_value(&params) but that is less efficient:
		// it reserializes the value, instead of just obtaining the underlying one 
		
		match self {
			RequestParams::Object(object) => Value::Object(object),
			RequestParams::Array(array) => Value::Array(array),
			RequestParams::None => Value::Null,
		}
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
		let elem_count = 3;
		let mut state = try!(serializer.serialize_struct("JsonRpcResponse", elem_count));
		{
			try!(serializer.serialize_struct_elt(&mut state, "jsonrpc", "2.0"));
			try!(serializer.serialize_struct_elt(&mut state, "id", &self.id));
			
			match self.result_or_error {
				ResponseResult::Result(ref value) => {
					try!(serializer.serialize_struct_elt(&mut state, "result", &value));
				}
				ResponseResult::Error(ref json_rpc_error) => {
					try!(serializer.serialize_struct_elt(&mut state, "error", &json_rpc_error)); 
				}
			}
		}
		serializer.serialize_struct_end(state)
	}
}

impl serde::Serialize for RpcError {
	fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
		where S: serde::Serializer
	{
		let elem_count = 3;
		let mut state = try!(serializer.serialize_struct("RpcError", elem_count)); 
		{
			try!(serializer.serialize_struct_elt(&mut state, "code", self.code));
			try!(serializer.serialize_struct_elt(&mut state, "message", &self.message));
			if let Some(ref data) = self.data {
				try!(serializer.serialize_struct_elt(&mut state, "data", data));
			}
		}
		serializer.serialize_struct_end(state)
	}
}

/* -----------------  JSON-RPC custom deserialization  ----------------- */

struct JsonRequestDeserializerHelper;

impl JsonDeserializerHelper<RpcError> for JsonRequestDeserializerHelper {
	
	fn new_error(&self, error_message: &str) -> RpcError {
		return error_JSON_RPC_InvalidRequest(error_message.to_string());
	}
	
}

pub type JsonRpcParseResult<T> = Result<T, RpcError>;


pub fn parse_jsonrpc_request(message: &str) -> JsonRpcParseResult<JsonRpcRequest> {
	let json_value : Value = 
	match serde_json::from_str(message) 
	{
		Ok(ok) => { ok } 
		Err(error) => { 
			return Err(error_JSON_RPC_ParseError(error));
		}
	};
	
	let mut helper = JsonRequestDeserializerHelper { };
	let mut json_request_obj : JsonObject = try!(helper.as_Object(json_value));
		
	parse_jsonrpc_request_jsonObject(&mut json_request_obj)
}

pub fn parse_jsonrpc_request_jsonObject(mut request_map: &mut JsonObject) -> JsonRpcParseResult<JsonRpcRequest> {
	let mut helper = JsonRequestDeserializerHelper { };
	
	let jsonrpc = try!(helper.obtain_String(&mut request_map, "jsonrpc"));
	if jsonrpc != "2.0" {
		return Err(error_JSON_RPC_InvalidRequest(r#"Property `jsonrpc` is not "2.0". "#))
	}
	let id = try!(parse_jsonrpc_id(request_map.remove("id")));
	let method = try!(helper.obtain_String(&mut request_map, "method"));
	let params = try!(helper.obtain_Value(&mut request_map, "params"));
	
	let params : RequestParams = match parse_jsonrpc_params(params) {
		Ok(ok) => ok,
		Err(error) => return Err(error_JSON_RPC_InvalidRequest(error)),
	};
	
	let jsonrpc_request = JsonRpcRequest { id : id, method : method, params : params }; 
	
	Ok(jsonrpc_request)
}

pub fn parse_jsonrpc_params(params: Value) -> GResult<RequestParams> {
	match params {
		Value::Object(object) => Ok(RequestParams::Object(object)),
		Value::Array(array) => Ok(RequestParams::Array(array)),
		Value::Null => Ok(RequestParams::None),
		_ => Err(ErrorMessage::create("Property `params` not an Object, Array, or null.".into())),
	}
}

pub fn parse_jsonrpc_id(id: Option<Value>) -> JsonRpcParseResult<Option<RpcId>> {
	let id : Value = match id {
		None => return Ok(None),
		Some(id) => id,
	};
	match id {
		Value::I64(number) => Ok(Some(RpcId::Number(number as u64))), // FIXME truncation
		Value::U64(number) => Ok(Some(RpcId::Number(number))),
		Value::String(string) => Ok(Some(RpcId::String(string))),
		Value::Null => Ok(None),
		_ => Err(error_JSON_RPC_InvalidRequest("Property `id` not a String or integer.")),
	}
}

/* -----------------  ----------------- */

#[derive(Debug, PartialEq, Clone)]
pub enum JsonRpcMessage {
	Request(JsonRpcRequest),
	Response(JsonRpcResponse),
}

impl serde::Serialize for JsonRpcMessage {
	fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
		where S: serde::Serializer
	{
		match *self {
			JsonRpcMessage::Request(ref request) => request.serialize(serializer),
			JsonRpcMessage::Response(ref response) => response.serialize(serializer),
		}
	}
}


/* ----------------- Tests ----------------- */

#[cfg(test)]
pub mod tests {
	
	use super::*;
	use util::tests::*;
	
	use serde;
	use serde_json;
	use serde_json::Value;
	use serde_json::builder::ObjectBuilder;
	use json_util::*;

	pub fn to_json<T: serde::Serialize>(value: &T) -> String {
		serde_json::to_string(value).unwrap()
	}
	
	pub fn check_error(result: RpcError, expected: RpcError) {
		assert_starts_with(&result.message, &expected.message);
		assert_eq!(result, RpcError { message : result.message.clone(), .. expected }); 
	}
	
	#[test]
	fn test__jsonrpc_params() {
		
		let sample_obj = unwrap_object_builder(ObjectBuilder::new().insert("xxx", 123));
		let sample_string = Value::String("blah".into());
		
		test__jsonrpc_params_serde(RequestParams::Object(sample_obj.clone()));
		test__jsonrpc_params_serde(RequestParams::Array(vec![sample_string.clone(), sample_string]));
		test__jsonrpc_params_serde(RequestParams::None);
	}
	
	fn test__jsonrpc_params_serde(params: RequestParams) {
		let params_string = to_json(&params);
		let params2 = parse_jsonrpc_params(serde_json::from_str(&params_string).unwrap()).unwrap();
		
		assert_equal(params, params2);
	}
	
	#[test]
	fn test__parse_jsonrpc_request() {
		
		let sample_params = unwrap_object_builder(ObjectBuilder::new()
			.insert("param", "2.0")
			.insert("foo", 123)
		);
		
		// Test invalid JSON
		check_error(parse_jsonrpc_request("{" ).unwrap_err(), error_JSON_RPC_ParseError(""));
		
		assert_equal(
			parse_jsonrpc_request("{ }"),
			Err(error_JSON_RPC_InvalidRequest("Property `jsonrpc` is missing."))
		);
		assert_equal(
			parse_jsonrpc_request(r#"{ "jsonrpc": "1.0" }"#),
			Err(error_JSON_RPC_InvalidRequest(r#"Property `jsonrpc` is not "2.0". "#))
		);
		
		assert_equal(
			parse_jsonrpc_request(r#"{ "jsonrpc": "2.0" }"#),
			Err(error_JSON_RPC_InvalidRequest("Property `method` is missing."))
		);
		assert_equal(
			parse_jsonrpc_request(r#"{ "jsonrpc": "2.0", "method":null }"#),
			Err(error_JSON_RPC_InvalidRequest("Value `null` is not a String."))
		);
		
		assert_equal(
			parse_jsonrpc_request(r#"{ "jsonrpc": "2.0", "method":"xxx" }"#),
			Err(error_JSON_RPC_InvalidRequest("Property `params` is missing."))
		);
		
		// Test valid request with params = null
		assert_equal(
			parse_jsonrpc_request(r#"{ "jsonrpc": "2.0", "method":"xxx", "params":null }"#),
			Ok(JsonRpcRequest { id : None, method : "xxx".into(), params : RequestParams::None, }) 
		);
		
		// --- Test serialization ---
		 
		// basic JsonRpcRequest
		let request = JsonRpcRequest::new(1, "myMethod".to_string(), sample_params.clone()); 
		let result = parse_jsonrpc_request(&to_json(&request)).unwrap();
		assert_eq!(request, result);
		
		// Test basic JsonRpcRequest, no params
		let request = JsonRpcRequest { id : None, method : "myMethod".to_string(), params : RequestParams::None, };
		let result = parse_jsonrpc_request(&to_json(&request)).unwrap();
		assert_eq!(result, request);
		
		// Test JsonRpcRequest with no id
		let sample_array_params = RequestParams::Array(vec![]);
		let request = JsonRpcRequest { id : None, method : "myMethod".to_string(), params : sample_array_params, };  
		let result = parse_jsonrpc_request(&to_json(&request)).unwrap();
		assert_eq!(result, request);
	}

	#[test]
	fn test_JsonRpcResponse_serialize() {
		
		fn sample_json_obj(foo: u32) -> Value {
			ObjectBuilder::new().insert("foo", foo).build()
		}
		
		let response = JsonRpcResponse::new_result(RpcId::Null, sample_json_obj(100));
		let response = unwrap_object(serde_json::from_str(&to_json(&response)).unwrap());
		assert_equal(response, unwrap_object_builder(ObjectBuilder::new()
			.insert("jsonrpc", "2.0")
			.insert("id", RpcId::Null)
			.insert("result", sample_json_obj(100))
		));
		
		
		let response = JsonRpcResponse::new_result(RpcId::Number(123), sample_json_obj(200));
		let response = unwrap_object(serde_json::from_str(&to_json(&response)).unwrap());
		assert_equal(response, unwrap_object_builder(ObjectBuilder::new()
			.insert("jsonrpc", "2.0")
			.insert("id", 123)
			.insert("result", sample_json_obj(200))
		));
		
		let response = JsonRpcResponse::new_result(RpcId::Null, sample_json_obj(200));
		let response = unwrap_object(serde_json::from_str(&to_json(&response)).unwrap());
		assert_equal(response, unwrap_object_builder(ObjectBuilder::new()
			.insert("jsonrpc", "2.0")
			.insert("id", Value::Null)
			.insert("result", sample_json_obj(200))
		));
		
		let response = JsonRpcResponse::new_error(RpcId::String("321".to_string()), RpcError{
			code: 5, message: "msg".to_string(), data: Some(sample_json_obj(300))
		});
		let response = unwrap_object(serde_json::from_str(&to_json(&response)).unwrap());
		assert_equal(response, unwrap_object_builder(ObjectBuilder::new()
			.insert("jsonrpc", "2.0")
			.insert("id", "321")
			.insert("error", unwrap_object_builder(ObjectBuilder::new()
				.insert("code", 5)
				.insert("message", "msg")
				.insert("data", sample_json_obj(300))
			))
		));
		
	}
	
}