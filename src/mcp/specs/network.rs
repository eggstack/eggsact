use crate::mcp::registry::{ToolCost, ToolExposure, ToolSpec, ToolStability};
use crate::mcp::schemas::*;
use crate::tools::*;

pub const NETWORK_TOOLS: &[ToolSpec] = &[
    ToolSpec {
        name: "ip_inspect",
        description: "Parse an IPv4 or IPv6 address, canonicalize it, expose exact numeric/byte forms, and classify explicit special-use ranges.",
        handler: ip_inspect,
        input_schema: ip_inspect_input,
        output_schema: ip_inspect_output,
        category: "network",
        tier: 2,
        profiles: &["full"],
        tags: &["network", "ip", "ipv4", "ipv6", "classification"],
        exposure: ToolExposure::Contextual,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "cidr_inspect",
        description: "Normalize an IPv4 or IPv6 CIDR, calculate exact boundaries and counts, and optionally test same-family containment.",
        handler: cidr_inspect,
        input_schema: cidr_inspect_input,
        output_schema: cidr_inspect_output,
        category: "network",
        tier: 2,
        profiles: &["full"],
        tags: &["network", "cidr", "subnet", "ipv4", "ipv6"],
        exposure: ToolExposure::Contextual,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
];
