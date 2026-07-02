use crate::mcp::registry::{ToolCost, ToolExposure, ToolSpec, ToolStability};
use crate::mcp::schemas::*;
use crate::tools::*;

pub const MARKDOWN_TOOLS: &[ToolSpec] = &[
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
];
