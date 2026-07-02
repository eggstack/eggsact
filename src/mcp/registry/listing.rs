use crate::text::levenshtein_distance;
use serde_json::Value;

use super::all_tools::{ALL_TOOLS, PROFILE_NAMES};
use super::types::{ToolExposure, ToolStability};

// ---------------------------------------------------------------------------
// Basic helpers
// ---------------------------------------------------------------------------

pub fn all_tools() -> &'static [super::types::ToolSpec] {
    ALL_TOOLS
}

pub fn get_tool(name: &str) -> Option<&'static super::types::ToolSpec> {
    ALL_TOOLS.iter().find(|t| t.name == name)
}

pub fn tool_names() -> Vec<&'static str> {
    ALL_TOOLS.iter().map(|t| t.name).collect()
}

pub fn tools_for_profile(profile: &str) -> Vec<&'static super::types::ToolSpec> {
    if profile == "full" {
        return ALL_TOOLS
            .iter()
            .filter(|t| t.exposure != ToolExposure::Hidden)
            .collect();
    }
    ALL_TOOLS
        .iter()
        .filter(|t| t.profiles.contains(&profile))
        .collect()
}

pub fn available_profiles() -> &'static [&'static str] {
    PROFILE_NAMES
}

pub fn tool_handler_for(name: &str) -> Option<super::types::ToolHandler> {
    get_tool(name).map(|spec| spec.handler)
}

pub fn tool_count() -> usize {
    ALL_TOOLS.len()
}

pub fn input_schema_for(name: &str) -> Option<Value> {
    get_tool(name).map(|spec| (spec.input_schema)())
}

pub fn output_schema_for(name: &str) -> Option<Value> {
    get_tool(name).map(|spec| (spec.output_schema)())
}

// ---------------------------------------------------------------------------
// MCP tool definition generation
// ---------------------------------------------------------------------------

pub fn mcp_tool_definitions() -> Vec<super::types::ToolDefinition> {
    ALL_TOOLS
        .iter()
        .filter(|t| t.exposure != ToolExposure::Hidden)
        .map(|spec| {
            let deprecated = if spec.stability == ToolStability::Deprecated {
                Some(true)
            } else {
                None
            };
            super::types::ToolDefinition {
                name: spec.name.to_string(),
                description: spec.description.to_string(),
                input_schema: (spec.input_schema)(),
                output_schema: Some((spec.output_schema)()),
                tier: Some(spec.tier),
                tags: Some(spec.tags.iter().map(|s| s.to_string()).collect()),
                deprecated,
                category: Some(spec.category.to_string()),
                llm_exposure: Some(spec.exposure.as_str().to_string()),
                cost: Some(spec.cost.as_str().to_string()),
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Audience-aware listing
// ---------------------------------------------------------------------------

/// Audience for tool listing, controlling which exposure levels are included.
///
/// - `Model`: Excludes `HarnessOnly` and `Hidden`. Safe for ordinary model-visible use.
/// - `Harness`: Includes `HarnessOnly` tools for selected profiles but excludes `Hidden`.
/// - `Debug`: Includes all non-hidden tools, including `ExpertOnly` and `HarnessOnly`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ToolListAudience {
    Model,
    Harness,
    Debug,
}

/// Filter tools by profile and audience.
///
/// For the `full` profile, hidden tools are always excluded.
/// For other profiles, only tools in the profile's `profiles` list are included.
/// The audience then further filters by exposure level.
pub fn tools_for_profile_audience(
    profile: &str,
    audience: ToolListAudience,
) -> Vec<&'static super::types::ToolSpec> {
    let profile_tools = tools_for_profile(profile);
    match audience {
        ToolListAudience::Model => profile_tools
            .into_iter()
            .filter(|t| t.exposure != ToolExposure::HarnessOnly)
            .collect(),
        ToolListAudience::Harness => profile_tools
            .into_iter()
            .filter(|t| t.exposure != ToolExposure::Hidden)
            .collect(),
        ToolListAudience::Debug => profile_tools,
    }
}

/// Get tool names for a profile and audience combination.
pub fn tool_names_for_profile_audience(
    profile: &str,
    audience: ToolListAudience,
) -> Vec<&'static str> {
    tools_for_profile_audience(profile, audience)
        .into_iter()
        .map(|spec| spec.name)
        .collect()
}

// ---------------------------------------------------------------------------
// Schema compaction
// ---------------------------------------------------------------------------

pub fn compact_input_schema(schema: &Value) -> Value {
    let obj = match schema.as_object() {
        Some(o) => o,
        None => return schema.clone(),
    };

    let mut compact = serde_json::Map::new();
    compact.insert(
        "type".to_string(),
        obj.get("type")
            .cloned()
            .unwrap_or_else(|| Value::String("object".to_string())),
    );

    if let Some(props) = obj.get("properties").and_then(|v| v.as_object()) {
        let mut compact_props = serde_json::Map::new();
        for (prop_name, prop_def) in props {
            if let Some(prop_obj) = prop_def.as_object() {
                let mut cp = serde_json::Map::new();
                if let Some(t) = prop_obj.get("type") {
                    cp.insert("type".to_string(), t.clone());
                }
                if let Some(e) = prop_obj.get("enum") {
                    cp.insert("enum".to_string(), e.clone());
                }
                if let Some(r) = prop_obj.get("required") {
                    cp.insert("required".to_string(), r.clone());
                }
                if let Some(items) = prop_obj.get("items") {
                    cp.insert("items".to_string(), items.clone());
                }
                for key in &[
                    "minimum",
                    "maximum",
                    "exclusiveMinimum",
                    "exclusiveMaximum",
                    "minLength",
                    "maxLength",
                    "pattern",
                    "minItems",
                    "maxItems",
                    "multipleOf",
                ] {
                    if let Some(v) = prop_obj.get(*key) {
                        cp.insert(key.to_string(), v.clone());
                    }
                }
                if let Some(desc) = prop_obj.get("description").and_then(|v| v.as_str()) {
                    let truncated = if desc.chars().count() > 80 {
                        format!("{}...", desc.chars().take(77).collect::<String>())
                    } else {
                        desc.to_string()
                    };
                    cp.insert("description".to_string(), Value::String(truncated));
                }
                compact_props.insert(prop_name.clone(), Value::Object(cp));
            } else {
                compact_props.insert(prop_name.clone(), prop_def.clone());
            }
        }
        compact.insert("properties".to_string(), Value::Object(compact_props));
    }

    if let Some(req) = obj.get("required") {
        compact.insert("required".to_string(), req.clone());
    }

    Value::Object(compact)
}

pub fn compact_output_schema(schema: &Value) -> Value {
    let obj = match schema.as_object() {
        Some(o) => o,
        None => return serde_json::json!({"type": "object"}),
    };

    let mut compact_output = serde_json::json!({"type": obj.get("type").unwrap_or(&Value::String("object".to_string()))});
    if let Some(props) = obj.get("properties").and_then(|v| v.as_object()) {
        let mut compact_props = serde_json::Map::new();
        for (key, prop) in props {
            let mut compact_prop = serde_json::json!({});
            if let Some(t) = prop.get("type") {
                compact_prop["type"] = t.clone();
            }
            if let Some(e) = prop.get("enum") {
                compact_prop["enum"] = e.clone();
            }
            compact_props.insert(key.clone(), compact_prop);
        }
        compact_output["properties"] = Value::Object(compact_props);
    }

    compact_output
}

// ---------------------------------------------------------------------------
// Suggestions
// ---------------------------------------------------------------------------

pub fn find_close_match<'a>(input: &str, tool_names: &[&'a str]) -> Option<&'a str> {
    if input.len() > 200 {
        return None;
    }
    let lower_input = input.to_lowercase();

    for &name in tool_names {
        if name.to_lowercase() == lower_input {
            return Some(name);
        }
    }

    fn at_word_boundary(sub: &str, s: &str) -> bool {
        if let Some(idx) = s.find(sub) {
            if idx == 0 {
                return true;
            }
            s.as_bytes().get(idx - 1) == Some(&b'_') || s.as_bytes().get(idx - 1) == Some(&b'-')
        } else {
            false
        }
    }

    let mut best_boundary: Option<(&str, usize)> = None;
    for &name in tool_names {
        let lower_name = name.to_lowercase();
        if at_word_boundary(&lower_input, &lower_name)
            || at_word_boundary(&lower_name, &lower_input)
        {
            let is_shorter = match best_boundary {
                Some((best_name, _)) => name.len() < best_name.len(),
                None => true,
            };
            if is_shorter {
                best_boundary = Some((name, 0));
            }
        }
    }
    if let Some((name, _)) = best_boundary {
        return Some(name);
    }

    let mut best: Option<(&str, usize)> = None;
    for &name in tool_names {
        let dist = levenshtein_distance(input, name);
        let threshold = input.chars().count().min(name.chars().count()) / 2;
        if dist <= threshold && best.is_none_or(|(_, best_dist)| dist < best_dist) {
            best = Some((name, dist));
        }
    }

    best.map(|(name, _)| name)
}
