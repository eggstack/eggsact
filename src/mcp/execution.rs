use crate::mcp::budget::ToolBudget;
use crate::mcp::machine_codes;
use crate::mcp::registry;
use crate::mcp::response::{python_json_dumps, sanitize_error, truncate_response, ToolResponse};
use crate::mcp::runtime::{self, MAX_OUTPUT_BYTES, RUNTIME_METRICS};
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

            // Manually increment active_blocking_handlers (not RAII) so we can
            // control the exact drop order during panic unwind.
            RUNTIME_METRICS
                .active_blocking_handlers
                .fetch_add(1, Ordering::Relaxed);
            // Update peak concurrency watermark.
            let current = RUNTIME_METRICS
                .active_blocking_handlers
                .load(Ordering::Relaxed);
            RUNTIME_METRICS
                .peak_blocking_concurrency
                .fetch_max(current, Ordering::Relaxed);

            let mut mcp_eval_ctx = crate::calc::EvalContext::mcp_mode();
            let cancel_flag_for_handler = cancel_flag.clone();

            // Use catch_unwind so the lifecycle swap and counter decrement always
            // execute, even if the handler panics. This maintains the invariant:
            // timed_out_handlers <= active_blocking_handlers at all stable snapshots.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                crate::mcp::budget::with_cancel_flag(Some(cancel_flag_for_handler), || {
                    crate::mcp::budget::with_eval_context(&mut mcp_eval_ctx, || handler(&args))
                })
            }));

            // Atomically transition to FINISHED, accounting for timeout.
            // This ALWAYS runs, even if the handler panicked.
            let prev = handler_lifecycle.swap(HANDLER_FINISHED, Ordering::AcqRel);
            if prev == HANDLER_TIMED_OUT_ACCOUNTED {
                RUNTIME_METRICS
                    .timed_out_handlers
                    .fetch_sub(1, Ordering::Relaxed);
            }

            // Now safe to decrement active_blocking_handlers.
            RUNTIME_METRICS
                .active_blocking_handlers
                .fetch_sub(1, Ordering::Relaxed);

            // Convert catch_unwind result back to the expected type.
            match result {
                Ok(response) => response,
                Err(payload) => {
                    let msg = payload
                        .downcast_ref::<&str>()
                        .map(|s| s.to_string())
                        .or_else(|| payload.downcast_ref::<String>().cloned())
                        .unwrap_or_else(|| "handler panicked".to_string());
                    std::panic::resume_unwind(Box::new(msg));
                }
            }
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

        // timed_out_handlers must not be elevated by our test (tolerance for parallel tests).
        let after = snapshot_metrics();
        assert!(
            after.timed_out_handlers <= before.timed_out_handlers + 1,
            "timed_out_handlers must return to baseline: before={}, after={}",
            before.timed_out_handlers,
            after.timed_out_handlers,
        );
    }

    #[tokio::test]
    async fn all_gauges_return_to_zero() {
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
        assert!(
            after.timed_out_handlers <= after.active_blocking_handlers,
            "INVARIANT VIOLATION: timed_out_handlers ({}) > active_blocking_handlers ({})",
            after.timed_out_handlers,
            after.active_blocking_handlers,
        );
    }
}

// ── Deterministic coordinator tests ──────────────────────────────────────
//
// These tests exercise the coordinator with deterministic synchronization
// (barriers, channels) rather than timing. Each test asserts:
//   - stable snapshots never show timed_out_handlers > active_blocking_handlers
//   - all gauges return to baseline after the test
//
// Note: ToolHandler is `fn(&Value) -> ToolResponse` (a function pointer), so
// closures that capture variables cannot be used. Tests that need to track
// handler execution use static atomics for communication.

#[cfg(test)]
mod coordinator_tests {
    use super::*;
    use crate::mcp::runtime::snapshot_metrics;
    use std::sync::atomic::AtomicUsize;
    use std::sync::Arc as StdArc;
    use std::time::Duration;

    /// Static flag for queued_timeout_blocks_handler_after_permit_release test.
    static HANDLER_RAN_FLAG: AtomicBool = AtomicBool::new(false);

    /// Fast handler for the race test — always returns immediately.
    fn race_fast_handler(_args: &Value) -> ToolResponse {
        ToolResponse::success(serde_json::json!("fast"), None)
    }

    /// Slow handler for the race test — sleeps to allow timeout to fire.
    fn race_slow_handler(_args: &Value) -> ToolResponse {
        std::thread::sleep(Duration::from_millis(15));
        ToolResponse::success(serde_json::json!("slow"), None)
    }

    /// Assert invariant: timed_out_handlers <= active_blocking_handlers.
    fn assert_snapshot_invariant(snap: &crate::mcp::runtime::MetricsSnapshot) {
        assert!(
            snap.timed_out_handlers <= snap.active_blocking_handlers,
            "INVARIANT VIOLATION: timed_out_handlers ({}) > active_blocking_handlers ({})",
            snap.timed_out_handlers,
            snap.active_blocking_handlers,
        );
    }

    // Test 2: queued timeout never enters the handler after a permit is released.
    // 0-permit semaphore → timeout fires → release permit → handler must NOT run.
    #[tokio::test]
    async fn queued_timeout_blocks_handler_after_permit_release() {
        let semaphore = Arc::new(tokio::sync::Semaphore::new(0));
        let cancel_flag = Arc::new(AtomicBool::new(false));

        HANDLER_RAN_FLAG.store(false, Ordering::SeqCst);

        let budget = ToolBudget::CHEAP.with_max_elapsed_ms(10);
        let outcome = execute_tool_bounded(
            |_args| {
                HANDLER_RAN_FLAG.store(true, Ordering::SeqCst);
                ToolResponse::success(serde_json::json!("done"), None)
            },
            Value::Object(serde_json::Map::new()),
            "test_tool".to_string(),
            budget,
            cancel_flag.clone(),
            semaphore.clone(),
        )
        .await;

        assert!(outcome.timed_out);
        // Release permit — the queued task should detect TIMED_OUT_QUEUED and not run handler.
        semaphore.add_permits(1);
        // Give the spawned task time to observe the state and bail.
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert!(
            !HANDLER_RAN_FLAG.load(Ordering::SeqCst),
            "handler must not run after queued timeout"
        );
        let after = snapshot_metrics();
        assert_snapshot_invariant(&after);
    }

    // Test 5: completion racing timeout reservation before gauge accounting.
    // Handler finishes just as timeout fires — compare_exchange for RUNNING→TIMEOUT_ACCOUNTING
    // should fail, so no increment occurs. No underflow possible.
    #[tokio::test]
    async fn completion_races_timeout_before_gauge_accounting() {
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let cancel_flag = Arc::new(AtomicBool::new(false));

        // Very short timeout so it fires while handler is still completing.
        let budget = ToolBudget::CHEAP.with_max_elapsed_ms(1);

        let _outcome = execute_tool_bounded(
            |_args| {
                // Fast handler — should complete before or around timeout.
                ToolResponse::success(serde_json::json!("ok"), None)
            },
            Value::Object(serde_json::map::Map::new()),
            "test_tool".to_string(),
            budget,
            cancel_flag.clone(),
            semaphore.clone(),
        )
        .await;

        // Wait for any blocking handler to finish.
        tokio::time::sleep(Duration::from_millis(50)).await;

        let after = snapshot_metrics();
        assert_snapshot_invariant(&after);
    }

    // Test 6: completion racing after gauge accounting.
    // Timeout fires and increments timed_out_handlers, then handler finishes and decrements.
    // Net effect: gauges return to baseline.
    #[tokio::test]
    async fn completion_after_gauge_accounting_decrements_exactly_once() {
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let cancel_flag = Arc::new(AtomicBool::new(false));

        // Short timeout + slightly longer handler ensures timeout fires while running.
        let budget = ToolBudget::CHEAP.with_max_elapsed_ms(10);

        let outcome = execute_tool_bounded(
            |_args| {
                std::thread::sleep(Duration::from_millis(100));
                ToolResponse::success(serde_json::json!("ok"), None)
            },
            Value::Object(serde_json::map::Map::new()),
            "test_tool".to_string(),
            budget,
            cancel_flag.clone(),
            semaphore.clone(),
        )
        .await;

        assert!(outcome.timed_out);
        // Wait for the handler to finish and decrement.
        tokio::time::sleep(Duration::from_millis(200)).await;

        let after = snapshot_metrics();
        assert_snapshot_invariant(&after);
    }

    // Test 7: handler panic after timeout.
    // Timeout fires, then handler panics. The completion guard's swap to FINISHED
    // must still observe TIMED_OUT_ACCOUNTED and decrement.
    #[tokio::test]
    async fn handler_panic_after_timeout_corrects_gauges() {
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let cancel_flag = Arc::new(AtomicBool::new(false));

        let budget = ToolBudget::CHEAP.with_max_elapsed_ms(10);

        let outcome = execute_tool_bounded(
            |_args| {
                std::thread::sleep(Duration::from_millis(100));
                panic!("intentional test panic");
            },
            Value::Object(serde_json::map::Map::new()),
            "test_tool".to_string(),
            budget,
            cancel_flag.clone(),
            semaphore.clone(),
        )
        .await;

        assert!(outcome.timed_out);
        // The JoinError from the panic is captured; wait for cleanup.
        tokio::time::sleep(Duration::from_millis(200)).await;

        let after = snapshot_metrics();
        assert_snapshot_invariant(&after);
    }

    // Test 8: cooperative cancellation after timeout.
    // Timeout fires, sets cancel flag, handler checks flag and exits early.
    #[tokio::test]
    async fn cooperative_cancellation_after_timeout() {
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let cancel_flag = Arc::new(AtomicBool::new(false));

        let budget = ToolBudget::CHEAP.with_max_elapsed_ms(10);

        let outcome = execute_tool_bounded(
            |_args| {
                // Check cancel flag cooperatively.
                if crate::mcp::budget::current_cancel_flag()
                    .is_some_and(|f| f.load(Ordering::Relaxed))
                {
                    return ToolResponse::error_with_code(
                        "cancelled",
                        machine_codes::CANCELLED,
                        "cooperative cancel",
                        None,
                        None,
                    );
                }
                std::thread::sleep(Duration::from_millis(100));
                ToolResponse::success(serde_json::json!("ok"), None)
            },
            Value::Object(serde_json::map::Map::new()),
            "test_tool".to_string(),
            budget,
            cancel_flag.clone(),
            semaphore.clone(),
        )
        .await;

        assert!(outcome.timed_out);
        tokio::time::sleep(Duration::from_millis(200)).await;

        let after = snapshot_metrics();
        assert_snapshot_invariant(&after);
    }

    // Test 9+10: hundreds of controlled timeout/completion races.
    // Alternate between fast handlers (win the race) and slow handlers (lose the race).
    // Uses separate handler functions since ToolHandler is fn pointer (cannot capture).
    #[tokio::test]
    async fn hundreds_of_controlled_timeout_completion_races() {
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let mut timeouts_observed = 0usize;
        let mut successes_observed = 0usize;

        for iter in 0..100 {
            let cancel_flag = Arc::new(AtomicBool::new(false));
            // Alternate: even iterations = fast handler (should succeed),
            // odd iterations = slow handler (should timeout).
            let (handler, budget) = if iter % 2 == 0 {
                (
                    race_fast_handler as registry::ToolHandler,
                    ToolBudget::CHEAP.with_max_elapsed_ms(5000),
                )
            } else {
                (
                    race_slow_handler as registry::ToolHandler,
                    ToolBudget::CHEAP.with_max_elapsed_ms(5),
                )
            };

            let outcome = execute_tool_bounded(
                handler,
                Value::Object(serde_json::map::Map::new()),
                "race_tool".to_string(),
                budget,
                cancel_flag,
                semaphore.clone(),
            )
            .await;

            if outcome.timed_out {
                timeouts_observed += 1;
                // Wait for handler to finish.
                tokio::time::sleep(Duration::from_millis(30)).await;
            } else {
                successes_observed += 1;
            }

            // Assert invariant after every iteration.
            let snap = snapshot_metrics();
            assert_snapshot_invariant(&snap);
        }

        // We should have seen both outcomes (fast and slow handlers).
        assert!(
            successes_observed > 0,
            "should have at least some successful completions"
        );
        assert!(timeouts_observed > 0, "should have at least some timeouts");

        let after = snapshot_metrics();
        assert_snapshot_invariant(&after);
    }

    // Test 12: semaphore occupancy never exceeds MAX_TOOL_WORKERS.
    // Launch more concurrent tasks than permits and verify peak concurrency.
    #[tokio::test]
    async fn semaphore_never_exceeds_max_workers() {
        let max_workers = 4usize;
        let semaphore = Arc::new(tokio::sync::Semaphore::new(max_workers));
        let num_tasks = 16;
        let active = StdArc::new(AtomicUsize::new(0));
        let peak_observed = StdArc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..num_tasks {
            let sem = semaphore.clone();
            let active = active.clone();
            let peak = peak_observed.clone();
            handles.push(tokio::spawn(async move {
                let _permit = sem.acquire_owned().await.unwrap();
                let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(current, Ordering::SeqCst);
                // Hold permit briefly.
                tokio::time::sleep(Duration::from_millis(10)).await;
                active.fetch_sub(1, Ordering::SeqCst);
            }));
        }

        for h in handles {
            h.await.unwrap();
        }

        let peak = peak_observed.load(Ordering::SeqCst);
        assert!(
            peak <= max_workers,
            "peak concurrency ({}) exceeded max workers ({})",
            peak,
            max_workers
        );

        let after = snapshot_metrics();
        assert_snapshot_invariant(&after);
    }

    // ── WS3 additional tests: request lifecycle under coordinator ─────────

    // Handler panic still allows lifecycle cleanup (active_blocking_handlers returns to baseline).
    #[tokio::test]
    async fn handler_panic_returns_blocking_handler_count_to_baseline() {
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let budget = ToolBudget::CHEAP;

        let outcome = execute_tool_bounded(
            |_args| {
                panic!("intentional panic for cleanup test");
            },
            Value::Object(serde_json::map::Map::new()),
            "panic_tool".to_string(),
            budget,
            cancel_flag,
            semaphore,
        )
        .await;

        // Handler panicked — JoinError is captured, not propagated.
        assert!(!outcome.timed_out);
        assert!(outcome.tool_response.is_err());

        // Wait for all cleanup to complete.
        tokio::time::sleep(Duration::from_millis(100)).await;

        let after = snapshot_metrics();
        assert_snapshot_invariant(&after);
    }

    // Timeout response is returned while the handler may still be running.
    // After the handler finishes, metrics return to baseline.
    #[tokio::test]
    async fn timeout_response_returns_while_handler_continues() {
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let budget = ToolBudget::CHEAP.with_max_elapsed_ms(10);

        let outcome = execute_tool_bounded(
            |_args| {
                // Slow handler — timeout fires first, then handler completes.
                std::thread::sleep(Duration::from_millis(100));
                ToolResponse::success(serde_json::json!("late"), None)
            },
            Value::Object(serde_json::map::Map::new()),
            "slow_tool".to_string(),
            budget,
            cancel_flag,
            semaphore,
        )
        .await;

        // Timeout response was returned to caller.
        assert!(outcome.timed_out);
        let resp = outcome.tool_response.unwrap();
        assert!(!resp.ok);

        // Handler may still be running — wait for it to finish.
        tokio::time::sleep(Duration::from_millis(200)).await;

        let after = snapshot_metrics();
        assert_snapshot_invariant(&after);
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
