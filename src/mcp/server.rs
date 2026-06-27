use crate::calc::set_mcp_mode;
use crate::mcp::schemas::*;
use crate::mcp::tools::*;
use crate::text::levenshtein_distance;
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::LazyLock;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Mutex;
use tokio::time::Instant;

const MAX_REQUEST_BYTES: usize = 1_000_000;
const MAX_OUTPUT_BYTES: usize = 1_000_000;
const MAX_REQUESTS_PER_SECOND: u32 = 10;
const MAX_REQUEST_ID_LENGTH: usize = 1024;
const MAX_TOOL_TIMEOUT_SECONDS: u64 = 30;
const MAX_CANCELLED_REQUESTS: usize = 10_000;
const MAX_TOOL_WORKERS: usize = 16;

const SCHEMA_DETAIL_FULL: &str = "full";

#[allow(clippy::type_complexity)]
const TOOL_HANDLERS: &[(&str, fn(&Value) -> ToolResponse)] = &[
    ("cargo_toml_inspect", cargo_toml_inspect),
    ("code_fence_extract", code_fence_extract),
    ("dotenv_validate", dotenv_validate),
    ("ini_validate", ini_validate),
    ("escape_text", escape_text),
    ("line_range_compare", line_range_compare_tool),
    ("line_range_extract", line_range_extract_tool),
    ("unescape_text", unescape_text),
    ("json_canonicalize", json_canonicalize),
    ("json_compare", json_compare),
    ("json_extract", json_extract),
    ("json_query", json_query),
    ("json_shape", json_shape_tool),
    ("list_compare", list_compare),
    ("list_dedupe", list_dedupe),
    ("list_sort", list_sort),
    ("math_eval", math_eval),
    ("patch_apply_check", patch_apply_check),
    ("patch_summary", patch_summary),
    ("path_analyze", path_analyze),
    ("path_compare", path_compare),
    ("path_normalize", path_normalize_tool),
    ("path_scope_check", path_scope_check),
    ("regex_finditer", regex_finditer_tool),
    ("regex_safety_check", regex_safety_check_tool),
    ("shell_split", shell_split),
    ("shell_quote_join", shell_quote_join),
    ("argv_compare", argv_compare),
    ("text_count", text_count),
    ("text_diff_explain", text_diff_explain),
    ("text_equal", text_equal),
    ("text_hash", text_hash),
    ("text_inspect", text_inspect),
    ("text_measure", text_measure),
    ("text_position", text_position),
    ("text_replace_check", text_replace_check_tool),
    ("text_truncate", text_truncate),
    ("text_transform", text_transform),
    ("text_window", text_window),
    ("toml_shape", toml_shape_tool),
    ("unit_convert", unit_convert),
    ("unit_info", unit_info),
    ("constant_lookup", constant_lookup),
    ("validate_brackets", validate_brackets),
    ("validate_json", validate_json),
    ("validate_regex", validate_regex),
    ("validate_schema_light", validate_schema_light_tool),
    ("validate_toml", validate_toml_tool),
    ("version_compare", version_compare_tool),
    ("version_constraint_check", version_constraint_check),
    ("identifier_analyze", identifier_analyze),
    ("glob_match", glob_match_tool),
    ("text_fingerprint", text_fingerprint_tool),
    ("identifier_inspect", identifier_inspect),
    ("identifier_table_inspect", identifier_table_inspect),
    ("markdown_structure", markdown_structure),
    ("unicode_policy_check", unicode_policy_check),
    ("canonicalize_text", canonicalize_text),
    ("prompt_input_inspect", prompt_input_inspect_tool),
    ("text_security_inspect", text_security_inspect),
    ("edit_preflight", edit_preflight),
    ("command_preflight", command_preflight),
    ("config_preflight", config_preflight),
    ("structured_data_compare", structured_data_compare),
];

static TOOL_METADATA: LazyLock<HashMap<&'static str, ToolMetadata>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    // Tier 0: Ultra-common
    m.insert(
        "math_eval",
        ToolMetadata {
            category: "math",
            tier: 0,
            profiles: &["full", "default", "human_math"],
            tags: &["math", "evaluation", "arithmetic", "units", "constants"],
            llm_exposure: "default",
            harness_use: &["none"],
            aliases: &[],
            cost: "moderate",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "text_equal",
        ToolMetadata {
            category: "text",
            tier: 0,
            profiles: &["full", "default", "codegg_core"],
            tags: &["text", "comparison", "equality", "unicode"],
            llm_exposure: "default",
            harness_use: &["none"],
            aliases: &[],
            cost: "cheap",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "text_count",
        ToolMetadata {
            category: "text",
            tier: 0,
            profiles: &["full", "default"],
            tags: &["text", "count", "character", "frequency"],
            llm_exposure: "default",
            harness_use: &["none"],
            aliases: &[],
            cost: "cheap",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "text_measure",
        ToolMetadata {
            category: "text",
            tier: 0,
            profiles: &["full", "default"],
            tags: &["text", "measurement", "unicode", "metrics"],
            llm_exposure: "default",
            harness_use: &["none"],
            aliases: &[],
            cost: "cheap",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "text_fingerprint",
        ToolMetadata {
            category: "text",
            tier: 0,
            profiles: &["full", "default", "codegg_core", "codegg_repo_audit"],
            tags: &[
                "text",
                "hash",
                "fingerprint",
                "sha256",
                "identity",
                "canonicalization",
            ],
            llm_exposure: "default",
            harness_use: &["edit_preflight"],
            aliases: &[],
            cost: "cheap",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "validate_json",
        ToolMetadata {
            category: "validation",
            tier: 0,
            profiles: &[
                "full",
                "default",
                "codegg_core",
                "codegg_core_min",
                "codegg_config",
            ],
            tags: &["validation", "json", "structured-data"],
            llm_exposure: "default",
            harness_use: &["config_preflight"],
            aliases: &[],
            cost: "cheap",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "path_normalize",
        ToolMetadata {
            category: "path",
            tier: 0,
            profiles: &["full", "default", "codegg_core"],
            tags: &["text", "path", "filesystem", "normalize"],
            llm_exposure: "default",
            harness_use: &["path_preflight"],
            aliases: &[],
            cost: "cheap",
            stability: "stable",
            composite: false,
        },
    );
    // Tier 1: Default coding-agent sanity
    m.insert(
        "text_diff_explain",
        ToolMetadata {
            category: "text",
            tier: 1,
            profiles: &["full", "default", "codegg_core", "codegg_patch"],
            tags: &["text", "diff", "comparison", "unicode"],
            llm_exposure: "default",
            harness_use: &["edit_preflight"],
            aliases: &[],
            cost: "moderate",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "text_inspect",
        ToolMetadata {
            category: "text",
            tier: 1,
            profiles: &["full", "default", "codegg_core", "codegg_unicode_security"],
            tags: &["text", "unicode", "inspection", "security"],
            llm_exposure: "default",
            harness_use: &["prompt_input_preflight"],
            aliases: &[],
            cost: "moderate",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "text_replace_check",
        ToolMetadata {
            category: "text",
            tier: 1,
            profiles: &[
                "full",
                "default",
                "codegg_core",
                "codegg_core_min",
                "codegg_patch",
            ],
            tags: &["text", "replace", "edit", "safety", "check"],
            llm_exposure: "default",
            harness_use: &["edit_preflight"],
            aliases: &[],
            cost: "cheap",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "line_range_extract",
        ToolMetadata {
            category: "text",
            tier: 1,
            profiles: &["full", "default", "codegg_patch"],
            tags: &["text", "line", "range", "extract", "offset"],
            llm_exposure: "default",
            harness_use: &["edit_preflight"],
            aliases: &[],
            cost: "cheap",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "json_compare",
        ToolMetadata {
            category: "json",
            tier: 1,
            profiles: &["full", "default", "codegg_config"],
            tags: &["json", "structured-data", "comparison", "config"],
            llm_exposure: "default",
            harness_use: &["config_preflight"],
            aliases: &[],
            cost: "moderate",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "json_canonicalize",
        ToolMetadata {
            category: "json",
            tier: 1,
            profiles: &["full", "default", "codegg_config"],
            tags: &["json", "canonical", "hash", "deterministic", "format"],
            llm_exposure: "default",
            harness_use: &["config_preflight"],
            aliases: &[],
            cost: "moderate",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "json_query",
        ToolMetadata {
            category: "json",
            tier: 1,
            profiles: &["full"],
            tags: &["json", "pointer", "extraction", "query", "rfc6901"],
            llm_exposure: "contextual",
            harness_use: &["none"],
            aliases: &[],
            cost: "moderate",
            stability: "deprecated",
            composite: false,
        },
    );
    m.insert(
        "validate_toml",
        ToolMetadata {
            category: "validation",
            tier: 1,
            profiles: &["full", "default", "codegg_core", "codegg_config"],
            tags: &[
                "validation",
                "structured-data",
                "toml",
                "config",
                "rust",
                "python",
            ],
            llm_exposure: "default",
            harness_use: &["config_preflight"],
            aliases: &[],
            cost: "cheap",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "validate_brackets",
        ToolMetadata {
            category: "validation",
            tier: 1,
            profiles: &["full", "default"],
            tags: &["validation", "brackets", "delimiters"],
            llm_exposure: "default",
            harness_use: &["none"],
            aliases: &[],
            cost: "cheap",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "validate_regex",
        ToolMetadata {
            category: "regex",
            tier: 1,
            profiles: &["full", "default"],
            tags: &["text", "regex", "validation", "pattern"],
            llm_exposure: "default",
            harness_use: &["command_preflight"],
            aliases: &[],
            cost: "moderate",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "regex_finditer",
        ToolMetadata {
            category: "regex",
            tier: 1,
            profiles: &["full", "default"],
            tags: &["text", "regex", "search", "find", "pattern"],
            llm_exposure: "default",
            harness_use: &["none"],
            aliases: &[],
            cost: "moderate",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "regex_safety_check",
        ToolMetadata {
            category: "regex",
            tier: 1,
            profiles: &["full", "default", "codegg_shell"],
            tags: &["text", "regex", "safety", "security", "backtracking"],
            llm_exposure: "default",
            harness_use: &["command_preflight"],
            aliases: &[],
            cost: "cheap",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "glob_match",
        ToolMetadata {
            category: "path",
            tier: 1,
            profiles: &["full", "default"],
            tags: &["text", "glob", "pattern", "path", "wildcard"],
            llm_exposure: "default",
            harness_use: &["none"],
            aliases: &[],
            cost: "cheap",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "identifier_inspect",
        ToolMetadata {
            category: "identifier",
            tier: 1,
            profiles: &["full", "default", "codegg_core", "codegg_unicode_security"],
            tags: &[
                "text",
                "identifier",
                "collision",
                "confusable",
                "security",
                "validation",
            ],
            llm_exposure: "default",
            harness_use: &["reasoning_only"],
            aliases: &[],
            cost: "moderate",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "escape_text",
        ToolMetadata {
            category: "text",
            tier: 1,
            profiles: &["full", "default"],
            tags: &["text", "escape", "encoding", "shell", "json", "regex"],
            llm_exposure: "default",
            harness_use: &["none"],
            aliases: &[],
            cost: "cheap",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "unescape_text",
        ToolMetadata {
            category: "text",
            tier: 1,
            profiles: &["full", "default"],
            tags: &["text", "escape", "encoding", "shell", "json", "regex"],
            llm_exposure: "default",
            harness_use: &["none"],
            aliases: &[],
            cost: "cheap",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "text_window",
        ToolMetadata {
            category: "text",
            tier: 1,
            profiles: &["full", "default"],
            tags: &["text", "position", "context", "unicode", "window"],
            llm_exposure: "default",
            harness_use: &["none"],
            aliases: &[],
            cost: "cheap",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "list_dedupe",
        ToolMetadata {
            category: "list",
            tier: 1,
            profiles: &["full", "default"],
            tags: &["list", "dedupe", "unique", "normalization"],
            llm_exposure: "default",
            harness_use: &["none"],
            aliases: &[],
            cost: "cheap",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "list_sort",
        ToolMetadata {
            category: "list",
            tier: 1,
            profiles: &["full", "default"],
            tags: &["list", "sort", "order", "normalization"],
            llm_exposure: "default",
            harness_use: &["none"],
            aliases: &[],
            cost: "cheap",
            stability: "stable",
            composite: false,
        },
    );
    // Tier 2: Contextual / heavier analysis
    m.insert(
        "unit_convert",
        ToolMetadata {
            category: "math",
            tier: 2,
            profiles: &["full", "human_math"],
            tags: &["math", "units", "conversion"],
            llm_exposure: "contextual",
            harness_use: &["none"],
            aliases: &[],
            cost: "cheap",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "unit_info",
        ToolMetadata {
            category: "math",
            tier: 2,
            profiles: &["full", "human_math"],
            tags: &["math", "units", "information"],
            llm_exposure: "contextual",
            harness_use: &["none"],
            aliases: &[],
            cost: "cheap",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "constant_lookup",
        ToolMetadata {
            category: "math",
            tier: 2,
            profiles: &["full", "human_math"],
            tags: &["math", "constants", "physics", "lookup"],
            llm_exposure: "contextual",
            harness_use: &["none"],
            aliases: &[],
            cost: "cheap",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "json_extract",
        ToolMetadata {
            category: "json",
            tier: 2,
            profiles: &["full", "codegg_config"],
            tags: &["json", "structured-data", "extraction", "config", "pointer"],
            llm_exposure: "contextual",
            harness_use: &["none"],
            aliases: &[],
            cost: "moderate",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "list_compare",
        ToolMetadata {
            category: "list",
            tier: 2,
            profiles: &["full"],
            tags: &["text", "list", "comparison", "set"],
            llm_exposure: "contextual",
            harness_use: &["none"],
            aliases: &[],
            cost: "moderate",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "line_range_compare",
        ToolMetadata {
            category: "text",
            tier: 2,
            profiles: &["full", "codegg_patch"],
            tags: &["text", "line", "range", "compare", "diff"],
            llm_exposure: "contextual",
            harness_use: &["edit_preflight"],
            aliases: &[],
            cost: "moderate",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "markdown_structure",
        ToolMetadata {
            category: "markdown",
            tier: 2,
            profiles: &["full", "codegg_repo_audit"],
            tags: &[
                "markdown",
                "structure",
                "headings",
                "code-fences",
                "links",
                "frontmatter",
            ],
            llm_exposure: "contextual",
            harness_use: &["none"],
            aliases: &[],
            cost: "moderate",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "code_fence_extract",
        ToolMetadata {
            category: "markdown",
            tier: 2,
            profiles: &["full", "codegg_repo_audit"],
            tags: &["markdown", "code-fences", "extraction", "fingerprint"],
            llm_exposure: "contextual",
            harness_use: &["none"],
            aliases: &[],
            cost: "moderate",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "patch_apply_check",
        ToolMetadata {
            category: "patch",
            tier: 2,
            profiles: &["full", "codegg_preflight", "codegg_patch"],
            tags: &["patch", "diff", "unified", "validation", "apply"],
            llm_exposure: "harness_only",
            harness_use: &["edit_preflight"],
            aliases: &[],
            cost: "moderate",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "patch_summary",
        ToolMetadata {
            category: "patch",
            tier: 2,
            profiles: &["full", "codegg_patch"],
            tags: &["patch", "diff", "unified", "summary", "statistics"],
            llm_exposure: "contextual",
            harness_use: &["none"],
            aliases: &[],
            cost: "moderate",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "path_analyze",
        ToolMetadata {
            category: "path",
            tier: 2,
            profiles: &["full"],
            tags: &["text", "path", "filesystem", "lexical"],
            llm_exposure: "contextual",
            harness_use: &["none"],
            aliases: &[],
            cost: "cheap",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "path_compare",
        ToolMetadata {
            category: "path",
            tier: 2,
            profiles: &["full"],
            tags: &["text", "path", "filesystem", "comparison"],
            llm_exposure: "contextual",
            harness_use: &["none"],
            aliases: &[],
            cost: "cheap",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "path_scope_check",
        ToolMetadata {
            category: "path",
            tier: 2,
            profiles: &["full", "codegg_preflight"],
            tags: &["text", "path", "filesystem", "security", "scope"],
            llm_exposure: "harness_only",
            harness_use: &["path_preflight"],
            aliases: &[],
            cost: "cheap",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "shell_split",
        ToolMetadata {
            category: "shell",
            tier: 2,
            profiles: &["full", "codegg_preflight", "codegg_shell"],
            tags: &["shell", "argv", "parsing", "security", "sanity"],
            llm_exposure: "harness_only",
            harness_use: &["command_preflight"],
            aliases: &[],
            cost: "cheap",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "shell_quote_join",
        ToolMetadata {
            category: "shell",
            tier: 2,
            profiles: &["full", "codegg_shell"],
            tags: &["shell", "argv", "quoting", "safety"],
            llm_exposure: "contextual",
            harness_use: &["none"],
            aliases: &[],
            cost: "cheap",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "argv_compare",
        ToolMetadata {
            category: "shell",
            tier: 2,
            profiles: &["full", "codegg_shell"],
            tags: &["shell", "argv", "comparison", "sanity"],
            llm_exposure: "contextual",
            harness_use: &["command_preflight"],
            aliases: &[],
            cost: "cheap",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "validate_schema_light",
        ToolMetadata {
            category: "validation",
            tier: 3,
            profiles: &["full", "codegg_config"],
            tags: &["validation", "json", "schema", "structured-data"],
            llm_exposure: "contextual",
            harness_use: &["config_preflight"],
            aliases: &[],
            cost: "moderate",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "toml_shape",
        ToolMetadata {
            category: "toml",
            tier: 2,
            profiles: &["full", "codegg_config"],
            tags: &["toml", "structure", "shape", "config", "validation"],
            llm_exposure: "contextual",
            harness_use: &["none"],
            aliases: &[],
            cost: "moderate",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "version_compare",
        ToolMetadata {
            category: "version",
            tier: 2,
            profiles: &["full", "codegg_config"],
            tags: &["version", "semver", "comparison"],
            llm_exposure: "contextual",
            harness_use: &["none"],
            aliases: &[],
            cost: "cheap",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "unicode_policy_check",
        ToolMetadata {
            category: "unicode",
            tier: 2,
            profiles: &["full", "codegg_preflight", "codegg_unicode_security"],
            tags: &["text", "unicode", "policy", "security", "validation"],
            llm_exposure: "harness_only",
            harness_use: &["prompt_input_preflight"],
            aliases: &[],
            cost: "moderate",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "canonicalize_text",
        ToolMetadata {
            category: "unicode",
            tier: 2,
            profiles: &["full", "codegg_unicode_security"],
            tags: &[
                "text",
                "unicode",
                "canonicalization",
                "normalization",
                "identity",
            ],
            llm_exposure: "contextual",
            harness_use: &["prompt_input_preflight"],
            aliases: &[],
            cost: "moderate",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "prompt_input_inspect",
        ToolMetadata {
            category: "text",
            tier: 2,
            profiles: &["full", "codegg_unicode_security", "codegg_preflight"],
            tags: &[
                "text",
                "security",
                "inspection",
                "prompt",
                "unicode",
                "hidden",
            ],
            llm_exposure: "harness_only",
            harness_use: &["prompt_input_preflight"],
            aliases: &[],
            cost: "moderate",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "text_hash",
        ToolMetadata {
            category: "text",
            tier: 2,
            profiles: &["full"],
            tags: &["text", "hash", "identity", "security"],
            llm_exposure: "contextual",
            harness_use: &["none"],
            aliases: &[],
            cost: "moderate",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "text_position",
        ToolMetadata {
            category: "text",
            tier: 2,
            profiles: &["full", "codegg_unicode_security"],
            tags: &["text", "position", "offset", "unicode", "lsp"],
            llm_exposure: "contextual",
            harness_use: &["none"],
            aliases: &[],
            cost: "cheap",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "text_transform",
        ToolMetadata {
            category: "text",
            tier: 2,
            profiles: &["full", "codegg_unicode_security"],
            tags: &[
                "text",
                "unicode",
                "transform",
                "normalization",
                "sanitation",
            ],
            llm_exposure: "contextual",
            harness_use: &["none"],
            aliases: &[],
            cost: "moderate",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "dotenv_validate",
        ToolMetadata {
            category: "config",
            tier: 2,
            profiles: &["full", "codegg_config"],
            tags: &["validation", "config", "env", "dotenv"],
            llm_exposure: "contextual",
            harness_use: &["config_preflight"],
            aliases: &[],
            cost: "cheap",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "ini_validate",
        ToolMetadata {
            category: "config",
            tier: 2,
            profiles: &["full", "codegg_config"],
            tags: &["validation", "config", "ini"],
            llm_exposure: "contextual",
            harness_use: &["config_preflight"],
            aliases: &[],
            cost: "cheap",
            stability: "stable",
            composite: false,
        },
    );
    // Tier 3: Specialized / domain-specific
    m.insert(
        "identifier_analyze",
        ToolMetadata {
            category: "identifier",
            tier: 3,
            profiles: &["full"],
            tags: &["text", "identifier", "naming", "validation", "language"],
            llm_exposure: "expert_only",
            harness_use: &["none"],
            aliases: &[],
            cost: "moderate",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "identifier_table_inspect",
        ToolMetadata {
            category: "identifier",
            tier: 3,
            profiles: &["full", "codegg_repo_audit"],
            tags: &[
                "text",
                "identifier",
                "collision",
                "naming",
                "style",
                "reserved",
                "validation",
            ],
            llm_exposure: "expert_only",
            harness_use: &["repo_audit"],
            aliases: &[],
            cost: "moderate",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "json_shape",
        ToolMetadata {
            category: "json",
            tier: 3,
            profiles: &["full", "codegg_repo_audit"],
            tags: &["json", "structured-data", "shape", "schema"],
            llm_exposure: "expert_only",
            harness_use: &["none"],
            aliases: &[],
            cost: "moderate",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "text_truncate",
        ToolMetadata {
            category: "text",
            tier: 3,
            profiles: &["full"],
            tags: &["text", "truncation", "grapheme", "unicode"],
            llm_exposure: "expert_only",
            harness_use: &["none"],
            aliases: &[],
            cost: "cheap",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "version_constraint_check",
        ToolMetadata {
            category: "version",
            tier: 3,
            profiles: &["full"],
            tags: &["version", "semver", "cargo", "constraint", "satisfiability"],
            llm_exposure: "expert_only",
            harness_use: &["none"],
            aliases: &[],
            cost: "cheap",
            stability: "stable",
            composite: false,
        },
    );
    m.insert(
        "cargo_toml_inspect",
        ToolMetadata {
            category: "cargo",
            tier: 3,
            profiles: &["full", "codegg_core", "codegg_repo_audit"],
            tags: &[
                "rust",
                "cargo",
                "toml",
                "dependencies",
                "workspace",
                "inspection",
            ],
            llm_exposure: "expert_only",
            harness_use: &["config_preflight"],
            aliases: &[],
            cost: "moderate",
            stability: "stable",
            composite: false,
        },
    );
    // Composite tools
    m.insert(
        "text_security_inspect",
        ToolMetadata {
            category: "text",
            tier: 1,
            profiles: &[
                "full",
                "codegg_core",
                "codegg_core_min",
                "codegg_preflight",
                "codegg_unicode_security",
            ],
            tags: &[
                "text",
                "unicode",
                "security",
                "composite",
                "prompt",
                "inspection",
            ],
            llm_exposure: "default",
            harness_use: &["prompt_input_preflight"],
            aliases: &[],
            cost: "heavy",
            stability: "stable",
            composite: true,
        },
    );
    m.insert(
        "edit_preflight",
        ToolMetadata {
            category: "patch",
            tier: 1,
            profiles: &[
                "full",
                "codegg_core",
                "codegg_core_min",
                "codegg_preflight",
                "codegg_patch",
            ],
            tags: &["patch", "edit", "preflight", "composite", "text"],
            llm_exposure: "default",
            harness_use: &["edit_preflight"],
            aliases: &[],
            cost: "heavy",
            stability: "stable",
            composite: true,
        },
    );
    m.insert(
        "command_preflight",
        ToolMetadata {
            category: "shell",
            tier: 1,
            profiles: &[
                "full",
                "codegg_core",
                "codegg_core_min",
                "codegg_preflight",
                "codegg_shell",
            ],
            tags: &["shell", "command", "preflight", "composite", "security"],
            llm_exposure: "default",
            harness_use: &["command_preflight"],
            aliases: &[],
            cost: "heavy",
            stability: "stable",
            composite: true,
        },
    );
    m.insert(
        "config_preflight",
        ToolMetadata {
            category: "config",
            tier: 1,
            profiles: &[
                "full",
                "codegg_core",
                "codegg_core_min",
                "codegg_preflight",
                "codegg_config",
            ],
            tags: &[
                "config",
                "validation",
                "json",
                "toml",
                "preflight",
                "composite",
            ],
            llm_exposure: "default",
            harness_use: &["config_preflight"],
            aliases: &[],
            cost: "heavy",
            stability: "stable",
            composite: true,
        },
    );
    m.insert(
        "structured_data_compare",
        ToolMetadata {
            category: "json",
            tier: 2,
            profiles: &["full", "codegg_core", "codegg_config"],
            tags: &[
                "json",
                "comparison",
                "config",
                "structured-data",
                "composite",
            ],
            llm_exposure: "contextual",
            harness_use: &["config_preflight"],
            aliases: &[],
            cost: "heavy",
            stability: "stable",
            composite: true,
        },
    );
    m
});

static PROFILE_NAMES: &[&str] = &[
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

static TOOL_PROFILES: LazyLock<HashMap<&'static str, Vec<&'static str>>> = LazyLock::new(|| {
    let mut profiles: HashMap<&'static str, Vec<&'static str>> = HashMap::new();
    for (&tool_name, meta) in TOOL_METADATA.iter() {
        for &profile in meta.profiles {
            profiles.entry(profile).or_default().push(tool_name);
        }
    }
    for list in profiles.values_mut() {
        list.sort();
    }
    profiles
});

use std::sync::RwLock;

static ACTIVE_PROFILE: LazyLock<RwLock<String>> = LazyLock::new(|| {
    let profile = std::env::var("EGGCALC_MCP_PROFILE").unwrap_or_else(|_| "full".to_string());
    if !PROFILE_NAMES.contains(&profile.as_str()) {
        let available: Vec<&str> = PROFILE_NAMES.iter().copied().collect();
        eprintln!(
            "Error: Invalid EGGCALC_MCP_PROFILE: {:?}. Available profiles: {}",
            profile,
            available.join(", ")
        );
        std::process::exit(1);
    }
    RwLock::new(profile)
});

static ACTIVE_SCHEMA_DETAIL: LazyLock<RwLock<String>> = LazyLock::new(|| {
    let detail = std::env::var("EGGCALC_MCP_SCHEMA_DETAIL")
        .unwrap_or_else(|_| SCHEMA_DETAIL_FULL.to_string());
    RwLock::new(detail)
});

/// Set the active MCP profile. Returns Ok(()) on success, or Err with available profiles on failure.
pub fn set_active_profile(name: &str) -> Result<(), String> {
    if !PROFILE_NAMES.contains(&name) {
        let available: Vec<&str> = PROFILE_NAMES.iter().copied().collect();
        return Err(format!(
            "Unknown profile: {:?}. Available profiles: {}",
            name,
            available.join(", ")
        ));
    }
    let mut profile = ACTIVE_PROFILE.write().map_err(|e| e.to_string())?;
    *profile = name.to_string();
    Ok(())
}

/// Get the currently active MCP profile name.
pub fn get_active_profile() -> String {
    let profile = ACTIVE_PROFILE.read().unwrap_or_else(|e| e.into_inner());
    profile.clone()
}

/// Set the schema detail level (compact, normal, full).
pub fn set_schema_detail(level: &str) -> Result<(), String> {
    if level != "compact" && level != "normal" && level != "full" {
        return Err(format!(
            "Invalid schema detail: {:?}. Use compact, normal, or full.",
            level
        ));
    }
    let mut detail = ACTIVE_SCHEMA_DETAIL.write().map_err(|e| e.to_string())?;
    *detail = level.to_string();
    Ok(())
}

/// Get the current schema detail level.
pub fn get_schema_detail() -> String {
    let detail = ACTIVE_SCHEMA_DETAIL
        .read()
        .unwrap_or_else(|e| e.into_inner());
    detail.clone()
}

fn get_profile_tools(profile: &str) -> Vec<&'static str> {
    if profile == "full" {
        return TOOL_METADATA
            .iter()
            .filter(|(_, meta)| meta.llm_exposure != "hidden")
            .map(|(&name, _)| name)
            .collect();
    }
    TOOL_PROFILES.get(profile).cloned().unwrap_or_default()
}

fn enrich_tool(tool: ToolDefinition) -> ToolDefinition {
    let meta = match TOOL_METADATA.get(tool.name.as_str()) {
        Some(m) => m,
        None => return tool,
    };
    let mut enriched = tool;
    enriched.tier = Some(meta.tier);
    enriched.category = Some(meta.category.to_string());
    enriched.llm_exposure = Some(meta.llm_exposure.to_string());
    enriched.cost = Some(meta.cost.to_string());
    if meta.stability == "deprecated" {
        enriched.deprecated = Some(true);
    }
    // Use tags from metadata
    let tags: Vec<String> = meta.tags.iter().map(|s| s.to_string()).collect();
    enriched.tags = Some(tags);
    // Populate output_schema from the static map if not already set
    if enriched.output_schema.is_none() {
        enriched.output_schema = OUTPUT_SCHEMAS.get(enriched.name.as_str()).cloned();
    }
    enriched
}

static OUTPUT_SCHEMAS: LazyLock<HashMap<&'static str, Value>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert("math_eval", serde_json::json!({"type":"object","properties":{"value":{"type":"string","description":"Evaluation result as string"},"type":{"type":"string","description":"Python type name of the result"},"unit":{"type":["string","null"],"description":"Unit name (only when result has units)"},"display":{"type":["string","null"],"description":"Human-readable result with units (only when result has units)"}}}));
    m.insert("unit_convert", serde_json::json!({"type":"object","properties":{"value":{"type":"number","description":"Converted value"},"from_unit":{"type":"string"},"to_unit":{"type":"string"},"factor":{"type":["number","null"],"description":"Conversion factor used (null for temperature conversions)"}}}));
    m.insert("unit_info", serde_json::json!({"type":"object","properties":{"unit":{"type":"string"},"canonical":{"type":"string","description":"Canonical unit name"},"category":{"type":"string","description":"Unit category (e.g., 'length', 'mass', 'temperature')"},"is_valid":{"type":"boolean"}}}));
    m.insert("constant_lookup", serde_json::json!({"type":"object","properties":{"name":{"type":"string"},"value":{"type":"number","description":"Constant value"},"symbol":{"type":"string","description":"Display symbol (e.g., 'N_A', 'h', 'c')"},"display_name":{"type":"string","description":"Human-readable name"}}}));
    m.insert("text_measure", serde_json::json!({"type":"object","properties":{"bytes_utf8":{"type":"integer"},"codepoints":{"type":"integer"},"graphemes":{"type":"integer"},"words":{"type":"integer"},"unique_words_casefolded":{"type":"integer"},"lines":{"type":"integer"},"nonempty_lines":{"type":"integer"},"blank_lines":{"type":"integer"},"max_line_length_codepoints":{"type":"integer"},"chars_no_whitespace":{"type":"integer"},"ascii":{"type":"integer"},"non_ascii":{"type":"integer"},"letters":{"type":"integer"},"digits":{"type":"integer"},"punctuation":{"type":"integer"},"symbols":{"type":"integer"},"spaces":{"type":"integer"},"control_chars":{"type":"integer"},"combining_marks":{"type":"integer"},"invisible_chars":{"type":"integer"},"newline_style":{"type":"string"},"ends_with_newline":{"type":"boolean"},"normalization":{"type":"object"},"unicode_risks":{"type":"object"},"warnings":{"type":"array"}}}));
    m.insert("text_equal", serde_json::json!({"type":"object","properties":{"equal":{"type":"boolean"},"mode":{"type":"object"},"raw_equal":{"type":"boolean"},"nfc_equal":{"type":"boolean"},"nfd_equal":{"type":"boolean"},"nfkc_equal":{"type":"boolean"},"nfkd_equal":{"type":"boolean"},"casefold_equal":{"type":"boolean"},"byte_equal":{"type":"boolean"},"lengths":{"type":"object"},"first_difference":{"type":["object","null"]},"classification":{"type":"string"}}}));
    m.insert("text_diff_explain", serde_json::json!({"type":"object","properties":{"equal":{"type":"boolean"},"classification":{"type":"string"},"summary":{"type":"object"},"a_metrics":{"type":"object"},"b_metrics":{"type":"object"},"diffs":{"type":"array"},"security_findings":{"type":"array"},"agent_instruction":{"type":"string"}}}));
    m.insert("text_inspect", serde_json::json!({"type":"object","properties":{"safe_repr":{"type":"string"},"metrics":{"type":"object"},"normalization":{"type":"object"},"normalization_diff":{"type":"boolean"},"normals_repr":{"type":["string","null"]},"invisibles":{"type":"array"},"bidi_controls":{"type":"array"},"mixed_scripts":{"type":"object"},"confusables":{"type":"array"},"warnings":{"type":"array"},"limits_applied":{"type":"array"},"normalize":{"type":"string"},"compare_normalized":{"type":"boolean"},"original":{"type":"object"},"normalized":{"type":["object","null"]},"normalization_findings":{"type":"array"}}}));
    m.insert("text_count", serde_json::json!({"type":"object","description":"With target: {count, positions, target, normalization, text_length_codepoints}. Without target: character frequency table as {char: count} pairs.","properties":{"count":{"type":"integer"},"positions":{"type":"array"},"target":{"type":["string","null"]},"normalization":{"type":["string","null"]},"text_length_codepoints":{"type":"integer"}}}));
    m.insert("text_truncate", serde_json::json!({"type":"object","properties":{"text":{"type":"string","description":"Result string (truncated if truncation occurred)"},"original_graphemes":{"type":"integer","description":"Original grapheme count"},"truncated_graphemes":{"type":"integer","description":"Grapheme count in result"},"truncated":{"type":"boolean","description":"True if text was truncated"}}}));
    m.insert("text_transform", serde_json::json!({"type":"object","properties":{"changed":{"type":"boolean"},"text":{"type":"string"},"operations_applied":{"type":"array","items":{"type":"string"}},"removed":{"type":"array"},"warnings":{"type":"array","items":{"type":"string"}},"summary":{"type":"string"}}}));
    m.insert("validate_brackets", serde_json::json!({"type":"object","properties":{"balanced":{"type":"boolean"},"unmatched_openers":{"type":"array"},"unmatched_closers":{"type":"array"}}}));
    m.insert("validate_json", serde_json::json!({"type":"object","properties":{"valid":{"type":"boolean"},"error":{"type":["string","null"]},"line":{"type":["integer","null"]},"column":{"type":["integer","null"]},"position":{"type":["integer","null"]},"type":{"type":["string","null"]},"top_level_keys":{"type":["array","null"],"items":{"type":"string"}}}}));
    m.insert("validate_regex", serde_json::json!({"type":"object","properties":{"valid_pattern":{"type":"boolean"},"results":{"type":"array"},"error":{"type":["string","null"]},"flags_used":{"type":"object"}}}));
    m.insert("list_compare", serde_json::json!({"type":"object","properties":{"equal":{"type":"boolean"},"first_diff_index":{"type":["integer","null"],"description":"Index of first difference (ordered mode)"},"equal_prefix_length":{"type":"integer","description":"Length of equal prefix (ordered mode)"},"aligned":{"type":"array","description":"Aligned operations (ordered mode)"},"count_deltas":{"type":"object","description":"Count differences (multiset mode)"},"only_in_a":{"type":"array"},"only_in_b":{"type":"array"},"missing_in_a":{"type":"array","description":"Alias for only_in_b"},"missing_in_b":{"type":"array","description":"Alias for only_in_a"},"duplicates_in_a":{"type":"array"},"duplicates_in_b":{"type":"array"},"near_matches":{"type":"array","description":"Items that differ only by edit distance"}}}));
    m.insert("validate_toml", serde_json::json!({"type":"object","properties":{"valid":{"type":"boolean"},"error":{"type":["string","null"]},"line":{"type":["integer","null"]},"column":{"type":["integer","null"]},"position":{"type":["integer","null"]},"type":{"type":["string","null"]},"top_level_keys":{"type":["array","null"]},"tables":{"type":["array","null"]}}}));
    m.insert("json_extract", serde_json::json!({"type":"object","properties":{"valid_json":{"type":"boolean"},"found":{"type":"boolean"},"pointer":{"type":"string"},"value_type":{"type":["string","null"]},"value":{"description":"Extracted value"},"preview":{"type":["string","null"]},"child_keys":{"type":["array","null"],"items":{"type":"string"}},"array_length":{"type":["integer","null"]},"truncated":{"type":"boolean"},"missing_at":{"type":["string","null"]},"reason":{"type":["string","null"]},"available_keys":{"type":["array","null"],"items":{"type":"string"}},"error":{"type":["string","null"]},"line":{"type":["integer","null"]},"column":{"type":["integer","null"]},"summary":{"type":"string"}}}));
    m.insert("json_compare", serde_json::json!({"type":"object","properties":{"valid_json_a":{"type":"boolean"},"valid_json_b":{"type":"boolean"},"equal":{"type":"boolean"},"same_type":{"type":"boolean"},"diff_count":{"type":"integer"},"diffs":{"type":"array","description":"List of differences"},"truncated":{"type":"boolean"},"summary":{"type":"string"}}}));
    m.insert("text_position", serde_json::json!({"type":"object","properties":{"valid":{"type":"boolean"},"byte_offset":{"type":["integer","null"]},"codepoint_index":{"type":["integer","null"]},"utf16_offset":{"type":["integer","null"]},"line":{"type":["integer","null"]},"column":{"type":["integer","null"]},"line_base":{"type":"integer"},"column_base":{"type":"integer"},"char":{"type":["string","null"]},"codepoint":{"type":["string","null"]},"name":{"type":["string","null"]},"line_text_preview":{"type":["string","null"]},"error":{"type":["string","null"]},"summary":{"type":"string"}}}));
    m.insert("text_hash", serde_json::json!({"type":"object","properties":{"encoding":{"type":"string"},"bytes":{"type":"integer"},"codepoints":{"type":"integer"},"hashes":{"type":"object","description":"Map of algorithm to hash value"},"warnings":{"type":"array","items":{"type":"string"}},"summary":{"type":"string"}}}));
    m.insert("escape_text", serde_json::json!({"type":"object","properties":{"mode":{"type":"string"},"escaped":{"type":"string"},"changed":{"type":"boolean"},"summary":{"type":"string"}}}));
    m.insert("unescape_text", serde_json::json!({"type":"object","properties":{"mode":{"type":"string"},"unescaped":{"type":"string"},"changed":{"type":"boolean"},"error":{"type":["string","null"]},"summary":{"type":"string"}}}));
    m.insert("identifier_analyze", serde_json::json!({"type":"object","properties":{"text":{"type":"string"},"classification":{"type":"string"},"python_valid":{"type":"boolean"},"python_keyword":{"type":"boolean"},"rust_valid":{"type":["boolean","null"]},"javascript_valid":{"type":["boolean","null"]},"env_valid":{"type":"boolean"},"suggestions":{"type":"object","description":"Map of language to suggested name"},"warnings":{"type":"array","items":{"type":"string"}},"summary":{"type":"string"}}}));
    m.insert("regex_finditer", serde_json::json!({"type":"object","properties":{"valid_pattern":{"type":"boolean"},"matches":{"type":"array","description":"List of regex matches with positions and groups"},"truncated":{"type":"boolean"},"match_count":{"type":"integer"},"error":{"type":["string","null"]}}}));
    m.insert("regex_safety_check", serde_json::json!({"type":"object","properties":{"valid_pattern":{"type":"boolean"},"risk":{"type":"string","enum":["low","medium","high"]},"findings":{"type":"array","description":"Safety findings with kind, span, and message","items":{"type":"object","properties":{"kind":{"type":"string"},"span":{"type":"array","items":{"type":"integer"}},"message":{"type":"string"}}}}}}));
    m.insert("validate_schema_light", serde_json::json!({"type":"object","properties":{"valid":{"type":"boolean"},"violations":{"type":"array","description":"Schema violations with path and message","items":{"type":"object","properties":{"path":{"type":"string"},"message":{"type":"string"},"value_type":{"type":["string","null"]},"expected_type":{"type":["string","null"]}}}},"truncated":{"type":"boolean"},"summary":{"type":"string"}}}));
    m.insert("path_normalize", serde_json::json!({"type":"object","properties":{"normalized":{"type":"string"},"is_absolute":{"type":"boolean"},"components":{"type":"array","items":{"type":"string"}},"warnings":{"type":"array","items":{"type":"string"}}}}));
    m.insert("path_analyze", serde_json::json!({"type":"object","properties":{"input":{"type":"string"},"style":{"type":"string"},"absolute":{"type":"boolean"},"has_traversal":{"type":"boolean"},"components":{"type":"array","items":{"type":"string"}},"parent":{"type":["string","null"]},"name":{"type":["string","null"]},"stem":{"type":["string","null"]},"suffix":{"type":["string","null"]},"suffixes":{"type":"array","items":{"type":"string"}},"hidden":{"type":"boolean"},"normalized_lexical":{"type":"string"},"warnings":{"type":"array","items":{"type":"string"}},"summary":{"type":"string"}}}));
    m.insert("path_compare", serde_json::json!({"type":"object","properties":{"equal":{"type":"boolean","description":"Whether paths are equal under normalization"},"left_normalized":{"type":"string","description":"Normalized left path"},"right_normalized":{"type":"string","description":"Normalized right path"},"differences":{"type":"array","description":"List of differences found"},"findings":{"type":"array","description":"Normalization notes"}}}));
    m.insert("path_scope_check", serde_json::json!({"type":"object","properties":{"inside_root":{"type":"boolean","description":"Whether target is lexically inside root"},"root_normalized":{"type":"string","description":"Normalized root path"},"target_normalized":{"type":"string","description":"Normalized target path"},"relative_path":{"type":"string","description":"Relative path from root to target (if inside)"},"escapes_via_dotdot":{"type":"boolean","description":"Whether target contains parent traversal"},"absolute_target":{"type":"string","description":"Absolute form of target"},"findings":{"type":"array","description":"Analysis notes"}}}));
    m.insert("json_shape", serde_json::json!({"type":"object","properties":{"valid":{"type":"boolean"},"shape":{"type":["object","null"],"description":"Nested shape structure with type, keys, and counts","properties":{"type":{"type":"string"},"keys":{"type":["object","null"]},"key_count":{"type":["integer","null"]},"item_types":{"type":["array","null"]},"item_count":{"type":["integer","null"]}}},"truncated":{"type":"boolean"},"summary":{"type":"string"}}}));
    m.insert("text_window", serde_json::json!({"type":"object","properties":{"position":{"type":"object","description":"Resolved position with byte_offset, codepoint_index, grapheme_index, line, column"},"line_text":{"type":"string"},"line_visible_repr":{"type":"string"},"before":{"type":"array","description":"Context lines before"},"after":{"type":"array","description":"Context lines after"},"newline_style":{"type":"string"},"at_codepoint":{"type":["object","null"]},"warnings":{"type":"array","items":{"type":"string"}}}}));
    m.insert("json_canonicalize", serde_json::json!({"type":"object","properties":{"valid":{"type":"boolean"},"canonical":{"type":["string","null"]},"minified":{"type":["string","null"]},"sha256":{"type":["string","null"]},"duplicate_keys":{"type":"array","items":{"type":"string"}},"top_level_type":{"type":["string","null"]},"top_level_keys":{"type":["array","null"],"items":{"type":"string"}},"error":{"type":["string","null"]},"line":{"type":["integer","null"]},"column":{"type":["integer","null"]}}}));
    m.insert("json_query", serde_json::json!({"type":"object","properties":{"found":{"type":"boolean"},"pointer":{"type":"string"},"value":{"description":"Extracted value"},"type":{"type":["string","null"]},"missing_at":{"type":["string","null"]},"reason":{"type":["string","null"]},"error":{"type":["string","null"]},"line":{"type":["integer","null"]},"column":{"type":["integer","null"]}}}));
    m.insert("glob_match", serde_json::json!({"type":"object","properties":{"matches":{"type":"boolean"},"normalized_pattern":{"type":"string"},"normalized_path":{"type":"string"},"matched_segment":{"type":["string","null"]},"unmatched_segment":{"type":["string","null"]},"summary":{"type":"string"}}}));
    m.insert("text_fingerprint", serde_json::json!({"type":"object","properties":{"sha256":{"type":"string"},"bytes_utf8":{"type":"integer"},"codepoints":{"type":"integer"},"graphemes":{"type":"integer"},"newline_style":{"type":"string"},"normalization":{"type":"object","description":"Normalization state details"},"summary":{"type":"string"}}}));
    m.insert("identifier_inspect", serde_json::json!({"type":"object","properties":{"identifiers":{"type":"array","description":"Per-identifier analysis with raw, normalized, valid, scripts, and issues"},"collisions":{"type":"array","description":"Detected collisions between identifiers"}}}));
    m.insert("version_compare", serde_json::json!({"type":"object","properties":{"comparison":{"type":"integer","description":"Comparison result: -1 (a < b), 0 (equal), 1 (a > b)"},"valid":{"type":"boolean","description":"Whether versions are valid for the scheme"},"scheme":{"type":"string"},"summary":{"type":"string"}}}));
    m.insert("toml_shape", serde_json::json!({"type":"object","properties":{"valid":{"type":"boolean"},"top_level_keys":{"type":["array","null"],"items":{"type":"string"}},"tables":{"type":["array","null"],"items":{"type":"string"}},"truncated":{"type":"boolean"},"summary":{"type":"string"}}}));
    m.insert("list_dedupe", serde_json::json!({"type":"object","properties":{"items":{"type":"array","items":{"type":"string"}},"original_count":{"type":"integer"},"deduped_count":{"type":"integer"},"duplicates_removed":{"type":"integer"}}}));
    m.insert("list_sort", serde_json::json!({"type":"object","properties":{"items":{"type":"array","items":{"type":"string"}},"original_count":{"type":"integer"},"sorted_count":{"type":"integer"}}}));
    m.insert("text_replace_check", serde_json::json!({"type":"object","properties":{"match_count":{"type":"integer","description":"Number of matches found"},"unique_match":{"type":"boolean","description":"True if exactly one match"},"expected_count_met":{"type":"boolean","description":"True if match count matches expected_count"},"would_change":{"type":"boolean","description":"True if replacement would change text"},"positions":{"type":"array","description":"Match positions with byte offsets and line/column"},"changed_text_fingerprint":{"type":"string","description":"SHA-256 fingerprint of changed text"},"newline_style_before":{"type":"string"},"newline_style_after":{"type":"string"},"preview_before":{"type":"string"},"preview_after":{"type":"string"},"findings":{"type":"array","description":"Warnings and info messages"}}}));
    m.insert("line_range_extract", serde_json::json!({"type":"object","properties":{"line_count_total":{"type":"integer","description":"Total line count in input"},"start_line":{"type":"integer"},"end_line":{"type":"integer"},"valid_range":{"type":"boolean","description":"True if range is within bounds"},"text":{"type":"string","description":"Extracted text (lines joined by LF)"},"lines":{"type":"array","description":"Structured line list"},"byte_start":{"type":"integer","description":"UTF-8 byte offset of start"},"byte_end":{"type":"integer","description":"UTF-8 byte offset of end"},"char_start":{"type":"integer","description":"Codepoint index of start"},"char_end":{"type":"integer","description":"Codepoint index of end"},"newline_style":{"type":"string"},"ends_with_newline":{"type":"boolean"},"fingerprint":{"type":"string"},"findings":{"type":"array"}}}));
    m.insert("line_range_compare", serde_json::json!({"type":"object","properties":{"equal":{"type":"boolean","description":"True if ranges are equal under the chosen mode"},"left_fingerprint":{"type":"string","description":"SHA-256 fingerprint of left range"},"right_fingerprint":{"type":"string","description":"SHA-256 fingerprint of right range"},"diff_summary":{"type":"string","description":"Human-readable diff summary"},"first_difference":{"type":"object","description":"First differing line (if any)"}}}));
    m.insert("shell_split", serde_json::json!({"type":"object","properties":{"parse_ok":{"type":"boolean","description":"True if the command parsed successfully"},"argv":{"type":"array","items":{"type":"string"},"description":"Parsed argument tokens"},"argc":{"type":"integer","description":"Number of arguments"},"features":{"type":"object","description":"Detected risky features","properties":{"has_pipe":{"type":"boolean"},"has_redirection":{"type":"boolean"},"has_command_substitution":{"type":"boolean"},"has_variable_expansion":{"type":"boolean"},"has_glob_pattern":{"type":"boolean"},"has_control_operator":{"type":"boolean"},"has_unbalanced_quotes":{"type":"boolean"}}},"findings":{"type":"array","items":{"type":"string"},"description":"Analysis notes and warnings"}}}));
    m.insert("shell_quote_join", serde_json::json!({"type":"object","properties":{"command":{"type":"string","description":"Safely quoted command string"},"roundtrip_ok":{"type":"boolean","description":"True if shell_split(quote_join(argv)) produces equivalent argv"},"findings":{"type":"array","items":{"type":"string"},"description":"Analysis notes"}}}));
    m.insert("argv_compare", serde_json::json!({"type":"object","properties":{"argv_equal":{"type":"boolean","description":"True if parsed argv lists are identical"},"left_argv":{"type":"array","items":{"type":"string"},"description":"Resolved left argv"},"right_argv":{"type":"array","items":{"type":"string"},"description":"Resolved right argv"},"first_difference":{"type":"integer","description":"Index of first differing token, or null if equal"},"findings":{"type":"array","items":{"type":"string"},"description":"Analysis notes"}}}));
    m.insert("markdown_structure", serde_json::json!({"type":"object","properties":{"headings":{"type":"array","description":"Headings with level, text, line, slug"},"code_fences":{"type":"array","description":"Code fences with language, lines, closed state"},"links":{"type":"array","description":"Links with visible text, target, mismatch flags"},"html_comments":{"type":"array","description":"HTML comments with text and position"},"frontmatter":{"type":"object","description":"Frontmatter detection (present, format, line range)"},"tables_detected":{"type":"boolean","description":"Whether Markdown tables were detected"},"findings":{"type":"array","items":{"type":"string"},"description":"Warnings and findings"}}}));
    m.insert("code_fence_extract", serde_json::json!({"type":"object","properties":{"blocks":{"type":"array","description":"Extracted code blocks with index, language, lines, content, fingerprint"},"unclosed_fences":{"type":"array","description":"Unclosed code fences found"},"findings":{"type":"array","items":{"type":"string"},"description":"Warnings and findings"}}}));
    m.insert("dotenv_validate", serde_json::json!({"type":"object","properties":{"parse_ok":{"type":"boolean","description":"True if no parse errors found"},"entries":{"type":"array","description":"Parsed entries with key, value, quote_style, line"},"duplicates":{"type":"array","description":"Duplicate key entries with line numbers"},"invalid_lines":{"type":"array","description":"Lines that failed to parse"},"requires_quoting":{"type":"array","description":"Keys whose values contain spaces and should be quoted"},"contains_expansion_syntax":{"type":"array","description":"Keys with ${VAR} or $VAR expansion syntax"},"findings":{"type":"array","items":{"type":"string"},"description":"Human-readable findings"}}}));
    m.insert("ini_validate", serde_json::json!({"type":"object","properties":{"parse_ok":{"type":"boolean","description":"True if no parse errors found"},"sections":{"type":"array","description":"Ordered list of section names"},"keys_by_section":{"type":"object","description":"Keys grouped by section"},"duplicates":{"type":"array","description":"Duplicate keys/sections with line numbers"},"invalid_lines":{"type":"array","description":"Lines that failed to parse"},"findings":{"type":"array","items":{"type":"string"},"description":"Human-readable findings"}}}));
    m.insert("patch_apply_check", serde_json::json!({"type":"object","properties":{"patch_parse_ok":{"type":"boolean","description":"True if patch parsed successfully"},"applies":{"type":"boolean","description":"True if all hunks applied cleanly"},"hunks_total":{"type":"integer","description":"Total number of hunks in patch"},"hunks_applied":{"type":"integer","description":"Number of hunks that applied successfully"},"hunks_failed":{"type":"integer","description":"Number of hunks that failed to apply"},"failed_hunks":{"type":"array","description":"Details of each failed hunk","items":{"type":"object","properties":{"hunk_index":{"type":"integer"},"old_start":{"type":"integer"},"old_count":{"type":"integer"},"expected_context":{"type":"array","items":{"type":"string"}},"actual_context":{"type":"array","items":{"type":"string"}},"reason":{"type":"string"}}}},"affected_line_ranges":{"type":"array","description":"Line ranges affected by successful hunks","items":{"type":"object","properties":{"start":{"type":"integer"},"end":{"type":"integer"}}}},"newline_style_before":{"type":"string","description":"Newline style in original text"},"newline_style_after":{"type":"string","description":"Newline style in result text"},"result_fingerprint":{"type":"string","description":"SHA-256 of the result text"},"result_text":{"type":["string","null"],"description":"Resulting text if requested"},"findings":{"type":"array","items":{"type":"string"},"description":"Analysis notes and warnings"}}}));
    m.insert("patch_summary", serde_json::json!({"type":"object","properties":{"files_changed":{"type":"integer","description":"Number of files changed"},"hunks_total":{"type":"integer","description":"Total number of hunks across all files"},"additions":{"type":"integer","description":"Total number of added lines"},"deletions":{"type":"integer","description":"Total number of deleted lines"},"renames_detected":{"type":"array","description":"Detected file renames","items":{"type":"object","properties":{"from":{"type":"string"},"to":{"type":"string"}}}},"binary_patch_detected":{"type":"boolean","description":"True if binary patch content detected"},"line_ranges_by_file":{"type":"object","description":"Line ranges affected per file","additionalProperties":{"type":"array","items":{"type":"object","properties":{"start":{"type":"integer"},"end":{"type":"integer"}}}}},"findings":{"type":"array","items":{"type":"string"},"description":"Analysis notes and warnings"}}}));
    m.insert("unicode_policy_check", serde_json::json!({"type":"object","properties":{"pass_":{"type":"boolean","description":"True if text passes the policy (no errors)"},"policy":{"type":"string","description":"Policy name that was applied"},"normalized_form":{"type":"string","description":"Text after normalization"},"findings":{"type":"array","description":"Policy findings with rule, severity, and message","items":{"type":"object","properties":{"rule":{"type":"string"},"severity":{"type":"string"},"message":{"type":"string"}}}},"summary":{"type":"string","description":"Human-readable summary"}}}));
    m.insert("canonicalize_text", serde_json::json!({"type":"object","properties":{"text":{"type":"string","description":"Canonicalized text"},"changed":{"type":"boolean","description":"True if text was modified"},"operations_applied":{"type":"array","description":"List of operations applied"},"fingerprint_before":{"type":"string","description":"SHA-256 of original text"},"fingerprint_after":{"type":"string","description":"SHA-256 of canonicalized text"},"mapping":{"type":"array","description":"Character mapping if return_mapping was True"},"findings":{"type":"array","items":{"type":"string"},"description":"Analysis notes and warnings"}}}));
    m.insert("identifier_table_inspect", serde_json::json!({"type":"object","properties":{"count":{"type":"integer","description":"Number of identifiers inspected"},"collisions":{"type":"array","description":"Detected collisions","items":{"type":"object","properties":{"kind":{"type":"string"},"names":{"type":"array","items":{"type":"string"}},"detail":{"type":"string"}}}},"reserved_keyword_hits":{"type":"array","description":"Identifiers matching reserved keywords","items":{"type":"object","properties":{"name":{"type":"string"},"language":{"type":"string"},"file":{"type":"string"},"line":{"type":"integer"}}}},"mixed_style_groups":{"type":"array","description":"Groups with mixed naming styles","items":{"type":"object","properties":{"stripped":{"type":"string"},"names":{"type":"array","items":{"type":"string"}},"styles":{"type":"array","items":{"type":"string"}}}}},"findings":{"type":"array","items":{"type":"string"}}}}));
    m.insert("version_constraint_check", serde_json::json!({"type":"object","properties":{"satisfies":{"type":"boolean","description":"Whether the version satisfies the constraint"},"parsed_version":{"type":"object","description":"Parsed version components"},"parsed_constraint":{"type":"object","description":"Parsed constraint components"},"scheme":{"type":"string","description":"Versioning scheme used"},"explanation":{"type":"string","description":"Human-readable explanation"},"findings":{"type":"array","items":{"type":"string"},"description":"Analysis notes and warnings"}}}));
    m.insert("cargo_toml_inspect", serde_json::json!({"type":"object","properties":{"parse_ok":{"type":"boolean","description":"Whether TOML parsed successfully"},"package":{"type":"object","description":"Package metadata from [package] section","properties":{"name":{"type":"string"},"version":{"type":"string"},"edition":{"type":"string"},"license":{"type":"string"},"repository":{"type":"string"},"readme":{"type":"string"}}},"workspace":{"type":"object","description":"Workspace section information","properties":{"present":{"type":"boolean"},"members":{"type":"array","items":{"type":"string"}},"exclude":{"type":"array","items":{"type":"string"}}}},"dependencies":{"type":"object","description":"Dependencies by section"},"path_dependencies":{"type":"array","items":{"type":"string"},"description":"Extracted path dependency values"},"suspicious_dependency_names":{"type":"array","items":{"type":"string"},"description":"Dependency names with suspicious patterns"},"duplicate_or_confusable_dependency_names":{"type":"array","items":{"type":"string"},"description":"Dependency names that normalize to the same form"},"findings":{"type":"array","items":{"type":"string"},"description":"Structural findings and warnings"}}}));
    m.insert("prompt_input_inspect", serde_json::json!({"type":"object","properties":{"findings":{"type":"array","description":"Structured findings with code, severity, message, span, and details","items":{"type":"object","properties":{"code":{"type":"string"},"severity":{"type":"string"},"message":{"type":"string"},"span":{"type":"object"},"details":{"type":"object"}}}},"summary":{"type":"string","description":"Human-readable summary"},"risk_score":{"type":"integer","description":"Deterministic risk score"},"recommended_next_tool":{"type":["string","array"],"description":"Recommended follow-up tool(s)"},"text_length":{"type":"integer","description":"Input text length"},"checks_run":{"type":"array","items":{"type":"string"},"description":"Checks that were executed"},"findings_truncated":{"type":"boolean","description":"True if findings were truncated due to limits"}}}));
    m.insert("text_security_inspect", serde_json::json!({"type":"object","properties":{"verdict":{"type":"string","enum":["allow","review","block"]},"policy":{"type":"string"},"findings":{"type":"array"},"machine_code":{"type":"string"},"normalized_changed":{"type":"boolean"},"recommended_action":{"type":"string"},"summary":{"type":"string"},"subresults":{"type":"object"}}}));
    m.insert("edit_preflight", serde_json::json!({"type":"object","properties":{"ok_to_apply":{"type":"boolean"},"mode":{"type":"string"},"findings":{"type":"array"},"machine_code":{"type":"string"},"recommended_next_tool":{"type":["string","null"]},"summary":{"type":"string"},"subresults":{"type":"object"}}}));
    m.insert("command_preflight", serde_json::json!({"type":"object","properties":{"verdict":{"type":"string","enum":["allow","review","block"]},"command":{"type":"string"},"platform":{"type":"string"},"policy":{"type":"string"},"findings":{"type":"array"},"machine_code":{"type":"string"},"summary":{"type":"string"},"subresults":{"type":"object"}}}));
    m.insert("config_preflight", serde_json::json!({"type":"object","properties":{"valid":{"type":"boolean"},"verdict":{"type":"string","enum":["valid","valid_with_warnings","invalid"]},"format":{"type":"string"},"findings":{"type":"array"},"machine_code":{"type":"string"},"summary":{"type":"string"},"subresults":{"type":"object"}}}));
    m.insert("structured_data_compare", serde_json::json!({"type":"object","properties":{"equal":{"type":"boolean"},"valid_a":{"type":"boolean"},"valid_b":{"type":"boolean"},"findings":{"type":"array"},"machine_code":{"type":"string"},"summary":{"type":"string"},"subresults":{"type":"object"}}}));
    m
});

fn list_tools_raw() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "math_eval".to_string(),
            description: "Evaluate arithmetic, unit conversions, constants, and scientific expressions deterministically. State-mutating functions (setvar, store, etc.) and non-deterministic functions (random, randint, gauss, etc.) are disabled. Use for math and unit tasks instead of asking the model to calculate.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "expression": {"type": "string", "description": "Math expression to evaluate (e.g., '5 + 3', '30m + 100ft', 'five plus three')", "maxLength": 10000}
                },
                "required": ["expression"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "unit_convert".to_string(),
            description: "Convert a numeric value from one unit to another using pre-defined conversion factors.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "value": {"type": "number", "description": "Numeric value to convert (must be finite; NaN and infinity are rejected)"},
                    "from_unit": {"type": "string", "description": "Source unit (e.g., 'km', 'ft', 'kg')"},
                    "to_unit": {"type": "string", "description": "Target unit (e.g., 'm', 'in', 'lb')"}
                },
                "required": ["value", "from_unit", "to_unit"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "unit_info".to_string(),
            description: "Get information about a unit including its canonical form and category.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {"unit": {"type": "string", "description": "Unit name or alias (e.g., 'km', 'kilogram', '℃')"}},
                "required": ["unit"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "constant_lookup".to_string(),
            description: "Look up physical constant values and symbols (Avogadro, Planck, speed of light, etc.).".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {"name": {"type": "string", "description": "Constant name (e.g., 'avogadro', 'planck', 'c', 'G')"}},
                "required": ["name"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "text_measure".to_string(),
            description: "Measure exact text properties: UTF-8 byte length, codepoint count, words, lines, whitespace, newline style, Unicode normalization state, invisibles, and mixed-script signals.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "Input string to measure"},
                    "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "normal", "description": "Detail level for output"}
                },
                "required": ["text"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "text_equal".to_string(),
            description: "Compare two strings under raw, Unicode-normalized, casefolded, or trimmed modes and report exact equality evidence.".to_string(),
            input_schema: serde_json::json!({
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
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "text_diff_explain".to_string(),
            description: "Explain why two strings differ, including spans, codepoints, Unicode names, normalization equivalence, confusables, invisibles, and agent-facing classification.".to_string(),
            input_schema: serde_json::json!({
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
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "text_inspect".to_string(),
            description: "Inspect a string for hidden characters, Unicode confusables, mixed scripts, normalization state, and display-safe representation. Can report both original and normalized text analysis.".to_string(),
            input_schema: serde_json::json!({
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
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "text_count".to_string(),
            description: "Count exact characters or produce a character frequency table with codepoint positions, grapheme clusters, bytes, or substring matches.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "Input string"},
                    "target": {"type": ["string", "null"], "default": null, "description": "Single character to count (None for frequency table)"},
                    "count_mode": {"type": "string", "enum": ["codepoint", "grapheme", "byte", "substring"], "default": "codepoint", "description": "Count mode: codepoint (Python str), grapheme (user-perceived), byte (UTF-8), substring"},
                    "normalization": {"type": "string", "enum": ["raw", "NFC", "NFKC"], "default": "raw", "description": "Unicode normalization form"}
                },
                "required": ["text"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "text_truncate".to_string(),
            description: "Truncate a string to a specified number of grapheme clusters (user-perceived characters). Preserves emoji, combining sequences, and flag sequences intact. Useful for AI agent prompts where visual length matters.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "Input string to truncate"},
                    "max_graphemes": {"type": "integer", "minimum": 0, "maximum": 1000000, "description": "Maximum number of grapheme clusters to return"}
                },
                "required": ["text", "max_graphemes"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "text_transform".to_string(),
            description: "Apply deterministic text transformations: Unicode normalization (NFC/NFD/NFKC/NFKD), casefold, trim, newline normalization, zero-width removal, bidi control stripping, and visible representation.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "Input string to transform"},
                    "operations": {"type": "array", "items": {"type": "string"}, "description": "Operations to apply: normalize_nfc, normalize_nfd, normalize_nfkc, normalize_nfkd, casefold, trim, trim_trailing_whitespace, normalize_newlines_lf, ensure_final_newline, strip_final_newline, remove_zero_width, remove_bidi_controls, visible_repr", "maxItems": 100},
                    "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "normal"}
                },
                "required": ["text", "operations"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "validate_brackets".to_string(),
            description: "Check whether delimiters are structurally balanced and report unmatched delimiters with line/column positions.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "Input string"},
                    "pairs": {"type": "object", "description": "Bracket pair mapping (default: () [] {} <>)"}
                },
                "required": ["text"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "validate_json".to_string(),
            description: "Validate JSON and report precise parse errors or top-level structure information.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {"text": {"type": "string", "description": "Input string to validate as JSON"}},
                "required": ["text"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "validate_regex".to_string(),
            description: "Test a Python regular expression against sample strings and report match/fullmatch status, spans, groups, and errors.".to_string(),
            input_schema: serde_json::json!({
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
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "list_compare".to_string(),
            description: "Compare two lists with explicit modes: ordered ( LCS-based alignment), set (presence only), multiset (count deltas). Near matches are optional and never replace exact missing/extra results.".to_string(),
            input_schema: serde_json::json!({
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
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "validate_toml".to_string(),
            description: "Validate TOML configuration files (Cargo.toml, pyproject.toml, etc.) and report parse errors with line/column positions.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "TOML document string to validate"},
                    "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "normal"}
                },
                "required": ["text"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "json_extract".to_string(),
            description: "Extract a value from JSON using RFC 6901 JSON Pointer (e.g., /foo/bar/0). Navigate nested objects and arrays.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "JSON document string"},
                    "pointer": {"type": "string", "default": "", "description": "RFC 6901 JSON Pointer path (e.g., /dependencies/tokio)"},
                    "max_output_chars": {"type": "integer", "default": 4000},
                    "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "normal"}
                },
                "required": ["text"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "json_compare".to_string(),
            description: "Compare two JSON documents semantically, ignoring formatting and key order.".to_string(),
            input_schema: serde_json::json!({
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
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "text_position".to_string(),
            description: "Convert between byte offsets, codepoint indices, line/column positions, and UTF-16 offsets.".to_string(),
            input_schema: serde_json::json!({
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
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "text_hash".to_string(),
            description: "Compute cryptographic hashes of text for identity checking.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string"},
                    "algorithms": {"type": "array", "items": {"type": "string"}, "default": ["sha256"], "description": "Hash algorithms (sha256, sha1, md5, crc32)", "maxItems": 10},
                    "encoding": {"type": "string", "default": "utf-8"},
                    "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "normal"}
                },
                "required": ["text"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "escape_text".to_string(),
            description: "Escape text for various output formats.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string"},
                    "mode": {"type": "string", "enum": ["json_string", "python_string", "rust_string", "posix_shell_single", "regex_literal", "markdown_inline_code", "markdown_code_block", "html_text", "url_component"]},
                    "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "normal"}
                },
                "required": ["text", "mode"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "unescape_text".to_string(),
            description: "Unescape text from various formats.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string"},
                    "mode": {"type": "string", "enum": ["json_string", "python_string", "unicode_escape", "url_component"]},
                    "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "normal"}
                },
                "required": ["text", "mode"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "identifier_analyze".to_string(),
            description: "Classify and validate identifier naming conventions across languages.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string"},
                    "languages": {"type": "array", "items": {"type": "string"}, "default": ["python", "rust", "javascript", "env"], "description": "Languages to check (python, rust, javascript, env)"},
                    "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "normal"}
                },
                "required": ["text"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "regex_finditer".to_string(),
            description: "Find all regex matches in text with positions, line/column info, and capture groups.".to_string(),
            input_schema: serde_json::json!({
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
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "regex_safety_check".to_string(),
            description: "Heuristic check for potential catastrophic backtracking risks in regex patterns. Flags nested quantifiers, repeated alternations, ambiguous dot-star, and backreferences.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string", "description": "Regular expression pattern to check"}
                },
                "required": ["pattern"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "validate_schema_light".to_string(),
            description: "Validate JSON against a simple schema format with type, required, enum, pattern, and nested constraints.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "JSON document to validate"},
                    "schema": {"type": "object", "description": "Schema to validate against"},
                    "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "normal"}
                },
                "required": ["text", "schema"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "path_normalize".to_string(),
            description: "Normalize a path using posixpath or ntpath semantics. Collapse dot segments, resolve components.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Path string to normalize"},
                    "platform": {"type": "string", "enum": ["posix", "windows"], "default": "posix", "description": "Platform semantics to use"},
                    "collapse_dot_segments": {"type": "boolean", "default": true, "description": "Collapse dot and dot-dot segments"},
                    "preserve_trailing_separator": {"type": "boolean", "default": false, "description": "Preserve trailing separator"}
                },
                "required": ["path"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "path_analyze".to_string(),
            description: "Analyze path components, extensions, hidden status, and traversal without filesystem access.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "style": {"type": "string", "enum": ["auto", "posix", "windows"], "default": "auto"},
                    "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "normal"}
                },
                "required": ["path"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "path_compare".to_string(),
            description: "Compare two paths under explicit normalization rules: separator normalization, dot-segment collapsing, and optional case-insensitive comparison.".to_string(),
            input_schema: serde_json::json!({
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
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "path_scope_check".to_string(),
            description: "Determine whether a target path remains lexically inside a declared root. Lexical only, does not resolve symlinks.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "root": {"type": "string", "description": "Root directory path"},
                    "target": {"type": "string", "description": "Target path to check"},
                    "platform": {"type": "string", "enum": ["posix", "windows"], "default": "posix", "description": "Platform semantics"},
                    "case_sensitive": {"type": "boolean", "default": true, "description": "Case-sensitive comparison"}
                },
                "required": ["root", "target"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "json_shape".to_string(),
            description: "Analyze the structure of a JSON document without returning values. Shows type, keys, and nested structure with configurable depth limits.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "JSON document string to analyze"},
                    "max_depth": {"type": "integer", "default": 4, "description": "Maximum depth for nested structure"},
                    "max_keys": {"type": "integer", "default": 100, "description": "Maximum keys to show per object"},
                    "max_array_items": {"type": "integer", "default": 5, "description": "Maximum array item previews"}
                },
                "required": ["text"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "text_window".to_string(),
            description: "Get a window around a position in text with context lines. Shows line at position with surrounding context, position metrics, and character details.".to_string(),
            input_schema: serde_json::json!({
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
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "json_canonicalize".to_string(),
            description: "Canonicalize JSON with deterministic formatting, key ordering, duplicate key detection, and stable hashes.".to_string(),
            input_schema: serde_json::json!({
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
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "json_query".to_string(),
            description: "Extract a value from JSON using RFC 6901 JSON Pointer. Navigate nested objects and arrays. Deprecated: use json_extract instead, which provides richer output including available_keys, missing_at, and detail levels.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "JSON document string"},
                    "pointer": {"type": "string", "default": "", "description": "RFC 6901 JSON Pointer path (e.g., /foo/bar/0)"}
                },
                "required": ["text"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: Some(true),
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "glob_match".to_string(),
            description: "Match a glob pattern against a path with explicit semantics: * matches within one segment, ** matches zero or more segments, ? matches one char. Python fnmatch limitations around ** are documented.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string", "description": "Glob pattern to match (e.g., src/**/*.rs)"},
                    "path": {"type": "string", "description": "Path string to match against"},
                    "platform": {"type": "string", "enum": ["posix", "windows"], "default": "posix", "description": "Path platform"},
                    "case_sensitive": {"type": "boolean", "default": true, "description": "Case-sensitive matching"}
                },
                "required": ["pattern", "path"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "text_fingerprint".to_string(),
            description: "Compute a deterministic SHA-256 fingerprint of text with canonicalization options for Unicode normalization, newline style, casefold, and final newline trimming.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "Input string to fingerprint"},
                    "unicode": {"type": "string", "enum": ["raw", "NFC", "NFD", "NFKC", "NFKD"], "default": "raw", "description": "Unicode normalization form"},
                    "newline": {"type": "string", "enum": ["raw", "LF"], "default": "raw", "description": "Newline normalization"},
                    "trim_final_newline": {"type": "boolean", "default": false, "description": "Remove trailing newline before hashing"},
                    "casefold": {"type": "boolean", "default": false, "description": "Apply casefolding before hashing"}
                },
                "required": ["text"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "identifier_inspect".to_string(),
            description: "Inspect identifiers for validity and collisions. Detects confusables, mixed scripts, normalization issues, and casefold collisions across a list of identifiers.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "identifiers": {"type": "array", "items": {"type": "string"}, "description": "List of identifier strings to inspect", "maxItems": 10000},
                    "language": {"type": "string", "enum": ["generic", "python", "rust", "javascript", "typescript", "json_key"], "default": "generic", "description": "Language for validation"},
                    "normalization": {"type": "string", "enum": ["raw", "NFC", "NFD", "NFKC", "NFKD"], "default": "NFC", "description": "Unicode normalization form"},
                    "casefold": {"type": "boolean", "default": false, "description": "Apply casefolding for collision detection"},
                    "check_confusables": {"type": "boolean", "default": true, "description": "Check for confusable characters"}
                },
                "required": ["identifiers"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "version_compare".to_string(),
            description: "Compare two version strings with explicit scheme. Supports semver (major.minor.patch), loose (numeric parts), and deferred pep440.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "a": {"type": "string", "description": "First version string"},
                    "b": {"type": "string", "description": "Second version string"},
                    "scheme": {"type": "string", "enum": ["semver", "pep440", "loose"], "default": "semver", "description": "Version scheme"}
                },
                "required": ["a", "b"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "toml_shape".to_string(),
            description: "Analyze the structure of a TOML document: top-level keys, tables, and nesting hierarchy.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "TOML document string"},
                    "max_tables": {"type": "integer", "default": 100, "minimum": 1, "maximum": 100000, "description": "Maximum tables to return"},
                    "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "normal"}
                },
                "required": ["text"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "list_dedupe".to_string(),
            description: "Remove duplicates from a list while preserving order. Supports Unicode normalization and casefolding.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "items": {"type": "array", "items": {"type": "string"}, "description": "List of strings to dedupe", "maxItems": 10000},
                    "normalization": {"type": "string", "enum": ["raw", "NFC", "NFD", "NFKC", "NFKD"], "default": "NFC"},
                    "casefold": {"type": "boolean", "default": false, "description": "Apply casefolding before comparison"},
                    "stable": {"type": "boolean", "default": true, "description": "Preserve first occurrence order"}
                },
                "required": ["items"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "list_sort".to_string(),
            description: "Sort a list of strings with Unicode normalization and casefold support.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "items": {"type": "array", "items": {"type": "string"}, "description": "List of strings to sort", "maxItems": 10000},
                    "normalization": {"type": "string", "enum": ["raw", "NFC", "NFD", "NFKC", "NFKD"], "default": "NFC"},
                    "casefold": {"type": "boolean", "default": false, "description": "Apply casefolding for sorting"},
                    "reverse": {"type": "boolean", "default": false, "description": "Sort in descending order"},
                    "stable": {"type": "boolean", "default": true, "description": "Preserve original order for equal elements"}
                },
                "required": ["items"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "text_replace_check".to_string(),
            description: "Check whether a text replacement would apply cleanly before an agent attempts to edit. Reports match count, positions, ambiguity, and optional preview of before/after.".to_string(),
            input_schema: serde_json::json!({
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
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "line_range_extract".to_string(),
            description: "Extract exact line ranges from text and return stable offsets, byte positions, line counts, and optional fingerprint.".to_string(),
            input_schema: serde_json::json!({
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
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "line_range_compare".to_string(),
            description: "Compare a line range from two text inputs with exact, trailing-whitespace-ignoring, or newline-normalizing comparison.".to_string(),
            input_schema: serde_json::json!({
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
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "shell_split".to_string(),
            description: "Parse a shell-like command string into argv tokens and report risky lexical features (pipes, redirections, command substitution, variable expansion, globs, control operators). Lexical POSIX-like parsing only, not full shell evaluation.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string", "description": "The shell command string to parse"},
                    "shell": {"type": "string", "enum": ["posix"], "default": "posix", "description": "Shell dialect (only posix is supported)"},
                    "detect_risky_features": {"type": "boolean", "default": true, "description": "Whether to detect risky lexical features"}
                },
                "required": ["command"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "shell_quote_join".to_string(),
            description: "Safely quote a list of argv tokens into a POSIX-like shell string. Verifies round-trip safety with shell_split.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "argv": {"type": "array", "items": {"type": "string"}, "description": "List of argument strings to join", "maxItems": 10000},
                    "shell": {"type": "string", "enum": ["posix"], "default": "posix", "description": "Shell dialect (only posix is supported)"}
                },
                "required": ["argv"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "argv_compare".to_string(),
            description: "Compare two command strings or argv lists by parsed argv tokens rather than raw text. Supports command strings, pre-parsed argv lists, or both.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "left_command": {"type": "string", "description": "Left command string to parse and compare"},
                    "right_command": {"type": "string", "description": "Right command string to parse and compare"},
                    "left_argv": {"type": "array", "items": {"type": "string"}, "description": "Left pre-parsed argv list", "maxItems": 10000},
                    "right_argv": {"type": "array", "items": {"type": "string"}, "description": "Right pre-parsed argv list", "maxItems": 10000},
                    "shell": {"type": "string", "enum": ["posix"], "default": "posix", "description": "Shell dialect (only posix is supported)"}
                },
                "required": []
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "markdown_structure".to_string(),
            description: "Parse Markdown structure with a deterministic line scanner: headings (level, text, slug), code fences (language, open/close state), links (visible vs target mismatch), HTML comments, frontmatter detection, and table detection. Not a full CommonMark parser.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "Markdown text to analyze"},
                    "include_sections": {"type": "boolean", "default": true, "description": "Include heading detection"},
                    "include_links": {"type": "boolean", "default": true, "description": "Include link detection"},
                    "include_code_fences": {"type": "boolean", "default": true, "description": "Include code fence detection"},
                    "include_html_comments": {"type": "boolean", "default": true, "description": "Include HTML comment detection"}
                },
                "required": ["text"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "code_fence_extract".to_string(),
            description: "Extract fenced code blocks from Markdown with exact line ranges, optional language filter, content, and SHA-256 fingerprints. Reports unclosed fences.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "Markdown text to scan"},
                    "language": {"type": "string", "description": "Optional language filter (case-insensitive)"},
                    "include_content": {"type": "boolean", "default": true, "description": "Include block content in output"}
                },
                "required": ["text"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "dotenv_validate".to_string(),
            description: "Validate .env-style key=value configuration text. Detects invalid keys, duplicate keys, missing quotes, and variable expansion syntax. Line-by-line parser, no shell evaluation.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": ".env file content to validate"},
                    "allow_export": {"type": "boolean", "default": true, "description": "Allow export KEY=VALUE syntax"},
                    "key_pattern": {"type": "string", "default": "^[A-Za-z_][A-Za-z0-9_]*$", "description": "Regex pattern keys must match"},
                    "duplicate_policy": {"type": "string", "enum": ["warn", "error", "allow"], "default": "warn", "description": "How to handle duplicate keys"}
                },
                "required": ["text"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "ini_validate".to_string(),
            description: "Validate simple INI-style configuration files. Supports [section] headers, key=value and key:value lines, comments. Detects duplicate sections, duplicate keys, and malformed lines.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "INI file content to validate"},
                    "duplicate_policy": {"type": "string", "enum": ["warn", "error", "allow"], "default": "warn", "description": "How to handle duplicate keys/sections"}
                },
                "required": ["text"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "patch_apply_check".to_string(),
            description: "Validate and simulate a unified diff against provided in-memory files/text without touching the filesystem. Reports parse status, application success, failed hunks with context, and optional result fingerprint.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "original_text": {"type": "string", "description": "The original source text to apply the patch to"},
                    "patch_text": {"type": "string", "description": "The unified diff patch text"},
                    "strict": {"type": "boolean", "default": true, "description": "If True, context lines must match exactly"},
                    "return_result_fingerprint": {"type": "boolean", "default": true, "description": "If True, compute SHA-256 fingerprint of the result"},
                    "return_result_text": {"type": "boolean", "default": false, "description": "If True, include the resulting text (bounded to 50000 chars)"}
                },
                "required": ["original_text", "patch_text"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "patch_summary".to_string(),
            description: "Summarize a unified diff without applying it. Reports file counts, hunk counts, additions, deletions, renames, and line ranges by file.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {"patch_text": {"type": "string", "description": "The unified diff text to summarize"}},
                "required": ["patch_text"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "unicode_policy_check".to_string(),
            description: "Apply a named deterministic Unicode safety policy to input text. Policies include identifier_strict (mixed scripts, bidi, confusables), filename_safe (control chars, path separators, reserved names), source_code, human_text (warn-only), json_key, and domain_like.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "Input text to check"},
                    "policy": {"type": "string", "enum": ["identifier_strict", "filename_safe", "source_code", "human_text", "json_key", "domain_like"], "description": "Policy to apply"},
                    "normalization": {"type": "string", "enum": ["raw", "NFC", "NFD", "NFKC", "NFKD"], "description": "Normalization form (default: policy-specific)"}
                },
                "required": ["text", "policy"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "canonicalize_text".to_string(),
            description: "Apply a named text canonicalization profile. Profiles include source_file_identity (NFC + LF + newline), identifier_compare (NFC + casefold), human_label_compare (NFC + casefold + whitespace collapse), json_key_compare (NFC + casefold), and path_segment_compare (NFC + lowercase + LF).".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "Input text to canonicalize"},
                    "profile": {"type": "string", "enum": ["source_file_identity", "identifier_compare", "human_label_compare", "json_key_compare", "path_segment_compare"], "description": "Canonicalization profile to apply"},
                    "return_mapping": {"type": "boolean", "default": false, "description": "If True, include a character mapping of changes"}
                },
                "required": ["text", "profile"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "identifier_table_inspect".to_string(),
            description: "Inspect a table of identifiers for casefold collisions, normalization collisions, confusable/near-collisions, style variants, reserved keyword hits, and mixed naming style groups. Accepts structured entries with name, kind, file, and line metadata.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "identifiers": {"type": "array", "items": {"type": "object", "properties": {"name": {"type": "string", "description": "Identifier name (required)"}, "kind": {"type": "string", "description": "Optional kind/category"}, "file": {"type": "string", "description": "Source file path"}, "line": {"type": "integer", "description": "Line number"}}, "required": ["name"]}, "description": "List of identifier entries to inspect", "maxItems": 10000},
                    "language": {"type": "string", "enum": ["generic", "python", "rust", "javascript", "typescript", "json_key"], "default": "python", "description": "Target language for reserved keyword checking"},
                    "checks": {"type": "array", "items": {"type": "string"}, "description": "Subset of checks: casefold, normalization, confusable, style, reserved, mixed_style"}
                },
                "required": ["identifiers"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "version_constraint_check".to_string(),
            description: "Check whether a version satisfies a constraint under a declared versioning scheme (semver or cargo). Supports comparison operators, caret, tilde, wildcard, range, and comma-separated constraints.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "version": {"type": "string", "description": "Version string to check (e.g., '1.2.3', '0.5.0-beta.1')"},
                    "constraint": {"type": "string", "description": "Version constraint (e.g., '>=1.0,<2.0', '^1.2.3', '~0.5', '1.*')"},
                    "scheme": {"type": "string", "enum": ["semver", "cargo"], "default": "semver", "description": "Versioning scheme to use for parsing and evaluation"}
                },
                "required": ["version", "constraint"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "cargo_toml_inspect".to_string(),
            description: "Inspect Cargo.toml text without network or filesystem access. Reports package metadata, workspace configuration, dependency forms (version/path/git/workspace), path dependencies, suspicious or confusable dependency names, and structural findings.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "The Cargo.toml content to inspect"},
                    "check_workspace": {"type": "boolean", "default": true, "description": "Whether to analyze [workspace] section"},
                    "check_dependencies": {"type": "boolean", "default": true, "description": "Whether to analyze dependency sections"}
                },
                "required": ["text"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "prompt_input_inspect".to_string(),
            description: "Deterministically inspect text for red flags that may influence agents or humans unexpectedly. Detects hidden Unicode characters, bidirectional controls, HTML comments, Markdown link mismatches, ANSI escapes, terminal controls, base64-like blobs, instruction-like phrases, and very long minified lines. This is NOT a prompt-injection detector -- it reports observable features only, not intent.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "The text to inspect for red flags"},
                    "checks": {"type": "array", "items": {"type": "string"}, "description": "Subset of checks to run: unicode_hidden, bidi, html_comments, markdown_links, ansi_escapes, terminal_controls, base64_like_blobs, instruction_phrases, long_minified_lines"},
                    "phrase_patterns": {"type": ["array", "null"], "items": {"type": "string"}, "description": "Optional literal strings or safe regexes to detect as instruction-like phrases. Pass null for no custom patterns."}
                },
                "required": ["text"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "text_security_inspect".to_string(),
            description: "Composite security-oriented text hygiene pass. Runs text_inspect, unicode_policy_check, canonicalize_text, prompt_input_inspect, and identifier_inspect depending on policy. Returns a verdict (allow/review/block) plus structured findings and machine codes.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "Input text to inspect"},
                    "policy": {"type": "string", "enum": ["default", "source_code", "prompt", "markdown", "identifier"], "default": "default", "description": "Security policy to apply"},
                    "normalize": {"type": "string", "enum": ["none", "NFC", "NFD", "NFKC", "NFKD"], "default": "none", "description": "Normalization form to analyze"},
                    "compare_normalized": {"type": "boolean", "default": false, "description": "Report both original and normalized analysis"},
                    "detail": {"type": "string", "enum": ["summary", "normal", "full"], "default": "summary", "description": "Detail level: summary (compact verdict only), normal, or full (includes subresults)"}
                },
                "required": ["text"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "edit_preflight".to_string(),
            description: "Composite: validate a proposed edit before applying it. Calls text_replace_check, patch_apply_check, line_range_extract, text_fingerprint, and text_diff_explain as needed. Returns ok_to_apply verdict with findings and machine codes.".to_string(),
            input_schema: serde_json::json!({
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
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "command_preflight".to_string(),
            description: "Composite: analyze a command before user approval or execution. Calls shell_split and regex_safety_check. Returns parsed argv, shell operators, risk findings, and a verdict. Must not execute anything.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string", "description": "Command string to analyze"},
                    "platform": {"type": "string", "enum": ["posix", "windows", "auto"], "default": "posix", "description": "Target platform"},
                    "policy": {"type": "string", "enum": ["default", "strict", "permissive"], "default": "default", "description": "Analysis policy"},
                    "working_directory": {"type": "string", "description": "Working directory context (informational)"}
                },
                "required": ["command"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "config_preflight".to_string(),
            description: "Composite: validate generated config text. Auto-detects format and runs the appropriate validator. Returns valid/invalid, detected format, parse error location, and machine code.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "Config text to validate"},
                    "format": {"type": "string", "enum": ["auto", "json", "toml", "dotenv", "ini", "cargo_toml"], "default": "auto", "description": "Config format (auto-detect if not specified)"},
                    "schema": {"type": "object", "description": "Optional JSON schema for validation"},
                    "strict": {"type": "boolean", "default": false, "description": "Strict validation mode"}
                },
                "required": ["text"]
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
        ToolDefinition {
            name: "structured_data_compare".to_string(),
            description: "Composite: compare structured config/data output. Calls json_compare, json_canonicalize, and json_shape. Returns equal/not-equal verdict with structured diffs.".to_string(),
            input_schema: serde_json::json!({
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
            }),
            output_schema: None, tier: None, tags: None, deprecated: None,
            category: None, llm_exposure: None, cost: None,
        },
    ]
}

fn list_tools() -> Vec<ToolDefinition> {
    list_tools_raw().into_iter().map(enrich_tool).collect()
}

fn compact_input_schema(schema: &Value) -> Value {
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

    // Compact each property: keep only whitelist of keys (matching Python)
    if let Some(props) = obj.get("properties").and_then(|v| v.as_object()) {
        let mut compact_props = serde_json::Map::new();
        for (prop_name, prop_def) in props {
            if let Some(prop_obj) = prop_def.as_object() {
                let mut cp = serde_json::Map::new();
                // Keep type
                if let Some(t) = prop_obj.get("type") {
                    cp.insert("type".to_string(), t.clone());
                }
                // Keep enum
                if let Some(e) = prop_obj.get("enum") {
                    cp.insert("enum".to_string(), e.clone());
                }
                // Keep required sub-fields
                if let Some(r) = prop_obj.get("required") {
                    cp.insert("required".to_string(), r.clone());
                }
                // Keep items for arrays
                if let Some(items) = prop_obj.get("items") {
                    cp.insert("items".to_string(), items.clone());
                }
                // Keep numeric constraints
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
                // Truncated description
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

    // Keep required at top level
    if let Some(req) = obj.get("required") {
        compact.insert("required".to_string(), req.clone());
    }

    Value::Object(compact)
}

fn compact_output_schema(schema: &Value) -> Value {
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

#[derive(Serialize, Debug, Clone)]
pub struct ToolMetadata {
    pub category: &'static str,
    pub tier: u8,
    pub profiles: &'static [&'static str],
    pub tags: &'static [&'static str],
    pub llm_exposure: &'static str,
    pub harness_use: &'static [&'static str],
    pub aliases: &'static [&'static str],
    pub cost: &'static str,
    pub stability: &'static str,
    pub composite: bool,
}

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

fn find_close_match<'a>(input: &str, tool_names: &[&'a str]) -> Option<&'a str> {
    if input.len() > 200 {
        return None;
    }
    let lower_input = input.to_lowercase();

    // First check for exact case-insensitive match
    for &name in tool_names {
        if name.to_lowercase() == lower_input {
            return Some(name);
        }
    }

    // Check for word boundary matches (both directions, like Python)
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
            // Python returns the shortest tool name when there are ties
            if best_boundary.is_none() || name.len() < best_boundary.unwrap().0.len() {
                best_boundary = Some((name, 0));
            }
        }
    }
    if let Some((name, _)) = best_boundary {
        return Some(name);
    }

    // Compute edit distance with threshold
    let mut best: Option<(&str, usize)> = None;
    for &name in tool_names {
        let dist = levenshtein_distance(input, name);
        let threshold = input.chars().count().min(name.chars().count()) / 2;
        if dist <= threshold {
            if best.map_or(true, |(_, best_dist)| dist < best_dist) {
                best = Some((name, dist));
            }
        }
    }

    best.map(|(name, _)| name)
}

static SCHEMA_CACHE: LazyLock<HashMap<String, Value>> = LazyLock::new(|| {
    let tools = list_tools();
    let mut map = HashMap::new();
    for tool in tools {
        map.insert(tool.name, tool.input_schema);
    }
    map
});

fn validate_property(value: &Value, schema: &Value, path: &str) -> Option<String> {
    validate_property_inner(value, schema, path, 10)
}

fn validate_property_inner(
    value: &Value,
    schema: &Value,
    path: &str,
    max_depth: usize,
) -> Option<String> {
    if max_depth == 0 {
        return Some(format!("Schema nesting too deep at '{}'", path));
    }

    let obj = match schema.as_object() {
        Some(o) => o,
        None => return Some(format!("Schema for '{}' must be an object", path)),
    };

    let expected_type = match obj.get("type") {
        Some(t) => t,
        None => return None,
    };

    let type_options: Vec<&str> = match expected_type {
        Value::String(s) => vec![s.as_str()],
        Value::Array(arr) => arr.iter().filter_map(|v| v.as_str()).collect(),
        _ => {
            return Some(format!(
                "Argument '{}' has unsupported 'type' (must be a string or list of strings)",
                path
            ))
        }
    };

    let valid_type = type_options.iter().any(|t| value_matches_type(value, t));
    if !valid_type {
        if type_options.len() == 1 {
            return Some(format!(
                "Argument '{}' must be {}, got {}",
                path,
                type_options[0],
                json_type_name(value)
            ));
        }
        return Some(format!(
            "Argument '{}' must be one of [{}], got {}",
            path,
            type_options.join(", "),
            json_type_name(value)
        ));
    }

    if type_options
        .iter()
        .all(|t| *t == "integer" || *t == "number")
        && matches!(value, Value::Bool(_))
    {
        if type_options.len() == 1 {
            return Some(format!(
                "Argument '{}' must be {}, got bool",
                path, type_options[0]
            ));
        }
        return Some(format!(
            "Argument '{}' must be one of [{}], got bool",
            path,
            type_options.join(", ")
        ));
    }

    if let Some(const_val) = obj.get("const") {
        if value != const_val {
            return Some(format!(
                "Argument '{}' must equal {}, got {}",
                path, const_val, value
            ));
        }
    }

    if let Some(enums) = obj.get("enum").and_then(|v| v.as_array()) {
        if !enums.iter().any(|e| e == value) {
            return Some(format!(
                "Argument '{}' must be one of: {}",
                path,
                enums
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }

    if type_options.iter().any(|t| *t == "string") && value.is_string() {
        if let Some(s) = value.as_str() {
            if let Some(min) = obj.get("minLength").and_then(|v| v.as_u64()) {
                if (s.chars().count() as u64) < min {
                    return Some(format!(
                        "Argument '{}' length {} is less than minLength {}",
                        path,
                        s.chars().count(),
                        min
                    ));
                }
            }
            if let Some(max) = obj.get("maxLength").and_then(|v| v.as_u64()) {
                if (s.chars().count() as u64) > max {
                    return Some(format!(
                        "Argument '{}' length {} exceeds maxLength {}",
                        path,
                        s.chars().count(),
                        max
                    ));
                }
            }
            if let Some(pattern) = obj.get("pattern").and_then(|v| v.as_str()) {
                match regex::Regex::new(pattern) {
                    Ok(re) => {
                        if re.find(s).is_none() {
                            return Some(format!(
                                "Argument '{}' does not match pattern '{}'",
                                path, pattern
                            ));
                        }
                    }
                    Err(e) => {
                        return Some(format!("Argument '{}' has invalid pattern: {}", path, e));
                    }
                }
            }
        }
    }

    let is_numeric = type_options
        .iter()
        .any(|t| *t == "number" || *t == "integer");
    let is_not_bool = !matches!(value, Value::Bool(_));
    if is_numeric && is_not_bool {
        if let Some(n) = value.as_f64() {
            if n.is_nan() {
                return Some(format!(
                    "Argument '{}' must be a finite number, got NaN",
                    path
                ));
            }
            if n.is_infinite() {
                let sign = if n > 0.0 { "+inf" } else { "-inf" };
                return Some(format!(
                    "Argument '{}' must be a finite number, got {}",
                    path, sign
                ));
            }
            if let Some(min) = obj.get("minimum").and_then(|v| v.as_f64()) {
                if n < min {
                    return Some(format!(
                        "Argument '{}' value {} is less than minimum {}",
                        path, n, min
                    ));
                }
            }
            if let Some(max) = obj.get("maximum").and_then(|v| v.as_f64()) {
                if n > max {
                    return Some(format!(
                        "Argument '{}' value {} exceeds maximum {}",
                        path, n, max
                    ));
                }
            }
            if let Some(excl_min) = obj.get("exclusiveMinimum").and_then(|v| v.as_f64()) {
                if n <= excl_min {
                    return Some(format!(
                        "Argument '{}' value {} must be > exclusiveMinimum {}",
                        path, n, excl_min
                    ));
                }
            }
            if let Some(excl_max) = obj.get("exclusiveMaximum").and_then(|v| v.as_f64()) {
                if n >= excl_max {
                    return Some(format!(
                        "Argument '{}' value {} must be < exclusiveMaximum {}",
                        path, n, excl_max
                    ));
                }
            }
            if let Some(multiple_of) = obj.get("multipleOf").and_then(|v| v.as_f64()) {
                if multiple_of > 0.0 {
                    let remainder = n % multiple_of;
                    let abs_check = remainder.abs() < 1e-12;
                    let rel_check = (remainder / multiple_of).abs() < 1e-9;
                    if !abs_check && !rel_check {
                        return Some(format!(
                            "Argument '{}' value {} is not a multiple of {}",
                            path, n, multiple_of
                        ));
                    }
                }
            }
        }
    }

    if type_options.iter().any(|t| *t == "object") && value.is_object() {
        let sub_props = obj.get("properties").and_then(|v| v.as_object());
        let sub_required = obj.get("required").and_then(|v| v.as_array());
        let sub_additional = obj
            .get("additionalProperties")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let has_sub_schema = sub_props.map_or(false, |p| !p.is_empty())
            || sub_required.map_or(false, |r| !r.is_empty());

        if has_sub_schema {
            if let Some(req) = sub_required {
                for field in req {
                    if let Some(field_name) = field.as_str() {
                        if !value.as_object().unwrap().contains_key(field_name) {
                            return Some(format!(
                                "Missing required field '{}' in '{}'",
                                field_name, path
                            ));
                        }
                    }
                }
            }

            if !sub_additional {
                if let (Some(props), Some(val_obj)) = (sub_props, value.as_object()) {
                    let unknown: Vec<&String> = val_obj
                        .keys()
                        .filter(|k| !props.contains_key(k.as_str()))
                        .collect();
                    if !unknown.is_empty() {
                        return Some(format!(
                            "Unexpected field(s) in '{}': {}",
                            path,
                            unknown
                                .iter()
                                .map(|s| s.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                    }
                }
            }

            if let Some(val_obj) = value.as_object() {
                for (sub_key, sub_val) in val_obj {
                    if let Some(props) = sub_props {
                        if let Some(sub_schema) = props.get(sub_key.as_str()) {
                            let sub_path = format!("{}.{}", path, sub_key);
                            if let Some(err) = validate_property_inner(
                                sub_val,
                                sub_schema,
                                &sub_path,
                                max_depth - 1,
                            ) {
                                return Some(err);
                            }
                        }
                    }
                }
            }
        }
    }

    if type_options.iter().any(|t| *t == "array") && value.is_array() {
        if let Some(arr) = value.as_array() {
            if let Some(min) = obj.get("minItems").and_then(|v| v.as_u64()) {
                if (arr.len() as u64) < min {
                    return Some(format!(
                        "Argument '{}' has {} items, less than minItems {}",
                        path,
                        arr.len(),
                        min
                    ));
                }
            }
            if let Some(max) = obj.get("maxItems").and_then(|v| v.as_u64()) {
                if (arr.len() as u64) > max {
                    return Some(format!(
                        "Argument '{}' has {} items, exceeds maxItems {}",
                        path,
                        arr.len(),
                        max
                    ));
                }
            }

            if obj.get("uniqueItems").and_then(|v| v.as_bool()) == Some(true) {
                let mut seen = HashSet::new();
                for item in arr {
                    let s = item.to_string();
                    if !seen.insert(s) {
                        return Some(format!(
                            "Argument '{}' has duplicate items but uniqueItems is True",
                            path
                        ));
                    }
                }
            }

            if let Some(items_schema) = obj.get("items") {
                for (i, item) in arr.iter().enumerate() {
                    let item_path = format!("{}[{}]", path, i);
                    if let Some(err) =
                        validate_property_inner(item, items_schema, &item_path, max_depth - 1)
                    {
                        return Some(err);
                    }
                }
            }
        }
    }

    None
}

fn value_matches_type(value: &Value, t: &str) -> bool {
    match t {
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value.is_i64() || value.is_u64(),
        "boolean" => value.is_boolean(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        "null" => value.is_null(),
        _ => false,
    }
}

fn validate_arguments(name: &str, arguments: &Value) -> Option<String> {
    let schema = match SCHEMA_CACHE.get(name) {
        Some(s) => s,
        None => return None,
    };

    let obj = match arguments.as_object() {
        Some(o) => o,
        None => return None,
    };

    let props = schema.get("properties").and_then(|v| v.as_object());
    let required = schema.get("required").and_then(|v| v.as_array());
    let additional = schema
        .get("additionalProperties")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Match Python's inspect.signature() behavior: report unexpected
    // keyword arguments before missing required arguments when both apply.
    if !additional {
        if let Some(p) = props {
            let mut unknown: Vec<&String> =
                obj.keys().filter(|k| !p.contains_key(k.as_str())).collect();
            unknown.sort();
            if !unknown.is_empty() {
                return Some(format!(
                    "Unexpected argument(s): {}",
                    unknown
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }
    }

    if let Some(req) = required {
        for field in req {
            if let Some(field_name) = field.as_str() {
                if !obj.contains_key(field_name) {
                    return Some(format!("Missing required argument: {}", field_name));
                }
            }
        }
    }

    if let Some(p) = props {
        for (key, value) in obj {
            if let Some(prop_schema) = p.get(key.as_str()) {
                if let Some(err) = validate_property(value, prop_schema, key) {
                    return Some(err);
                }
            }
        }
    }

    None
}

fn escape_ascii_json(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii() {
            result.push(c);
        } else {
            let mut utf16 = [0u16; 2];
            for unit in c.encode_utf16(&mut utf16).iter() {
                result.push_str(&format!("\\u{:04x}", unit));
            }
        }
    }
    result
}

fn python_json_dumps<T: Serialize>(value: &T) -> String {
    struct PythonStyleFormatter;

    impl serde_json::ser::Formatter for PythonStyleFormatter {
        fn begin_array_value<W: std::io::Write + ?Sized>(
            &mut self,
            writer: &mut W,
            first: bool,
        ) -> std::io::Result<()> {
            if first {
                Ok(())
            } else {
                writer.write_all(b", ")
            }
        }

        fn begin_object_key<W: std::io::Write + ?Sized>(
            &mut self,
            writer: &mut W,
            first: bool,
        ) -> std::io::Result<()> {
            if first {
                Ok(())
            } else {
                writer.write_all(b", ")
            }
        }

        fn begin_object_value<W: std::io::Write + ?Sized>(
            &mut self,
            writer: &mut W,
        ) -> std::io::Result<()> {
            writer.write_all(b": ")
        }
    }

    let mut buf = Vec::new();
    {
        let mut serializer = serde_json::Serializer::with_formatter(&mut buf, PythonStyleFormatter);
        if value.serialize(&mut serializer).is_err() {
            return String::new();
        }
    }
    let serialized = String::from_utf8(buf).unwrap_or_default();
    escape_ascii_json(&serialized)
}

fn wrap_tool_response(tool_response: &ToolResponse) -> serde_json::Value {
    let text = python_json_dumps(tool_response);
    if tool_response.ok {
        serde_json::json!({
            "content": [{"type": "text", "text": text}],
        })
    } else {
        serde_json::json!({
            "content": [{"type": "text", "text": text}],
            "isError": true,
        })
    }
}

struct RateLimiter {
    timestamps: VecDeque<Instant>,
}

impl RateLimiter {
    fn new() -> Self {
        Self {
            timestamps: VecDeque::new(),
        }
    }

    fn check(&mut self) -> bool {
        let now = Instant::now();
        while let Some(&front) = self.timestamps.front() {
            if now.duration_since(front) > Duration::from_secs(1) {
                self.timestamps.pop_front();
            } else {
                break;
            }
        }
        if self.timestamps.len() < MAX_REQUESTS_PER_SECOND as usize {
            self.timestamps.push_back(now);
            true
        } else {
            false
        }
    }
}

fn truncate_2000(s: &str) -> String {
    s.chars().take(2000).collect()
}

struct CancelledRequests {
    set: HashSet<Value>,
    order: VecDeque<Value>,
}

impl CancelledRequests {
    fn new() -> Self {
        Self {
            set: HashSet::new(),
            order: VecDeque::new(),
        }
    }

    fn insert(&mut self, id: Value) {
        if !self.set.contains(&id) {
            self.set.insert(id.clone());
            self.order.push_back(id);
        }
        while self.set.len() > MAX_CANCELLED_REQUESTS {
            if let Some(oldest) = self.order.pop_front() {
                self.set.remove(&oldest);
            } else {
                break;
            }
        }
    }

    fn remove(&mut self, id: &Value) {
        if self.set.remove(id) {
            // Best-effort removal from order queue (linear scan)
            if let Some(pos) = self.order.iter().position(|x| x == id) {
                self.order.remove(pos);
            }
        }
    }

    fn contains(&self, id: &Value) -> bool {
        self.set.contains(id)
    }
}

// MCP-safe defaults: set once on first request, matching Python's idempotent check.
static MCP_DEFAULTS_CONFIGURED: AtomicBool = AtomicBool::new(false);

fn ensure_mcp_defaults() {
    if !MCP_DEFAULTS_CONFIGURED.swap(true, Ordering::SeqCst) {
        set_mcp_mode();
    }
}

async fn handle_request_async(
    request: &JsonRpcRequest,
    cancelled: &Arc<tokio::sync::Mutex<CancelledRequests>>,
    tool_semaphore: &Arc<tokio::sync::Semaphore>,
) -> Option<serde_json::Value> {
    // Ensure MCP-safe evaluator defaults are in effect. Idempotent: a one-time
    // check is enough to set mcp_mode and disable random/side-effect functions.
    ensure_mcp_defaults();

    match request.method.as_str() {
        "initialize" => Some(
            serde_json::to_value(InitializeResult {
                protocol_version: "2024-11-05".to_string(),
                capabilities: Capabilities {
                    tools: ToolsCapability {
                        list_changed: false,
                    },
                },
                server_info: ServerInfo {
                    name: "eggsact".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                },
            })
            .unwrap(),
        ),

        "tools/list" => {
            let params = request.params.as_ref();
            if let Some(p) = params {
                if !p.is_object() {
                    return Some(serde_json::json!({
                        "jsonrpc": "2.0",
                        "error": {"code": -32600, "message": "Invalid params: expected object"},
                        "id": request.id
                    }));
                }
            }
            // Validate param types (matching Python messages exactly)
            if let Some(p) = params {
                if let Some(d) = p.get("schema_detail") {
                    if !d.is_string() || !matches!(d.as_str(), Some("compact" | "normal" | "full"))
                    {
                        return Some(serde_json::json!({
                            "jsonrpc": "2.0",
                            "error": {"code": -32600, "message": "Invalid 'schema_detail' parameter: expected compact, normal, or full"},
                            "id": request.id
                        }));
                    }
                }
                if let Some(t) = p.get("tier") {
                    // Python treats bool as int (isinstance(True, int) == True)
                    if !t.is_i64() && !t.is_u64() && !t.is_boolean() {
                        return Some(serde_json::json!({
                            "jsonrpc": "2.0",
                            "error": {"code": -32600, "message": "Invalid 'tier' parameter: expected integer"},
                            "id": request.id
                        }));
                    }
                }
                if let Some(t) = p.get("tags") {
                    if !t.is_array() {
                        return Some(serde_json::json!({
                            "jsonrpc": "2.0",
                            "error": {"code": -32600, "message": "Invalid 'tags' parameter: expected array"},
                            "id": request.id
                        }));
                    }
                    if !t.as_array().unwrap().iter().all(|v| v.is_string()) {
                        return Some(serde_json::json!({
                            "jsonrpc": "2.0",
                            "error": {"code": -32600, "message": "Invalid 'tags' parameter: all items must be strings"},
                            "id": request.id
                        }));
                    }
                }
                if let Some(n) = p.get("names") {
                    if !n.is_array() {
                        return Some(serde_json::json!({
                            "jsonrpc": "2.0",
                            "error": {"code": -32600, "message": "Invalid 'names' parameter: expected array"},
                            "id": request.id
                        }));
                    }
                    if !n.as_array().unwrap().iter().all(|v| v.is_string()) {
                        return Some(serde_json::json!({
                            "jsonrpc": "2.0",
                            "error": {"code": -32600, "message": "Invalid 'names' parameter: all items must be strings"},
                            "id": request.id
                        }));
                    }
                }
                if let Some(pr) = p.get("profile") {
                    if !pr.is_string() {
                        return Some(serde_json::json!({
                            "jsonrpc": "2.0",
                            "error": {"code": -32600, "message": "Invalid 'profile' parameter: expected string"},
                            "id": request.id
                        }));
                    }
                }
            }
            let schema_detail = get_schema_detail();
            let detail = params
                .and_then(|p| p.get("schema_detail"))
                .and_then(|d| d.as_str())
                .unwrap_or(&schema_detail);
            let names_filter = params
                .and_then(|p| p.get("names"))
                .and_then(|n| n.as_array());
            let profile_filter = params
                .and_then(|p| p.get("profile"))
                .and_then(|p| p.as_str());
            let tier_filter = params.and_then(|p| p.get("tier")).and_then(|t| {
                // Python treats bool as int (isinstance(True, int) == True)
                match t {
                    Value::Number(n) => n.as_u64(),
                    Value::Bool(b) => Some(if *b { 1 } else { 0 }),
                    _ => None,
                }
            });
            let tags_filter = params
                .and_then(|p| p.get("tags"))
                .and_then(|t| t.as_array());

            let active_profile = get_active_profile();
            let effective_profile = profile_filter.unwrap_or(&active_profile);
            if effective_profile != "full" && !PROFILE_NAMES.contains(&effective_profile) {
                let available = PROFILE_NAMES.join(", ");
                return Some(serde_json::json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32602,
                        "message": format!("Unknown MCP profile: '{}'. Available profiles: {}", effective_profile, available)
                    },
                    "id": request.id
                }));
            }
            let profile_tools = get_profile_tools(effective_profile);
            let profile_set: HashSet<&str> = profile_tools.into_iter().collect();

            let mut tools = list_tools();

            // Filter by profile
            tools.retain(|t| profile_set.contains(t.name.as_str()));

            // Filter by names
            if let Some(names) = names_filter {
                let name_set: HashSet<&str> = names.iter().filter_map(|n| n.as_str()).collect();
                tools.retain(|t| name_set.contains(t.name.as_str()));
            }

            // Filter by tier
            if let Some(tier) = tier_filter {
                tools.retain(|t| t.tier == Some(tier as u8));
            }

            // Filter by tags (all specified tags must be present)
            if let Some(tags) = tags_filter {
                let tag_set: HashSet<&str> = tags.iter().filter_map(|t| t.as_str()).collect();
                tools.retain(|t| {
                    if let Some(ref tool_tags) = t.tags {
                        tag_set
                            .iter()
                            .all(|tag| tool_tags.iter().any(|tt| tt.as_str() == *tag))
                    } else {
                        false
                    }
                });
            }

            if detail == "compact" {
                for tool in &mut tools {
                    // Truncate description to 120 chars
                    if tool.description.chars().count() > 120 {
                        let truncated: String = tool.description.chars().take(117).collect();
                        tool.description = truncated;
                        tool.description.push_str("...");
                    }
                    // Compact input schema: strip defaults, truncate property descriptions
                    tool.input_schema = compact_input_schema(&tool.input_schema);
                    // Compact output schema: keep top-level keys/types only
                    if let Some(ref output) = tool.output_schema.clone() {
                        tool.output_schema = Some(compact_output_schema(output));
                    }
                    // Python compact mode: drops tier and tags, keeps category/llm_exposure/cost
                    tool.tier = None;
                    tool.tags = None;
                }
            } else {
                // Non-compact mode: include deprecated field for all tools (Python parity)
                for tool in &mut tools {
                    tool.deprecated = Some(tool.deprecated.unwrap_or(false));
                }
            }

            Some(serde_json::json!({"tools": tools}))
        }

        "tools/call" => {
            let params = match request.params.as_ref() {
                Some(p) => {
                    if !p.is_object() {
                        return Some(serde_json::json!({
                            "jsonrpc": "2.0",
                            "error": {"code": -32600, "message": "Invalid params: expected object"},
                            "id": request.id
                        }));
                    }
                    p
                }
                None => {
                    return Some(serde_json::json!({
                        "jsonrpc": "2.0",
                        "error": {"code": -32600, "message": "Invalid params: expected object"},
                        "id": request.id
                    }));
                }
            };
            let name = match params.get("name").and_then(|v| v.as_str()) {
                Some(n) => n,
                None => {
                    return Some(serde_json::json!({
                        "jsonrpc": "2.0",
                        "error": {"code": -32600, "message": "Invalid params: missing tool name"},
                        "id": request.id
                    }));
                }
            };
            let arguments_val = match params.get("arguments") {
                Some(v) if v.is_object() => v.clone(),
                Some(_) => {
                    return Some(serde_json::json!({
                        "jsonrpc": "2.0",
                        "error": {"code": -32600, "message": "Invalid arguments: expected object"},
                        "id": request.id
                    }));
                }
                None => serde_json::Value::Object(serde_json::Map::new()),
            };

            // Check if request was cancelled before execution
            if let Some(ref id) = request.id {
                let mut cancelled_set = cancelled.lock().await;
                if cancelled_set.contains(id) {
                    // Remove from cancelled set so reuse of same id won't re-trigger
                    cancelled_set.remove(id);
                    return Some(wrap_tool_response(&ToolResponse::error(
                        "cancelled",
                        &format!("Tool '{}' request was cancelled", name),
                        None,
                        Some(name),
                    )));
                }
            }

            // Look up the tool handler (exact match only)
            let canonical_name = name.to_string();
            let handler_opt = TOOL_HANDLERS
                .iter()
                .find(|(tool_name, _)| *tool_name == name)
                .map(|(_, h)| *h);

            if handler_opt.is_none() {
                // Unknown tool — return -32601 (matching Python)
                let tool_names: Vec<&str> = TOOL_HANDLERS.iter().map(|(n, _)| *n).collect();
                let msg = match find_close_match(name, &tool_names) {
                    Some(m) => format!("Unknown tool: {}. Did you mean: {}?", name, m),
                    None => format!("Unknown tool: {}", name),
                };
                return Some(serde_json::json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32601,
                        "message": msg
                    },
                    "id": request.id
                }));
            }

            // Enforce active profile: reject tools not in the current profile
            let profile_tools = get_profile_tools(&get_active_profile());
            if !profile_tools.contains(&&*canonical_name) {
                return Some(serde_json::json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32602,
                        "message": format!(
                            "Tool '{}' is not available in profile '{}'. Use tools/list to see available tools, or switch profile.",
                            canonical_name, get_active_profile()
                        )
                    },
                    "id": request.id
                }));
            }

            let handler = handler_opt.unwrap();

            if let Some(msg) = validate_arguments(&canonical_name, &arguments_val) {
                return Some(serde_json::json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32602,
                        "message": format!(
                            "Invalid arguments for tool '{}': {}",
                            canonical_name, msg
                        )
                    },
                    "id": request.id
                }));
            }

            let name_owned = canonical_name.to_string();
            let args_clone = arguments_val.clone();
            let sem = tool_semaphore.clone();

            let result =
                tokio::time::timeout(Duration::from_secs(MAX_TOOL_TIMEOUT_SECONDS), async move {
                    let _permit = sem
                        .acquire()
                        .await
                        .expect("tool semaphore unexpectedly closed");
                    tokio::task::spawn_blocking(move || handler(&args_clone)).await
                })
                .await;

            match result {
                Ok(Ok(tool_response)) => {
                    // Check output size
                    let output = python_json_dumps(&tool_response);
                    if output.is_empty() {
                        Some(wrap_tool_response(&ToolResponse::error(
                            "serialization_error",
                            "Failed to serialize tool response",
                            None,
                            Some(&name_owned),
                        )))
                    } else if output.len() > MAX_OUTPUT_BYTES {
                        Some(wrap_tool_response(
                            &ToolResponse::error(
                                "output_too_large",
                                &format!(
                                    "Output exceeds {} bytes and was truncated",
                                    MAX_OUTPUT_BYTES
                                ),
                                Some(vec![
                                    "Try reducing input size or using a summary/detail option"
                                        .to_string(),
                                ]),
                                Some(&name_owned),
                            )
                            .with_warnings(vec![
                                "Output was truncated due to size limit".to_string(),
                            ]),
                        ))
                    } else {
                        Some(wrap_tool_response(&tool_response))
                    }
                }
                Ok(Err(join_err)) => Some(serde_json::json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32000,
                        "message": format!("Tool execution error: {}", truncate_2000(&sanitize_error(&join_err.to_string())))
                    },
                    "id": request.id
                })),
                Err(_timeout) => Some(wrap_tool_response(&ToolResponse::error(
                    "timeout",
                    &format!(
                        "Tool '{}' execution timed out after {}s",
                        name_owned, MAX_TOOL_TIMEOUT_SECONDS
                    ),
                    Some(vec!["Try a simpler input or shorter text".to_string()]),
                    Some(&name_owned),
                ))),
            }
        }

        "notifications/initialized" => None,

        "notifications/cancelled" => {
            if let Some(params) = &request.params {
                if let Some(request_id) = params.get("requestId") {
                    // Validate type: must be str or int, not bool
                    match request_id {
                        Value::Bool(_) => {}
                        Value::String(s) => {
                            if s.len() <= MAX_REQUEST_ID_LENGTH {
                                let mut cancelled_set = cancelled.lock().await;
                                cancelled_set.insert(request_id.clone());
                            }
                        }
                        Value::Number(n) if n.is_i64() || n.is_u64() => {
                            if request_id.to_string().len() <= MAX_REQUEST_ID_LENGTH {
                                let mut cancelled_set = cancelled.lock().await;
                                cancelled_set.insert(request_id.clone());
                            }
                        }
                        _ => {}
                    }
                }
            }
            None
        }

        "ping" => Some(serde_json::json!({})),

        "profiles/list" => {
            if let Some(ref params) = request.params {
                if !params.is_object() {
                    return Some(serde_json::json!({
                        "jsonrpc": "2.0",
                        "error": {"code": -32600, "message": "Invalid params: expected object"},
                        "id": request.id
                    }));
                }
            }
            let active = get_active_profile();
            let mut profiles_info = serde_json::Map::new();
            for &name in PROFILE_NAMES {
                let tool_list = TOOL_PROFILES.get(name).cloned().unwrap_or_default();
                let tool_names: Vec<Value> = tool_list
                    .into_iter()
                    .map(|n| Value::String(n.to_string()))
                    .collect();
                profiles_info.insert(
                    name.to_string(),
                    serde_json::json!({
                        "tools": tool_names,
                        "tool_count": tool_names.len(),
                    }),
                );
            }
            Some(serde_json::json!({
                "active_profile": active,
                "profiles": serde_json::Value::Object(profiles_info),
                "available_profiles": PROFILE_NAMES,
            }))
        }

        _ => {
            let display_method = if request.method.len() > 100 {
                // Python truncates by byte length: method[:100]
                let truncated = &request.method.as_bytes()[..100];
                // Find a valid UTF-8 boundary
                let mut end = truncated.len();
                while end > 0 && !request.method.is_char_boundary(end) {
                    end -= 1;
                }
                format!("{}...", &request.method[..end])
            } else {
                request.method.clone()
            };
            Some(
                serde_json::to_value(JsonRpcError {
                    jsonrpc: "2.0".to_string(),
                    error: JsonRpcErrorDetail {
                        code: -32601,
                        message: format!("Method not found: {}", display_method),
                    },
                    id: request.id.clone(),
                })
                .unwrap(),
            )
        }
    }
}

pub async fn main() -> ! {
    let stdin = tokio::io::stdin();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();

    let rate_limiter = Arc::new(Mutex::new(RateLimiter::new()));
    let cancelled = Arc::new(tokio::sync::Mutex::new(CancelledRequests::new()));
    let tool_semaphore = Arc::new(tokio::sync::Semaphore::new(MAX_TOOL_WORKERS));

    loop {
        let line = match lines.next_line().await {
            Ok(Some(line)) => line,
            Ok(None) | Err(_) => break,
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Request size limit
        if trimmed.len() > MAX_REQUEST_BYTES {
            let error_response = JsonRpcError {
                jsonrpc: "2.0".to_string(),
                error: JsonRpcErrorDetail {
                    code: -32700,
                    message: format!(
                        "Request exceeds maximum size of {} bytes",
                        MAX_REQUEST_BYTES
                    ),
                },
                id: None,
            };
            if let Ok(output) = serde_json::to_string(&error_response) {
                println!("{}", output);
            }
            continue;
        }

        // Reject batch requests (check before JSON parse, matching Python)
        if trimmed.starts_with('[') {
            let error_response = JsonRpcError {
                jsonrpc: "2.0".to_string(),
                error: JsonRpcErrorDetail {
                    code: -32600,
                    message: "Batch requests are not supported".to_string(),
                },
                id: None,
            };
            if let Ok(output) = serde_json::to_string(&error_response) {
                println!("{}", output);
            }
            continue;
        }

        // Parse JSON into generic Value for field-level validation
        let request_value: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => {
                let error_response = JsonRpcError {
                    jsonrpc: "2.0".to_string(),
                    error: JsonRpcErrorDetail {
                        code: -32700,
                        message: "Parse error: invalid JSON".to_string(),
                    },
                    id: None,
                };
                if let Ok(output) = serde_json::to_string(&error_response) {
                    println!("{}", output);
                }
                continue;
            }
        };

        // Validate top-level is object
        if !request_value.is_object() {
            let error_response = JsonRpcError {
                jsonrpc: "2.0".to_string(),
                error: JsonRpcErrorDetail {
                    code: -32600,
                    message: "Invalid Request: expected JSON object".to_string(),
                },
                id: None,
            };
            if let Ok(output) = serde_json::to_string(&error_response) {
                println!("{}", output);
            }
            continue;
        }

        // Validate jsonrpc version
        let actual_version = request_value
            .get("jsonrpc")
            .and_then(|v| v.as_str())
            .unwrap_or("null");
        if actual_version != "2.0" {
            let error_response = JsonRpcError {
                jsonrpc: "2.0".to_string(),
                error: JsonRpcErrorDetail {
                    code: -32600,
                    message: format!(
                        "Invalid Request: jsonrpc must be '2.0', got '{}'",
                        actual_version
                    ),
                },
                id: request_value.get("id").cloned(),
            };
            if let Ok(output) = serde_json::to_string(&error_response) {
                println!("{}", output);
            }
            continue;
        }

        // Validate method
        let method = match request_value.get("method") {
            Some(v) if v.is_string() => v.as_str().unwrap().to_string(),
            Some(_) => {
                let error_response = JsonRpcError {
                    jsonrpc: "2.0".to_string(),
                    error: JsonRpcErrorDetail {
                        code: -32600,
                        message: "Invalid Request: 'method' must be a string".to_string(),
                    },
                    id: request_value.get("id").cloned(),
                };
                if let Ok(output) = serde_json::to_string(&error_response) {
                    println!("{}", output);
                }
                continue;
            }
            None => {
                let error_response = JsonRpcError {
                    jsonrpc: "2.0".to_string(),
                    error: JsonRpcErrorDetail {
                        code: -32600,
                        message: "Invalid Request: missing 'method'".to_string(),
                    },
                    id: request_value.get("id").cloned(),
                };
                if let Ok(output) = serde_json::to_string(&error_response) {
                    println!("{}", output);
                }
                continue;
            }
        };

        // Rate limiting
        {
            let mut limiter = rate_limiter.lock().await;
            if !limiter.check() {
                let error_response = JsonRpcError {
                    jsonrpc: "2.0".to_string(),
                    error: JsonRpcErrorDetail {
                        code: -32600,
                        message: format!(
                            "Rate limit exceeded: max {} requests per second",
                            MAX_REQUESTS_PER_SECOND
                        ),
                    },
                    id: request_value.get("id").cloned(),
                };
                if let Ok(output) = serde_json::to_string(&error_response) {
                    println!("{}", output);
                }
                continue;
            }
        }

        // Validate request id
        let id = request_value.get("id");
        if let Some(id_val) = id {
            // Reject boolean, array, object, and float ids per JSON-RPC 2.0 spec
            if id_val.is_boolean() || id_val.is_array() || id_val.is_object() {
                let error_response = JsonRpcError {
                    jsonrpc: "2.0".to_string(),
                    error: JsonRpcErrorDetail {
                        code: -32600,
                        message: "Invalid Request: 'id' must be a string, integer, or null"
                            .to_string(),
                    },
                    id: None,
                };
                if let Ok(output) = serde_json::to_string(&error_response) {
                    println!("{}", output);
                }
                continue;
            }
            // Reject float IDs (JSON numbers that aren't integers)
            // Use as_i64()/as_u64() for exact integer detection — as_f64() loses
            // precision for integers >2^53 and would silently accept them.
            if id_val.is_number() && id_val.as_i64().is_none() && id_val.as_u64().is_none() {
                let error_response = JsonRpcError {
                    jsonrpc: "2.0".to_string(),
                    error: JsonRpcErrorDetail {
                        code: -32600,
                        message: "Invalid Request: 'id' must be a string, integer, or null"
                            .to_string(),
                    },
                    id: None,
                };
                if let Ok(output) = serde_json::to_string(&error_response) {
                    println!("{}", output);
                }
                continue;
            }
            let id_str = id_val.to_string();
            if id_str.len() > MAX_REQUEST_ID_LENGTH {
                let error_response = JsonRpcError {
                    jsonrpc: "2.0".to_string(),
                    error: JsonRpcErrorDetail {
                        code: -32600,
                        message: format!(
                            "Invalid Request: 'id' exceeds maximum length of {}",
                            MAX_REQUEST_ID_LENGTH
                        ),
                    },
                    id: None,
                };
                if let Ok(output) = serde_json::to_string(&error_response) {
                    println!("{}", output);
                }
                continue;
            }
        }

        // Construct JsonRpcRequest from validated value
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method,
            params: request_value.get("params").cloned(),
            id: id.cloned(),
        };

        // Handle notifications (no id) and requests (with id)
        let maybe_result = {
            let request_clone = JsonRpcRequest {
                jsonrpc: request.jsonrpc.clone(),
                method: request.method.clone(),
                params: request.params.clone(),
                id: request.id.clone(),
            };
            let cancelled_clone = cancelled.clone();
            let semaphore_clone = tool_semaphore.clone();
            let handle = tokio::spawn(async move {
                handle_request_async(&request_clone, &cancelled_clone, &semaphore_clone).await
            });
            match handle.await {
                Ok(result) => result,
                Err(join_err) => {
                    let msg = if join_err.is_cancelled() {
                        "task cancelled".to_string()
                    } else {
                        let panic_msg = join_err.into_panic();
                        match panic_msg.downcast_ref::<&str>() {
                            Some(s) => s.to_string(),
                            None => match panic_msg.downcast_ref::<String>() {
                                Some(s) => s.clone(),
                                None => "unknown error".to_string(),
                            },
                        }
                    };
                    Some(serde_json::json!({
                        "jsonrpc": "2.0",
                        "error": {"code": -32603, "message": truncate_2000(&sanitize_error(&format!("Internal error: {}", msg)))},
                        "id": request.id
                    }))
                }
            }
        };
        if let Some(result) = maybe_result {
            // Check if this is already a JSON-RPC error (has "error" key at top level)
            if result.get("error").is_some() && result.get("result").is_none() {
                // Already a JSON-RPC error response, output directly
                if let Ok(output) = serde_json::to_string(&result) {
                    println!("{}", output);
                }
            } else {
                let response = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    result,
                    id: request.id,
                };

                if let Ok(output) = serde_json::to_string(&response) {
                    println!("{}", output);
                }
            }
        }
    }

    std::process::exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_bug018_pattern_matches_anywhere_in_string() {
        let schema = json!({"type": "string", "pattern": "[0-9]+"});
        let result = validate_property_inner(&json!("abc123"), &schema, "test", 10);
        assert!(
            result.is_none(),
            "pattern [0-9]+ should match 'abc123' at position 3, got: {:?}",
            result
        );
    }

    #[test]
    fn test_bug018_pattern_anchored_accepts() {
        let schema = json!({"type": "string", "pattern": "^[A-Z]"});
        let result = validate_property_inner(&json!("Hello"), &schema, "test", 10);
        assert!(
            result.is_none(),
            "pattern ^[A-Z] should match 'Hello', got: {:?}",
            result
        );
    }

    #[test]
    fn test_bug018_pattern_anchored_rejects() {
        let schema = json!({"type": "string", "pattern": "^[A-Z]"});
        let result = validate_property_inner(&json!("hello"), &schema, "test", 10);
        assert!(result.is_some(), "pattern ^[A-Z] should reject 'hello'");
    }

    #[test]
    fn test_bug018_pattern_no_match_rejects() {
        let schema = json!({"type": "string", "pattern": "^[0-9]+$"});
        let result = validate_property_inner(&json!("abc123def"), &schema, "test", 10);
        assert!(
            result.is_some(),
            "pattern ^[0-9]+$ should reject 'abc123def'"
        );
    }

    #[test]
    fn test_bug019_multipleof_relative_tolerance() {
        let schema = json!({"type": "number", "multipleOf": 3.0});
        let result = validate_property_inner(&json!(9.000000001), &schema, "test", 10);
        assert!(
            result.is_none(),
            "9.000000001 should pass multipleOf 3.0 with relative tolerance, got: {:?}",
            result
        );
    }

    #[test]
    fn test_bug019_multipleof_exact_value() {
        let schema = json!({"type": "number", "multipleOf": 5.0});
        let result = validate_property_inner(&json!(15.0), &schema, "test", 10);
        assert!(
            result.is_none(),
            "15.0 should pass multipleOf 5.0, got: {:?}",
            result
        );
    }

    #[test]
    fn test_bug019_multipleof_rejects_non_multiple() {
        let schema = json!({"type": "number", "multipleOf": 3.0});
        let result = validate_property_inner(&json!(7.5), &schema, "test", 10);
        assert!(result.is_some(), "7.5 should fail multipleOf 3.0");
    }

    #[test]
    fn test_bug019_multipleof_large_value() {
        // 10000000000.0000001 is very close to 10^10, and 1e-9 * 10^19 = 1e10.
        // Due to f64 precision, use a large value that IS a clean multiple:
        // 3000000000.0 = 3.0 * 1000000000.0
        let schema = json!({"type": "number", "multipleOf": 3.0});
        let result = validate_property_inner(&json!(3000000000.0), &schema, "test", 10);
        assert!(
            result.is_none(),
            "3000000000.0 should pass multipleOf 3.0, got: {:?}",
            result
        );
    }
}
