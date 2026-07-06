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
            "new": {"type": "string", "description": "Replacement text (required for literal and line_range modes)"},
            "patch": {"type": "string", "description": "Unified diff patch (patch mode)"},
            "start_line": {"type": "integer", "description": "First line (line_range mode)"},
            "end_line": {"type": "integer", "description": "Last line inclusive (line_range mode)"},
            "expected_fingerprint": {"type": "string", "description": "Expected SHA-256 fingerprint for verification"},
            "strict": {"type": "boolean", "default": true, "description": "Strict mode for patch matching"},
            "file_path": {"type": "string", "description": "Target file path (enables path scope check when workspace_root is set)"},
            "workspace_root": {"type": "string", "description": "Workspace root directory (enables path scope validation)"},
            "newline_policy": {"type": "string", "enum": ["skip", "check", "normalize_lf", "normalize_crlf"], "default": "skip", "description": "Newline style policy: skip, check (flag mixed), normalize_lf, normalize_crlf"},
            "unicode_policy": {"type": "string", "enum": ["skip", "default", "source_code", "identifier"], "default": "skip", "description": "Unicode security policy: skip, default, source_code, identifier"},
            "edit_metadata": {"type": "object", "properties": {"description": {"type": "string"}, "author": {"type": "string"}, "source_tool": {"type": "string", "description": "Tool that originated this edit"}, "session_id": {"type": "string", "description": "Session identifier for traceability"}, "request_id": {"type": "string", "description": "Request identifier for traceability"}}, "description": "Edit metadata for diagnostics and traceability"}
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

pub fn diff_risk_classify_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "patch_text": {"type": "string", "description": "Unified diff text to classify"},
            "workspace_root": {"type": "string", "description": "Optional workspace root for path-scope context"},
            "max_patch_chars": {"type": "integer", "default": 200000, "description": "Maximum patch text length to process"},
            "detail": {
                "type": "string",
                "enum": ["summary", "normal", "full"],
                "default": "normal",
                "description": "Detail level for output"
            },
            "policy": {
                "type": "object",
                "description": "Optional policy overrides for risk classification",
                "properties": {
                    "review_ci_changes": {"type": "boolean", "default": true, "description": "Flag CI changes for review"},
                    "review_dependency_changes": {"type": "boolean", "default": true, "description": "Flag dependency changes for review"},
                    "review_security_sensitive_paths": {"type": "boolean", "default": true, "description": "Flag security-sensitive path changes for review"},
                    "allow_docs_only": {"type": "boolean", "default": true, "description": "Allow docs-only diffs without review"}
                }
            }
        },
        "required": ["patch_text"]
    })
}

pub fn diff_risk_classify_output() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "summary": {"type": "string", "description": "Compact human-readable summary"},
            "patch_summary": {
                "type": "object",
                "description": "Selected fields from patch parsing",
                "properties": {
                    "files_changed": {"type": "integer"},
                    "hunks_total": {"type": "integer"},
                    "additions": {"type": "integer"},
                    "deletions": {"type": "integer"},
                    "renames_detected": {"type": "array", "items": {"type": "object"}},
                    "binary_patch_detected": {"type": "boolean"}
                }
            },
            "risk_categories": {
                "type": "array",
                "items": {"type": "string"},
                "description": "Risk categories identified in this diff"
            },
            "files_by_category": {
                "type": "object",
                "description": "Files grouped by risk category",
                "additionalProperties": {"type": "array", "items": {"type": "string"}}
            },
            "review_focus": {
                "type": "array",
                "items": {"type": "object"},
                "description": "Priority review items with path/category reasons"
            },
            "recommended_next_tool": {
                "type": ["string", "null"],
                "description": "Recommended next tool for this diff"
            },
            "verdict": {
                "type": "string",
                "enum": ["allow", "review", "block"],
                "description": "Overall risk verdict"
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

pub fn edit_preflight_output() -> Value {
    serde_json::json!({"type":"object","properties":{"ok_to_apply":{"type":"boolean"},"mode":{"type":"string"},"verdict":{"type":"string","enum":["allow","review","block","safe_to_apply","safe_with_warnings"],"description":"Typed verdict for programmatic routing"},"findings":{"type":"array"},"machine_code":{"type":"string","description":"Primary machine code (highest-priority finding)"},"secondary_machine_codes":{"type":"array","items":{"type":"string"},"description":"Additional machine codes when multiple findings exist"},"recommended_next_tool":{"type":["string","null"]},"summary":{"type":"string"},"subresults":{"type":"object"},"path_scope":{"type":["object","null"],"properties":{"inside_root":{"type":"boolean"},"escapes_via_dotdot":{"type":"boolean"},"relative_path":{"type":"string"},"normalized_target":{"type":["string","null"],"description":"Normalized absolute target path (lexical only, no symlink resolution)"},"reason":{"type":["string","null"],"description":"Human-readable reason for the path scope decision"}},"description":"Path scope check result (lexical only, no symlink resolution)"},"newline_check":{"type":["object","null"],"properties":{"style":{"type":"string"},"mixed":{"type":"boolean"},"policy":{"type":"string"},"recommended_normalization":{"type":["string","null"]},"original_style":{"type":["string","null"],"description":"Newline style detected in the original text"},"replacement_style":{"type":["string","null"],"description":"Newline style detected in the replacement text"}},"description":"Newline style check result"},"unicode_check":{"type":["object","null"],"properties":{"verdict":{"type":"string"},"machine_code":{"type":"string"},"finding_count":{"type":"integer"},"findings":{"type":"array","items":{"type":"object","description":"Structured findings from text_security_inspect"}}},"description":"Unicode security check result"},"fingerprint":{"type":["object","null"],"properties":{"sha256":{"type":"string"},"newline_style":{"type":"string"}},"description":"Fingerprint verification result"}}})
}
