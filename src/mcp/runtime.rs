use crate::calc::set_mcp_mode;
use crate::mcp::registry;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, RwLock};
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::Instant;

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

static ACTIVE_SCHEMA_DETAIL: LazyLock<RwLock<String>> = LazyLock::new(|| {
    let detail = std::env::var("EGGCALC_MCP_SCHEMA_DETAIL")
        .unwrap_or_else(|_| SCHEMA_DETAIL_FULL.to_string());
    RwLock::new(detail)
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
    if level != "compact" && level != "normal" && level != "full" {
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
/// looks up the corresponding active request, and sets its cancel flag.
/// Returns `true` if a cancel flag was actually set.
///
/// This function is extracted from the server's notification handler so
/// it can be unit-tested in isolation without spawning the stdio loop.
#[doc(hidden)]
pub fn apply_cancellation(active: &ActiveRequests, request_id: &Value) -> bool {
    let map = match active.try_lock() {
        Ok(g) => g,
        Err(_) => return false,
    };
    match request_id {
        Value::Bool(_) => false,
        Value::String(s) if s.len() <= MAX_REQUEST_ID_LENGTH => {
            if let Some(req) = map.get(request_id) {
                req.cancel_flag.store(true, Ordering::Relaxed);
                true
            } else {
                false
            }
        }
        Value::Number(n)
            if (n.is_i64() || n.is_u64())
                && request_id.to_string().len() <= MAX_REQUEST_ID_LENGTH =>
        {
            if let Some(req) = map.get(request_id) {
                req.cancel_flag.store(true, Ordering::Relaxed);
                true
            } else {
                false
            }
        }
        _ => false,
    }
}
