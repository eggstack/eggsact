// Re-export from the new focused modules for backward compatibility.
// New code should import directly from the specific modules.

pub use crate::mcp::protocol::{
    Capabilities, InitializeResult, JsonRpcError, JsonRpcErrorDetail, JsonRpcRequest,
    JsonRpcResponse, ServerInfo, ToolsCapability,
};
pub use crate::mcp::response::{sanitize_error, ToolResponse};
