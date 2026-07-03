use serde_json::Value;

pub fn repo_manifest_inspect_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "paths": {
                "type": "array",
                "items": {"type": "string"},
                "description": "List of relative file paths in the repository",
                "maxItems": 200
            },
            "file_summaries": {
                "type": "object",
                "description": "Optional map from path to small text sample or complete text for known manifests",
                "additionalProperties": {"type": "string"}
            },
            "workspace_root": {
                "type": "string",
                "description": "Optional workspace root for path-scope context"
            },
            "max_paths": {
                "type": "integer",
                "default": 200,
                "description": "Maximum number of paths to process"
            }
        },
        "required": ["paths"]
    })
}

pub fn repo_manifest_inspect_output() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "project_types": {
                "type": "array",
                "items": {"type": "string"},
                "enum": ["rust", "python", "node", "go", "mixed", "unknown"],
                "description": "Detected project types"
            },
            "manifest_paths": {
                "type": "object",
                "description": "Detected manifest files by category",
                "properties": {
                    "rust": {"type": "array", "items": {"type": "string"}},
                    "python": {"type": "array", "items": {"type": "string"}},
                    "node": {"type": "array", "items": {"type": "string"}},
                    "go": {"type": "array", "items": {"type": "string"}},
                    "other": {"type": "array", "items": {"type": "string"}}
                }
            },
            "config_paths": {
                "type": "array",
                "items": {"type": "string"},
                "description": "Detected configuration file paths"
            },
            "lockfile_paths": {
                "type": "array",
                "items": {"type": "string"},
                "description": "Detected lockfile paths"
            },
            "tool_hints": {
                "type": "object",
                "description": "Suggested tools and commands for this repo type",
                "properties": {
                    "inspect_tools": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Recommended inspection tools (e.g. cargo_toml_inspect)"
                    },
                    "commands": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Recommended build/test commands"
                    }
                }
            },
            "verdict": {
                "type": "string",
                "enum": ["allow", "review", "block"],
                "description": "Whether repo structure is deterministically classifiable"
            },
            "findings": {
                "type": "array",
                "items": {"type": "object"},
                "description": "Structured findings"
            }
        }
    })
}

pub fn config_file_inspect_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "file_path": {
                "type": "string",
                "description": "Path to the config file"
            },
            "text": {
                "type": "string",
                "description": "Content of the config file"
            },
            "format": {
                "type": "string",
                "enum": ["auto", "json", "toml", "yaml", "dotenv", "ini", "cargo_toml", "package_json", "pyproject"],
                "default": "auto",
                "description": "Config file format. Auto-detect from file_path and content if not specified."
            },
            "policy": {
                "type": "object",
                "description": "Optional policy overrides for risk detection",
                "properties": {
                    "allow_debug_flags": {
                        "type": "boolean",
                        "default": false,
                        "description": "Allow debug flags without review"
                    },
                    "allow_insecure_urls": {
                        "type": "boolean",
                        "default": false,
                        "description": "Allow http:// URLs without review"
                    },
                    "allow_command_hooks": {
                        "type": "boolean",
                        "default": false,
                        "description": "Allow command hooks without review"
                    }
                }
            }
        },
        "required": ["file_path", "text"]
    })
}

pub fn config_file_inspect_output() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "file_path": {
                "type": "string",
                "description": "The config file path"
            },
            "format": {
                "type": "string",
                "description": "Detected or specified format"
            },
            "parse_ok": {
                "type": "boolean",
                "description": "Whether the config file parsed successfully"
            },
            "shape_summary": {
                "type": "object",
                "description": "High-level shape summary of the config"
            },
            "risky_keys": {
                "type": "array",
                "items": {"type": "object"},
                "description": "Keys that pose security or operational risk"
            },
            "secret_risks": {
                "type": "array",
                "items": {"type": "object"},
                "description": "Secret-like key/value pairs detected"
            },
            "insecure_urls": {
                "type": "array",
                "items": {"type": "object"},
                "description": "Insecure URLs (http:// where https:// expected)"
            },
            "debug_flags": {
                "type": "array",
                "items": {"type": "object"},
                "description": "Debug flags that may be risky in production"
            },
            "command_hooks": {
                "type": "array",
                "items": {"type": "object"},
                "description": "Executable hook fields detected"
            },
            "verdict": {
                "type": "string",
                "enum": ["allow", "review", "block"],
                "description": "Whether config requires review"
            },
            "machine_code": {
                "type": "string",
                "description": "Machine-readable response code"
            },
            "findings": {
                "type": "array",
                "items": {"type": "object"},
                "description": "Structured findings with severity and disposition"
            }
        }
    })
}
