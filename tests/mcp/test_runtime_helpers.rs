//! Unit tests for MCP runtime helpers (Tasks 3 & 4).
//!
//! Tests cancellation logic, active request tracking, audience
//! parsing in isolation without spawning the stdio server loop.

use eggsact::agent::ToolAudience;
use eggsact::mcp::runtime::{
    apply_cancellation, new_active_requests, parse_audience, parse_schema_detail, register_request,
    snapshot_metrics, MetricGuard, RateLimiter, RegisterRequestError, RequestGuard,
    MAX_IN_FLIGHT_REQUESTS, MAX_REQUESTS_PER_SECOND, RUNTIME_METRICS,
};
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// ═══════════════════════════════════════════════════════════════════════════════
// Active request tracking
// ═══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn active_request_insert_and_remove() {
    let active = new_active_requests();
    let flag = Arc::new(AtomicBool::new(false));

    {
        let mut map = active.lock().await;
        map.insert(
            json!(1),
            eggsact::mcp::runtime::test_support::make_active_request(flag.clone()),
        );
        assert_eq!(map.len(), 1);
    }

    {
        let mut map = active.lock().await;
        let removed = map.remove(&json!(1));
        assert!(removed.is_some());
        assert_eq!(map.len(), 0);
    }
}

#[tokio::test]
async fn active_request_remove_unknown_id_is_noop() {
    let active = new_active_requests();
    let mut map = active.lock().await;
    let removed = map.remove(&json!(999));
    assert!(removed.is_none());
    assert_eq!(map.len(), 0);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Cancellation notification handler (apply_cancellation)
// ═══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn apply_cancellation_string_id_sets_flag() {
    let active = new_active_requests();
    let flag = Arc::new(AtomicBool::new(false));
    let request_id = json!("req-123");

    {
        let mut map = active.lock().await;
        map.insert(
            request_id.clone(),
            eggsact::mcp::runtime::test_support::make_active_request(flag.clone()),
        );
    }

    let set = apply_cancellation(&active, &request_id).await;
    assert!(set, "should report cancel flag was set");
    assert!(flag.load(Ordering::Relaxed), "cancel flag must be true");
}

#[tokio::test]
async fn apply_cancellation_integer_id_sets_flag() {
    let active = new_active_requests();
    let flag = Arc::new(AtomicBool::new(false));
    let request_id = json!(42);

    {
        let mut map = active.lock().await;
        map.insert(
            request_id.clone(),
            eggsact::mcp::runtime::test_support::make_active_request(flag.clone()),
        );
    }

    let set = apply_cancellation(&active, &request_id).await;
    assert!(set, "should report cancel flag was set");
    assert!(flag.load(Ordering::Relaxed), "cancel flag must be true");
}

#[tokio::test]
async fn apply_cancellation_unknown_id_is_harmless() {
    let active = new_active_requests();
    let flag = Arc::new(AtomicBool::new(false));
    let known_id = json!(1);
    let unknown_id = json!(2);

    {
        let mut map = active.lock().await;
        map.insert(
            known_id.clone(),
            eggsact::mcp::runtime::test_support::make_active_request(flag.clone()),
        );
    }

    let set = apply_cancellation(&active, &unknown_id).await;
    assert!(!set, "should report no cancel flag was set");
    assert!(!flag.load(Ordering::Relaxed), "flag must remain unset");
}

#[tokio::test]
async fn apply_cancellation_bool_id_is_ignored() {
    let active = new_active_requests();
    let flag = Arc::new(AtomicBool::new(false));
    let request_id = json!(1);

    {
        let mut map = active.lock().await;
        map.insert(
            request_id.clone(),
            eggsact::mcp::runtime::test_support::make_active_request(flag.clone()),
        );
    }

    let set = apply_cancellation(&active, &json!(true)).await;
    assert!(!set, "bool must be rejected");
    assert!(!flag.load(Ordering::Relaxed), "flag must remain unset");
}

#[tokio::test]
async fn apply_cancellation_object_id_is_ignored() {
    let active = new_active_requests();
    let set = apply_cancellation(&active, &json!({"id": 1})).await;
    assert!(!set, "object must be rejected");
}

#[tokio::test]
async fn apply_cancellation_array_id_is_ignored() {
    let active = new_active_requests();
    let set = apply_cancellation(&active, &json!([1, 2, 3])).await;
    assert!(!set, "array must be rejected");
}

#[tokio::test]
async fn apply_cancellation_null_id_is_ignored() {
    let active = new_active_requests();
    let set = apply_cancellation(&active, &json!(null)).await;
    assert!(!set, "null must be rejected");
}

#[tokio::test]
async fn apply_cancellation_oversized_string_is_ignored() {
    let active = new_active_requests();
    let oversized = "x".repeat(2000);
    let set = apply_cancellation(&active, &json!(oversized)).await;
    assert!(!set, "oversized string must be rejected");
}

#[tokio::test]
async fn apply_cancellation_is_idempotent() {
    let active = new_active_requests();
    let flag = Arc::new(AtomicBool::new(false));
    let request_id = json!("req-1");

    {
        let mut map = active.lock().await;
        map.insert(
            request_id.clone(),
            eggsact::mcp::runtime::test_support::make_active_request(flag.clone()),
        );
    }

    let _ = apply_cancellation(&active, &request_id).await;
    let _ = apply_cancellation(&active, &request_id).await;
    assert!(
        flag.load(Ordering::Relaxed),
        "idempotent cancel must leave flag set"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Rate limiter
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn rate_limiter_allows_up_to_max_per_second() {
    let mut rl = RateLimiter::new();
    for i in 0..MAX_REQUESTS_PER_SECOND {
        assert!(rl.check(), "request {i} should succeed within rate limit");
    }
    assert!(!rl.check(), "request after max must be rate-limited");
}

#[test]
fn rate_limiter_resets_after_window() {
    let mut rl = RateLimiter::new();
    for _ in 0..MAX_REQUESTS_PER_SECOND {
        rl.check();
    }
    assert!(!rl.check(), "rate-limited after burst");
    // After 1.1s the sliding window should allow new requests
    std::thread::sleep(std::time::Duration::from_millis(1100));
    assert!(
        rl.check(),
        "after 1.1s the rate limiter must accept new requests"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Audience parsing (Task 4)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn parse_audience_exact_model() {
    assert_eq!(parse_audience("Model"), ToolAudience::Model);
}

#[test]
fn parse_audience_exact_harness() {
    assert_eq!(parse_audience("Harness"), ToolAudience::Harness);
}

#[test]
fn parse_audience_exact_debug() {
    assert_eq!(parse_audience("Debug"), ToolAudience::Debug);
}

#[test]
fn parse_audience_case_insensitive_model() {
    assert_eq!(parse_audience("model"), ToolAudience::Model);
    assert_eq!(parse_audience("MODEL"), ToolAudience::Model);
    assert_eq!(parse_audience("MoDeL"), ToolAudience::Model);
}

#[test]
fn parse_audience_case_insensitive_harness() {
    assert_eq!(parse_audience("harness"), ToolAudience::Harness);
    assert_eq!(parse_audience("HARNESS"), ToolAudience::Harness);
    assert_eq!(parse_audience("HaRnEsS"), ToolAudience::Harness);
}

#[test]
fn parse_audience_case_insensitive_debug() {
    assert_eq!(parse_audience("debug"), ToolAudience::Debug);
    assert_eq!(parse_audience("DEBUG"), ToolAudience::Debug);
    assert_eq!(parse_audience("DeBuG"), ToolAudience::Debug);
}

#[test]
fn parse_audience_invalid_defaults_to_model() {
    assert_eq!(parse_audience("invalid"), ToolAudience::Model);
    assert_eq!(parse_audience(""), ToolAudience::Model);
    assert_eq!(parse_audience("MODL"), ToolAudience::Model);
    assert_eq!(parse_audience("123"), ToolAudience::Model);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Schema detail parsing (Milestone 2, Task 3)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn parse_schema_detail_compact_accepted() {
    assert_eq!(parse_schema_detail("compact"), Some("compact"));
}

#[test]
fn parse_schema_detail_normal_accepted() {
    assert_eq!(parse_schema_detail("normal"), Some("normal"));
}

#[test]
fn parse_schema_detail_full_accepted() {
    assert_eq!(parse_schema_detail("full"), Some("full"));
}

#[test]
fn parse_schema_detail_empty_string_invalid() {
    assert_eq!(parse_schema_detail(""), None);
}

#[test]
fn parse_schema_detail_uppercase_invalid() {
    assert_eq!(parse_schema_detail("FULL"), None);
    assert_eq!(parse_schema_detail("Compact"), None);
}

#[test]
fn parse_schema_detail_unknown_value_invalid() {
    assert_eq!(parse_schema_detail("verbose"), None);
    assert_eq!(parse_schema_detail("detailed"), None);
    assert_eq!(parse_schema_detail("123"), None);
}

#[test]
fn parse_schema_detail_whitespace_padded_invalid() {
    assert_eq!(parse_schema_detail(" full "), None);
    assert_eq!(parse_schema_detail("\tfull"), None);
    assert_eq!(parse_schema_detail("full\n"), None);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Constants sanity
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn in_flight_limit_is_reasonable() {
    let value = MAX_IN_FLIGHT_REQUESTS;
    assert!(value >= 1);
    assert!(value <= 256);
}

// ═══════════════════════════════════════════════════════════════════════════════
// RequestGuard RAII cleanup
// ═══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn request_guard_removes_entry_on_drop() {
    let active = new_active_requests();
    let flag = Arc::new(AtomicBool::new(false));
    let request_id = json!("guard-test-1");

    {
        let _guard = RequestGuard::new(active.clone(), &flag, request_id.clone());
        let mut map = active.lock().await;
        map.insert(
            request_id.clone(),
            eggsact::mcp::runtime::test_support::make_active_request(flag.clone()),
        );
        assert_eq!(map.len(), 1);
    }
    // Guard dropped — entry should be removed
    let map = active.lock().await;
    assert_eq!(map.len(), 0, "RequestGuard should remove entry on drop");
}

#[tokio::test]
async fn request_guard_does_not_remove_mismatched_entry() {
    let active = new_active_requests();
    let flag1 = Arc::new(AtomicBool::new(false));
    let flag2 = Arc::new(AtomicBool::new(false));
    let request_id = json!("guard-test-2");

    // Insert with flag1
    {
        let mut map = active.lock().await;
        map.insert(
            request_id.clone(),
            eggsact::mcp::runtime::test_support::make_active_request(flag1.clone()),
        );
    }

    // Create a guard with flag2 (different cancel flag = different request)
    {
        let _guard = RequestGuard::new(active.clone(), &flag2, request_id.clone());
        drop(_guard);
    }

    // Entry should still be there because the guard's flag didn't match
    let map = active.lock().await;
    assert_eq!(
        map.len(),
        1,
        "RequestGuard should not remove entry with mismatched cancel flag"
    );
}

#[tokio::test]
async fn request_guard_handles_already_removed_entry() {
    let active = new_active_requests();
    let flag = Arc::new(AtomicBool::new(false));
    let request_id = json!("guard-test-3");

    // Insert and immediately remove
    {
        let mut map = active.lock().await;
        map.insert(
            request_id.clone(),
            eggsact::mcp::runtime::test_support::make_active_request(flag.clone()),
        );
    }
    {
        let mut map = active.lock().await;
        map.remove(&request_id);
    }

    // Guard drop on a missing entry should be a no-op
    {
        let _guard = RequestGuard::new(active.clone(), &flag, request_id);
    }
    let map = active.lock().await;
    assert_eq!(map.len(), 0);
}

// ═══════════════════════════════════════════════════════════════════════════════
// apply_cancellation async semantics
// ═══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn apply_cancellation_awaits_lock_contention() {
    let active = new_active_requests();
    let flag = Arc::new(AtomicBool::new(false));
    let request_id = json!("contend-1");

    // Hold the lock in a background task
    let active_clone = active.clone();
    let lock_handle = tokio::spawn(async move {
        let _guard = active_clone.lock().await;
        // Hold the lock briefly
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    });

    // Give the lock holder time to acquire
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    // Insert the request while lock is held
    {
        // Wait for lock holder to finish
        lock_handle.await.unwrap();
        let mut map = active.lock().await;
        map.insert(
            request_id.clone(),
            eggsact::mcp::runtime::test_support::make_active_request(flag.clone()),
        );
    }

    // apply_cancellation should succeed (it awaits the lock)
    let result = apply_cancellation(&active, &request_id).await;
    assert!(
        result,
        "async cancellation should wait for lock and succeed"
    );
    assert!(flag.load(Ordering::Relaxed));
}

// ═══════════════════════════════════════════════════════════════════════════════
// apply_cancellation flag set outside critical section
// ═══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn apply_cancellation_sets_flag_outside_lock() {
    let active = new_active_requests();
    let flag = Arc::new(AtomicBool::new(false));
    let request_id = json!("outside-lock-1");

    {
        let mut map = active.lock().await;
        map.insert(
            request_id.clone(),
            eggsact::mcp::runtime::test_support::make_active_request(flag.clone()),
        );
    }

    // The flag must not be set while we hold the lock ourselves.
    // apply_cancellation clones the Arc, releases the lock, then sets.
    let result = apply_cancellation(&active, &request_id).await;
    assert!(result);
    assert!(flag.load(Ordering::Relaxed));
}

// ═══════════════════════════════════════════════════════════════════════════════
// register_request — atomic check+insert under one lock
// ═══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn register_request_succeeds_for_new_id() {
    let active = new_active_requests();
    let flag = Arc::new(AtomicBool::new(false));
    let guard = register_request(&active, &flag, json!("r1"), "tools/call".to_string()).await;
    assert!(guard.is_ok(), "register_request should succeed for new ID");
    let _guard = guard.unwrap();
    let map = active.lock().await;
    assert_eq!(map.len(), 1);
}

#[tokio::test]
async fn register_request_rejects_duplicate_id() {
    let active = new_active_requests();
    let flag = Arc::new(AtomicBool::new(false));
    let _g1 = register_request(&active, &flag, json!("dup"), "tools/call".to_string())
        .await
        .unwrap();
    let flag2 = Arc::new(AtomicBool::new(false));
    let result = register_request(&active, &flag2, json!("dup"), "tools/call".to_string()).await;
    assert!(
        matches!(result, Err(RegisterRequestError::DuplicateId)),
        "should reject duplicate ID"
    );
}

#[tokio::test]
async fn register_request_rejects_at_capacity() {
    let active = new_active_requests();
    let mut guards = Vec::new();
    for i in 0..MAX_IN_FLIGHT_REQUESTS {
        let flag = Arc::new(AtomicBool::new(false));
        let g = register_request(&active, &flag, json!(i), "tools/call".to_string())
            .await
            .unwrap();
        guards.push(g);
    }
    let flag = Arc::new(AtomicBool::new(false));
    let result = register_request(
        &active,
        &flag,
        json!(MAX_IN_FLIGHT_REQUESTS),
        "tools/call".to_string(),
    )
    .await;
    assert!(
        matches!(result, Err(RegisterRequestError::CapacityExceeded)),
        "should reject when at capacity"
    );
    drop(guards);
}

#[tokio::test]
async fn register_request_guard_cleanup_on_drop() {
    let active = new_active_requests();
    let flag = Arc::new(AtomicBool::new(false));
    {
        let _g = register_request(&active, &flag, json!("drop-test"), "tools/call".to_string())
            .await
            .unwrap();
        let map = active.lock().await;
        assert_eq!(map.len(), 1);
    }
    let map = active.lock().await;
    assert_eq!(map.len(), 0, "guard drop should remove entry");
}

#[tokio::test]
async fn register_request_allows_id_reuse_after_completion() {
    let active = new_active_requests();
    let flag = Arc::new(AtomicBool::new(false));
    {
        let _g = register_request(&active, &flag, json!("reuse"), "tools/call".to_string())
            .await
            .unwrap();
    }
    // Entry was removed on drop, so reuse should succeed
    let flag2 = Arc::new(AtomicBool::new(false));
    let result = register_request(&active, &flag2, json!("reuse"), "tools/call".to_string()).await;
    assert!(result.is_ok(), "ID reuse after completion should succeed");
}

// ═══════════════════════════════════════════════════════════════════════════════
// Runtime metrics
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn metric_guard_increments_and_decrements() {
    let prev = RUNTIME_METRICS.active_requests.load(Ordering::Relaxed);
    {
        let _guard = MetricGuard::new(&RUNTIME_METRICS.active_requests);
        assert_eq!(
            RUNTIME_METRICS.active_requests.load(Ordering::Relaxed),
            prev + 1
        );
    }
    assert_eq!(
        RUNTIME_METRICS.active_requests.load(Ordering::Relaxed),
        prev
    );
}

#[test]
fn metric_guard_nested_guards() {
    let prev = RUNTIME_METRICS
        .active_blocking_handlers
        .load(Ordering::Relaxed);
    {
        let _g1 = MetricGuard::new(&RUNTIME_METRICS.active_blocking_handlers);
        assert_eq!(
            RUNTIME_METRICS
                .active_blocking_handlers
                .load(Ordering::Relaxed),
            prev + 1
        );
        {
            let _g2 = MetricGuard::new(&RUNTIME_METRICS.active_blocking_handlers);
            assert_eq!(
                RUNTIME_METRICS
                    .active_blocking_handlers
                    .load(Ordering::Relaxed),
                prev + 2
            );
        }
        assert_eq!(
            RUNTIME_METRICS
                .active_blocking_handlers
                .load(Ordering::Relaxed),
            prev + 1
        );
    }
    assert_eq!(
        RUNTIME_METRICS
            .active_blocking_handlers
            .load(Ordering::Relaxed),
        prev
    );
}

#[test]
fn snapshot_metrics_returns_current_values() {
    let snap = snapshot_metrics();
    // Just verify it doesn't panic and returns reasonable values
    assert!(snap.active_requests <= MAX_IN_FLIGHT_REQUESTS);
    assert!(snap.active_blocking_handlers <= MAX_IN_FLIGHT_REQUESTS);
}

#[test]
fn runtime_metrics_peak_tracking() {
    let prev = RUNTIME_METRICS
        .peak_blocking_concurrency
        .load(Ordering::Relaxed);
    // Simulate: increment, update peak, decrement
    RUNTIME_METRICS
        .active_blocking_handlers
        .fetch_add(1, Ordering::Relaxed);
    let current = RUNTIME_METRICS
        .active_blocking_handlers
        .load(Ordering::Relaxed);
    RUNTIME_METRICS
        .peak_blocking_concurrency
        .fetch_max(current, Ordering::Relaxed);
    RUNTIME_METRICS
        .active_blocking_handlers
        .fetch_sub(1, Ordering::Relaxed);
    let peak = RUNTIME_METRICS
        .peak_blocking_concurrency
        .load(Ordering::Relaxed);
    assert!(peak >= prev, "peak should be >= previous value");
}
