use crate::agent::{Profile, ToolAudience, ToolCallError, ToolCallOutcome, ToolRegistry};
use crate::mcp::budget::{budget_for_tool, BudgetContext};
use crate::mcp::compat::CompatibilityMode;
use crate::mcp::machine_codes;
use crate::mcp::protocol::{
    already_initialized, invalid_request, json_rpc_error, method_not_found, not_initialized,
    EggsactExtensions, ExperimentalCapabilities, InitializeParams, InitializeResult,
    JsonRpcRequest, JsonRpcResponse, ServerCapabilities, ServerInfo, ToolsCapability,
};
use crate::mcp::registry;
use crate::mcp::response::{
    python_json_dumps, sanitize_error, truncate_response, wrap_tool_response, ToolResponse,
};
use crate::mcp::runtime::{
    self, apply_cancellation, get_active_audience, get_active_profile, get_schema_detail,
    negotiate_protocol_version, new_active_requests, register_request, MetricGuard,
    NegotiatedProtocol, RateLimiter, RegisterRequestError, SessionState, MAX_OUTPUT_BYTES,
    MAX_REQUESTS_PER_SECOND, MAX_REQUEST_BYTES, MAX_REQUEST_ID_LENGTH, MAX_TOOL_WORKERS,
    MCP_SERVER_NAME, RUNTIME_METRICS,
};
use serde_json::Value;
use std::io::Write;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Mutex;

/// Handler lifecycle states for timeout metric accounting.
const HANDLER_RUNNING: u8 = 0;
const HANDLER_TIMED_OUT: u8 = 1;
const HANDLER_FINISHED: u8 = 2;

pub fn mcp_tool_count() -> usize {
    registry::tool_count()
}

/// Truncate a request ID for display in error messages, avoiding DoS via
/// oversized IDs in log output.
fn truncate_id_display(id: &Value) -> String {
    let s = id.to_string();
    if s.len() > 128 {
        format!("{}...", &s[..124])
    } else {
        s
    }
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
                    let cancel_flag_for_handler = cancel_flag.clone();
                    let budget_context =
                        BudgetContext::new(tool_budget).with_cancellation(cancel_flag.clone());

                    // Use budget-derived timeout. The outer tokio::time::timeout
                    // governs how long we wait; the spawned blocking task may
                    // continue after the timeout fires (Rust cannot kill threads).
                    //
                    // Owned permits ensure the semaphore permit is held until the
                    // blocking handler actually exits — even after a timeout. This
                    // prevents timed-out handlers from releasing their permit while
                    // the blocking closure continues, which would allow more than
                    // MAX_TOOL_WORKERS concurrent blocking computations.
                    let timeout_ms = tool_budget.max_elapsed_ms;
                    // Track handler lifecycle for timeout metric accounting.
                    let handler_lifecycle =
                        Arc::new(std::sync::atomic::AtomicU8::new(HANDLER_RUNNING));
                    let handler_lifecycle_for_handler = handler_lifecycle.clone();
                    let result =
                        tokio::time::timeout(Duration::from_millis(timeout_ms), async move {
                            let permit = match sem.acquire_owned().await {
                                Ok(p) => p,
                                Err(_) => {
                                    return Ok::<_, tokio::task::JoinError>(
                            crate::mcp::response::ToolResponse::error_with_code(
                                "internal_error",
                                "INTERNAL_ERROR",
                                "Tool execution semaphore unavailable (server shutting down)",
                                None,
                                None,
                            ),
                        );
                                }
                            };
                            // Check cancellation before spawning blocking work — avoids
                            // consuming a worker permit for an already-cancelled request.
                            if cancel_flag_for_handler.load(Ordering::Relaxed) {
                                return Ok(crate::mcp::response::ToolResponse::error_with_code(
                                    "cancelled",
                                    machine_codes::CANCELLED,
                                    &format!("Tool '{}' request was cancelled", name),
                                    None,
                                    Some(name),
                                ));
                            }
                            tokio::task::spawn_blocking(move || {
                                // Permit is moved into the closure and held until the
                                // handler exits, ensuring MAX_TOOL_WORKERS is respected
                                // even after client-facing timeouts.
                                let _permit = permit;
                                // RAII guard tracks active blocking handler count.
                                let _blocking_guard =
                                    MetricGuard::new(&RUNTIME_METRICS.active_blocking_handlers);
                                // Update peak concurrency watermark.
                                let current = RUNTIME_METRICS
                                    .active_blocking_handlers
                                    .load(Ordering::Relaxed);
                                RUNTIME_METRICS
                                    .peak_blocking_concurrency
                                    .fetch_max(current, Ordering::Relaxed);
                                let mut mcp_eval_ctx = crate::calc::EvalContext::mcp_mode();
                                let result = crate::mcp::budget::with_cancel_flag(
                                    Some(cancel_flag_for_handler.clone()),
                                    || {
                                        crate::mcp::budget::with_eval_context(
                                            &mut mcp_eval_ctx,
                                            || handler(&args_clone),
                                        )
                                    },
                                );
                                // Atomically transition HANDLER_TIMED_OUT → HANDLER_FINISHED,
                                // decrementing timed_out_handlers only if we won the race.
                                // If previous state was HANDLER_RUNNING (no timeout yet), just mark finished.
                                let prev = handler_lifecycle_for_handler
                                    .swap(HANDLER_FINISHED, Ordering::AcqRel);
                                if prev == HANDLER_TIMED_OUT {
                                    RUNTIME_METRICS
                                        .timed_out_handlers
                                        .fetch_sub(1, Ordering::Relaxed);
                                }
                                result
                            })
                            .await
                        })
                        .await;

                    match result {
                        Ok(Ok(tool_response)) => {
                            // Apply budget-aware truncation FIRST (findings cap,
                            // result payload shrinking). This lets per-tool budget
                            // limits have priority over the absolute MCP hard cap.
                            let mut response = tool_response;
                            truncate_response(&mut response, &budget_context.budget);

                            let output = python_json_dumps(&response);
                            if output.is_empty() {
                                Some(wrap_tool_response(&ToolResponse::error_with_code(
                                    "serialization_error",
                                    machine_codes::SERIALIZATION_ERROR,
                                    "Failed to serialize tool response",
                                    None,
                                    Some(&name_owned),
                                )))
                            } else if output.len() > MAX_OUTPUT_BYTES {
                                Some(wrap_tool_response(
                                    &ToolResponse::error_with_code(
                                        "output_too_large",
                                        machine_codes::OUTPUT_TOO_LARGE,
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
                                Some(wrap_tool_response(&response))
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
                        Err(_timeout) => {
                            // Signal cancellation to the running handler so it can
                            // exit cooperatively at the next should_stop() check.
                            // Note: the spawned blocking task may continue running
                            // after this point — we cannot kill threads in Rust.
                            cancel_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                            RUNTIME_METRICS
                                .total_timeouts
                                .fetch_add(1, Ordering::Relaxed);
                            // Try to transition HANDLER_RUNNING → HANDLER_TIMED_OUT.
                            // If the handler already finished (HANDLER_FINISHED), we still count the
                            // timeout response but don't increment timed_out_handlers.
                            let prev = handler_lifecycle.compare_exchange(
                                HANDLER_RUNNING,
                                HANDLER_TIMED_OUT,
                                Ordering::AcqRel,
                                Ordering::Relaxed,
                            );
                            if prev.is_ok() {
                                RUNTIME_METRICS
                                    .timed_out_handlers
                                    .fetch_add(1, Ordering::Relaxed);
                            }
                            Some(wrap_tool_response(&ToolResponse::error_with_code(
                        "timeout",
                        machine_codes::TIMEOUT,
                        &format!(
                            "Tool '{}' execution timed out after {}s (budget: {}ms max). The cancel flag was set cooperatively; the handler may continue briefly.",
                            name_owned,
                            timeout_ms / 1000,
                            timeout_ms
                        ),
                        Some(vec![
                            "Try a simpler input or shorter text".to_string(),
                            "The tool handler checks cancellation cooperatively and may not stop immediately".to_string(),
                        ]),
                        Some(&name_owned),
                    )))
                        }
                    }
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
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();

    let rate_limiter = Arc::new(Mutex::new(RateLimiter::new()));
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
            let _ = tx
                .send(json_rpc_error(
                    -32700,
                    format!(
                        "Request exceeds maximum size: {} bytes received, {} bytes maximum",
                        trimmed.len(),
                        MAX_REQUEST_BYTES
                    ),
                    None,
                ))
                .await;
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
                            e, &*state
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

        // Rate limiting — applies only to requests, not notifications.
        {
            let mut limiter = rate_limiter.lock().await;
            if !limiter.check() {
                let _ = tx
                    .send(invalid_request(
                        format!(
                            "Rate limit exceeded: max {} requests per second",
                            MAX_REQUESTS_PER_SECOND
                        ),
                        request.id.clone(),
                    ))
                    .await;
                continue;
            }
        }

        // Register the active request atomically under one lock acquisition.
        // This checks in-flight limits, duplicate IDs, and inserts the entry
        // in a single lock window — no separate contains_key/insert race.
        let cancel_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let request_id = request.id.clone().unwrap();
        let guard = match register_request(
            &active_requests,
            &cancel_flag,
            request_id.clone(),
            request.method.clone(),
        )
        .await
        {
            Ok(g) => g,
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
                    .send(json_rpc_error(
                        -32600,
                        "Too many in-flight requests",
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

        join_set.spawn(async move {
            // RAII guard tracks active request count for diagnostics.
            let _active_guard = MetricGuard::new(&RUNTIME_METRICS.active_requests);

            let maybe_result = handle_request_async(
                &request_clone,
                &cancel_flag_clone,
                &semaphore_clone,
                &session_state_clone,
            )
            .await;

            // RequestGuard handles active-request cleanup on drop.
            drop(guard);

            // Send response through the channel.
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
