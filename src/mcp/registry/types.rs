use crate::mcp::response::ToolResponse;
use serde::Serialize;
use serde_json::Value;

#[derive(Serialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
    #[serde(rename = "outputSchema", skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_exposure: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<String>,
}

/// Function pointer type for tool handler implementations.
pub type ToolHandler = fn(&Value) -> ToolResponse;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ToolExposure {
    Default,
    Contextual,
    ExpertOnly,
    HarnessOnly,
    Hidden,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ToolCost {
    Cheap,
    Moderate,
    Heavy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ToolStability {
    Stable,
    Deprecated,
    Experimental,
}

impl ToolExposure {
    pub fn as_str(&self) -> &'static str {
        match self {
            ToolExposure::Default => "default",
            ToolExposure::Contextual => "contextual",
            ToolExposure::ExpertOnly => "expert_only",
            ToolExposure::HarnessOnly => "harness_only",
            ToolExposure::Hidden => "hidden",
        }
    }
}

impl ToolCost {
    pub fn as_str(&self) -> &'static str {
        match self {
            ToolCost::Cheap => "cheap",
            ToolCost::Moderate => "moderate",
            ToolCost::Heavy => "heavy",
        }
    }
}

impl ToolStability {
    pub fn as_str(&self) -> &'static str {
        match self {
            ToolStability::Stable => "stable",
            ToolStability::Deprecated => "deprecated",
            ToolStability::Experimental => "experimental",
        }
    }
}

pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub handler: ToolHandler,
    pub input_schema: fn() -> Value,
    pub output_schema: fn() -> Value,
    pub category: &'static str,
    pub tier: u8,
    pub profiles: &'static [&'static str],
    pub tags: &'static [&'static str],
    pub exposure: ToolExposure,
    pub harness_use: &'static [&'static str],
    pub aliases: &'static [&'static str],
    pub cost: ToolCost,
    pub stability: ToolStability,
    pub composite: bool,
}
