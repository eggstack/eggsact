use crate::mcp::registry::{ToolCost, ToolExposure, ToolSpec, ToolStability};
use crate::mcp::schemas::*;
use crate::tools::*;

pub const REPO_TOOLS: &[ToolSpec] = &[
    ToolSpec {
        name: "repo_manifest_inspect",
        description: "Classify project manifests from a bounded path list. Detects Rust, Python, Node, Go, mixed, or unknown projects and emits tool hints for downstream inspection and command policy.",
        handler: repo_manifest_inspect,
        input_schema: repo_manifest_inspect_input,
        output_schema: repo_manifest_inspect_output,
        category: "repo",
        tier: 2,
        profiles: &["full", "codegg_repo_audit"],
        tags: &["repo", "manifest", "project", "classification", "inspection"],
        exposure: ToolExposure::Contextual,
        harness_use: &[],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "config_file_inspect",
        description: "Composite: inspect a single config file beyond syntax validity. Detects risky keys, secret-like values, insecure URLs, debug flags, command hooks, and TLS/hostname issues. Returns structured findings with severity and disposition.",
        handler: config_file_inspect,
        input_schema: config_file_inspect_input,
        output_schema: config_file_inspect_output,
        category: "repo",
        tier: 2,
        profiles: &["full", "codegg_config", "codegg_repo_audit"],
        tags: &[
            "config",
            "inspection",
            "secrets",
            "security",
            "risk",
            "preflight",
            "composite",
        ],
        exposure: ToolExposure::Contextual,
        harness_use: &["config_file_inspect"],
        aliases: &[],
        cost: ToolCost::Moderate,
        stability: ToolStability::Stable,
        composite: true,
    },
];
