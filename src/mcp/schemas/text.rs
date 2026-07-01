use serde_json::Value;

pub fn text_measure_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "Input string to measure"},
            "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "normal", "description": "Detail level for output"}
        },
        "required": ["text"]
    })
}

pub fn text_equal_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "a": {"type": "string", "description": "First string"},
            "b": {"type": "string", "description": "Second string"},
            "normalization": {"type": "string", "enum": ["raw", "NFC", "NFD", "NFKC", "NFKD"], "default": "raw", "description": "Unicode normalization form"},
            "casefold": {"type": "boolean", "default": false, "description": "Use casefolded comparison"},
            "trim": {"type": "boolean", "default": false, "description": "Trim whitespace"},
            "ignore_newline_style": {"type": "boolean", "default": false, "description": "Normalize different newline styles before comparison"},
            "ignore_trailing_whitespace": {"type": "boolean", "default": false, "description": "Ignore trailing whitespace on each line"},
            "ignore_final_newline": {"type": "boolean", "default": false, "description": "Ignore trailing newline at end of strings"}
        },
        "required": ["a", "b"]
    })
}

pub fn text_diff_explain_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "a": {"type": "string", "description": "First string"},
            "b": {"type": "string", "description": "Second string"},
            "max_diffs": {"type": "integer", "default": 20, "minimum": 0, "maximum": 10000, "description": "Maximum diff spans to return"},
            "include_codepoints": {"type": "boolean", "default": true, "description": "Include codepoint details"},
            "include_context": {"type": "boolean", "default": true, "description": "Include context notes"},
            "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "normal", "description": "Detail level: summary (compact), normal, or full"}
        },
        "required": ["a", "b"]
    })
}

pub fn text_inspect_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "Input string to inspect"},
            "include_codepoints": {"type": "boolean", "default": true, "description": "Include codepoint details in invisibles"},
            "include_confusables": {"type": "boolean", "default": true, "description": "Check for confusables"},
            "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "normal", "description": "Detail level: summary (compact), normal, or full"},
            "normalize": {"type": "string", "enum": ["none", "NFC", "NFD", "NFKC", "NFKD"], "default": "none", "description": "Normalization form to analyze"},
            "compare_normalized": {"type": "boolean", "default": false, "description": "Report both original and normalized analysis"}
        },
        "required": ["text"]
    })
}

pub fn text_count_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "Input string"},
            "target": {"type": ["string", "null"], "default": null, "description": "Single character to count (None for frequency table)"},
            "count_mode": {"type": "string", "enum": ["codepoint", "grapheme", "byte", "substring"], "default": "codepoint", "description": "Count mode: codepoint (Python str), grapheme (user-perceived), byte (UTF-8), substring"},
            "normalization": {"type": "string", "enum": ["raw", "NFC", "NFKC"], "default": "raw", "description": "Unicode normalization form"}
        },
        "required": ["text"]
    })
}

pub fn text_truncate_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "Input string to truncate"},
            "max_graphemes": {"type": "integer", "minimum": 0, "maximum": 1000000, "description": "Maximum number of grapheme clusters to return"}
        },
        "required": ["text", "max_graphemes"]
    })
}

pub fn text_transform_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "Input string to transform"},
            "operations": {"type": "array", "items": {"type": "string"}, "description": "Operations to apply: normalize_nfc, normalize_nfd, normalize_nfkc, normalize_nfkd, casefold, trim, trim_trailing_whitespace, normalize_newlines_lf, ensure_final_newline, strip_final_newline, remove_zero_width, remove_bidi_controls, visible_repr", "maxItems": 100},
            "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "normal"}
        },
        "required": ["text", "operations"]
    })
}

pub fn text_position_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string"},
            "byte_offset": {"type": "integer", "minimum": 0, "maximum": 1000000000},
            "codepoint_index": {"type": "integer", "minimum": 0, "maximum": 1000000000},
            "line": {"type": "integer", "minimum": 0, "maximum": 1000000000},
            "column": {"type": "integer", "minimum": 0, "maximum": 1000000000},
            "utf16_offset": {"type": "integer", "minimum": 0, "maximum": 1000000000},
            "line_base": {"type": "integer", "default": 1, "minimum": 0, "maximum": 1},
            "column_base": {"type": "integer", "default": 1, "minimum": 0, "maximum": 1},
            "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "normal"}
        },
        "required": ["text"]
    })
}

pub fn text_hash_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string"},
            "algorithms": {"type": "array", "items": {"type": "string"}, "default": ["sha256"], "description": "Hash algorithms (sha256, sha1, md5, crc32)", "maxItems": 10},
            "encoding": {"type": "string", "default": "utf-8"},
            "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "normal"}
        },
        "required": ["text"]
    })
}

pub fn escape_text_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string"},
            "mode": {"type": "string", "enum": ["json_string", "python_string", "rust_string", "posix_shell_single", "regex_literal", "markdown_inline_code", "markdown_code_block", "html_text", "url_component"]},
            "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "normal"}
        },
        "required": ["text", "mode"]
    })
}

pub fn unescape_text_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string"},
            "mode": {"type": "string", "enum": ["json_string", "python_string", "unicode_escape", "url_component"]},
            "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "normal"}
        },
        "required": ["text", "mode"]
    })
}

pub fn text_window_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "Input string to analyze"},
            "position": {
                "type": "object",
                "description": "Position specification with kind and value",
                "properties": {
                    "kind": {"type": "string", "enum": ["byte_offset", "codepoint_index", "grapheme_index", "line_column"]},
                    "value": {"type": "integer", "description": "Value for byte_offset, codepoint_index, or grapheme_index"},
                    "byte_offset": {"type": "integer", "description": "UTF-8 byte offset (alternative to value)"},
                    "codepoint_index": {"type": "integer", "description": "Codepoint index (alternative to value)"},
                    "grapheme_index": {"type": "integer", "description": "Grapheme index (alternative to value)"},
                    "line": {"type": "integer", "description": "Line number for line_column kind"},
                    "column": {"type": "integer", "description": "Column number for line_column kind"},
                    "line_base": {"type": "integer", "default": 1, "description": "Base for line numbers (1 for 1-based)"},
                    "column_base": {"type": "integer", "default": 1, "description": "Base for column numbers (1 for 1-based)"}
                },
                "required": ["kind"]
            },
            "context_lines": {"type": "integer", "default": 2, "description": "Number of context lines before and after"},
            "include_visible_repr": {"type": "boolean", "default": true, "description": "Include visible representation of the line"}
        },
        "required": ["text", "position"]
    })
}

pub fn text_fingerprint_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "Input string to fingerprint"},
            "unicode": {"type": "string", "enum": ["raw", "NFC", "NFD", "NFKC", "NFKD"], "default": "raw", "description": "Unicode normalization form"},
            "newline": {"type": "string", "enum": ["raw", "LF"], "default": "raw", "description": "Newline normalization"},
            "trim_final_newline": {"type": "boolean", "default": false, "description": "Remove trailing newline before hashing"},
            "casefold": {"type": "boolean", "default": false, "description": "Apply casefolding before hashing"}
        },
        "required": ["text"]
    })
}

pub fn text_replace_check_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "Source text to search in"},
            "old": {"type": "string", "description": "Text to find"},
            "new": {"type": "string", "description": "Replacement text"},
            "mode": {"type": "string", "enum": ["exact", "nfc", "nfkc", "casefold", "whitespace_collapse"], "default": "exact", "description": "Matching mode"},
            "expected_count": {"type": "integer", "description": "Expected number of matches (optional)"},
            "allow_multiple": {"type": "boolean", "default": false, "description": "If False and more than one match, add a finding"},
            "newline_policy": {"type": "string", "enum": ["preserve", "normalize_lf", "normalize_crlf"], "default": "preserve", "description": "How to handle newlines"},
            "return_preview": {"type": "boolean", "default": false, "description": "If True, include before/after text previews"},
            "max_preview_chars": {"type": "integer", "default": 2000, "description": "Maximum characters in preview output"}
        },
        "required": ["text", "old", "new"]
    })
}

pub fn line_range_extract_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "Input text"},
            "start_line": {"type": "integer", "minimum": 0, "maximum": 100000000, "description": "First line to extract"},
            "end_line": {"type": "integer", "minimum": 0, "maximum": 100000000, "description": "Last line to extract (inclusive)"},
            "line_base": {"type": "integer", "default": 1, "minimum": 0, "maximum": 1, "description": "Base for line numbers (1 for 1-based, 0 for 0-based)"},
            "include_line_numbers": {"type": "boolean", "default": false, "description": "Include line number in each line dict"},
            "include_fingerprint": {"type": "boolean", "default": true, "description": "Compute SHA-256 fingerprint of extracted text"}
        },
        "required": ["text", "start_line", "end_line"]
    })
}

pub fn line_range_compare_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "left_text": {"type": "string", "description": "First text input"},
            "right_text": {"type": "string", "description": "Second text input"},
            "start_line": {"type": "integer", "minimum": 0, "maximum": 100000000, "description": "First line to compare"},
            "end_line": {"type": "integer", "minimum": 0, "maximum": 100000000, "description": "Last line to compare (inclusive)"},
            "line_base": {"type": "integer", "default": 1, "minimum": 0, "maximum": 1, "description": "Base for line numbers"},
            "comparison_mode": {"type": "string", "enum": ["exact", "ignore_trailing_whitespace", "normalize_newlines"], "default": "exact", "description": "Comparison mode"}
        },
        "required": ["left_text", "right_text", "start_line", "end_line"]
    })
}

pub fn prompt_input_inspect_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "The text to inspect for red flags"},
            "checks": {"type": "array", "items": {"type": "string"}, "description": "Subset of checks to run: unicode_hidden, bidi, html_comments, markdown_links, ansi_escapes, terminal_controls, base64_like_blobs, instruction_phrases, long_minified_lines"},
            "phrase_patterns": {"type": ["array", "null"], "items": {"type": "string"}, "description": "Optional literal strings or safe regexes to detect as instruction-like phrases. Pass null for no custom patterns."}
        },
        "required": ["text"]
    })
}

pub fn text_security_inspect_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "Input text to inspect"},
            "policy": {"type": "string", "enum": ["default", "source_code", "prompt", "markdown", "identifier"], "default": "default", "description": "Security policy to apply"},
            "normalize": {"type": "string", "enum": ["none", "NFC", "NFD", "NFKC", "NFKD"], "default": "none", "description": "Normalization form to analyze"},
            "compare_normalized": {"type": "boolean", "default": false, "description": "Report both original and normalized analysis"},
            "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "summary", "description": "Detail level: summary (compact verdict only), normal, or full (includes subresults)"}
        },
        "required": ["text"]
    })
}

pub fn text_measure_output() -> Value {
    serde_json::json!({"type":"object","properties":{"bytes_utf8":{"type":"integer"},"codepoints":{"type":"integer"},"graphemes":{"type":"integer"},"words":{"type":"integer"},"unique_words_casefolded":{"type":"integer"},"lines":{"type":"integer"},"nonempty_lines":{"type":"integer"},"blank_lines":{"type":"integer"},"max_line_length_codepoints":{"type":"integer"},"chars_no_whitespace":{"type":"integer"},"ascii":{"type":"integer"},"non_ascii":{"type":"integer"},"letters":{"type":"integer"},"digits":{"type":"integer"},"punctuation":{"type":"integer"},"symbols":{"type":"integer"},"spaces":{"type":"integer"},"control_chars":{"type":"integer"},"combining_marks":{"type":"integer"},"invisible_chars":{"type":"integer"},"newline_style":{"type":"string"},"ends_with_newline":{"type":"boolean"},"normalization":{"type":"object"},"unicode_risks":{"type":"object"},"warnings":{"type":"array"}}})
}

pub fn text_equal_output() -> Value {
    serde_json::json!({"type":"object","properties":{"equal":{"type":"boolean"},"mode":{"type":"object"},"raw_equal":{"type":"boolean"},"nfc_equal":{"type":"boolean"},"nfd_equal":{"type":"boolean"},"nfkc_equal":{"type":"boolean"},"nfkd_equal":{"type":"boolean"},"casefold_equal":{"type":"boolean"},"byte_equal":{"type":"boolean"},"lengths":{"type":"object"},"first_difference":{"type":["object","null"]},"classification":{"type":"string"}}})
}

pub fn text_diff_explain_output() -> Value {
    serde_json::json!({"type":"object","properties":{"equal":{"type":"boolean"},"classification":{"type":"string"},"summary":{"type":"object"},"a_metrics":{"type":"object"},"b_metrics":{"type":"object"},"diffs":{"type":"array"},"security_findings":{"type":"array"},"agent_instruction":{"type":"string"}}})
}

pub fn text_inspect_output() -> Value {
    serde_json::json!({"type":"object","properties":{"safe_repr":{"type":"string"},"metrics":{"type":"object"},"normalization":{"type":"object"},"normalization_diff":{"type":"boolean"},"normals_repr":{"type":["string","null"]},"invisibles":{"type":"array"},"bidi_controls":{"type":"array"},"mixed_scripts":{"type":"object"},"confusables":{"type":"array"},"warnings":{"type":"array"},"limits_applied":{"type":"array"},"normalize":{"type":"string"},"compare_normalized":{"type":"boolean"},"original":{"type":"object"},"normalized":{"type":["object","null"]},"normalization_findings":{"type":"array"}}})
}

pub fn text_count_output() -> Value {
    serde_json::json!({"type":"object","description":"With target: {count, positions, target, normalization, text_length_codepoints}. Without target: character frequency table as {char: count} pairs.","properties":{"count":{"type":"integer"},"positions":{"type":"array"},"target":{"type":["string","null"]},"normalization":{"type":["string","null"]},"text_length_codepoints":{"type":"integer"}}})
}

pub fn text_truncate_output() -> Value {
    serde_json::json!({"type":"object","properties":{"text":{"type":"string","description":"Result string (truncated if truncation occurred)"},"original_graphemes":{"type":"integer","description":"Original grapheme count"},"truncated_graphemes":{"type":"integer","description":"Grapheme count in result"},"truncated":{"type":"boolean","description":"True if text was truncated"}}})
}

pub fn text_transform_output() -> Value {
    serde_json::json!({"type":"object","properties":{"changed":{"type":"boolean"},"text":{"type":"string"},"operations_applied":{"type":"array","items":{"type":"string"}},"removed":{"type":"array"},"warnings":{"type":"array","items":{"type":"string"}},"summary":{"type":"string"}}})
}

pub fn text_position_output() -> Value {
    serde_json::json!({"type":"object","properties":{"valid":{"type":"boolean"},"byte_offset":{"type":["integer","null"]},"codepoint_index":{"type":["integer","null"]},"utf16_offset":{"type":["integer","null"]},"line":{"type":["integer","null"]},"column":{"type":["integer","null"]},"line_base":{"type":"integer"},"column_base":{"type":"integer"},"char":{"type":["string","null"]},"codepoint":{"type":["string","null"]},"name":{"type":["string","null"]},"line_text_preview":{"type":["string","null"]},"error":{"type":["string","null"]},"summary":{"type":"string"}}})
}

pub fn text_hash_output() -> Value {
    serde_json::json!({"type":"object","properties":{"encoding":{"type":"string"},"bytes":{"type":"integer"},"codepoints":{"type":"integer"},"hashes":{"type":"object","description":"Map of algorithm to hash value"},"warnings":{"type":"array","items":{"type":"string"}},"summary":{"type":"string"}}})
}

pub fn escape_text_output() -> Value {
    serde_json::json!({"type":"object","properties":{"mode":{"type":"string"},"escaped":{"type":"string"},"changed":{"type":"boolean"},"summary":{"type":"string"}}})
}

pub fn unescape_text_output() -> Value {
    serde_json::json!({"type":"object","properties":{"mode":{"type":"string"},"unescaped":{"type":"string"},"changed":{"type":"boolean"},"error":{"type":["string","null"]},"summary":{"type":"string"}}})
}

pub fn text_window_output() -> Value {
    serde_json::json!({"type":"object","properties":{"position":{"type":"object","description":"Resolved position with byte_offset, codepoint_index, grapheme_index, line, column"},"line_text":{"type":"string"},"line_visible_repr":{"type":"string"},"before":{"type":"array","description":"Context lines before"},"after":{"type":"array","description":"Context lines after"},"newline_style":{"type":"string"},"at_codepoint":{"type":["object","null"]},"warnings":{"type":"array","items":{"type":"string"}}}})
}

pub fn text_fingerprint_output() -> Value {
    serde_json::json!({"type":"object","properties":{"sha256":{"type":"string"},"bytes_utf8":{"type":"integer"},"codepoints":{"type":"integer"},"graphemes":{"type":"integer"},"newline_style":{"type":"string"},"normalization":{"type":"object","description":"Normalization state details"},"summary":{"type":"string"}}})
}

pub fn text_replace_check_output() -> Value {
    serde_json::json!({"type":"object","properties":{"match_count":{"type":"integer","description":"Number of matches found"},"unique_match":{"type":"boolean","description":"True if exactly one match"},"expected_count_met":{"type":"boolean","description":"True if match count matches expected_count"},"would_change":{"type":"boolean","description":"True if replacement would change text"},"positions":{"type":"array","description":"Match positions with byte offsets and line/column"},"changed_text_fingerprint":{"type":"string","description":"SHA-256 fingerprint of changed text"},"newline_style_before":{"type":"string"},"newline_style_after":{"type":"string"},"preview_before":{"type":"string"},"preview_after":{"type":"string"},"findings":{"type":"array","description":"Warnings and info messages"}}})
}

pub fn line_range_extract_output() -> Value {
    serde_json::json!({"type":"object","properties":{"line_count_total":{"type":"integer","description":"Total line count in input"},"start_line":{"type":"integer"},"end_line":{"type":"integer"},"valid_range":{"type":"boolean","description":"True if range is within bounds"},"text":{"type":"string","description":"Extracted text with original line separators preserved"},"lines":{"type":"array","description":"Structured line list"},"byte_start":{"type":"integer","description":"UTF-8 byte offset of start"},"byte_end":{"type":"integer","description":"UTF-8 byte offset of end"},"char_start":{"type":"integer","description":"Codepoint index of start"},"char_end":{"type":"integer","description":"Codepoint index of end"},"newline_style":{"type":"string"},"ends_with_newline":{"type":"boolean"},"fingerprint":{"type":"string"},"findings":{"type":"array"}}})
}

pub fn line_range_compare_output() -> Value {
    serde_json::json!({"type":"object","properties":{"equal":{"type":"boolean","description":"True if ranges are equal under the chosen mode"},"left_fingerprint":{"type":"string","description":"SHA-256 fingerprint of left range"},"right_fingerprint":{"type":"string","description":"SHA-256 fingerprint of right range"},"diff_summary":{"type":"string","description":"Human-readable diff summary"},"first_difference":{"type":"object","description":"First differing line (if any)"}}})
}

pub fn prompt_input_inspect_output() -> Value {
    serde_json::json!({"type":"object","properties":{"findings":{"type":"array","description":"Structured findings with code, severity, message, span, and details","items":{"type":"object","properties":{"code":{"type":"string"},"severity":{"type":"string"},"message":{"type":"string"},"span":{"type":"object"},"details":{"type":"object"}}}},"summary":{"type":"string","description":"Human-readable summary"},"risk_score":{"type":"integer","description":"Deterministic risk score"},"recommended_next_tool":{"type":["string","array"],"description":"Recommended follow-up tool(s)"},"text_length":{"type":"integer","description":"Input text length"},"checks_run":{"type":"array","items":{"type":"string"},"description":"Checks that were executed"},"findings_truncated":{"type":"boolean","description":"True if findings were truncated due to limits"}}})
}

pub fn text_security_inspect_output() -> Value {
    serde_json::json!({"type":"object","properties":{"verdict":{"type":"string","enum":["allow","review","block"]},"policy":{"type":"string"},"findings":{"type":"array"},"machine_code":{"type":"string"},"normalized_changed":{"type":"boolean"},"recommended_action":{"type":"string"},"summary":{"type":"string"},"subresults":{"type":"object"}}})
}
