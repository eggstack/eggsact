use crate::mcp::registry::{ToolCost, ToolExposure, ToolSpec, ToolStability};
use crate::mcp::schemas::*;
use crate::tools::*;

pub const DEPENDENCY_TOOLS: &[ToolSpec] = &[
    ToolSpec {
        name: "dependency_edit_preflight",
        description: "Composite: inspect proposed dependency file changes before applying. Detects additions, removals, version changes, source changes (registry/path/git/url), script/hook changes, and patch overrides across Rust, Python, and Node ecosystems.",
        handler: dependency_edit_preflight,
        input_schema: dependency_edit_preflight_input,
        output_schema: dependency_edit_preflight_output,
        category: "dependency",
        tier: 2,
        profiles: &[
            "full",
            "codegg_config",
            "codegg_repo_audit",
            "codegg_preflight",
        ],
        tags: &[
            "dependencies",
            "cargo",
            "python",
            "node",
            "preflight",
            "composite",
        ],
        // `Contextual` because dependency preflight is a composite,
        // harness-oriented gate. Models may ask for risk summaries, but
        // the canonical use is to gate harness-applied dependency edits.
        exposure: ToolExposure::Contextual,
        harness_use: &["dependency_edit_preflight"],
        aliases: &[],
        cost: ToolCost::Moderate,
        stability: ToolStability::Stable,
        composite: true,
    },
];
