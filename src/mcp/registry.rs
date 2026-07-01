use crate::mcp::response::ToolResponse;
use crate::mcp::schemas::*;
use crate::text::levenshtein_distance;
use crate::tools::*;
use serde::Serialize;
use serde_json::Value;

#[derive(Serialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
    #[serde(rename = "outputSchema", skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_exposure: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<String>,
}

/// Function pointer type for tool handler implementations.
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

// ---------------------------------------------------------------------------
// Audience-aware listing
// ---------------------------------------------------------------------------

/// Audience for tool listing, controlling which exposure levels are included.
///
/// - `Model`: Excludes `HarnessOnly` and `Hidden`. Safe for ordinary model-visible use.
/// - `Harness`: Includes `HarnessOnly` tools for selected profiles but excludes `Hidden`.
/// - `Debug`: Includes all non-hidden tools, including `ExpertOnly` and `HarnessOnly`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ToolListAudience {
    Model,
    Harness,
    Debug,
}

/// Filter tools by profile and audience.
///
/// For the `full` profile, hidden tools are always excluded.
/// For other profiles, only tools in the profile's `profiles` list are included.
/// The audience then further filters by exposure level.
pub fn tools_for_profile_audience(
    profile: &str,
    audience: ToolListAudience,
) -> Vec<&'static ToolSpec> {
    let profile_tools = tools_for_profile(profile);
    match audience {
        ToolListAudience::Model => profile_tools
            .into_iter()
            .filter(|t| t.exposure != ToolExposure::HarnessOnly)
            .collect(),
        ToolListAudience::Harness => profile_tools
            .into_iter()
            .filter(|t| t.exposure != ToolExposure::Hidden)
            .collect(),
        ToolListAudience::Debug => profile_tools,
    }
}

/// Get tool names for a profile and audience combination.
pub fn tool_names_for_profile_audience(
    profile: &str,
    audience: ToolListAudience,
) -> Vec<&'static str> {
    tools_for_profile_audience(profile, audience)
        .into_iter()
        .map(|spec| spec.name)
        .collect()
}

pub fn compact_input_schema(schema: &Value) -> Value {
    let obj = match schema.as_object() {
        Some(o) => o,
        None => return schema.clone(),
    };

    let mut compact = serde_json::Map::new();
    compact.insert(
        "type".to_string(),
        obj.get("type")
            .cloned()
            .unwrap_or_else(|| Value::String("object".to_string())),
    );

    if let Some(props) = obj.get("properties").and_then(|v| v.as_object()) {
        let mut compact_props = serde_json::Map::new();
        for (prop_name, prop_def) in props {
            if let Some(prop_obj) = prop_def.as_object() {
                let mut cp = serde_json::Map::new();
                if let Some(t) = prop_obj.get("type") {
                    cp.insert("type".to_string(), t.clone());
                }
                if let Some(e) = prop_obj.get("enum") {
                    cp.insert("enum".to_string(), e.clone());
                }
                if let Some(r) = prop_obj.get("required") {
                    cp.insert("required".to_string(), r.clone());
                }
                if let Some(items) = prop_obj.get("items") {
                    cp.insert("items".to_string(), items.clone());
                }
                for key in &[
                    "minimum",
                    "maximum",
                    "exclusiveMinimum",
                    "exclusiveMaximum",
                    "minLength",
                    "maxLength",
                    "pattern",
                    "minItems",
                    "maxItems",
                    "multipleOf",
                ] {
                    if let Some(v) = prop_obj.get(*key) {
                        cp.insert(key.to_string(), v.clone());
                    }
                }
                if let Some(desc) = prop_obj.get("description").and_then(|v| v.as_str()) {
                    let truncated = if desc.chars().count() > 80 {
                        format!("{}...", desc.chars().take(77).collect::<String>())
                    } else {
                        desc.to_string()
                    };
                    cp.insert("description".to_string(), Value::String(truncated));
                }
                compact_props.insert(prop_name.clone(), Value::Object(cp));
            } else {
                compact_props.insert(prop_name.clone(), prop_def.clone());
            }
        }
        compact.insert("properties".to_string(), Value::Object(compact_props));
    }

    if let Some(req) = obj.get("required") {
        compact.insert("required".to_string(), req.clone());
    }

    Value::Object(compact)
}

pub fn compact_output_schema(schema: &Value) -> Value {
    let obj = match schema.as_object() {
        Some(o) => o,
        None => return serde_json::json!({"type": "object"}),
    };

    let mut compact_output = serde_json::json!({"type": obj.get("type").unwrap_or(&Value::String("object".to_string()))});
    if let Some(props) = obj.get("properties").and_then(|v| v.as_object()) {
        let mut compact_props = serde_json::Map::new();
        for (key, prop) in props {
            let mut compact_prop = serde_json::json!({});
            if let Some(t) = prop.get("type") {
                compact_prop["type"] = t.clone();
            }
            if let Some(e) = prop.get("enum") {
                compact_prop["enum"] = e.clone();
            }
            compact_props.insert(key.clone(), compact_prop);
        }
        compact_output["properties"] = Value::Object(compact_props);
    }

    compact_output
}

pub fn find_close_match<'a>(input: &str, tool_names: &[&'a str]) -> Option<&'a str> {
    if input.len() > 200 {
        return None;
    }
    let lower_input = input.to_lowercase();

    for &name in tool_names {
        if name.to_lowercase() == lower_input {
            return Some(name);
        }
    }

    fn at_word_boundary(sub: &str, s: &str) -> bool {
        if let Some(idx) = s.find(sub) {
            if idx == 0 {
                return true;
            }
            s.as_bytes().get(idx - 1) == Some(&b'_') || s.as_bytes().get(idx - 1) == Some(&b'-')
        } else {
            false
        }
    }

    let mut best_boundary: Option<(&str, usize)> = None;
    for &name in tool_names {
        let lower_name = name.to_lowercase();
        if at_word_boundary(&lower_input, &lower_name)
            || at_word_boundary(&lower_name, &lower_input)
        {
            let is_shorter = match best_boundary {
                Some((best_name, _)) => name.len() < best_name.len(),
                None => true,
            };
            if is_shorter {
                best_boundary = Some((name, 0));
            }
        }
    }
    if let Some((name, _)) = best_boundary {
        return Some(name);
    }

    let mut best: Option<(&str, usize)> = None;
    for &name in tool_names {
        let dist = levenshtein_distance(input, name);
        let threshold = input.chars().count().min(name.chars().count()) / 2;
        if dist <= threshold && best.is_none_or(|(_, best_dist)| dist < best_dist) {
            best = Some((name, dist));
        }
    }

    best.map(|(name, _)| name)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Enum serialization preserves current strings --

    #[test]
    fn exposure_enum_serializes_to_expected_strings() {
        assert_eq!(ToolExposure::Default.as_str(), "default");
        assert_eq!(ToolExposure::Contextual.as_str(), "contextual");
        assert_eq!(ToolExposure::ExpertOnly.as_str(), "expert_only");
        assert_eq!(ToolExposure::HarnessOnly.as_str(), "harness_only");
        assert_eq!(ToolExposure::Hidden.as_str(), "hidden");
    }

    #[test]
    fn cost_enum_serializes_to_expected_strings() {
        assert_eq!(ToolCost::Cheap.as_str(), "cheap");
        assert_eq!(ToolCost::Moderate.as_str(), "moderate");
        assert_eq!(ToolCost::Heavy.as_str(), "heavy");
    }

    #[test]
    fn stability_enum_serializes_to_expected_strings() {
        assert_eq!(ToolStability::Stable.as_str(), "stable");
        assert_eq!(ToolStability::Deprecated.as_str(), "deprecated");
        assert_eq!(ToolStability::Experimental.as_str(), "experimental");
    }

    // -- Every tool has a valid exposure enum (compile-time guarantee via type system) --

    #[test]
    fn all_tools_have_valid_exposure() {
        for spec in ALL_TOOLS {
            // exposure is typed as ToolExposure, so this always has a valid variant.
            // This test documents the invariant and ensures as_str() works.
            let _ = spec.exposure.as_str();
        }
    }

    // -- Hidden tools excluded from ordinary listing --

    #[test]
    fn hidden_tools_excluded_from_mcp_definitions() {
        let definitions = mcp_tool_definitions();
        for tool in &definitions {
            assert_ne!(tool.llm_exposure.as_deref(), Some("hidden"));
        }
    }

    #[test]
    fn full_profile_excludes_hidden_tools() {
        let tools = tools_for_profile("full");
        for spec in tools {
            assert_ne!(spec.exposure, ToolExposure::Hidden);
        }
    }

    // -- Harness-only tools excluded from model audience lists --

    #[test]
    fn harness_only_excluded_from_model_audience_default() {
        let model_tools = tool_names_for_profile_audience("default", ToolListAudience::Model);
        for spec in ALL_TOOLS {
            if spec.exposure == ToolExposure::HarnessOnly && spec.profiles.contains(&"default") {
                assert!(
                    !model_tools.contains(&spec.name),
                    "harness_only tool '{}' should not appear in default model audience",
                    spec.name
                );
            }
        }
    }

    #[test]
    fn harness_only_excluded_from_model_audience_codegg_core_min() {
        let model_tools =
            tool_names_for_profile_audience("codegg_core_min", ToolListAudience::Model);
        for spec in ALL_TOOLS {
            if spec.exposure == ToolExposure::HarnessOnly
                && spec.profiles.contains(&"codegg_core_min")
            {
                assert!(
                    !model_tools.contains(&spec.name),
                    "harness_only tool '{}' should not appear in codegg_core_min model audience",
                    spec.name
                );
            }
        }
    }

    #[test]
    fn harness_only_excluded_from_model_audience_codegg_core() {
        let model_tools = tool_names_for_profile_audience("codegg_core", ToolListAudience::Model);
        for spec in ALL_TOOLS {
            if spec.exposure == ToolExposure::HarnessOnly && spec.profiles.contains(&"codegg_core")
            {
                assert!(
                    !model_tools.contains(&spec.name),
                    "harness_only tool '{}' should not appear in codegg_core model audience",
                    spec.name
                );
            }
        }
    }

    #[test]
    fn harness_only_excluded_from_model_audience_full() {
        let model_tools = tool_names_for_profile_audience("full", ToolListAudience::Model);
        for spec in ALL_TOOLS {
            if spec.exposure == ToolExposure::HarnessOnly {
                assert!(
                    !model_tools.contains(&spec.name),
                    "harness_only tool '{}' should not appear in full model audience",
                    spec.name
                );
            }
        }
    }

    // -- Harness audience includes expected preflight tools --

    #[test]
    fn harness_audience_includes_harness_only_tools() {
        let harness_tools =
            tool_names_for_profile_audience("codegg_preflight", ToolListAudience::Harness);
        // codegg_preflight should include harness-only tools
        let harness_only_in_profile: Vec<&str> = ALL_TOOLS
            .iter()
            .filter(|t| {
                t.exposure == ToolExposure::HarnessOnly && t.profiles.contains(&"codegg_preflight")
            })
            .map(|t| t.name)
            .collect();
        for name in &harness_only_in_profile {
            assert!(
                harness_tools.contains(name),
                "harness audience for codegg_preflight should include '{}'",
                name
            );
        }
    }

    // -- Profile snapshots --
    //
    // These are exact snapshots of critical codegg-facing profile+audience
    // combinations. Update intentionally when profile contents change.
    // Keep sorted alphabetically before snapshotting (see snapshot_names helper).

    /// Helper: sorted tool names for a profile+audience.
    fn snapshot_names(profile: &str, audience: ToolListAudience) -> Vec<String> {
        let mut names: Vec<String> = tool_names_for_profile_audience(profile, audience)
            .into_iter()
            .map(String::from)
            .collect();
        names.sort();
        names
    }

    #[test]
    fn profile_snapshot_codegg_core_min_model() {
        let actual = snapshot_names("codegg_core_min", ToolListAudience::Model);
        let expected = vec![
            "command_preflight",
            "config_preflight",
            "edit_preflight",
            "text_replace_check",
            "text_security_inspect",
            "validate_json",
        ];
        assert_eq!(
            actual,
            expected.into_iter().map(String::from).collect::<Vec<_>>()
        );
    }

    #[test]
    fn profile_snapshot_codegg_core_model() {
        let actual = snapshot_names("codegg_core", ToolListAudience::Model);
        let expected = vec![
            "cargo_toml_inspect",
            "command_preflight",
            "config_preflight",
            "edit_preflight",
            "identifier_inspect",
            "path_normalize",
            "structured_data_compare",
            "text_diff_explain",
            "text_equal",
            "text_fingerprint",
            "text_inspect",
            "text_replace_check",
            "text_security_inspect",
            "validate_json",
            "validate_toml",
        ];
        assert_eq!(
            actual,
            expected.into_iter().map(String::from).collect::<Vec<_>>()
        );
    }

    #[test]
    fn profile_snapshot_codegg_preflight_harness() {
        let actual = snapshot_names("codegg_preflight", ToolListAudience::Harness);
        let expected = vec![
            "command_preflight",
            "config_preflight",
            "edit_preflight",
            "patch_apply_check",
            "path_scope_check",
            "prompt_input_inspect",
            "shell_split",
            "text_security_inspect",
            "unicode_policy_check",
        ];
        assert_eq!(
            actual,
            expected.into_iter().map(String::from).collect::<Vec<_>>()
        );
    }

    #[test]
    fn profile_snapshot_codegg_patch_model() {
        let actual = snapshot_names("codegg_patch", ToolListAudience::Model);
        let expected = vec![
            "edit_preflight",
            "line_range_compare",
            "line_range_extract",
            "patch_summary",
            "text_diff_explain",
            "text_replace_check",
        ];
        assert_eq!(
            actual,
            expected.into_iter().map(String::from).collect::<Vec<_>>()
        );
    }

    #[test]
    fn profile_snapshot_codegg_patch_harness() {
        let actual = snapshot_names("codegg_patch", ToolListAudience::Harness);
        let expected = vec![
            "edit_preflight",
            "line_range_compare",
            "line_range_extract",
            "patch_apply_check",
            "patch_summary",
            "text_diff_explain",
            "text_replace_check",
        ];
        assert_eq!(
            actual,
            expected.into_iter().map(String::from).collect::<Vec<_>>()
        );
    }

    #[test]
    fn profile_snapshot_codegg_config_model() {
        let actual = snapshot_names("codegg_config", ToolListAudience::Model);
        let expected = vec![
            "config_preflight",
            "dotenv_validate",
            "ini_validate",
            "json_canonicalize",
            "json_compare",
            "json_extract",
            "structured_data_compare",
            "toml_shape",
            "validate_json",
            "validate_schema_light",
            "validate_toml",
            "version_compare",
        ];
        assert_eq!(
            actual,
            expected.into_iter().map(String::from).collect::<Vec<_>>()
        );
    }

    #[test]
    fn profile_snapshot_codegg_config_harness() {
        let actual = snapshot_names("codegg_config", ToolListAudience::Harness);
        let expected = vec![
            "config_preflight",
            "dotenv_validate",
            "ini_validate",
            "json_canonicalize",
            "json_compare",
            "json_extract",
            "structured_data_compare",
            "toml_shape",
            "validate_json",
            "validate_schema_light",
            "validate_toml",
            "version_compare",
        ];
        assert_eq!(
            actual,
            expected.into_iter().map(String::from).collect::<Vec<_>>()
        );
    }

    #[test]
    fn profile_snapshot_codegg_shell_model() {
        let actual = snapshot_names("codegg_shell", ToolListAudience::Model);
        let expected = vec![
            "argv_compare",
            "command_preflight",
            "regex_safety_check",
            "shell_quote_join",
        ];
        assert_eq!(
            actual,
            expected.into_iter().map(String::from).collect::<Vec<_>>()
        );
    }

    #[test]
    fn profile_snapshot_codegg_shell_harness() {
        let actual = snapshot_names("codegg_shell", ToolListAudience::Harness);
        let expected = vec![
            "argv_compare",
            "command_preflight",
            "regex_safety_check",
            "shell_quote_join",
            "shell_split",
        ];
        assert_eq!(
            actual,
            expected.into_iter().map(String::from).collect::<Vec<_>>()
        );
    }

    #[test]
    fn profile_snapshot_codegg_unicode_security_model() {
        let actual = snapshot_names("codegg_unicode_security", ToolListAudience::Model);
        let expected = vec![
            "canonicalize_text",
            "identifier_inspect",
            "text_inspect",
            "text_position",
            "text_security_inspect",
            "text_transform",
        ];
        assert_eq!(
            actual,
            expected.into_iter().map(String::from).collect::<Vec<_>>()
        );
    }

    #[test]
    fn profile_snapshot_codegg_unicode_security_harness() {
        let actual = snapshot_names("codegg_unicode_security", ToolListAudience::Harness);
        let expected = vec![
            "canonicalize_text",
            "identifier_inspect",
            "prompt_input_inspect",
            "text_inspect",
            "text_position",
            "text_security_inspect",
            "text_transform",
            "unicode_policy_check",
        ];
        assert_eq!(
            actual,
            expected.into_iter().map(String::from).collect::<Vec<_>>()
        );
    }

    #[test]
    fn profile_snapshot_codegg_repo_audit_model() {
        let actual = snapshot_names("codegg_repo_audit", ToolListAudience::Model);
        let expected = vec![
            "cargo_toml_inspect",
            "code_fence_extract",
            "identifier_table_inspect",
            "json_shape",
            "markdown_structure",
            "text_fingerprint",
        ];
        assert_eq!(
            actual,
            expected.into_iter().map(String::from).collect::<Vec<_>>()
        );
    }

    #[test]
    fn profile_snapshot_human_math_model() {
        let names = snapshot_names("human_math", ToolListAudience::Model);
        assert!(!names.is_empty(), "human_math model should have tools");
    }

    #[test]
    fn profile_snapshot_default_model() {
        let names = snapshot_names("default", ToolListAudience::Model);
        assert!(!names.is_empty(), "default model should have tools");
        for name in &names {
            let spec = get_tool(name).expect("tool should exist");
            assert_ne!(spec.exposure, ToolExposure::HarnessOnly);
        }
    }

    // -- Invalid profile handling --

    #[test]
    fn invalid_profile_returns_empty() {
        let tools = tools_for_profile("nonexistent_profile");
        assert!(tools.is_empty());
    }

    #[test]
    fn invalid_profile_audience_returns_empty() {
        let tools = tools_for_profile_audience("nonexistent_profile", ToolListAudience::Model);
        assert!(tools.is_empty());
    }
}
