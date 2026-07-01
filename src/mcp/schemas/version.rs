use serde_json::Value;

pub fn version_compare_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "a": {"type": "string", "description": "First version string"},
            "b": {"type": "string", "description": "Second version string"},
            "scheme": {"type": "string", "enum": ["semver", "pep440", "loose"], "default": "semver", "description": "Version scheme"}
        },
        "required": ["a", "b"]
    })
}

pub fn version_constraint_check_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "version": {"type": "string", "description": "Version string to check (e.g., '1.2.3', '0.5.0-beta.1')"},
            "constraint": {"type": "string", "description": "Version constraint (e.g., '>=1.0,<2.0', '^1.2.3', '~0.5', '1.*')"},
            "scheme": {"type": "string", "enum": ["semver", "cargo"], "default": "semver", "description": "Versioning scheme to use for parsing and evaluation"}
        },
        "required": ["version", "constraint"]
    })
}

pub fn version_compare_output() -> Value {
    serde_json::json!({"type":"object","properties":{"comparison":{"type":"integer","description":"Comparison result: -1 (a < b), 0 (equal), 1 (a > b)"},"valid":{"type":"boolean","description":"Whether versions are valid for the scheme"},"scheme":{"type":"string"},"summary":{"type":"string"}}})
}

pub fn version_constraint_check_output() -> Value {
    serde_json::json!({"type":"object","properties":{"satisfies":{"type":"boolean","description":"Whether the version satisfies the constraint"},"parsed_version":{"type":"object","description":"Parsed version components"},"parsed_constraint":{"type":"object","description":"Parsed constraint components"},"scheme":{"type":"string","description":"Versioning scheme used"},"explanation":{"type":"string","description":"Human-readable explanation"},"findings":{"type":"array","items":{"type":"string"},"description":"Analysis notes and warnings"}}})
}
