use crate::mcp::protocol::{JsonRpcError, JsonRpcErrorDetail, JsonRpcRequest, JsonRpcResponse};
use crate::mcp::registry;
use crate::mcp::response::{sanitize_error, ToolResponse};
use crate::mcp::runtime::{
    self, get_active_profile, get_schema_detail, CancelledRequests, RateLimiter, MAX_OUTPUT_BYTES,
    MAX_REQUESTS_PER_SECOND, MAX_REQUEST_BYTES, MAX_REQUEST_ID_LENGTH, MAX_TOOL_TIMEOUT_SECONDS,
    MAX_TOOL_WORKERS, MCP_PROTOCOL_VERSION, MCP_SERVER_NAME,
};
use crate::mcp::schema_validation::validate_arguments;
use crate::text::levenshtein_distance;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Mutex;

fn get_profile_tools(profile: &str) -> Vec<&'static str> {
    registry::tools_for_profile(profile)
        .into_iter()
        .map(|spec| spec.name)
        .collect()
}

fn list_tools() -> Vec<ToolDefinition> {
    registry::mcp_tool_definitions()
}

pub fn mcp_tool_count() -> usize {
    registry::tool_count()
}

fn compact_input_schema(schema: &Value) -> Value {
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

    // Compact each property: keep only whitelist of keys (matching Python)
    if let Some(props) = obj.get("properties").and_then(|v| v.as_object()) {
        let mut compact_props = serde_json::Map::new();
        for (prop_name, prop_def) in props {
            if let Some(prop_obj) = prop_def.as_object() {
                let mut cp = serde_json::Map::new();
                // Keep type
                if let Some(t) = prop_obj.get("type") {
                    cp.insert("type".to_string(), t.clone());
                }
                // Keep enum
                if let Some(e) = prop_obj.get("enum") {
                    cp.insert("enum".to_string(), e.clone());
                }
                // Keep required sub-fields
                if let Some(r) = prop_obj.get("required") {
                    cp.insert("required".to_string(), r.clone());
                }
                // Keep items for arrays
                if let Some(items) = prop_obj.get("items") {
                    cp.insert("items".to_string(), items.clone());
                }
                // Keep numeric constraints
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
                // Truncated description
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

    // Keep required at top level
    if let Some(req) = obj.get("required") {
        compact.insert("required".to_string(), req.clone());
    }

    Value::Object(compact)
}

fn compact_output_schema(schema: &Value) -> Value {
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

#[derive(Serialize, Debug, Clone)]
pub struct ToolMetadata {
    pub category: &'static str,
    pub tier: u8,
    pub profiles: &'static [&'static str],
    pub tags: &'static [&'static str],
    pub llm_exposure: &'static str,
    pub harness_use: &'static [&'static str],
    pub aliases: &'static [&'static str],
    pub cost: &'static str,
    pub stability: &'static str,
    pub composite: bool,
}

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

fn find_close_match<'a>(input: &str, tool_names: &[&'a str]) -> Option<&'a str> {
    if input.len() > 200 {
        return None;
    }
    let lower_input = input.to_lowercase();

    // First check for exact case-insensitive match
    for &name in tool_names {
        if name.to_lowercase() == lower_input {
            return Some(name);
        }
    }

    // Check for word boundary matches (both directions, like Python)
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
            // Python returns the shortest tool name when there are ties
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

    // Compute edit distance with threshold
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

fn escape_ascii_json(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii() {
            result.push(c);
        } else {
            let mut utf16 = [0u16; 2];
            for unit in c.encode_utf16(&mut utf16).iter() {
                result.push_str(&format!("\\u{:04x}", unit));
            }
        }
    }
    result
}

fn python_json_dumps<T: Serialize>(value: &T) -> String {
    struct PythonStyleFormatter;

    impl serde_json::ser::Formatter for PythonStyleFormatter {
        fn begin_array_value<W: std::io::Write + ?Sized>(
            &mut self,
            writer: &mut W,
            first: bool,
        ) -> std::io::Result<()> {
            if first {
                Ok(())
            } else {
                writer.write_all(b", ")
            }
        }

        fn begin_object_key<W: std::io::Write + ?Sized>(
            &mut self,
            writer: &mut W,
            first: bool,
        ) -> std::io::Result<()> {
            if first {
                Ok(())
            } else {
                writer.write_all(b", ")
            }
        }

        fn begin_object_value<W: std::io::Write + ?Sized>(
            &mut self,
            writer: &mut W,
        ) -> std::io::Result<()> {
            writer.write_all(b": ")
        }
    }

    let mut buf = Vec::new();
    {
        let mut serializer = serde_json::Serializer::with_formatter(&mut buf, PythonStyleFormatter);
        if value.serialize(&mut serializer).is_err() {
            return String::new();
        }
    }
    let serialized = String::from_utf8(buf).unwrap_or_default();
    escape_ascii_json(&serialized)
}

fn wrap_tool_response(tool_response: &ToolResponse) -> serde_json::Value {
    let text = python_json_dumps(tool_response);
    if tool_response.ok {
        serde_json::json!({
            "content": [{"type": "text", "text": text}],
        })
    } else {
        serde_json::json!({
            "content": [{"type": "text", "text": text}],
            "isError": true,
        })
    }
}

fn json_rpc_error(code: i32, message: impl Into<String>, id: Option<Value>) -> Value {
    serde_json::to_value(JsonRpcError {
        jsonrpc: "2.0".to_string(),
        error: JsonRpcErrorDetail {
            code,
            message: message.into(),
        },
        id,
    })
    .unwrap_or_else(|_| {
        serde_json::json!({
            "jsonrpc": "2.0",
            "error": {"code": -32603, "message": "Internal error: failed to serialize error response"},
            "id": null
        })
    })
}

fn invalid_request(message: impl Into<String>, id: Option<Value>) -> Value {
    json_rpc_error(-32600, message, id)
}

fn method_not_found(message: impl Into<String>, id: Option<Value>) -> Value {
    json_rpc_error(-32601, message, id)
}

fn write_json_line(value: &Value) {
    if let Ok(output) = serde_json::to_string(value) {
        println!("{}", output);
    }
}

async fn handle_request_async(
    request: &JsonRpcRequest,
    cancelled: &Arc<tokio::sync::Mutex<CancelledRequests>>,
    tool_semaphore: &Arc<tokio::sync::Semaphore>,
) -> Option<serde_json::Value> {
    // Ensure MCP-safe evaluator defaults are in effect. Idempotent: a one-time
    // check is enough to set mcp_mode and disable random/side-effect functions.
    runtime::ensure_mcp_defaults();

    match request.method.as_str() {
        "initialize" => Some(
            serde_json::to_value(crate::mcp::protocol::InitializeResult {
                protocol_version: MCP_PROTOCOL_VERSION.to_string(),
                capabilities: crate::mcp::protocol::Capabilities {
                    tools: crate::mcp::protocol::ToolsCapability {
                        list_changed: false,
                    },
                },
                server_info: crate::mcp::protocol::ServerInfo {
                    name: MCP_SERVER_NAME.to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                },
            })
            .unwrap(),
        ),

        "tools/list" => {
            let params = request.params.as_ref();
            if let Some(p) = params {
                if !p.is_object() {
                    return Some(invalid_request(
                        "Invalid params: expected object",
                        request.id.clone(),
                    ));
                }
            }
            // Validate param types (matching Python messages exactly)
            if let Some(p) = params {
                if let Some(d) = p.get("schema_detail") {
                    if !d.is_string() || !matches!(d.as_str(), Some("compact" | "normal" | "full"))
                    {
                        return Some(invalid_request(
                            "Invalid 'schema_detail' parameter: expected compact, normal, or full",
                            request.id.clone(),
                        ));
                    }
                }
                if let Some(t) = p.get("tier") {
                    // Python treats bool as int (isinstance(True, int) == True)
                    if !t.is_i64() && !t.is_u64() && !t.is_boolean() {
                        return Some(invalid_request(
                            "Invalid 'tier' parameter: expected integer",
                            request.id.clone(),
                        ));
                    }
                }
                if let Some(t) = p.get("tags") {
                    match t.as_array() {
                        Some(tags) if tags.iter().all(|v| v.is_string()) => {}
                        Some(_) => {
                            return Some(invalid_request(
                                "Invalid 'tags' parameter: all items must be strings",
                                request.id.clone(),
                            ));
                        }
                        None => {
                            return Some(invalid_request(
                                "Invalid 'tags' parameter: expected array",
                                request.id.clone(),
                            ));
                        }
                    }
                }
                if let Some(n) = p.get("names") {
                    match n.as_array() {
                        Some(names) if names.iter().all(|v| v.is_string()) => {}
                        Some(_) => {
                            return Some(invalid_request(
                                "Invalid 'names' parameter: all items must be strings",
                                request.id.clone(),
                            ));
                        }
                        None => {
                            return Some(invalid_request(
                                "Invalid 'names' parameter: expected array",
                                request.id.clone(),
                            ));
                        }
                    }
                }
                if let Some(pr) = p.get("profile") {
                    if !pr.is_string() {
                        return Some(invalid_request(
                            "Invalid 'profile' parameter: expected string",
                            request.id.clone(),
                        ));
                    }
                }
            }
            let schema_detail = get_schema_detail();
            let detail = params
                .and_then(|p| p.get("schema_detail"))
                .and_then(|d| d.as_str())
                .unwrap_or(&schema_detail);
            let names_filter = params
                .and_then(|p| p.get("names"))
                .and_then(|n| n.as_array());
            let profile_filter = params
                .and_then(|p| p.get("profile"))
                .and_then(|p| p.as_str());
            let tier_filter = params.and_then(|p| p.get("tier")).and_then(|t| {
                // Python treats bool as int (isinstance(True, int) == True)
                match t {
                    Value::Number(n) => n.as_u64(),
                    Value::Bool(b) => Some(if *b { 1 } else { 0 }),
                    _ => None,
                }
            });
            let tags_filter = params
                .and_then(|p| p.get("tags"))
                .and_then(|t| t.as_array());

            let active_profile = get_active_profile();
            let effective_profile = profile_filter.unwrap_or(&active_profile);
            if effective_profile != "full" && !registry::PROFILE_NAMES.contains(&effective_profile)
            {
                let available = registry::PROFILE_NAMES.join(", ");
                return Some(serde_json::json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32602,
                        "message": format!("Unknown MCP profile: '{}'. Available profiles: {}", effective_profile, available)
                    },
                    "id": request.id
                }));
            }
            let profile_tools = get_profile_tools(effective_profile);
            let profile_set: HashSet<&str> = profile_tools.into_iter().collect();

            let mut tools = list_tools();

            // Filter by profile
            tools.retain(|t| profile_set.contains(t.name.as_str()));

            // Filter by names
            if let Some(names) = names_filter {
                let name_set: HashSet<&str> = names.iter().filter_map(|n| n.as_str()).collect();
                tools.retain(|t| name_set.contains(t.name.as_str()));
            }

            // Filter by tier
            if let Some(tier) = tier_filter {
                tools.retain(|t| t.tier == Some(tier as u8));
            }

            // Filter by tags (all specified tags must be present)
            if let Some(tags) = tags_filter {
                let tag_set: HashSet<&str> = tags.iter().filter_map(|t| t.as_str()).collect();
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

            if detail == "compact" {
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

            Some(serde_json::json!({"tools": tools}))
        }

        "tools/call" => {
            let params = match request.params.as_ref() {
                Some(p) => {
                    if !p.is_object() {
                        return Some(invalid_request(
                            "Invalid params: expected object",
                            request.id.clone(),
                        ));
                    }
                    p
                }
                None => {
                    return Some(invalid_request(
                        "Invalid params: expected object",
                        request.id.clone(),
                    ));
                }
            };
            let name = match params.get("name").and_then(|v| v.as_str()) {
                Some(n) => n,
                None => {
                    return Some(invalid_request(
                        "Invalid params: missing tool name",
                        request.id.clone(),
                    ));
                }
            };
            let arguments_val = match params.get("arguments") {
                Some(v) if v.is_object() => v.clone(),
                Some(_) => {
                    return Some(invalid_request(
                        "Invalid arguments: expected object",
                        request.id.clone(),
                    ));
                }
                None => serde_json::Value::Object(serde_json::Map::new()),
            };

            // Check if request was cancelled before execution
            if let Some(ref id) = request.id {
                let mut cancelled_set = cancelled.lock().await;
                if cancelled_set.contains(id) {
                    // Remove from cancelled set so reuse of same id won't re-trigger
                    cancelled_set.remove(id);
                    return Some(wrap_tool_response(&ToolResponse::error(
                        "cancelled",
                        &format!("Tool '{}' request was cancelled", name),
                        None,
                        Some(name),
                    )));
                }
            }

            // Look up the tool handler (exact match only)
            let canonical_name = name.to_string();
            let handler = match registry::tool_handler_for(name) {
                Some(handler) => handler,
                None => {
                    // Unknown tool — return -32601 (matching Python)
                    let tool_names = registry::tool_names();
                    let tool_name_refs: Vec<&str> = tool_names.to_vec();
                    let msg = match find_close_match(name, &tool_name_refs) {
                        Some(m) => format!("Unknown tool: {}. Did you mean: {}?", name, m),
                        None => format!("Unknown tool: {}", name),
                    };
                    return Some(method_not_found(msg, request.id.clone()));
                }
            };

            // Enforce active profile: reject tools not in the current profile
            let profile_tools = get_profile_tools(&get_active_profile());
            if !profile_tools.contains(&&*canonical_name) {
                return Some(json_rpc_error(
                    -32602,
                    format!(
                        "Tool '{}' is not available in profile '{}'. Use tools/list to see available tools, or switch profile.",
                        canonical_name,
                        get_active_profile()
                    ),
                    request.id.clone(),
                ));
            }

            if let Some(msg) = validate_arguments(&canonical_name, &arguments_val) {
                return Some(json_rpc_error(
                    -32602,
                    format!("Invalid arguments for tool '{}': {}", canonical_name, msg),
                    request.id.clone(),
                ));
            }

            let name_owned = canonical_name.to_string();
            let args_clone = arguments_val.clone();
            let sem = tool_semaphore.clone();

            let result =
                tokio::time::timeout(Duration::from_secs(MAX_TOOL_TIMEOUT_SECONDS), async move {
                    let _permit = sem
                        .acquire()
                        .await
                        .expect("tool semaphore unexpectedly closed");
                    tokio::task::spawn_blocking(move || handler(&args_clone)).await
                })
                .await;

            match result {
                Ok(Ok(tool_response)) => {
                    // Check output size
                    let output = python_json_dumps(&tool_response);
                    if output.is_empty() {
                        Some(wrap_tool_response(&ToolResponse::error(
                            "serialization_error",
                            "Failed to serialize tool response",
                            None,
                            Some(&name_owned),
                        )))
                    } else if output.len() > MAX_OUTPUT_BYTES {
                        Some(wrap_tool_response(
                            &ToolResponse::error(
                                "output_too_large",
                                &format!(
                                    "Output exceeds {} bytes and was truncated",
                                    MAX_OUTPUT_BYTES
                                ),
                                Some(vec![
                                    "Try reducing input size or using a summary/detail option"
                                        .to_string(),
                                ]),
                                Some(&name_owned),
                            )
                            .with_warnings(vec![
                                "Output was truncated due to size limit".to_string(),
                            ]),
                        ))
                    } else {
                        Some(wrap_tool_response(&tool_response))
                    }
                }
                Ok(Err(join_err)) => Some(json_rpc_error(
                    -32000,
                    format!(
                        "Tool execution error: {}",
                        runtime::truncate_2000(&sanitize_error(&join_err.to_string()))
                    ),
                    request.id.clone(),
                )),
                Err(_timeout) => Some(wrap_tool_response(&ToolResponse::error(
                    "timeout",
                    &format!(
                        "Tool '{}' execution timed out after {}s",
                        name_owned, MAX_TOOL_TIMEOUT_SECONDS
                    ),
                    Some(vec!["Try a simpler input or shorter text".to_string()]),
                    Some(&name_owned),
                ))),
            }
        }

        "notifications/initialized" => None,

        "notifications/cancelled" => {
            if let Some(params) = &request.params {
                if let Some(request_id) = params.get("requestId") {
                    // Validate type: must be str or int, not bool
                    match request_id {
                        Value::Bool(_) => {}
                        Value::String(s) => {
                            if s.len() <= MAX_REQUEST_ID_LENGTH {
                                let mut cancelled_set = cancelled.lock().await;
                                cancelled_set.insert(request_id.clone());
                            }
                        }
                        Value::Number(n)
                            if (n.is_i64() || n.is_u64())
                                && request_id.to_string().len() <= MAX_REQUEST_ID_LENGTH =>
                        {
                            let mut cancelled_set = cancelled.lock().await;
                            cancelled_set.insert(request_id.clone());
                        }
                        _ => {}
                    }
                }
            }
            None
        }

        "ping" => Some(serde_json::json!({})),

        "profiles/list" => {
            if let Some(ref params) = request.params {
                if !params.is_object() {
                    return Some(invalid_request(
                        "Invalid params: expected object",
                        request.id.clone(),
                    ));
                }
            }
            let active = get_active_profile();
            let mut profiles_info = serde_json::Map::new();
            for &name in registry::PROFILE_NAMES {
                let tool_specs = registry::tools_for_profile(name);
                let mut tool_names: Vec<Value> = tool_specs
                    .into_iter()
                    .map(|spec| Value::String(spec.name.to_string()))
                    .collect();
                tool_names.sort_by(|a, b| a.as_str().unwrap_or("").cmp(b.as_str().unwrap_or("")));
                profiles_info.insert(
                    name.to_string(),
                    serde_json::json!({
                        "tools": tool_names,
                        "tool_count": tool_names.len(),
                    }),
                );
            }
            Some(serde_json::json!({
                "active_profile": active,
                "profiles": serde_json::Value::Object(profiles_info),
                "available_profiles": registry::PROFILE_NAMES,
            }))
        }

        _ => {
            let display_method = if request.method.len() > 100 {
                // Python truncates by byte length: method[:100]
                let truncated = &request.method.as_bytes()[..100];
                // Find a valid UTF-8 boundary
                let mut end = truncated.len();
                while end > 0 && !request.method.is_char_boundary(end) {
                    end -= 1;
                }
                format!("{}...", &request.method[..end])
            } else {
                request.method.clone()
            };
            Some(method_not_found(
                format!("Method not found: {}", display_method),
                request.id.clone(),
            ))
        }
    }
}

pub async fn main() -> ! {
    let stdin = tokio::io::stdin();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();

    let rate_limiter = Arc::new(Mutex::new(RateLimiter::new()));
    let cancelled = Arc::new(tokio::sync::Mutex::new(CancelledRequests::new()));
    let tool_semaphore = Arc::new(tokio::sync::Semaphore::new(MAX_TOOL_WORKERS));

    loop {
        let line = match lines.next_line().await {
            Ok(Some(line)) => line,
            Ok(None) | Err(_) => break,
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Request size limit
        if trimmed.len() > MAX_REQUEST_BYTES {
            write_json_line(&json_rpc_error(
                -32700,
                format!(
                    "Request exceeds maximum size of {} bytes",
                    MAX_REQUEST_BYTES
                ),
                None,
            ));
            continue;
        }

        // Reject batch requests (check before JSON parse, matching Python)
        if trimmed.starts_with('[') {
            write_json_line(&invalid_request("Batch requests are not supported", None));
            continue;
        }

        // Parse JSON into generic Value for field-level validation
        let request_value: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => {
                write_json_line(&json_rpc_error(-32700, "Parse error: invalid JSON", None));
                continue;
            }
        };

        // Validate top-level is object
        if !request_value.is_object() {
            write_json_line(&invalid_request(
                "Invalid Request: expected JSON object",
                None,
            ));
            continue;
        }

        // Validate jsonrpc version
        let actual_version = request_value
            .get("jsonrpc")
            .and_then(|v| v.as_str())
            .unwrap_or("null");
        if actual_version != "2.0" {
            write_json_line(&invalid_request(
                format!(
                    "Invalid Request: jsonrpc must be '2.0', got '{}'",
                    actual_version
                ),
                request_value.get("id").cloned(),
            ));
            continue;
        }

        // Validate method
        let method = match request_value.get("method") {
            Some(Value::String(method)) => method.clone(),
            Some(_) => {
                write_json_line(&invalid_request(
                    "Invalid Request: 'method' must be a string",
                    request_value.get("id").cloned(),
                ));
                continue;
            }
            None => {
                write_json_line(&invalid_request(
                    "Invalid Request: missing 'method'",
                    request_value.get("id").cloned(),
                ));
                continue;
            }
        };

        // Rate limiting
        {
            let mut limiter = rate_limiter.lock().await;
            if !limiter.check() {
                write_json_line(&invalid_request(
                    format!(
                        "Rate limit exceeded: max {} requests per second",
                        MAX_REQUESTS_PER_SECOND
                    ),
                    request_value.get("id").cloned(),
                ));
                continue;
            }
        }

        // Validate request id
        let id = request_value.get("id");
        if let Some(id_val) = id {
            // Reject boolean, array, object, and float ids per JSON-RPC 2.0 spec
            if id_val.is_boolean() || id_val.is_array() || id_val.is_object() {
                write_json_line(&invalid_request(
                    "Invalid Request: 'id' must be a string, integer, or null",
                    None,
                ));
                continue;
            }
            // Reject float IDs (JSON numbers that aren't integers)
            // Use as_i64()/as_u64() for exact integer detection — as_f64() loses
            // precision for integers >2^53 and would silently accept them.
            if id_val.is_number() && id_val.as_i64().is_none() && id_val.as_u64().is_none() {
                write_json_line(&invalid_request(
                    "Invalid Request: 'id' must be a string, integer, or null",
                    None,
                ));
                continue;
            }
            let id_str = id_val.to_string();
            if id_str.len() > MAX_REQUEST_ID_LENGTH {
                write_json_line(&invalid_request(
                    format!(
                        "Invalid Request: 'id' exceeds maximum length of {}",
                        MAX_REQUEST_ID_LENGTH
                    ),
                    None,
                ));
                continue;
            }
        }

        // Construct JsonRpcRequest from validated value
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method,
            params: request_value.get("params").cloned(),
            id: id.cloned(),
        };

        // Handle notifications (no id) and requests (with id)
        let maybe_result = {
            let request_clone = JsonRpcRequest {
                jsonrpc: request.jsonrpc.clone(),
                method: request.method.clone(),
                params: request.params.clone(),
                id: request.id.clone(),
            };
            let cancelled_clone = cancelled.clone();
            let semaphore_clone = tool_semaphore.clone();
            let handle = tokio::spawn(async move {
                handle_request_async(&request_clone, &cancelled_clone, &semaphore_clone).await
            });
            match handle.await {
                Ok(result) => result,
                Err(join_err) => {
                    let msg = if join_err.is_cancelled() {
                        "task cancelled".to_string()
                    } else {
                        let panic_msg = join_err.into_panic();
                        match panic_msg.downcast_ref::<&str>() {
                            Some(s) => s.to_string(),
                            None => match panic_msg.downcast_ref::<String>() {
                                Some(s) => s.clone(),
                                None => "unknown error".to_string(),
                            },
                        }
                    };
                    Some(json_rpc_error(
                        -32603,
                        runtime::truncate_2000(&sanitize_error(&format!(
                            "Internal error: {}",
                            msg
                        ))),
                        request.id.clone(),
                    ))
                }
            }
        };
        if let Some(result) = maybe_result {
            // Check if this is already a JSON-RPC error (has "error" key at top level)
            if result.get("error").is_some() && result.get("result").is_none() {
                // Already a JSON-RPC error response, output directly
                write_json_line(&result);
            } else {
                let response = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    result,
                    id: request.id,
                };

                if let Ok(value) = serde_json::to_value(response) {
                    write_json_line(&value);
                }
            }
        }
    }

    std::process::exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::schema_validation::validate_property_inner;
    use serde_json::json;
    use std::collections::HashSet;

    #[test]
    fn tool_registration_tables_are_in_sync() {
        let definitions = registry::mcp_tool_definitions();
        let mut definition_names = HashSet::new();
        for tool in &definitions {
            assert!(
                definition_names.insert(tool.name.as_str()),
                "duplicate tool definition: {}",
                tool.name
            );
        }

        let registry_names = registry::tool_names();
        for &name in &registry_names {
            assert!(
                definition_names.contains(name),
                "registry tool lacks definition: {name}"
            );
            assert!(
                registry::tool_handler_for(name).is_some(),
                "registry tool lacks handler: {name}"
            );
        }

        for name in &definition_names {
            assert!(
                registry_names.contains(name),
                "tool definition lacks registry entry: {name}"
            );
        }

        assert_eq!(mcp_tool_count(), registry::tool_count());
    }

    #[test]
    fn test_bug018_pattern_matches_anywhere_in_string() {
        let schema = json!({"type": "string", "pattern": "[0-9]+"});
        let result = validate_property_inner(&json!("abc123"), &schema, "test", 10);
        assert!(
            result.is_none(),
            "pattern [0-9]+ should match 'abc123' at position 3, got: {:?}",
            result
        );
    }

    #[test]
    fn test_bug018_pattern_anchored_accepts() {
        let schema = json!({"type": "string", "pattern": "^[A-Z]"});
        let result = validate_property_inner(&json!("Hello"), &schema, "test", 10);
        assert!(
            result.is_none(),
            "pattern ^[A-Z] should match 'Hello', got: {:?}",
            result
        );
    }

    #[test]
    fn test_bug018_pattern_anchored_rejects() {
        let schema = json!({"type": "string", "pattern": "^[A-Z]"});
        let result = validate_property_inner(&json!("hello"), &schema, "test", 10);
        assert!(result.is_some(), "pattern ^[A-Z] should reject 'hello'");
    }

    #[test]
    fn test_bug018_pattern_no_match_rejects() {
        let schema = json!({"type": "string", "pattern": "^[0-9]+$"});
        let result = validate_property_inner(&json!("abc123def"), &schema, "test", 10);
        assert!(
            result.is_some(),
            "pattern ^[0-9]+$ should reject 'abc123def'"
        );
    }

    #[test]
    fn test_bug019_multipleof_relative_tolerance() {
        let schema = json!({"type": "number", "multipleOf": 3.0});
        let result = validate_property_inner(&json!(9.000000001), &schema, "test", 10);
        assert!(
            result.is_none(),
            "9.000000001 should pass multipleOf 3.0 with relative tolerance, got: {:?}",
            result
        );
    }

    #[test]
    fn test_bug019_multipleof_exact_value() {
        let schema = json!({"type": "number", "multipleOf": 5.0});
        let result = validate_property_inner(&json!(15.0), &schema, "test", 10);
        assert!(
            result.is_none(),
            "15.0 should pass multipleOf 5.0, got: {:?}",
            result
        );
    }

    #[test]
    fn test_bug019_multipleof_rejects_non_multiple() {
        let schema = json!({"type": "number", "multipleOf": 3.0});
        let result = validate_property_inner(&json!(7.5), &schema, "test", 10);
        assert!(result.is_some(), "7.5 should fail multipleOf 3.0");
    }

    #[test]
    fn test_bug019_multipleof_large_value() {
        // 10000000000.0000001 is very close to 10^10, and 1e-9 * 10^19 = 1e10.
        // Due to f64 precision, use a large value that IS a clean multiple:
        // 3000000000.0 = 3.0 * 1000000000.0
        let schema = json!({"type": "number", "multipleOf": 3.0});
        let result = validate_property_inner(&json!(3000000000.0), &schema, "test", 10);
        assert!(
            result.is_none(),
            "3000000000.0 should pass multipleOf 3.0, got: {:?}",
            result
        );
    }
}
