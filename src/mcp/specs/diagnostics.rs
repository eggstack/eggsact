use crate::mcp::registry::{ToolCost, ToolExposure, ToolSpec, ToolStability};
use crate::mcp::schemas::*;
use crate::tools::*;

pub const DIAGNOSTICS_TOOLS: &[ToolSpec] = &[
    ToolSpec {
        name: "runtime_diagnostics",
        description: "Return structured runtime diagnostics including active profile, tool counts, budget tiers, and environment status. For harness/debug audiences only.",
        handler: runtime_diagnostics,
        input_schema: runtime_diagnostics_input,
        output_schema: runtime_diagnostics_output,
        category: "diagnostics",
        tier: 3,
        profiles: &["full"],
        tags: &["diagnostics", "runtime", "harness"],
        exposure: ToolExposure::HarnessOnly,
        harness_use: &["runtime introspection", "diagnostics"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
];
