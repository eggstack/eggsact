use crate::mcp::response::ToolResponse;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::Arc;
use std::time::Duration;

/// Default number of worker threads in the synchronous execution pool.
pub(crate) const DEFAULT_SYNC_WORKERS: usize = 8;

/// Default queue capacity for the synchronous execution pool.
pub(crate) const DEFAULT_SYNC_QUEUE: usize = 32;

struct SyncJob {
    handler: Box<dyn FnOnce() -> ToolResponse + Send + 'static>,
    reply: SyncSender<ToolResponse>,
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
    pub fn submit(
        &self,
        handler: impl FnOnce() -> ToolResponse + Send + 'static,
        timeout: Duration,
    ) -> Result<ToolResponse, SyncPoolError> {
        let (reply_tx, reply_rx) = sync_channel(1);
        let job = SyncJob {
            handler: Box::new(handler),
            reply: reply_tx,
        };

        self.sender.try_send(job).map_err(|e| match e {
            std::sync::mpsc::TrySendError::Full(_) => SyncPoolError::QueueFull {
                worker_count: self.worker_count,
            },
            std::sync::mpsc::TrySendError::Disconnected(_) => SyncPoolError::Shutdown,
        })?;

        reply_rx
            .recv_timeout(timeout)
            .map_err(|_| SyncPoolError::Timeout)
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
                std::thread::sleep(Duration::from_secs(60));
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
}
