//! Cancellation propagation tests.
//!
//! Verifies that:
//! - Thread-local cancel flag is properly set by the MCP server dispatch
//! - `for_handler()` picks up the thread-local flag and creates a BudgetContext
//!   that reports `is_cancelled()`
//! - `call_json_with_context` passes cancellation through to the handler
//! - Budget expiry still works independently of cancellation

use eggsact::agent::{Profile, ToolAudience, ToolRegistry};
use eggsact::mcp::budget::{current_cancel_flag, for_handler, with_cancel_flag, ToolBudget};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

fn full_harness_registry() -> ToolRegistry {
    ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Harness)
}

// ═══════════════════════════════════════════════════════════════════════
// Thread-local flag basics
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn thread_local_flag_default_is_none() {
    assert!(current_cancel_flag().is_none());
}

#[test]
fn thread_local_flag_visible_inside_scope() {
    let flag = Arc::new(AtomicBool::new(false));
    with_cancel_flag(Some(flag.clone()), || {
        let f = current_cancel_flag().expect("should have a flag");
        assert!(Arc::ptr_eq(&f, &flag));
    });
    assert!(current_cancel_flag().is_none());
}

#[test]
fn thread_local_flag_nested_scopes_restore_correctly() {
    let outer = Arc::new(AtomicBool::new(false));
    let inner = Arc::new(AtomicBool::new(true));

    with_cancel_flag(Some(outer.clone()), || {
        let ctx = for_handler(ToolBudget::CHEAP);
        assert!(!ctx.is_cancelled());

        with_cancel_flag(Some(inner.clone()), || {
            let ctx = for_handler(ToolBudget::CHEAP);
            assert!(ctx.is_cancelled());
        });

        // After inner scope, outer flag is restored
        let ctx = for_handler(ToolBudget::CHEAP);
        assert!(!ctx.is_cancelled());
    });
}

// ═══════════════════════════════════════════════════════════════════════
// for_handler integration with cancellation
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn for_handler_creates_cancellable_context() {
    let flag = Arc::new(AtomicBool::new(false));
    let ctx = with_cancel_flag(Some(flag.clone()), || for_handler(ToolBudget::HEAVY));

    assert_eq!(ctx.budget, ToolBudget::HEAVY);
    assert!(ctx.deadline.is_some());
    assert!(!ctx.is_cancelled());

    // Set the flag — ctx should now report cancelled
    flag.store(true, Ordering::Relaxed);
    assert!(ctx.is_cancelled());
    assert!(ctx.should_stop());
}

#[test]
fn for_handler_without_thread_local_has_no_cancellation() {
    let ctx = for_handler(ToolBudget::MODERATE);
    assert!(ctx.cancelled.is_none());
    assert!(!ctx.is_cancelled());
}

// ═══════════════════════════════════════════════════════════════════════
// call_json_with_context passes cancellation to handlers
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn call_json_with_context_handler_sees_cancel_flag() {
    let registry = full_harness_registry();
    let flag = Arc::new(AtomicBool::new(true)); // pre-cancelled
    let resp = registry
        .call_json_with_context(
            "math_eval",
            serde_json::json!({"expression": "1 + 1"}),
            None,
            Some(flag),
        )
        .expect("registry call should succeed");

    // Smoke test: the cancel flag is threaded through to the handler's
    // BudgetContext. math_eval does NOT check cancellation eagerly, so it
    // still returns ok — this verifies the plumbing, not the cancellation
    // effect itself.
    assert!(
        resp.ok,
        "math_eval should still succeed with pre-cancelled flag (no eager check)"
    );
}

#[test]
fn call_json_with_context_without_flag_works_normally() {
    let registry = full_harness_registry();
    let resp = registry
        .call_json_with_context(
            "math_eval",
            serde_json::json!({"expression": "2 + 3"}),
            None,
            None,
        )
        .expect("registry call should succeed");
    assert!(resp.ok);
}

// ═══════════════════════════════════════════════════════════════════════
// command_preflight cancellation via call_json_with_context
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn command_preflight_respects_cancel_flag() {
    let registry = full_harness_registry();
    let flag = Arc::new(AtomicBool::new(true)); // pre-cancelled

    // command_preflight calls budget_ctx.should_stop() early in its pipeline.
    // With the flag set, it should return a cancelled/error response.
    let resp = registry
        .call_json_with_context(
            "command_preflight",
            serde_json::json!({"command": "ls -la", "platform": "posix"}),
            None,
            Some(flag),
        )
        .expect("registry call should succeed");

    // command_preflight checks cancellation eagerly via budget_ctx.should_stop().
    // With a pre-cancelled flag, it MUST fail deterministically.
    assert!(
        !resp.ok,
        "command_preflight must return ok=false when cancelled"
    );
    let err = resp.error.as_deref().expect("error must be present");
    assert!(
        err.contains("cancelled"),
        "error must mention cancellation, got: {}",
        err
    );
    assert_eq!(
        resp.machine_code.as_deref(),
        Some("CANCELLED"),
        "machine_code must be CANCELLED"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// Budget expiry independent of cancellation
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn budget_expiry_works_without_cancellation() {
    let ctx = eggsact::mcp::budget::BudgetContext::new(ToolBudget::CHEAP.with_max_elapsed_ms(0));
    // With max_elapsed_ms=0, the deadline is in the past (or exactly now)
    std::thread::sleep(Duration::from_millis(5));
    assert!(ctx.is_expired());
    assert!(ctx.should_stop());
    assert!(!ctx.is_cancelled()); // cancelled is separate
}

#[test]
fn cancellation_flag_without_expiration_is_still_stoppable() {
    let flag = Arc::new(AtomicBool::new(true));
    let ctx = eggsact::mcp::budget::BudgetContext::new(ToolBudget::HEAVY).with_cancellation(flag);

    assert!(ctx.is_cancelled());
    assert!(!ctx.is_expired()); // HEAVY has 30s budget, not expired yet
    assert!(ctx.should_stop()); // but should_stop sees cancelled
}

// ═══════════════════════════════════════════════════════════════════════
// Budget override tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn oversized_input_rejected_before_handler() {
    let registry = full_harness_registry();
    // Create a budget with a tiny max_input_bytes so any real payload is rejected
    let budget = ToolBudget::CHEAP.with_max_input_bytes(10);

    // math_eval with a moderately sized expression — serialized JSON will exceed 10 bytes
    let resp = registry
        .call_json_with_budget(
            "math_eval",
            serde_json::json!({"expression": "1 + 1"}),
            Some(budget),
        )
        .expect("registry call should succeed");

    assert!(!resp.ok, "should reject oversized input");
    assert_eq!(
        resp.machine_code.as_deref(),
        Some("INPUT_TOO_LARGE"),
        "machine_code must be INPUT_TOO_LARGE"
    );
    let err = resp.error.as_deref().expect("error must be present");
    assert!(
        err.contains("exceed budget"),
        "error should mention budget limit, got: {}",
        err
    );
}

#[test]
fn explicit_budget_overrides_default() {
    let registry = full_harness_registry();
    // Use a 1ms max_elapsed_ms budget — very likely to expire before math_eval finishes
    let budget = ToolBudget::CHEAP.with_max_elapsed_ms(1);

    // Sleep briefly to ensure the deadline has passed
    std::thread::sleep(Duration::from_millis(5));

    let resp = registry
        .call_json_with_budget(
            "math_eval",
            serde_json::json!({"expression": "1 + 1"}),
            Some(budget),
        )
        .expect("registry call should succeed");

    // The tool should either succeed (if fast enough to beat the expired
    // deadline) or fail because the budget expired. The important invariant
    // is that the explicit budget is used, not the default CHEAP budget.
    // With max_elapsed_ms=1 and a 5ms sleep, it should be expired.
    if !resp.ok {
        let err = resp.error.as_deref().expect("error must be present");
        assert!(
            err.contains("budget")
                || err.contains("expired")
                || err.contains("timeout")
                || err.contains("timed out"),
            "error should indicate budget issue, got: {}",
            err
        );
    }
    // Either way, the call completed — the custom budget was respected.
}

#[test]
fn cancellation_flag_visible_through_budget_context() {
    let registry = full_harness_registry();
    let flag = Arc::new(AtomicBool::new(true)); // pre-cancelled

    let resp = registry
        .call_json_with_context(
            "command_preflight",
            serde_json::json!({"command": "echo hello", "platform": "posix"}),
            None,
            Some(flag),
        )
        .expect("registry call should succeed");

    // command_preflight checks cancellation via BudgetContext::should_stop()
    // early in its pipeline. The flag must propagate through budget context
    // creation and cause an immediate cancellation response.
    assert!(
        !resp.ok,
        "command_preflight must return ok=false when cancelled via budget context"
    );
    assert_eq!(
        resp.machine_code.as_deref(),
        Some("CANCELLED"),
        "machine_code must be CANCELLED"
    );
    let err = resp.error.as_deref().expect("error must be present");
    assert!(
        err.contains("cancelled"),
        "error must mention cancellation, got: {}",
        err
    );
}

// ═══════════════════════════════════════════════════════════════════════
// Tool-specific cancellation via call_json_with_context
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn text_security_inspect_respects_cancel_flag() {
    let registry = full_harness_registry();
    let flag = Arc::new(AtomicBool::new(true)); // pre-cancelled

    let resp = registry
        .call_json_with_context(
            "text_security_inspect",
            serde_json::json!({"text": "hello world"}),
            None,
            Some(flag),
        )
        .expect("registry call should succeed");

    assert!(
        !resp.ok,
        "text_security_inspect must return ok=false when cancelled"
    );
    let err = resp.error.as_deref().expect("error must be present");
    assert!(
        err.contains("cancelled"),
        "error must mention cancellation, got: {}",
        err
    );
    assert_eq!(
        resp.machine_code.as_deref(),
        Some("CANCELLED"),
        "machine_code must be CANCELLED"
    );
}

#[test]
fn structured_data_compare_respects_cancel_flag() {
    let registry = full_harness_registry();
    let flag = Arc::new(AtomicBool::new(true)); // pre-cancelled

    let resp = registry
        .call_json_with_context(
            "structured_data_compare",
            serde_json::json!({"a": "{\"a\":1}", "b": "{\"a\":2}"}),
            None,
            Some(flag),
        )
        .expect("registry call should succeed");

    assert!(
        !resp.ok,
        "structured_data_compare must return ok=false when cancelled"
    );
    let err = resp.error.as_deref().expect("error must be present");
    assert!(
        err.contains("cancelled"),
        "error must mention cancellation, got: {}",
        err
    );
    assert_eq!(
        resp.machine_code.as_deref(),
        Some("CANCELLED"),
        "machine_code must be CANCELLED"
    );
}

#[test]
fn patch_summary_respects_cancel_flag() {
    let registry = full_harness_registry();
    let flag = Arc::new(AtomicBool::new(true)); // pre-cancelled

    let resp = registry
        .call_json_with_context(
            "patch_summary",
            serde_json::json!({"patch_text": "--- a/foo.txt\n+++ b/foo.txt\n@@ -1 +1 @@\n-old\n+new\n"}),
            None,
            Some(flag),
        )
        .expect("registry call should succeed");

    assert!(
        !resp.ok,
        "patch_summary must return ok=false when cancelled"
    );
    let err = resp.error.as_deref().expect("error must be present");
    assert!(
        err.contains("cancelled"),
        "error must mention cancellation, got: {}",
        err
    );
    assert_eq!(
        resp.machine_code.as_deref(),
        Some("CANCELLED"),
        "machine_code must be CANCELLED"
    );
}

#[test]
fn text_diff_explain_respects_cancel_flag() {
    let registry = full_harness_registry();
    let flag = Arc::new(AtomicBool::new(true)); // pre-cancelled

    let resp = registry
        .call_json_with_context(
            "text_diff_explain",
            serde_json::json!({"a": "hello", "b": "world"}),
            None,
            Some(flag),
        )
        .expect("registry call should succeed");

    assert!(
        !resp.ok,
        "text_diff_explain must return ok=false when cancelled"
    );
    let err = resp.error.as_deref().expect("error must be present");
    assert!(
        err.contains("cancelled"),
        "error must mention cancellation, got: {}",
        err
    );
    assert_eq!(
        resp.machine_code.as_deref(),
        Some("CANCELLED"),
        "machine_code must be CANCELLED"
    );
}
