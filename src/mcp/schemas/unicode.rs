use serde_json::Value;

pub fn unicode_policy_check_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "Input text to check"},
            "policy": {"type": "string", "enum": ["identifier_strict", "filename_safe", "source_code", "human_text", "json_key", "domain_like"], "description": "Policy to apply"},
            "normalization": {"type": "string", "enum": ["raw", "NFC", "NFD", "NFKC", "NFKD"], "description": "Normalization form (default: policy-specific)"}
        },
        "required": ["text", "policy"]
    })
}

pub fn canonicalize_text_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "Input text to canonicalize"},
            "profile": {"type": "string", "enum": ["source_file_identity", "identifier_compare", "human_label_compare", "json_key_compare", "path_segment_compare"], "description": "Canonicalization profile to apply"},
            "return_mapping": {"type": "boolean", "default": false, "description": "If True, include a character mapping of changes"}
        },
        "required": ["text", "profile"]
    })
}

pub fn unicode_policy_check_output() -> Value {
    serde_json::json!({"type":"object","properties":{"pass_":{"type":"boolean","description":"True if text passes the policy (no errors)"},"policy":{"type":"string","description":"Policy name that was applied"},"normalized_form":{"type":"string","description":"Text after normalization"},"findings":{"type":"array","description":"Policy findings with rule, severity, and message","items":{"type":"object","properties":{"rule":{"type":"string"},"severity":{"type":"string"},"message":{"type":"string"}}}},"summary":{"type":"string","description":"Human-readable summary"}}})
}

pub fn canonicalize_text_output() -> Value {
    serde_json::json!({"type":"object","properties":{"text":{"type":"string","description":"Canonicalized text"},"changed":{"type":"boolean","description":"True if text was modified"},"operations_applied":{"type":"array","description":"List of operations applied"},"fingerprint_before":{"type":"string","description":"SHA-256 of original text"},"fingerprint_after":{"type":"string","description":"SHA-256 of canonicalized text"},"mapping":{"type":"array","description":"Character mapping if return_mapping was True"},"findings":{"type":"array","items":{"type":"string"},"description":"Analysis notes and warnings"}}})
}
