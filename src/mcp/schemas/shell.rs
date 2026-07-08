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
            "platform": {"type": "string", "enum": ["posix", "windows", "auto"], "default": "posix", "description": "Target platform: 'posix' (fully supported), 'auto' (currently resolves to POSIX behavior), or 'windows' (recognized but unsupported — returns UNSUPPORTED_FEATURE)"},
            "policy": {"type": "string", "enum": ["default", "strict", "permissive"], "default": "default", "description": "Built-in analysis policy"},
            "policy_config": {
                "type": "object",
                "description": "Structured policy configuration that refines or overrides the built-in policy enum",
                "properties": {
                    "allow_commands": {"type": "array", "items": {"type": "string"}, "description": "Explicit allow list of programs"},
                    "deny_commands": {"type": "array", "items": {"type": "string"}, "description": "Explicit deny list of programs (overrides allow)"},
                    "allow_subcommands": {"type": "object", "description": "Per-program allowed subcommands", "additionalProperties": {"type": "array", "items": {"type": "string"}}},
                    "deny_subcommands": {"type": "object", "description": "Per-program denied subcommands (overrides allow)", "additionalProperties": {"type": "array", "items": {"type": "string"}}},
                    "allow_network": {"type": "boolean", "description": "Allow network access (default false)"},
                    "allow_filesystem_write": {"type": "boolean", "description": "Allow filesystem writes (default false)"},
                    "allow_process_control": {"type": "boolean", "description": "Allow process control (default false)"},
                    "allow_env_mutation": {"type": "boolean", "description": "Allow environment variable mutation (default false)"},
                    "max_command_length": {"type": "integer", "description": "Maximum command length in characters", "default": 10000}
                }
            },
            "working_directory": {"type": "string", "description": "Working directory context (informational)"}
        },
        "required": ["command"]
    })
}

pub fn shell_split_output() -> Value {
    serde_json::json!({"type":"object","properties":{"parse_ok":{"type":"boolean","description":"True if the command parsed successfully"},"argv":{"type":"array","items":{"type":"string"},"description":"Parsed argument tokens"},"argc":{"type":"integer","description":"Number of arguments"},"features":{"type":"object","description":"Detected risky features","properties":{"has_pipe":{"type":"boolean"},"has_redirection":{"type":"boolean"},"has_command_substitution":{"type":"boolean"},"has_variable_expansion":{"type":"boolean"},"has_glob_pattern":{"type":"boolean"},"has_control_operator":{"type":"boolean"},"has_background":{"type":"boolean"},"has_unbalanced_quotes":{"type":"boolean"}}},"findings":{"type":"array","items":{"type":"string"},"description":"Analysis notes and warnings"}}})
}

pub fn shell_quote_join_output() -> Value {
    serde_json::json!({"type":"object","properties":{"command":{"type":"string","description":"Safely quoted command string"},"roundtrip_ok":{"type":"boolean","description":"True if shell_split(quote_join(argv)) produces equivalent argv"},"findings":{"type":"array","items":{"type":"string"},"description":"Analysis notes"}}})
}

pub fn argv_compare_output() -> Value {
    serde_json::json!({"type":"object","properties":{"argv_equal":{"type":"boolean","description":"True if parsed argv lists are identical"},"left_argv":{"type":"array","items":{"type":"string"},"description":"Resolved left argv"},"right_argv":{"type":"array","items":{"type":"string"},"description":"Resolved right argv"},"first_difference":{"type":["integer","null"],"description":"Index of first differing token, or null if equal"},"findings":{"type":"array","items":{"type":"string"},"description":"Analysis notes"}}})
}

pub fn command_preflight_output() -> Value {
    serde_json::json!({"type":"object","properties":{"verdict":{"type":"string","enum":["allow","review","block"]},"command":{"type":"string"},"platform":{"type":"string"},"policy":{"type":"string"},"program":{"type":"string","description":"Extracted program name"},"subcommand":{"type":"string","description":"Extracted subcommand"},"features":{"type":"array","items":{"type":"string"},"description":"Detected risky shell features"},"findings":{"type":"array"},"matched_rules":{"type":"array","items":{"type":"string"},"description":"Policy rules that matched"},"machine_code":{"type":"string"},"summary":{"type":"string"},"subresults":{"type":"object"},"working_directory":{"type":"string"}}})
}
