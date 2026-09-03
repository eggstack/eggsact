use crate::mcp::registry::{ToolCost, ToolExposure, ToolSpec, ToolStability};
use crate::mcp::schemas::*;
use crate::tools::*;

pub const TEMPORAL_TOOLS: &[ToolSpec] = &[
    ToolSpec {
        name: "datetime_convert",
        description: "Convert RFC 3339 and signed Unix timestamps exactly across fixed offsets, returning canonical calendar and decimal-unit forms.",
        handler: datetime_convert,
        input_schema: datetime_convert_input,
        output_schema: datetime_convert_output,
        category: "temporal",
        tier: 2,
        profiles: &["full"],
        tags: &["datetime", "timestamp", "rfc3339", "unix", "fixed-offset"],
        exposure: ToolExposure::Contextual,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "cron_inspect",
        description: "Parse bounded five-field cron schedules and find strictly later runs in the fixed offset carried by a supplied RFC 3339 instant.",
        handler: cron_inspect,
        input_schema: cron_inspect_input,
        output_schema: cron_inspect_output,
        category: "temporal",
        tier: 2,
        profiles: &["full"],
        tags: &["cron", "schedule", "datetime", "fixed-offset"],
        exposure: ToolExposure::Contextual,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Moderate,
        stability: ToolStability::Stable,
        composite: false,
    },
];
