use serde_json::Value;

pub fn validate_regex_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "pattern": {"type": "string", "description": "Regular expression pattern", "maxLength": 1000},
            "samples": {"type": "array", "items": {"type": "string"}, "description": "List of strings to test against", "maxItems": 100},
            "flags": {"type": "array", "items": {"type": "string"}, "description": "Flag names (IGNORECASE, MULTILINE, etc.)", "maxItems": 10},
            "ignore_case": {"type": "boolean", "default": false, "description": "Use IGNORECASE flag"},
            "multiline": {"type": "boolean", "default": false, "description": "Use MULTILINE flag"},
            "dotall": {"type": "boolean", "default": false, "description": "Use DOTALL flag"},
            "ascii": {"type": "boolean", "default": false, "description": "Use ASCII flag"}
        },
        "required": ["pattern", "samples"]
    })
}

pub fn regex_finditer_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "pattern": {"type": "string", "description": "Regular expression pattern", "maxLength": 1000},
            "text": {"type": "string", "description": "Input string to search"},
            "flags": {"type": "array", "items": {"type": "string"}, "description": "Flag names (IGNORECASE, MULTILINE, DOTALL, etc.)", "maxItems": 10},
            "max_matches": {"type": "integer", "default": 100, "maximum": 1000, "description": "Maximum matches to return"},
            "include_line_column": {"type": "boolean", "default": true, "description": "Include line and column info"},
            "include_groups": {"type": "boolean", "default": true, "description": "Include capture groups"}
        },
        "required": ["pattern", "text"]
    })
}

pub fn regex_safety_check_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "pattern": {"type": "string", "description": "Regular expression pattern to check"}
        },
        "required": ["pattern"]
    })
}

pub fn validate_regex_output() -> Value {
    serde_json::json!({"type":"object","properties":{"valid_pattern":{"type":"boolean"},"results":{"type":"array"},"error":{"type":["string","null"]},"flags_used":{"type":"object"}}})
}

pub fn regex_finditer_output() -> Value {
    serde_json::json!({"type":"object","properties":{"valid_pattern":{"type":"boolean"},"matches":{"type":"array","description":"List of regex matches with positions and groups"},"truncated":{"type":"boolean"},"match_count":{"type":"integer"},"error":{"type":["string","null"]}}})
}

pub fn regex_safety_check_output() -> Value {
    serde_json::json!({"type":"object","properties":{"valid_pattern":{"type":"boolean"},"risk":{"type":"string","enum":["low","medium","high"]},"findings":{"type":"array","description":"Safety findings with kind, span, and message","items":{"type":"object","properties":{"kind":{"type":"string"},"span":{"type":"array","items":{"type":"integer"}},"message":{"type":"string"}}}}}})
}
