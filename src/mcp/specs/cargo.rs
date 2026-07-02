use crate::mcp::registry::{ToolCost, ToolExposure, ToolSpec, ToolStability};
use crate::mcp::schemas::*;
use crate::tools::*;

pub const CARGO_TOOLS: &[ToolSpec] = &[
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
];
