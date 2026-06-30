use crate::mcp::schemas::ToolResponse;
use crate::mcp::server::ToolDefinition;
use crate::mcp::tools::*;
use serde_json::Value;

pub type ToolHandler = fn(&Value) -> ToolResponse;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ToolExposure {
    Default,
    Contextual,
    ExpertOnly,
    HarnessOnly,
    Hidden,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ToolCost {
    Cheap,
    Moderate,
    Heavy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ToolStability {
    Stable,
    Deprecated,
    Experimental,
}

impl ToolExposure {
    pub fn as_str(&self) -> &'static str {
        match self {
            ToolExposure::Default => "default",
            ToolExposure::Contextual => "contextual",
            ToolExposure::ExpertOnly => "expert_only",
            ToolExposure::HarnessOnly => "harness_only",
            ToolExposure::Hidden => "hidden",
        }
    }
}

impl ToolCost {
    pub fn as_str(&self) -> &'static str {
        match self {
            ToolCost::Cheap => "cheap",
            ToolCost::Moderate => "moderate",
            ToolCost::Heavy => "heavy",
        }
    }
}

impl ToolStability {
    pub fn as_str(&self) -> &'static str {
        match self {
            ToolStability::Stable => "stable",
            ToolStability::Deprecated => "deprecated",
            ToolStability::Experimental => "experimental",
        }
    }
}

pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub handler: ToolHandler,
    pub input_schema: fn() -> Value,
    pub output_schema: fn() -> Value,
    pub category: &'static str,
    pub tier: u8,
    pub profiles: &'static [&'static str],
    pub tags: &'static [&'static str],
    pub exposure: ToolExposure,
    pub harness_use: &'static [&'static str],
    pub aliases: &'static [&'static str],
    pub cost: ToolCost,
    pub stability: ToolStability,
    pub composite: bool,
}

// ---------------------------------------------------------------------------
// Input schema functions
// ---------------------------------------------------------------------------

fn math_eval_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "expression": {"type": "string", "description": "Math expression to evaluate (e.g., '5 + 3', '30m + 100ft', 'five plus three')", "maxLength": 10000}
        },
        "required": ["expression"]
    })
}

fn unit_convert_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "value": {"type": "number", "description": "Numeric value to convert (must be finite; NaN and infinity are rejected)"},
            "from_unit": {"type": "string", "description": "Source unit (e.g., 'km', 'ft', 'kg')"},
            "to_unit": {"type": "string", "description": "Target unit (e.g., 'm', 'in', 'lb')"}
        },
        "required": ["value", "from_unit", "to_unit"]
    })
}

fn unit_info_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {"unit": {"type": "string", "description": "Unit name or alias (e.g., 'km', 'kilogram', '℃')"}},
        "required": ["unit"]
    })
}

fn constant_lookup_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {"name": {"type": "string", "description": "Constant name (e.g., 'avogadro', 'planck', 'c', 'G')"}},
        "required": ["name"]
    })
}

fn text_measure_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "Input string to measure"},
            "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "normal", "description": "Detail level for output"}
        },
        "required": ["text"]
    })
}

fn text_equal_input() -> Value {
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

fn text_diff_explain_input() -> Value {
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

fn text_inspect_input() -> Value {
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

fn text_count_input() -> Value {
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

fn text_truncate_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "Input string to truncate"},
            "max_graphemes": {"type": "integer", "minimum": 0, "maximum": 1000000, "description": "Maximum number of grapheme clusters to return"}
        },
        "required": ["text", "max_graphemes"]
    })
}

fn text_transform_input() -> Value {
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

fn validate_brackets_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "Input string"},
            "pairs": {"type": "object", "description": "Bracket pair mapping (default: () [] {} <>)"}
        },
        "required": ["text"]
    })
}

fn validate_json_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {"text": {"type": "string", "description": "Input string to validate as JSON"}},
        "required": ["text"]
    })
}

fn validate_regex_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "pattern": {"type": "string", "description": "Regular expression pattern", "maxLength": 1000},
            "samples": {"type": "array", "items": {"type": "string"}, "description": "List of strings to test against", "maxItems": 100},
            "flags": {"type": "array", "items": {"type": "string"}, "description": "Flag names (IGNORECASE, MULTILINE, etc.)", "maxItems": 10},
            "ignore_case": {"type": "boolean", "default": false, "description": "Use IGNORECASE flag"},
            "multiline": {"type": "boolean", "default": false, "description": "Use MULTILINE flag"},
            "dotall": {"type": "boolean", "default": false, "description": "Use DOTALL flag"},
            "ascii": {"type": "boolean", "default": false, "description": "Use ASCII flag"}
        },
        "required": ["pattern", "samples"]
    })
}

fn list_compare_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "a": {"type": "array", "items": {"type": "string"}, "description": "First list", "maxItems": 10000},
            "b": {"type": "array", "items": {"type": "string"}, "description": "Second list", "maxItems": 10000},
            "mode": {"type": "string", "enum": ["ordered", "set", "multiset"], "default": "set", "description": "Comparison mode: ordered (first diff, aligned ops), set (presence only), multiset (count deltas)"},
            "casefold": {"type": "boolean", "default": false, "description": "Casefold elements before comparison"},
            "normalization": {"type": "string", "enum": ["raw", "NFC", "NFD", "NFKC", "NFKD"], "default": "NFC", "description": "Unicode normalization form"},
            "trim": {"type": "boolean", "default": false, "description": "Trim whitespace from each element"},
            "include_near_matches": {"type": "boolean", "default": false, "description": "Include near matches (fuzzy matching)"},
            "near_match_threshold": {"type": "integer", "default": 2, "description": "Maximum edit distance for near matches"},
            "ignore_order": {"type": "boolean", "description": "Legacy: use mode=set or mode=multiset instead"},
            "treat_as_multiset": {"type": "boolean", "description": "Legacy: use mode=multiset instead"},
        },
        "required": ["a", "b"]
    })
}

fn validate_toml_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "TOML document string to validate"},
            "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "normal"}
        },
        "required": ["text"]
    })
}

fn json_extract_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "JSON document string"},
            "pointer": {"type": "string", "default": "", "description": "RFC 6901 JSON Pointer path (e.g., /dependencies/tokio)"},
            "max_output_chars": {"type": "integer", "default": 4000},
            "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "normal"}
        },
        "required": ["text"]
    })
}

fn json_compare_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "a": {"type": "string", "description": "First JSON document"},
            "b": {"type": "string", "description": "Second JSON document"},
            "ignore_object_order": {"type": "boolean", "default": true},
            "ignore_array_order": {"type": "boolean", "default": false},
            "numeric_string_equivalence": {"type": "boolean", "default": false},
            "casefold_keys": {"type": "boolean", "default": false},
            "max_diffs": {"type": "integer", "default": 50, "minimum": 0, "maximum": 10000},
            "treat_missing_null_as_equal": {"type": "boolean", "default": false},
            "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "normal"}
        },
        "required": ["a", "b"]
    })
}

fn text_position_input() -> Value {
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

fn text_hash_input() -> Value {
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

fn escape_text_input() -> Value {
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

fn unescape_text_input() -> Value {
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

fn identifier_analyze_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string"},
            "languages": {"type": "array", "items": {"type": "string"}, "default": ["python", "rust", "javascript", "env"], "description": "Languages to check (python, rust, javascript, env)"},
            "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "normal"}
        },
        "required": ["text"]
    })
}

fn regex_finditer_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "pattern": {"type": "string", "description": "Regular expression pattern", "maxLength": 1000},
            "text": {"type": "string", "description": "Input string to search"},
            "flags": {"type": "array", "items": {"type": "string"}, "description": "Flag names (IGNORECASE, MULTILINE, DOTALL, etc.)", "maxItems": 10},
            "max_matches": {"type": "integer", "default": 100, "maximum": 1000, "description": "Maximum matches to return"},
            "include_line_column": {"type": "boolean", "default": true, "description": "Include line and column info"},
            "include_groups": {"type": "boolean", "default": true, "description": "Include capture groups"}
        },
        "required": ["pattern", "text"]
    })
}

fn regex_safety_check_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "pattern": {"type": "string", "description": "Regular expression pattern to check"}
        },
        "required": ["pattern"]
    })
}

fn validate_schema_light_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "JSON document to validate"},
            "schema": {"type": "object", "description": "Schema to validate against"},
            "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "normal"}
        },
        "required": ["text", "schema"]
    })
}

fn path_normalize_input() -> Value {
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

fn path_analyze_input() -> Value {
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

fn path_compare_input() -> Value {
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

fn path_scope_check_input() -> Value {
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

fn json_shape_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "JSON document string to analyze"},
            "max_depth": {"type": "integer", "default": 4, "description": "Maximum depth for nested structure"},
            "max_keys": {"type": "integer", "default": 100, "description": "Maximum keys to show per object"},
            "max_array_items": {"type": "integer", "default": 5, "description": "Maximum array item previews"}
        },
        "required": ["text"]
    })
}

fn text_window_input() -> Value {
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

fn json_canonicalize_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "Input JSON string to canonicalize"},
            "sort_keys": {"type": "boolean", "default": true, "description": "Sort object keys alphabetically"},
            "trailing_newline": {"type": "boolean", "default": false, "description": "Add a trailing newline to the canonical form"},
            "indent": {"type": ["integer", "null"], "description": "Indentation spaces (null for minified)"},
            "ensure_ascii": {"type": "boolean", "default": false, "description": "Use ASCII escaping for non-ASCII characters"},
            "detect_duplicate_keys": {"type": "boolean", "default": true, "description": "Report duplicate keys in the input"}
        },
        "required": ["text"]
    })
}

fn json_query_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "JSON document string"},
            "pointer": {"type": "string", "default": "", "description": "RFC 6901 JSON Pointer path (e.g., /foo/bar/0)"}
        },
        "required": ["text"]
    })
}

fn glob_match_input() -> Value {
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

fn text_fingerprint_input() -> Value {
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

fn identifier_inspect_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "identifiers": {"type": "array", "items": {"type": "string"}, "description": "List of identifier strings to inspect", "maxItems": 10000},
            "language": {"type": "string", "enum": ["generic", "python", "rust", "javascript", "typescript", "json_key"], "default": "generic", "description": "Language for validation"},
            "normalization": {"type": "string", "enum": ["raw", "NFC", "NFD", "NFKC", "NFKD"], "default": "NFC", "description": "Unicode normalization form"},
            "casefold": {"type": "boolean", "default": false, "description": "Apply casefolding for collision detection"},
            "check_confusables": {"type": "boolean", "default": true, "description": "Check for confusable characters"}
        },
        "required": ["identifiers"]
    })
}

fn version_compare_input() -> Value {
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

fn toml_shape_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "TOML document string"},
            "max_tables": {"type": "integer", "default": 100, "minimum": 1, "maximum": 100000, "description": "Maximum tables to return"},
            "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "normal"}
        },
        "required": ["text"]
    })
}

fn list_dedupe_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "items": {"type": "array", "items": {"type": "string"}, "description": "List of strings to dedupe", "maxItems": 10000},
            "normalization": {"type": "string", "enum": ["raw", "NFC", "NFD", "NFKC", "NFKD"], "default": "NFC"},
            "casefold": {"type": "boolean", "default": false, "description": "Apply casefolding before comparison"},
            "stable": {"type": "boolean", "default": true, "description": "Accepted for compatibility; deduplication keeps first occurrence order"}
        },
        "required": ["items"]
    })
}

fn list_sort_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "items": {"type": "array", "items": {"type": "string"}, "description": "List of strings to sort", "maxItems": 10000},
            "normalization": {"type": "string", "enum": ["raw", "NFC", "NFD", "NFKC", "NFKD"], "default": "NFC"},
            "casefold": {"type": "boolean", "default": false, "description": "Apply casefolding for sorting"},
            "reverse": {"type": "boolean", "default": false, "description": "Sort in descending order"},
            "stable": {"type": "boolean", "default": true, "description": "Accepted for compatibility; Python sorting is always stable"}
        },
        "required": ["items"]
    })
}

fn text_replace_check_input() -> Value {
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

fn line_range_extract_input() -> Value {
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

fn line_range_compare_input() -> Value {
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

fn shell_split_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "command": {"type": "string", "description": "The shell command string to parse"},
            "shell": {"type": "string", "enum": ["posix"], "default": "posix", "description": "Shell dialect (only posix is supported)"},
            "detect_risky_features": {"type": "boolean", "default": true, "description": "Whether to detect risky lexical features"}
        },
        "required": ["command"]
    })
}

fn shell_quote_join_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "argv": {"type": "array", "items": {"type": "string"}, "description": "List of argument strings to join", "maxItems": 10000},
            "shell": {"type": "string", "enum": ["posix"], "default": "posix", "description": "Shell dialect (only posix is supported)"}
        },
        "required": ["argv"]
    })
}

fn argv_compare_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "left_command": {"type": "string", "description": "Left command string to parse and compare"},
            "right_command": {"type": "string", "description": "Right command string to parse and compare"},
            "left_argv": {"type": "array", "items": {"type": "string"}, "description": "Left pre-parsed argv list", "maxItems": 10000},
            "right_argv": {"type": "array", "items": {"type": "string"}, "description": "Right pre-parsed argv list", "maxItems": 10000},
            "shell": {"type": "string", "enum": ["posix"], "default": "posix", "description": "Shell dialect (only posix is supported)"}
        },
        "required": []
    })
}

fn markdown_structure_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "Markdown text to analyze"},
            "include_sections": {"type": "boolean", "default": true, "description": "Include heading detection"},
            "include_links": {"type": "boolean", "default": true, "description": "Include link detection"},
            "include_code_fences": {"type": "boolean", "default": true, "description": "Include code fence detection"},
            "include_html_comments": {"type": "boolean", "default": true, "description": "Include HTML comment detection"}
        },
        "required": ["text"]
    })
}

fn code_fence_extract_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "Markdown text to scan"},
            "language": {"type": "string", "description": "Optional language filter (case-insensitive)"},
            "include_content": {"type": "boolean", "default": true, "description": "Include block content in output"}
        },
        "required": ["text"]
    })
}

fn dotenv_validate_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": ".env file content to validate"},
            "allow_export": {"type": "boolean", "default": true, "description": "Allow export KEY=VALUE syntax"},
            "key_pattern": {"type": "string", "default": "^[A-Za-z_][A-Za-z0-9_]*$", "description": "Regex pattern keys must match"},
            "duplicate_policy": {"type": "string", "enum": ["warn", "error", "allow"], "default": "warn", "description": "How to handle duplicate keys"}
        },
        "required": ["text"]
    })
}

fn ini_validate_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "INI file content to validate"},
            "duplicate_policy": {"type": "string", "enum": ["warn", "error", "allow"], "default": "warn", "description": "How to handle duplicate keys/sections"}
        },
        "required": ["text"]
    })
}

fn patch_apply_check_input() -> Value {
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

fn patch_summary_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {"patch_text": {"type": "string", "description": "The unified diff text to summarize"}},
        "required": ["patch_text"]
    })
}

fn unicode_policy_check_input() -> Value {
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

fn canonicalize_text_input() -> Value {
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

fn identifier_table_inspect_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "identifiers": {"type": "array", "items": {"type": "object", "properties": {"name": {"type": "string", "description": "Identifier name (required)"}, "kind": {"type": "string", "description": "Optional kind/category"}, "file": {"type": "string", "description": "Source file path"}, "line": {"type": "integer", "description": "Line number"}}, "required": ["name"]}, "description": "List of identifier entries to inspect", "maxItems": 10000},
            "language": {"type": "string", "enum": ["generic", "python", "rust", "javascript", "typescript", "json_key"], "default": "python", "description": "Target language for reserved keyword checking"},
            "checks": {"type": "array", "items": {"type": "string"}, "description": "Subset of checks: casefold, normalization, confusable, style, reserved, mixed_style"}
        },
        "required": ["identifiers"]
    })
}

fn version_constraint_check_input() -> Value {
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

fn cargo_toml_inspect_input() -> Value {
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

fn prompt_input_inspect_input() -> Value {
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

fn text_security_inspect_input() -> Value {
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

fn edit_preflight_input() -> Value {
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

fn command_preflight_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "command": {"type": "string", "description": "Command string to analyze"},
            "platform": {"type": "string", "enum": ["posix", "windows", "auto"], "default": "posix", "description": "Target platform"},
            "policy": {"type": "string", "enum": ["default", "strict", "permissive"], "default": "default", "description": "Analysis policy"},
            "working_directory": {"type": "string", "description": "Working directory context (informational)"}
        },
        "required": ["command"]
    })
}

fn config_preflight_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": {"type": "string", "description": "Config text to validate"},
            "format": {"type": "string", "enum": ["auto", "json", "toml", "dotenv", "ini", "cargo_toml"], "default": "auto", "description": "Config format (auto-detect if not specified)"},
            "schema": {"type": "object", "description": "Optional JSON schema for validation"},
            "strict": {"type": "boolean", "default": false, "description": "Strict validation mode"}
        },
        "required": ["text"]
    })
}

fn structured_data_compare_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "a": {"type": "string", "description": "First JSON string"},
            "b": {"type": "string", "description": "Second JSON string"},
            "format": {"type": "string", "enum": ["json"], "default": "json", "description": "Data format (json only for now)"},
            "ignore_object_order": {"type": "boolean", "default": true, "description": "Ignore object key order"},
            "ignore_array_order": {"type": "boolean", "default": false, "description": "Sort arrays before comparison"},
            "max_diffs": {"type": "integer", "default": 50, "description": "Maximum differences to report"}
        },
        "required": ["a", "b"]
    })
}

// ---------------------------------------------------------------------------
// Output schema functions
// ---------------------------------------------------------------------------

fn math_eval_output() -> Value {
    serde_json::json!({"type":"object","properties":{"value":{"type":"string","description":"Evaluation result as string"},"type":{"type":"string","description":"Python type name of the result"},"unit":{"type":["string","null"],"description":"Unit name (only when result has units)"},"display":{"type":["string","null"],"description":"Human-readable result with units (only when result has units)"}}})
}

fn unit_convert_output() -> Value {
    serde_json::json!({"type":"object","properties":{"value":{"type":"number","description":"Converted value"},"from_unit":{"type":"string"},"to_unit":{"type":"string"},"factor":{"type":["number","null"],"description":"Conversion factor used (null for temperature conversions)"}}})
}

fn unit_info_output() -> Value {
    serde_json::json!({"type":"object","properties":{"unit":{"type":"string"},"canonical":{"type":"string","description":"Canonical unit name"},"category":{"type":"string","description":"Unit category (e.g., 'length', 'mass', 'temperature')"},"is_valid":{"type":"boolean"}}})
}

fn constant_lookup_output() -> Value {
    serde_json::json!({"type":"object","properties":{"name":{"type":"string"},"value":{"type":"number","description":"Constant value"},"symbol":{"type":"string","description":"Display symbol (e.g., 'N_A', 'h', 'c')"},"display_name":{"type":"string","description":"Human-readable name"}}})
}

fn text_measure_output() -> Value {
    serde_json::json!({"type":"object","properties":{"bytes_utf8":{"type":"integer"},"codepoints":{"type":"integer"},"graphemes":{"type":"integer"},"words":{"type":"integer"},"unique_words_casefolded":{"type":"integer"},"lines":{"type":"integer"},"nonempty_lines":{"type":"integer"},"blank_lines":{"type":"integer"},"max_line_length_codepoints":{"type":"integer"},"chars_no_whitespace":{"type":"integer"},"ascii":{"type":"integer"},"non_ascii":{"type":"integer"},"letters":{"type":"integer"},"digits":{"type":"integer"},"punctuation":{"type":"integer"},"symbols":{"type":"integer"},"spaces":{"type":"integer"},"control_chars":{"type":"integer"},"combining_marks":{"type":"integer"},"invisible_chars":{"type":"integer"},"newline_style":{"type":"string"},"ends_with_newline":{"type":"boolean"},"normalization":{"type":"object"},"unicode_risks":{"type":"object"},"warnings":{"type":"array"}}})
}

fn text_equal_output() -> Value {
    serde_json::json!({"type":"object","properties":{"equal":{"type":"boolean"},"mode":{"type":"object"},"raw_equal":{"type":"boolean"},"nfc_equal":{"type":"boolean"},"nfd_equal":{"type":"boolean"},"nfkc_equal":{"type":"boolean"},"nfkd_equal":{"type":"boolean"},"casefold_equal":{"type":"boolean"},"byte_equal":{"type":"boolean"},"lengths":{"type":"object"},"first_difference":{"type":["object","null"]},"classification":{"type":"string"}}})
}

fn text_diff_explain_output() -> Value {
    serde_json::json!({"type":"object","properties":{"equal":{"type":"boolean"},"classification":{"type":"string"},"summary":{"type":"object"},"a_metrics":{"type":"object"},"b_metrics":{"type":"object"},"diffs":{"type":"array"},"security_findings":{"type":"array"},"agent_instruction":{"type":"string"}}})
}

fn text_inspect_output() -> Value {
    serde_json::json!({"type":"object","properties":{"safe_repr":{"type":"string"},"metrics":{"type":"object"},"normalization":{"type":"object"},"normalization_diff":{"type":"boolean"},"normals_repr":{"type":["string","null"]},"invisibles":{"type":"array"},"bidi_controls":{"type":"array"},"mixed_scripts":{"type":"object"},"confusables":{"type":"array"},"warnings":{"type":"array"},"limits_applied":{"type":"array"},"normalize":{"type":"string"},"compare_normalized":{"type":"boolean"},"original":{"type":"object"},"normalized":{"type":["object","null"]},"normalization_findings":{"type":"array"}}})
}

fn text_count_output() -> Value {
    serde_json::json!({"type":"object","description":"With target: {count, positions, target, normalization, text_length_codepoints}. Without target: character frequency table as {char: count} pairs.","properties":{"count":{"type":"integer"},"positions":{"type":"array"},"target":{"type":["string","null"]},"normalization":{"type":["string","null"]},"text_length_codepoints":{"type":"integer"}}})
}

fn text_truncate_output() -> Value {
    serde_json::json!({"type":"object","properties":{"text":{"type":"string","description":"Result string (truncated if truncation occurred)"},"original_graphemes":{"type":"integer","description":"Original grapheme count"},"truncated_graphemes":{"type":"integer","description":"Grapheme count in result"},"truncated":{"type":"boolean","description":"True if text was truncated"}}})
}

fn text_transform_output() -> Value {
    serde_json::json!({"type":"object","properties":{"changed":{"type":"boolean"},"text":{"type":"string"},"operations_applied":{"type":"array","items":{"type":"string"}},"removed":{"type":"array"},"warnings":{"type":"array","items":{"type":"string"}},"summary":{"type":"string"}}})
}

fn validate_brackets_output() -> Value {
    serde_json::json!({"type":"object","properties":{"balanced":{"type":"boolean"},"unmatched_openers":{"type":"array"},"unmatched_closers":{"type":"array"}}})
}

fn validate_json_output() -> Value {
    serde_json::json!({"type":"object","properties":{"valid":{"type":"boolean"},"error":{"type":["string","null"]},"line":{"type":["integer","null"]},"column":{"type":["integer","null"]},"position":{"type":["integer","null"]},"type":{"type":["string","null"]},"top_level_keys":{"type":["array","null"],"items":{"type":"string"}}}})
}

fn validate_regex_output() -> Value {
    serde_json::json!({"type":"object","properties":{"valid_pattern":{"type":"boolean"},"results":{"type":"array"},"error":{"type":["string","null"]},"flags_used":{"type":"object"}}})
}

fn list_compare_output() -> Value {
    serde_json::json!({"type":"object","properties":{"equal":{"type":"boolean"},"first_diff_index":{"type":["integer","null"],"description":"Index of first difference (ordered mode)"},"equal_prefix_length":{"type":"integer","description":"Length of equal prefix (ordered mode)"},"aligned":{"type":"array","description":"Aligned operations (ordered mode)"},"count_deltas":{"type":"object","description":"Count differences (multiset mode)"},"only_in_a":{"type":"array"},"only_in_b":{"type":"array"},"missing_in_a":{"type":"array","description":"Alias for only_in_b"},"missing_in_b":{"type":"array","description":"Alias for only_in_a"},"duplicates_in_a":{"type":"array"},"duplicates_in_b":{"type":"array"},"near_matches":{"type":"array","description":"Items that differ only by edit distance"}}})
}

fn validate_toml_output() -> Value {
    serde_json::json!({"type":"object","properties":{"valid":{"type":"boolean"},"error":{"type":["string","null"]},"line":{"type":["integer","null"]},"column":{"type":["integer","null"]},"position":{"type":["integer","null"]},"type":{"type":["string","null"]},"top_level_keys":{"type":["array","null"]},"tables":{"type":["array","null"]}}})
}

fn json_extract_output() -> Value {
    serde_json::json!({"type":"object","properties":{"valid_json":{"type":"boolean"},"found":{"type":"boolean"},"pointer":{"type":"string"},"value_type":{"type":["string","null"]},"value":{"description":"Extracted value"},"preview":{"type":["string","null"]},"child_keys":{"type":["array","null"],"items":{"type":"string"}},"array_length":{"type":["integer","null"]},"truncated":{"type":"boolean"},"missing_at":{"type":["string","null"]},"reason":{"type":["string","null"]},"available_keys":{"type":["array","null"],"items":{"type":"string"}},"error":{"type":["string","null"]},"line":{"type":["integer","null"]},"column":{"type":["integer","null"]},"summary":{"type":"string"}}})
}

fn json_compare_output() -> Value {
    serde_json::json!({"type":"object","properties":{"valid_json_a":{"type":"boolean"},"valid_json_b":{"type":"boolean"},"equal":{"type":"boolean"},"same_type":{"type":"boolean"},"diff_count":{"type":"integer"},"diffs":{"type":"array","description":"List of differences"},"truncated":{"type":"boolean"},"summary":{"type":"string"}}})
}

fn text_position_output() -> Value {
    serde_json::json!({"type":"object","properties":{"valid":{"type":"boolean"},"byte_offset":{"type":["integer","null"]},"codepoint_index":{"type":["integer","null"]},"utf16_offset":{"type":["integer","null"]},"line":{"type":["integer","null"]},"column":{"type":["integer","null"]},"line_base":{"type":"integer"},"column_base":{"type":"integer"},"char":{"type":["string","null"]},"codepoint":{"type":["string","null"]},"name":{"type":["string","null"]},"line_text_preview":{"type":["string","null"]},"error":{"type":["string","null"]},"summary":{"type":"string"}}})
}

fn text_hash_output() -> Value {
    serde_json::json!({"type":"object","properties":{"encoding":{"type":"string"},"bytes":{"type":"integer"},"codepoints":{"type":"integer"},"hashes":{"type":"object","description":"Map of algorithm to hash value"},"warnings":{"type":"array","items":{"type":"string"}},"summary":{"type":"string"}}})
}

fn escape_text_output() -> Value {
    serde_json::json!({"type":"object","properties":{"mode":{"type":"string"},"escaped":{"type":"string"},"changed":{"type":"boolean"},"summary":{"type":"string"}}})
}

fn unescape_text_output() -> Value {
    serde_json::json!({"type":"object","properties":{"mode":{"type":"string"},"unescaped":{"type":"string"},"changed":{"type":"boolean"},"error":{"type":["string","null"]},"summary":{"type":"string"}}})
}

fn identifier_analyze_output() -> Value {
    serde_json::json!({"type":"object","properties":{"text":{"type":"string"},"classification":{"type":"string"},"python_valid":{"type":"boolean"},"python_keyword":{"type":"boolean"},"rust_valid":{"type":["boolean","null"]},"javascript_valid":{"type":["boolean","null"]},"env_valid":{"type":"boolean"},"suggestions":{"type":"object","description":"Map of language to suggested name"},"warnings":{"type":"array","items":{"type":"string"}},"summary":{"type":"string"}}})
}

fn regex_finditer_output() -> Value {
    serde_json::json!({"type":"object","properties":{"valid_pattern":{"type":"boolean"},"matches":{"type":"array","description":"List of regex matches with positions and groups"},"truncated":{"type":"boolean"},"match_count":{"type":"integer"},"error":{"type":["string","null"]}}})
}

fn regex_safety_check_output() -> Value {
    serde_json::json!({"type":"object","properties":{"valid_pattern":{"type":"boolean"},"risk":{"type":"string","enum":["low","medium","high"]},"findings":{"type":"array","description":"Safety findings with kind, span, and message","items":{"type":"object","properties":{"kind":{"type":"string"},"span":{"type":"array","items":{"type":"integer"}},"message":{"type":"string"}}}}}})
}

fn validate_schema_light_output() -> Value {
    serde_json::json!({"type":"object","properties":{"valid":{"type":"boolean"},"violations":{"type":"array","description":"Schema violations with path and message","items":{"type":"object","properties":{"path":{"type":"string"},"message":{"type":"string"},"value_type":{"type":["string","null"]},"expected_type":{"type":["string","null"]}}}},"truncated":{"type":"boolean"},"summary":{"type":"string"}}})
}

fn path_normalize_output() -> Value {
    serde_json::json!({"type":"object","properties":{"normalized":{"type":"string"},"is_absolute":{"type":"boolean"},"components":{"type":"array","items":{"type":"string"}},"warnings":{"type":"array","items":{"type":"string"}}}})
}

fn path_analyze_output() -> Value {
    serde_json::json!({"type":"object","properties":{"input":{"type":"string"},"style":{"type":"string"},"absolute":{"type":"boolean"},"has_traversal":{"type":"boolean"},"components":{"type":"array","items":{"type":"string"}},"parent":{"type":["string","null"]},"name":{"type":["string","null"]},"stem":{"type":["string","null"]},"suffix":{"type":["string","null"]},"suffixes":{"type":"array","items":{"type":"string"}},"hidden":{"type":"boolean"},"normalized_lexical":{"type":"string"},"warnings":{"type":"array","items":{"type":"string"}},"summary":{"type":"string"}}})
}

fn path_compare_output() -> Value {
    serde_json::json!({"type":"object","properties":{"equal":{"type":"boolean","description":"Whether paths are equal under normalization"},"left_normalized":{"type":"string","description":"Normalized left path"},"right_normalized":{"type":"string","description":"Normalized right path"},"differences":{"type":"array","description":"List of differences found"},"findings":{"type":"array","description":"Normalization notes"}}})
}

fn path_scope_check_output() -> Value {
    serde_json::json!({"type":"object","properties":{"inside_root":{"type":"boolean","description":"Whether target is lexically inside root"},"root_normalized":{"type":"string","description":"Normalized root path"},"target_normalized":{"type":"string","description":"Normalized target path"},"relative_path":{"type":"string","description":"Relative path from root to target (if inside)"},"escapes_via_dotdot":{"type":"boolean","description":"Whether target contains parent traversal"},"absolute_target":{"type":"string","description":"Absolute form of target"},"findings":{"type":"array","description":"Analysis notes"}}})
}

fn json_shape_output() -> Value {
    serde_json::json!({"type":"object","properties":{"valid":{"type":"boolean"},"shape":{"type":["object","null"],"description":"Nested shape structure with type, keys, and counts","properties":{"type":{"type":"string"},"keys":{"type":["object","null"]},"key_count":{"type":["integer","null"]},"item_types":{"type":["array","null"]},"item_count":{"type":["integer","null"]}}},"truncated":{"type":"boolean"},"summary":{"type":"string"}}})
}

fn text_window_output() -> Value {
    serde_json::json!({"type":"object","properties":{"position":{"type":"object","description":"Resolved position with byte_offset, codepoint_index, grapheme_index, line, column"},"line_text":{"type":"string"},"line_visible_repr":{"type":"string"},"before":{"type":"array","description":"Context lines before"},"after":{"type":"array","description":"Context lines after"},"newline_style":{"type":"string"},"at_codepoint":{"type":["object","null"]},"warnings":{"type":"array","items":{"type":"string"}}}})
}

fn json_canonicalize_output() -> Value {
    serde_json::json!({"type":"object","properties":{"valid":{"type":"boolean"},"canonical":{"type":["string","null"]},"minified":{"type":["string","null"]},"sha256":{"type":["string","null"]},"duplicate_keys":{"type":"array","items":{"type":"string"}},"top_level_type":{"type":["string","null"]},"top_level_keys":{"type":["array","null"],"items":{"type":"string"}},"error":{"type":["string","null"]},"line":{"type":["integer","null"]},"column":{"type":["integer","null"]}}})
}

fn json_query_output() -> Value {
    serde_json::json!({"type":"object","properties":{"found":{"type":"boolean"},"pointer":{"type":"string"},"value":{"description":"Extracted value"},"type":{"type":["string","null"]},"missing_at":{"type":["string","null"]},"reason":{"type":["string","null"]},"error":{"type":["string","null"]},"line":{"type":["integer","null"]},"column":{"type":["integer","null"]}}})
}

fn glob_match_output() -> Value {
    serde_json::json!({"type":"object","properties":{"matches":{"type":"boolean"},"normalized_pattern":{"type":"string"},"normalized_path":{"type":"string"},"matched_segment":{"type":["string","null"]},"unmatched_segment":{"type":["string","null"]},"summary":{"type":"string"}}})
}

fn text_fingerprint_output() -> Value {
    serde_json::json!({"type":"object","properties":{"sha256":{"type":"string"},"bytes_utf8":{"type":"integer"},"codepoints":{"type":"integer"},"graphemes":{"type":"integer"},"newline_style":{"type":"string"},"normalization":{"type":"object","description":"Normalization state details"},"summary":{"type":"string"}}})
}

fn identifier_inspect_output() -> Value {
    serde_json::json!({"type":"object","properties":{"identifiers":{"type":"array","description":"Per-identifier analysis with raw, normalized, valid, scripts, and issues"},"collisions":{"type":"array","description":"Detected collisions between identifiers"}}})
}

fn version_compare_output() -> Value {
    serde_json::json!({"type":"object","properties":{"comparison":{"type":"integer","description":"Comparison result: -1 (a < b), 0 (equal), 1 (a > b)"},"valid":{"type":"boolean","description":"Whether versions are valid for the scheme"},"scheme":{"type":"string"},"summary":{"type":"string"}}})
}

fn toml_shape_output() -> Value {
    serde_json::json!({"type":"object","properties":{"valid":{"type":"boolean"},"top_level_keys":{"type":["array","null"],"items":{"type":"string"}},"tables":{"type":["array","null"],"items":{"type":"string"}},"truncated":{"type":"boolean"},"summary":{"type":"string"}}})
}

fn list_dedupe_output() -> Value {
    serde_json::json!({"type":"object","properties":{"items":{"type":"array","items":{"type":"string"}},"original_count":{"type":"integer"},"deduped_count":{"type":"integer"},"duplicates_removed":{"type":"integer"}}})
}

fn list_sort_output() -> Value {
    serde_json::json!({"type":"object","properties":{"items":{"type":"array","items":{"type":"string"}},"original_count":{"type":"integer"},"sorted_count":{"type":"integer"}}})
}

fn text_replace_check_output() -> Value {
    serde_json::json!({"type":"object","properties":{"match_count":{"type":"integer","description":"Number of matches found"},"unique_match":{"type":"boolean","description":"True if exactly one match"},"expected_count_met":{"type":"boolean","description":"True if match count matches expected_count"},"would_change":{"type":"boolean","description":"True if replacement would change text"},"positions":{"type":"array","description":"Match positions with byte offsets and line/column"},"changed_text_fingerprint":{"type":"string","description":"SHA-256 fingerprint of changed text"},"newline_style_before":{"type":"string"},"newline_style_after":{"type":"string"},"preview_before":{"type":"string"},"preview_after":{"type":"string"},"findings":{"type":"array","description":"Warnings and info messages"}}})
}

fn line_range_extract_output() -> Value {
    serde_json::json!({"type":"object","properties":{"line_count_total":{"type":"integer","description":"Total line count in input"},"start_line":{"type":"integer"},"end_line":{"type":"integer"},"valid_range":{"type":"boolean","description":"True if range is within bounds"},"text":{"type":"string","description":"Extracted text with original line separators preserved"},"lines":{"type":"array","description":"Structured line list"},"byte_start":{"type":"integer","description":"UTF-8 byte offset of start"},"byte_end":{"type":"integer","description":"UTF-8 byte offset of end"},"char_start":{"type":"integer","description":"Codepoint index of start"},"char_end":{"type":"integer","description":"Codepoint index of end"},"newline_style":{"type":"string"},"ends_with_newline":{"type":"boolean"},"fingerprint":{"type":"string"},"findings":{"type":"array"}}})
}

fn line_range_compare_output() -> Value {
    serde_json::json!({"type":"object","properties":{"equal":{"type":"boolean","description":"True if ranges are equal under the chosen mode"},"left_fingerprint":{"type":"string","description":"SHA-256 fingerprint of left range"},"right_fingerprint":{"type":"string","description":"SHA-256 fingerprint of right range"},"diff_summary":{"type":"string","description":"Human-readable diff summary"},"first_difference":{"type":"object","description":"First differing line (if any)"}}})
}

fn shell_split_output() -> Value {
    serde_json::json!({"type":"object","properties":{"parse_ok":{"type":"boolean","description":"True if the command parsed successfully"},"argv":{"type":"array","items":{"type":"string"},"description":"Parsed argument tokens"},"argc":{"type":"integer","description":"Number of arguments"},"features":{"type":"object","description":"Detected risky features","properties":{"has_pipe":{"type":"boolean"},"has_redirection":{"type":"boolean"},"has_command_substitution":{"type":"boolean"},"has_variable_expansion":{"type":"boolean"},"has_glob_pattern":{"type":"boolean"},"has_control_operator":{"type":"boolean"},"has_unbalanced_quotes":{"type":"boolean"}}},"findings":{"type":"array","items":{"type":"string"},"description":"Analysis notes and warnings"}}})
}

fn shell_quote_join_output() -> Value {
    serde_json::json!({"type":"object","properties":{"command":{"type":"string","description":"Safely quoted command string"},"roundtrip_ok":{"type":"boolean","description":"True if shell_split(quote_join(argv)) produces equivalent argv"},"findings":{"type":"array","items":{"type":"string"},"description":"Analysis notes"}}})
}

fn argv_compare_output() -> Value {
    serde_json::json!({"type":"object","properties":{"argv_equal":{"type":"boolean","description":"True if parsed argv lists are identical"},"left_argv":{"type":"array","items":{"type":"string"},"description":"Resolved left argv"},"right_argv":{"type":"array","items":{"type":"string"},"description":"Resolved right argv"},"first_difference":{"type":"integer","description":"Index of first differing token, or null if equal"},"findings":{"type":"array","items":{"type":"string"},"description":"Analysis notes"}}})
}

fn markdown_structure_output() -> Value {
    serde_json::json!({"type":"object","properties":{"headings":{"type":"array","description":"Headings with level, text, line, slug"},"code_fences":{"type":"array","description":"Code fences with language, lines, closed state"},"links":{"type":"array","description":"Links with visible text, target, mismatch flags"},"html_comments":{"type":"array","description":"HTML comments with text and position"},"frontmatter":{"type":"object","description":"Frontmatter detection (present, format, line range)"},"tables_detected":{"type":"boolean","description":"Whether Markdown tables were detected"},"findings":{"type":"array","items":{"type":"string"},"description":"Warnings and findings"}}})
}

fn code_fence_extract_output() -> Value {
    serde_json::json!({"type":"object","properties":{"blocks":{"type":"array","description":"Extracted code blocks with index, language, lines, content, fingerprint"},"unclosed_fences":{"type":"array","description":"Unclosed code fences found"},"findings":{"type":"array","items":{"type":"string"},"description":"Warnings and findings"}}})
}

fn dotenv_validate_output() -> Value {
    serde_json::json!({"type":"object","properties":{"parse_ok":{"type":"boolean","description":"True if no parse errors found"},"entries":{"type":"array","description":"Parsed entries with key, value, quote_style, line"},"duplicates":{"type":"array","description":"Duplicate key entries with line numbers"},"invalid_lines":{"type":"array","description":"Lines that failed to parse"},"requires_quoting":{"type":"array","description":"Keys whose values contain spaces and should be quoted"},"contains_expansion_syntax":{"type":"array","description":"Keys with ${VAR} or $VAR expansion syntax"},"findings":{"type":"array","items":{"type":"string"},"description":"Human-readable findings"}}})
}

fn ini_validate_output() -> Value {
    serde_json::json!({"type":"object","properties":{"parse_ok":{"type":"boolean","description":"True if no parse errors found"},"sections":{"type":"array","description":"Ordered list of section names"},"keys_by_section":{"type":"object","description":"Keys grouped by section"},"duplicates":{"type":"array","description":"Duplicate keys/sections with line numbers"},"invalid_lines":{"type":"array","description":"Lines that failed to parse"},"findings":{"type":"array","items":{"type":"string"},"description":"Human-readable findings"}}})
}

fn patch_apply_check_output() -> Value {
    serde_json::json!({"type":"object","properties":{"patch_parse_ok":{"type":"boolean","description":"True if patch parsed successfully"},"applies":{"type":"boolean","description":"True if all hunks applied cleanly"},"hunks_total":{"type":"integer","description":"Total number of hunks in patch"},"hunks_applied":{"type":"integer","description":"Number of hunks that applied successfully"},"hunks_failed":{"type":"integer","description":"Number of hunks that failed to apply"},"failed_hunks":{"type":"array","description":"Details of each failed hunk","items":{"type":"object","properties":{"hunk_index":{"type":"integer"},"old_start":{"type":"integer"},"old_count":{"type":"integer"},"expected_context":{"type":"array","items":{"type":"string"}},"actual_context":{"type":"array","items":{"type":"string"}},"reason":{"type":"string"}}}},"affected_line_ranges":{"type":"array","description":"Line ranges affected by successful hunks","items":{"type":"object","properties":{"start":{"type":"integer"},"end":{"type":"integer"}}}},"newline_style_before":{"type":"string","description":"Newline style in original text"},"newline_style_after":{"type":"string","description":"Newline style in result text"},"result_fingerprint":{"type":"string","description":"SHA-256 of the result text"},"result_text":{"type":["string","null"],"description":"Resulting text if requested"},"findings":{"type":"array","items":{"type":"string"},"description":"Analysis notes and warnings"}}})
}

fn patch_summary_output() -> Value {
    serde_json::json!({"type":"object","properties":{"files_changed":{"type":"integer","description":"Number of files changed"},"hunks_total":{"type":"integer","description":"Total number of hunks across all files"},"additions":{"type":"integer","description":"Total number of added lines"},"deletions":{"type":"integer","description":"Total number of deleted lines"},"renames_detected":{"type":"array","description":"Detected file renames","items":{"type":"object","properties":{"from":{"type":"string"},"to":{"type":"string"}}}},"binary_patch_detected":{"type":"boolean","description":"True if binary patch content detected"},"line_ranges_by_file":{"type":"object","description":"Line ranges affected per file","additionalProperties":{"type":"array","items":{"type":"object","properties":{"start":{"type":"integer"},"end":{"type":"integer"}}}}},"findings":{"type":"array","items":{"type":"string"},"description":"Analysis notes and warnings"}}})
}

fn unicode_policy_check_output() -> Value {
    serde_json::json!({"type":"object","properties":{"pass_":{"type":"boolean","description":"True if text passes the policy (no errors)"},"policy":{"type":"string","description":"Policy name that was applied"},"normalized_form":{"type":"string","description":"Text after normalization"},"findings":{"type":"array","description":"Policy findings with rule, severity, and message","items":{"type":"object","properties":{"rule":{"type":"string"},"severity":{"type":"string"},"message":{"type":"string"}}}},"summary":{"type":"string","description":"Human-readable summary"}}})
}

fn canonicalize_text_output() -> Value {
    serde_json::json!({"type":"object","properties":{"text":{"type":"string","description":"Canonicalized text"},"changed":{"type":"boolean","description":"True if text was modified"},"operations_applied":{"type":"array","description":"List of operations applied"},"fingerprint_before":{"type":"string","description":"SHA-256 of original text"},"fingerprint_after":{"type":"string","description":"SHA-256 of canonicalized text"},"mapping":{"type":"array","description":"Character mapping if return_mapping was True"},"findings":{"type":"array","items":{"type":"string"},"description":"Analysis notes and warnings"}}})
}

fn identifier_table_inspect_output() -> Value {
    serde_json::json!({"type":"object","properties":{"count":{"type":"integer","description":"Number of identifiers inspected"},"collisions":{"type":"array","description":"Detected collisions","items":{"type":"object","properties":{"kind":{"type":"string"},"names":{"type":"array","items":{"type":"string"}},"detail":{"type":"string"}}}},"reserved_keyword_hits":{"type":"array","description":"Identifiers matching reserved keywords","items":{"type":"object","properties":{"name":{"type":"string"},"language":{"type":"string"},"file":{"type":"string"},"line":{"type":"integer"}}}},"mixed_style_groups":{"type":"array","description":"Groups with mixed naming styles","items":{"type":"object","properties":{"stripped":{"type":"string"},"names":{"type":"array","items":{"type":"string"}},"styles":{"type":"array","items":{"type":"string"}}}}},"findings":{"type":"array","items":{"type":"string"}}}})
}

fn version_constraint_check_output() -> Value {
    serde_json::json!({"type":"object","properties":{"satisfies":{"type":"boolean","description":"Whether the version satisfies the constraint"},"parsed_version":{"type":"object","description":"Parsed version components"},"parsed_constraint":{"type":"object","description":"Parsed constraint components"},"scheme":{"type":"string","description":"Versioning scheme used"},"explanation":{"type":"string","description":"Human-readable explanation"},"findings":{"type":"array","items":{"type":"string"},"description":"Analysis notes and warnings"}}})
}

fn cargo_toml_inspect_output() -> Value {
    serde_json::json!({"type":"object","properties":{"parse_ok":{"type":"boolean","description":"Whether TOML parsed successfully"},"package":{"type":"object","description":"Package metadata from [package] section","properties":{"name":{"type":"string"},"version":{"type":"string"},"edition":{"type":"string"},"license":{"type":"string"},"repository":{"type":"string"},"readme":{"type":"string"}}},"workspace":{"type":"object","description":"Workspace section information","properties":{"present":{"type":"boolean"},"members":{"type":"array","items":{"type":"string"}},"exclude":{"type":"array","items":{"type":"string"}}}},"dependencies":{"type":"object","description":"Dependencies by section"},"path_dependencies":{"type":"array","items":{"type":"string"},"description":"Extracted path dependency values"},"suspicious_dependency_names":{"type":"array","items":{"type":"string"},"description":"Dependency names with suspicious patterns"},"duplicate_or_confusable_dependency_names":{"type":"array","items":{"type":"string"},"description":"Dependency names that normalize to the same form"},"findings":{"type":"array","items":{"type":"string"},"description":"Structural findings and warnings"}}})
}

fn prompt_input_inspect_output() -> Value {
    serde_json::json!({"type":"object","properties":{"findings":{"type":"array","description":"Structured findings with code, severity, message, span, and details","items":{"type":"object","properties":{"code":{"type":"string"},"severity":{"type":"string"},"message":{"type":"string"},"span":{"type":"object"},"details":{"type":"object"}}}},"summary":{"type":"string","description":"Human-readable summary"},"risk_score":{"type":"integer","description":"Deterministic risk score"},"recommended_next_tool":{"type":["string","array"],"description":"Recommended follow-up tool(s)"},"text_length":{"type":"integer","description":"Input text length"},"checks_run":{"type":"array","items":{"type":"string"},"description":"Checks that were executed"},"findings_truncated":{"type":"boolean","description":"True if findings were truncated due to limits"}}})
}

fn text_security_inspect_output() -> Value {
    serde_json::json!({"type":"object","properties":{"verdict":{"type":"string","enum":["allow","review","block"]},"policy":{"type":"string"},"findings":{"type":"array"},"machine_code":{"type":"string"},"normalized_changed":{"type":"boolean"},"recommended_action":{"type":"string"},"summary":{"type":"string"},"subresults":{"type":"object"}}})
}

fn edit_preflight_output() -> Value {
    serde_json::json!({"type":"object","properties":{"ok_to_apply":{"type":"boolean"},"mode":{"type":"string"},"findings":{"type":"array"},"machine_code":{"type":"string"},"recommended_next_tool":{"type":["string","null"]},"summary":{"type":"string"},"subresults":{"type":"object"}}})
}

fn command_preflight_output() -> Value {
    serde_json::json!({"type":"object","properties":{"verdict":{"type":"string","enum":["allow","review","block"]},"command":{"type":"string"},"platform":{"type":"string"},"policy":{"type":"string"},"findings":{"type":"array"},"machine_code":{"type":"string"},"summary":{"type":"string"},"subresults":{"type":"object"}}})
}

fn config_preflight_output() -> Value {
    serde_json::json!({"type":"object","properties":{"valid":{"type":"boolean"},"verdict":{"type":"string","enum":["valid","valid_with_warnings","invalid"]},"format":{"type":"string"},"findings":{"type":"array"},"machine_code":{"type":"string"},"summary":{"type":"string"},"subresults":{"type":"object"}}})
}

fn structured_data_compare_output() -> Value {
    serde_json::json!({"type":"object","properties":{"equal":{"type":"boolean"},"valid_a":{"type":"boolean"},"valid_b":{"type":"boolean"},"findings":{"type":"array"},"machine_code":{"type":"string"},"summary":{"type":"string"},"subresults":{"type":"object"}}})
}

// ---------------------------------------------------------------------------
// Static registry
// ---------------------------------------------------------------------------

pub const ALL_TOOLS: &[ToolSpec] = &[
    ToolSpec {
        name: "math_eval",
        description: "Evaluate arithmetic, unit conversions, constants, and scientific expressions deterministically. State-mutating functions (setvar, store, etc.) and non-deterministic functions (random, randint, gauss, etc.) are disabled. Use for math and unit tasks instead of asking the model to calculate.",
        handler: math_eval,
        input_schema: math_eval_input,
        output_schema: math_eval_output,
        category: "math",
        tier: 0,
        profiles: &["full", "default", "human_math"],
        tags: &["math", "evaluation", "arithmetic", "units", "constants"],
        exposure: ToolExposure::Default,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Moderate,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "unit_convert",
        description: "Convert a numeric value from one unit to another using pre-defined conversion factors.",
        handler: unit_convert,
        input_schema: unit_convert_input,
        output_schema: unit_convert_output,
        category: "math",
        tier: 2,
        profiles: &["full", "human_math"],
        tags: &["math", "units", "conversion"],
        exposure: ToolExposure::Contextual,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "unit_info",
        description: "Get information about a unit including its canonical form and category.",
        handler: unit_info,
        input_schema: unit_info_input,
        output_schema: unit_info_output,
        category: "math",
        tier: 2,
        profiles: &["full", "human_math"],
        tags: &["math", "units", "information"],
        exposure: ToolExposure::Contextual,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "constant_lookup",
        description: "Look up physical constant values and symbols (Avogadro, Planck, speed of light, etc.).",
        handler: constant_lookup,
        input_schema: constant_lookup_input,
        output_schema: constant_lookup_output,
        category: "math",
        tier: 2,
        profiles: &["full", "human_math"],
        tags: &["math", "constants", "physics", "lookup"],
        exposure: ToolExposure::Contextual,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "text_measure",
        description: "Measure exact text properties: UTF-8 byte length, codepoint count, words, lines, whitespace, newline style, Unicode normalization state, invisibles, and mixed-script signals.",
        handler: text_measure,
        input_schema: text_measure_input,
        output_schema: text_measure_output,
        category: "text",
        tier: 0,
        profiles: &["full", "default"],
        tags: &["text", "measurement", "unicode", "metrics"],
        exposure: ToolExposure::Default,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "text_equal",
        description: "Compare two strings under raw, Unicode-normalized, casefolded, or trimmed modes and report exact equality evidence.",
        handler: text_equal,
        input_schema: text_equal_input,
        output_schema: text_equal_output,
        category: "text",
        tier: 0,
        profiles: &["full", "default", "codegg_core"],
        tags: &["text", "comparison", "equality", "unicode"],
        exposure: ToolExposure::Default,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "text_diff_explain",
        description: "Explain why two strings differ, including spans, codepoints, Unicode names, normalization equivalence, confusables, invisibles, and agent-facing classification.",
        handler: text_diff_explain,
        input_schema: text_diff_explain_input,
        output_schema: text_diff_explain_output,
        category: "text",
        tier: 1,
        profiles: &["full", "default", "codegg_core", "codegg_patch"],
        tags: &["text", "diff", "comparison", "unicode"],
        exposure: ToolExposure::Default,
        harness_use: &["edit_preflight"],
        aliases: &[],
        cost: ToolCost::Moderate,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "text_inspect",
        description: "Inspect a string for hidden characters, Unicode confusables, mixed scripts, normalization state, and display-safe representation. Can report both original and normalized text analysis.",
        handler: text_inspect,
        input_schema: text_inspect_input,
        output_schema: text_inspect_output,
        category: "text",
        tier: 1,
        profiles: &["full", "default", "codegg_core", "codegg_unicode_security"],
        tags: &["text", "unicode", "inspection", "security"],
        exposure: ToolExposure::Default,
        harness_use: &["prompt_input_preflight"],
        aliases: &[],
        cost: ToolCost::Moderate,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "text_count",
        description: "Count exact characters or produce a character frequency table with codepoint positions, grapheme clusters, bytes, or substring matches.",
        handler: text_count,
        input_schema: text_count_input,
        output_schema: text_count_output,
        category: "text",
        tier: 0,
        profiles: &["full", "default"],
        tags: &["text", "count", "character", "frequency"],
        exposure: ToolExposure::Default,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "text_truncate",
        description: "Truncate a string to a specified number of grapheme clusters (user-perceived characters). Preserves emoji, combining sequences, and flag sequences intact. Useful for AI agent prompts where visual length matters.",
        handler: text_truncate,
        input_schema: text_truncate_input,
        output_schema: text_truncate_output,
        category: "text",
        tier: 3,
        profiles: &["full"],
        tags: &["text", "truncation", "grapheme", "unicode"],
        exposure: ToolExposure::ExpertOnly,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "text_transform",
        description: "Apply deterministic text transformations: Unicode normalization (NFC/NFD/NFKC/NFKD), casefold, trim, newline normalization, zero-width removal, bidi control stripping, and visible representation.",
        handler: text_transform,
        input_schema: text_transform_input,
        output_schema: text_transform_output,
        category: "text",
        tier: 2,
        profiles: &["full", "codegg_unicode_security"],
        tags: &["text", "unicode", "transform", "normalization", "sanitation"],
        exposure: ToolExposure::Contextual,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Moderate,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "validate_brackets",
        description: "Check whether delimiters are structurally balanced and report unmatched delimiters with line/column positions.",
        handler: validate_brackets,
        input_schema: validate_brackets_input,
        output_schema: validate_brackets_output,
        category: "validation",
        tier: 1,
        profiles: &["full", "default"],
        tags: &["validation", "brackets", "delimiters"],
        exposure: ToolExposure::Default,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "validate_json",
        description: "Validate JSON and report precise parse errors or top-level structure information.",
        handler: validate_json,
        input_schema: validate_json_input,
        output_schema: validate_json_output,
        category: "validation",
        tier: 0,
        profiles: &["full", "default", "codegg_core", "codegg_core_min", "codegg_config"],
        tags: &["validation", "json", "structured-data"],
        exposure: ToolExposure::Default,
        harness_use: &["config_preflight"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "validate_regex",
        description: "Test a Python regular expression against sample strings and report match/fullmatch status, spans, groups, and errors.",
        handler: validate_regex,
        input_schema: validate_regex_input,
        output_schema: validate_regex_output,
        category: "regex",
        tier: 1,
        profiles: &["full", "default"],
        tags: &["text", "regex", "validation", "pattern"],
        exposure: ToolExposure::Default,
        harness_use: &["command_preflight"],
        aliases: &[],
        cost: ToolCost::Moderate,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "list_compare",
        description: "Compare two lists with explicit modes: ordered ( LCS-based alignment), set (presence only), multiset (count deltas). Near matches are optional and never replace exact missing/extra results.",
        handler: list_compare,
        input_schema: list_compare_input,
        output_schema: list_compare_output,
        category: "list",
        tier: 2,
        profiles: &["full"],
        tags: &["text", "list", "comparison", "set"],
        exposure: ToolExposure::Contextual,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Moderate,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "validate_toml",
        description: "Validate TOML configuration files (Cargo.toml, pyproject.toml, etc.) and report parse errors with line/column positions.",
        handler: validate_toml_tool,
        input_schema: validate_toml_input,
        output_schema: validate_toml_output,
        category: "validation",
        tier: 1,
        profiles: &["full", "default", "codegg_core", "codegg_config"],
        tags: &["validation", "structured-data", "toml", "config", "rust", "python"],
        exposure: ToolExposure::Default,
        harness_use: &["config_preflight"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "json_extract",
        description: "Extract a value from JSON using RFC 6901 JSON Pointer (e.g., /foo/bar/0). Navigate nested objects and arrays.",
        handler: json_extract,
        input_schema: json_extract_input,
        output_schema: json_extract_output,
        category: "json",
        tier: 2,
        profiles: &["full", "codegg_config"],
        tags: &["json", "structured-data", "extraction", "config", "pointer"],
        exposure: ToolExposure::Contextual,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Moderate,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "json_compare",
        description: "Compare two JSON documents semantically, ignoring formatting and key order.",
        handler: json_compare,
        input_schema: json_compare_input,
        output_schema: json_compare_output,
        category: "json",
        tier: 1,
        profiles: &["full", "default", "codegg_config"],
        tags: &["json", "structured-data", "comparison", "config"],
        exposure: ToolExposure::Default,
        harness_use: &["config_preflight"],
        aliases: &[],
        cost: ToolCost::Moderate,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "text_position",
        description: "Convert between byte offsets, codepoint indices, line/column positions, and UTF-16 offsets.",
        handler: text_position,
        input_schema: text_position_input,
        output_schema: text_position_output,
        category: "text",
        tier: 2,
        profiles: &["full", "codegg_unicode_security"],
        tags: &["text", "position", "offset", "unicode", "lsp"],
        exposure: ToolExposure::Contextual,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "text_hash",
        description: "Compute cryptographic hashes of text for identity checking.",
        handler: text_hash,
        input_schema: text_hash_input,
        output_schema: text_hash_output,
        category: "text",
        tier: 2,
        profiles: &["full"],
        tags: &["text", "hash", "identity", "security"],
        exposure: ToolExposure::Contextual,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Moderate,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "escape_text",
        description: "Escape text for various output formats.",
        handler: escape_text,
        input_schema: escape_text_input,
        output_schema: escape_text_output,
        category: "text",
        tier: 1,
        profiles: &["full", "default"],
        tags: &["text", "escape", "encoding", "shell", "json", "regex"],
        exposure: ToolExposure::Default,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "unescape_text",
        description: "Unescape text from various formats.",
        handler: unescape_text,
        input_schema: unescape_text_input,
        output_schema: unescape_text_output,
        category: "text",
        tier: 1,
        profiles: &["full", "default"],
        tags: &["text", "escape", "encoding", "shell", "json", "regex"],
        exposure: ToolExposure::Default,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "identifier_analyze",
        description: "Classify and validate identifier naming conventions across languages.",
        handler: identifier_analyze,
        input_schema: identifier_analyze_input,
        output_schema: identifier_analyze_output,
        category: "identifier",
        tier: 3,
        profiles: &["full"],
        tags: &["text", "identifier", "naming", "validation", "language"],
        exposure: ToolExposure::ExpertOnly,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Moderate,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "regex_finditer",
        description: "Find all regex matches in text with positions, line/column info, and capture groups.",
        handler: regex_finditer_tool,
        input_schema: regex_finditer_input,
        output_schema: regex_finditer_output,
        category: "regex",
        tier: 1,
        profiles: &["full", "default"],
        tags: &["text", "regex", "search", "find", "pattern"],
        exposure: ToolExposure::Default,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Moderate,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "regex_safety_check",
        description: "Heuristic check for potential catastrophic backtracking risks in regex patterns. Flags nested quantifiers, repeated alternations, ambiguous dot-star, and backreferences.",
        handler: regex_safety_check_tool,
        input_schema: regex_safety_check_input,
        output_schema: regex_safety_check_output,
        category: "regex",
        tier: 1,
        profiles: &["full", "default", "codegg_shell"],
        tags: &["text", "regex", "safety", "security", "backtracking"],
        exposure: ToolExposure::Default,
        harness_use: &["command_preflight"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "validate_schema_light",
        description: "Validate JSON against a simple schema format with type, required, enum, pattern, and nested constraints.",
        handler: validate_schema_light_tool,
        input_schema: validate_schema_light_input,
        output_schema: validate_schema_light_output,
        category: "validation",
        tier: 3,
        profiles: &["full", "codegg_config"],
        tags: &["validation", "json", "schema", "structured-data"],
        exposure: ToolExposure::Contextual,
        harness_use: &["config_preflight"],
        aliases: &[],
        cost: ToolCost::Moderate,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "path_normalize",
        description: "Normalize a path using posixpath or ntpath semantics. Collapse dot segments, resolve components.",
        handler: path_normalize_tool,
        input_schema: path_normalize_input,
        output_schema: path_normalize_output,
        category: "path",
        tier: 0,
        profiles: &["full", "default", "codegg_core"],
        tags: &["text", "path", "filesystem", "normalize"],
        exposure: ToolExposure::Default,
        harness_use: &["path_preflight"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "path_analyze",
        description: "Analyze path components, extensions, hidden status, and traversal without filesystem access.",
        handler: path_analyze,
        input_schema: path_analyze_input,
        output_schema: path_analyze_output,
        category: "path",
        tier: 2,
        profiles: &["full"],
        tags: &["text", "path", "filesystem", "lexical"],
        exposure: ToolExposure::Contextual,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "path_compare",
        description: "Compare two paths under explicit normalization rules: separator normalization, dot-segment collapsing, and optional case-insensitive comparison.",
        handler: path_compare,
        input_schema: path_compare_input,
        output_schema: path_compare_output,
        category: "path",
        tier: 2,
        profiles: &["full"],
        tags: &["text", "path", "filesystem", "comparison"],
        exposure: ToolExposure::Contextual,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "path_scope_check",
        description: "Determine whether a target path remains lexically inside a declared root. Lexical only, does not resolve symlinks.",
        handler: path_scope_check,
        input_schema: path_scope_check_input,
        output_schema: path_scope_check_output,
        category: "path",
        tier: 2,
        profiles: &["full", "codegg_preflight"],
        tags: &["text", "path", "filesystem", "security", "scope"],
        exposure: ToolExposure::HarnessOnly,
        harness_use: &["path_preflight"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "json_shape",
        description: "Analyze the structure of a JSON document without returning values. Shows type, keys, and nested structure with configurable depth limits.",
        handler: json_shape_tool,
        input_schema: json_shape_input,
        output_schema: json_shape_output,
        category: "json",
        tier: 3,
        profiles: &["full", "codegg_repo_audit"],
        tags: &["json", "structured-data", "shape", "schema"],
        exposure: ToolExposure::ExpertOnly,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Moderate,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "text_window",
        description: "Get a window around a position in text with context lines. Shows line at position with surrounding context, position metrics, and character details.",
        handler: text_window,
        input_schema: text_window_input,
        output_schema: text_window_output,
        category: "text",
        tier: 1,
        profiles: &["full", "default"],
        tags: &["text", "position", "context", "unicode", "window"],
        exposure: ToolExposure::Default,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "json_canonicalize",
        description: "Canonicalize JSON with deterministic formatting, key ordering, duplicate key detection, and stable hashes.",
        handler: json_canonicalize,
        input_schema: json_canonicalize_input,
        output_schema: json_canonicalize_output,
        category: "json",
        tier: 1,
        profiles: &["full", "default", "codegg_config"],
        tags: &["json", "canonical", "hash", "deterministic", "format"],
        exposure: ToolExposure::Default,
        harness_use: &["config_preflight"],
        aliases: &[],
        cost: ToolCost::Moderate,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "json_query",
        description: "Extract a value from JSON using RFC 6901 JSON Pointer. Navigate nested objects and arrays. Deprecated: use json_extract instead, which provides richer output including available_keys, missing_at, and detail levels.",
        handler: json_query,
        input_schema: json_query_input,
        output_schema: json_query_output,
        category: "json",
        tier: 1,
        profiles: &["full"],
        tags: &["json", "pointer", "extraction", "query", "rfc6901"],
        exposure: ToolExposure::Contextual,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Moderate,
        stability: ToolStability::Deprecated,
        composite: false,
    },
    ToolSpec {
        name: "glob_match",
        description: "Match a glob pattern against a path with explicit semantics: * matches within one segment, ** matches zero or more segments, ? matches one char. Python fnmatch limitations around ** are documented.",
        handler: glob_match_tool,
        input_schema: glob_match_input,
        output_schema: glob_match_output,
        category: "path",
        tier: 1,
        profiles: &["full", "default"],
        tags: &["text", "glob", "pattern", "path", "wildcard"],
        exposure: ToolExposure::Default,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "text_fingerprint",
        description: "Compute a deterministic SHA-256 fingerprint of text with canonicalization options for Unicode normalization, newline style, casefold, and final newline trimming.",
        handler: text_fingerprint_tool,
        input_schema: text_fingerprint_input,
        output_schema: text_fingerprint_output,
        category: "text",
        tier: 0,
        profiles: &["full", "default", "codegg_core", "codegg_repo_audit"],
        tags: &["text", "hash", "fingerprint", "sha256", "identity", "canonicalization"],
        exposure: ToolExposure::Default,
        harness_use: &["edit_preflight"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "identifier_inspect",
        description: "Inspect identifiers for validity and collisions. Detects confusables, mixed scripts, normalization issues, and casefold collisions across a list of identifiers.",
        handler: identifier_inspect,
        input_schema: identifier_inspect_input,
        output_schema: identifier_inspect_output,
        category: "identifier",
        tier: 1,
        profiles: &["full", "default", "codegg_core", "codegg_unicode_security"],
        tags: &["text", "identifier", "collision", "confusable", "security", "validation"],
        exposure: ToolExposure::Default,
        harness_use: &["reasoning_only"],
        aliases: &[],
        cost: ToolCost::Moderate,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "version_compare",
        description: "Compare two version strings with explicit scheme. Supports semver (major.minor.patch), loose (numeric parts), and deferred pep440.",
        handler: version_compare_tool,
        input_schema: version_compare_input,
        output_schema: version_compare_output,
        category: "version",
        tier: 2,
        profiles: &["full", "codegg_config"],
        tags: &["version", "semver", "comparison"],
        exposure: ToolExposure::Contextual,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "toml_shape",
        description: "Analyze the structure of a TOML document: top-level keys, tables, and nesting hierarchy.",
        handler: toml_shape_tool,
        input_schema: toml_shape_input,
        output_schema: toml_shape_output,
        category: "toml",
        tier: 2,
        profiles: &["full", "codegg_config"],
        tags: &["toml", "structure", "shape", "config", "validation"],
        exposure: ToolExposure::Contextual,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Moderate,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "list_dedupe",
        description: "Remove duplicates from a list while preserving order. Supports Unicode normalization and casefolding.",
        handler: list_dedupe,
        input_schema: list_dedupe_input,
        output_schema: list_dedupe_output,
        category: "list",
        tier: 1,
        profiles: &["full", "default"],
        tags: &["list", "dedupe", "unique", "normalization"],
        exposure: ToolExposure::Default,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "list_sort",
        description: "Sort a list of strings with Unicode normalization and casefold support.",
        handler: list_sort,
        input_schema: list_sort_input,
        output_schema: list_sort_output,
        category: "list",
        tier: 1,
        profiles: &["full", "default"],
        tags: &["list", "sort", "order", "normalization"],
        exposure: ToolExposure::Default,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "text_replace_check",
        description: "Check whether a text replacement would apply cleanly before an agent attempts to edit. Reports match count, positions, ambiguity, and optional preview of before/after.",
        handler: text_replace_check_tool,
        input_schema: text_replace_check_input,
        output_schema: text_replace_check_output,
        category: "text",
        tier: 1,
        profiles: &["full", "default", "codegg_core", "codegg_core_min", "codegg_patch"],
        tags: &["text", "replace", "edit", "safety", "check"],
        exposure: ToolExposure::Default,
        harness_use: &["edit_preflight"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "line_range_extract",
        description: "Extract exact line ranges from text and return stable offsets, byte positions, line counts, and optional fingerprint.",
        handler: line_range_extract_tool,
        input_schema: line_range_extract_input,
        output_schema: line_range_extract_output,
        category: "text",
        tier: 1,
        profiles: &["full", "default", "codegg_patch"],
        tags: &["text", "line", "range", "extract", "offset"],
        exposure: ToolExposure::Default,
        harness_use: &["edit_preflight"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "line_range_compare",
        description: "Compare a line range from two text inputs with exact, trailing-whitespace-ignoring, or newline-normalizing comparison.",
        handler: line_range_compare_tool,
        input_schema: line_range_compare_input,
        output_schema: line_range_compare_output,
        category: "text",
        tier: 2,
        profiles: &["full", "codegg_patch"],
        tags: &["text", "line", "range", "compare", "diff"],
        exposure: ToolExposure::Contextual,
        harness_use: &["edit_preflight"],
        aliases: &[],
        cost: ToolCost::Moderate,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "shell_split",
        description: "Parse a shell-like command string into argv tokens and report risky lexical features (pipes, redirections, command substitution, variable expansion, globs, control operators). Lexical POSIX-like parsing only, not full shell evaluation.",
        handler: shell_split,
        input_schema: shell_split_input,
        output_schema: shell_split_output,
        category: "shell",
        tier: 2,
        profiles: &["full", "codegg_preflight", "codegg_shell"],
        tags: &["shell", "argv", "parsing", "security", "sanity"],
        exposure: ToolExposure::HarnessOnly,
        harness_use: &["command_preflight"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "shell_quote_join",
        description: "Safely quote a list of argv tokens into a POSIX-like shell string. Verifies round-trip safety with shell_split.",
        handler: shell_quote_join,
        input_schema: shell_quote_join_input,
        output_schema: shell_quote_join_output,
        category: "shell",
        tier: 2,
        profiles: &["full", "codegg_shell"],
        tags: &["shell", "argv", "quoting", "safety"],
        exposure: ToolExposure::Contextual,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "argv_compare",
        description: "Compare two command strings or argv lists by parsed argv tokens rather than raw text. Supports command strings, pre-parsed argv lists, or both.",
        handler: argv_compare,
        input_schema: argv_compare_input,
        output_schema: argv_compare_output,
        category: "shell",
        tier: 2,
        profiles: &["full", "codegg_shell"],
        tags: &["shell", "argv", "comparison", "sanity"],
        exposure: ToolExposure::Contextual,
        harness_use: &["command_preflight"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "markdown_structure",
        description: "Parse Markdown structure with a deterministic line scanner: headings (level, text, slug), code fences (language, open/close state), links (visible vs target mismatch), HTML comments, frontmatter detection, and table detection. Not a full CommonMark parser.",
        handler: markdown_structure,
        input_schema: markdown_structure_input,
        output_schema: markdown_structure_output,
        category: "markdown",
        tier: 2,
        profiles: &["full", "codegg_repo_audit"],
        tags: &["markdown", "structure", "headings", "code-fences", "links", "frontmatter"],
        exposure: ToolExposure::Contextual,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Moderate,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "code_fence_extract",
        description: "Extract fenced code blocks from Markdown with exact line ranges, optional language filter, content, and SHA-256 fingerprints. Reports unclosed fences.",
        handler: code_fence_extract,
        input_schema: code_fence_extract_input,
        output_schema: code_fence_extract_output,
        category: "markdown",
        tier: 2,
        profiles: &["full", "codegg_repo_audit"],
        tags: &["markdown", "code-fences", "extraction", "fingerprint"],
        exposure: ToolExposure::Contextual,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Moderate,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "dotenv_validate",
        description: "Validate .env-style key=value configuration text. Detects invalid keys, duplicate keys, missing quotes, and variable expansion syntax. Line-by-line parser, no shell evaluation.",
        handler: dotenv_validate,
        input_schema: dotenv_validate_input,
        output_schema: dotenv_validate_output,
        category: "config",
        tier: 2,
        profiles: &["full", "codegg_config"],
        tags: &["validation", "config", "env", "dotenv"],
        exposure: ToolExposure::Contextual,
        harness_use: &["config_preflight"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "ini_validate",
        description: "Validate simple INI-style configuration files. Supports [section] headers, key=value and key:value lines, comments. Detects duplicate sections, duplicate keys, and malformed lines.",
        handler: ini_validate,
        input_schema: ini_validate_input,
        output_schema: ini_validate_output,
        category: "config",
        tier: 2,
        profiles: &["full", "codegg_config"],
        tags: &["validation", "config", "ini"],
        exposure: ToolExposure::Contextual,
        harness_use: &["config_preflight"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "patch_apply_check",
        description: "Validate and simulate a unified diff against provided in-memory files/text without touching the filesystem. Reports parse status, application success, failed hunks with context, and optional result fingerprint.",
        handler: patch_apply_check,
        input_schema: patch_apply_check_input,
        output_schema: patch_apply_check_output,
        category: "patch",
        tier: 2,
        profiles: &["full", "codegg_preflight", "codegg_patch"],
        tags: &["patch", "diff", "unified", "validation", "apply"],
        exposure: ToolExposure::HarnessOnly,
        harness_use: &["edit_preflight"],
        aliases: &[],
        cost: ToolCost::Moderate,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "patch_summary",
        description: "Summarize a unified diff without applying it. Reports file counts, hunk counts, additions, deletions, renames, and line ranges by file.",
        handler: patch_summary,
        input_schema: patch_summary_input,
        output_schema: patch_summary_output,
        category: "patch",
        tier: 2,
        profiles: &["full", "codegg_patch"],
        tags: &["patch", "diff", "unified", "summary", "statistics"],
        exposure: ToolExposure::Contextual,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Moderate,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "unicode_policy_check",
        description: "Apply a named deterministic Unicode safety policy to input text. Policies include identifier_strict (mixed scripts, bidi, confusables), filename_safe (control chars, path separators, reserved names), source_code, human_text (warn-only), json_key, and domain_like.",
        handler: unicode_policy_check,
        input_schema: unicode_policy_check_input,
        output_schema: unicode_policy_check_output,
        category: "unicode",
        tier: 2,
        profiles: &["full", "codegg_preflight", "codegg_unicode_security"],
        tags: &["text", "unicode", "policy", "security", "validation"],
        exposure: ToolExposure::HarnessOnly,
        harness_use: &["prompt_input_preflight"],
        aliases: &[],
        cost: ToolCost::Moderate,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "canonicalize_text",
        description: "Apply a named text canonicalization profile. Profiles include source_file_identity (NFC + LF + newline), identifier_compare (NFC + casefold), human_label_compare (NFC + casefold + whitespace collapse), json_key_compare (NFC + casefold), and path_segment_compare (NFC + lowercase + LF).",
        handler: canonicalize_text,
        input_schema: canonicalize_text_input,
        output_schema: canonicalize_text_output,
        category: "unicode",
        tier: 2,
        profiles: &["full", "codegg_unicode_security"],
        tags: &["text", "unicode", "canonicalization", "normalization", "identity"],
        exposure: ToolExposure::Contextual,
        harness_use: &["prompt_input_preflight"],
        aliases: &[],
        cost: ToolCost::Moderate,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "identifier_table_inspect",
        description: "Inspect a table of identifiers for casefold collisions, normalization collisions, confusable/near-collisions, style variants, reserved keyword hits, and mixed naming style groups. Accepts structured entries with name, kind, file, and line metadata.",
        handler: identifier_table_inspect,
        input_schema: identifier_table_inspect_input,
        output_schema: identifier_table_inspect_output,
        category: "identifier",
        tier: 3,
        profiles: &["full", "codegg_repo_audit"],
        tags: &["text", "identifier", "collision", "naming", "style", "reserved", "validation"],
        exposure: ToolExposure::ExpertOnly,
        harness_use: &["repo_audit"],
        aliases: &[],
        cost: ToolCost::Moderate,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "version_constraint_check",
        description: "Check whether a version satisfies a constraint under a declared versioning scheme (semver or cargo). Supports comparison operators, caret, tilde, wildcard, range, and comma-separated constraints.",
        handler: version_constraint_check,
        input_schema: version_constraint_check_input,
        output_schema: version_constraint_check_output,
        category: "version",
        tier: 3,
        profiles: &["full"],
        tags: &["version", "semver", "cargo", "constraint", "satisfiability"],
        exposure: ToolExposure::ExpertOnly,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "cargo_toml_inspect",
        description: "Inspect Cargo.toml text without network or filesystem access. Reports package metadata, workspace configuration, dependency forms (version/path/git/workspace), path dependencies, suspicious or confusable dependency names, and structural findings.",
        handler: cargo_toml_inspect,
        input_schema: cargo_toml_inspect_input,
        output_schema: cargo_toml_inspect_output,
        category: "cargo",
        tier: 3,
        profiles: &["full", "codegg_core", "codegg_repo_audit"],
        tags: &["rust", "cargo", "toml", "dependencies", "workspace", "inspection"],
        exposure: ToolExposure::ExpertOnly,
        harness_use: &["config_preflight"],
        aliases: &[],
        cost: ToolCost::Moderate,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "prompt_input_inspect",
        description: "Deterministically inspect text for red flags that may influence agents or humans unexpectedly. Detects hidden Unicode characters, bidirectional controls, HTML comments, Markdown link mismatches, ANSI escapes, terminal controls, base64-like blobs, instruction-like phrases, and very long minified lines. This is NOT a prompt-injection detector -- it reports observable features only, not intent.",
        handler: prompt_input_inspect_tool,
        input_schema: prompt_input_inspect_input,
        output_schema: prompt_input_inspect_output,
        category: "text",
        tier: 2,
        profiles: &["full", "codegg_unicode_security", "codegg_preflight"],
        tags: &["text", "security", "inspection", "prompt", "unicode", "hidden"],
        exposure: ToolExposure::HarnessOnly,
        harness_use: &["prompt_input_preflight"],
        aliases: &[],
        cost: ToolCost::Moderate,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "text_security_inspect",
        description: "Composite security-oriented text hygiene pass. Runs text_inspect, unicode_policy_check, canonicalize_text, prompt_input_inspect, and identifier_inspect depending on policy. Returns a verdict (allow/review/block) plus structured findings and machine codes.",
        handler: text_security_inspect,
        input_schema: text_security_inspect_input,
        output_schema: text_security_inspect_output,
        category: "text",
        tier: 1,
        profiles: &["full", "codegg_core", "codegg_core_min", "codegg_preflight", "codegg_unicode_security"],
        tags: &["text", "unicode", "security", "composite", "prompt", "inspection"],
        exposure: ToolExposure::Default,
        harness_use: &["prompt_input_preflight"],
        aliases: &[],
        cost: ToolCost::Heavy,
        stability: ToolStability::Stable,
        composite: true,
    },
    ToolSpec {
        name: "edit_preflight",
        description: "Composite: validate a proposed edit before applying it. Calls text_replace_check, patch_apply_check, line_range_extract, text_fingerprint, and text_diff_explain as needed. Returns ok_to_apply verdict with findings and machine codes.",
        handler: edit_preflight,
        input_schema: edit_preflight_input,
        output_schema: edit_preflight_output,
        category: "patch",
        tier: 1,
        profiles: &["full", "codegg_core", "codegg_core_min", "codegg_preflight", "codegg_patch"],
        tags: &["patch", "edit", "preflight", "composite", "text"],
        exposure: ToolExposure::Default,
        harness_use: &["edit_preflight"],
        aliases: &[],
        cost: ToolCost::Heavy,
        stability: ToolStability::Stable,
        composite: true,
    },
    ToolSpec {
        name: "command_preflight",
        description: "Composite: analyze a command before user approval or execution. Calls shell_split and regex_safety_check. Returns parsed argv, shell operators, risk findings, and a verdict. Must not execute anything.",
        handler: command_preflight,
        input_schema: command_preflight_input,
        output_schema: command_preflight_output,
        category: "shell",
        tier: 1,
        profiles: &["full", "codegg_core", "codegg_core_min", "codegg_preflight", "codegg_shell"],
        tags: &["shell", "command", "preflight", "composite", "security"],
        exposure: ToolExposure::Default,
        harness_use: &["command_preflight"],
        aliases: &[],
        cost: ToolCost::Heavy,
        stability: ToolStability::Stable,
        composite: true,
    },
    ToolSpec {
        name: "config_preflight",
        description: "Composite: validate generated config text. Auto-detects format and runs the appropriate validator. Returns valid/invalid, detected format, parse error location, and machine code.",
        handler: config_preflight,
        input_schema: config_preflight_input,
        output_schema: config_preflight_output,
        category: "config",
        tier: 1,
        profiles: &["full", "codegg_core", "codegg_core_min", "codegg_preflight", "codegg_config"],
        tags: &["config", "validation", "json", "toml", "preflight", "composite"],
        exposure: ToolExposure::Default,
        harness_use: &["config_preflight"],
        aliases: &[],
        cost: ToolCost::Heavy,
        stability: ToolStability::Stable,
        composite: true,
    },
    ToolSpec {
        name: "structured_data_compare",
        description: "Composite: compare structured config/data output. Calls json_compare, json_canonicalize, and json_shape. Returns equal/not-equal verdict with structured diffs.",
        handler: structured_data_compare,
        input_schema: structured_data_compare_input,
        output_schema: structured_data_compare_output,
        category: "json",
        tier: 2,
        profiles: &["full", "codegg_core", "codegg_config"],
        tags: &["json", "comparison", "config", "structured-data", "composite"],
        exposure: ToolExposure::Contextual,
        harness_use: &["config_preflight"],
        aliases: &[],
        cost: ToolCost::Heavy,
        stability: ToolStability::Stable,
        composite: true,
    },
];

// ---------------------------------------------------------------------------
// Profile constants
// ---------------------------------------------------------------------------

pub const PROFILE_NAMES: &[&str] = &[
    "full",
    "default",
    "codegg_core_min",
    "codegg_core",
    "codegg_preflight",
    "codegg_patch",
    "codegg_config",
    "codegg_unicode_security",
    "codegg_shell",
    "codegg_repo_audit",
    "human_math",
];

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

pub fn all_tools() -> &'static [ToolSpec] {
    ALL_TOOLS
}

pub fn get_tool(name: &str) -> Option<&'static ToolSpec> {
    ALL_TOOLS.iter().find(|t| t.name == name)
}

pub fn tool_names() -> Vec<&'static str> {
    ALL_TOOLS.iter().map(|t| t.name).collect()
}

pub fn tools_for_profile(profile: &str) -> Vec<&'static ToolSpec> {
    if profile == "full" {
        return ALL_TOOLS
            .iter()
            .filter(|t| t.exposure != ToolExposure::Hidden)
            .collect();
    }
    ALL_TOOLS
        .iter()
        .filter(|t| t.profiles.contains(&profile))
        .collect()
}

pub fn available_profiles() -> &'static [&'static str] {
    PROFILE_NAMES
}

pub fn mcp_tool_definitions() -> Vec<ToolDefinition> {
    ALL_TOOLS
        .iter()
        .filter(|t| t.exposure != ToolExposure::Hidden)
        .map(|spec| {
            let deprecated = if spec.stability == ToolStability::Deprecated {
                Some(true)
            } else {
                None
            };
            ToolDefinition {
                name: spec.name.to_string(),
                description: spec.description.to_string(),
                input_schema: (spec.input_schema)(),
                output_schema: Some((spec.output_schema)()),
                tier: Some(spec.tier),
                tags: Some(spec.tags.iter().map(|s| s.to_string()).collect()),
                deprecated,
                category: Some(spec.category.to_string()),
                llm_exposure: Some(spec.exposure.as_str().to_string()),
                cost: Some(spec.cost.as_str().to_string()),
            }
        })
        .collect()
}

pub fn input_schema_for(name: &str) -> Option<Value> {
    get_tool(name).map(|spec| (spec.input_schema)())
}

pub fn output_schema_for(name: &str) -> Option<Value> {
    get_tool(name).map(|spec| (spec.output_schema)())
}

pub fn tool_handler_for(name: &str) -> Option<ToolHandler> {
    get_tool(name).map(|spec| spec.handler)
}

pub fn tool_count() -> usize {
    ALL_TOOLS.len()
}
