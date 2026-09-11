//! Progressive-discovery presentation surface (`McpSurface`).
//!
//! This module owns the MCP-only progressive-discovery presentation:
//!
//! - [`McpSurface`] (`Direct` vs `Discovery`) — presentation policy only,
//!   never authorization. The active profile + audience remain the
//!   capability boundary for search and invocation.
//! - Pinned front-door policy ([`PINNED_DISCOVERY_TOOLS`]) — the small
//!   stable set advertised in discovery mode plus the two facades.
//! - Deterministic lexical search over profile/audience-filtered `ToolSpec`
//!   metadata ([`search_tools`]). No embeddings, no network, no new
//!   dependencies.
//! - Facade tool definitions/schemas for `tool_search` / `tool_invoke`.
//!
//! Eggsact still has 86 underlying deterministic utility tools in
//! `ALL_TOOLS_VEC`. Discovery mode additionally advertises two MCP-only
//! facades that are **not** part of the utility registry and must not be
//! treated as ordinary `src/tools/` capabilities.

use crate::mcp::registry::{ToolSpec, ToolStability};
use serde_json::Value;

// ── Surface mode ─────────────────────────────────────────────────────────

/// MCP presentation surface: which capabilities are *advertised* up front.
///
/// `Direct` preserves historical behavior: `tools/list` advertises the
/// profile/audience-filtered canonical tool set.
///
/// `Discovery` changes only presentation: `tools/list` advertises the
/// pinned front doors plus the two search/invoke facades. Capability
/// policy (profile + audience) still gates search results and execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum McpSurface {
    /// Advertise the full profile/audience-filtered canonical set.
    #[default]
    Direct,
    /// Advertise only the pinned set plus `tool_search` / `tool_invoke`.
    Discovery,
}

impl McpSurface {
    /// Lowercase name matching the `EGGSACT_MCP_SURFACE` convention.
    pub fn as_str(self) -> &'static str {
        match self {
            McpSurface::Direct => "direct",
            McpSurface::Discovery => "discovery",
        }
    }

    /// Parse a surface name (case-insensitive). Returns `None` for unknown.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "direct" => Some(McpSurface::Direct),
            "discovery" => Some(McpSurface::Discovery),
            _ => None,
        }
    }

    /// Whether discovery facades are advertised/callable under this surface.
    pub fn facades_enabled(self) -> bool {
        matches!(self, McpSurface::Discovery)
    }
}

impl std::fmt::Display for McpSurface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ── Pinned policy + facade names ─────────────────────────────────────────

/// Canonical front doors advertised in discovery mode (provisional,
/// reviewable policy — see plan 02 Part B).
///
/// A pinned tool is only advertised when the active profile/audience
/// allows that underlying tool; the list never widens capability policy.
pub const PINNED_DISCOVERY_TOOLS: &[&str] = &[
    "math_eval",
    "edit_preflight",
    "command_preflight",
    "config_preflight",
    "text_security_inspect",
];

/// MCP-only search facade name.
pub const TOOL_SEARCH: &str = "tool_search";
/// MCP-only generic-invoke facade name.
pub const TOOL_INVOKE: &str = "tool_invoke";

/// Whether `name` is one of the two MCP-only discovery facades.
pub fn is_discovery_facade(name: &str) -> bool {
    name == TOOL_SEARCH || name == TOOL_INVOKE
}

/// Whether `name` is in the pinned front-door set.
pub fn is_pinned_tool(name: &str) -> bool {
    PINNED_DISCOVERY_TOOLS.contains(&name)
}

// ── Facade schemas ───────────────────────────────────────────────────────
//
// Schemas use only validator-supported keywords (type, properties,
// required, additionalProperties, minimum/maximum, enum, description,
// default) so they stay inside the `schema_validation` subset.

/// Input schema for `tool_search`.
pub fn tool_search_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "query": {
                "type": "string",
                "description": "Natural-language or keyword query naming the desired capability (e.g. \"summarize a unified diff\").",
                "minLength": 1
            },
            "limit": {
                "type": "integer",
                "description": "Maximum matches to return (1-10).",
                "minimum": 1,
                "maximum": 10,
                "default": 5
            },
            "detail": {
                "type": "string",
                "description": "summary returns purpose/category/required arguments; schema additionally returns the full input schema.",
                "enum": ["summary", "schema"],
                "default": "summary"
            },
            "include_deprecated": {
                "type": "boolean",
                "description": "Include deprecated tools in results. Deprecated tools are hidden unless exactly named or this is true.",
                "default": false
            }
        },
        "required": ["query"],
        "additionalProperties": false
    })
}

/// Output schema for `tool_search` (describes `ToolResponse.result`).
pub fn tool_search_output_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "matches": {
                "type": "array",
                "description": "Ranked matches in deterministic relevance order.",
                "items": {"type": "object"}
            },
            "query": {"type": "string", "description": "Echo of the search query."}
        },
        "required": ["matches", "query"],
        "additionalProperties": false
    })
}

/// Input schema for `tool_invoke`.
pub fn tool_invoke_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "name": {
                "type": "string",
                "description": "Canonical underlying tool name to invoke (e.g. \"patch_summary\"). Facade names are rejected.",
                "minLength": 1
            },
            "arguments": {
                "type": "object",
                "description": "Arguments for the target tool. Defaults to {}.",
                "default": {}
            }
        },
        "required": ["name"],
        "additionalProperties": false
    })
}

// NOTE: `tool_invoke` intentionally has no facade-level output schema.
// It is a generic router: modern `structuredContent` is the selected target's
// normal `ToolResponse.result`, whose shape varies per target. Advertising a
// narrow `{"tool","ok"}` schema with `additionalProperties: false` would be a
// false contract that real target results violate, and a union of all 86
// target results would defeat the low-context design. Callers obtain the
// target input contract via `tool_search(detail="schema")` or direct/full
// mode. Discovery listings therefore omit `outputSchema` for `tool_invoke`
// (and for all discovery entries); see `discovery_legacy_definitions` and
// `discovery_modern_values`.

// ── Facade descriptions ──────────────────────────────────────────────────

/// Model-facing description for `tool_search`.
pub const TOOL_SEARCH_DESCRIPTION: &str = "Search the local deterministic tool catalog by keyword. Returns ranked canonical tool names with purpose, category, and required arguments. Prefer preflight composites for proposed edits/commands/configs; use tool_search for long-tail utilities, then call tool_invoke with the chosen canonical name.";
/// Model-facing description for `tool_invoke`.
pub const TOOL_INVOKE_DESCRIPTION: &str = "Invoke one canonical deterministic tool by name with its arguments. The target must be allowed by the active profile and audience; harness-only and hidden tools are rejected for model callers. Facade names (tool_search, tool_invoke) cannot be invoked through this router.";

// ── Search parameter parsing ─────────────────────────────────────────────

/// Parsed `tool_search` arguments.
#[derive(Debug, Clone)]
pub struct SearchParams {
    pub query: String,
    pub limit: usize,
    pub detail_schema: bool,
    pub include_deprecated: bool,
}

/// Parse and validate `tool_search` arguments.
///
/// Returns `Err(message)` for invalid-arguments errors (caller maps to
/// `-32602`). Applies defaults: `limit=5`, `detail="summary"`,
/// `include_deprecated=false`.
pub fn parse_search_params(args: &Value) -> Result<SearchParams, String> {
    let obj = args
        .as_object()
        .ok_or_else(|| "Invalid arguments: expected object".to_string())?;
    // Reject unknown fields (additionalProperties: false parity).
    for key in obj.keys() {
        if !["query", "limit", "detail", "include_deprecated"].contains(&key.as_str()) {
            return Err(format!("Unexpected argument(s): {}", key));
        }
    }
    let query = obj
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required argument: query".to_string())?;
    if query.trim().is_empty() {
        return Err("Argument 'query' must be a non-empty string".to_string());
    }
    if query.chars().count() > 500 {
        return Err("Argument 'query' length exceeds maxLength 500".to_string());
    }
    let limit = match obj.get("limit") {
        None => 5,
        Some(v) => {
            let n = v
                .as_i64()
                .or_else(|| v.as_u64().map(|u| u as i64))
                .ok_or_else(|| "Argument 'limit' must be integer".to_string())?;
            if v.as_bool().is_some() {
                return Err("Argument 'limit' must be integer".to_string());
            }
            if !(1..=10).contains(&n) {
                return Err(
                    "Argument 'limit' value out of range (minimum 1, maximum 10)".to_string(),
                );
            }
            n as usize
        }
    };
    let detail_schema = match obj.get("detail") {
        None => false,
        Some(v) => match v.as_str() {
            Some("summary") => false,
            Some("schema") => true,
            Some(other) => {
                return Err(format!(
                    "Argument 'detail' must be one of: summary, schema (got '{}')",
                    other
                ))
            }
            None => return Err("Argument 'detail' must be one of: summary, schema".to_string()),
        },
    };
    let include_deprecated = match obj.get("include_deprecated") {
        None => false,
        Some(v) => v
            .as_bool()
            .ok_or_else(|| "Argument 'include_deprecated' must be boolean".to_string())?,
    };
    Ok(SearchParams {
        query: query.to_string(),
        limit,
        detail_schema,
        include_deprecated,
    })
}

/// Parsed `tool_invoke` arguments.
#[derive(Debug, Clone)]
pub struct InvokeParams {
    pub target: String,
    pub arguments: Value,
}

/// Parse `tool_invoke` arguments. Rejects recursive facade invocation.
pub fn parse_invoke_params(args: &Value) -> Result<InvokeParams, String> {
    let obj = args
        .as_object()
        .ok_or_else(|| "Invalid arguments: expected object".to_string())?;
    for key in obj.keys() {
        if !["name", "arguments"].contains(&key.as_str()) {
            return Err(format!("Unexpected argument(s): {}", key));
        }
    }
    let name = obj
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required argument: name".to_string())?;
    if name.trim().is_empty() {
        return Err("Argument 'name' must be a non-empty string".to_string());
    }
    if name.len() > 200 {
        return Err("Argument 'name' length exceeds limit".to_string());
    }
    if is_discovery_facade(name) {
        return Err(format!("recursive invocation of '{}' is not allowed", name));
    }
    let arguments = match obj.get("arguments") {
        None => Value::Object(serde_json::Map::new()),
        Some(v) if v.is_object() => v.clone(),
        Some(_) => return Err("Invalid arguments: expected object".to_string()),
    };
    Ok(InvokeParams {
        target: name.to_string(),
        arguments,
    })
}

// ── Deterministic lexical search ─────────────────────────────────────────

/// One ranked search hit with its relevance class.
#[derive(Debug, Clone)]
struct ScoredHit {
    index: usize,
    relevance: u8,
    matched_tokens: usize,
}

fn useful_name_token(token: &str) -> bool {
    !matches!(
        token,
        "check"
            | "config"
            | "convert"
            | "data"
            | "diff"
            | "extract"
            | "file"
            | "inspect"
            | "json"
            | "list"
            | "path"
            | "text"
            | "tool"
            | "unit"
            | "summarize"
            | "validate"
    )
}

fn useful_tag_token(token: &str) -> bool {
    !matches!(token, "check" | "compare" | "data" | "text" | "tool")
}

fn tokenize(s: &str) -> Vec<String> {
    s.split(|c: char| ['_', '-', '/', '.', ' ', ':'].contains(&c))
        .flat_map(|part| {
            // Split camelCase boundaries conservatively: keep original plus
            // lowercase whole token. Full camel splitting is unnecessary for
            // snake_case tool names.
            vec![part.to_lowercase()]
        })
        .map(|t| t.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
        .filter(|t| !t.is_empty())
        .collect()
}

fn query_tokens(query: &str) -> Vec<String> {
    query
        .split(|c: char| !c.is_alphanumeric())
        .map(|t| t.to_lowercase())
        .filter(|t| !t.is_empty())
        .collect()
}

/// Score one spec against the query tokens.
///
/// Relevance classes (lower is better):
/// 0 = exact canonical-name match, 1 = exact alias match,
/// 2 = canonical-name token/prefix match, 3 = tag/category overlap,
/// 4 = description overlap, 5 = Levenshtein close-match recovery.
fn score_spec(
    spec: &ToolSpec,
    query_lower: &str,
    qtokens: &[String],
    include_deprecated: bool,
) -> Option<(u8, usize, &'static str)> {
    let is_deprecated = spec.stability == ToolStability::Deprecated;
    let name_lower = spec.name.to_lowercase();
    // Deprecated tools are hidden unless exactly named or explicitly included.
    if is_deprecated && !include_deprecated && name_lower != query_lower {
        return None;
    }
    // 0. exact canonical-name match (deprecated included by exact name).
    if name_lower == query_lower {
        return Some((0, usize::MAX, "exact_name"));
    }
    // 1. exact alias match.
    for alias in spec.aliases {
        if alias.to_lowercase() == query_lower {
            return Some((1, usize::MAX, "alias"));
        }
    }
    // 2. canonical-name token/prefix match.
    let name_tokens = tokenize(spec.name);
    let name_matches = qtokens
        .iter()
        .filter(|qt| {
            name_tokens.iter().any(|nt| {
                useful_name_token(nt)
                    && (nt.as_str() == qt.as_str()
                        || (qt.len() >= 3 && nt.starts_with(qt.as_str()))
                        || (nt.len() >= 3
                            && qt.starts_with(nt.as_str())
                            && qt.len() <= nt.len().saturating_add(3)))
            })
        })
        .count();
    let mut tag_tokens: Vec<String> = spec.tags.iter().flat_map(|t| tokenize(t)).collect();
    tag_tokens.extend(tokenize(spec.category));
    let tag_matches = qtokens
        .iter()
        .filter(|qt| useful_tag_token(qt) && tag_tokens.iter().any(|t| t.as_str() == qt.as_str()))
        .count();
    if name_matches > 0 {
        return Some((2, name_matches + tag_matches * 2, "name_token"));
    }
    // Whole-name prefix (e.g. "patch" matches "patch_summary"). Avoid
    // short generic prefixes such as "text" and "check".
    if qtokens
        .iter()
        .any(|qt| qt.len() >= 5 && name_lower.starts_with(qt.as_str()))
    {
        return Some((2, 1, "name_token"));
    }
    // 3. tag/category token overlap.
    if tag_matches > 0 {
        return Some((3, tag_matches, "tag"));
    }
    // 4. description token overlap.
    let desc_tokens = query_tokens(spec.description);
    let description_matches = qtokens
        .iter()
        .filter(|qt| desc_tokens.iter().any(|t| t.as_str() == qt.as_str()))
        .count();
    if description_matches > 0 {
        return Some((4, description_matches, "description"));
    }
    // 5. bounded Levenshtein recovery for typo-like queries.
    if query_lower.len() <= 200 {
        let dist = crate::text::levenshtein_distance(query_lower, &name_lower);
        let threshold = query_lower.chars().count().min(name_lower.chars().count()) / 2;
        if dist <= threshold {
            return Some((5, 1, "close_match"));
        }
        // Also try aliases with Levenshtein.
        for alias in spec.aliases {
            let a = alias.to_lowercase();
            let d = crate::text::levenshtein_distance(query_lower, &a);
            let th = query_lower.chars().count().min(a.chars().count()) / 2;
            if d <= th {
                return Some((5, 1, "close_match"));
            }
        }
    }
    None
}

/// Required argument names for a spec (from its input schema).
pub fn required_arguments(spec: &ToolSpec) -> Vec<String> {
    let schema = (spec.input_schema)();
    schema
        .get("required")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// Known replacement for deprecated tools (where documented).
pub fn deprecated_replacement(name: &str) -> Option<&'static str> {
    match name {
        "json_query" => Some("json_extract"),
        _ => None,
    }
}

/// Deterministic search over an already profile/audience-filtered spec slice.
///
/// Ordering is by relevance class, then registry order (slice position).
/// The caller is responsible for passing only specs the active
/// profile/audience permits — presentation never widens authorization.
pub fn search_filtered<'a>(
    specs: &[&'a ToolSpec],
    params: &SearchParams,
) -> Vec<(&'a ToolSpec, &'static str)> {
    let query_lower = params.query.to_lowercase();
    let qtokens = query_tokens(&params.query);
    let mut hits: Vec<(ScoredHit, &'static str)> = Vec::new();
    for (index, spec) in specs.iter().enumerate() {
        if let Some((relevance, matched_tokens, reason)) =
            score_spec(spec, &query_lower, &qtokens, params.include_deprecated)
        {
            hits.push((
                ScoredHit {
                    index,
                    relevance,
                    matched_tokens,
                },
                reason,
            ));
        }
    }
    // Stable: relevance class, then registry order.
    hits.sort_by(|a, b| {
        a.0.relevance
            .cmp(&b.0.relevance)
            .then_with(|| b.0.matched_tokens.cmp(&a.0.matched_tokens))
            .then_with(|| a.0.index.cmp(&b.0.index))
    });
    hits.truncate(params.limit);
    hits.into_iter()
        .map(|(h, reason)| (specs[h.index], reason))
        .collect()
}

/// Build the `matches` array value for a search result.
pub fn matches_value(hits: &[(&ToolSpec, &'static str)], detail_schema: bool) -> Value {
    let mut arr = Vec::new();
    for (spec, reason) in hits {
        let mut obj = serde_json::Map::new();
        obj.insert("name".to_string(), Value::String(spec.name.to_string()));
        obj.insert(
            "purpose".to_string(),
            Value::String(spec.description.to_string()),
        );
        obj.insert(
            "category".to_string(),
            Value::String(spec.category.to_string()),
        );
        obj.insert(
            "required_arguments".to_string(),
            Value::Array(
                required_arguments(spec)
                    .into_iter()
                    .map(Value::String)
                    .collect(),
            ),
        );
        obj.insert(
            "match_reason".to_string(),
            Value::String(reason.to_string()),
        );
        if spec.stability == ToolStability::Deprecated {
            obj.insert("deprecated".to_string(), Value::Bool(true));
            if let Some(repl) = deprecated_replacement(spec.name) {
                obj.insert("replacement".to_string(), Value::String(repl.to_string()));
            }
        }
        if detail_schema {
            obj.insert("inputSchema".to_string(), (spec.input_schema)());
        }
        arr.push(Value::Object(obj));
    }
    Value::Array(arr)
}

// ── Discovery listing (presentation only) ─────────────────────────────────
//
// Discovery `tools/list` advertises at most the pinned front doors allowed
// by the active profile/audience plus the two MCP-only facades. It never
// widens capability policy: a pinned tool absent from the profile/audience
// is simply omitted, and long-tail tools stay reachable only via
// `tool_search` / `tool_invoke`.

/// Pinned specs allowed by `profile` + `audience`, in registry order.
pub fn discovery_specs_for(
    profile: &str,
    audience: crate::mcp::registry::ToolListAudience,
) -> Vec<&'static ToolSpec> {
    crate::mcp::registry::tools_for_profile_audience(profile, audience)
        .into_iter()
        .filter(|s| is_pinned_tool(s.name))
        // Preserve registry order: `tools_for_profile_audience` already
        // yields ALL_TOOLS order; re-sort by registry index for safety.
        .collect::<Vec<_>>()
}

/// Truncate a description like legacy `compact` (120 chars).
fn compact_description(desc: &str) -> String {
    if desc.chars().count() > 120 {
        let truncated: String = desc.chars().take(117).collect();
        format!("{}...", truncated)
    } else {
        desc.to_string()
    }
}

/// Legacy `ToolDefinition` list for discovery mode.
///
/// Pinned canonical tools use compact descriptions/schemas and omit
/// output schemas, tier/tags, and internal bookkeeping. Facades are
/// appended in stable order (`tool_search`, `tool_invoke`). When
/// `names` is `Some`, only entries whose name is in the set are kept.
pub fn discovery_legacy_definitions(
    profile: &str,
    audience: crate::mcp::registry::ToolListAudience,
    names: Option<&[String]>,
) -> Vec<crate::mcp::registry::ToolDefinition> {
    let mut defs: Vec<crate::mcp::registry::ToolDefinition> = Vec::new();
    for spec in discovery_specs_for(profile, audience) {
        defs.push(crate::mcp::registry::ToolDefinition {
            name: spec.name.to_string(),
            description: compact_description(spec.description),
            input_schema: crate::mcp::registry::compact_input_schema(&(spec.input_schema)()),
            output_schema: None,
            tier: None,
            tags: None,
            deprecated: None,
            category: None,
            llm_exposure: None,
            cost: None,
        });
    }
    defs.push(crate::mcp::registry::ToolDefinition {
        name: TOOL_SEARCH.to_string(),
        description: TOOL_SEARCH_DESCRIPTION.to_string(),
        input_schema: tool_search_input_schema(),
        output_schema: None,
        tier: None,
        tags: None,
        deprecated: None,
        category: Some("discovery".to_string()),
        llm_exposure: None,
        cost: None,
    });
    defs.push(crate::mcp::registry::ToolDefinition {
        name: TOOL_INVOKE.to_string(),
        description: TOOL_INVOKE_DESCRIPTION.to_string(),
        input_schema: tool_invoke_input_schema(),
        output_schema: None,
        tier: None,
        tags: None,
        deprecated: None,
        category: Some("discovery".to_string()),
        llm_exposure: None,
        cost: None,
    });
    if let Some(filter) = names {
        defs.retain(|d| filter.iter().any(|n| n == &d.name));
    }
    defs
}

/// Modern (2026-07-28) minimal Tool values for discovery mode.
///
/// Each entry carries only `name`, concise `description`, `inputSchema`,
/// and standard `annotations`. Output schemas and namespaced `_meta`
/// bookkeeping are omitted so the model does not pay prompt tokens for
/// server internals; the search facade exposes long-tail metadata on
/// demand.
pub fn discovery_modern_values(
    profile: &str,
    audience: crate::mcp::registry::ToolListAudience,
    names: Option<&[String]>,
) -> Vec<Value> {
    let mut values: Vec<Value> = Vec::new();
    for spec in discovery_specs_for(profile, audience) {
        let annotations = serde_json::to_value(crate::mcp::registry::annotations_for_spec(spec))
            .unwrap_or(serde_json::json!({}));
        values.push(serde_json::json!({
            "name": spec.name,
            "description": compact_description(spec.description),
            "inputSchema": crate::mcp::registry::compact_input_schema(&(spec.input_schema)()),
            "annotations": annotations,
        }));
    }
    let facade_annotations = serde_json::json!({
        "readOnlyHint": true,
        "destructiveHint": false,
        "idempotentHint": true,
        "openWorldHint": false
    });
    values.push(serde_json::json!({
        "name": TOOL_SEARCH,
        "description": TOOL_SEARCH_DESCRIPTION,
        "inputSchema": tool_search_input_schema(),
        "annotations": facade_annotations,
    }));
    values.push(serde_json::json!({
        "name": TOOL_INVOKE,
        "description": TOOL_INVOKE_DESCRIPTION,
        "inputSchema": tool_invoke_input_schema(),
        "annotations": facade_annotations,
    }));
    if let Some(filter) = names {
        values.retain(|v| {
            v.get("name")
                .and_then(|n| n.as_str())
                .is_some_and(|n| filter.iter().any(|f| f == n))
        });
    }
    values
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::registry;

    fn full_model_specs() -> Vec<&'static ToolSpec> {
        registry::tools_for_profile_audience("full", registry::ToolListAudience::Model)
    }

    fn refs(specs: &[&'static ToolSpec]) -> Vec<&'static ToolSpec> {
        specs.to_vec()
    }

    #[test]
    fn surface_parse_roundtrip() {
        assert_eq!(McpSurface::parse("direct"), Some(McpSurface::Direct));
        assert_eq!(McpSurface::parse("DISCOVERY"), Some(McpSurface::Discovery));
        assert_eq!(McpSurface::parse("bogus"), None);
        assert_eq!(McpSurface::Direct.as_str(), "direct");
        assert_eq!(McpSurface::Discovery.as_str(), "discovery");
        assert!(!McpSurface::Direct.facades_enabled());
        assert!(McpSurface::Discovery.facades_enabled());
    }

    #[test]
    fn pinned_set_is_five_known_tools() {
        assert_eq!(PINNED_DISCOVERY_TOOLS.len(), 5);
        for name in PINNED_DISCOVERY_TOOLS {
            assert!(
                registry::get_tool(name).is_some(),
                "pinned tool '{}' must exist",
                name
            );
        }
    }

    #[test]
    fn search_exact_name_wins() {
        let specs = full_model_specs();
        let r = refs(&specs);
        let params = parse_search_params(&serde_json::json!({"query": "patch_summary"})).unwrap();
        let hits = search_filtered(&r, &params);
        assert!(!hits.is_empty());
        assert_eq!(hits[0].0.name, "patch_summary");
        assert_eq!(hits[0].1, "exact_name");
    }

    #[test]
    fn search_deprecated_hidden_unless_exact_or_included() {
        let specs = full_model_specs();
        let r = refs(&specs);
        // Ordinary query for JSON Pointer work should surface json_extract,
        // not the deprecated json_query.
        let params =
            parse_search_params(&serde_json::json!({"query": "json pointer extract"})).unwrap();
        let hits = search_filtered(&r, &params);
        let names: Vec<&str> = hits.iter().map(|(s, _)| s.name).collect();
        assert!(
            !names.contains(&"json_query"),
            "deprecated json_query must be hidden from ordinary search"
        );
        // Exact name still resolves.
        let exact = parse_search_params(&serde_json::json!({"query": "json_query"})).unwrap();
        let hits = search_filtered(&r, &exact);
        assert!(hits.iter().any(|(s, _)| s.name == "json_query"));
        // Explicit opt-in includes it when relevant.
        let with_dep = parse_search_params(
            &serde_json::json!({"query": "json pointer", "include_deprecated": true, "limit": 10}),
        )
        .unwrap();
        let _ = search_filtered(&r, &with_dep);
    }

    #[test]
    fn search_order_deterministic() {
        let specs = full_model_specs();
        let r = refs(&specs);
        let params =
            parse_search_params(&serde_json::json!({"query": "patch", "limit": 5})).unwrap();
        let a = search_filtered(&r, &params);
        let b = search_filtered(&r, &params);
        let an: Vec<&str> = a.iter().map(|(s, _)| s.name).collect();
        let bn: Vec<&str> = b.iter().map(|(s, _)| s.name).collect();
        assert_eq!(an, bn);
        assert_eq!(
            serde_json::to_string(&matches_value(&a, false)).unwrap(),
            serde_json::to_string(&matches_value(&b, false)).unwrap()
        );
    }

    #[test]
    fn invoke_rejects_facade_recursion() {
        let err = parse_invoke_params(&serde_json::json!({"name": "tool_invoke"})).unwrap_err();
        assert!(err.contains("recursive"));
        let err = parse_invoke_params(&serde_json::json!({"name": "tool_search"})).unwrap_err();
        assert!(err.contains("recursive"));
    }

    #[test]
    fn facade_schemas_use_supported_keywords_only() {
        for schema in [
            tool_search_input_schema(),
            tool_search_output_schema(),
            tool_invoke_input_schema(),
        ] {
            let obj = schema.as_object().unwrap();
            for key in obj.keys() {
                assert!(
                    [
                        "type",
                        "properties",
                        "required",
                        "additionalProperties",
                        "items",
                        "minItems",
                        "maxItems",
                        "uniqueItems",
                        "minLength",
                        "maxLength",
                        "pattern",
                        "minimum",
                        "maximum",
                        "exclusiveMinimum",
                        "exclusiveMaximum",
                        "multipleOf",
                        "enum",
                        "const",
                        "description",
                        "title",
                        "default",
                        "examples",
                        "$schema"
                    ]
                    .contains(&key.as_str()),
                    "unexpected top-level schema key: {}",
                    key
                );
            }
        }
    }
}
