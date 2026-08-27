use crate::mcp::response::ToolResponse;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Default number of worker threads in the synchronous execution pool.
pub(crate) const DEFAULT_SYNC_WORKERS: usize = 8;

/// Default queue capacity for the synchronous execution pool.
pub(crate) const DEFAULT_SYNC_QUEUE: usize = 32;

struct SyncJob {
    handler: Box<dyn FnOnce() -> ToolResponse + Send + 'static>,
    reply: SyncSender<ToolResponse>,
    cancel_flag: Arc<AtomicBool>,
    deadline: Instant,
    /// Set by the submitter when its wait times out. The worker checks this
    /// after finishing the job to release the pool-health "stuck" gauge.
    abandoned: Arc<AtomicBool>,
}

/// Caller-side handle for an in-flight job.
struct PendingJob {
    reply_rx: Receiver<ToolResponse>,
    cancel_flag: Arc<AtomicBool>,
    abandoned: Arc<AtomicBool>,
    stuck: Arc<AtomicUsize>,
}

#[cfg(test)]
pub(crate) struct TestEnqueueSignal {
    entered: std::sync::Mutex<bool>,
    ready: std::sync::Condvar,
}

#[cfg(test)]
impl TestEnqueueSignal {
    pub(crate) fn new() -> Self {
        Self {
            entered: std::sync::Mutex::new(false),
            ready: std::sync::Condvar::new(),
        }
    }

    pub(crate) fn signal(&self) {
        *self.entered.lock().unwrap() = true;
        self.ready.notify_all();
    }

    pub(crate) fn wait_until_entered(&self) {
        let mut entered = self.entered.lock().unwrap();
        while !*entered {
            entered = self.ready.wait(entered).unwrap();
        }
    }
}

/// Bounded synchronous worker pool for in-process tool execution.
///
/// The pool provides concurrency limiting and elapsed-time enforcement for
/// budget-aware registry APIs (`call_json_with_budget`, `call_json_with_context`,
/// `call_json_with_execution_context`). It uses a fixed number of long-lived
/// worker threads with a bounded work queue.
///
/// This pool is **not** used by the MCP server, which uses Tokio's
/// `spawn_blocking` for tool execution.
pub(crate) struct SyncExecutionPool {
    sender: SyncSender<SyncJob>,
    worker_count: usize,
    /// Number of jobs whose submitter timed out but whose handler is still
    /// occupying (or about to occupy) a worker. A sustained non-zero value
    /// means the pool's effective capacity is reduced by handlers that
    /// ignore cooperative cancellation.
    stuck: Arc<AtomicUsize>,
}

impl SyncExecutionPool {
    /// Create a pool with the default worker and queue limits.
    pub fn new() -> Self {
        Self::with_limits(DEFAULT_SYNC_WORKERS, DEFAULT_SYNC_QUEUE)
    }

    /// Create a pool with explicit worker and queue limits.
    ///
    /// `worker_count` controls the number of long-lived worker threads.
    /// `queue_capacity` controls how many jobs can be queued before
    /// submission is rejected with `SyncPoolError::QueueFull`.
    pub fn with_limits(worker_count: usize, queue_capacity: usize) -> Self {
        let (sender, receiver) = sync_channel(queue_capacity);
        // NOTE: workers share the receiver behind a mutex because
        // std::sync::mpsc::Receiver is not Sync; only one worker can block in
        // recv() at a time. Replacing this with a lock-free multi-consumer
        // channel (e.g. crossbeam-channel) would remove that serialization.
        let receiver = Arc::new(std::sync::Mutex::new(receiver));
        let stuck = Arc::new(AtomicUsize::new(0));

        for _ in 0..worker_count {
            let rx = receiver.clone();
            let stuck = stuck.clone();
            if let Err(e) = std::thread::Builder::new()
                .name("eggsact-sync-worker".to_string())
                .spawn(move || worker_loop(rx, stuck))
            {
                eprintln!("eggsact: failed to spawn sync worker: {e}");
            }
        }

        Self {
            sender,
            worker_count,
            stuck,
        }
    }

    /// Submit a job to the pool and wait for the result.
    ///
    /// The `handler` closure runs on a worker thread. The `timeout` parameter
    /// controls how long the caller waits for the result before returning
    /// `SyncPoolError::Timeout`. Note that the handler may continue running
    /// on the worker thread even after the caller receives a timeout — the
    /// pool does not kill threads.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn submit(
        &self,
        handler: impl FnOnce() -> ToolResponse + Send + 'static,
        timeout: Duration,
    ) -> Result<ToolResponse, SyncPoolError> {
        self.submit_cancellable(handler, timeout, Arc::new(AtomicBool::new(false)))
    }

    /// Enqueue a job into the worker queue and return a handle for waiting.
    ///
    /// Shared by `submit_cancellable` and the test-only
    /// `submit_cancellable_with_enqueue_signal`.
    fn enqueue_job(
        &self,
        handler: impl FnOnce() -> ToolResponse + Send + 'static,
        cancel_flag: &Arc<AtomicBool>,
        deadline: Instant,
    ) -> Result<PendingJob, SyncPoolError> {
        let (reply_tx, reply_rx) = sync_channel(1);
        let abandoned = Arc::new(AtomicBool::new(false));
        let job = SyncJob {
            handler: Box::new(handler),
            reply: reply_tx,
            cancel_flag: cancel_flag.clone(),
            deadline,
            abandoned: abandoned.clone(),
        };

        self.sender.try_send(job).map_err(|e| match e {
            std::sync::mpsc::TrySendError::Full(_) => SyncPoolError::QueueFull {
                worker_count: self.worker_count,
            },
            std::sync::mpsc::TrySendError::Disconnected(_) => SyncPoolError::Shutdown,
        })?;

        Ok(PendingJob {
            reply_rx,
            cancel_flag: cancel_flag.clone(),
            abandoned,
            stuck: self.stuck.clone(),
        })
    }

    /// Submit a job to the pool with an explicit cancellation flag.
    ///
    /// The flag is set to `true` on timeout so that the handler (if still
    /// running or queued) can observe the cancellation and exit early.
    pub fn submit_cancellable(
        &self,
        handler: impl FnOnce() -> ToolResponse + Send + 'static,
        timeout: Duration,
        cancel_flag: Arc<AtomicBool>,
    ) -> Result<ToolResponse, SyncPoolError> {
        let deadline = Instant::now() + timeout;
        let pending = self.enqueue_job(handler, &cancel_flag, deadline)?;
        wait_for_reply(&pending, timeout)
    }

    /// Test-only submission helper that signals after the job is queued.
    #[cfg(test)]
    fn submit_cancellable_with_enqueue_signal(
        &self,
        handler: impl FnOnce() -> ToolResponse + Send + 'static,
        timeout: Duration,
        cancel_flag: Arc<AtomicBool>,
        signal: Arc<TestEnqueueSignal>,
    ) -> Result<ToolResponse, SyncPoolError> {
        let deadline = Instant::now() + timeout;
        let pending = self.enqueue_job(handler, &cancel_flag, deadline)?;
        signal.signal();
        wait_for_reply(&pending, timeout)
    }

    /// Return the number of worker threads in this pool.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn worker_count(&self) -> usize {
        self.worker_count
    }

    /// Number of jobs whose submitter timed out but whose handler has not
    /// yet been reaped by a worker. While this gauge is non-zero, the pool's
    /// effective worker capacity is reduced; handlers stuck indefinitely
    /// keep it elevated permanently (threads are never killed).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn stuck_workers(&self) -> usize {
        self.stuck.load(Ordering::SeqCst)
    }
}

/// Release the pool-health "stuck" gauge for a job whose submitter already
/// timed out. Called exactly once per abandoned job, when the worker finishes
/// (or skips) it — so the 1:1 increment/decrement pairing never underflows.
fn reap_if_abandoned(job_abandoned: &AtomicBool, stuck: &AtomicUsize) {
    if job_abandoned.load(Ordering::SeqCst) {
        stuck.fetch_sub(1, Ordering::SeqCst);
    }
}

fn worker_loop(receiver: Arc<std::sync::Mutex<Receiver<SyncJob>>>, stuck: Arc<AtomicUsize>) {
    loop {
        let job = {
            let rx = receiver.lock().unwrap();
            match rx.recv() {
                Ok(job) => job,
                Err(_) => break,
            }
        };

        // Preflight: if the deadline has already expired, skip invocation.
        // This check runs after dequeue and before the handler is invoked.
        if Instant::now() >= job.deadline {
            job.cancel_flag.store(true, Ordering::SeqCst);
            let _ = job.reply.send(ToolResponse::error_with_code(
                "timeout",
                crate::mcp::machine_codes::TIMEOUT,
                "Tool handler deadline expired before execution",
                None,
                None,
            ));
            reap_if_abandoned(&job.abandoned, &stuck);
            continue;
        }

        // Preflight: if the cancellation flag was already set before the
        // worker dequeued this job, skip invocation. The job was cancelled
        // while still queued.
        if job.cancel_flag.load(Ordering::Acquire) {
            let _ = job.reply.send(ToolResponse::error_with_code(
                "cancelled",
                crate::mcp::machine_codes::CANCELLED,
                "Tool handler was cancelled while queued",
                None,
                None,
            ));
            reap_if_abandoned(&job.abandoned, &stuck);
            continue;
        }

        // Use catch_unwind so a panicking job does not kill the worker thread.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (job.handler)()));
        let response = match result {
            Ok(resp) => resp,
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
        };
        let _ = job.reply.send(response);
        reap_if_abandoned(&job.abandoned, &stuck);
    }
}

#[derive(Debug)]
pub(crate) enum SyncPoolError {
    /// The caller's timeout expired before the worker completed.
    Timeout,
    /// All workers are busy and the queue is full.
    QueueFull { worker_count: usize },
    /// The pool's channel has been disconnected (pool shut down).
    Shutdown,
}

impl SyncPoolError {
    /// Convert this pool error into a `ToolResponse` for the given tool.
    #[allow(clippy::wrong_self_convention)]
    pub fn to_tool_response(self, tool_name: &str) -> ToolResponse {
        match self {
            SyncPoolError::Timeout => ToolResponse::error_with_code(
                "timeout",
                crate::mcp::machine_codes::TIMEOUT,
                &format!("Tool '{}' timed out in sync execution pool", tool_name),
                Some(vec!["Try a simpler input".to_string()]),
                Some(tool_name),
            ),
            SyncPoolError::QueueFull { worker_count } => ToolResponse::error_with_code(
                "resource_exhausted",
                crate::mcp::machine_codes::RESOURCE_EXHAUSTED,
                &format!(
                    "Sync execution pool exhausted: all {} workers busy and queue is full",
                    worker_count
                ),
                Some(vec!["Retry after a moment".to_string()]),
                Some(tool_name),
            ),
            SyncPoolError::Shutdown => ToolResponse::error_with_code(
                "internal_error",
                crate::mcp::machine_codes::INTERNAL_ERROR,
                "Sync execution pool is shutting down",
                None,
                Some(tool_name),
            ),
        }
    }
}

/// Wait for a worker reply and classify the outcome.
///
/// On timeout, marks the job as abandoned (raising the pool-health "stuck"
/// gauge until a worker reaps it) and sets the cancellation flag before
/// returning `SyncPoolError::Timeout` so the handler (if still running or
/// queued) can observe the cancellation and exit early. On disconnected
/// sender, returns `SyncPoolError::Shutdown` without setting the flag (the
/// pool channel has shut down, not this invocation).
fn wait_for_reply(pending: &PendingJob, timeout: Duration) -> Result<ToolResponse, SyncPoolError> {
    match pending.reply_rx.recv_timeout(timeout) {
        Ok(response) => Ok(response),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            pending.abandoned.store(true, Ordering::SeqCst);
            pending.stuck.fetch_add(1, Ordering::SeqCst);
            pending.cancel_flag.store(true, Ordering::SeqCst);
            Err(SyncPoolError::Timeout)
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err(SyncPoolError::Shutdown),
    }
}

/// Process-wide synchronous execution pool instance.
static SYNC_POOL: std::sync::LazyLock<SyncExecutionPool> =
    std::sync::LazyLock::new(SyncExecutionPool::new);

/// Access the process-wide synchronous execution pool.
pub(crate) fn sync_pool() -> &'static SyncExecutionPool {
    &SYNC_POOL
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    struct BlockingJobGate {
        state: std::sync::Mutex<(bool, bool)>,
        ready: std::sync::Condvar,
    }

    impl BlockingJobGate {
        fn new() -> Self {
            Self {
                state: std::sync::Mutex::new((false, false)),
                ready: std::sync::Condvar::new(),
            }
        }

        fn arrive_and_wait(&self) {
            let mut state = self.state.lock().unwrap();
            state.0 = true;
            self.ready.notify_all();
            while !state.1 {
                state = self.ready.wait(state).unwrap();
            }
        }

        fn wait_until_started(&self) {
            let mut state = self.state.lock().unwrap();
            while !state.0 {
                state = self.ready.wait(state).unwrap();
            }
        }

        fn release(&self) {
            self.state.lock().unwrap().1 = true;
            self.ready.notify_all();
        }
    }

    #[test]
    fn two_jobs_run_concurrently() {
        let pool = Arc::new(SyncExecutionPool::with_limits(2, 2));
        let gate1 = Arc::new(BlockingJobGate::new());
        let gate2 = Arc::new(BlockingJobGate::new());
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));

        let g1 = gate1.clone();
        let a1 = active.clone();
        let pk1 = peak.clone();
        let p1 = pool.clone();
        let h1 = std::thread::spawn(move || {
            p1.submit(
                move || {
                    let now = a1.fetch_add(1, Ordering::SeqCst) + 1;
                    pk1.fetch_max(now, Ordering::SeqCst);
                    g1.arrive_and_wait();
                    a1.fetch_sub(1, Ordering::SeqCst);
                    ToolResponse::success(serde_json::json!({"id": 1}), Some("test"))
                },
                Duration::from_secs(5),
            )
        });

        let g2 = gate2.clone();
        let a2 = active.clone();
        let pk2 = peak.clone();
        let p2 = pool.clone();
        let h2 = std::thread::spawn(move || {
            p2.submit(
                move || {
                    let now = a2.fetch_add(1, Ordering::SeqCst) + 1;
                    pk2.fetch_max(now, Ordering::SeqCst);
                    g2.arrive_and_wait();
                    a2.fetch_sub(1, Ordering::SeqCst);
                    ToolResponse::success(serde_json::json!({"id": 2}), Some("test"))
                },
                Duration::from_secs(5),
            )
        });

        // Wait until both handlers are simultaneously inside their bodies.
        gate1.wait_until_started();
        gate2.wait_until_started();

        // Both handlers are now blocked at the gate, proving overlap.
        assert_eq!(
            peak.load(Ordering::SeqCst),
            2,
            "both handlers must be running simultaneously"
        );

        // Release both gates.
        gate1.release();
        gate2.release();

        let r1 = h1.join().expect("h1 panic");
        let r2 = h2.join().expect("h2 panic");
        assert!(r1.expect("job1").ok);
        assert!(r2.expect("job2").ok);

        // Pool remains usable.
        let r = pool.submit(
            move || ToolResponse::success(serde_json::json!("sentinel"), Some("test")),
            Duration::from_secs(5),
        );
        assert!(r.unwrap().ok, "pool must remain usable");
    }

    #[test]
    fn timeout_returns_within_bound() {
        let pool = SyncExecutionPool::with_limits(1, 1);
        let start = std::time::Instant::now();
        let result = pool.submit(
            move || {
                std::thread::sleep(Duration::from_secs(5));
                ToolResponse::success(serde_json::json!({}), Some("test"))
            },
            Duration::from_millis(50),
        );
        let elapsed = start.elapsed();
        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), SyncPoolError::Timeout),
            "expected Timeout error"
        );
        assert!(
            elapsed < Duration::from_secs(2),
            "timeout should return within configured bound, took {:?}",
            elapsed
        );
    }

    #[test]
    fn stuck_worker_gauge_tracks_abandoned_handlers() {
        let pool = Arc::new(SyncExecutionPool::with_limits(1, 1));
        let gate = Arc::new(BlockingJobGate::new());

        // Handler blocks on the gate and ignores cooperative cancellation,
        // so the worker stays occupied after the caller times out.
        let p1 = pool.clone();
        let gate_for_job = gate.clone();
        let h1 = std::thread::spawn(move || {
            p1.submit(
                move || {
                    gate_for_job.arrive_and_wait();
                    ToolResponse::success(serde_json::json!({}), Some("test"))
                },
                Duration::from_millis(50),
            )
        });
        gate.wait_until_started();

        // Wait past the 50ms timeout so the submitter has marked the job
        // abandoned, then confirm the gauge reflects the lost capacity.
        std::thread::sleep(Duration::from_millis(250));
        assert_eq!(
            pool.stuck_workers(),
            1,
            "handler running past its deadline must be counted as stuck"
        );

        // Releasing the handler lets the worker reap it and restore the
        // gauge to zero.
        gate.release();
        let result = h1.join().expect("submitter panic");
        assert!(matches!(result, Err(SyncPoolError::Timeout)));
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while pool.stuck_workers() != 0 {
            assert!(
                std::time::Instant::now() < deadline,
                "stuck gauge must drop back to zero after the handler finishes"
            );
            std::thread::sleep(Duration::from_millis(10));
        }

        // Pool remains usable.
        let r = pool.submit(
            move || ToolResponse::success(serde_json::json!("sentinel"), Some("test")),
            Duration::from_secs(5),
        );
        assert!(r.unwrap().ok, "pool must remain usable");
    }

    #[test]
    fn queue_saturation_returns_queue_full() {
        // Pool with 1 worker, queue capacity 1 → can handle 2 concurrent jobs max.
        let pool = Arc::new(SyncExecutionPool::with_limits(1, 1));
        let blocker = Arc::new(BlockingJobGate::new());
        let enqueued = Arc::new(TestEnqueueSignal::new());

        // Job 1: blocks the worker via a deterministic gate.
        let p1 = pool.clone();
        let blocker_for_job = blocker.clone();
        let h1 = std::thread::spawn(move || {
            p1.submit(
                move || {
                    blocker_for_job.arrive_and_wait();
                    ToolResponse::success(serde_json::json!({}), Some("test"))
                },
                Duration::from_secs(5),
            )
        });
        blocker.wait_until_started();

        // Job 2: goes into the queue buffer. Use enqueue signal to confirm
        // insertion instead of sleeping.
        let p2 = pool.clone();
        let enqueue_signal = enqueued.clone();
        let h2 = std::thread::spawn(move || {
            p2.submit_cancellable_with_enqueue_signal(
                move || ToolResponse::success(serde_json::json!({}), Some("test")),
                Duration::from_secs(5),
                Arc::new(AtomicBool::new(false)),
                enqueue_signal,
            )
        });
        enqueued.wait_until_entered();

        // Job 3: worker busy + queue full → QueueFull.
        let r3 = pool.submit(
            move || ToolResponse::success(serde_json::json!({}), Some("test")),
            Duration::from_millis(200),
        );
        assert!(
            matches!(r3, Err(SyncPoolError::QueueFull { worker_count: 1 })),
            "expected QueueFull, got {:?}",
            r3
        );

        // Release and drain.
        blocker.release();
        let r1 = h1.join().expect("h1 panic");
        let r2 = h2.join().expect("h2 panic");
        assert!(r1.expect("job1").ok);
        assert!(r2.expect("job2").ok);

        // Pool remains usable.
        let r = pool.submit(
            move || ToolResponse::success(serde_json::json!("sentinel"), Some("test")),
            Duration::from_secs(5),
        );
        assert!(r.unwrap().ok, "pool must remain usable after saturation");
    }

    #[test]
    fn worker_recovers_after_job_completion() {
        let pool = SyncExecutionPool::with_limits(1, 1);
        let r1 = pool.submit(
            move || ToolResponse::success(serde_json::json!({"step": 1}), Some("test")),
            Duration::from_secs(5),
        );
        assert!(r1.unwrap().ok);
        // Worker should be free now.
        let r2 = pool.submit(
            move || ToolResponse::success(serde_json::json!({"step": 2}), Some("test")),
            Duration::from_secs(5),
        );
        assert!(r2.unwrap().ok);
    }

    #[test]
    fn cancellation_flag_visible_inside_handler() {
        let pool = SyncExecutionPool::with_limits(1, 4);
        let flag = Arc::new(AtomicBool::new(true));
        let flag_clone = flag.clone();
        let result = pool.submit(
            move || {
                crate::mcp::budget::with_cancel_flag(Some(flag_clone), || {
                    let f = crate::mcp::budget::current_cancel_flag();
                    let is_set = f.is_some_and(|f| f.load(Ordering::Relaxed));
                    ToolResponse::success(serde_json::json!({"cancelled": is_set}), Some("test"))
                })
            },
            Duration::from_secs(5),
        );
        let resp = result.unwrap();
        assert!(resp.ok);
        let cancelled = resp.result.unwrap()["cancelled"].as_bool().unwrap();
        assert!(
            cancelled,
            "cancellation flag should be visible inside handler"
        );
    }

    #[test]
    fn worker_count_reflects_construction() {
        let pool = SyncExecutionPool::with_limits(4, 8);
        assert_eq!(pool.worker_count(), 4);
    }

    #[test]
    fn to_tool_response_timeout() {
        let resp = SyncPoolError::Timeout.to_tool_response("my_tool");
        assert!(!resp.ok);
        assert_eq!(
            resp.machine_code.as_deref(),
            Some(crate::mcp::machine_codes::TIMEOUT)
        );
    }

    #[test]
    fn to_tool_response_queue_full() {
        let resp = SyncPoolError::QueueFull { worker_count: 8 }.to_tool_response("my_tool");
        assert!(!resp.ok);
        assert_eq!(
            resp.machine_code.as_deref(),
            Some(crate::mcp::machine_codes::RESOURCE_EXHAUSTED)
        );
    }

    #[test]
    fn to_tool_response_shutdown() {
        let resp = SyncPoolError::Shutdown.to_tool_response("my_tool");
        assert!(!resp.ok);
        assert_eq!(
            resp.machine_code.as_deref(),
            Some(crate::mcp::machine_codes::INTERNAL_ERROR)
        );
    }

    // ── WS4 additional tests ─────────────────────────────────────────────

    // Panic in one job does not kill the worker permanently.
    #[test]
    fn panic_in_job_does_not_kill_worker() {
        let pool = SyncExecutionPool::with_limits(1, 4);

        // Job 1: panics. catch_unwind converts it to an error ToolResponse.
        let r1 = pool.submit(
            move || {
                panic!("intentional worker panic");
            },
            Duration::from_secs(5),
        );
        let resp1 = r1.expect("channel should not disconnect (catch_unwind handles panic)");
        assert!(!resp1.ok, "panicking job should return error response");

        // Job 2: should succeed — worker survived the panic.
        let r2 = pool.submit(
            move || ToolResponse::success(serde_json::json!("recovered"), Some("test")),
            Duration::from_secs(5),
        );
        let resp2 = r2.unwrap();
        assert!(resp2.ok, "worker must survive a panic in a previous job");
    }

    // Eval context thread-local is restored before the next job.
    #[test]
    fn eval_context_not_leaked_between_jobs() {
        let pool = SyncExecutionPool::with_limits(1, 4);

        // Job 1: set a cancel flag in thread-local, then complete.
        let flag1 = Arc::new(AtomicBool::new(true));
        let f1 = flag1.clone();
        let r1 = pool.submit(
            move || {
                crate::mcp::budget::with_cancel_flag(Some(f1), || {
                    // Verify the flag is set inside this job.
                    let f = crate::mcp::budget::current_cancel_flag();
                    let is_set = f.is_some_and(|f| f.load(Ordering::Relaxed));
                    ToolResponse::success(serde_json::json!({"set_in_job1": is_set}), Some("test"))
                })
            },
            Duration::from_secs(5),
        );
        let resp = r1.unwrap();
        assert!(resp.ok);
        assert!(resp.result.unwrap()["set_in_job1"].as_bool().unwrap());

        // Job 2: verify the cancel flag from job1 is NOT visible.
        let r2 = pool.submit(
            move || {
                let f = crate::mcp::budget::current_cancel_flag();
                let is_set = f.is_some_and(|f| f.load(Ordering::Relaxed));
                ToolResponse::success(serde_json::json!({"leaked": is_set}), Some("test"))
            },
            Duration::from_secs(5),
        );
        let resp = r2.unwrap();
        assert!(resp.ok);
        assert!(
            !resp.result.unwrap()["leaked"].as_bool().unwrap(),
            "cancel flag from previous job must not leak to next job"
        );
    }

    // Repeated timeouts do not increase worker count beyond the fixed pool size.
    // The pool is constructed with a fixed number of workers; verify the count
    // is stable and the pool accepts new work after timeouts.
    #[test]
    fn repeated_timeouts_pool_stays_usable() {
        let pool = SyncExecutionPool::with_limits(2, 4);
        assert_eq!(pool.worker_count(), 2);

        // Submit 3 jobs that time out quickly (handler sleeps 50ms, timeout 10ms).
        // After timeout, the handler finishes within 50ms, freeing the worker.
        for _ in 0..3 {
            let _ = pool.submit(
                move || {
                    std::thread::sleep(Duration::from_millis(50));
                    ToolResponse::success(serde_json::json!({}), Some("test"))
                },
                Duration::from_millis(10),
            );
            // Small delay so the handler can finish and free the worker.
            std::thread::sleep(Duration::from_millis(20));
        }

        // Wait for all slow handlers to complete.
        std::thread::sleep(Duration::from_millis(100));

        // Pool should still be usable — submit a fast job.
        let r = pool.submit(
            move || ToolResponse::success(serde_json::json!("after_timeouts"), Some("test")),
            Duration::from_secs(5),
        );
        assert!(
            r.unwrap().ok,
            "pool must remain usable after repeated timeouts"
        );
        assert_eq!(pool.worker_count(), 2, "worker count must not change");
    }

    // ── WS3 cancellation/deadline tests ──────────────────────────────────

    #[test]
    fn timeout_sets_cancel_flag() {
        let pool = SyncExecutionPool::with_limits(1, 4);
        let flag = Arc::new(AtomicBool::new(false));
        let flag_clone = flag.clone();

        let result = pool.submit_cancellable(
            move || {
                std::thread::sleep(Duration::from_secs(5));
                ToolResponse::success(serde_json::json!({}), Some("test"))
            },
            Duration::from_millis(10),
            flag_clone,
        );

        // The cancel flag is set inside wait_for_reply before submit_cancellable
        // returns, so no settlement sleep is needed.
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SyncPoolError::Timeout));
        assert!(
            flag.load(Ordering::SeqCst),
            "cancel_flag must be true after timeout"
        );
    }

    #[test]
    fn running_cooperative_handler_exits_on_flag() {
        let pool = SyncExecutionPool::with_limits(1, 4);
        let flag = Arc::new(AtomicBool::new(false));
        let flag_clone = flag.clone();

        // Submit a handler that polls the flag every 5ms.
        let flag_for_handler = flag.clone();
        let result = pool.submit_cancellable(
            move || {
                for _ in 0..200 {
                    if flag_for_handler.load(Ordering::SeqCst) {
                        return ToolResponse::success(
                            serde_json::json!({"exited_early": true}),
                            Some("test"),
                        );
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                ToolResponse::success(serde_json::json!({"exited_early": false}), Some("test"))
            },
            Duration::from_millis(10),
            flag_clone,
        );

        // Wait for the handler to notice the flag and exit.
        std::thread::sleep(Duration::from_millis(200));

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SyncPoolError::Timeout));
        assert!(flag.load(Ordering::SeqCst));
    }

    #[test]
    fn queued_timed_out_job_is_skipped_before_sentinel() {
        let pool = Arc::new(SyncExecutionPool::with_limits(1, 4));
        let blocker = Arc::new(BlockingJobGate::new());
        let expired_ran = Arc::new(AtomicBool::new(false));
        let expired_cancel = Arc::new(AtomicBool::new(false));

        let p1 = pool.clone();
        let blocker_for_job = blocker.clone();
        let h1 = std::thread::spawn(move || {
            p1.submit(
                move || {
                    blocker_for_job.arrive_and_wait();
                    ToolResponse::success(serde_json::json!("blocker"), Some("test"))
                },
                Duration::from_secs(5),
            )
        });
        blocker.wait_until_started();

        let p2 = pool.clone();
        let ran = expired_ran.clone();
        let cancel = expired_cancel.clone();
        let h2 = std::thread::spawn(move || {
            p2.submit_cancellable(
                move || {
                    ran.store(true, Ordering::SeqCst);
                    ToolResponse::success(serde_json::json!("expired"), Some("test"))
                },
                Duration::from_millis(20),
                cancel,
            )
        });

        let expired_result = h2.join().expect("expired submitter must not panic");
        assert!(matches!(expired_result, Err(SyncPoolError::Timeout)));
        assert!(expired_cancel.load(Ordering::SeqCst));
        assert!(!expired_ran.load(Ordering::SeqCst));

        // Queue order is blocker -> expired -> sentinel. Once the sentinel
        // completes, the single FIFO worker necessarily examined the expired
        // position before invoking the sentinel.
        let sentinel_ran = Arc::new(AtomicBool::new(false));
        let p3 = pool.clone();
        let sentinel_flag = sentinel_ran.clone();
        let sentinel = std::thread::spawn(move || {
            p3.submit(
                move || {
                    sentinel_flag.store(true, Ordering::SeqCst);
                    ToolResponse::success(serde_json::json!("sentinel"), Some("test"))
                },
                Duration::from_secs(5),
            )
        });

        blocker.release();
        assert!(
            h1.join()
                .expect("blocking submitter must not panic")
                .unwrap()
                .ok
        );
        assert!(
            sentinel
                .join()
                .expect("sentinel submitter must not panic")
                .unwrap()
                .ok
        );
        assert!(sentinel_ran.load(Ordering::SeqCst));
        assert!(!expired_ran.load(Ordering::SeqCst));

        let recovered = pool.submit(
            || ToolResponse::success(serde_json::json!("recovered"), Some("test")),
            Duration::from_secs(5),
        );
        assert!(recovered.unwrap().ok, "pool must remain usable");
    }

    #[test]
    fn timed_out_running_retains_worker() {
        // Pool with 1 worker, queue capacity 1.
        let pool = Arc::new(SyncExecutionPool::with_limits(1, 1));
        let gate = Arc::new(BlockingJobGate::new());

        // Job 1: blocks the worker.
        let p1 = pool.clone();
        let g = gate.clone();
        let h1 = std::thread::spawn(move || {
            p1.submit_cancellable(
                move || {
                    g.arrive_and_wait();
                    ToolResponse::success(serde_json::json!({}), Some("test"))
                },
                Duration::from_millis(50),
                Arc::new(AtomicBool::new(false)),
            )
        });

        // Wait for the handler to start (worker is occupied).
        gate.wait_until_started();

        // Wait for the caller to receive timeout.
        let result = h1.join().expect("submitter must not panic");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SyncPoolError::Timeout));

        // Worker is still busy. Submit a sentinel that goes into the queue.
        // Use enqueue signal to confirm it's in the queue before submitting job 3.
        let sentinel_ran = Arc::new(AtomicBool::new(false));
        let sentinel_flag = sentinel_ran.clone();
        let sentinel_enqueued = Arc::new(TestEnqueueSignal::new());
        let p2 = pool.clone();
        let enqueue_signal = sentinel_enqueued.clone();
        let sentinel = std::thread::spawn(move || {
            p2.submit_cancellable_with_enqueue_signal(
                move || {
                    sentinel_flag.store(true, Ordering::SeqCst);
                    ToolResponse::success(serde_json::json!("sentinel"), Some("test"))
                },
                Duration::from_secs(5),
                Arc::new(AtomicBool::new(false)),
                enqueue_signal,
            )
        });
        sentinel_enqueued.wait_until_entered();

        // Queue is now full (worker busy + sentinel queued).
        // Submit job 3 → QueueFull.
        let r3 = pool.submit(
            move || ToolResponse::success(serde_json::json!({}), Some("test")),
            Duration::from_millis(50),
        );
        assert!(
            matches!(r3, Err(SyncPoolError::QueueFull { worker_count: 1 })),
            "expected QueueFull while timed-out handler still owns the worker, got {:?}",
            r3
        );

        // Release the handler.
        gate.release();

        // Wait for sentinel to complete (proves worker finished job 1).
        let sentinel_result = sentinel.join().expect("sentinel must not panic");
        assert!(sentinel_result.unwrap().ok);
        assert!(sentinel_ran.load(Ordering::SeqCst));

        // Pool recovers.
        let r4 = pool.submit(
            move || ToolResponse::success(serde_json::json!("after"), Some("test")),
            Duration::from_secs(5),
        );
        assert!(r4.unwrap().ok, "pool must recover after handler finishes");
    }

    #[test]
    fn queue_saturation_does_not_set_cancel() {
        let pool = Arc::new(SyncExecutionPool::with_limits(1, 1));
        let flag = Arc::new(AtomicBool::new(false));
        let blocker = Arc::new(BlockingJobGate::new());
        let enqueued = Arc::new(TestEnqueueSignal::new());

        // Job 1: blocks the worker.
        let p1 = pool.clone();
        let blocker_for_job = blocker.clone();
        let h1 = std::thread::spawn(move || {
            p1.submit(
                move || {
                    blocker_for_job.arrive_and_wait();
                    ToolResponse::success(serde_json::json!({}), Some("test"))
                },
                Duration::from_secs(5),
            )
        });
        blocker.wait_until_started();

        // Job 2: fills the queue. Use enqueue signal for deterministic ordering.
        let p2 = pool.clone();
        let enqueue_signal = enqueued.clone();
        let h2 = std::thread::spawn(move || {
            p2.submit_cancellable_with_enqueue_signal(
                move || ToolResponse::success(serde_json::json!({}), Some("test")),
                Duration::from_secs(5),
                Arc::new(AtomicBool::new(false)),
                enqueue_signal,
            )
        });
        enqueued.wait_until_entered();

        // Job 3: submit with own flag. Should get QueueFull, flag must NOT be set.
        let flag_clone = flag.clone();
        let r3 = pool.submit_cancellable(
            move || ToolResponse::success(serde_json::json!({}), Some("test")),
            Duration::from_millis(100),
            flag_clone,
        );
        assert!(
            matches!(r3, Err(SyncPoolError::QueueFull { .. })),
            "expected QueueFull, got {:?}",
            r3
        );
        assert!(
            !flag.load(Ordering::SeqCst),
            "cancel flag must NOT be set on QueueFull"
        );

        // Release and drain.
        blocker.release();
        let r1 = h1.join().expect("h1 panic");
        let r2 = h2.join().expect("h2 panic");
        assert!(r1.expect("job1").ok);
        assert!(r2.expect("job2").ok);
    }

    #[test]
    fn disconnected_maps_to_shutdown() {
        // We can't easily drop the pool's receiver, but we can verify that
        // the Shutdown variant is produced by send_error and has the right
        // machine code. The actual disconnection path is tested indirectly
        // through pool drop semantics.
        let resp = SyncPoolError::Shutdown.to_tool_response("my_tool");
        assert!(!resp.ok);
        assert_eq!(
            resp.machine_code.as_deref(),
            Some(crate::mcp::machine_codes::INTERNAL_ERROR)
        );
        assert_ne!(
            resp.machine_code.as_deref(),
            Some(crate::mcp::machine_codes::TIMEOUT),
            "Shutdown must not map to TIMEOUT"
        );
    }

    // ── wait_for_reply classification tests ──────────────────────────

    #[cfg(test)]
    fn test_pending_job(
        reply_rx: Receiver<ToolResponse>,
        cancel_flag: Arc<AtomicBool>,
    ) -> PendingJob {
        PendingJob {
            reply_rx,
            cancel_flag,
            abandoned: Arc::new(AtomicBool::new(false)),
            stuck: Arc::new(AtomicUsize::new(0)),
        }
    }

    #[test]
    fn wait_for_reply_success_returns_response() {
        let (tx, rx) = sync_channel(1);
        let flag = Arc::new(AtomicBool::new(false));
        tx.send(ToolResponse::success(serde_json::json!("ok"), Some("test")))
            .unwrap();
        let pending = test_pending_job(rx, flag.clone());
        let result = wait_for_reply(&pending, Duration::from_secs(1));
        assert!(result.is_ok());
        assert!(result.unwrap().ok);
        assert!(
            !flag.load(Ordering::SeqCst),
            "flag must not be set on success"
        );
    }

    #[test]
    fn wait_for_reply_timeout_sets_cancel_flag() {
        let (_tx, rx) = sync_channel::<ToolResponse>(1);
        let flag = Arc::new(AtomicBool::new(false));
        let pending = test_pending_job(rx, flag.clone());
        let result = wait_for_reply(&pending, Duration::from_millis(10));
        assert!(
            matches!(result, Err(SyncPoolError::Timeout)),
            "expected Timeout, got {:?}",
            result
        );
        assert!(flag.load(Ordering::SeqCst), "flag must be set on timeout");
    }

    #[test]
    fn wait_for_reply_disconnected_returns_shutdown() {
        let (tx, rx) = sync_channel::<ToolResponse>(1);
        let flag = Arc::new(AtomicBool::new(false));
        drop(tx);
        let pending = test_pending_job(rx, flag.clone());
        let result = wait_for_reply(&pending, Duration::from_secs(1));
        assert!(
            matches!(result, Err(SyncPoolError::Shutdown)),
            "expected Shutdown, got {:?}",
            result
        );
        assert!(
            !flag.load(Ordering::SeqCst),
            "flag must NOT be set on disconnected shutdown"
        );
    }

    #[test]
    fn wait_for_reply_timeout_with_sender_retained_sets_cancel() {
        let (_tx, rx) = sync_channel::<ToolResponse>(1);
        let flag = Arc::new(AtomicBool::new(false));
        let pending = test_pending_job(rx, flag.clone());
        let result = wait_for_reply(&pending, Duration::from_millis(5));
        assert!(matches!(result, Err(SyncPoolError::Timeout)));
        assert!(flag.load(Ordering::SeqCst));
        // Sender is still alive — this is a timeout, not shutdown.
    }

    #[test]
    fn repeated_timeouts_do_not_increase_worker_count() {
        let pool = SyncExecutionPool::with_limits(2, 4);
        assert_eq!(pool.worker_count(), 2);

        for _ in 0..5 {
            let _ = pool.submit_cancellable(
                move || {
                    std::thread::sleep(Duration::from_millis(50));
                    ToolResponse::success(serde_json::json!({}), Some("test"))
                },
                Duration::from_millis(5),
                Arc::new(AtomicBool::new(false)),
            );
            std::thread::sleep(Duration::from_millis(15));
        }

        std::thread::sleep(Duration::from_millis(200));

        let r = pool.submit(
            move || ToolResponse::success(serde_json::json!("final"), Some("test")),
            Duration::from_secs(5),
        );
        assert!(r.unwrap().ok, "pool must be usable after repeated timeouts");
        assert_eq!(pool.worker_count(), 2, "worker count must not increase");
    }

    #[test]
    fn queued_externally_cancelled_job_is_skipped_before_sentinel() {
        let pool = Arc::new(SyncExecutionPool::with_limits(1, 4));
        let blocker = Arc::new(BlockingJobGate::new());
        let cancelled_ran = Arc::new(AtomicBool::new(false));
        let cancelled_flag = Arc::new(AtomicBool::new(false));
        let enqueued = Arc::new(TestEnqueueSignal::new());

        let p1 = pool.clone();
        let blocker_for_job = blocker.clone();
        let h1 = std::thread::spawn(move || {
            p1.submit(
                move || {
                    blocker_for_job.arrive_and_wait();
                    ToolResponse::success(serde_json::json!("blocker"), Some("test"))
                },
                Duration::from_secs(5),
            )
        });
        blocker.wait_until_started();

        let p2 = pool.clone();
        let ran = cancelled_ran.clone();
        let flag = cancelled_flag.clone();
        let enqueue_signal = enqueued.clone();
        let h2 = std::thread::spawn(move || {
            p2.submit_cancellable_with_enqueue_signal(
                move || {
                    ran.store(true, Ordering::SeqCst);
                    ToolResponse::success(serde_json::json!("cancelled"), Some("test"))
                },
                Duration::from_secs(5),
                flag,
                enqueue_signal,
            )
        });

        enqueued.wait_until_entered();
        cancelled_flag.store(true, Ordering::SeqCst);

        let sentinel_ran = Arc::new(AtomicBool::new(false));
        let p3 = pool.clone();
        let sentinel_flag = sentinel_ran.clone();
        let sentinel = std::thread::spawn(move || {
            p3.submit(
                move || {
                    sentinel_flag.store(true, Ordering::SeqCst);
                    ToolResponse::success(serde_json::json!("sentinel"), Some("test"))
                },
                Duration::from_secs(5),
            )
        });

        blocker.release();
        assert!(
            h1.join()
                .expect("blocking submitter must not panic")
                .unwrap()
                .ok
        );

        let cancelled_response = h2
            .join()
            .expect("cancelled submitter must not panic")
            .expect("queued cancellation returns response");
        assert!(!cancelled_response.ok);
        assert_eq!(
            cancelled_response.machine_code.as_deref(),
            Some(crate::mcp::machine_codes::CANCELLED)
        );
        assert!(cancelled_flag.load(Ordering::SeqCst));
        assert!(!cancelled_ran.load(Ordering::SeqCst));

        assert!(
            sentinel
                .join()
                .expect("sentinel submitter must not panic")
                .unwrap()
                .ok
        );
        assert!(sentinel_ran.load(Ordering::SeqCst));
        assert!(!cancelled_ran.load(Ordering::SeqCst));
    }
}
