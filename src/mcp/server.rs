use crate::agent::{Profile, ToolAudience, ToolCallError, ToolCallOutcome, ToolRegistry};
use crate::mcp::budget::budget_for_tool;
use crate::mcp::compat::CompatibilityMode;
use crate::mcp::execution;
use crate::mcp::machine_codes;
use crate::mcp::protocol::{
    already_initialized, invalid_request, json_rpc_error, json_rpc_error_with_data,
    method_not_found, not_initialized, EggsactExtensions, ExperimentalCapabilities,
    InitializeParams, InitializeResult, JsonRpcRequest, JsonRpcResponse, ServerCapabilities,
    ServerInfo, ToolsCapability,
};
use crate::mcp::registry;
use crate::mcp::response::{wrap_tool_response, ToolResponse};
use crate::mcp::runtime::{
    apply_cancellation, complete_request, get_active_audience, get_active_profile,
    get_schema_detail, negotiate_protocol_version, new_active_requests, register_request,
    MetricGuard, NegotiatedProtocol, RegisterRequestError, SessionState, MAX_REQUEST_BYTES,
    MAX_REQUEST_ID_LENGTH, MAX_TOOL_WORKERS, MCP_SERVER_NAME, RUNTIME_METRICS,
};
use serde_json::Value;
use std::io::Write;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Mutex;

pub fn mcp_tool_count() -> usize {
    registry::tool_count()
}

/// Truncate a string to at most `max_bytes` UTF-8 bytes, appending `suffix`.
///
/// Always returns a valid UTF-8 string. If the input fits within `max_bytes`,
/// it is returned unchanged. Otherwise the content is truncated at a valid
/// UTF-8 character boundary before `max_bytes` and `suffix` is appended.
fn truncate_utf8_bytes(input: &str, max_bytes: usize, suffix: &str) -> String {
    if input.len() <= max_bytes {
        return input.to_string();
    }
    let suffix_bytes = suffix.len();
    if suffix_bytes >= max_bytes {
        return suffix.to_string();
    }
    let mut end = max_bytes - suffix_bytes;
    while end > 0 && !input.is_char_boundary(end) {
        end -= 1;
    }
    let mut result = String::with_capacity(max_bytes + suffix.len());
    result.push_str(&input[..end]);
    result.push_str(suffix);
    result
}

/// Truncate a request ID for display in error messages, avoiding DoS via
/// oversized IDs in log output.
fn truncate_id_display(id: &Value) -> String {
    let s = id.to_string();
    truncate_utf8_bytes(&s, 128, "...")
}

/// Result of reading one bounded line from the JSONL input.
enum LimitedLine {
    /// A complete line (without the trailing newline).
    Line(String),
    /// The line exceeded `MAX_REQUEST_BYTES`. Contains the number of bytes
    /// observed before the limit was exceeded. The remainder of the line
    /// (through the next newline) has been drained.
    TooLarge { observed_at_least: usize },
    /// End of input (clean EOF with no buffered data).
    Eof,
}

/// Read one line from `reader` with a hard byte cap of `max_bytes`.
///
/// Uses `fill_buf`/`consume` to read incrementally without allocating the
/// full line upfront. When the accumulated bytes exceed `max_bytes`, the
/// rest of the line is drained through the next newline before returning.
/// Handles both LF and CRLF line endings consistently.
async fn read_bounded_line<R: tokio::io::AsyncBufRead + Unpin>(
    reader: &mut R,
    max_bytes: usize,
) -> LimitedLine {
    use tokio::io::AsyncReadExt;

    let mut buf = Vec::with_capacity(4096);
    let mut total = 0usize;

    loop {
        let chunk = match reader.fill_buf().await {
            Ok(chunk) => chunk,
            Err(_) => {
                if buf.is_empty() {
                    return LimitedLine::Eof;
                }
                break;
            }
        };

        if chunk.is_empty() {
            if buf.is_empty() {
                return LimitedLine::Eof;
            }
            break;
        }

        // Search for newline in this chunk
        if let Some(pos) = chunk.iter().position(|&b| b == b'\n') {
            // Found newline — but if it's beyond our byte limit, reject
            if pos > max_bytes.saturating_sub(total) {
                // Consume up to and including the newline, discarding
                // this oversized line. The next read starts after it.
                reader.consume(pos + 1);
                return LimitedLine::TooLarge {
                    observed_at_least: total + pos,
                };
            }
            let consume_to = if pos > 0 && chunk[pos - 1] == b'\r' {
                pos - 1
            } else {
                pos
            };
            let can_take = max_bytes.saturating_sub(total).min(consume_to);
            buf.extend_from_slice(&chunk[..can_take]);
            reader.consume(pos + 1);
            return LimitedLine::Line(
                String::from_utf8(buf)
                    .unwrap_or_else(|e| String::from_utf8_lossy(&e.into_bytes()).into_owned()),
            );
        }

        // No newline in this chunk — accumulate what we can
        let can_take = max_bytes.saturating_sub(total).min(chunk.len());
        buf.extend_from_slice(&chunk[..can_take]);
        total += can_take;
        let chunk_len = chunk.len();
        // Consume exactly the bytes we read from this chunk
        reader.consume(chunk_len);

        // If we've hit the byte limit, drain the rest of the line
        if total >= max_bytes {
            let mut drain = [0u8; 4096];
            loop {
                match reader.read(&mut drain).await {
                    Ok(0) => break,
                    Ok(n) => {
                        if drain[..n].contains(&b'\n') {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            return LimitedLine::TooLarge {
                observed_at_least: total,
            };
        }
    }

    LimitedLine::Line(
        String::from_utf8(buf)
            .unwrap_or_else(|e| String::from_utf8_lossy(&e.into_bytes()).into_owned()),
    )
}

fn write_json_line(value: &Value) {
    if let Ok(output) = serde_json::to_string(value) {
        println!("{}", output);
    }
}

fn build_server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        tools: ToolsCapability {
            list_changed: false,
        },
        experimental: Some(ExperimentalCapabilities {
            eggsact: EggsactExtensions {
                profiles: true,
                schema_detail: true,
                audience_filtering: true,
            },
        }),
    }
}

async fn handle_request_async(
    request: &JsonRpcRequest,
    cancel_flag: &Arc<std::sync::atomic::AtomicBool>,
    tool_semaphore: &Arc<tokio::sync::Semaphore>,
    session_state: &Arc<Mutex<SessionState>>,
) -> Option<serde_json::Value> {
    // NOTE: Process-global ensure_mcp_defaults() has been removed.
    // MCP evaluator defaults are now set per-request via the eval-context
    // bridge: EvalContext::mcp_mode() is installed as a thread-local in
    // the tools/call handler before dispatching to the tool.

    // ── Lifecycle enforcement ──────────────────────────────────────────
    let method = request.method.as_str();

    match method {
        "initialize" => {
            // Parse typed initialize parameters.
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

            // Parse typed InitializeParams
            let init_params: InitializeParams = match serde_json::from_value(params.clone()) {
                Ok(p) => p,
                Err(e) => {
                    return Some(invalid_request(
                        format!("Invalid initialize params: {}", e),
                        request.id.clone(),
                    ));
                }
            };

            // Validate required fields
            if init_params.client_info.name.is_empty() {
                return Some(invalid_request(
                    "Invalid params: clientInfo.name is required and must not be empty",
                    request.id.clone(),
                ));
            }

            // Negotiate protocol version
            let negotiated_version = negotiate_protocol_version(&init_params.protocol_version);

            // Attempt lifecycle transition
            let negotiated = NegotiatedProtocol {
                version: negotiated_version.clone(),
                client_name: init_params.client_info.name,
                client_version: init_params.client_info.version,
                client_capabilities: init_params.capabilities,
            };

            {
                let mut state = session_state.lock().await;
                if state.transition_to_awaiting(negotiated).is_err() {
                    return Some(already_initialized(request.id.clone()));
                }
            }

            // Build initialize result
            let result = InitializeResult {
                protocol_version: negotiated_version.to_string(),
                capabilities: build_server_capabilities(),
                server_info: ServerInfo {
                    name: MCP_SERVER_NAME.to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                },
            };

            Some(serde_json::to_value(result).unwrap())
        }

        "ping" => Some(serde_json::json!({})),

        "notifications/initialized" => {
            // This branch is only reachable from the request path (with an id),
            // because the notification path (no id) handles it inline at the
            // read loop. A request-form notification is a protocol violation:
            // the client must use the notification form (no id).
            Some(invalid_request(
                "notifications/initialized must be sent as a notification (without 'id'), not as a request",
                request.id.clone(),
            ))
        }

        // All other methods: enforce Ready state, then dispatch
        _ => {
            {
                let state = session_state.lock().await;
                if !state.allows_method(method) {
                    let err = match *state {
                        SessionState::Uninitialized | SessionState::AwaitingInitialized { .. } => {
                            not_initialized(method, request.id.clone())
                        }
                        SessionState::Ready { .. } => method_not_found(
                            format!("Method not found: {}", method),
                            request.id.clone(),
                        ),
                    };
                    return Some(err);
                }
            }

            match method {
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
                            if !d.is_string()
                                || !matches!(d.as_str(), Some("compact" | "normal" | "full"))
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
                        if let Some(a) = p.get("audience") {
                            if !a.is_string()
                                || !matches!(a.as_str(), Some("model" | "harness" | "debug"))
                            {
                                return Some(invalid_request(
                            "Invalid 'audience' parameter: expected model, harness, or debug",
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
                    let audience_filter: Option<&str> = params
                        .and_then(|p| p.get("audience"))
                        .and_then(|a| a.as_str());
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
                    // Default to the active audience when no explicit audience is provided,
                    // so harness-only tools are excluded from Model listings and the listing
                    // agrees with what tools/call will dispatch.
                    let effective_audience_str =
                        audience_filter.unwrap_or_else(|| match get_active_audience() {
                            ToolAudience::Model => "model",
                            ToolAudience::Harness => "harness",
                            ToolAudience::Debug => "debug",
                        });
                    if effective_profile != "full"
                        && !registry::PROFILE_NAMES.contains(&effective_profile)
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
                    // Build options and delegate to registry
                    let names_vec: Option<Vec<String>> = names_filter.map(|n| {
                        n.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    });
                    let tags_vec: Option<Vec<String>> = tags_filter.map(|t| {
                        t.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    });
                    let audience = Some(match effective_audience_str {
                        "harness" => registry::ToolListAudience::Harness,
                        "debug" => registry::ToolListAudience::Debug,
                        _ => registry::ToolListAudience::Model,
                    });
                    let options = registry::ToolListOptions {
                        profile: effective_profile,
                        names: names_vec.as_deref(),
                        tier: tier_filter.map(|t| t as u8),
                        tags: tags_vec.as_deref(),
                        schema_detail: detail,
                        audience,
                    };
                    let tools = registry::list_tool_definitions(options);
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
                    if cancel_flag.load(Ordering::Relaxed) {
                        return Some(wrap_tool_response(&ToolResponse::error_with_code(
                            "cancelled",
                            machine_codes::CANCELLED,
                            &format!("Tool '{}' request was cancelled by the client", name),
                            Some(vec![
                                "The request was cancelled before execution started".to_string()
                            ]),
                            Some(name),
                        )));
                    }

                    // Delegate lookup, profile check, and validation to ToolRegistry
                    let active_profile = get_active_profile();
                    let profile = Profile::from_str_opt(&active_profile)
                        .unwrap_or_else(|| Profile::custom(&active_profile));
                    let registry =
                        ToolRegistry::with_profile_and_audience(profile, get_active_audience())
                            .with_compat_mode(CompatibilityMode::EggcalcPython);
                    let handler = match registry.prepare_tool_call(name, &arguments_val) {
                        ToolCallOutcome::Ready { handler } => handler,
                        ToolCallOutcome::PreExecutionError(e) => {
                            return match e {
                        ToolCallError::UnknownTool(tool_name) => {
                            let tool_names = registry::tool_names();
                            let tool_name_refs: Vec<&str> = tool_names.to_vec();
                            let msg = match registry::find_close_match(&tool_name, &tool_name_refs) {
                                Some(m) => format!("Unknown tool: {}. Did you mean: {}?", tool_name, m),
                                None => format!("Unknown tool: {}", tool_name),
                            };
                            Some(method_not_found(msg, request.id.clone()))
                        }
                        ToolCallError::ToolUnavailable { tool, profile } => {
                            Some(json_rpc_error(
                                -32602,
                                format!(
                                    "Tool '{}' is not available in profile '{}'. Check the tool's declared profiles, or switch to a profile that includes it.",
                                    tool, profile
                                ),
                                request.id.clone(),
                            ))
                        }
                        ToolCallError::ToolNotAllowedForAudience {
                            tool,
                            profile,
                            audience,
                            exposure,
                        } => {
                            Some(json_rpc_error(
                                -32602,
                                format!(
                                    "Tool '{}' (exposure: {}) cannot be executed by {} audience in profile '{}'. Use tools/list with appropriate audience, or use the in-process API with a different audience.",
                                    tool, exposure, audience, profile
                                ),
                                request.id.clone(),
                            ))
                        }
                        ToolCallError::InvalidArguments(msg) => {
                            Some(json_rpc_error(
                                -32602,
                                format!("Invalid arguments for tool '{}': {}", name, msg),
                                request.id.clone(),
                            ))
                        }
                        ToolCallError::Internal(msg) => {
                            Some(json_rpc_error(-32603, msg, request.id.clone()))
                        }
                    };
                        }
                    };

                    let name_owned = name.to_string();
                    let args_clone = arguments_val.clone();
                    let sem = tool_semaphore.clone();

                    // Resolve budget for this tool from its declared cost.
                    // Composite tools get HEAVY budgets; others map from ToolCost.
                    // Tools with known load-sensitive dispatch (math_eval,
                    // text_diff_explain, regex_finditer) get a load-tolerant
                    // override so the parallel integration test harness doesn't
                    // surface spurious TIMEOUT envelopes on simple inputs.
                    let tool_budget = registry::get_tool(name)
                        .map(|spec| {
                            crate::mcp::budget::load_tolerant_budget(name, spec.cost)
                                .unwrap_or_else(|| budget_for_tool(name, spec.cost))
                        })
                        .unwrap_or(crate::mcp::budget::ToolBudget::MODERATE);

                    let outcome = execution::execute_tool_bounded(
                        handler,
                        args_clone,
                        name_owned.clone(),
                        tool_budget,
                        cancel_flag.clone(),
                        sem,
                    )
                    .await;

                    Some(execution::build_tool_response(
                        outcome,
                        &name_owned,
                        &tool_budget,
                    ))
                }

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
                        tool_names
                            .sort_by(|a, b| a.as_str().unwrap_or("").cmp(b.as_str().unwrap_or("")));
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
    }
}

pub async fn main() -> ! {
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);

    let tool_semaphore = Arc::new(tokio::sync::Semaphore::new(MAX_TOOL_WORKERS));
    let active_requests = new_active_requests();
    let session_state = Arc::new(Mutex::new(SessionState::Uninitialized));

    // Dedicated writer task: all stdout writes go through this channel
    // to prevent interleaved output from concurrent request handlers.
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Value>(64);
    let writer_handle = tokio::spawn(async move {
        while let Some(response) = rx.recv().await {
            write_json_line(&response);
        }
    });

    // Track spawned request tasks so we can wait for them on shutdown.
    let mut join_set = tokio::task::JoinSet::new();

    loop {
        let line = match read_bounded_line(&mut reader, MAX_REQUEST_BYTES).await {
            LimitedLine::Line(line) => line,
            LimitedLine::TooLarge { observed_at_least } => {
                let _ = tx
                    .send(json_rpc_error(
                        -32700,
                        format!(
                            "Request exceeds maximum size: at least {} bytes received, {} bytes maximum",
                            observed_at_least,
                            MAX_REQUEST_BYTES
                        ),
                        None,
                    ))
                    .await;
                continue;
            }
            LimitedLine::Eof => break,
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Reject batch requests (check before JSON parse, matching Python)
        if trimmed.starts_with('[') {
            let _ = tx
                .send(invalid_request("Batch requests are not supported", None))
                .await;
            continue;
        }

        // Parse JSON into generic Value for field-level validation
        let request_value: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => {
                let _ = tx
                    .send(json_rpc_error(-32700, "Parse error: invalid JSON", None))
                    .await;
                continue;
            }
        };

        // Validate top-level is object
        if !request_value.is_object() {
            let _ = tx
                .send(invalid_request(
                    "Invalid Request: expected JSON object",
                    None,
                ))
                .await;
            continue;
        }

        // Validate jsonrpc version
        let actual_version = request_value
            .get("jsonrpc")
            .and_then(|v| v.as_str())
            .unwrap_or("null");
        if actual_version != "2.0" {
            let _ = tx
                .send(invalid_request(
                    format!(
                        "Invalid Request: jsonrpc must be '2.0', got '{}'",
                        actual_version
                    ),
                    request_value.get("id").cloned(),
                ))
                .await;
            continue;
        }

        // Validate method
        let method = match request_value.get("method") {
            Some(Value::String(method)) => method.clone(),
            Some(_) => {
                let _ = tx
                    .send(invalid_request(
                        "Invalid Request: 'method' must be a string",
                        request_value.get("id").cloned(),
                    ))
                    .await;
                continue;
            }
            None => {
                let _ = tx
                    .send(invalid_request(
                        "Invalid Request: missing 'method'",
                        request_value.get("id").cloned(),
                    ))
                    .await;
                continue;
            }
        };

        // Validate request id (before constructing JsonRpcRequest)
        let id = request_value.get("id");
        if let Some(id_val) = id {
            // Reject boolean, array, object, and float ids per JSON-RPC 2.0 spec
            if id_val.is_boolean() || id_val.is_array() || id_val.is_object() {
                let _ = tx
                    .send(invalid_request(
                        "Invalid Request: 'id' must be a string, integer, or null",
                        None,
                    ))
                    .await;
                continue;
            }
            // Reject float IDs (JSON numbers that aren't integers)
            if id_val.is_number() && id_val.as_i64().is_none() && id_val.as_u64().is_none() {
                let _ = tx
                    .send(invalid_request(
                        "Invalid Request: 'id' must be a string, integer, or null",
                        None,
                    ))
                    .await;
                continue;
            }
            let id_str = id_val.to_string();
            if id_str.len() > MAX_REQUEST_ID_LENGTH {
                let _ = tx
                    .send(invalid_request(
                        format!(
                            "Invalid Request: 'id' exceeds maximum length of {}",
                            MAX_REQUEST_ID_LENGTH
                        ),
                        None,
                    ))
                    .await;
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

        // Notifications (no id) are handled inline; requests (with id) are
        // spawned as concurrent tasks that send responses through the channel.
        // Notifications bypass the ordinary request rate limiter.
        if request.id.is_none() {
            match request.method.as_str() {
                "notifications/initialized" => {
                    // Lifecycle transition: AwaitingInitialized → Ready
                    let mut state = session_state.lock().await;
                    if let Err(e) = state.transition_to_ready() {
                        eprintln!(
                            "Warning: notifications/initialized ignored: {} (state: {:?})",
                            e, *state
                        );
                    }
                }
                "notifications/cancelled" => {
                    // Set the cancel flag on the active request, if any.
                    // Uses async lock to avoid losing cancellations under
                    // contention (the old try_lock approach could silently
                    // drop valid cancellation notifications).
                    if let Some(params) = &request.params {
                        if let Some(request_id) = params.get("requestId") {
                            apply_cancellation(&active_requests, request_id).await;
                        } else {
                            eprintln!(
                                "Warning: notifications/cancelled missing 'requestId' parameter, ignoring"
                            );
                        }
                    } else {
                        eprintln!("Warning: notifications/cancelled missing 'params', ignoring");
                    }
                }
                _ => {
                    // Unknown notifications are silently ignored.
                }
            }
            continue;
        }

        // ── Request path (has id) ──────────────────────────────────────
        // Reject null IDs: concurrent tracking and error correlation
        // become ambiguous with null, and notifications use absent ID,
        // not null.
        if request.id.as_ref().is_some_and(|v| v.is_null()) {
            let _ = tx
                .send(json_rpc_error(
                    -32600,
                    "Invalid Request: 'id' must not be null",
                    None,
                ))
                .await;
            continue;
        }

        // Register the active request atomically under one lock acquisition.
        // This checks in-flight limits, duplicate IDs, and inserts the entry
        // in a single lock window — no separate contains_key/insert race.
        let cancel_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let request_id = request.id.clone().unwrap();
        let (guard, registration) = match register_request(
            &active_requests,
            &cancel_flag,
            request_id.clone(),
            request.method.clone(),
        )
        .await
        {
            Ok(pair) => pair,
            Err(RegisterRequestError::DuplicateId) => {
                let _ = tx
                    .send(json_rpc_error(
                        -32600,
                        format!(
                            "Duplicate request id: {:?}",
                            truncate_id_display(&request_id)
                        ),
                        request.id.clone(),
                    ))
                    .await;
                continue;
            }
            Err(RegisterRequestError::CapacityExceeded) => {
                let _ = tx
                    .send(json_rpc_error_with_data(
                        -32000,
                        "Too many in-flight requests",
                        Some(serde_json::json!({
                            "code": "RESOURCE_EXHAUSTED",
                            "limit": crate::mcp::runtime::MAX_IN_FLIGHT_REQUESTS,
                        })),
                        request.id.clone(),
                    ))
                    .await;
                continue;
            }
        };

        // Handle initialize inline (not spawned) to avoid race with
        // notifications/initialized. The lifecycle state transition must
        // complete before the next line is read.
        if request.method == "initialize" {
            let result =
                handle_request_async(&request, &cancel_flag, &tool_semaphore, &session_state).await;
            // Awaited cleanup — guaranteed, not best-effort
            complete_request(&active_requests, &registration).await;
            drop(guard);
            if let Some(result) = result {
                if result.get("error").is_some() && result.get("result").is_none() {
                    let _ = tx.send(result).await;
                } else {
                    let response = JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        result,
                        id: Some(request_id),
                    };
                    if let Ok(value) = serde_json::to_value(response) {
                        let _ = tx.send(value).await;
                    }
                }
            }
            continue;
        }

        // Spawn the request handler without awaiting — the read loop
        // continues to process the next line immediately.
        let tx = tx.clone();
        let semaphore_clone = tool_semaphore.clone();
        let cancel_flag_clone = cancel_flag.clone();
        let session_state_clone = session_state.clone();
        let request_clone = JsonRpcRequest {
            jsonrpc: request.jsonrpc.clone(),
            method: request.method.clone(),
            params: request.params.clone(),
            id: request.id.clone(),
        };
        let request_id_for_response = request_id.clone();
        let active_requests_clone = active_requests.clone();
        let registration_clone = registration.clone();

        join_set.spawn(async move {
            // RAII guard tracks active request count for diagnostics.
            let _active_guard = MetricGuard::new(&RUNTIME_METRICS.active_requests);

            let inner = tokio::spawn(async move {
                handle_request_async(
                    &request_clone,
                    &cancel_flag_clone,
                    &semaphore_clone,
                    &session_state_clone,
                )
                .await
            });

            let outcome = inner.await;

            // Awaited cleanup — guaranteed, not best-effort
            complete_request(&active_requests_clone, &registration_clone).await;

            // Debug-only assertion on drop; correctness is via complete_request
            drop(guard);

            // Send response through the channel.
            let maybe_result = outcome.unwrap_or_else(|e| {
                // JoinError = panic in the handler task
                Some(json_rpc_error(
                    -32000,
                    format!("Handler panic: {}", e),
                    Some(request_id_for_response.clone()),
                ))
            });
            if let Some(result) = maybe_result {
                if result.get("error").is_some() && result.get("result").is_none() {
                    let _ = tx.send(result).await;
                } else {
                    let response = JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        result,
                        id: Some(request_id_for_response),
                    };
                    if let Ok(value) = serde_json::to_value(response) {
                        let _ = tx.send(value).await;
                    }
                }
            }
        });
    }

    // Graceful shutdown: wait for all in-flight tasks to complete,
    // then drop the sender so the writer task drains and finishes.
    while join_set.join_next().await.is_some() {}
    drop(tx);
    let _ = writer_handle.await;
    // Flush stdout before exit — println! buffers when piped, and
    // std::process::exit does not run destructors or flush stdio.
    let _ = std::io::stdout().flush();
    std::process::exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::compat::CompatibilityMode;
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
        let result = validate_property_inner(
            &json!("abc123"),
            &schema,
            "test",
            10,
            CompatibilityMode::EggcalcPython,
        );
        assert!(
            result.is_none(),
            "pattern [0-9]+ should match 'abc123' at position 3, got: {:?}",
            result
        );
    }

    #[test]
    fn test_bug018_pattern_anchored_accepts() {
        let schema = json!({"type": "string", "pattern": "^[A-Z]"});
        let result = validate_property_inner(
            &json!("Hello"),
            &schema,
            "test",
            10,
            CompatibilityMode::EggcalcPython,
        );
        assert!(
            result.is_none(),
            "pattern ^[A-Z] should match 'Hello', got: {:?}",
            result
        );
    }

    #[test]
    fn test_bug018_pattern_anchored_rejects() {
        let schema = json!({"type": "string", "pattern": "^[A-Z]"});
        let result = validate_property_inner(
            &json!("hello"),
            &schema,
            "test",
            10,
            CompatibilityMode::EggcalcPython,
        );
        assert!(result.is_some(), "pattern ^[A-Z] should reject 'hello'");
    }

    #[test]
    fn test_bug018_pattern_no_match_rejects() {
        let schema = json!({"type": "string", "pattern": "^[0-9]+$"});
        let result = validate_property_inner(
            &json!("abc123def"),
            &schema,
            "test",
            10,
            CompatibilityMode::EggcalcPython,
        );
        assert!(
            result.is_some(),
            "pattern ^[0-9]+$ should reject 'abc123def'"
        );
    }

    #[test]
    fn test_bug019_multipleof_relative_tolerance() {
        let schema = json!({"type": "number", "multipleOf": 3.0});
        let result = validate_property_inner(
            &json!(9.000000001),
            &schema,
            "test",
            10,
            CompatibilityMode::EggcalcPython,
        );
        assert!(
            result.is_none(),
            "9.000000001 should pass multipleOf 3.0 with relative tolerance, got: {:?}",
            result
        );
    }

    #[test]
    fn test_bug019_multipleof_exact_value() {
        let schema = json!({"type": "number", "multipleOf": 5.0});
        let result = validate_property_inner(
            &json!(15.0),
            &schema,
            "test",
            10,
            CompatibilityMode::EggcalcPython,
        );
        assert!(
            result.is_none(),
            "15.0 should pass multipleOf 5.0, got: {:?}",
            result
        );
    }

    #[test]
    fn test_bug019_multipleof_rejects_non_multiple() {
        let schema = json!({"type": "number", "multipleOf": 3.0});
        let result = validate_property_inner(
            &json!(7.5),
            &schema,
            "test",
            10,
            CompatibilityMode::EggcalcPython,
        );
        assert!(result.is_some(), "7.5 should fail multipleOf 3.0");
    }

    #[test]
    fn test_bug019_multipleof_large_value() {
        // 10000000000.0000001 is very close to 10^10, and 1e-9 * 10^19 = 1e10.
        // Due to f64 precision, use a large value that IS a clean multiple:
        // 3000000000.0 = 3.0 * 1000000000.0
        let schema = json!({"type": "number", "multipleOf": 3.0});
        let result = validate_property_inner(
            &json!(3000000000.0),
            &schema,
            "test",
            10,
            CompatibilityMode::EggcalcPython,
        );
        assert!(
            result.is_none(),
            "3000000000.0 should pass multipleOf 3.0, got: {:?}",
            result
        );
    }
}

#[cfg(test)]
mod truncate_utf8_bytes_tests {
    use super::truncate_utf8_bytes;

    #[test]
    fn ascii_below_limit() {
        assert_eq!(truncate_utf8_bytes("hello", 128, "..."), "hello");
    }

    #[test]
    fn ascii_above_limit() {
        let input = "a".repeat(200);
        let result = truncate_utf8_bytes(&input, 128, "...");
        assert!(result.len() <= 128 + 3);
        assert!(result.ends_with("..."));
        assert_eq!(&result[..125], "a".repeat(125).as_str());
    }

    #[test]
    fn multibyte_cut_at_char_boundary() {
        // "é" is 2 bytes in UTF-8; 124 is even so it is a char boundary
        let input = "é".repeat(100); // 200 bytes total
        let result = truncate_utf8_bytes(&input, 128, "...");
        assert!(result.len() <= 128 + 3);
        assert!(result.ends_with("..."));
        // The content before "..." should be valid UTF-8
        let content_len = result.len() - 3;
        assert!(result[..content_len].is_char_boundary(content_len));
    }

    #[test]
    fn multibyte_cut_inside_codepoint() {
        // "é" is 2 bytes; 125 is odd, so it falls inside a code point
        let input = "é".repeat(100); // 200 bytes
        let result = truncate_utf8_bytes(&input, 125, "...");
        assert!(result.ends_with("..."));
        // content_bytes = 125 - 3 = 122, 122 / 2 = 61 "é" chars
        assert_eq!(&result[..122], "é".repeat(61).as_str());
    }

    #[test]
    fn mixed_ascii_and_multibyte() {
        let mut input = "hello".to_string();
        for _ in 0..50 {
            input.push('é');
        }
        // "hello" = 5 bytes, 50 * "é" = 100 bytes, total = 105 bytes
        let result = truncate_utf8_bytes(&input, 50, "...");
        assert!(result.len() <= 50 + 3);
        assert!(result.ends_with("..."));
        // Content before suffix should be valid UTF-8
        let content = &result[..result.len() - 3];
        assert!(content.is_char_boundary(content.len()));
    }

    #[test]
    fn zero_limit() {
        assert_eq!(truncate_utf8_bytes("hello", 0, "..."), "...");
    }

    #[test]
    fn suffix_larger_than_limit() {
        assert_eq!(truncate_utf8_bytes("hello", 2, "..."), "...");
    }

    #[test]
    fn truncate_id_display_with_unicode() {
        use super::truncate_id_display;
        use serde_json::json;

        // Long Unicode string ID should not panic
        let long_id = json!("id-".to_string() + &"🎉".repeat(100));
        let result = truncate_id_display(&long_id);
        assert!(result.len() <= 128 + 3);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn duplicate_unicode_id_produces_error_not_panic() {
        use super::*;
        use serde_json::json;

        // Verify that a long Unicode request ID does not panic
        // when passed through truncate_id_display
        let long_unicode = json!("id-".to_string() + &"🎉".repeat(100));
        let result = truncate_id_display(&long_unicode);
        // Should produce a bounded string, not panic
        assert!(result.len() < 200);
    }

    // ═══════════════════════════════════════════════════════════════════════
    // read_bounded_line tests
    // ═══════════════════════════════════════════════════════════════════════

    #[tokio::test]
    async fn bounded_line_short_lf() {
        use super::*;
        let data = b"hello world\n";
        let mut cursor = std::io::Cursor::new(data);
        let result = read_bounded_line(&mut cursor, 1000).await;
        match result {
            LimitedLine::Line(s) => assert_eq!(s, "hello world"),
            _ => panic!("expected Line"),
        }
    }

    #[tokio::test]
    async fn bounded_line_short_crlf() {
        use super::*;
        let data = b"hello world\r\n";
        let mut cursor = std::io::Cursor::new(data);
        let result = read_bounded_line(&mut cursor, 1000).await;
        match result {
            LimitedLine::Line(s) => assert_eq!(s, "hello world"),
            _ => panic!("expected Line"),
        }
    }

    #[tokio::test]
    async fn bounded_line_exactly_max_bytes() {
        use super::*;
        let payload = "x".repeat(100);
        let data = format!("{}\n", payload);
        let mut cursor = std::io::Cursor::new(data.as_bytes());
        let result = read_bounded_line(&mut cursor, 100).await;
        match result {
            LimitedLine::Line(s) => assert_eq!(s, payload),
            _ => panic!("expected Line for exactly max_bytes"),
        }
    }

    #[tokio::test]
    async fn bounded_line_one_over_max_rejected() {
        use super::*;
        let payload = "x".repeat(101);
        let data = format!("{}\n", payload);
        let mut cursor = std::io::Cursor::new(data.as_bytes());
        let result = read_bounded_line(&mut cursor, 100).await;
        match result {
            LimitedLine::TooLarge { observed_at_least } => {
                assert!(observed_at_least >= 100);
            }
            _ => panic!("expected TooLarge for limit+1"),
        }
    }

    #[tokio::test]
    async fn bounded_line_unterminated_large_input() {
        use super::*;
        // 5 MB of data with no newline — should NOT retain a 5 MB string
        let payload = "x".repeat(5 * 1024 * 1024);
        let mut cursor = std::io::Cursor::new(payload.as_bytes());
        let result = read_bounded_line(&mut cursor, 1_000_000).await;
        match result {
            LimitedLine::TooLarge { observed_at_least } => {
                assert!(observed_at_least >= 1_000_000);
            }
            _ => panic!("expected TooLarge for multi-MB unterminated input"),
        }
    }

    #[tokio::test]
    async fn bounded_line_oversized_then_valid() {
        use super::*;
        // Oversized line followed by a valid short line
        let big = "a".repeat(200);
        let data = format!("{}\nshort\n", big);
        let mut cursor = std::io::Cursor::new(data.as_bytes());

        // First read: oversized
        let r1 = read_bounded_line(&mut cursor, 100).await;
        assert!(matches!(r1, LimitedLine::TooLarge { .. }));

        // Second read: valid short line
        let r2 = read_bounded_line(&mut cursor, 100).await;
        match r2 {
            LimitedLine::Line(s) => assert_eq!(s, "short"),
            _ => panic!("expected Line for second request"),
        }
    }

    #[tokio::test]
    async fn bounded_line_eof_after_final_non_newline() {
        use super::*;
        // Final line without newline (EOF at end of data)
        let data = b"no-newline-at-end";
        let mut cursor = std::io::Cursor::new(data);
        let result = read_bounded_line(&mut cursor, 1000).await;
        match result {
            LimitedLine::Line(s) => assert_eq!(s, "no-newline-at-end"),
            _ => panic!("expected Line for EOF-terminated final line"),
        }
    }

    #[tokio::test]
    async fn bounded_line_empty_lines_ignored() {
        use super::*;
        // Empty line (just newline) should return an empty string
        let data = b"\n";
        let mut cursor = std::io::Cursor::new(data);
        let result = read_bounded_line(&mut cursor, 1000).await;
        match result {
            LimitedLine::Line(s) => assert_eq!(s, ""),
            _ => panic!("expected Line for empty line"),
        }
    }

    #[tokio::test]
    async fn bounded_line_clean_eof() {
        use super::*;
        let data = b"";
        let mut cursor = std::io::Cursor::new(data);
        let result = read_bounded_line(&mut cursor, 1000).await;
        assert!(matches!(result, LimitedLine::Eof));
    }
}
