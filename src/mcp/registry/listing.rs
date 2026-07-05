use std::collections::HashSet;

use crate::text::levenshtein_distance;
use serde_json::Value;

use super::all_tools::{all_tools as all_tools_slice, PROFILE_NAMES};
use super::types::{ToolExposure, ToolStability};

// ---------------------------------------------------------------------------
// Basic helpers
// ---------------------------------------------------------------------------

pub fn all_tools() -> &'static [super::types::ToolSpec] {
    all_tools_slice()
}

pub fn get_tool(name: &str) -> Option<&'static super::types::ToolSpec> {
    all_tools_slice().iter().find(|t| t.name == name)
}

pub fn tool_names() -> Vec<&'static str> {
    all_tools_slice().iter().map(|t| t.name).collect()
}

pub fn tools_for_profile(profile: &str) -> Vec<&'static super::types::ToolSpec> {
    if profile == "full" {
        return all_tools_slice()
            .iter()
            .filter(|t| t.exposure != ToolExposure::Hidden)
            .collect();
    }
    all_tools_slice()
        .iter()
        .filter(|t| t.profiles.contains(&profile))
        .collect()
}

pub fn available_profiles() -> &'static [&'static str] {
    PROFILE_NAMES
}

// ---------------------------------------------------------------------------
// Route-critical tool classification
// ---------------------------------------------------------------------------

/// Tools that participate in routing decisions and must emit structured
/// envelope fields (machine_code, verdict) on every successful response.
///
/// Route-critical tools are preflight/composite tools whose verdict drives
/// downstream action selection. Simple utility tools are NOT in this list
/// and should not be forced into artificial verdicts.
pub const ROUTE_CRITICAL_TOOLS: &[&str] = &[
    "edit_preflight",
    "command_preflight",
    "config_preflight",
    "patch_apply_check",
    "text_security_inspect",
];

/// Returns `true` if `name` is a route-critical tool.
pub fn is_route_critical(name: &str) -> bool {
    ROUTE_CRITICAL_TOOLS.contains(&name)
}

pub fn tool_handler_for(name: &str) -> Option<super::types::ToolHandler> {
    get_tool(name).map(|spec| spec.handler)
}

pub fn tool_count() -> usize {
    all_tools_slice().len()
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
    all_tools_slice()
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
// Tool list filtering (tools/list handler logic)
// ---------------------------------------------------------------------------

/// Options for filtering tool definitions in tools/list.
pub struct ToolListOptions<'a> {
    pub profile: &'a str,
    pub names: Option<&'a [String]>,
    pub tier: Option<u8>,
    pub tags: Option<&'a [String]>,
    pub schema_detail: &'a str,
}

/// Filter tool definitions by profile, names, tier, tags, and schema detail.
///
/// This is the core listing logic that was previously in server.rs.
/// It returns a `Vec<ToolDefinition>` ready for MCP serialization.
pub fn list_tool_definitions(options: ToolListOptions<'_>) -> Vec<super::types::ToolDefinition> {
    let profile_tools = tools_for_profile(options.profile);
    let mut tools: Vec<super::types::ToolDefinition> = profile_tools
        .into_iter()
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
        .collect();

    // Filter by names
    if let Some(names) = options.names {
        let name_set: HashSet<&str> = names.iter().map(|s| s.as_str()).collect();
        tools.retain(|t| name_set.contains(t.name.as_str()));
    }

    // Filter by tier
    if let Some(tier) = options.tier {
        tools.retain(|t| t.tier == Some(tier));
    }

    // Filter by tags (all specified tags must be present)
    if let Some(tags) = options.tags {
        let tag_set: HashSet<&str> = tags.iter().map(|s| s.as_str()).collect();
        tools.retain(|t| {
            if let Some(ref tool_tags) = t.tags {
                tag_set
                    .iter()
                    .all(|tag| tool_tags.iter().any(|tt| tt.as_str() == *tag))
            } else {
                false
            }
        });
    }

    if options.schema_detail == "compact" {
        for tool in &mut tools {
            // Truncate description to 120 chars
            if tool.description.chars().count() > 120 {
                let truncated: String = tool.description.chars().take(117).collect();
                tool.description = truncated;
                tool.description.push_str("...");
            }
            // Compact input schema: strip defaults, truncate property descriptions
            tool.input_schema = compact_input_schema(&tool.input_schema);
            // Compact output schema: keep top-level keys/types only
            if let Some(ref output) = tool.output_schema.clone() {
                tool.output_schema = Some(compact_output_schema(output));
            }
            // Python compact mode: drops tier and tags, keeps category/llm_exposure/cost
            tool.tier = None;
            tool.tags = None;
        }
    } else {
        // Non-compact mode: include deprecated field for all tools (Python parity)
        for tool in &mut tools {
            tool.deprecated = Some(tool.deprecated.unwrap_or(false));
        }
    }

    tools
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_all_tools_for_full_profile() {
        let options = ToolListOptions {
            profile: "full",
            names: None,
            tier: None,
            tags: None,
            schema_detail: "normal",
        };
        let tools = list_tool_definitions(options);
        assert!(!tools.is_empty());
        // full profile should exclude hidden tools
        for tool in &tools {
            assert_ne!(tool.llm_exposure.as_deref(), Some("hidden"));
        }
    }

    #[test]
    fn list_tool_definitions_names_filter() {
        let options = ToolListOptions {
            profile: "full",
            names: Some(&[String::from("math_eval"), String::from("text_equal")]),
            tier: None,
            tags: None,
            schema_detail: "normal",
        };
        let tools = list_tool_definitions(options);
        assert_eq!(tools.len(), 2);
        assert!(tools.iter().any(|t| t.name == "math_eval"));
        assert!(tools.iter().any(|t| t.name == "text_equal"));
    }

    #[test]
    fn list_tool_definitions_tier_filter() {
        let options = ToolListOptions {
            profile: "full",
            names: None,
            tier: Some(0),
            tags: None,
            schema_detail: "normal",
        };
        let tools = list_tool_definitions(options);
        for tool in &tools {
            assert_eq!(tool.tier, Some(0));
        }
    }

    #[test]
    fn list_tool_definitions_compact_schema() {
        let options = ToolListOptions {
            profile: "full",
            names: Some(&[String::from("math_eval")]),
            tier: None,
            tags: None,
            schema_detail: "compact",
        };
        let tools = list_tool_definitions(options);
        assert_eq!(tools.len(), 1);
        let tool = &tools[0];
        // Compact mode strips tier and tags
        assert_eq!(tool.tier, None);
        assert_eq!(tool.tags, None);
        // Description should be truncated if > 120 chars
        if tool.description.chars().count() > 120 {
            assert!(tool.description.ends_with("..."));
        }
    }

    #[test]
    fn list_tool_definitions_tags_filter() {
        let options = ToolListOptions {
            profile: "full",
            names: None,
            tier: None,
            tags: Some(&[String::from("math")]),
            schema_detail: "normal",
        };
        let tools = list_tool_definitions(options);
        for tool in &tools {
            let tags = tool.tags.as_ref().unwrap();
            assert!(
                tags.iter().any(|t| t == "math"),
                "tool {} missing 'math' tag",
                tool.name
            );
        }
    }

    #[test]
    fn list_tool_definitions_profile_filter() {
        let options = ToolListOptions {
            profile: "human_math",
            names: None,
            tier: None,
            tags: None,
            schema_detail: "normal",
        };
        let tools = list_tool_definitions(options);
        // human_math should have math tools
        assert!(tools.iter().any(|t| t.name == "math_eval"));
        // Should NOT have text tools
        assert!(!tools.iter().any(|t| t.name == "text_equal"));
    }
}
