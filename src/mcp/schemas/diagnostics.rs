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
            "active_audience": { "type": "string" },
            "tool_count": { "type": "integer" },
            "route_critical_tools": { "type": "array", "items": { "type": "string" } },
            "profile_tool_count": { "type": "integer" },
            "model_visible_tool_count": { "type": "integer" },
            "harness_visible_tool_count": { "type": "integer" },
            "compatibility_mode": { "type": "string" },
            "budget_tier_summary": { "type": "object" },
            "runtime": { "type": "object" },
            "known_env_vars": { "type": "array", "items": { "type": "string" } },
            "generated_doc_command": { "type": "string" },
            "verification_command": { "type": "string" },
            "generated_data": { "type": "object" },
            "parity_available": { "type": "boolean" }
        },
        "required": ["active_profile", "tool_count"]
    })
}

pub fn profile_inspect_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "profile": {
                "type": "string",
                "description": "Profile name to inspect (e.g., 'full', 'codegg_core_min')."
            }
        },
        "required": ["profile"],
        "additionalProperties": false
    })
}

pub fn profile_inspect_output() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "tool_count": { "type": "integer" },
            "model_visible_tool_count": { "type": "integer" },
            "harness_visible_tool_count": { "type": "integer" },
            "intended_audience": { "type": "string", "enum": ["model", "harness", "debug", "mixed", "unknown"] },
            "purpose": { "type": "string" },
            "contains_route_critical_tools": { "type": "boolean" },
            "contains_harness_only_tools": { "type": "boolean" },
            "representative_tools": { "type": "array", "items": { "type": "string" } },
            "warnings": { "type": "array", "items": { "type": "string" } }
        },
        "required": ["name", "tool_count"]
    })
}

pub fn tool_availability_explain_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "tool": {
                "type": "string",
                "description": "Tool name to check availability for."
            },
            "profile": {
                "type": "string",
                "description": "Profile to check against. Defaults to the active runtime profile."
            },
            "audience": {
                "type": "string",
                "description": "Audience to check callability for. Defaults to the active runtime audience."
            }
        },
        "required": ["tool"],
        "additionalProperties": false
    })
}

pub fn tool_availability_explain_output() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "exists": { "type": "boolean" },
            "available_in_profile": { "type": "boolean" },
            "callable_by_audience": { "type": "boolean" },
            "exposure": { "type": "string" },
            "profiles": { "type": "array", "items": { "type": "string" } },
            "reason": { "type": "string" },
            "suggested_tool": { "type": "string" },
            "suggested_profile": { "type": "string" },
            "suggested_audience": { "type": "string" }
        },
        "required": ["exists", "available_in_profile", "callable_by_audience", "reason"]
    })
}
