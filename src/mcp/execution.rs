use crate::mcp::budget::ToolBudget;
use crate::mcp::machine_codes;
use crate::mcp::registry;
use crate::mcp::response::{python_json_dumps, sanitize_error, truncate_response, ToolResponse};
use crate::mcp::runtime::{self, MAX_OUTPUT_BYTES, RUNTIME_METRICS};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

// ── Handler lifecycle (mutex-owned transitions) ──────────────────────────

/// Handler lifecycle phases, protected by a Mutex.
///
/// All state transitions are serialized through the mutex, eliminating
/// load-then-CAS gaps and ensuring each increment/decrement is atomic
/// with the phase change.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HandlerPhase {
    Queued,
    Running,
    TimedOutQueued,
    TimedOutRunning,
    Finished,
}

/// Result of `begin_running`: either proceed to run or abort.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BeginRunning {
    Run,
    CancelledBeforeStart,
}

/// Disposition of a timeout attempt, observed under the lifecycle lock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TimeoutDisposition {
    Queued,
    Running,
    AlreadyFinished,
}

/// Mutex-protected handler lifecycle. All state transitions and gauge
/// mutations are atomic with respect to the phase change, eliminating
/// the load-then-CAS gaps and overwrite races of the previous AtomicU8
/// design.
struct HandlerLifecycle {
    phase: Mutex<HandlerPhase>,
}

impl HandlerLifecycle {
    /// Create a new lifecycle in Queued state.
    fn new() -> Self {
        Self {
            phase: Mutex::new(HandlerPhase::Queued),
        }
    }

    /// Attempt to transition Queued → Running (or detect TimedOutQueued).
    ///
    /// Returns `CancelledBeforeStart` if the timeout already transitioned
    /// the phase to `TimedOutQueued`.
    fn begin_running(&self, metrics: &runtime::RuntimeMetrics) -> BeginRunning {
        let mut phase = self.phase.lock().unwrap();
        match *phase {
            HandlerPhase::Queued => {
                metrics
                    .active_blocking_handlers
                    .fetch_add(1, Ordering::Relaxed);
                let current = metrics.active_blocking_handlers.load(Ordering::Relaxed);
                metrics
                    .peak_blocking_concurrency
                    .fetch_max(current, Ordering::Relaxed);
                *phase = HandlerPhase::Running;
                BeginRunning::Run
            }
            HandlerPhase::TimedOutQueued => BeginRunning::CancelledBeforeStart,
            other => panic!("begin_running called in unexpected phase: {:?}", other),
        }
    }

    /// Attempt to record a timeout for this handler.
    ///
    /// Returns the phase the handler was in, which determines the timeout
    /// task's accounting actions.
    fn record_timeout(&self, metrics: &runtime::RuntimeMetrics) -> TimeoutDisposition {
        let mut phase = self.phase.lock().unwrap();
        match *phase {
            HandlerPhase::Queued => {
                *phase = HandlerPhase::TimedOutQueued;
                TimeoutDisposition::Queued
            }
            HandlerPhase::Running => {
                metrics.timed_out_handlers.fetch_add(1, Ordering::Relaxed);
                *phase = HandlerPhase::TimedOutRunning;
                TimeoutDisposition::Running
            }
            HandlerPhase::TimedOutQueued
            | HandlerPhase::TimedOutRunning
            | HandlerPhase::Finished => TimeoutDisposition::AlreadyFinished,
        }
    }

    /// Transition the handler to Finished, accounting for any prior timeout.
    ///
    /// This always runs (via catch_unwind), so gauges are always corrected.
    fn finish(&self, metrics: &runtime::RuntimeMetrics) {
        let mut phase = self.phase.lock().unwrap();
        match *phase {
            HandlerPhase::Running => {
                *phase = HandlerPhase::Finished;
                metrics
                    .active_blocking_handlers
                    .fetch_sub(1, Ordering::Relaxed);
            }
            HandlerPhase::TimedOutRunning => {
                metrics.timed_out_handlers.fetch_sub(1, Ordering::Relaxed);
                *phase = HandlerPhase::Finished;
                metrics
                    .active_blocking_handlers
                    .fetch_sub(1, Ordering::Relaxed);
            }
            HandlerPhase::TimedOutQueued => {
                *phase = HandlerPhase::Finished;
            }
            HandlerPhase::Finished => {
                debug_assert!(false, "double completion detected");
            }
            HandlerPhase::Queued => {
                debug_assert!(false, "finish called while still Queued");
            }
        }
    }
}

// ── Test hooks ──────────────────────────────────────────────────────────

/// Test-only hooks for deterministic execution testing. Each hook is an
/// optional `Notify` that, when `Some`, blocks execution at that point
/// until the test releases it. `None` means no waiting.
///
/// This struct is always available (not cfg(test)) so that
/// `execute_tool_bounded` can construct `ExecutionHooks::none()` without
/// conditional compilation.
pub(crate) struct ExecutionHooks {
    pub permit_acquired: Option<std::sync::Arc<tokio::sync::Notify>>,
    pub before_lifecycle_start: Option<std::sync::Arc<tokio::sync::Notify>>,
    pub running_established: Option<std::sync::Arc<tokio::sync::Notify>>,
    pub before_timeout_lock: Option<std::sync::Arc<tokio::sync::Notify>>,
    pub timeout_transition_done: Option<std::sync::Arc<tokio::sync::Notify>>,
    #[allow(dead_code)]
    pub handler_about_to_finish: Option<std::sync::Arc<tokio::sync::Notify>>,
    #[allow(dead_code)]
    pub lifecycle_complete: Option<std::sync::Arc<tokio::sync::Notify>>,
}

impl ExecutionHooks {
    pub fn none() -> Self {
        Self {
            permit_acquired: None,
            before_lifecycle_start: None,
            running_established: None,
            before_timeout_lock: None,
            timeout_transition_done: None,
            handler_about_to_finish: None,
            lifecycle_complete: None,
        }
    }
}

// ── Test handler statics ────────────────────────────────────────────────
//
// Since `ToolHandler` is `fn(&Value) -> ToolResponse` (function pointer),
// closures that capture state cannot be used. Tests communicate with
// handlers via static atomics.

#[cfg(test)]
static TEST_HANDLER_SHOULD_BLOCK: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
static TEST_HANDLER_RELEASED: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
fn test_handler_blocking(_args: &Value) -> ToolResponse {
    while TEST_HANDLER_SHOULD_BLOCK.load(Ordering::SeqCst) {
        if TEST_HANDLER_RELEASED.load(Ordering::SeqCst) {
            break;
        }
        std::hint::spin_loop();
    }
    ToolResponse::success(serde_json::json!("ok"), None)
}

#[cfg(test)]
fn test_handler_fast(_args: &Value) -> ToolResponse {
    ToolResponse::success(serde_json::json!("ok"), None)
}

#[cfg(test)]
fn test_handler_slow_cancel(_args: &Value) -> ToolResponse {
    while !crate::mcp::budget::current_cancel_flag().is_some_and(|f| f.load(Ordering::Relaxed)) {
        std::hint::spin_loop();
    }
    ToolResponse::error_with_code(
        "cancelled",
        machine_codes::CANCELLED,
        "cooperative cancel",
        None,
        None,
    )
}

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
/// Lifecycle:
/// 1. Starts in Queued state (HandlerLifecycle::new()).
/// 2. Awaits semaphore acquisition.
/// 3. On permit acquired: calls `begin_running` — if CancelledBeforeStart
///    (timeout already set to TimedOutQueued), releases permit and returns.
/// 4. Spawns blocking work with a completion guard; `finish()` always runs.
/// 5. Timeout path: sets cancel flag, calls `record_timeout()`.
pub(crate) async fn execute_tool_bounded(
    handler: registry::ToolHandler,
    args: Value,
    tool_name: String,
    budget: ToolBudget,
    cancel_flag: std::sync::Arc<AtomicBool>,
    semaphore: std::sync::Arc<tokio::sync::Semaphore>,
) -> ExecutionOutcome {
    execute_tool_bounded_inner(
        handler,
        args,
        tool_name,
        budget,
        cancel_flag,
        semaphore,
        ExecutionHooks::none(),
        &RUNTIME_METRICS,
    )
    .await
}

/// Core implementation shared by production and test paths.
///
/// `metrics` must be `'static` because it is captured by `spawn_blocking`.
/// Production callers pass `&RUNTIME_METRICS` (a static). Test callers
/// use `Box::leak` to obtain a `'static` reference to isolated metrics.
#[allow(clippy::too_many_arguments)]
async fn execute_tool_bounded_inner(
    handler: registry::ToolHandler,
    args: Value,
    tool_name: String,
    budget: ToolBudget,
    cancel_flag: std::sync::Arc<AtomicBool>,
    semaphore: std::sync::Arc<tokio::sync::Semaphore>,
    hooks: ExecutionHooks,
    metrics: &'static runtime::RuntimeMetrics,
) -> ExecutionOutcome {
    let timeout_ms = budget.max_elapsed_ms;
    let tool_name_for_timeout = tool_name.clone();

    let lifecycle = std::sync::Arc::new(HandlerLifecycle::new());
    let lifecycle_for_timeout = lifecycle.clone();
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

        if let Some(ref notify) = hooks.permit_acquired {
            notify.notify_one();
        }

        if let Some(ref notify) = hooks.before_lifecycle_start {
            notify.notified().await;
        }

        match lifecycle.begin_running(metrics) {
            BeginRunning::CancelledBeforeStart => {
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
            BeginRunning::Run => {}
        }

        if let Some(ref notify) = hooks.running_established {
            notify.notify_one();
        }

        if cancel_flag.load(Ordering::Relaxed) {
            lifecycle.finish(metrics);
            return Ok(ToolResponse::error_with_code(
                "cancelled",
                machine_codes::CANCELLED,
                &format!("Tool '{}' request was cancelled", tool_name),
                None,
                Some(&tool_name),
            ));
        }

        let lifecycle_block = lifecycle.clone();
        let cancel_flag_block = cancel_flag.clone();

        tokio::task::spawn_blocking(move || {
            let _permit = permit;

            let mut mcp_eval_ctx = crate::calc::EvalContext::mcp_mode();
            let cancel_flag_handler = cancel_flag_block.clone();

            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                crate::mcp::budget::with_cancel_flag(Some(cancel_flag_handler), || {
                    crate::mcp::budget::with_eval_context(&mut mcp_eval_ctx, || handler(&args))
                })
            }));

            lifecycle_block.finish(metrics);

            match result {
                Ok(response) => response,
                Err(payload) => {
                    let msg = payload
                        .downcast_ref::<&str>()
                        .map(|s| s.to_string())
                        .or_else(|| payload.downcast_ref::<String>().cloned())
                        .unwrap_or_else(|| "handler panicked".to_string());
                    ToolResponse::error_with_code(
                        "internal_error",
                        crate::mcp::machine_codes::INTERNAL_ERROR,
                        &format!("Tool handler panicked: {}", msg),
                        None,
                        None,
                    )
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
            cancel_flag_for_timeout.store(true, Ordering::Relaxed);
            metrics.total_timeouts.fetch_add(1, Ordering::Relaxed);

            if let Some(ref notify) = hooks.before_timeout_lock {
                notify.notify_one();
            }

            match lifecycle_for_timeout.record_timeout(metrics) {
                TimeoutDisposition::Queued | TimeoutDisposition::Running => {}
                TimeoutDisposition::AlreadyFinished => {}
            }

            if let Some(ref notify) = hooks.timeout_transition_done {
                notify.notify_one();
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

/// Test-only entry point that accepts hooks and isolated metrics.
///
/// `metrics` is leaked (via `Box::leak`) to obtain a `&'static` reference
/// that can be captured by `spawn_blocking`. This is acceptable in tests.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn execute_tool_bounded_with_hooks(
    handler: registry::ToolHandler,
    args: Value,
    tool_name: String,
    budget: ToolBudget,
    cancel_flag: std::sync::Arc<AtomicBool>,
    semaphore: std::sync::Arc<tokio::sync::Semaphore>,
    hooks: ExecutionHooks,
    metrics: std::sync::Arc<runtime::RuntimeMetrics>,
) -> ExecutionOutcome {
    let metrics_ptr: *const runtime::RuntimeMetrics = std::sync::Arc::into_raw(metrics);
    // SAFETY: We converted from Arc, which guarantees the data is valid.
    // We intentionally leak the memory (Arc's strong count becomes effectively
    // immortal) so the reference is 'static for spawn_blocking. In tests this
    // is acceptable.
    let metrics_static: &'static runtime::RuntimeMetrics = unsafe { &*metrics_ptr };
    execute_tool_bounded_inner(
        handler,
        args,
        tool_name,
        budget,
        cancel_flag,
        semaphore,
        hooks,
        metrics_static,
    )
    .await
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
    use crate::mcp::runtime::MetricsSnapshot;
    use std::sync::atomic::AtomicUsize;
    use std::sync::Arc;

    fn assert_snapshot_invariant(snap: &MetricsSnapshot) {
        assert!(
            snap.timed_out_handlers <= snap.active_blocking_handlers,
            "INVARIANT VIOLATION: timed_out_handlers ({}) > active_blocking_handlers ({})",
            snap.timed_out_handlers,
            snap.active_blocking_handlers,
        );
    }

    fn snapshot_from_metrics(m: &runtime::RuntimeMetrics) -> MetricsSnapshot {
        MetricsSnapshot {
            active_requests: m.active_requests.load(Ordering::Relaxed),
            active_blocking_handlers: m.active_blocking_handlers.load(Ordering::Relaxed),
            timed_out_handlers: m.timed_out_handlers.load(Ordering::Relaxed),
            total_timeouts: m.total_timeouts.load(Ordering::Relaxed),
            peak_blocking_concurrency: m.peak_blocking_concurrency.load(Ordering::Relaxed),
        }
    }

    fn new_test_metrics() -> Arc<runtime::RuntimeMetrics> {
        Arc::new(runtime::RuntimeMetrics::new_for_test())
    }

    /// Reset all test handler statics to defaults.
    fn reset_test_handler_statics() {
        TEST_HANDLER_SHOULD_BLOCK.store(false, Ordering::SeqCst);
        TEST_HANDLER_RELEASED.store(false, Ordering::SeqCst);
    }

    // ── Test 1: queued_timeout_blocks_handler_after_permit_release ─────

    static TEST1_HANDLER_RAN: AtomicBool = AtomicBool::new(false);

    fn test1_handler(_args: &Value) -> ToolResponse {
        TEST1_HANDLER_RAN.store(true, Ordering::SeqCst);
        ToolResponse::success(serde_json::json!("done"), None)
    }

    #[tokio::test]
    async fn queued_timeout_blocks_handler_after_permit_release() {
        reset_test_handler_statics();
        let metrics = new_test_metrics();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(0));
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let budget = ToolBudget::CHEAP.with_max_elapsed_ms(10);

        TEST1_HANDLER_RAN.store(false, Ordering::SeqCst);

        let outcome = execute_tool_bounded_with_hooks(
            test1_handler as registry::ToolHandler,
            Value::Object(serde_json::Map::new()),
            "test_tool".to_string(),
            budget,
            cancel_flag.clone(),
            semaphore.clone(),
            ExecutionHooks::none(),
            metrics.clone(),
        )
        .await;

        assert!(outcome.timed_out);
        assert!(!TEST1_HANDLER_RAN.load(Ordering::SeqCst));

        semaphore.add_permits(1);
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert!(
            !TEST1_HANDLER_RAN.load(Ordering::SeqCst),
            "handler must not run after queued timeout"
        );
        assert_snapshot_invariant(&snapshot_from_metrics(&metrics));
    }

    // ── Test 2: timeout_after_permit_but_before_closure_start ──────────
    //
    // The handler sleeps deterministically, guaranteeing the tokio timeout
    // fires while the handler is running.

    #[tokio::test]
    async fn timeout_after_permit_but_before_closure_start() {
        let metrics = new_test_metrics();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let budget = ToolBudget::CHEAP.with_max_elapsed_ms(10);

        let outcome = execute_tool_bounded_with_hooks(
            |_args| {
                std::thread::sleep(Duration::from_millis(200));
                ToolResponse::success(serde_json::json!("ok"), None)
            },
            Value::Object(serde_json::Map::new()),
            "test_tool".to_string(),
            budget,
            cancel_flag.clone(),
            semaphore.clone(),
            ExecutionHooks::none(),
            metrics.clone(),
        )
        .await;

        assert!(outcome.timed_out);
        tokio::time::sleep(Duration::from_millis(250)).await;
        assert_snapshot_invariant(&snapshot_from_metrics(&metrics));
    }

    // ── Test 3: running_timeout_increments_exactly_once ────────────────

    #[tokio::test]
    async fn running_timeout_increments_exactly_once() {
        let metrics = new_test_metrics();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let budget = ToolBudget::CHEAP.with_max_elapsed_ms(10);

        let outcome = execute_tool_bounded_with_hooks(
            |_args| {
                std::thread::sleep(Duration::from_millis(200));
                ToolResponse::success(serde_json::json!("ok"), None)
            },
            Value::Object(serde_json::Map::new()),
            "test_tool".to_string(),
            budget,
            cancel_flag.clone(),
            semaphore.clone(),
            ExecutionHooks::none(),
            metrics.clone(),
        )
        .await;

        assert!(outcome.timed_out);
        // Handler is still sleeping — timed_out_handlers must be exactly 1.
        assert_eq!(
            metrics.timed_out_handlers.load(Ordering::Relaxed),
            1,
            "timed_out_handlers should be exactly 1 while handler is still running"
        );

        // Wait for handler to finish and decrement.
        tokio::time::sleep(Duration::from_millis(250)).await;

        assert_eq!(
            metrics.timed_out_handlers.load(Ordering::Relaxed),
            0,
            "timed_out_handlers must return to 0 after handler finishes"
        );
        assert_snapshot_invariant(&snapshot_from_metrics(&metrics));
    }

    // ── Test 4: completion_wins_race ───────────────────────────────────

    #[tokio::test]
    async fn completion_wins_race() {
        reset_test_handler_statics();
        let metrics = new_test_metrics();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let budget = ToolBudget::CHEAP.with_max_elapsed_ms(1);

        let _outcome = execute_tool_bounded_with_hooks(
            test_handler_fast as registry::ToolHandler,
            Value::Object(serde_json::Map::new()),
            "test_tool".to_string(),
            budget,
            cancel_flag.clone(),
            semaphore.clone(),
            ExecutionHooks::none(),
            metrics.clone(),
        )
        .await;

        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_snapshot_invariant(&snapshot_from_metrics(&metrics));
    }

    // ── Test 5: timeout_wins_race ──────────────────────────────────────

    #[tokio::test]
    async fn timeout_wins_race() {
        reset_test_handler_statics();
        let metrics = new_test_metrics();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let budget = ToolBudget::CHEAP.with_max_elapsed_ms(10);

        let outcome = execute_tool_bounded_with_hooks(
            test_handler_slow_cancel as registry::ToolHandler,
            Value::Object(serde_json::Map::new()),
            "test_tool".to_string(),
            budget,
            cancel_flag.clone(),
            semaphore.clone(),
            ExecutionHooks::none(),
            metrics.clone(),
        )
        .await;

        assert!(outcome.timed_out);
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_snapshot_invariant(&snapshot_from_metrics(&metrics));
    }

    // ── Test 6: panic_after_timeout_corrects_gauges ───────────────────

    #[tokio::test]
    async fn panic_after_timeout_corrects_gauges() {
        let metrics = new_test_metrics();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let budget = ToolBudget::CHEAP.with_max_elapsed_ms(10);

        let outcome = execute_tool_bounded_with_hooks(
            |_args| {
                std::thread::sleep(Duration::from_millis(200));
                panic!("intentional test panic");
            },
            Value::Object(serde_json::Map::new()),
            "test_tool".to_string(),
            budget,
            cancel_flag.clone(),
            semaphore.clone(),
            ExecutionHooks::none(),
            metrics.clone(),
        )
        .await;

        assert!(outcome.timed_out);

        // Wait for the handler to panic and lifecycle cleanup to complete.
        tokio::time::sleep(Duration::from_millis(300)).await;

        assert_eq!(
            metrics.timed_out_handlers.load(Ordering::Relaxed),
            0,
            "timed_out_handlers must be 0 after panic cleanup"
        );
        assert_eq!(
            metrics.active_blocking_handlers.load(Ordering::Relaxed),
            0,
            "active_blocking_handlers must be 0 after panic cleanup"
        );
        assert_snapshot_invariant(&snapshot_from_metrics(&metrics));
    }

    // ── Test 7: cancellation_flag_visible_after_timeout ────────────────

    #[tokio::test]
    async fn cancellation_flag_visible_after_timeout() {
        let metrics = new_test_metrics();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let budget = ToolBudget::CHEAP.with_max_elapsed_ms(10);

        let outcome = execute_tool_bounded_with_hooks(
            |_args| {
                // Sleep so timeout fires, then check cancel flag.
                std::thread::sleep(Duration::from_millis(200));
                let cancelled = crate::mcp::budget::current_cancel_flag()
                    .is_some_and(|f| f.load(Ordering::Relaxed));
                if cancelled {
                    ToolResponse::error_with_code(
                        "cancelled",
                        machine_codes::CANCELLED,
                        "cooperative cancel",
                        None,
                        None,
                    )
                } else {
                    ToolResponse::success(serde_json::json!("ok"), None)
                }
            },
            Value::Object(serde_json::Map::new()),
            "test_tool".to_string(),
            budget,
            cancel_flag.clone(),
            semaphore.clone(),
            ExecutionHooks::none(),
            metrics.clone(),
        )
        .await;

        assert!(outcome.timed_out);
        assert!(cancel_flag.load(Ordering::Relaxed));

        tokio::time::sleep(Duration::from_millis(250)).await;
        assert_snapshot_invariant(&snapshot_from_metrics(&metrics));
    }

    // ── Test 8: no_double_completion ───────────────────────────────────

    #[tokio::test]
    async fn no_double_completion() {
        reset_test_handler_statics();
        let metrics = new_test_metrics();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let budget = ToolBudget::CHEAP;

        let outcome = execute_tool_bounded_with_hooks(
            test_handler_fast as registry::ToolHandler,
            Value::Object(serde_json::Map::new()),
            "test_tool".to_string(),
            budget,
            cancel_flag.clone(),
            semaphore.clone(),
            ExecutionHooks::none(),
            metrics.clone(),
        )
        .await;

        assert!(!outcome.timed_out);
        assert!(outcome.tool_response.is_ok());
        tokio::time::sleep(Duration::from_millis(10)).await;

        assert_snapshot_invariant(&snapshot_from_metrics(&metrics));
    }

    // ── Test 9: hundreds_of_controlled_interleavings ───────────────────

    #[tokio::test]
    async fn five_hundred_controlled_interleavings() {
        reset_test_handler_statics();
        let metrics = new_test_metrics();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let mut timeouts_observed = 0usize;
        let mut successes_observed = 0usize;

        for iter in 0..500 {
            let cancel_flag = Arc::new(AtomicBool::new(false));
            TEST_HANDLER_SHOULD_BLOCK.store(false, Ordering::SeqCst);
            TEST_HANDLER_RELEASED.store(false, Ordering::SeqCst);

            let (handler, budget) = if iter % 2 == 0 {
                (
                    test_handler_fast as registry::ToolHandler,
                    ToolBudget::CHEAP.with_max_elapsed_ms(5000),
                )
            } else {
                (
                    test_handler_blocking as registry::ToolHandler,
                    ToolBudget::CHEAP.with_max_elapsed_ms(5),
                )
            };

            if iter % 2 != 0 {
                TEST_HANDLER_SHOULD_BLOCK.store(true, Ordering::SeqCst);
            }

            let outcome = execute_tool_bounded_with_hooks(
                handler,
                Value::Object(serde_json::Map::new()),
                "race_tool".to_string(),
                budget,
                cancel_flag,
                semaphore.clone(),
                ExecutionHooks::none(),
                metrics.clone(),
            )
            .await;

            if outcome.timed_out {
                timeouts_observed += 1;
                TEST_HANDLER_SHOULD_BLOCK.store(false, Ordering::SeqCst);
                TEST_HANDLER_RELEASED.store(true, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(20)).await;
            } else {
                successes_observed += 1;
            }

            assert_snapshot_invariant(&snapshot_from_metrics(&metrics));
        }

        assert!(
            successes_observed > 0,
            "should have at least some successful completions"
        );
        assert!(timeouts_observed > 0, "should have at least some timeouts");
        assert_snapshot_invariant(&snapshot_from_metrics(&metrics));
    }

    // ── Test 10: worker_bound_never_exceeded ───────────────────────────

    #[tokio::test]
    async fn worker_bound_never_exceeded() {
        reset_test_handler_statics();
        let max_workers = 4usize;
        let metrics = new_test_metrics();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(max_workers));
        let num_tasks = 16;
        let active = Arc::new(AtomicUsize::new(0));
        let peak_observed = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..num_tasks {
            let sem = semaphore.clone();
            let active = active.clone();
            let peak = peak_observed.clone();
            handles.push(tokio::spawn(async move {
                let _permit = sem.acquire_owned().await.unwrap();
                let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(current, Ordering::SeqCst);
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
        assert_snapshot_invariant(&snapshot_from_metrics(&metrics));
    }

    // ── Basic execution test ───────────────────────────────────────────

    #[tokio::test]
    async fn basic_execution_completes_successfully() {
        reset_test_handler_statics();
        let metrics = new_test_metrics();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let tool_budget = ToolBudget::CHEAP;

        let outcome = execute_tool_bounded_with_hooks(
            |_args| ToolResponse::success(serde_json::json!("hello"), None),
            Value::Object(serde_json::Map::new()),
            "test_tool".to_string(),
            tool_budget,
            cancel_flag.clone(),
            semaphore.clone(),
            ExecutionHooks::none(),
            metrics.clone(),
        )
        .await;

        assert!(!outcome.timed_out);
        let resp = outcome.tool_response.unwrap();
        assert!(resp.error.is_none());
        assert_eq!(resp.result.as_ref().unwrap().as_str().unwrap(), "hello");
        assert_snapshot_invariant(&snapshot_from_metrics(&metrics));
    }

    #[tokio::test]
    async fn handler_panic_returns_blocking_handler_count_to_baseline() {
        let metrics = new_test_metrics();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let budget = ToolBudget::CHEAP;

        fn always_panic_handler(_args: &Value) -> ToolResponse {
            panic!("intentional test panic");
        }

        let outcome = execute_tool_bounded_with_hooks(
            always_panic_handler as registry::ToolHandler,
            Value::Object(serde_json::Map::new()),
            "panic_tool".to_string(),
            budget,
            cancel_flag,
            semaphore,
            ExecutionHooks::none(),
            metrics.clone(),
        )
        .await;

        assert!(!outcome.timed_out);
        // Panic is caught by catch_unwind and converted to an error ToolResponse.
        let resp = outcome.tool_response.unwrap();
        assert!(!resp.ok, "panicked handler should return ok=false response");

        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_snapshot_invariant(&snapshot_from_metrics(&metrics));
    }

    #[tokio::test]
    async fn timeout_response_returns_while_handler_continues() {
        reset_test_handler_statics();
        let metrics = new_test_metrics();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let budget = ToolBudget::CHEAP.with_max_elapsed_ms(10);

        let outcome = execute_tool_bounded_with_hooks(
            |_args| {
                std::thread::sleep(Duration::from_millis(100));
                ToolResponse::success(serde_json::json!("late"), None)
            },
            Value::Object(serde_json::Map::new()),
            "slow_tool".to_string(),
            budget,
            cancel_flag,
            semaphore,
            ExecutionHooks::none(),
            metrics.clone(),
        )
        .await;

        assert!(outcome.timed_out);
        let resp = outcome.tool_response.unwrap();
        assert!(!resp.ok);

        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_snapshot_invariant(&snapshot_from_metrics(&metrics));
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
