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

/// Standard MCP tool annotations (2026-07-28 and legacy eras).
///
/// Eggsact tools are local deterministic computations/classifiers: they do not
/// modify the external environment and do not access an open world. For every
/// model-visible tool that statement is exact, so the registry advertises:
/// `readOnlyHint=true`, `destructiveHint=false`, `idempotentHint=true`,
/// `openWorldHint=false`. Annotations are hints, not enforcement; audience,
/// profile, and budget rules remain the deterministic controls.
#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolAnnotations {
    pub read_only_hint: bool,
    pub destructive_hint: bool,
    pub idempotent_hint: bool,
    pub open_world_hint: bool,
}

impl ToolAnnotations {
    /// Annotations for eggsact's local deterministic tools.
    pub const fn local_deterministic() -> Self {
        Self {
            read_only_hint: true,
            destructive_hint: false,
            idempotent_hint: true,
            open_world_hint: false,
        }
    }
}

/// Standard annotations for a tool spec. Currently uniform because every
/// registered tool is a local deterministic computation; if a future tool
/// violates the claim, branch here rather than setting hints mechanically.
pub fn annotations_for_spec(_spec: &ToolSpec) -> ToolAnnotations {
    ToolAnnotations::local_deterministic()
}

/// Build the modern (2026-07-28) Tool JSON value.
///
/// Protocol-standard fields stay top-level (`name`, `description`,
/// `inputSchema`, `outputSchema`, `annotations`); eggsact-only registry
/// metadata moves under one stable namespaced `_meta` object so modern
/// clients never depend on nonstandard top-level keys.
pub fn modern_tool_value(
    spec: &ToolSpec,
    description: String,
    input_schema: Value,
    output_schema: Option<Value>,
    compact: bool,
) -> Value {
    let annotations = annotations_for_spec(spec);
    let deprecated = spec.stability == ToolStability::Deprecated;
    // Compact mirrors legacy compact: drop tier/tags, keep the rest.
    let (tier, tags) = if compact {
        (None, None)
    } else {
        (
            Some(spec.tier),
            Some(
                spec.tags
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>(),
            ),
        )
    };
    let mut meta_inner = serde_json::Map::new();
    if let Some(t) = tier {
        meta_inner.insert("tier".to_string(), Value::from(t));
    }
    if let Some(t) = tags {
        meta_inner.insert(
            "tags".to_string(),
            Value::Array(t.into_iter().map(Value::String).collect()),
        );
    }
    meta_inner.insert(
        "category".to_string(),
        Value::String(spec.category.to_string()),
    );
    meta_inner.insert(
        "llm_exposure".to_string(),
        Value::String(spec.exposure.as_str().to_string()),
    );
    meta_inner.insert(
        "cost".to_string(),
        Value::String(spec.cost.as_str().to_string()),
    );
    if deprecated {
        meta_inner.insert("deprecated".to_string(), Value::Bool(true));
    }
    let mut obj = serde_json::Map::new();
    obj.insert("name".to_string(), Value::String(spec.name.to_string()));
    obj.insert("description".to_string(), Value::String(description));
    obj.insert("inputSchema".to_string(), input_schema);
    if let Some(out) = output_schema {
        obj.insert("outputSchema".to_string(), out);
    }
    if let Ok(ann) = serde_json::to_value(annotations) {
        obj.insert("annotations".to_string(), ann);
    }
    obj.insert(
        "_meta".to_string(),
        Value::Object({
            let mut m = serde_json::Map::new();
            m.insert(
                crate::mcp::runtime::EGGSACT_META_NAMESPACE.to_string(),
                Value::Object(meta_inner),
            );
            m
        }),
    );
    Value::Object(obj)
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

#[derive(Clone, Copy)]
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
