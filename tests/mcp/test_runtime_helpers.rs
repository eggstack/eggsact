//! Unit tests for MCP runtime helpers (Tasks 3 & 4).
//!
//! Tests cancellation logic, active request tracking, and audience
//! parsing in isolation without spawning the stdio server loop.

use eggsact::agent::ToolAudience;
use eggsact::mcp::runtime::{
    apply_cancellation, new_active_requests, parse_audience, parse_schema_detail, RateLimiter,
    MAX_IN_FLIGHT_REQUESTS, MAX_REQUESTS_PER_SECOND,
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

    let set = apply_cancellation(&active, &request_id);
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

    let set = apply_cancellation(&active, &request_id);
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

    let set = apply_cancellation(&active, &unknown_id);
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

    let set = apply_cancellation(&active, &json!(true));
    assert!(!set, "bool must be rejected");
    assert!(!flag.load(Ordering::Relaxed), "flag must remain unset");
}

#[tokio::test]
async fn apply_cancellation_object_id_is_ignored() {
    let active = new_active_requests();
    let set = apply_cancellation(&active, &json!({"id": 1}));
    assert!(!set, "object must be rejected");
}

#[tokio::test]
async fn apply_cancellation_array_id_is_ignored() {
    let active = new_active_requests();
    let set = apply_cancellation(&active, &json!([1, 2, 3]));
    assert!(!set, "array must be rejected");
}

#[tokio::test]
async fn apply_cancellation_null_id_is_ignored() {
    let active = new_active_requests();
    let set = apply_cancellation(&active, &json!(null));
    assert!(!set, "null must be rejected");
}

#[tokio::test]
async fn apply_cancellation_oversized_string_is_ignored() {
    let active = new_active_requests();
    let oversized = "x".repeat(2000);
    let set = apply_cancellation(&active, &json!(oversized));
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

    let _ = apply_cancellation(&active, &request_id);
    let _ = apply_cancellation(&active, &request_id);
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
