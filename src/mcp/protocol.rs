use serde::{Deserialize, Serialize};
use serde_json::Value;

pub fn json_rpc_error(code: i32, message: impl Into<String>, id: Option<Value>) -> Value {
    serde_json::to_value(JsonRpcError {
        jsonrpc: "2.0".to_string(),
        error: JsonRpcErrorDetail {
            code,
            message: message.into(),
        },
        id,
    })
    .unwrap_or_else(|_| {
        serde_json::json!({
            "jsonrpc": "2.0",
            "error": {"code": -32603, "message": "Internal error: failed to serialize error response"},
            "id": null
        })
    })
}

pub fn invalid_request(message: impl Into<String>, id: Option<Value>) -> Value {
    json_rpc_error(-32600, message, id)
}

pub fn method_not_found(message: impl Into<String>, id: Option<Value>) -> Value {
    json_rpc_error(-32601, message, id)
}

#[derive(Deserialize, Debug)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: Option<serde_json::Value>,
    pub id: Option<serde_json::Value>,
}

#[derive(Serialize, Debug)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub result: serde_json::Value,
}

#[derive(Serialize, Debug)]
pub struct JsonRpcError {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub error: JsonRpcErrorDetail,
}

#[derive(Serialize, Debug)]
pub struct JsonRpcErrorDetail {
    pub code: i32,
    pub message: String,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub protocol_version: String,
    pub capabilities: Capabilities,
    pub server_info: ServerInfo,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub tools: ToolsCapability,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ToolsCapability {
    pub list_changed: bool,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}
