use serde_json::Value;

pub fn patch_apply_check_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "original_text": {"type": "string", "description": "The original source text to apply the patch to"},
            "patch_text": {"type": "string", "description": "The unified diff patch text"},
            "strict": {"type": "boolean", "default": true, "description": "If True, context lines must match exactly"},
            "return_result_fingerprint": {"type": "boolean", "default": true, "description": "If True, compute SHA-256 fingerprint of the result"},
            "return_result_text": {"type": "boolean", "default": false, "description": "If True, include the resulting text (bounded to 50000 chars)"}
        },
        "required": ["original_text", "patch_text"]
    })
}

pub fn patch_summary_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {"patch_text": {"type": "string", "description": "The unified diff text to summarize"}},
        "required": ["patch_text"]
    })
}

pub fn edit_preflight_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "original": {"type": "string", "description": "Original source text"},
            "replacement_mode": {"type": "string", "enum": ["literal", "patch", "line_range"], "default": "literal", "description": "Edit mode: literal (old/new), patch (unified diff), or line_range"},
            "old": {"type": "string", "description": "Text to find (literal mode)"},
            "new": {"type": "string", "description": "Replacement text (literal mode)"},
            "patch": {"type": "string", "description": "Unified diff patch (patch mode)"},
            "start_line": {"type": "integer", "description": "First line (line_range mode)"},
            "end_line": {"type": "integer", "description": "Last line inclusive (line_range mode)"},
            "expected_fingerprint": {"type": "string", "description": "Expected SHA-256 fingerprint for verification"},
            "strict": {"type": "boolean", "default": true, "description": "Strict mode for patch matching"}
        },
        "required": ["original"]
    })
}

pub fn patch_apply_check_output() -> Value {
    serde_json::json!({"type":"object","properties":{"patch_parse_ok":{"type":"boolean","description":"True if patch parsed successfully"},"applies":{"type":"boolean","description":"True if all hunks applied cleanly"},"hunks_total":{"type":"integer","description":"Total number of hunks in patch"},"hunks_applied":{"type":"integer","description":"Number of hunks that applied successfully"},"hunks_failed":{"type":"integer","description":"Number of hunks that failed to apply"},"failed_hunks":{"type":"array","description":"Details of each failed hunk","items":{"type":"object","properties":{"hunk_index":{"type":"integer"},"old_start":{"type":"integer"},"old_count":{"type":"integer"},"expected_context":{"type":"array","items":{"type":"string"}},"actual_context":{"type":"array","items":{"type":"string"}},"reason":{"type":"string"}}}},"affected_line_ranges":{"type":"array","description":"Line ranges affected by successful hunks","items":{"type":"object","properties":{"start":{"type":"integer"},"end":{"type":"integer"}}}},"newline_style_before":{"type":"string","description":"Newline style in original text"},"newline_style_after":{"type":"string","description":"Newline style in result text"},"result_fingerprint":{"type":"string","description":"SHA-256 of the result text"},"result_text":{"type":["string","null"],"description":"Resulting text if requested"},"findings":{"type":"array","items":{"type":"string"},"description":"Analysis notes and warnings"}}})
}

pub fn patch_summary_output() -> Value {
    serde_json::json!({"type":"object","properties":{"files_changed":{"type":"integer","description":"Number of files changed"},"hunks_total":{"type":"integer","description":"Total number of hunks across all files"},"additions":{"type":"integer","description":"Total number of added lines"},"deletions":{"type":"integer","description":"Total number of deleted lines"},"renames_detected":{"type":"array","description":"Detected file renames","items":{"type":"object","properties":{"from":{"type":"string"},"to":{"type":"string"}}}},"binary_patch_detected":{"type":"boolean","description":"True if binary patch content detected"},"line_ranges_by_file":{"type":"object","description":"Line ranges affected per file","additionalProperties":{"type":"array","items":{"type":"object","properties":{"start":{"type":"integer"},"end":{"type":"integer"}}}}},"findings":{"type":"array","items":{"type":"string"},"description":"Analysis notes and warnings"}}})
}

pub fn edit_preflight_output() -> Value {
    serde_json::json!({"type":"object","properties":{"ok_to_apply":{"type":"boolean"},"mode":{"type":"string"},"findings":{"type":"array"},"machine_code":{"type":"string"},"recommended_next_tool":{"type":["string","null"]},"summary":{"type":"string"},"subresults":{"type":"object"}}})
}
