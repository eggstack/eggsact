use serde_json::Value;

pub fn toml_shape_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "TOML document string"},
            "max_tables": {"type": "integer", "default": 100, "minimum": 1, "maximum": 100000, "description": "Maximum tables to return"},
            "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "normal"}
        },
        "required": ["text"]
    })
}

pub fn toml_shape_output() -> Value {
    serde_json::json!({"type":"object","properties":{"valid":{"type":"boolean"},"top_level_keys":{"type":["array","null"],"items":{"type":"string"}},"tables":{"type":["array","null"],"items":{"type":"string"}},"truncated":{"type":"boolean"},"summary":{"type":"string"}}})
}
