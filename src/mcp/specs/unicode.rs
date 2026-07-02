use crate::mcp::registry::{ToolCost, ToolExposure, ToolSpec, ToolStability};
use crate::mcp::schemas::*;
use crate::tools::*;

pub const UNICODE_TOOLS: &[ToolSpec] = &[
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
];
