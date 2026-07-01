use serde_json::Value;

pub fn shell_split_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "command": {"type": "string", "description": "The shell command string to parse"},
            "shell": {"type": "string", "enum": ["posix"], "default": "posix", "description": "Shell dialect (only posix is supported)"},
            "detect_risky_features": {"type": "boolean", "default": true, "description": "Whether to detect risky lexical features"}
        },
        "required": ["command"]
    })
}

pub fn shell_quote_join_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "argv": {"type": "array", "items": {"type": "string"}, "description": "List of argument strings to join", "maxItems": 10000},
            "shell": {"type": "string", "enum": ["posix"], "default": "posix", "description": "Shell dialect (only posix is supported)"}
        },
        "required": ["argv"]
    })
}

pub fn argv_compare_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "left_command": {"type": "string", "description": "Left command string to parse and compare"},
            "right_command": {"type": "string", "description": "Right command string to parse and compare"},
            "left_argv": {"type": "array", "items": {"type": "string"}, "description": "Left pre-parsed argv list", "maxItems": 10000},
            "right_argv": {"type": "array", "items": {"type": "string"}, "description": "Right pre-parsed argv list", "maxItems": 10000},
            "shell": {"type": "string", "enum": ["posix"], "default": "posix", "description": "Shell dialect (only posix is supported)"}
        },
        "required": []
    })
}

pub fn command_preflight_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "command": {"type": "string", "description": "Command string to analyze"},
            "platform": {"type": "string", "enum": ["posix", "windows", "auto"], "default": "posix", "description": "Target platform"},
            "policy": {"type": "string", "enum": ["default", "strict", "permissive"], "default": "default", "description": "Analysis policy"},
            "working_directory": {"type": "string", "description": "Working directory context (informational)"}
        },
        "required": ["command"]
    })
}

pub fn shell_split_output() -> Value {
    serde_json::json!({"type":"object","properties":{"parse_ok":{"type":"boolean","description":"True if the command parsed successfully"},"argv":{"type":"array","items":{"type":"string"},"description":"Parsed argument tokens"},"argc":{"type":"integer","description":"Number of arguments"},"features":{"type":"object","description":"Detected risky features","properties":{"has_pipe":{"type":"boolean"},"has_redirection":{"type":"boolean"},"has_command_substitution":{"type":"boolean"},"has_variable_expansion":{"type":"boolean"},"has_glob_pattern":{"type":"boolean"},"has_control_operator":{"type":"boolean"},"has_unbalanced_quotes":{"type":"boolean"}}},"findings":{"type":"array","items":{"type":"string"},"description":"Analysis notes and warnings"}}})
}

pub fn shell_quote_join_output() -> Value {
    serde_json::json!({"type":"object","properties":{"command":{"type":"string","description":"Safely quoted command string"},"roundtrip_ok":{"type":"boolean","description":"True if shell_split(quote_join(argv)) produces equivalent argv"},"findings":{"type":"array","items":{"type":"string"},"description":"Analysis notes"}}})
}

pub fn argv_compare_output() -> Value {
    serde_json::json!({"type":"object","properties":{"argv_equal":{"type":"boolean","description":"True if parsed argv lists are identical"},"left_argv":{"type":"array","items":{"type":"string"},"description":"Resolved left argv"},"right_argv":{"type":"array","items":{"type":"string"},"description":"Resolved right argv"},"first_difference":{"type":"integer","description":"Index of first differing token, or null if equal"},"findings":{"type":"array","items":{"type":"string"},"description":"Analysis notes"}}})
}

pub fn command_preflight_output() -> Value {
    serde_json::json!({"type":"object","properties":{"verdict":{"type":"string","enum":["allow","review","block"]},"command":{"type":"string"},"platform":{"type":"string"},"policy":{"type":"string"},"findings":{"type":"array"},"machine_code":{"type":"string"},"summary":{"type":"string"},"subresults":{"type":"object"}}})
}
