//! Deterministic measurements for the progressive-discovery surface.
//!
//! This module intentionally measures serialized UTF-8 JSON rather than model
//! tokens. It is used by regression tests, diagnostics, and maintainer-facing
//! evaluation tooling without adding a tokenizer or provider dependency.

use serde::Serialize;
use serde_json::{json, Value};

use super::discovery;
use super::registry::{self, ToolDefinition, ToolListAudience};

/// Byte-level measurements for one serialized `tools/list` catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DefinitionMetrics {
    pub profile: String,
    pub audience: String,
    pub surface: String,
    pub advertised_tools: usize,
    pub serialized_bytes: usize,
    pub description_bytes: usize,
    pub input_schema_bytes: usize,
    pub output_schema_bytes: usize,
    pub metadata_bytes: usize,
}

fn audience_name(audience: ToolListAudience) -> &'static str {
    match audience {
        ToolListAudience::Model => "model",
        ToolListAudience::Harness => "harness",
        ToolListAudience::Debug => "debug",
    }
}

fn schema_bytes(value: &Value) -> usize {
    serde_json::to_vec(value).map_or(0, |bytes| bytes.len())
}

fn definition_metadata(definition: &ToolDefinition) -> Value {
    json!({
        "deprecated": definition.deprecated,
        "tier": definition.tier,
        "tags": definition.tags,
        "category": definition.category,
        "llm_exposure": definition.llm_exposure,
        "cost": definition.cost,
    })
}

fn metrics_for_definitions(
    profile: &str,
    audience: ToolListAudience,
    surface: &str,
    definitions: &[ToolDefinition],
) -> DefinitionMetrics {
    let serialized = serde_json::to_vec(&json!({ "tools": definitions })).unwrap_or_default();
    let description_bytes = definitions
        .iter()
        .map(|definition| definition.description.len())
        .sum();
    let input_schema_bytes = definitions
        .iter()
        .map(|definition| schema_bytes(&definition.input_schema))
        .sum();
    let output_schema_bytes = definitions
        .iter()
        .filter_map(|definition| definition.output_schema.as_ref())
        .map(schema_bytes)
        .sum();
    let metadata_bytes = definitions
        .iter()
        .map(|definition| schema_bytes(&definition_metadata(definition)))
        .sum();

    DefinitionMetrics {
        profile: profile.to_string(),
        audience: audience_name(audience).to_string(),
        surface: surface.to_string(),
        advertised_tools: definitions.len(),
        serialized_bytes: serialized.len(),
        description_bytes,
        input_schema_bytes,
        output_schema_bytes,
        metadata_bytes,
    }
}

/// Measure the exact legacy `tools/list` Tool definitions for a profile.
pub fn direct_metrics(profile: &str, audience: ToolListAudience) -> DefinitionMetrics {
    let definitions = registry::list_tool_definitions(registry::ToolListOptions {
        profile,
        names: None,
        tier: None,
        tags: None,
        schema_detail: "full",
        audience: Some(audience),
    });
    metrics_for_definitions(profile, audience, "direct", &definitions)
}

/// Measure the exact legacy discovery `tools/list` Tool definitions.
pub fn discovery_metrics(profile: &str, audience: ToolListAudience) -> DefinitionMetrics {
    let definitions = discovery::discovery_legacy_definitions(profile, audience, None);
    metrics_for_definitions(profile, audience, "discovery", &definitions)
}

/// Return the baseline profiles required by the rollout plan.
pub fn baseline_metrics() -> Vec<DefinitionMetrics> {
    [
        ("full", false),
        ("default", false),
        ("codegg_core_min", false),
        ("full", true),
    ]
    .into_iter()
    .map(|(profile, discovery_surface)| {
        if discovery_surface {
            discovery_metrics(profile, ToolListAudience::Model)
        } else {
            direct_metrics(profile, ToolListAudience::Model)
        }
    })
    .collect()
}

/// Compute the discovery/direct serialized-byte ratio for one profile.
pub fn discovery_ratio(profile: &str, audience: ToolListAudience) -> f64 {
    let direct = direct_metrics(profile, audience).serialized_bytes as f64;
    let discovery = discovery_metrics(profile, audience).serialized_bytes as f64;
    if direct == 0.0 {
        0.0
    } else {
        discovery / direct
    }
}

/// Serialize a search result for deterministic response-size measurements.
pub fn search_result_bytes(matches: &Value) -> usize {
    serde_json::to_vec(matches).map_or(0, |bytes| bytes.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_shapes_are_stable_and_discovery_is_small() {
        let baselines = baseline_metrics();
        assert_eq!(baselines.len(), 4);
        assert_eq!(baselines[0].surface, "direct");
        assert_eq!(baselines[3].surface, "discovery");
        assert!(baselines[3].advertised_tools <= 10);
        assert!(baselines[3].serialized_bytes < baselines[0].serialized_bytes / 4);
        assert!(discovery_ratio("full", ToolListAudience::Model) <= 0.25);
    }

    #[test]
    fn metrics_account_for_schema_and_metadata_bytes() {
        let direct = direct_metrics("full", ToolListAudience::Model);
        assert!(direct.description_bytes > 0);
        assert!(direct.input_schema_bytes > 0);
        assert!(direct.output_schema_bytes > 0);
        assert!(direct.metadata_bytes > 0);
        assert!(direct.serialized_bytes > direct.description_bytes);
    }
}
