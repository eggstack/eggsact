use crate::mcp::registry::{ToolCost, ToolExposure, ToolSpec, ToolStability};
use crate::mcp::schemas::*;
use crate::tools::*;

pub const TOML_TOOLS: &[ToolSpec] = &[ToolSpec {
    name: "toml_shape",
    description:
        "Analyze the structure of a TOML document: top-level keys, tables, and nesting hierarchy.",
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
}];
