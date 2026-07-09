use serde_json::Value;

pub fn import_export_inspect_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "source": {"type": "string", "description": "Source text to scan for import/export statements"},
            "language": {"type": "string", "enum": ["rust", "python", "javascript", "typescript", "go", "auto"], "default": "auto", "description": "Source language. Auto-detect if not specified."},
            "include_line_text": {"type": "boolean", "default": false, "description": "Include the raw line text for each statement"},
            "max_statements": {"type": "integer", "default": 500, "minimum": 0, "description": "Maximum number of statements to return"}
        },
        "required": ["source"]
    })
}

pub fn import_export_inspect_output() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "language": {"type": "string", "description": "Detected or specified language"},
            "statements": {
                "type": "array",
                "description": "Extracted import/export statements",
                "items": {
                    "type": "object",
                    "properties": {
                        "kind": {"type": "string", "description": "Statement kind (use, import, from_import, etc.)"},
                        "module": {"type": "string", "description": "Module path"},
                        "symbols": {"type": "string", "description": "Imported symbols"},
                        "line": {"type": "integer", "description": "Line number (1-indexed)"},
                        "confidence": {"type": "string", "description": "Detection confidence (high, medium, low)"},
                        "raw_text": {"type": "string", "description": "Raw line text (if requested)"}
                    }
                }
            },
            "statement_count": {"type": "integer", "description": "Total statements found"},
            "truncated": {"type": "boolean", "description": "True if results were truncated"},
            "warnings": {"type": "array", "items": {"type": "string"}},
            "limitations": {"type": "array", "items": {"type": "string"}, "description": "Known limitations of the detection"},
            "verdict": {"type": "string", "enum": ["allow", "review", "block"]},
            "machine_code": {"type": "string"},
            "findings": {"type": "array", "items": {"type": "object"}}
        }
    })
}

pub fn code_block_map_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "source": {"type": "string", "description": "Source text to analyze for code blocks"},
            "language": {"type": "string", "enum": ["rust", "python", "javascript", "typescript", "go", "markdown", "auto"], "default": "auto", "description": "Source language. Auto-detect if not specified."},
            "max_blocks": {"type": "integer", "default": 500, "description": "Maximum number of blocks to return"},
            "include_nested": {"type": "boolean", "default": false, "description": "Include nested blocks (functions inside classes, etc.)"}
        },
        "required": ["source"]
    })
}

pub fn code_block_map_output() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "language": {"type": "string", "description": "Detected or specified language"},
            "blocks": {
                "type": "array",
                "description": "Top-level code blocks detected",
                "items": {
                    "type": "object",
                    "properties": {
                        "kind": {"type": "string", "description": "Block kind (fn, struct, class, def, heading, etc.)"},
                        "name": {"type": "string", "description": "Block name"},
                        "start_line": {"type": "integer", "description": "Start line (1-indexed)"},
                        "end_line": {"type": "integer", "description": "End line (1-indexed, inclusive)"},
                        "confidence": {"type": "string", "description": "Detection confidence"},
                        "raw_signature": {"type": "string", "description": "Raw signature line"}
                    }
                }
            },
            "block_count": {"type": "integer", "description": "Total blocks found"},
            "truncated": {"type": "boolean", "description": "True if results were truncated"},
            "warnings": {"type": "array", "items": {"type": "string"}},
            "limitations": {"type": "array", "items": {"type": "string"}, "description": "Known limitations"},
            "verdict": {"type": "string", "enum": ["allow", "review", "block"]},
            "machine_code": {"type": "string"},
            "findings": {"type": "array", "items": {"type": "object"}}
        }
    })
}

pub fn symbol_name_diff_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "old_source": {"type": "string", "description": "Original source text"},
            "new_source": {"type": "string", "description": "Modified source text"},
            "language": {"type": "string", "enum": ["rust", "python", "javascript", "typescript", "go", "auto"], "default": "auto", "description": "Source language"},
            "rename_similarity_threshold": {"type": "number", "default": 0.6, "description": "Minimum similarity ratio (0.0-1.0) to consider a pair as possible rename"}
        },
        "required": ["old_source", "new_source"]
    })
}

pub fn symbol_name_diff_output() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "language": {"type": "string", "description": "Detected or specified language"},
            "added": {"type": "array", "items": {"type": "string"}, "description": "Newly added symbol names"},
            "removed": {"type": "array", "items": {"type": "string"}, "description": "Removed symbol names"},
            "unchanged": {"type": "array", "items": {"type": "string"}, "description": "Symbols present in both versions"},
            "possible_renames": {
                "type": "array",
                "description": "Symbol pairs that may be renames",
                "items": {
                    "type": "object",
                    "properties": {
                        "old": {"type": "string"},
                        "new": {"type": "string"},
                        "confidence": {"type": "string"}
                    }
                }
            },
            "added_count": {"type": "integer"},
            "removed_count": {"type": "integer"},
            "unchanged_count": {"type": "integer"},
            "warnings": {"type": "array", "items": {"type": "string"}},
            "limitations": {"type": "array", "items": {"type": "string"}},
            "verdict": {"type": "string", "enum": ["allow", "review", "block"]},
            "machine_code": {"type": "string"},
            "findings": {"type": "array", "items": {"type": "object"}}
        }
    })
}

pub fn lockfile_inspect_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "path": {"type": "string", "description": "Lockfile path (used for ecosystem auto-detection)"},
            "before": {"type": "string", "description": "Previous lockfile content"},
            "after": {"type": "string", "description": "Current lockfile content"},
            "diff": {"type": "string", "description": "Unified diff of lockfile changes"},
            "ecosystem": {"type": "string", "enum": ["cargo", "npm", "pnpm", "yarn", "uv", "poetry", "go", "auto"], "default": "auto", "description": "Lockfile ecosystem"},
            "max_packages": {"type": "integer", "default": 500, "description": "Maximum number of packages to analyze"}
        }
    })
}

pub fn lockfile_inspect_output() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "ecosystem": {"type": "string", "description": "Detected or specified ecosystem"},
            "changes": {
                "type": "array",
                "description": "Detected package changes",
                "items": {
                    "type": "object",
                    "properties": {
                        "type": {"type": "string", "enum": ["added", "removed", "updated"]},
                        "package": {"type": "string"},
                        "version": {"type": "string"},
                        "old_version": {"type": "string"},
                        "new_version": {"type": "string"},
                        "source": {"type": "string"}
                    }
                }
            },
            "change_count": {"type": "integer", "description": "Total number of changes detected"},
            "large_churn": {"type": "boolean", "description": "True if many packages changed (>50)"},
            "summary": {"type": "string", "description": "Human-readable summary"},
            "limitations": {"type": "array", "items": {"type": "string"}, "description": "Known limitations"},
            "verdict": {"type": "string", "enum": ["allow", "review", "block"]},
            "machine_code": {"type": "string"},
            "findings": {"type": "array", "items": {"type": "object"}}
        }
    })
}
