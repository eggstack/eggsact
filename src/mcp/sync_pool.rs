use crate::mcp::response::ToolResponse;
use std::sync::atomic::{AtomicBool, Ordering};
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
        let receiver = Arc::new(std::sync::Mutex::new(receiver));

        for _ in 0..worker_count {
            let rx = receiver.clone();
            std::thread::Builder::new()
                .name("eggsact-sync-worker".to_string())
                .spawn(move || worker_loop(rx))
                .expect("failed to spawn sync worker");
        }

        Self {
            sender,
            worker_count,
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
        let (reply_tx, reply_rx) = sync_channel(1);
        let job = SyncJob {
            handler: Box::new(handler),
            reply: reply_tx,
            cancel_flag: cancel_flag.clone(),
            deadline,
        };

        self.sender.try_send(job).map_err(|e| match e {
            std::sync::mpsc::TrySendError::Full(_) => SyncPoolError::QueueFull {
                worker_count: self.worker_count,
            },
            std::sync::mpsc::TrySendError::Disconnected(_) => SyncPoolError::Shutdown,
        })?;

        wait_for_reply(&reply_rx, timeout, &cancel_flag)
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
        let (reply_tx, reply_rx) = sync_channel(1);
        let job = SyncJob {
            handler: Box::new(handler),
            reply: reply_tx,
            cancel_flag: cancel_flag.clone(),
            deadline,
        };

        self.sender.try_send(job).map_err(|e| match e {
            std::sync::mpsc::TrySendError::Full(_) => SyncPoolError::QueueFull {
                worker_count: self.worker_count,
            },
            std::sync::mpsc::TrySendError::Disconnected(_) => SyncPoolError::Shutdown,
        })?;
        signal.signal();

        wait_for_reply(&reply_rx, timeout, &cancel_flag)
    }

    /// Return the number of worker threads in this pool.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn worker_count(&self) -> usize {
        self.worker_count
    }
}

fn worker_loop(receiver: Arc<std::sync::Mutex<Receiver<SyncJob>>>) {
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
/// On timeout, sets the cancellation flag before returning `SyncPoolError::Timeout`
/// so the handler (if still running or queued) can observe the cancellation and
/// exit early. On disconnected sender, returns `SyncPoolError::Shutdown` without
/// setting the flag (the pool channel has shut down, not this invocation).
fn wait_for_reply(
    reply_rx: &Receiver<ToolResponse>,
    timeout: Duration,
    cancel_flag: &Arc<AtomicBool>,
) -> Result<ToolResponse, SyncPoolError> {
    match reply_rx.recv_timeout(timeout) {
        Ok(response) => Ok(response),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            cancel_flag.store(true, Ordering::SeqCst);
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
    use std::sync::atomic::{AtomicBool, Ordering};

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
        let pool = SyncExecutionPool::with_limits(2, 4);
        let started_first = Arc::new(AtomicBool::new(false));
        let started_second = Arc::new(AtomicBool::new(false));

        let s1 = started_first.clone();
        let r1 = pool.submit(
            move || {
                s1.store(true, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(50));
                ToolResponse::success(serde_json::json!({"id": 1}), Some("test"))
            },
            Duration::from_secs(5),
        );

        // Wait a tiny bit so the first job is running on a worker
        std::thread::sleep(Duration::from_millis(10));

        let s2 = started_second.clone();
        let r2 = pool.submit(
            move || {
                s2.store(true, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(50));
                ToolResponse::success(serde_json::json!({"id": 2}), Some("test"))
            },
            Duration::from_secs(5),
        );

        let resp1 = r1.expect("first job should succeed");
        let resp2 = r2.expect("second job should succeed");
        assert!(resp1.ok);
        assert!(resp2.ok);
        // Both jobs ran
        assert!(started_first.load(Ordering::SeqCst));
        assert!(started_second.load(Ordering::SeqCst));
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
    fn queue_saturation_returns_queue_full() {
        // Pool with 1 worker, queue capacity 1 → can handle 2 concurrent jobs max.
        let pool = Arc::new(SyncExecutionPool::with_limits(1, 1));

        // Submit job 1 from a separate thread (long-running, blocks the worker).
        let p1 = pool.clone();
        let h1 = std::thread::spawn(move || {
            p1.submit(
                move || {
                    std::thread::sleep(Duration::from_millis(500));
                    ToolResponse::success(serde_json::json!({}), Some("test"))
                },
                Duration::from_secs(5),
            )
        });
        // Wait for job 1 to be accepted by the worker.
        std::thread::sleep(Duration::from_millis(50));

        // Submit job 2 from a separate thread (goes into the queue buffer).
        let p2 = pool.clone();
        let h2 = std::thread::spawn(move || {
            p2.submit(
                move || {
                    std::thread::sleep(Duration::from_millis(500));
                    ToolResponse::success(serde_json::json!({}), Some("test"))
                },
                Duration::from_secs(5),
            )
        });
        // Wait for job 2 to land in the buffer.
        std::thread::sleep(Duration::from_millis(50));

        // Job 3 from the main thread — worker busy + queue full → QueueFull.
        let r3 = pool.submit(
            move || ToolResponse::success(serde_json::json!({}), Some("test")),
            Duration::from_millis(200),
        );
        assert!(
            matches!(r3, Err(SyncPoolError::QueueFull { worker_count: 1 })),
            "expected QueueFull, got {:?}",
            r3
        );

        // Drain the queued jobs so threads don't hang.
        let r1 = h1.join().expect("h1 panic");
        let r2 = h2.join().expect("h2 panic");
        assert!(r1.expect("job1").ok);
        assert!(r2.expect("job2").ok);
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

        // Give the timeout path time to set the flag.
        std::thread::sleep(Duration::from_millis(20));

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
        // After timeout, the worker is still occupied until the handler finishes.
        let pool = SyncExecutionPool::with_limits(1, 1);
        let handler_started = Arc::new(AtomicBool::new(false));
        let started = handler_started.clone();

        let result = pool.submit_cancellable(
            move || {
                started.store(true, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(200));
                ToolResponse::success(serde_json::json!({}), Some("test"))
            },
            Duration::from_millis(10),
            Arc::new(AtomicBool::new(false)),
        );

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SyncPoolError::Timeout));
        assert!(handler_started.load(Ordering::SeqCst));

        // Worker is still busy — a new submission should fail with QueueFull
        // if the queue is also full.
        let r2 = pool.submit(
            move || ToolResponse::success(serde_json::json!({}), Some("test")),
            Duration::from_millis(50),
        );
        // Depending on timing, this may be QueueFull or may succeed if the
        // handler finished fast. Either is acceptable.
        let _ = r2;

        // Wait for the handler to finish and the worker to drain queued jobs.
        std::thread::sleep(Duration::from_millis(500));

        // Pool should be usable again.
        let r3 = pool.submit(
            move || ToolResponse::success(serde_json::json!("after"), Some("test")),
            Duration::from_secs(5),
        );
        assert!(r3.unwrap().ok, "pool must recover after handler finishes");
    }

    #[test]
    fn queue_saturation_does_not_set_cancel() {
        let pool = Arc::new(SyncExecutionPool::with_limits(1, 1));
        let flag = Arc::new(AtomicBool::new(false));

        // Block the worker.
        let p1 = pool.clone();
        let h1 = std::thread::spawn(move || {
            p1.submit(
                move || {
                    std::thread::sleep(Duration::from_millis(300));
                    ToolResponse::success(serde_json::json!({}), Some("test"))
                },
                Duration::from_secs(5),
            )
        });
        std::thread::sleep(Duration::from_millis(20));

        // Fill the queue.
        let p2 = pool.clone();
        let h2 = std::thread::spawn(move || {
            p2.submit(
                move || {
                    std::thread::sleep(Duration::from_millis(300));
                    ToolResponse::success(serde_json::json!({}), Some("test"))
                },
                Duration::from_secs(5),
            )
        });
        std::thread::sleep(Duration::from_millis(20));

        // This should get QueueFull, NOT Timeout.
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

        // Drain.
        let _ = h1.join();
        let _ = h2.join();
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

    #[test]
    fn wait_for_reply_success_returns_response() {
        let (tx, rx) = sync_channel(1);
        let flag = Arc::new(AtomicBool::new(false));
        tx.send(ToolResponse::success(serde_json::json!("ok"), Some("test")))
            .unwrap();
        let result = wait_for_reply(&rx, Duration::from_secs(1), &flag);
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
        let result = wait_for_reply(&rx, Duration::from_millis(10), &flag);
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
        let result = wait_for_reply(&rx, Duration::from_secs(1), &flag);
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
        let result = wait_for_reply(&rx, Duration::from_millis(5), &flag);
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
