use serde_json::Value;

pub fn dependency_edit_preflight_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "file_path": {
                "type": "string",
                "description": "Path to the dependency file being edited (e.g. Cargo.toml, pyproject.toml, package.json)"
            },
            "old_text": {
                "type": "string",
                "description": "Current content of the file (before edit)"
            },
            "new_text": {
                "type": "string",
                "description": "Proposed content of the file (after edit)"
            },
            "ecosystem": {
                "type": "string",
                "enum": ["auto", "rust", "python", "node"],
                "default": "auto",
                "description": "Dependency ecosystem. Auto-detect from file_path if not specified."
            },
            "policy": {
                "type": "object",
                "description": "Optional policy overrides",
                "properties": {
                    "allow_path_deps": {
                        "type": "boolean",
                        "default": true,
                        "description": "Allow path dependencies without review"
                    },
                    "allow_git_deps": {
                        "type": "boolean",
                        "default": false,
                        "description": "Allow git dependencies without review"
                    },
                    "allow_patch_sections": {
                        "type": "boolean",
                        "default": false,
                        "description": "Allow [patch] sections without review"
                    }
                }
            }
        },
        "required": ["file_path", "old_text", "new_text"]
    })
}

pub fn dependency_edit_preflight_output() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "file_path": {
                "type": "string",
                "description": "The dependency file path"
            },
            "ecosystem": {
                "type": "string",
                "enum": ["rust", "python", "node"],
                "description": "Detected or specified ecosystem"
            },
            "verdict": {
                "type": "string",
                "enum": ["allow", "review", "block"],
                "description": "Whether the dependency edit is safe to apply"
            },
            "machine_code": {
                "type": "string",
                "description": "Machine-readable response code"
            },
            "dependency_changes": {
                "type": "object",
                "description": "Summary of dependency changes",
                "properties": {
                    "added": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Newly added dependency names"
                    },
                    "removed": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Removed dependency names"
                    },
                    "version_changed": {
                        "type": "array",
                        "items": {"type": "object"},
                        "description": "Dependencies with version constraint changes"
                    },
                    "source_changed": {
                        "type": "array",
                        "items": {"type": "object"},
                        "description": "Dependencies with source type changes (registry→git, etc.)"
                    }
                }
            },
            "hook_changes": {
                "type": "array",
                "items": {"type": "object"},
                "description": "Script or hook changes detected (install, postinstall, build, etc.)"
            },
            "findings": {
                "type": "array",
                "items": {"type": "object"},
                "description": "Structured findings with severity and disposition"
            }
        }
    })
}
