use crate::agent::ToolAudience;
use crate::calc::set_mcp_mode;
use crate::mcp::registry;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock, RwLock};
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::Instant;

/// RAII guard for an active request entry. On drop, removes the entry from
/// the active-request map if the cancel flag still matches (preventing an
/// old task from removing a newer request that reused the same ID).
#[doc(hidden)]
pub struct RequestGuard {
    active: ActiveRequests,
    cancel_flag_addr: usize,
    request_id: Value,
}

impl RequestGuard {
    #[doc(hidden)]
    pub fn new(active: ActiveRequests, cancel_flag: &Arc<AtomicBool>, request_id: Value) -> Self {
        Self {
            active,
            cancel_flag_addr: Arc::as_ptr(cancel_flag) as usize,
            request_id,
        }
    }
}

impl Drop for RequestGuard {
    fn drop(&mut self) {
        // Best-effort removal. Use try_lock to avoid blocking on drop;
        // if the map is contended, the entry will be cleaned up by
        // the next operation or shutdown.
        if let Ok(mut map) = self.active.try_lock() {
            if let Some(entry) = map.get(&self.request_id) {
                let entry_addr = Arc::as_ptr(&entry.cancel_flag) as usize;
                if entry_addr == self.cancel_flag_addr {
                    map.remove(&self.request_id);
                }
            }
        }
    }
}

#[doc(hidden)]
pub const MAX_REQUESTS_PER_SECOND: u32 = 10;
#[doc(hidden)]
pub const MAX_IN_FLIGHT_REQUESTS: usize = 32;

#[doc(hidden)]
pub const MAX_TOOL_WORKERS: usize = 16;
#[doc(hidden)]
pub const MAX_REQUEST_ID_LENGTH: usize = 1024;

// Items below are exposed for tests in `tests/`. They are not part of the
// stable API surface; downstream crates should not depend on them. They
// are `pub` (rather than `pub(crate)`) so integration tests can reach them
// through `eggsact::mcp::runtime::*` and `eggsact::mcp::runtime::test_support::*`.
#[doc(hidden)]
pub const MAX_REQUEST_BYTES: usize = 1_000_000;
#[doc(hidden)]
pub const MAX_OUTPUT_BYTES: usize = 1_000_000;

pub(crate) const SCHEMA_DETAIL_FULL: &str = "full";

pub const MCP_PROTOCOL_VERSION: &str = "2024-11-05";
pub const MCP_SERVER_NAME: &str = "eggsact";

static ACTIVE_PROFILE: LazyLock<RwLock<String>> = LazyLock::new(|| {
    let profile = std::env::var("EGGCALC_MCP_PROFILE").unwrap_or_else(|_| "full".to_string());
    if !registry::PROFILE_NAMES.contains(&profile.as_str()) {
        let available: Vec<&str> = registry::PROFILE_NAMES.to_vec();
        eprintln!(
            "Error: Invalid EGGCALC_MCP_PROFILE: {:?}. Available profiles: {}",
            profile,
            available.join(", ")
        );
        std::process::exit(1);
    }
    RwLock::new(profile)
});

/// Parse a schema detail string, returning the validated value or `None` for
/// invalid input. Valid values are `"compact"`, `"normal"`, and `"full"`.
/// Empty strings and unknown values return `None`.
#[doc(hidden)]
pub fn parse_schema_detail(s: &str) -> Option<&'static str> {
    match s {
        "compact" => Some("compact"),
        "normal" => Some("normal"),
        "full" => Some("full"),
        _ => None,
    }
}

static ACTIVE_SCHEMA_DETAIL: LazyLock<RwLock<String>> = LazyLock::new(|| {
    let raw = std::env::var("EGGCALC_MCP_SCHEMA_DETAIL")
        .unwrap_or_else(|_| SCHEMA_DETAIL_FULL.to_string());
    match parse_schema_detail(&raw) {
        Some(valid) => RwLock::new(valid.to_string()),
        None => {
            eprintln!(
                "Warning: Invalid EGGCALC_MCP_SCHEMA_DETAIL: {:?}. \
                 Accepted values: compact, normal, full. Defaulting to full.",
                raw
            );
            RwLock::new(SCHEMA_DETAIL_FULL.to_string())
        }
    }
});

/// Parse an audience string into a `ToolAudience` variant.
///
/// Matching is case-insensitive. Invalid values default to `Model` with a
/// diagnostic warning on stderr. This function is exposed as `pub` (not
/// `pub(crate)`) so integration tests can reach it.
#[doc(hidden)]
pub fn parse_audience(s: &str) -> ToolAudience {
    match s.to_lowercase().as_str() {
        "model" => ToolAudience::Model,
        "harness" => ToolAudience::Harness,
        "debug" => ToolAudience::Debug,
        other => {
            eprintln!(
                "Warning: Invalid EGGCALC_MCP_AUDIENCE: {:?}. Defaulting to Model. Use Model, Harness, or Debug.",
                other
            );
            ToolAudience::Model
        }
    }
}

static ACTIVE_AUDIENCE: LazyLock<RwLock<ToolAudience>> = LazyLock::new(|| {
    let audience_str =
        std::env::var("EGGCALC_MCP_AUDIENCE").unwrap_or_else(|_| "Model".to_string());
    RwLock::new(parse_audience(&audience_str))
});

static MCP_DEFAULTS_CONFIGURED: AtomicBool = AtomicBool::new(false);

pub fn set_active_profile(name: &str) -> Result<(), String> {
    if !registry::PROFILE_NAMES.contains(&name) {
        let available: Vec<&str> = registry::PROFILE_NAMES.to_vec();
        return Err(format!(
            "Unknown profile: {:?}. Available profiles: {}",
            name,
            available.join(", ")
        ));
    }
    let mut profile = ACTIVE_PROFILE.write().map_err(|e| e.to_string())?;
    *profile = name.to_string();
    Ok(())
}

pub fn get_active_profile() -> String {
    let profile = ACTIVE_PROFILE.read().unwrap_or_else(|e| e.into_inner());
    profile.clone()
}

pub fn set_schema_detail(level: &str) -> Result<(), String> {
    if parse_schema_detail(level).is_none() {
        return Err(format!(
            "Invalid schema detail: {:?}. Use compact, normal, or full.",
            level
        ));
    }
    let mut detail = ACTIVE_SCHEMA_DETAIL.write().map_err(|e| e.to_string())?;
    *detail = level.to_string();
    Ok(())
}

pub fn get_schema_detail() -> String {
    let detail = ACTIVE_SCHEMA_DETAIL
        .read()
        .unwrap_or_else(|e| e.into_inner());
    detail.clone()
}

pub fn get_active_audience() -> ToolAudience {
    let audience = ACTIVE_AUDIENCE.read().unwrap_or_else(|e| e.into_inner());
    *audience
}

pub fn ensure_mcp_defaults() {
    if !MCP_DEFAULTS_CONFIGURED.swap(true, Ordering::SeqCst) {
        set_mcp_mode();
    }
}

pub fn truncate_2000(s: &str) -> String {
    s.chars().take(2000).collect()
}

pub struct RateLimiter {
    timestamps: VecDeque<Instant>,
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            timestamps: VecDeque::new(),
        }
    }

    pub fn check(&mut self) -> bool {
        let now = Instant::now();
        while let Some(&front) = self.timestamps.front() {
            if now.duration_since(front) > Duration::from_secs(1) {
                self.timestamps.pop_front();
            } else {
                break;
            }
        }
        if self.timestamps.len() < MAX_REQUESTS_PER_SECOND as usize {
            self.timestamps.push_back(now);
            true
        } else {
            false
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Active request tracking
// ═══════════════════════════════════════════════════════════════════════════════

/// State for an in-flight MCP request, used for concurrent request handling.
#[doc(hidden)]
pub struct ActiveRequest {
    pub cancel_flag: Arc<AtomicBool>,
    #[allow(dead_code)]
    pub started_at: Instant,
    #[allow(dead_code)]
    pub method: String,
}

#[doc(hidden)]
pub type ActiveRequests = Arc<Mutex<HashMap<Value, ActiveRequest>>>;

/// Create a new shared active requests map.
#[doc(hidden)]
pub fn new_active_requests() -> ActiveRequests {
    Arc::new(Mutex::new(HashMap::new()))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Runtime metrics
// ═══════════════════════════════════════════════════════════════════════════════

/// Global runtime metrics for the MCP server. All counters are atomic and
/// RAII-guarded so they decrement correctly on panic/unwind.
pub struct RuntimeMetrics {
    /// Number of currently active request tasks (registered in the active map).
    pub active_requests: AtomicUsize,
    /// Number of currently running blocking handler closures (inside spawn_blocking).
    pub active_blocking_handlers: AtomicUsize,
    /// Number of handlers that have timed out but whose blocking closure is still running.
    pub timed_out_handlers: AtomicUsize,
    /// Total number of timeout responses returned to clients.
    pub total_timeouts: AtomicUsize,
    /// Peak number of concurrent blocking handlers observed.
    pub peak_blocking_concurrency: AtomicUsize,
}

impl RuntimeMetrics {
    const fn new() -> Self {
        Self {
            active_requests: AtomicUsize::new(0),
            active_blocking_handlers: AtomicUsize::new(0),
            timed_out_handlers: AtomicUsize::new(0),
            total_timeouts: AtomicUsize::new(0),
            peak_blocking_concurrency: AtomicUsize::new(0),
        }
    }
}

/// Global runtime metrics instance.
#[doc(hidden)]
pub static RUNTIME_METRICS: LazyLock<RuntimeMetrics> = LazyLock::new(RuntimeMetrics::new);

/// RAII guard that increments a metric on creation and decrements on drop.
/// Used for active_requests and active_blocking_handlers counters.
#[doc(hidden)]
pub struct MetricGuard {
    counter: &'static AtomicUsize,
}

impl MetricGuard {
    /// Create a new guard that increments the given counter immediately.
    pub fn new(counter: &'static AtomicUsize) -> Self {
        counter.fetch_add(1, Ordering::Relaxed);
        Self { counter }
    }
}

impl Drop for MetricGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Snapshot of runtime metrics for diagnostics.
#[doc(hidden)]
pub struct MetricsSnapshot {
    pub active_requests: usize,
    pub active_blocking_handlers: usize,
    pub timed_out_handlers: usize,
    pub total_timeouts: usize,
    pub peak_blocking_concurrency: usize,
}

/// Take a snapshot of current runtime metrics.
#[doc(hidden)]
pub fn snapshot_metrics() -> MetricsSnapshot {
    MetricsSnapshot {
        active_requests: RUNTIME_METRICS.active_requests.load(Ordering::Relaxed),
        active_blocking_handlers: RUNTIME_METRICS
            .active_blocking_handlers
            .load(Ordering::Relaxed),
        timed_out_handlers: RUNTIME_METRICS.timed_out_handlers.load(Ordering::Relaxed),
        total_timeouts: RUNTIME_METRICS.total_timeouts.load(Ordering::Relaxed),
        peak_blocking_concurrency: RUNTIME_METRICS
            .peak_blocking_concurrency
            .load(Ordering::Relaxed),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Request registration
// ═══════════════════════════════════════════════════════════════════════════════

/// Errors that can occur when registering a new active request.
#[derive(Debug)]
#[doc(hidden)]
pub enum RegisterRequestError {
    /// A request with this non-null ID is already active.
    DuplicateId,
    /// The in-flight request limit has been reached.
    CapacityExceeded,
}

/// Register a new active request under a single lock acquisition.
///
/// Checks in-flight limits and duplicate IDs atomically, then inserts the
/// request into the active map. Returns a `RequestGuard` whose drop removes
/// the entry (with generation matching to prevent stale cleanup).
#[doc(hidden)]
pub async fn register_request(
    active: &ActiveRequests,
    cancel_flag: &Arc<AtomicBool>,
    request_id: Value,
    method: String,
) -> Result<RequestGuard, RegisterRequestError> {
    let mut map = active.lock().await;
    if map.len() >= MAX_IN_FLIGHT_REQUESTS {
        return Err(RegisterRequestError::CapacityExceeded);
    }
    if map.contains_key(&request_id) {
        return Err(RegisterRequestError::DuplicateId);
    }
    map.insert(
        request_id.clone(),
        ActiveRequest {
            cancel_flag: cancel_flag.clone(),
            started_at: Instant::now(),
            method,
        },
    );
    Ok(RequestGuard::new(active.clone(), cancel_flag, request_id))
}

/// Test-only helpers for constructing runtime state from integration tests.
///
/// This module is `#[doc(hidden)]` and not part of the public API; it is
/// exposed only so `tests/` integration tests can construct runtime
/// primitives without relying on `pub(crate)` items.
#[doc(hidden)]
pub mod test_support {
    use super::{ActiveRequest, Instant};
    use std::sync::{atomic::AtomicBool, Arc};

    pub fn make_active_request(cancel_flag: Arc<AtomicBool>) -> ActiveRequest {
        ActiveRequest {
            cancel_flag,
            started_at: Instant::now(),
            method: "test".to_string(),
        }
    }
}

/// Apply a cancellation notification to a single active request ID.
///
/// Validates the request ID type (string or integer within size limits),
/// looks up the corresponding active request, clones its cancel flag Arc,
/// releases the lock, then sets the flag outside the critical section.
/// Returns `true` if a cancel flag was actually set.
///
/// This is an async function that properly awaits the active-map lock
/// rather than using `try_lock()`, which can lose cancellations when
/// the map is briefly contended.
///
/// This function is extracted from the server's notification handler so
/// it can be unit-tested in isolation without spawning the stdio loop.
#[doc(hidden)]
pub async fn apply_cancellation(active: &ActiveRequests, request_id: &Value) -> bool {
    // Validate ID type and clone the cancel flag Arc while holding the lock.
    let maybe_flag: Option<Arc<AtomicBool>> = {
        let map = active.lock().await;
        match request_id {
            Value::Bool(_) => None,
            Value::String(s) if s.len() <= MAX_REQUEST_ID_LENGTH => {
                map.get(request_id).map(|req| req.cancel_flag.clone())
            }
            Value::Number(n)
                if (n.is_i64() || n.is_u64())
                    && request_id.to_string().len() <= MAX_REQUEST_ID_LENGTH =>
            {
                map.get(request_id).map(|req| req.cancel_flag.clone())
            }
            _ => None,
        }
    };
    // Set the flag outside the critical section — no lock held.
    if let Some(flag) = maybe_flag {
        flag.store(true, Ordering::Relaxed);
        true
    } else {
        false
    }
}
