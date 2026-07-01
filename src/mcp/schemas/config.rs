use serde_json::Value;

pub fn dotenv_validate_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": ".env file content to validate"},
            "allow_export": {"type": "boolean", "default": true, "description": "Allow export KEY=VALUE syntax"},
            "key_pattern": {"type": "string", "default": "^[A-Za-z_][A-Za-z0-9_]*$", "description": "Regex pattern keys must match"},
            "duplicate_policy": {"type": "string", "enum": ["warn", "error", "allow"], "default": "warn", "description": "How to handle duplicate keys"}
        },
        "required": ["text"]
    })
}

pub fn ini_validate_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "INI file content to validate"},
            "duplicate_policy": {"type": "string", "enum": ["warn", "error", "allow"], "default": "warn", "description": "How to handle duplicate keys/sections"}
        },
        "required": ["text"]
    })
}

pub fn config_preflight_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "Config text to validate"},
            "format": {"type": "string", "enum": ["auto", "json", "toml", "dotenv", "ini", "cargo_toml"], "default": "auto", "description": "Config format (auto-detect if not specified)"},
            "schema": {"type": "object", "description": "Optional JSON schema for validation"},
            "strict": {"type": "boolean", "default": false, "description": "Strict validation mode"}
        },
        "required": ["text"]
    })
}

pub fn dotenv_validate_output() -> Value {
    serde_json::json!({"type":"object","properties":{"parse_ok":{"type":"boolean","description":"True if no parse errors found"},"entries":{"type":"array","description":"Parsed entries with key, value, quote_style, line"},"duplicates":{"type":"array","description":"Duplicate key entries with line numbers"},"invalid_lines":{"type":"array","description":"Lines that failed to parse"},"requires_quoting":{"type":"array","description":"Keys whose values contain spaces and should be quoted"},"contains_expansion_syntax":{"type":"array","description":"Keys with ${VAR} or $VAR expansion syntax"},"findings":{"type":"array","items":{"type":"string"},"description":"Human-readable findings"}}})
}

pub fn ini_validate_output() -> Value {
    serde_json::json!({"type":"object","properties":{"parse_ok":{"type":"boolean","description":"True if no parse errors found"},"sections":{"type":"array","description":"Ordered list of section names"},"keys_by_section":{"type":"object","description":"Keys grouped by section"},"duplicates":{"type":"array","description":"Duplicate keys/sections with line numbers"},"invalid_lines":{"type":"array","description":"Lines that failed to parse"},"findings":{"type":"array","items":{"type":"string"},"description":"Human-readable findings"}}})
}

pub fn config_preflight_output() -> Value {
    serde_json::json!({"type":"object","properties":{"valid":{"type":"boolean"},"verdict":{"type":"string","enum":["valid","valid_with_warnings","invalid"]},"format":{"type":"string"},"findings":{"type":"array"},"machine_code":{"type":"string"},"summary":{"type":"string"},"subresults":{"type":"object"}}})
}
