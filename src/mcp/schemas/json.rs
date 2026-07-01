use serde_json::Value;

pub fn json_extract_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "JSON document string"},
            "pointer": {"type": "string", "default": "", "description": "RFC 6901 JSON Pointer path (e.g., /dependencies/tokio)"},
            "max_output_chars": {"type": "integer", "default": 4000},
            "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "normal"}
        },
        "required": ["text"]
    })
}

pub fn json_compare_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "a": {"type": "string", "description": "First JSON document"},
            "b": {"type": "string", "description": "Second JSON document"},
            "ignore_object_order": {"type": "boolean", "default": true},
            "ignore_array_order": {"type": "boolean", "default": false},
            "numeric_string_equivalence": {"type": "boolean", "default": false},
            "casefold_keys": {"type": "boolean", "default": false},
            "max_diffs": {"type": "integer", "default": 50, "minimum": 0, "maximum": 10000},
            "treat_missing_null_as_equal": {"type": "boolean", "default": false},
            "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "normal"}
        },
        "required": ["a", "b"]
    })
}

pub fn json_shape_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "JSON document string to analyze"},
            "max_depth": {"type": "integer", "default": 4, "description": "Maximum depth for nested structure"},
            "max_keys": {"type": "integer", "default": 100, "description": "Maximum keys to show per object"},
            "max_array_items": {"type": "integer", "default": 5, "description": "Maximum array item previews"}
        },
        "required": ["text"]
    })
}

pub fn json_canonicalize_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "Input JSON string to canonicalize"},
            "sort_keys": {"type": "boolean", "default": true, "description": "Sort object keys alphabetically"},
            "trailing_newline": {"type": "boolean", "default": false, "description": "Add a trailing newline to the canonical form"},
            "indent": {"type": ["integer", "null"], "description": "Indentation spaces (null for minified)"},
            "ensure_ascii": {"type": "boolean", "default": false, "description": "Use ASCII escaping for non-ASCII characters"},
            "detect_duplicate_keys": {"type": "boolean", "default": true, "description": "Report duplicate keys in the input"}
        },
        "required": ["text"]
    })
}

pub fn json_query_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "JSON document string"},
            "pointer": {"type": "string", "default": "", "description": "RFC 6901 JSON Pointer path (e.g., /foo/bar/0)"}
        },
        "required": ["text"]
    })
}

pub fn structured_data_compare_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "a": {"type": "string", "description": "First JSON string"},
            "b": {"type": "string", "description": "Second JSON string"},
            "format": {"type": "string", "enum": ["json"], "default": "json", "description": "Data format (json only for now)"},
            "ignore_object_order": {"type": "boolean", "default": true, "description": "Ignore object key order"},
            "ignore_array_order": {"type": "boolean", "default": false, "description": "Sort arrays before comparison"},
            "max_diffs": {"type": "integer", "default": 50, "description": "Maximum differences to report"}
        },
        "required": ["a", "b"]
    })
}

pub fn json_extract_output() -> Value {
    serde_json::json!({"type":"object","properties":{"valid_json":{"type":"boolean"},"found":{"type":"boolean"},"pointer":{"type":"string"},"value_type":{"type":["string","null"]},"value":{"description":"Extracted value"},"preview":{"type":["string","null"]},"child_keys":{"type":["array","null"],"items":{"type":"string"}},"array_length":{"type":["integer","null"]},"truncated":{"type":"boolean"},"missing_at":{"type":["string","null"]},"reason":{"type":["string","null"]},"available_keys":{"type":["array","null"],"items":{"type":"string"}},"error":{"type":["string","null"]},"line":{"type":["integer","null"]},"column":{"type":["integer","null"]},"summary":{"type":"string"}}})
}

pub fn json_compare_output() -> Value {
    serde_json::json!({"type":"object","properties":{"valid_json_a":{"type":"boolean"},"valid_json_b":{"type":"boolean"},"equal":{"type":"boolean"},"same_type":{"type":"boolean"},"diff_count":{"type":"integer"},"diffs":{"type":"array","description":"List of differences"},"truncated":{"type":"boolean"},"summary":{"type":"string"}}})
}

pub fn json_shape_output() -> Value {
    serde_json::json!({"type":"object","properties":{"valid":{"type":"boolean"},"shape":{"type":["object","null"],"description":"Nested shape structure with type, keys, and counts","properties":{"type":{"type":"string"},"keys":{"type":["object","null"]},"key_count":{"type":["integer","null"]},"item_types":{"type":["array","null"]},"item_count":{"type":["integer","null"]}}},"truncated":{"type":"boolean"},"summary":{"type":"string"}}})
}

pub fn json_canonicalize_output() -> Value {
    serde_json::json!({"type":"object","properties":{"valid":{"type":"boolean"},"canonical":{"type":["string","null"]},"minified":{"type":["string","null"]},"sha256":{"type":["string","null"]},"duplicate_keys":{"type":"array","items":{"type":"string"}},"top_level_type":{"type":["string","null"]},"top_level_keys":{"type":["array","null"],"items":{"type":"string"}},"error":{"type":["string","null"]},"line":{"type":["integer","null"]},"column":{"type":["integer","null"]}}})
}

pub fn json_query_output() -> Value {
    serde_json::json!({"type":"object","properties":{"found":{"type":"boolean"},"pointer":{"type":"string"},"value":{"description":"Extracted value"},"type":{"type":["string","null"]},"missing_at":{"type":["string","null"]},"reason":{"type":["string","null"]},"error":{"type":["string","null"]},"line":{"type":["integer","null"]},"column":{"type":["integer","null"]}}})
}

pub fn structured_data_compare_output() -> Value {
    serde_json::json!({"type":"object","properties":{"equal":{"type":"boolean"},"valid_a":{"type":"boolean"},"valid_b":{"type":"boolean"},"findings":{"type":"array"},"machine_code":{"type":"string"},"summary":{"type":"string"},"subresults":{"type":"object"}}})
}
