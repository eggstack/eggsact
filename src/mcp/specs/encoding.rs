use crate::mcp::registry::{ToolCost, ToolExposure, ToolSpec, ToolStability};
use crate::mcp::schemas::*;
use crate::tools::*;

pub const ENCODING_TOOLS: &[ToolSpec] = &[
    ToolSpec {
        name: "codec_convert",
        description: "Convert bounded text/byte payloads among UTF-8, strict hex, standard Base64, and Base64URL with canonical output.",
        handler: codec_convert,
        input_schema: codec_convert_input,
        output_schema: codec_convert_output,
        category: "encoding",
        tier: 2,
        profiles: &["full"],
        tags: &["encoding", "hex", "base64", "utf8"],
        exposure: ToolExposure::Contextual,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
    ToolSpec {
        name: "radix_convert",
        description: "Convert signed-magnitude integers between bases 2 through 36 using checked u128 arithmetic and canonical digits.",
        handler: radix_convert,
        input_schema: radix_convert_input,
        output_schema: radix_convert_output,
        category: "encoding",
        tier: 2,
        profiles: &["full"],
        tags: &["encoding", "radix", "integer", "base"],
        exposure: ToolExposure::Contextual,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Cheap,
        stability: ToolStability::Stable,
        composite: false,
    },
];
