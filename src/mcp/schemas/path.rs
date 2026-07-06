use serde_json::Value;

pub fn path_normalize_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "path": {"type": "string", "description": "Path string to normalize"},
            "platform": {"type": "string", "enum": ["posix", "windows"], "default": "posix", "description": "Platform semantics to use"},
            "collapse_dot_segments": {"type": "boolean", "default": true, "description": "Collapse dot and dot-dot segments"},
            "preserve_trailing_separator": {"type": "boolean", "default": false, "description": "Preserve trailing separator"}
        },
        "required": ["path"]
    })
}

pub fn path_analyze_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "path": {"type": "string"},
            "style": {"type": "string", "enum": ["auto", "posix", "windows"], "default": "auto"},
            "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "normal"}
        },
        "required": ["path"]
    })
}

pub fn path_compare_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "left": {"type": "string", "description": "First path string"},
            "right": {"type": "string", "description": "Second path string"},
            "platform": {"type": "string", "enum": ["posix", "windows"], "default": "posix", "description": "Platform semantics"},
            "case_sensitive": {"type": "boolean", "default": true, "description": "Case-sensitive comparison"},
            "normalize_separators": {"type": "boolean", "default": true, "description": "Normalize path separators"},
            "collapse_dot_segments": {"type": "boolean", "default": true, "description": "Collapse . and .. segments"}
        },
        "required": ["left", "right"]
    })
}

pub fn path_scope_check_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "root": {"type": "string", "description": "Root directory path"},
            "target": {"type": "string", "description": "Target path to check"},
            "platform": {"type": "string", "enum": ["posix", "windows"], "default": "posix", "description": "Platform semantics"},
            "case_sensitive": {"type": "boolean", "default": true, "description": "Case-sensitive comparison"}
        },
        "required": ["root", "target"]
    })
}

pub fn glob_match_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "pattern": {"type": "string", "description": "Glob pattern to match (e.g., src/**/*.rs)"},
            "path": {"type": "string", "description": "Path string to match against"},
            "platform": {"type": "string", "enum": ["posix", "windows"], "default": "posix", "description": "Path platform"},
            "case_sensitive": {"type": "boolean", "default": true, "description": "Case-sensitive matching"}
        },
        "required": ["pattern", "path"]
    })
}

pub fn path_normalize_output() -> Value {
    serde_json::json!({"type":"object","properties":{"normalized":{"type":"string"},"is_absolute":{"type":"boolean"},"components":{"type":"array","items":{"type":"string"}},"warnings":{"type":"array","items":{"type":"string"}}}})
}

pub fn path_analyze_output() -> Value {
    serde_json::json!({"type":"object","properties":{"input":{"type":"string"},"style":{"type":"string"},"absolute":{"type":"boolean"},"has_traversal":{"type":"boolean"},"components":{"type":"array","items":{"type":"string"}},"parent":{"type":["string","null"]},"name":{"type":["string","null"]},"stem":{"type":["string","null"]},"suffix":{"type":["string","null"]},"suffixes":{"type":"array","items":{"type":"string"}},"hidden":{"type":"boolean"},"normalized_lexical":{"type":"string"},"warnings":{"type":"array","items":{"type":"string"}},"summary":{"type":"string"}}})
}

pub fn path_compare_output() -> Value {
    serde_json::json!({"type":"object","properties":{"equal":{"type":"boolean","description":"Whether paths are equal under normalization"},"left_normalized":{"type":"string","description":"Normalized left path"},"right_normalized":{"type":"string","description":"Normalized right path"},"differences":{"type":"array","description":"List of differences found"},"findings":{"type":"array","description":"Normalization notes"}}})
}

pub fn path_scope_check_output() -> Value {
    serde_json::json!({"type":"object","properties":{"inside_root":{"type":"boolean","description":"Whether target is lexically inside root"},"root_normalized":{"type":"string","description":"Normalized root path"},"target_normalized":{"type":"string","description":"Normalized target path"},"relative_path":{"type":"string","description":"Relative path from root to target (if inside)"},"escapes_via_dotdot":{"type":"boolean","description":"Whether target contains parent traversal"},"absolute_target":{"type":"string","description":"Absolute form of target"},"findings":{"type":"array","description":"Analysis notes"}}})
}

pub fn glob_match_output() -> Value {
    serde_json::json!({"type":"object","properties":{"matches":{"type":"boolean"},"normalized_pattern":{"type":"string"},"normalized_path":{"type":"string"},"matched_segment":{"type":["string","null"]},"unmatched_segment":{"type":["string","null"]},"summary":{"type":"string"}}})
}

pub fn path_batch_scope_check_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "root": {"type": "string", "description": "Root directory path"},
            "targets": {"type": "array", "items": {"type": "string"}, "description": "Target paths to check against root"},
            "max_targets": {"type": "integer", "default": 1000, "description": "Maximum number of targets to process"},
            "allow_absolute": {"type": "boolean", "default": false, "description": "If true, absolute targets are not flagged as errors"},
            "case_sensitive": {"type": "boolean", "default": true, "description": "Case-sensitive path comparison"},
            "platform": {"type": "string", "default": "posix", "enum": ["posix", "windows", "auto"], "description": "Path comparison platform (windows and auto return UNSUPPORTED_FEATURE)"}
        },
        "required": ["root", "targets"]
    })
}

pub fn path_batch_scope_check_output() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "all_inside_root": {"type": "boolean", "description": "Whether all targets are lexically inside root"},
            "targets_checked": {"type": "integer", "description": "Number of targets processed"},
            "escaping_targets": {"type": "array", "items": {"type": "string"}, "description": "Targets that resolve outside root"},
            "absolute_targets": {"type": "array", "items": {"type": "string"}, "description": "Targets that were absolute paths"},
            "dotdot_targets": {"type": "array", "items": {"type": "string"}, "description": "Targets containing parent traversal segments"},
            "normalized_targets": {"type": "array", "items": {"type": "object"}, "description": "Mapping of original to normalized paths"},
            "duplicate_normalized_targets": {"type": "array", "items": {"type": "object"}, "description": "Normalized targets reached by multiple inputs"},
            "findings": {"type": "array", "items": {"type": "object"}, "description": "Structured findings"},
            "verdict": {"type": "string", "enum": ["allow", "review", "block"], "description": "Overall verdict"},
            "machine_code": {"type": "string", "description": "Machine-readable response code"}
        }
    })
}
