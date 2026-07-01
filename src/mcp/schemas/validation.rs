use serde_json::Value;

pub fn validate_brackets_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "Input string"},
            "pairs": {"type": "object", "description": "Bracket pair mapping (default: () [] {} <>)"}
        },
        "required": ["text"]
    })
}

pub fn validate_json_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {"text": {"type": "string", "description": "Input string to validate as JSON"}},
        "required": ["text"]
    })
}

pub fn validate_toml_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "TOML document string to validate"},
            "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "normal"}
        },
        "required": ["text"]
    })
}

pub fn validate_schema_light_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "JSON document to validate"},
            "schema": {"type": "object", "description": "Schema to validate against"},
            "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "normal"}
        },
        "required": ["text", "schema"]
    })
}

pub fn validate_brackets_output() -> Value {
    serde_json::json!({"type":"object","properties":{"balanced":{"type":"boolean"},"unmatched_openers":{"type":"array"},"unmatched_closers":{"type":"array"}}})
}

pub fn validate_json_output() -> Value {
    serde_json::json!({"type":"object","properties":{"valid":{"type":"boolean"},"error":{"type":["string","null"]},"line":{"type":["integer","null"]},"column":{"type":["integer","null"]},"position":{"type":["integer","null"]},"type":{"type":["string","null"]},"top_level_keys":{"type":["array","null"],"items":{"type":"string"}}}})
}

pub fn validate_toml_output() -> Value {
    serde_json::json!({"type":"object","properties":{"valid":{"type":"boolean"},"error":{"type":["string","null"]},"line":{"type":["integer","null"]},"column":{"type":["integer","null"]},"position":{"type":["integer","null"]},"type":{"type":["string","null"]},"top_level_keys":{"type":["array","null"]},"tables":{"type":["array","null"]}}})
}

pub fn validate_schema_light_output() -> Value {
    serde_json::json!({"type":"object","properties":{"valid":{"type":"boolean"},"violations":{"type":"array","description":"Schema violations with path and message","items":{"type":"object","properties":{"path":{"type":"string"},"message":{"type":"string"},"value_type":{"type":["string","null"]},"expected_type":{"type":["string","null"]}}}},"truncated":{"type":"boolean"},"summary":{"type":"string"}}})
}
