use serde_json::Value;

pub fn cargo_toml_inspect_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "The Cargo.toml content to inspect"},
            "check_workspace": {"type": "boolean", "default": true, "description": "Whether to analyze [workspace] section"},
            "check_dependencies": {"type": "boolean", "default": true, "description": "Whether to analyze dependency sections"}
        },
        "required": ["text"]
    })
}

pub fn cargo_toml_inspect_output() -> Value {
    serde_json::json!({"type":"object","properties":{"parse_ok":{"type":"boolean","description":"Whether TOML parsed successfully"},"package":{"type":"object","description":"Package metadata from [package] section","properties":{"name":{"type":"string"},"version":{"type":"string"},"edition":{"type":"string"},"license":{"type":"string"},"repository":{"type":"string"},"readme":{"type":"string"}}},"workspace":{"type":"object","description":"Workspace section information","properties":{"present":{"type":"boolean"},"members":{"type":"array","items":{"type":"string"}},"exclude":{"type":"array","items":{"type":"string"}}}},"dependencies":{"type":"object","description":"Dependencies by section"},"path_dependencies":{"type":"array","items":{"type":"string"},"description":"Extracted path dependency values"},"suspicious_dependency_names":{"type":"array","items":{"type":"string"},"description":"Dependency names with suspicious patterns"},"duplicate_or_confusable_dependency_names":{"type":"array","items":{"type":"string"},"description":"Dependency names that normalize to the same form"},"findings":{"type":"array","items":{"type":"string"},"description":"Structural findings and warnings"}}})
}
