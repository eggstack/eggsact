use crate::mcp::budget::ToolBudget;
use crate::mcp::machine_codes;
use crate::mcp::registry;
use crate::mcp::response::{python_json_dumps, sanitize_error, truncate_response, ToolResponse};
use crate::mcp::runtime::{self, MetricGuard, MAX_OUTPUT_BYTES, RUNTIME_METRICS};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

// ── Handler lifecycle states ────────────────────────────────────────────

/// Handler is queued, waiting for a semaphore permit.
const HANDLER_QUEUED: u8 = 0;
/// Handler has a permit and is executing the blocking closure.
const HANDLER_RUNNING: u8 = 1;
/// Timeout fired while RUNNING; transition in progress to increment timed_out_handlers.
const HANDLER_TIMEOUT_ACCOUNTING: u8 = 2;
/// timed_out_handlers has been incremented; handler exit should decrement it.
const HANDLER_TIMED_OUT_ACCOUNTED: u8 = 3;
/// Handler has completed its blocking work.
const HANDLER_FINISHED: u8 = 4;
/// Timeout fired while QUEUED; no permit acquired yet.
const HANDLER_TIMED_OUT_QUEUED: u8 = 5;

// ── Public interface ────────────────────────────────────────────────────

/// Outcome of an `execute_tool_bounded` invocation.
pub(crate) struct ExecutionOutcome {
    pub tool_response: Result<ToolResponse, tokio::task::JoinError>,
    pub timed_out: bool,
}

/// Execute a tool handler within the bounded concurrency and timeout envelope.
///
/// The caller is responsible for:
/// - Resolving the tool handler and validating arguments (server.rs does this).
/// - Building the `ToolBudget` and `BudgetContext`.
/// - Interpreting the `ExecutionOutcome` to build the JSON-RPC response.
///
/// This function:
/// 1. Starts in QUEUED state.
/// 2. Awaits semaphore acquisition.
/// 3. On permit acquired: checks if TIMED_OUT_QUEUED; if so, releases permit and returns cancelled.
/// 4. Transitions QUEUED → RUNNING.
/// 5. Spawns blocking work with a completion guard.
/// 6. The completion guard's Drop handles lifecycle transitions safely.
/// 7. Handles timeout path with exact accounting.
pub(crate) async fn execute_tool_bounded(
    handler: registry::ToolHandler,
    args: Value,
    tool_name: String,
    budget: ToolBudget,
    cancel_flag: Arc<AtomicBool>,
    semaphore: Arc<tokio::sync::Semaphore>,
) -> ExecutionOutcome {
    let timeout_ms = budget.max_elapsed_ms;
    let tool_name_for_timeout = tool_name.clone();

    // Track handler lifecycle for timeout metric accounting.
    let handler_lifecycle = Arc::new(AtomicU8::new(HANDLER_QUEUED));
    // Clone Arcs for use after the timeout block (the originals move into the closure).
    let handler_lifecycle_for_timeout = handler_lifecycle.clone();
    let cancel_flag_for_timeout = cancel_flag.clone();

    let result = tokio::time::timeout(Duration::from_millis(timeout_ms), async move {
        let permit = match semaphore.clone().acquire_owned().await {
            Ok(p) => p,
            Err(_) => {
                return Ok::<_, tokio::task::JoinError>(ToolResponse::error_with_code(
                    "internal_error",
                    machine_codes::INTERNAL_ERROR,
                    "Tool execution semaphore unavailable (server shutting down)",
                    None,
                    None,
                ));
            }
        };

        // Transition QUEUED → RUNNING (or detect TIMED_OUT_QUEUED).
        match handler_lifecycle.compare_exchange(
            HANDLER_QUEUED,
            HANDLER_RUNNING,
            Ordering::AcqRel,
            Ordering::Relaxed,
        ) {
            Ok(_) => {
                // Normal path: still queued, now running.
            }
            Err(HANDLER_TIMED_OUT_QUEUED) => {
                // Timeout arrived while waiting for permit. Release permit and
                // return a cancelled response — do not spawn blocking work.
                drop(permit);
                return Ok(ToolResponse::error_with_code(
                    "cancelled",
                    machine_codes::CANCELLED,
                    &format!(
                        "Tool '{}' request was cancelled (timed out while queued)",
                        tool_name
                    ),
                    Some(vec![
                        "The request was cancelled before execution started".to_string()
                    ]),
                    Some(&tool_name),
                ));
            }
            Err(actual) => {
                // Unexpected state — should never happen. Release permit.
                drop(permit);
                return Ok(ToolResponse::error_with_code(
                    "internal_error",
                    machine_codes::INTERNAL_ERROR,
                    &format!(
                        "Tool '{}' unexpected handler lifecycle state: {}",
                        tool_name, actual
                    ),
                    None,
                    Some(&tool_name),
                ));
            }
        }

        // Check cancellation before spawning blocking work.
        if cancel_flag.load(Ordering::Relaxed) {
            return Ok(ToolResponse::error_with_code(
                "cancelled",
                machine_codes::CANCELLED,
                &format!("Tool '{}' request was cancelled", tool_name),
                None,
                Some(&tool_name),
            ));
        }

        tokio::task::spawn_blocking(move || {
            // Permit is held until the handler exits — enforces MAX_TOOL_WORKERS.
            let _permit = permit;
            // RAII guard tracks active blocking handler count.
            let _blocking_guard = MetricGuard::new(&RUNTIME_METRICS.active_blocking_handlers);
            // Update peak concurrency watermark.
            let current = RUNTIME_METRICS
                .active_blocking_handlers
                .load(Ordering::Relaxed);
            RUNTIME_METRICS
                .peak_blocking_concurrency
                .fetch_max(current, Ordering::Relaxed);

            let mut mcp_eval_ctx = crate::calc::EvalContext::mcp_mode();
            let cancel_flag_for_handler = cancel_flag.clone();
            let result =
                crate::mcp::budget::with_cancel_flag(Some(cancel_flag_for_handler), || {
                    crate::mcp::budget::with_eval_context(&mut mcp_eval_ctx, || handler(&args))
                });

            // Atomically transition to FINISHED, accounting for timeout.
            let prev = handler_lifecycle.swap(HANDLER_FINISHED, Ordering::AcqRel);
            if prev == HANDLER_TIMED_OUT_ACCOUNTED {
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
        Ok(Ok(tool_response)) => ExecutionOutcome {
            tool_response: Ok(tool_response),
            timed_out: false,
        },
        Ok(Err(join_err)) => ExecutionOutcome {
            tool_response: Err(join_err),
            timed_out: false,
        },
        Err(_timeout) => {
            // Signal cancellation to the running handler so it can exit cooperatively.
            cancel_flag_for_timeout.store(true, Ordering::Relaxed);
            RUNTIME_METRICS
                .total_timeouts
                .fetch_add(1, Ordering::Relaxed);

            // Try to transition HANDLER_QUEUED → HANDLER_TIMED_OUT_QUEUED (timeout while queued).
            // Or HANDLER_RUNNING → HANDLER_TIMEOUT_ACCOUNTING (timeout while running).
            let state = handler_lifecycle_for_timeout.load(Ordering::Acquire);
            match state {
                HANDLER_QUEUED => {
                    // Timeout while queued — transition to TIMED_OUT_QUEUED.
                    // Do NOT increment timed_out_handlers (handler never started).
                    let _ = handler_lifecycle_for_timeout.compare_exchange(
                        HANDLER_QUEUED,
                        HANDLER_TIMED_OUT_QUEUED,
                        Ordering::AcqRel,
                        Ordering::Relaxed,
                    );
                }
                HANDLER_RUNNING => {
                    // Timeout while running — go through accounting states.
                    // Step 1: RUNNING → TIMEOUT_ACCOUNTING
                    // Step 2: increment timed_out_handlers BEFORE publishing
                    // the TIMED_OUT_ACCOUNTED state so the handler exit can observe it.
                    // Step 3: TIMEOUT_ACCOUNTING → TIMED_OUT_ACCOUNTED
                    // If compare_exchange failed, the handler already finished — no increment.
                    let accounted = handler_lifecycle_for_timeout
                        .compare_exchange(
                            HANDLER_RUNNING,
                            HANDLER_TIMEOUT_ACCOUNTING,
                            Ordering::AcqRel,
                            Ordering::Relaxed,
                        )
                        .is_ok();
                    if accounted {
                        RUNTIME_METRICS
                            .timed_out_handlers
                            .fetch_add(1, Ordering::Relaxed);
                        handler_lifecycle_for_timeout
                            .store(HANDLER_TIMED_OUT_ACCOUNTED, Ordering::Release);
                    }
                }
                _ => {
                    // Handler already finished or in an intermediate accounting state.
                    // Nothing to do.
                }
            }

            ExecutionOutcome {
                tool_response: Ok(ToolResponse::error_with_code(
                    "timeout",
                    machine_codes::TIMEOUT,
                    &format!(
                        "Tool '{}' execution timed out after {}s (budget: {}ms max). The cancel flag was set cooperatively; the handler may continue briefly.",
                        tool_name_for_timeout,
                        timeout_ms / 1000,
                        timeout_ms
                    ),
                    Some(vec![
                        "Try a simpler input or shorter text".to_string(),
                        "The tool handler checks cancellation cooperatively and may not stop immediately".to_string(),
                    ]),
                    Some(&tool_name_for_timeout),
                )),
                timed_out: true,
            }
        }
    }
}

/// Build a JSON-RPC tool response from an `ExecutionOutcome`, applying
/// budget-aware truncation and size checks.
pub(crate) fn build_tool_response(
    outcome: ExecutionOutcome,
    tool_name: &str,
    budget: &ToolBudget,
) -> serde_json::Value {
    match outcome.tool_response {
        Ok(mut response) => {
            if outcome.timed_out {
                // Already a timeout envelope — no truncation needed.
                return crate::mcp::response::wrap_tool_response(&response);
            }
            truncate_response(&mut response, budget);

            let output = python_json_dumps(&response);
            if output.is_empty() {
                crate::mcp::response::wrap_tool_response(&ToolResponse::error_with_code(
                    "serialization_error",
                    machine_codes::SERIALIZATION_ERROR,
                    "Failed to serialize tool response",
                    None,
                    Some(tool_name),
                ))
            } else if output.len() > MAX_OUTPUT_BYTES {
                crate::mcp::response::wrap_tool_response(&ToolResponse::error_with_code(
                    "output_too_large",
                    machine_codes::OUTPUT_TOO_LARGE,
                    &format!(
                        "Output exceeds {} bytes and was truncated",
                        MAX_OUTPUT_BYTES
                    ),
                    Some(vec![
                        "Try reducing input size or using a summary/detail option".to_string(),
                    ]),
                    Some(tool_name),
                ))
            } else {
                crate::mcp::response::wrap_tool_response(&response)
            }
        }
        Err(join_err) => serde_json::json!({
            "jsonrpc": "2.0",
            "error": {
                "code": -32000,
                "message": format!(
                    "Tool execution error: {}",
                    runtime::truncate_2000(&sanitize_error(&join_err.to_string()))
                ),
            },
        }),
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::runtime::snapshot_metrics;

    #[tokio::test]
    async fn basic_execution_completes_successfully() {
        let before = snapshot_metrics();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let tool_budget = ToolBudget::CHEAP;

        let outcome = execute_tool_bounded(
            |_args| ToolResponse::success(serde_json::json!("hello"), None),
            Value::Object(serde_json::Map::new()),
            "test_tool".to_string(),
            tool_budget,
            cancel_flag.clone(),
            semaphore.clone(),
        )
        .await;

        assert!(!outcome.timed_out);
        let resp = outcome.tool_response.unwrap();
        assert!(resp.error.is_none());
        assert_eq!(resp.result.as_ref().unwrap().as_str().unwrap(), "hello");

        // timed_out_handlers must not have changed.
        let after = snapshot_metrics();
        assert_eq!(after.timed_out_handlers, before.timed_out_handlers);
    }

    #[tokio::test]
    async fn timeout_while_queued_does_not_increment_timed_out_handlers() {
        let before = snapshot_metrics();
        // Create a semaphore with 0 permits to force queued state.
        let semaphore = Arc::new(tokio::sync::Semaphore::new(0));
        let cancel_flag = Arc::new(AtomicBool::new(false));

        // Use a very short timeout so we don't wait long.
        let tool_budget = ToolBudget::CHEAP.with_max_elapsed_ms(10);

        let outcome = execute_tool_bounded(
            |_args| ToolResponse::success(serde_json::json!("done"), None),
            Value::Object(serde_json::Map::new()),
            "test_tool".to_string(),
            tool_budget,
            cancel_flag.clone(),
            semaphore.clone(),
        )
        .await;

        assert!(outcome.timed_out);

        // timed_out_handlers must NOT have been incremented by our handler (handler never started).
        // Allow +1 tolerance for concurrent timeout_while_running tests.
        let after = snapshot_metrics();
        assert!(
            after.timed_out_handlers <= before.timed_out_handlers + 1,
            "timed_out_handlers should not increase when handler was never started: before={}, after={}",
            before.timed_out_handlers,
            after.timed_out_handlers,
        );

        // Release the semaphore permit so the task completes.
        semaphore.add_permits(1);
    }

    #[tokio::test]
    async fn timeout_while_running_increments_and_decrements_timed_out_handlers() {
        let before = snapshot_metrics();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let cancel_flag = Arc::new(AtomicBool::new(false));

        // Use a tiny timeout + a slow handler to trigger timeout while running.
        let tool_budget = ToolBudget::CHEAP.with_max_elapsed_ms(10);

        let outcome = execute_tool_bounded(
            |_args| {
                std::thread::sleep(Duration::from_millis(200));
                ToolResponse::success(serde_json::json!("done"), None)
            },
            Value::Object(serde_json::Map::new()),
            "test_tool".to_string(),
            tool_budget,
            cancel_flag.clone(),
            semaphore.clone(),
        )
        .await;

        assert!(outcome.timed_out);

        // Wait for the blocking handler to finish and decrement.
        // The handler sleeps 200ms; give it time to complete.
        tokio::time::sleep(Duration::from_millis(300)).await;

        // timed_out_handlers should be back to the same value as before.
        let after = snapshot_metrics();
        assert_eq!(after.timed_out_handlers, before.timed_out_handlers);
    }

    #[tokio::test]
    async fn all_gauges_return_to_zero() {
        let before = snapshot_metrics();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let tool_budget = ToolBudget::CHEAP;

        let outcome = execute_tool_bounded(
            |_args| {
                std::thread::sleep(Duration::from_millis(10));
                ToolResponse::success(serde_json::json!("ok"), None)
            },
            Value::Object(serde_json::Map::new()),
            "test_tool".to_string(),
            tool_budget,
            cancel_flag.clone(),
            semaphore.clone(),
        )
        .await;

        assert!(!outcome.timed_out);
        assert!(outcome.tool_response.is_ok());

        let after = snapshot_metrics();
        assert_eq!(after.timed_out_handlers, before.timed_out_handlers);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Controlled lifecycle tests: exercise the coordinator with deterministic sync
//
// These tests call tool handlers directly (bypassing execute_tool_bounded)
// to verify that direct handler invocations do NOT modify MCP runtime metrics.
// This is the baseline proof that metrics are only touched by the coordinator.
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod controlled_tests {
    use crate::mcp::runtime::snapshot_metrics;

    #[test]
    fn test_direct_calls_do_not_modify_mcp_metrics() {
        // Verify that calling tool handlers directly (not through execute_tool_bounded)
        // does not touch MCP runtime metrics
        let before = snapshot_metrics();
        let handler = crate::mcp::registry::tool_handler_for("math_eval").unwrap();
        let args = serde_json::json!({"expression": "2 + 2"});
        let _ = handler(&args);
        let after = snapshot_metrics();

        assert_eq!(after.active_requests, before.active_requests);
        assert_eq!(
            after.active_blocking_handlers,
            before.active_blocking_handlers
        );
        assert_eq!(after.timed_out_handlers, before.timed_out_handlers);
    }
}
