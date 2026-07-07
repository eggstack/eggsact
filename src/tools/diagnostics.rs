use crate::mcp::machine_codes;
use crate::mcp::registry;
use crate::mcp::response::ToolResponse;
use crate::mcp::runtime;
use serde_json::{json, Value};

pub fn runtime_diagnostics(_args: &Value) -> ToolResponse {
    let active_profile = runtime::get_active_profile();
    let tool_count = registry::tool_count();
    let route_critical_tools: Vec<&str> = registry::ROUTE_CRITICAL_TOOLS.to_vec();
    let profile_tool_count = registry::tools_for_profile(&active_profile).len();

    let all_tools_list = registry::all_tools_vec();
    let mut cheap = 0u32;
    let mut moderate = 0u32;
    let mut heavy = 0u32;
    for spec in all_tools_list {
        match spec.cost {
            registry::ToolCost::Cheap => cheap += 1,
            registry::ToolCost::Moderate => moderate += 1,
            registry::ToolCost::Heavy => heavy += 1,
        }
    }

    let result = json!({
        "active_profile": active_profile,
        "tool_count": tool_count,
        "route_critical_tools": route_critical_tools,
        "profile_tool_count": profile_tool_count,
        "compatibility_mode": "eggcalc_python",
        "budget_tier_summary": {
            "cheap": cheap,
            "moderate": moderate,
            "heavy": heavy,
        },
        "known_env_vars": [
            "EGGCALC_NO_CONFIG",
            "EGGCALC_MCP_PROFILE",
            "EGGCALC_MCP_AUDIENCE",
            "EGGCALC_MCP_SCHEMA_DETAIL",
        ],
        "generated_doc_command": "cargo run --bin generate-docs",
        "parity_available": std::path::Path::new("../eggcalc").exists(),
    });

    ToolResponse::success(result, Some("runtime_diagnostics"))
        .with_tool("runtime_diagnostics")
        .with_machine_code(machine_codes::OK)
}
