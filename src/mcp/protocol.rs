use serde::{Deserialize, Serialize};
use serde_json::Value;

// ═══════════════════════════════════════════════════════════════════════════════
// JSON-RPC error constructors
// ═══════════════════════════════════════════════════════════════════════════════

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

/// JSON-RPC error with optional structured `data` field for lifecycle errors.
pub fn json_rpc_error_with_data(
    code: i32,
    message: impl Into<String>,
    data: Option<Value>,
    id: Option<Value>,
) -> Value {
    let detail = JsonRpcErrorDetailWithData {
        code,
        message: message.into(),
        data,
    };
    serde_json::to_value(JsonRpcErrorWithData {
        jsonrpc: "2.0".to_string(),
        error: detail,
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

/// Invalid params (JSON-RPC -32602). Used for malformed modern `_meta`
/// envelopes: missing required fields or wrong types.
pub fn invalid_params(message: impl Into<String>, id: Option<Value>) -> Value {
    json_rpc_error(-32602, message, id)
}

/// Unsupported protocol version (`-32022` per 2026-07-28).
///
/// Returned when a modern request explicitly names a revision the server does
/// not implement. Lists all supported revisions so the client can retry with
/// a mutually supported version instead of silently entering legacy state.
pub fn unsupported_protocol_version(requested: &str, id: Option<Value>) -> Value {
    json_rpc_error_with_data(
        -32022,
        "Unsupported protocol version",
        Some(serde_json::json!({
            "supported": crate::mcp::runtime::SUPPORTED_PROTOCOL_VERSIONS,
            "requested": requested,
        })),
        id,
    )
}

/// Attach (or replace) the JSON-RPC `id` on an already-built error value.
///
/// `parse_modern_request_meta` builds envelope errors without an id; the
/// dispatcher patches the originating request id here so correlation is
/// preserved without threading ids through the parser.
pub fn with_request_id(mut error_value: Value, id: Option<Value>) -> Value {
    if let Some(obj) = error_value.as_object_mut() {
        obj.insert("id".to_string(), id.unwrap_or(Value::Null));
    }
    error_value
}

/// Build the reserved result `_meta` object carrying server identity.
///
/// Servers SHOULD include `io.modelcontextprotocol/serverInfo` on every
/// modern result. Identity is advisory (display/logging/debugging) and must
/// not drive behavior or security decisions.
pub fn server_info_meta() -> Value {
    serde_json::json!({
        crate::mcp::runtime::META_SERVER_INFO: {
            "name": crate::mcp::runtime::MCP_SERVER_NAME,
            "version": env!("CARGO_PKG_VERSION"),
        }
    })
}

/// Lifecycle error: method called before initialization.
pub fn not_initialized(method: &str, id: Option<Value>) -> Value {
    json_rpc_error_with_data(
        -32600,
        format!(
            "Server not initialized. Send 'initialize' request first (got '{}')",
            method
        ),
        Some(serde_json::json!({
            "code": "NOT_INITIALIZED",
            "expected_method": "initialize",
        })),
        id,
    )
}

/// Lifecycle error: duplicate initialize request.
pub fn already_initialized(id: Option<Value>) -> Value {
    json_rpc_error_with_data(
        -32600,
        "Server already initialized",
        Some(serde_json::json!({
            "code": "ALREADY_INITIALIZED",
        })),
        id,
    )
}

/// Unsupported-version response for a classification mismatch where the
/// inbound message was claim-less and therefore has no truthful requested
/// version to report.
pub fn unsupported_protocol_version_without_requested(id: Option<Value>) -> Value {
    json_rpc_error_with_data(
        -32022,
        "Unsupported protocol version",
        Some(serde_json::json!({
            "supported": crate::mcp::runtime::SUPPORTED_PROTOCOL_VERSIONS,
        })),
        id,
    )
}

// ═══════════════════════════════════════════════════════════════════════════════
// JSON-RPC types
// ═══════════════════════════════════════════════════════════════════════════════

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
pub struct JsonRpcErrorWithData {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub error: JsonRpcErrorDetailWithData,
}

#[derive(Serialize, Debug)]
pub struct JsonRpcErrorDetailWithData {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Initialize parameters (Workstream 1)
// ═══════════════════════════════════════════════════════════════════════════════

/// Client implementation info sent during initialize.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ImplementationInfo {
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
}

/// Client capabilities declared during initialize.
///
/// Unknown fields are tolerated for forward compatibility; known fields are
/// type-checked.
#[derive(Deserialize, Debug, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClientCapabilities {
    /// Whether the client supports roots (file system roots).
    #[serde(default)]
    pub roots: Option<Value>,
    /// Whether the client supports sampling (LLM sampling requests).
    #[serde(default)]
    pub sampling: Option<Value>,
    /// Whether the client supports elicitation.
    #[serde(default)]
    pub elicitation: Option<Value>,
    /// Experimental capabilities (unknown shape — tolerated).
    #[serde(default)]
    pub experimental: Option<Value>,
}

/// Parameters for the `initialize` request.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    /// The MCP protocol version requested by the client.
    pub protocol_version: String,
    /// Client capabilities.
    #[serde(default)]
    pub capabilities: ClientCapabilities,
    /// Client implementation name and optional version.
    pub client_info: ImplementationInfo,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Server capabilities (Workstream 4)
// ═══════════════════════════════════════════════════════════════════════════════

/// Server capabilities advertised in the initialize response.
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ServerCapabilities {
    /// Tool capabilities (list changed notifications).
    pub tools: ToolsCapability,
    /// eggsact-specific experimental extensions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<ExperimentalCapabilities>,
}

/// eggsact extension capabilities.
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExperimentalCapabilities {
    /// eggsact-specific extensions.
    pub eggsact: EggsactExtensions,
}

/// eggsact extension flags.
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EggsactExtensions {
    /// Whether profiles/list is supported.
    pub profiles: bool,
    /// Whether schema_detail filtering is supported.
    pub schema_detail: bool,
    /// Whether audience filtering is supported.
    pub audience_filtering: bool,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Initialize result (revised)
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    pub server_info: ServerInfo,
    /// Concise cross-tool instructions (additive; few hundred bytes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Legacy compatibility types (preserved for backward compat)
// ═══════════════════════════════════════════════════════════════════════════════

/// Legacy `Capabilities` type for backward-compatible serialization.
/// Only used if code still references the old shape; prefer `ServerCapabilities`.
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub tools: ToolsCapability,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ToolsCapability {
    pub list_changed: bool,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Modern 2026-07-28 discovery (server/discover)
// ═══════════════════════════════════════════════════════════════════════════════

/// `server/discover` has no body parameters beyond the standard per-request
/// `_meta` envelope. This empty marker exists so the wire contract is typed;
/// dispatch reads `_meta` via `parse_modern_request_meta` instead.
#[derive(Deserialize, Debug, Default)]
pub struct DiscoverRequest {
    #[serde(default)]
    pub _meta: Option<Value>,
}

/// Modern discovery result per the final 2026-07-28 schema.
///
/// - `supportedVersions`: all revisions the server implements, in deterministic
///   preference order (modern first).
/// - `capabilities`: same tool capability shape as legacy initialize.
/// - `instructions`: concise cross-tool guidance (few hundred bytes).
/// - `ttlMs` / `cacheScope`: required cache hints for the modern result.
/// - `_meta[io.modelcontextprotocol/serverInfo]`: modern server identity; the
///   body carries no `serverInfo` member.
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverResult {
    pub result_type: String,
    pub supported_versions: Vec<String>,
    pub capabilities: ServerCapabilities,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    pub ttl_ms: u64,
    pub cache_scope: String,
    #[serde(rename = "_meta")]
    pub meta: Value,
}

impl DiscoverResult {
    pub fn current(capabilities: ServerCapabilities) -> Self {
        Self {
            result_type: "complete".to_string(),
            supported_versions: crate::mcp::runtime::SUPPORTED_PROTOCOL_VERSIONS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            capabilities,
            instructions: Some(crate::mcp::runtime::SERVER_INSTRUCTIONS.to_string()),
            ttl_ms: crate::mcp::runtime::MODERN_CACHE_TTL_MS,
            cache_scope: crate::mcp::runtime::MODERN_CACHE_SCOPE.to_string(),
            meta: server_info_meta(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_initialize_params_minimal() {
        let params: InitializeParams = serde_json::from_value(serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "test-client"}
        }))
        .unwrap();
        assert_eq!(params.protocol_version, "2024-11-05");
        assert_eq!(params.client_info.name, "test-client");
        assert!(params.client_info.version.is_none());
    }

    #[test]
    fn deserialize_initialize_params_full() {
        let params: InitializeParams = serde_json::from_value(serde_json::json!({
            "protocolVersion": "2025-11-25",
            "capabilities": {
                "roots": {"listChanged": true},
                "sampling": {}
            },
            "clientInfo": {"name": "my-client", "version": "1.2.3"}
        }))
        .unwrap();
        assert_eq!(params.protocol_version, "2025-11-25");
        assert_eq!(params.client_info.name, "my-client");
        assert_eq!(params.client_info.version.as_deref(), Some("1.2.3"));
        assert!(params.capabilities.roots.is_some());
        assert!(params.capabilities.sampling.is_some());
    }

    #[test]
    fn deserialize_initialize_params_unknown_capabilities_tolerated() {
        let params: InitializeParams = serde_json::from_value(serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {"futureCapability": true},
            "clientInfo": {"name": "test"}
        }))
        .unwrap();
        assert_eq!(params.protocol_version, "2024-11-05");
    }

    #[test]
    fn initialize_result_serialization() {
        let result = InitializeResult {
            protocol_version: "2024-11-05".to_string(),
            capabilities: ServerCapabilities {
                tools: ToolsCapability {
                    list_changed: false,
                },
                experimental: Some(ExperimentalCapabilities {
                    eggsact: EggsactExtensions {
                        profiles: true,
                        schema_detail: true,
                        audience_filtering: true,
                    },
                }),
            },
            server_info: ServerInfo {
                name: "eggsact".to_string(),
                version: "1.1.5".to_string(),
            },
            instructions: Some("test instructions".to_string()),
        };
        let value = serde_json::to_value(&result).unwrap();
        assert_eq!(value["protocolVersion"], "2024-11-05");
        assert_eq!(value["capabilities"]["tools"]["listChanged"], false);
        assert_eq!(
            value["capabilities"]["experimental"]["eggsact"]["profiles"],
            true
        );
        assert_eq!(
            value["capabilities"]["experimental"]["eggsact"]["schemaDetail"],
            true
        );
        assert_eq!(
            value["capabilities"]["experimental"]["eggsact"]["audienceFiltering"],
            true
        );
        assert_eq!(value["serverInfo"]["name"], "eggsact");
    }

    #[test]
    fn lifecycle_error_structures() {
        let err = not_initialized("tools/list", Some(Value::Number(1.into())));
        let parsed: Value = serde_json::from_str(&err.to_string()).unwrap();
        assert_eq!(parsed["error"]["code"], -32600);
        assert!(parsed["error"]["message"]
            .as_str()
            .unwrap()
            .contains("not initialized"));
        let data = parsed["error"]["data"].as_object().unwrap();
        assert_eq!(data["code"], "NOT_INITIALIZED");

        let err = already_initialized(Some(Value::Number(2.into())));
        let parsed: Value = serde_json::from_str(&err.to_string()).unwrap();
        assert_eq!(parsed["error"]["code"], -32600);
        assert_eq!(parsed["error"]["data"]["code"], "ALREADY_INITIALIZED");
    }
}
