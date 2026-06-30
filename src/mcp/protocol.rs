use serde::{Deserialize, Serialize};

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
