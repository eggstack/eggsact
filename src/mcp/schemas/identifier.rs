use serde_json::Value;

pub fn identifier_analyze_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string"},
            "languages": {"type": "array", "items": {"type": "string"}, "default": ["python", "rust", "javascript", "env"], "description": "Languages to check (python, rust, javascript, env)"},
            "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "normal"}
        },
        "required": ["text"]
    })
}

pub fn identifier_inspect_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "identifiers": {"type": "array", "items": {"type": "string"}, "description": "List of identifier strings to inspect", "maxItems": 10000},
            "language": {"type": "string", "enum": ["generic", "python", "rust", "javascript", "typescript", "json_key"], "default": "generic", "description": "Language for validation"},
            "normalization": {"type": "string", "enum": ["raw", "NFC", "NFD", "NFKC", "NFKD"], "default": "NFC", "description": "Unicode normalization form"},
            "casefold": {"type": "boolean", "default": false, "description": "Apply casefolding for collision detection"},
            "check_confusables": {"type": "boolean", "default": true, "description": "Check for confusable characters"}
        },
        "required": ["identifiers"]
    })
}

pub fn identifier_table_inspect_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "identifiers": {"type": "array", "items": {"type": "object", "properties": {"name": {"type": "string", "description": "Identifier name (required)"}, "kind": {"type": "string", "description": "Optional kind/category"}, "file": {"type": "string", "description": "Source file path"}, "line": {"type": "integer", "description": "Line number"}}, "required": ["name"]}, "description": "List of identifier entries to inspect", "maxItems": 10000},
            "language": {"type": "string", "enum": ["generic", "python", "rust", "javascript", "typescript", "json_key"], "default": "python", "description": "Target language for reserved keyword checking"},
            "checks": {"type": "array", "items": {"type": "string"}, "description": "Subset of checks: casefold, normalization, confusable, style, reserved, mixed_style"}
        },
        "required": ["identifiers"]
    })
}

pub fn identifier_analyze_output() -> Value {
    serde_json::json!({"type":"object","properties":{"text":{"type":"string"},"classification":{"type":"string"},"python_valid":{"type":"boolean"},"python_keyword":{"type":"boolean"},"rust_valid":{"type":["boolean","null"]},"javascript_valid":{"type":["boolean","null"]},"env_valid":{"type":"boolean"},"suggestions":{"type":"object","description":"Map of language to suggested name"},"warnings":{"type":"array","items":{"type":"string"}},"summary":{"type":"string"}}})
}

pub fn identifier_inspect_output() -> Value {
    serde_json::json!({"type":"object","properties":{"identifiers":{"type":"array","description":"Per-identifier analysis with raw, normalized, valid, scripts, and issues"},"collisions":{"type":"array","description":"Detected collisions between identifiers"}}})
}

pub fn identifier_table_inspect_output() -> Value {
    serde_json::json!({"type":"object","properties":{"count":{"type":"integer","description":"Number of identifiers inspected"},"collisions":{"type":"array","description":"Detected collisions","items":{"type":"object","properties":{"kind":{"type":"string"},"names":{"type":"array","items":{"type":"string"}},"detail":{"type":"string"}}}},"reserved_keyword_hits":{"type":"array","description":"Identifiers matching reserved keywords","items":{"type":"object","properties":{"name":{"type":"string"},"language":{"type":"string"},"file":{"type":"string"},"line":{"type":"integer"}}}},"mixed_style_groups":{"type":"array","description":"Groups with mixed naming styles","items":{"type":"object","properties":{"stripped":{"type":"string"},"names":{"type":"array","items":{"type":"string"}},"styles":{"type":"array","items":{"type":"string"}}}}},"findings":{"type":"array","items":{"type":"string"}}}})
}
