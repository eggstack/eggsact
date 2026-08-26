use crate::mcp::budget::ToolBudget;
use crate::mcp::machine_codes;
use crate::mcp::registry;
use crate::mcp::response::{python_json_dumps, sanitize_error, truncate_response, ToolResponse};
use crate::mcp::runtime::{self, MAX_OUTPUT_BYTES, RUNTIME_METRICS};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
#[allow(unused_imports)]
use std::sync::{Arc, Mutex};
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
    Error,
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
                let current = metrics
                    .active_blocking_handlers
                    .fetch_add(1, Ordering::Relaxed)
                    + 1;
                metrics
                    .peak_blocking_concurrency
                    .fetch_max(current, Ordering::Relaxed);
                *phase = HandlerPhase::Running;
                BeginRunning::Run
            }
            HandlerPhase::TimedOutQueued => BeginRunning::CancelledBeforeStart,
            other => {
                debug_assert!(
                    false,
                    "begin_running called in unexpected phase: {:?}",
                    other
                );
                BeginRunning::Error
            }
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
                *phase = HandlerPhase::Finished;
            }
        }
    }
}

// ── Test gates (cfg(test) — excluded from release builds) ────────────────

#[cfg(test)]
/// Blocking-side test gate for hook sites inside `spawn_blocking`.
///
/// `arrive_and_wait` signals arrival and then blocks the calling thread
/// until the test releases it. `wait_until_entered` is async and uses
/// a `Notify` so tests can poll it from a Tokio runtime.
#[allow(dead_code)]
#[derive(Clone)]
pub(crate) struct BlockingTestGate {
    entered: Arc<tokio::sync::Notify>,
    state: Arc<(std::sync::Mutex<bool>, std::sync::Condvar)>,
}

#[cfg(test)]
#[allow(dead_code)]
impl BlockingTestGate {
    pub fn new() -> Self {
        Self {
            entered: Arc::new(tokio::sync::Notify::new()),
            state: Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new())),
        }
    }

    /// Signal that the boundary was reached, then block until released.
    /// Must only be called from `spawn_blocking` or test-owned OS threads.
    pub fn arrive_and_wait(&self) {
        self.entered.notify_one();
        let (lock, cv) = &*self.state;
        let mut released = lock.lock().unwrap();
        while !*released {
            released = cv.wait(released).unwrap();
        }
    }

    /// Wait until the gate reports arrival. Async, for use from test code.
    pub async fn wait_until_entered(&self) {
        self.entered.notified().await;
    }

    /// Release the gate so `arrive_and_wait` can return.
    pub fn release(&self) {
        let (lock, cv) = &*self.state;
        *lock.lock().unwrap() = true;
        cv.notify_all();
    }
}

#[cfg(test)]
impl Default for BlockingTestGate {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
/// One-way notification emitted when a blocking execution closure exits.
///
/// This is used only by deterministic tests. `Notify` retains one permit when
/// the closure exits before the test begins waiting, so the observation is not
/// scheduler-sensitive.
#[derive(Clone, Default)]
pub(crate) struct ClosureExitSignal {
    exited: Arc<tokio::sync::Notify>,
}

#[cfg(test)]
#[allow(dead_code)]
impl ClosureExitSignal {
    pub fn new() -> Self {
        Self {
            exited: Arc::new(tokio::sync::Notify::new()),
        }
    }

    pub fn signal(&self) {
        self.exited.notify_one();
    }

    pub async fn wait(&self) {
        self.exited.notified().await;
    }
}

#[cfg(test)]
/// Async-side test gate for hook sites in async code (e.g. the timeout path).
///
/// `arrive_and_wait` is async and must not be called from `spawn_blocking`.
#[allow(dead_code)]
#[derive(Clone, Default)]
pub(crate) struct AsyncTestGate {
    entered: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Notify>,
}

#[cfg(test)]
#[allow(dead_code)]
impl AsyncTestGate {
    pub fn new() -> Self {
        Self {
            entered: Arc::new(tokio::sync::Notify::new()),
            release: Arc::new(tokio::sync::Notify::new()),
        }
    }

    /// Signal arrival, then wait for release. Async — do not call from
    /// `spawn_blocking`.
    pub async fn arrive_and_wait(&self) {
        self.entered.notify_one();
        self.release.notified().await;
    }

    /// Wait until the gate reports arrival.
    pub async fn wait_until_entered(&self) {
        self.entered.notified().await;
    }

    /// Release the gate so `arrive_and_wait` can return.
    pub fn release(&self) {
        self.release.notify_one();
    }
}

// ── Test hooks ──────────────────────────────────────────────────────────

/// Test-only hooks for deterministic execution testing.
///
/// Each hook is an optional gate that, when `Some`, pauses execution at
/// that lifecycle boundary until the test releases it. `None` means no
/// waiting — the production no-op path.
///
/// In non-test builds all fields are compiled away (zero-sized struct),
/// so `ExecutionHooks::none()` is a no-op constructor and none of the
/// gate types are linked into the binary.
#[derive(Clone)]
pub(crate) struct ExecutionHooks {
    /// Diagnostic-only notification after permit acquisition (async side).
    #[cfg(test)]
    pub permit_acquired: Option<Arc<tokio::sync::Notify>>,
    /// Inside the blocking closure, after closure entry but before
    /// `begin_running`.
    #[cfg(test)]
    pub before_begin_running: Option<Arc<BlockingTestGate>>,
    /// After `begin_running` and active-handler accounting are complete.
    #[cfg(test)]
    pub running_established: Option<Arc<BlockingTestGate>>,
    /// Immediately before invoking the handler, after the cancellation check.
    #[cfg(test)]
    pub before_handler: Option<Arc<BlockingTestGate>>,
    /// After caller timeout and cancellation signaling, but before
    /// `record_timeout` takes the lifecycle lock (async side).
    #[cfg(test)]
    pub before_timeout_record: Option<Arc<AsyncTestGate>>,
    /// After `record_timeout` completes (async side).
    #[cfg(test)]
    pub timeout_recorded: Option<Arc<AsyncTestGate>>,
    /// After handler return or caught panic, but before lifecycle `finish`.
    #[cfg(test)]
    pub before_finish: Option<Arc<BlockingTestGate>>,
    /// After lifecycle completion and gauge correction.
    #[cfg(test)]
    pub finished: Option<Arc<BlockingTestGate>>,
    /// Signalled exactly once when the blocking closure exits.
    #[cfg(test)]
    pub closure_exited: Option<Arc<ClosureExitSignal>>,
}

impl ExecutionHooks {
    #[cfg(test)]
    pub fn none() -> Self {
        Self {
            permit_acquired: None,
            before_begin_running: None,
            running_established: None,
            before_handler: None,
            before_timeout_record: None,
            timeout_recorded: None,
            before_finish: None,
            finished: None,
            closure_exited: None,
        }
    }

    #[cfg(not(test))]
    pub fn none() -> Self {
        Self {}
    }
}

// ── Test handler statics ────────────────────────────────────────────────
//
// Since `ToolHandler` is `fn(&Value) -> ToolResponse` (function pointer),
// closures that capture state cannot be used. Tests communicate with
// handlers through static backing arrays protected by exclusive RAII leases.

#[cfg(test)]
const TEST_BLOCK_SLOTS: usize = 16;

#[cfg(test)]
static BLOCK_SLOTS: [AtomicBool; TEST_BLOCK_SLOTS] =
    [const { AtomicBool::new(false) }; TEST_BLOCK_SLOTS];

#[cfg(test)]
static RELEASE_SLOTS: [AtomicBool; TEST_BLOCK_SLOTS] =
    [const { AtomicBool::new(false) }; TEST_BLOCK_SLOTS];

#[cfg(test)]
static SLOT_IN_USE: [AtomicBool; TEST_BLOCK_SLOTS] =
    [const { AtomicBool::new(false) }; TEST_BLOCK_SLOTS];

#[cfg(test)]
struct TestSlotLease {
    index: usize,
}

#[cfg(test)]
impl TestSlotLease {
    fn index(&self) -> usize {
        self.index
    }

    fn block(&self) {
        BLOCK_SLOTS[self.index].store(true, Ordering::SeqCst);
        RELEASE_SLOTS[self.index].store(false, Ordering::SeqCst);
    }

    fn release(&self) {
        BLOCK_SLOTS[self.index].store(false, Ordering::SeqCst);
        RELEASE_SLOTS[self.index].store(true, Ordering::SeqCst);
    }
}

#[cfg(test)]
impl Drop for TestSlotLease {
    fn drop(&mut self) {
        BLOCK_SLOTS[self.index].store(false, Ordering::SeqCst);
        RELEASE_SLOTS[self.index].store(false, Ordering::SeqCst);
        SLOT_IN_USE[self.index].store(false, Ordering::Release);
    }
}

#[cfg(test)]
fn acquire_test_slot() -> TestSlotLease {
    for (index, slot_in_use) in SLOT_IN_USE.iter().enumerate() {
        if slot_in_use
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            return TestSlotLease { index };
        }
    }
    panic!("no test blocking slots available");
}

#[cfg(test)]
fn test_handler_blocking_slot(args: &Value) -> ToolResponse {
    let slot = args
        .get("_block_slot")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize
        % TEST_BLOCK_SLOTS;
    while BLOCK_SLOTS[slot].load(Ordering::SeqCst) {
        if RELEASE_SLOTS[slot].load(Ordering::SeqCst) {
            break;
        }
        std::hint::spin_loop();
    }
    ToolResponse::success(serde_json::json!("ok"), None)
}

#[cfg(test)]
fn block_slot_args(slot: &TestSlotLease) -> Value {
    serde_json::json!({ "_block_slot": slot.index() })
}

#[cfg(test)]
fn test_handler_fast(_args: &Value) -> ToolResponse {
    ToolResponse::success(serde_json::json!("ok"), None)
}

// ── Test handler for mutable-context commit/rollback tests ─────────────
//
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

/// Ensures the calculator regex cache warm-up has been started, exactly once
/// per process, on a detached OS thread (release builds only).
///
/// See the comment at the call site in [`execute_tool_bounded_inner`] for why
/// this must not run inline on an async runtime thread and why debug builds
/// skip warm-up entirely.
#[cfg(not(debug_assertions))]
fn ensure_calculator_warmed_detached() {
    static CALC_WARMUP_STARTED: AtomicBool = AtomicBool::new(false);
    if CALC_WARMUP_STARTED.swap(true, Ordering::AcqRel) {
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("calc-regex-warmup".into())
        .spawn(crate::calc::normalize::warm_calculator_regex_cache);
    if let Err(err) = spawned {
        // Thread spawn failed (thread-creator limits): fall back to warming
        // inline. Slow, but correct — and better than never compiling the
        // patterns at all.
        CALC_WARMUP_STARTED.store(false, Ordering::Release);
        crate::calc::normalize::warm_calculator_regex_cache();
        let _ = err;
    }
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
    _hooks: ExecutionHooks,
    metrics: &'static runtime::RuntimeMetrics,
) -> ExecutionOutcome {
    // Warm one-time calculator initialization before starting the bounded
    // dispatch window. This keeps regex compilation out of the first call's
    // elapsed-time budget for both MCP and in-process execution.
    //
    // Release builds only. In debug builds these ~40 lazy regex compilations
    // (several embedding the large UNIT_ALT alternation) take multiple
    // wall-clock SECONDS of full-CPU time per process — which both starved the
    // runtime timer wheel when run inline (hanging CI's cooperative-cancel
    // test) and, even detached, added that cost to EVERY short-lived process:
    // each CLI invocation and each MCP server subprocess in the integration
    // test suite. Debug builds therefore keep plain lazy initialization.
    //
    // The warm-up is also started on a detached OS thread rather than inline:
    // running it on the calling async thread delayed arming the bounded
    // `tokio::time::timeout` below by seconds and starved any in-flight
    // watchdog timers. A detached thread keeps the timer wheel free; LazyLock
    // initialization makes later concurrent derefs safe once compilation
    // finishes. Trade-off: a request racing the background compile may still
    // charge some remaining compilation to its budget — strictly better than
    // blocking every caller's runtime thread behind one initializer.
    #[cfg(not(debug_assertions))]
    ensure_calculator_warmed_detached();
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

        #[cfg(test)]
        if let Some(ref notify) = _hooks.permit_acquired {
            notify.notify_one();
        }

        let lifecycle_block = lifecycle.clone();
        let cancel_flag_block = cancel_flag.clone();
        let tool_name_block = tool_name.clone();
        #[cfg(test)]
        let hooks_for_block = ExecutionHooks {
            permit_acquired: None,
            before_begin_running: _hooks.before_begin_running.clone(),
            running_established: _hooks.running_established.clone(),
            before_handler: _hooks.before_handler.clone(),
            before_timeout_record: None,
            timeout_recorded: None,
            before_finish: _hooks.before_finish.clone(),
            finished: _hooks.finished.clone(),
            closure_exited: _hooks.closure_exited.clone(),
        };

        tokio::task::spawn_blocking(move || {
            #[cfg(test)]
            struct ClosureExitGuard(Option<Arc<ClosureExitSignal>>);

            #[cfg(test)]
            impl Drop for ClosureExitGuard {
                fn drop(&mut self) {
                    if let Some(signal) = self.0.take() {
                        signal.signal();
                    }
                }
            }

            #[cfg(test)]
            let _closure_exit = ClosureExitGuard(hooks_for_block.closure_exited.clone());
            let _permit = permit;

            // Signal that the blocking closure has been entered, before
            // any lifecycle transition. Tests use this to coordinate
            // exactly when the closure starts vs. when the outer timeout fires.
            #[cfg(test)]
            if let Some(ref gate) = hooks_for_block.before_begin_running {
                gate.arrive_and_wait();
            }

            // Begin running inside the blocking closure, not before it.
            // This ensures `active_blocking_handlers` only counts closures
            // that have actually started executing.
            match lifecycle_block.begin_running(metrics) {
                BeginRunning::CancelledBeforeStart => {
                    return ToolResponse::error_with_code(
                        "cancelled",
                        machine_codes::CANCELLED,
                        &format!(
                            "Tool '{}' request was cancelled (timed out while queued)",
                            tool_name_block
                        ),
                        Some(vec![
                            "The request was cancelled before execution started".to_string()
                        ]),
                        Some(&tool_name_block),
                    );
                }
                BeginRunning::Run => {}
                BeginRunning::Error => {
                    lifecycle_block.finish(metrics);
                    return ToolResponse::error_with_code(
                        "internal_error",
                        machine_codes::INTERNAL_ERROR,
                        &format!(
                            "Tool '{}' lifecycle error: begin_running called in unexpected phase",
                            tool_name_block
                        ),
                        None,
                        Some(&tool_name_block),
                    );
                }
            }

            #[cfg(test)]
            if let Some(ref gate) = hooks_for_block.running_established {
                gate.arrive_and_wait();
            }

            if cancel_flag_block.load(Ordering::Acquire) {
                lifecycle_block.finish(metrics);
                return ToolResponse::error_with_code(
                    "cancelled",
                    machine_codes::CANCELLED,
                    &format!("Tool '{}' request was cancelled", tool_name_block),
                    None,
                    Some(&tool_name_block),
                );
            }

            #[cfg(test)]
            if let Some(ref gate) = hooks_for_block.before_handler {
                gate.arrive_and_wait();
            }

            let mut mcp_eval_ctx = crate::calc::EvalContext::mcp_mode();
            let cancel_flag_handler = cancel_flag_block.clone();

            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                crate::mcp::budget::with_cancel_flag(Some(cancel_flag_handler), || {
                    crate::mcp::budget::with_eval_context(&mut mcp_eval_ctx, || handler(&args))
                })
            }));

            #[cfg(test)]
            if let Some(ref gate) = hooks_for_block.before_finish {
                gate.arrive_and_wait();
            }

            lifecycle_block.finish(metrics);

            #[cfg(test)]
            if let Some(ref gate) = hooks_for_block.finished {
                gate.arrive_and_wait();
            }

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
            cancel_flag_for_timeout.store(true, Ordering::Release);
            metrics.total_timeouts.fetch_add(1, Ordering::Relaxed);

            #[cfg(test)]
            if let Some(ref gate) = _hooks.before_timeout_record {
                gate.arrive_and_wait().await;
            }

            match lifecycle_for_timeout.record_timeout(metrics) {
                TimeoutDisposition::Queued | TimeoutDisposition::Running => {}
                TimeoutDisposition::AlreadyFinished => {}
            }

            #[cfg(test)]
            if let Some(ref gate) = _hooks.timeout_recorded {
                gate.arrive_and_wait().await;
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
///
/// `id` is the originating request ID, attached to the error response when
/// the handler task's join fails (JSON-RPC 2.0 requires `id` on responses
/// to requests so clients can correlate them).
pub(crate) fn build_tool_response(
    outcome: ExecutionOutcome,
    tool_name: &str,
    budget: &ToolBudget,
    id: Option<Value>,
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
        Err(join_err) => crate::mcp::protocol::json_rpc_error(
            -32000,
            format!(
                "Tool execution error: {}",
                runtime::truncate_2000(&sanitize_error(&join_err.to_string()))
            ),
            id,
        ),
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::runtime::MetricsSnapshot;
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

    #[test]
    fn slot_leases_are_exclusive_under_parallel_allocation() {
        let workers = 8;
        let barrier = Arc::new(std::sync::Barrier::new(workers));
        let claimed = Arc::new([const { AtomicBool::new(false) }; TEST_BLOCK_SLOTS]);
        let mut handles = Vec::new();

        for _ in 0..workers {
            let barrier = barrier.clone();
            let claimed = claimed.clone();
            handles.push(std::thread::spawn(move || {
                barrier.wait();
                for _ in 0..100 {
                    let lease = acquire_test_slot();
                    let index = lease.index();
                    assert!(
                        claimed[index]
                            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                            .is_ok(),
                        "slot {} was allocated to two parallel test invocations",
                        index
                    );
                    std::thread::yield_now();
                    assert!(claimed[index].swap(false, Ordering::Release));
                    drop(lease);
                }
            }));
        }

        for handle in handles {
            handle
                .join()
                .expect("slot lease stress thread must not panic");
        }
    }

    // ── Smoke: queued timeout does not run handler ──────────────────────
    //
    // Timing-based behavior verification (not a gate-controlled test).
    // Uses a 0-permit semaphore so the handler never acquires a permit.

    static TEST1_HANDLER_RAN: AtomicBool = AtomicBool::new(false);

    fn test1_handler(_args: &Value) -> ToolResponse {
        TEST1_HANDLER_RAN.store(true, Ordering::SeqCst);
        ToolResponse::success(serde_json::json!("done"), None)
    }

    #[tokio::test]
    async fn queued_timeout_smoke_does_not_run_handler() {
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

    // ── Smoke: timeout returns while handler continues ──────────────────
    //
    // Timing-based behavior verification (not a gate-controlled test).
    // The handler sleeps deterministically, guaranteeing the tokio timeout
    // fires while the handler is running.

    #[tokio::test]
    async fn timeout_smoke_returns_while_handler_continues() {
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

    // ── Smoke: running timeout increments exactly once ──────────────────

    #[tokio::test]
    async fn running_timeout_smoke_increments_once() {
        let metrics = new_test_metrics();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let budget = ToolBudget::CHEAP.with_max_elapsed_ms(10);
        let finished = Arc::new(BlockingTestGate::new());

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
            ExecutionHooks {
                finished: Some(finished.clone()),
                ..ExecutionHooks::none()
            },
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

        // Wait for the lifecycle to finish and decrement. The hook is after
        // `HandlerLifecycle::finish`, so this is an exact synchronization
        // point rather than a scheduler-dependent delay.
        finished.wait_until_entered().await;

        assert_eq!(
            metrics.timed_out_handlers.load(Ordering::Relaxed),
            0,
            "timed_out_handlers must return to 0 after handler finishes"
        );
        assert_snapshot_invariant(&snapshot_from_metrics(&metrics));
        finished.release();
    }

    // ── Smoke: no double completion ────────────────────────────────────

    #[tokio::test]
    async fn no_double_completion_smoke() {
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

    // ── Basic execution test ───────────────────────────────────────────

    #[tokio::test]
    async fn basic_execution_completes_successfully() {
        let metrics = new_test_metrics();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let tool_budget = ToolBudget::CHEAP;
        let closure_exited = Arc::new(ClosureExitSignal::new());

        let outcome = execute_tool_bounded_with_hooks(
            |_args| ToolResponse::success(serde_json::json!("hello"), None),
            Value::Object(serde_json::Map::new()),
            "test_tool".to_string(),
            tool_budget,
            cancel_flag.clone(),
            semaphore.clone(),
            ExecutionHooks {
                closure_exited: Some(closure_exited.clone()),
                ..ExecutionHooks::none()
            },
            metrics.clone(),
        )
        .await;

        assert!(!outcome.timed_out);
        let resp = outcome.tool_response.unwrap();
        assert!(resp.error.is_none());
        assert_eq!(resp.result.as_ref().unwrap().as_str().unwrap(), "hello");
        assert!(
            tokio::time::timeout(Duration::from_secs(5), closure_exited.wait())
                .await
                .is_ok(),
            "normal blocking closure must signal exit"
        );
        assert_snapshot_invariant(&snapshot_from_metrics(&metrics));
    }

    // ── Smoke: panic cleanup returns gauges to baseline ─────────────────

    #[tokio::test]
    async fn panic_cleanup_smoke() {
        let metrics = new_test_metrics();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let budget = ToolBudget::CHEAP;
        let closure_exited = Arc::new(ClosureExitSignal::new());

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
            ExecutionHooks {
                closure_exited: Some(closure_exited.clone()),
                ..ExecutionHooks::none()
            },
            metrics.clone(),
        )
        .await;

        assert!(!outcome.timed_out);
        // Panic is caught by catch_unwind and converted to an error ToolResponse.
        let resp = outcome.tool_response.unwrap();
        assert!(!resp.ok, "panicked handler should return ok=false response");
        assert_eq!(
            resp.machine_code.as_deref(),
            Some(machine_codes::INTERNAL_ERROR)
        );

        assert!(
            tokio::time::timeout(Duration::from_secs(5), closure_exited.wait())
                .await
                .is_ok(),
            "caught-panic blocking closure must signal exit"
        );
        assert_snapshot_invariant(&snapshot_from_metrics(&metrics));
    }

    // ── Smoke: timeout returns while handler continues after return ─────

    #[tokio::test]
    async fn timeout_smoke_handler_continues_after_return() {
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
// Deterministic lifecycle tests: gate-controlled exact interleavings
//
// These tests use ExecutionHooks gates to control exact lifecycle transitions.
// Sleeps are used only as bounded "did not happen" observations, never to
// establish ordering. Each test uses unique per-invocation gate state.
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod deterministic_tests {
    use super::*;
    use crate::mcp::runtime::MetricsSnapshot;
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

    fn blocking_gate() -> Arc<BlockingTestGate> {
        Arc::new(BlockingTestGate::new())
    }

    fn async_gate() -> Arc<AsyncTestGate> {
        Arc::new(AsyncTestGate::new())
    }

    // ── Test A: timeout after permit but before lifecycle start ─────────
    //
    // 1. Use a semaphore with one permit.
    // 2. Install a blocking `before_begin_running` gate.
    // 3. Spawn the coordinator with a short but nonzero budget.
    // 4. Wait until the blocking closure reaches `before_begin_running`.
    // 5. Do not release the gate until the caller-facing timeout has returned.
    // 6. Assert the timeout response was returned.
    // 7. Assert `total_timeouts == 1`.
    // 8. Assert `active_blocking_handlers == 0` and `timed_out_handlers == 0`
    //    while the closure remains gated.
    // 9. Release `before_begin_running`.
    // 10. Prove the actual handler body was never invoked.
    // 11. Wait for the detached closure to exit via `closure_exited`.
    // 12. Assert all gauges remain zero except cumulative timeout count.

    #[tokio::test]
    async fn timeout_after_permit_before_lifecycle_start() {
        static HANDLER_RAN: AtomicBool = AtomicBool::new(false);
        HANDLER_RAN.store(false, Ordering::SeqCst);

        let metrics = new_test_metrics();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let budget = ToolBudget::CHEAP.with_max_elapsed_ms(50);

        let before_begin_running = blocking_gate();
        let closure_exited = Arc::new(ClosureExitSignal::new());
        let hooks = ExecutionHooks {
            before_begin_running: Some(before_begin_running.clone()),
            closure_exited: Some(closure_exited.clone()),
            ..ExecutionHooks::none()
        };

        // Spawn the coordinator so we can interact with gates while it runs.
        let call = tokio::spawn(execute_tool_bounded_with_hooks(
            |_args| {
                HANDLER_RAN.store(true, Ordering::SeqCst);
                ToolResponse::success(serde_json::json!("done"), None)
            },
            Value::Object(serde_json::Map::new()),
            "test_tool".to_string(),
            budget,
            cancel_flag,
            semaphore,
            hooks,
            metrics.clone(),
        ));

        // 4. Wait until the blocking closure reaches before_begin_running.
        before_begin_running.wait_until_entered().await;

        // 5-6. The timeout will fire while the closure is gated. Await the
        //    coordinator to get the timeout outcome.
        let outcome = call.await.unwrap();
        assert!(outcome.timed_out, "must time out while closure is gated");

        // 7. Assert total_timeouts == 1.
        assert_eq!(
            metrics.total_timeouts.load(Ordering::Relaxed),
            1,
            "total_timeouts must be exactly 1"
        );

        // 8. Assert active_blocking_handlers == 0 and timed_out_handlers == 0
        //    while the closure remains gated.
        assert_eq!(
            metrics.active_blocking_handlers.load(Ordering::Relaxed),
            0,
            "active_blocking_handlers must be 0 while closure is gated at before_begin_running"
        );
        assert_eq!(
            metrics.timed_out_handlers.load(Ordering::Relaxed),
            0,
            "timed_out_handlers must be 0 — queued timeout does not increment it"
        );

        // 9. Release before_begin_running so the closure can proceed.
        before_begin_running.release();

        // 10. Prove the handler body was never invoked.
        assert!(
            !HANDLER_RAN.load(Ordering::SeqCst),
            "handler must not run after queued timeout"
        );

        // 11. The blocking closure will call begin_running, see
        // CancelledBeforeStart, and return. Wait for actual closure exit with
        // a watchdog that cannot establish the expected ordering.
        assert!(
            tokio::time::timeout(Duration::from_secs(5), closure_exited.wait())
                .await
                .is_ok(),
            "blocking closure must signal exit after queued timeout"
        );

        // 12. Assert the handler was still never invoked and all gauges remain
        // zero except cumulative timeout count.
        assert!(!HANDLER_RAN.load(Ordering::SeqCst));
        let snap = snapshot_from_metrics(&metrics);
        assert_eq!(snap.active_blocking_handlers, 0);
        assert_eq!(snap.timed_out_handlers, 0);
        assert_eq!(snap.total_timeouts, 1);
        assert_snapshot_invariant(&snap);
    }

    // ── Test B: completion wins the timeout-record race ─────────────────
    //
    // 1. Start a handler that returns quickly (fast handler).
    // 2. Allow it to establish Running (before_finish is reached after return).
    // 3. Allow the deadline to expire (timeout fires).
    // 4. Pause the timeout branch at before_timeout_record.
    // 5. Release before_finish so the lifecycle transitions to Finished.
    // 6. Wait for the finished hook.
    // 7. Release before_timeout_record.
    // 8. record_timeout must observe Finished and must not increment
    //    timed_out_handlers.
    // 9. The caller still receives a timeout because its deadline expired.
    // 10. Final gauges must be zero.

    #[tokio::test]
    async fn completion_wins_timeout_record_race() {
        let metrics = new_test_metrics();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let budget = ToolBudget::CHEAP.with_max_elapsed_ms(100);

        let before_finish = blocking_gate();
        let finished = blocking_gate();
        let before_timeout_record = async_gate();
        let timeout_recorded = async_gate();
        let hooks = ExecutionHooks {
            before_finish: Some(before_finish.clone()),
            finished: Some(finished.clone()),
            before_timeout_record: Some(before_timeout_record.clone()),
            timeout_recorded: Some(timeout_recorded.clone()),
            ..ExecutionHooks::none()
        };

        let call = tokio::spawn(execute_tool_bounded_with_hooks(
            test_handler_fast as registry::ToolHandler,
            Value::Object(serde_json::Map::new()),
            "test_tool".to_string(),
            budget,
            cancel_flag,
            semaphore,
            hooks,
            metrics.clone(),
        ));

        // 2. Wait for before_finish (handler has returned, Running established).
        before_finish.wait_until_entered().await;

        // 3. Wait for the timeout to fire (before_timeout_record reached).
        before_timeout_record.wait_until_entered().await;

        // 5. Release before_finish so lifecycle transitions to Finished.
        before_finish.release();

        // 6. Wait for finished hook.
        finished.wait_until_entered().await;
        finished.release();

        // 7. Release before_timeout_record so record_timeout can run.
        before_timeout_record.release();

        // 8. Wait for timeout_recorded (record_timeout has completed).
        timeout_recorded.wait_until_entered().await;
        timeout_recorded.release();

        let outcome = call.await.unwrap();

        // 9. Caller still receives a timeout.
        assert!(outcome.timed_out, "caller must receive timeout");

        // 8. timed_out_handlers must NOT have been incremented.
        assert_eq!(
            metrics.timed_out_handlers.load(Ordering::Relaxed),
            0,
            "completion wins: timed_out_handlers must not be incremented"
        );

        // 10. Final gauges must be zero.
        let snap = snapshot_from_metrics(&metrics);
        assert_eq!(snap.active_blocking_handlers, 0);
        assert_eq!(snap.timed_out_handlers, 0);
        assert_eq!(snap.total_timeouts, 1);
        assert_snapshot_invariant(&snap);
    }

    // ── Test C: timeout wins the completion race ────────────────────────
    //
    // 1. Start a handler that returns quickly (fast handler).
    // 2. Allow it to establish Running (before_finish is reached after return).
    // 3. Allow the deadline to expire (timeout fires).
    // 4. Pause the timeout branch at before_timeout_record.
    // 5. Release before_timeout_record so record_timeout sees Running and
    //    increments timed_out_handlers.
    // 6. Wait for timeout_recorded.
    // 7. Assert active_blocking_handlers == 1 and timed_out_handlers == 1.
    // 8. Release before_finish so lifecycle transitions to Finished and
    //    decrements both gauges.
    // 9. Wait for finished hook.
    // 10. Assert both gauges return exactly to zero.
    //
    // No sleep may be used to decide when timeout recording has completed.

    #[tokio::test]
    async fn timeout_wins_completion_race() {
        let metrics = new_test_metrics();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let budget = ToolBudget::CHEAP.with_max_elapsed_ms(100);

        let before_finish = blocking_gate();
        let finished = blocking_gate();
        let before_timeout_record = async_gate();
        let timeout_recorded = async_gate();
        let hooks = ExecutionHooks {
            before_finish: Some(before_finish.clone()),
            finished: Some(finished.clone()),
            before_timeout_record: Some(before_timeout_record.clone()),
            timeout_recorded: Some(timeout_recorded.clone()),
            ..ExecutionHooks::none()
        };

        let call = tokio::spawn(execute_tool_bounded_with_hooks(
            test_handler_fast as registry::ToolHandler,
            Value::Object(serde_json::Map::new()),
            "test_tool".to_string(),
            budget,
            cancel_flag,
            semaphore,
            hooks,
            metrics.clone(),
        ));

        // 2. Wait for before_finish (handler has returned, Running established).
        before_finish.wait_until_entered().await;

        // 3. Wait for the timeout to fire (before_timeout_record reached).
        before_timeout_record.wait_until_entered().await;

        // 5. Release before_timeout_record so record_timeout sees Running.
        before_timeout_record.release();

        // 6. Wait for timeout_recorded.
        timeout_recorded.wait_until_entered().await;
        timeout_recorded.release();

        // 7. Assert gauges: active == 1, timed_out == 1.
        assert_eq!(
            metrics.active_blocking_handlers.load(Ordering::Relaxed),
            1,
            "active_blocking_handlers must be 1 while handler is still running"
        );
        assert_eq!(
            metrics.timed_out_handlers.load(Ordering::Relaxed),
            1,
            "timed_out_handlers must be exactly 1"
        );

        // 8. Release before_finish so lifecycle transitions to Finished.
        before_finish.release();

        // 9. Wait for finished hook.
        finished.wait_until_entered().await;
        finished.release();

        // 10. Assert both gauges return exactly to zero.
        let snap = snapshot_from_metrics(&metrics);
        assert_eq!(snap.active_blocking_handlers, 0);
        assert_eq!(snap.timed_out_handlers, 0);
        assert_snapshot_invariant(&snap);

        let outcome = call.await.unwrap();
        assert!(outcome.timed_out, "caller must receive timeout");
    }

    // ── Test D: panic after recorded timeout ────────────────────────────
    //
    // The handler-entry gate holds the handler immediately before invocation.
    // The timeout is recorded while that gate is held, then the gate is
    // released and the handler panics immediately. No handler sleep is used
    // to establish the running-timeout ordering.

    #[tokio::test]
    async fn panic_after_timeout() {
        let metrics = new_test_metrics();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let budget = ToolBudget::CHEAP.with_max_elapsed_ms(100);

        let before_handler = blocking_gate();
        let before_finish = blocking_gate();
        let finished = blocking_gate();
        let before_timeout_record = async_gate();
        let timeout_recorded = async_gate();
        let closure_exited = Arc::new(ClosureExitSignal::new());
        let hooks = ExecutionHooks {
            before_handler: Some(before_handler.clone()),
            before_finish: Some(before_finish.clone()),
            finished: Some(finished.clone()),
            before_timeout_record: Some(before_timeout_record.clone()),
            timeout_recorded: Some(timeout_recorded.clone()),
            closure_exited: Some(closure_exited.clone()),
            ..ExecutionHooks::none()
        };

        let call = tokio::spawn(execute_tool_bounded_with_hooks(
            |_args| panic!("intentional test panic"),
            Value::Object(serde_json::Map::new()),
            "test_tool".to_string(),
            budget,
            cancel_flag,
            semaphore,
            hooks,
            metrics.clone(),
        ));

        // 1. Running is established before the handler-entry gate.
        before_handler.wait_until_entered().await;

        // 2. Wait for timeout to fire while the handler is held at entry.
        before_timeout_record.wait_until_entered().await;

        // 3. Assert both running gauges equal one.
        assert_eq!(
            metrics.active_blocking_handlers.load(Ordering::Relaxed),
            1,
            "active_blocking_handlers must be 1 while handler is still running"
        );
        assert_eq!(
            metrics.timed_out_handlers.load(Ordering::Relaxed),
            0,
            "timed_out_handlers must be 0 before record_timeout is released"
        );

        // Release before_timeout_record so record_timeout sees Running.
        before_timeout_record.release();
        timeout_recorded.wait_until_entered().await;
        timeout_recorded.release();

        // Now timed_out_handlers should be 1.
        assert_eq!(
            metrics.timed_out_handlers.load(Ordering::Relaxed),
            1,
            "timed_out_handlers must be 1 after timeout recording"
        );

        // 4. Release the handler-entry gate. The handler panics immediately;
        // catch_unwind converts it and the closure reaches before_finish.
        before_handler.release();
        before_finish.release();

        // 5. Wait for lifecycle completion and actual closure exit.
        finished.wait_until_entered().await;
        finished.release();
        assert!(
            tokio::time::timeout(Duration::from_secs(5), closure_exited.wait())
                .await
                .is_ok(),
            "panicking blocking closure must signal exit"
        );

        // 6. Assert both gauges return to zero.
        let snap = snapshot_from_metrics(&metrics);
        assert_eq!(snap.active_blocking_handlers, 0);
        assert_eq!(snap.timed_out_handlers, 0);
        assert_snapshot_invariant(&snap);

        // 7. The caller already timed out; panic cleanup is still complete.
        let outcome = call.await.unwrap();
        assert!(outcome.timed_out, "caller must receive timeout");
    }

    // ── Test E: cooperative cancellation visibility ─────────────────────
    //
    // Use a handler that waits until the timeout flag becomes true and then
    // exits. Do not implement this with a fixed 200 ms sleep.
    //
    // Assert:
    // - timeout sets the exact flag passed to the handler;
    // - handler observes it;
    // - lifecycle gauges return to zero;
    // - no replacement thread is created.

    #[tokio::test]
    async fn cooperative_cancellation_visibility() {
        static COOPERATIVE_CANCEL_OBSERVED: AtomicBool = AtomicBool::new(false);

        // Non-capturing closure: can be coerced to fn pointer.
        // Spins on the thread-local cancel flag installed by the coordinator.
        //
        // The hard 30 s bail-out is load-bearing: `spawn_blocking` closures
        // cannot be aborted, so if this handler spun forever, runtime shutdown
        // after ANY assertion failure in this test would block on the join and
        // wedge the whole test process (that is precisely how CI hung for
        // >60 minutes). Bounding the spin guarantees the blocking thread — and
        // therefore the process — always terminates.
        fn cancel_polling_handler(_args: &Value) -> ToolResponse {
            let give_up_at = std::time::Instant::now() + Duration::from_secs(30);
            while let Some(flag) = crate::mcp::budget::current_cancel_flag() {
                if flag.load(Ordering::Acquire) {
                    COOPERATIVE_CANCEL_OBSERVED.store(true, Ordering::SeqCst);
                    return ToolResponse::success(serde_json::json!("cancelled"), None);
                }
                if std::time::Instant::now() >= give_up_at {
                    return ToolResponse::success(serde_json::json!("handler-bailout"), None);
                }
                std::hint::spin_loop();
            }
            ToolResponse::success(serde_json::json!("ok"), None)
        }

        let metrics = new_test_metrics();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let budget = ToolBudget::CHEAP.with_max_elapsed_ms(50);

        let before_finish = blocking_gate();
        let finished = blocking_gate();
        let hooks = ExecutionHooks {
            before_finish: Some(before_finish.clone()),
            finished: Some(finished.clone()),
            ..ExecutionHooks::none()
        };

        COOPERATIVE_CANCEL_OBSERVED.store(false, Ordering::SeqCst);

        let call = tokio::spawn(execute_tool_bounded_with_hooks(
            cancel_polling_handler as registry::ToolHandler,
            Value::Object(serde_json::Map::new()),
            "test_tool".to_string(),
            budget,
            cancel_flag.clone(),
            semaphore,
            hooks,
            metrics.clone(),
        ));

        // Wait for the handler to observe the cancel flag. The wait runs on
        // a dedicated OS thread that polls the static and forwards the
        // observation through a `tokio::sync::Notify`; the test task parks on
        // `notified()` inside a 5 s watchdog so the runtime's timer wheel
        // stays free to fire the coordinator's 50 ms timeout. (An in-runtime
        // `yield_now` spin here is what allowed the historical hang: once the
        // watchdog started spinning, the late-armed coordinator timer could
        // starve behind it.)
        let observed = Arc::new(tokio::sync::Notify::new());
        let observed_for_poller = observed.clone();
        let cancel_poller = std::thread::Builder::new()
            .name("cooperative-cancel-poller".into())
            .spawn(move || {
                // Polled on a dedicated OS thread so the loop is independent
                // of whichever runtime scheduling decisions the test is
                // making. A bounded deadline guarantees the poller exits
                // even if the coordinator never sets the flag — otherwise
                // the panic from the watchdog's `assert!` would unwind the
                // test thread while this thread keeps the process alive.
                let deadline = std::time::Instant::now() + Duration::from_secs(5);
                while !COOPERATIVE_CANCEL_OBSERVED.load(Ordering::Acquire) {
                    if std::time::Instant::now() >= deadline {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
                observed_for_poller.notify_one();
            })
            .expect("failed to spawn cancel poller");

        let watchdog = tokio::time::timeout(Duration::from_secs(5), async {
            observed.notified().await;
        });
        let observed_in_time = watchdog.await.is_ok();
        cancel_poller
            .join()
            .expect("cancel poller thread must not panic");
        assert!(
            observed_in_time,
            "handler must observe the cancel flag within 5s"
        );

        // The timeout has fired and the cancel flag is set.
        assert!(
            cancel_flag.load(Ordering::Relaxed),
            "timeout must set the cancel flag"
        );
        assert!(
            COOPERATIVE_CANCEL_OBSERVED.load(Ordering::SeqCst),
            "handler must observe the cancel flag"
        );

        // Wait for before_finish (handler has returned).
        before_finish.wait_until_entered().await;

        // Release before_finish so lifecycle can complete.
        before_finish.release();

        // Wait for finished.
        finished.wait_until_entered().await;
        finished.release();

        // Assert gauges return to zero.
        let snap = snapshot_from_metrics(&metrics);
        assert_eq!(snap.active_blocking_handlers, 0);
        assert_eq!(snap.timed_out_handlers, 0);
        assert_snapshot_invariant(&snap);

        let outcome = call.await.unwrap();
        assert!(outcome.timed_out, "caller must receive timeout");
    }

    // ── Test F: 100 exact interleavings (50 completion + 50 timeout) ───
    //
    // Runs 100 iterations, alternating:
    // - 50 completion-wins sequences using the exact gates from Test B;
    // - 50 timeout-wins sequences using the exact gates from Test C.
    //
    // Requirements:
    // - no sleep to release or settle a handler;
    // - unique per-iteration gate state;
    // - no manually assigned slot; any static backing storage uses an
    //   exclusive RAII lease;
    // - exact expected outcome count: 50 timeout responses and 50
    //   selected completion outcomes according to the test design;
    // - gauges asserted at quiescence after every iteration;
    // - peak worker count never exceeds the configured semaphore size.
    //
    // A separate ignored stress test runs 500 iterations.

    #[tokio::test]
    async fn one_hundred_exact_interleavings() {
        let metrics = new_test_metrics();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let mut timeouts_observed = 0usize;
        let mut successes_observed = 0usize;

        for iter in 0..100 {
            let cancel_flag = Arc::new(AtomicBool::new(false));
            let is_timeout_case = iter % 2 == 0;

            let before_finish = blocking_gate();
            let finished = blocking_gate();
            let before_timeout_record = async_gate();
            let timeout_recorded = async_gate();
            let hooks = ExecutionHooks {
                before_finish: Some(before_finish.clone()),
                finished: Some(finished.clone()),
                before_timeout_record: Some(before_timeout_record.clone()),
                timeout_recorded: Some(timeout_recorded.clone()),
                ..ExecutionHooks::none()
            };

            let budget = ToolBudget::CHEAP.with_max_elapsed_ms(100);

            let call = tokio::spawn(execute_tool_bounded_with_hooks(
                test_handler_fast as registry::ToolHandler,
                Value::Object(serde_json::Map::new()),
                format!("iter_{}", iter),
                budget,
                cancel_flag,
                semaphore.clone(),
                hooks,
                metrics.clone(),
            ));

            // Wait for before_finish (handler has returned, Running established).
            before_finish.wait_until_entered().await;

            // Wait for the timeout to fire.
            before_timeout_record.wait_until_entered().await;

            if is_timeout_case {
                // Timeout-wins: release before_timeout_record first.
                before_timeout_record.release();
                timeout_recorded.wait_until_entered().await;
                timeout_recorded.release();
                assert_eq!(
                    metrics.timed_out_handlers.load(Ordering::Relaxed),
                    1,
                    "iter {}: timed_out_handlers must be 1",
                    iter
                );
                // Then release before_finish.
                before_finish.release();
                finished.wait_until_entered().await;
                finished.release();
                timeouts_observed += 1;
            } else {
                // Completion-wins: release before_finish first.
                before_finish.release();
                finished.wait_until_entered().await;
                finished.release();
                // Then release before_timeout_record.
                before_timeout_record.release();
                timeout_recorded.wait_until_entered().await;
                timeout_recorded.release();
                assert_eq!(
                    metrics.timed_out_handlers.load(Ordering::Relaxed),
                    0,
                    "iter {}: timed_out_handlers must be 0 (completion wins)",
                    iter
                );
                successes_observed += 1;
            }

            // Gauges asserted at quiescence after every iteration.
            let snap = snapshot_from_metrics(&metrics);
            assert_eq!(
                snap.active_blocking_handlers, 0,
                "iter {}: active must be 0",
                iter
            );
            assert_eq!(
                snap.timed_out_handlers, 0,
                "iter {}: timed_out must be 0",
                iter
            );
            assert_snapshot_invariant(&snap);

            // Ensure the coordinator task completed.
            let outcome = call.await.unwrap();
            assert!(
                outcome.timed_out,
                "iter {}: caller must receive timeout",
                iter
            );
        }

        assert_eq!(
            successes_observed, 50,
            "exactly 50 successful completions expected"
        );
        assert_eq!(timeouts_observed, 50, "exactly 50 timeouts expected");
        assert_snapshot_invariant(&snapshot_from_metrics(&metrics));
    }

    /// 500-iteration stress test (ignored in ordinary CI).
    ///
    /// Run with: `cargo test --locked --all-features --lib mcp::execution::deterministic_tests::five_hundred_exact_interleavings -- --ignored`
    #[tokio::test]
    #[ignore = "stress test: 500 iterations, run manually"]
    async fn five_hundred_exact_interleavings() {
        let metrics = new_test_metrics();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let mut timeouts_observed = 0usize;
        let mut successes_observed = 0usize;

        for iter in 0..500 {
            let cancel_flag = Arc::new(AtomicBool::new(false));
            let is_timeout_case = iter % 2 == 0;

            let before_finish = blocking_gate();
            let finished = blocking_gate();
            let before_timeout_record = async_gate();
            let timeout_recorded = async_gate();
            let hooks = ExecutionHooks {
                before_finish: Some(before_finish.clone()),
                finished: Some(finished.clone()),
                before_timeout_record: Some(before_timeout_record.clone()),
                timeout_recorded: Some(timeout_recorded.clone()),
                ..ExecutionHooks::none()
            };

            let budget = ToolBudget::CHEAP.with_max_elapsed_ms(100);

            let call = tokio::spawn(execute_tool_bounded_with_hooks(
                test_handler_fast as registry::ToolHandler,
                Value::Object(serde_json::Map::new()),
                format!("iter_{}", iter),
                budget,
                cancel_flag,
                semaphore.clone(),
                hooks,
                metrics.clone(),
            ));

            before_finish.wait_until_entered().await;
            before_timeout_record.wait_until_entered().await;

            if is_timeout_case {
                before_timeout_record.release();
                timeout_recorded.wait_until_entered().await;
                timeout_recorded.release();
                before_finish.release();
                finished.wait_until_entered().await;
                finished.release();
                timeouts_observed += 1;
            } else {
                before_finish.release();
                finished.wait_until_entered().await;
                finished.release();
                before_timeout_record.release();
                timeout_recorded.wait_until_entered().await;
                timeout_recorded.release();
                successes_observed += 1;
            }

            let snap = snapshot_from_metrics(&metrics);
            assert_eq!(
                snap.active_blocking_handlers, 0,
                "iter {}: active must be 0",
                iter
            );
            assert_eq!(
                snap.timed_out_handlers, 0,
                "iter {}: timed_out must be 0",
                iter
            );
            assert_snapshot_invariant(&snap);

            let outcome = call.await.unwrap();
            assert!(
                outcome.timed_out,
                "iter {}: caller must receive timeout",
                iter
            );
        }

        assert_eq!(
            successes_observed, 250,
            "exactly 250 successful completions expected"
        );
        assert_eq!(timeouts_observed, 250, "exactly 250 timeouts expected");
        assert_snapshot_invariant(&snapshot_from_metrics(&metrics));
    }

    // ── Test G: real N+1 worker bound ───────────────────────────────────
    //
    // 1. Configure a shared semaphore with N = 3.
    // 2. Start three handlers, each gated after Running.
    // 3. Wait until all three are running.
    // 4. Assert active and peak concurrency equal three.
    // 5. Start a fourth coordinator invocation with a generous timeout.
    // 6. Prove the fourth invocation has not reached running_established
    //    while all three permits remain occupied.
    // 7. Use a short bounded non-event observation only for this negative assertion.
    // 8. Release exactly one running handler.
    // 9. Wait until the fourth invocation reaches running_established.
    // 10. Assert active concurrency remains three, not four.
    // 11. Release remaining handlers.
    // 12. Join every coordinator task.
    // 13. Assert final gauges are zero and peak concurrency is exactly three.

    #[tokio::test]
    async fn worker_bound_n_plus_one() {
        let max_workers = 3usize;
        let metrics = new_test_metrics();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(max_workers));

        // Each handler owns an exclusive RAII slot lease for its lifetime.
        let mut handles = Vec::new();
        for i in 0..max_workers {
            let sem = semaphore.clone();
            let cancel_flag = Arc::new(AtomicBool::new(false));
            let budget = ToolBudget::CHEAP.with_max_elapsed_ms(30_000);
            let running_established = blocking_gate();
            let finished = blocking_gate();
            let hooks = ExecutionHooks {
                running_established: Some(running_established.clone()),
                finished: Some(finished.clone()),
                ..ExecutionHooks::none()
            };

            let slot = acquire_test_slot();
            slot.block();

            let handle = tokio::spawn(execute_tool_bounded_with_hooks(
                test_handler_blocking_slot as registry::ToolHandler,
                block_slot_args(&slot),
                format!("worker_{}", i),
                budget,
                cancel_flag,
                sem,
                hooks,
                metrics.clone(),
            ));
            handles.push((i, handle, running_established, finished, slot));
        }

        // 3. Wait until all three are running.
        for (_, _, re, _, _) in &handles {
            re.wait_until_entered().await;
            re.release();
        }

        // 4. Assert active and peak concurrency equal three.
        let snap = snapshot_from_metrics(&metrics);
        assert_eq!(
            snap.active_blocking_handlers, max_workers,
            "active_blocking_handlers must equal max_workers"
        );
        assert_eq!(
            snap.peak_blocking_concurrency, max_workers,
            "peak_blocking_concurrency must equal max_workers"
        );

        // 5. Start a fourth coordinator invocation with a generous timeout.
        let fourth_cancel = Arc::new(AtomicBool::new(false));
        let fourth_budget = ToolBudget::CHEAP.with_max_elapsed_ms(30_000);
        let fourth_running = blocking_gate();
        let fourth_finished = blocking_gate();
        let fourth_hooks = ExecutionHooks {
            running_established: Some(fourth_running.clone()),
            finished: Some(fourth_finished.clone()),
            ..ExecutionHooks::none()
        };

        let fourth_slot = acquire_test_slot();
        fourth_slot.block();
        let fourth_handle = tokio::spawn(execute_tool_bounded_with_hooks(
            test_handler_blocking_slot as registry::ToolHandler,
            block_slot_args(&fourth_slot),
            "worker_3".to_string(),
            fourth_budget,
            fourth_cancel,
            semaphore.clone(),
            fourth_hooks,
            metrics.clone(),
        ));

        // 6-7. Prove the fourth invocation has not reached running_established
        //    while all three permits remain occupied. Use a short bounded
        //    non-event observation only for this negative assertion.
        let negative = tokio::time::timeout(
            Duration::from_millis(50),
            fourth_running.wait_until_entered(),
        );
        assert!(
            negative.await.is_err(),
            "fourth invocation must not reach running_established while all permits are occupied"
        );

        // 8. Release exactly one running handler.
        handles[0].4.release();
        // Also release finished so the worker can fully exit and free its permit.
        handles[0].3.release();

        // 9. Wait until the fourth invocation reaches running_established.
        fourth_running.wait_until_entered().await;

        // 10. Assert active concurrency remains three, not four.
        let snap = snapshot_from_metrics(&metrics);
        assert_eq!(
            snap.active_blocking_handlers, max_workers,
            "active concurrency must remain {} after one release, not {}",
            max_workers, snap.active_blocking_handlers
        );

        // 11. Release remaining handlers.
        for (_, _, _, _, slot) in handles.iter().skip(1) {
            slot.release();
        }
        // Release remaining finished gates for workers 2 and 3.
        for (_, _, _, fin, _) in &handles {
            fin.release();
        }
        // Release the fourth handler.
        fourth_slot.release();
        fourth_running.release();
        fourth_finished.release();

        // 12. Join every coordinator task.
        for (_, handle, _, _, _) in handles {
            let outcome = handle.await;
            assert!(outcome.is_ok(), "handler must complete without panic");
        }
        let fourth_outcome = fourth_handle.await.unwrap();
        assert!(!fourth_outcome.timed_out, "fourth handler must succeed");

        // 13. Assert final gauges are zero and peak concurrency is exactly three.
        let snap = snapshot_from_metrics(&metrics);
        assert_eq!(snap.active_blocking_handlers, 0);
        assert_eq!(snap.timed_out_handlers, 0);
        assert_eq!(snap.peak_blocking_concurrency, max_workers);
        assert_snapshot_invariant(&snap);
    }

    // ── Deterministic: completion wins (simple) ─────────────────────────
    //
    // 1. Start a fast handler with a generous timeout.
    // 2. Wait for finished (handler finished).
    // 3. Assert no timeout, gauges at zero.

    #[tokio::test]
    async fn deterministic_completion_wins() {
        let metrics = new_test_metrics();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let budget = ToolBudget::CHEAP.with_max_elapsed_ms(5000);

        let finished = blocking_gate();
        let hooks = ExecutionHooks {
            finished: Some(finished.clone()),
            ..ExecutionHooks::none()
        };

        let call = tokio::spawn(execute_tool_bounded_with_hooks(
            test_handler_fast as registry::ToolHandler,
            Value::Object(serde_json::Map::new()),
            "test_tool".to_string(),
            budget,
            cancel_flag,
            semaphore,
            hooks,
            metrics.clone(),
        ));

        // Wait for lifecycle to complete via hook.
        finished.wait_until_entered().await;

        // Release the finished gate so the blocking closure can complete.
        finished.release();

        let outcome = call.await.unwrap();

        assert!(
            !outcome.timed_out,
            "fast handler must complete before timeout"
        );
        assert!(outcome.tool_response.is_ok());

        let snap = snapshot_from_metrics(&metrics);
        assert_eq!(snap.active_blocking_handlers, 0);
        assert_eq!(snap.timed_out_handlers, 0);
        assert_eq!(snap.peak_blocking_concurrency, 1);
        assert_snapshot_invariant(&snap);
    }

    // ── Repeated 100-iteration test ────────────────────────────────────
    //
    // Runs 100 iterations with varying handlers and timeouts. Blocking
    // invocations retain their slot lease until their closure has signalled
    // actual exit, so no settlement sleep is needed.

    #[tokio::test]
    async fn repeated_single_threaded_100_iterations() {
        let metrics = new_test_metrics();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));

        for iter in 0..100 {
            let cancel_flag = Arc::new(AtomicBool::new(false));

            if iter % 3 == 2 {
                let slot = acquire_test_slot();
                slot.block();
                let closure_exited = Arc::new(ClosureExitSignal::new());
                let outcome = execute_tool_bounded_with_hooks(
                    test_handler_blocking_slot as registry::ToolHandler,
                    block_slot_args(&slot),
                    format!("iter_{}", iter),
                    ToolBudget::CHEAP.with_max_elapsed_ms(5),
                    cancel_flag,
                    semaphore.clone(),
                    ExecutionHooks {
                        closure_exited: Some(closure_exited.clone()),
                        ..ExecutionHooks::none()
                    },
                    metrics.clone(),
                )
                .await;

                assert!(outcome.timed_out, "iter {}: blocking must time out", iter);
                slot.release();
                assert!(
                    tokio::time::timeout(Duration::from_secs(5), closure_exited.wait())
                        .await
                        .is_ok(),
                    "iter {}: blocking closure must signal exit",
                    iter
                );
            } else {
                let outcome = execute_tool_bounded_with_hooks(
                    test_handler_fast as registry::ToolHandler,
                    Value::Object(serde_json::Map::new()),
                    format!("iter_{}", iter),
                    if iter % 3 == 0 {
                        ToolBudget::CHEAP.with_max_elapsed_ms(5000)
                    } else {
                        ToolBudget::CHEAP.with_max_elapsed_ms(100)
                    },
                    cancel_flag,
                    semaphore.clone(),
                    ExecutionHooks::none(),
                    metrics.clone(),
                )
                .await;
                assert!(
                    !outcome.timed_out,
                    "iter {}: fast handler must finish",
                    iter
                );
            }

            assert_snapshot_invariant(&snapshot_from_metrics(&metrics));
        }

        // Final invariant check.
        let snap = snapshot_from_metrics(&metrics);
        assert_eq!(snap.active_blocking_handlers, 0);
        assert_eq!(snap.timed_out_handlers, 0);
        assert_snapshot_invariant(&snap);
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
