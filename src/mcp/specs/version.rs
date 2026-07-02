use crate::mcp::registry::{ToolCost, ToolExposure, ToolSpec, ToolStability};
use crate::mcp::schemas::*;
use crate::tools::*;

pub const VERSION_TOOLS: &[ToolSpec] = &[
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
];
