use serde_json::Value;

pub fn runtime_diagnostics_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {},
        "additionalProperties": false
    })
}

pub fn runtime_diagnostics_output() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "active_profile": { "type": "string" },
            "tool_count": { "type": "integer" },
            "route_critical_tools": { "type": "array", "items": { "type": "string" } },
            "profile_tool_count": { "type": "integer" },
            "compatibility_mode": { "type": "string" },
            "budget_tier_summary": { "type": "object" },
            "known_env_vars": { "type": "array", "items": { "type": "string" } },
            "generated_doc_command": { "type": "string" },
            "parity_available": { "type": "boolean" }
        },
        "required": ["active_profile", "tool_count"]
    })
}
