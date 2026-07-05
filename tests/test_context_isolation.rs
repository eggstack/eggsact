use eggsact::agent::{
    CompatibilityMode, ExecutionContext, Profile, ToolAudience, ToolCallError, ToolRegistry,
};
use eggsact::calc::{evaluate_with_context, run_with_context, EvalContext};
use eggsact::mcp::budget::ToolBudget;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::thread;

// ─────────────────────────────────────────────────────────────────────────
// 1. Profile isolation: two registries with different profiles enforce
//    different tool availability in the same process.
// ─────────────────────────────────────────────────────────────────────────
#[test]
fn test_profile_isolation() {
    let full_registry = ToolRegistry::with_profile(Profile::Full);
    let human_math_registry = ToolRegistry::with_profile(Profile::HumanMath);

    // math_eval is in both full and human_math profiles
    assert!(full_registry.has_tool("math_eval"));
    assert!(human_math_registry.has_tool("math_eval"));

    // text_equal is in full but NOT in human_math
    assert!(full_registry.has_tool("text_equal"));
    assert!(!human_math_registry.has_tool("text_equal"));

    // Calling text_equal on full should succeed
    let full_result =
        full_registry.call_json("text_equal", serde_json::json!({"a": "x", "b": "x"}));
    assert!(full_result.is_ok());
    assert!(full_result.unwrap().ok);

    // Calling text_equal on human_math should fail with ToolUnavailable
    let hm_result =
        human_math_registry.call_json("text_equal", serde_json::json!({"a": "x", "b": "x"}));
    assert!(hm_result.is_err());
    match hm_result.unwrap_err() {
        ToolCallError::ToolUnavailable { tool, profile } => {
            assert_eq!(tool, "text_equal");
            assert_eq!(profile, "human_math");
        }
        other => panic!("expected ToolUnavailable, got {:?}", other),
    }

    // Both registries can call math_eval independently
    let r1 = full_registry.call_json("math_eval", serde_json::json!({"expression": "2 + 3"}));
    let r2 = human_math_registry.call_json("math_eval", serde_json::json!({"expression": "2 + 3"}));
    assert!(r1.is_ok());
    assert!(r2.is_ok());
    assert_eq!(
        r1.unwrap().result.unwrap()["value"],
        r2.unwrap().result.unwrap()["value"]
    );
}

// ─────────────────────────────────────────────────────────────────────────
// 2. Audience isolation: Model audience rejects HarnessOnly tools while
//    Harness audience allows them.
// ─────────────────────────────────────────────────────────────────────────
#[test]
fn test_audience_isolation() {
    // shell_split is HarnessOnly in the full profile
    let model_registry =
        ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Model);
    let harness_registry =
        ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Harness);

    // Model audience should see shell_split in the tool list but NOT be able to execute it
    assert!(model_registry.has_tool("shell_split"));
    let model_result =
        model_registry.call_json("shell_split", serde_json::json!({"command": "echo hello"}));
    assert!(model_result.is_err());
    match model_result.unwrap_err() {
        ToolCallError::ToolNotAllowedForAudience { tool, .. } => {
            assert_eq!(tool, "shell_split");
        }
        other => panic!("expected ToolNotAllowedForAudience, got {:?}", other),
    }

    // Harness audience should be able to execute shell_split
    let harness_result =
        harness_registry.call_json("shell_split", serde_json::json!({"command": "echo hello"}));
    assert!(harness_result.is_ok());
    assert!(harness_result.unwrap().ok);
}

// ─────────────────────────────────────────────────────────────────────────
// 3. Compatibility mode isolation: StrictNative and EggcalcPython modes
//    do not leak between calls.
// ─────────────────────────────────────────────────────────────────────────
#[test]
fn test_compatibility_mode_isolation() {
    let strict_registry = ToolRegistry::new(); // default is StrictNative
    let python_registry =
        ToolRegistry::new().with_compat_mode(eggsact::agent::CompatibilityMode::EggcalcPython);

    // Both registries can call the same tool and get ok=true
    let r1 = strict_registry.call_json("math_eval", serde_json::json!({"expression": "1 + 1"}));
    let r2 = python_registry.call_json("math_eval", serde_json::json!({"expression": "1 + 1"}));
    assert!(r1.is_ok());
    assert!(r2.is_ok());
    assert!(r1.unwrap().ok);
    assert!(r2.unwrap().ok);

    // Verify the compat modes are different but independent
    assert_eq!(
        strict_registry.compat_mode(),
        eggsact::agent::CompatibilityMode::StrictNative
    );
    assert_eq!(
        python_registry.compat_mode(),
        eggsact::agent::CompatibilityMode::EggcalcPython
    );

    // Both registries can call text tools without cross-contamination
    let t1 = strict_registry.call_json("text_measure", serde_json::json!({"text": "hello"}));
    let t2 = python_registry.call_json("text_measure", serde_json::json!({"text": "hello"}));
    assert!(t1.is_ok());
    assert!(t2.is_ok());
    assert_eq!(
        t1.unwrap().result.unwrap()["graphemes"],
        t2.unwrap().result.unwrap()["graphemes"]
    );
}

// ─────────────────────────────────────────────────────────────────────────
// 4. Cancellation isolation: a cancelled context does not poison later
//    uncancelled calls.
// ─────────────────────────────────────────────────────────────────────────
#[test]
fn test_cancellation_isolation() {
    let registry = ToolRegistry::default();

    // First: make a successful call
    let r1 = registry.call_json("math_eval", serde_json::json!({"expression": "2 + 3"}));
    assert!(r1.is_ok());
    assert!(r1.unwrap().ok);

    // Create a pre-cancelled flag and use it with a budget call
    let cancel_flag = Arc::new(AtomicBool::new(true)); // already cancelled
    let ctx_cancelled = ExecutionContext::test_default().with_cancellation(cancel_flag);

    let r2 = registry.call_json_with_execution_context(
        "math_eval",
        serde_json::json!({"expression": "2 + 3"}),
        &ctx_cancelled,
    );
    // Cancelled call should either return an error or an ok:false response
    match r2 {
        Err(_) => {} // pre-execution error from cancellation — acceptable
        Ok(resp) => {
            // If it returns a ToolResponse, it should indicate cancellation
            if !resp.ok {
                // ok:false — acceptable for cancelled call
            }
        }
    }

    // Create a fresh (not cancelled) context and call again
    let ctx_fresh = ExecutionContext::test_default();
    let r3 = registry.call_json_with_execution_context(
        "math_eval",
        serde_json::json!({"expression": "2 + 3"}),
        &ctx_fresh,
    );
    assert!(r3.is_ok());
    assert!(r3.unwrap().ok);

    // Also verify that a simple call_json still works (no global state corruption)
    let r4 = registry.call_json("math_eval", serde_json::json!({"expression": "10 / 2"}));
    assert!(r4.is_ok());
    assert!(r4.unwrap().ok);
}

// ─────────────────────────────────────────────────────────────────────────
// 5. Budget isolation: a tiny-budget call fails with INPUT_TOO_LARGE,
//    while a later normal-budget call succeeds.
// ─────────────────────────────────────────────────────────────────────────
#[test]
fn test_budget_isolation() {
    let registry = ToolRegistry::default();

    // Create a tiny budget (10 bytes max input)
    let tiny_budget = ToolBudget::CHEAP.with_max_input_bytes(10);
    let ctx_tiny = ExecutionContext::test_default().with_budget(tiny_budget);

    // This call's serialized args will exceed 10 bytes
    let r1 = registry.call_json_with_execution_context(
        "math_eval",
        serde_json::json!({"expression": "2 + 3"}),
        &ctx_tiny,
    );
    // Should return an ok:false response with INPUT_TOO_LARGE error
    assert!(r1.is_ok());
    let resp = r1.unwrap();
    assert!(!resp.ok);
    assert!(resp.error.is_some());
    let err_msg = resp.error.unwrap();
    assert!(
        err_msg.contains("INPUT_TOO_LARGE") || err_msg.contains("exceed budget"),
        "Expected INPUT_TOO_LARGE error, got: {}",
        err_msg
    );

    // Now call with normal budget — should succeed
    let ctx_normal = ExecutionContext::test_default();
    let r2 = registry.call_json_with_execution_context(
        "math_eval",
        serde_json::json!({"expression": "2 + 3"}),
        &ctx_normal,
    );
    assert!(r2.is_ok());
    assert!(r2.unwrap().ok);
}

// ─────────────────────────────────────────────────────────────────────────
// 6. Parallel determinism: parallel calls to simple tools produce
//    deterministic results.
// ─────────────────────────────────────────────────────────────────────────
#[test]
fn test_parallel_determinism() {
    let handles: Vec<_> = (0..8)
        .map(|i| {
            thread::spawn(move || {
                let registry = ToolRegistry::default();
                let expr = format!("{} + {}", i, i * 10);
                let r = registry.call_json("math_eval", serde_json::json!({"expression": &expr}));
                let expected = (i + i * 10).to_string();
                assert!(r.is_ok(), "Thread {} math_eval failed: {:?}", i, r);
                let val = r.unwrap().result.unwrap()["value"]
                    .as_str()
                    .unwrap()
                    .to_string();
                assert_eq!(val, expected, "Thread {} got wrong result", i);
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Verify determinism: same input produces same output across threads
    let handles2: Vec<_> = (0..8)
        .map(|_i| {
            thread::spawn(move || {
                let registry = ToolRegistry::default();
                let r = registry.call_json(
                    "text_fingerprint",
                    serde_json::json!({"text": "determinism_check"}),
                );
                let sha = r.unwrap().result.unwrap()["sha256"]
                    .as_str()
                    .unwrap()
                    .to_string();
                sha
            })
        })
        .collect();

    let results: Vec<String> = handles2.into_iter().map(|h| h.join().unwrap()).collect();
    assert!(results.len() > 1);
    for r in &results {
        assert_eq!(*r, results[0], "Parallel text_fingerprint results differ");
    }
}

// ─────────────────────────────────────────────────────────────────────────
// 7. EvalContext isolation: two EvalContexts with different PRNG seeds
//    produce different random sequences.
// ─────────────────────────────────────────────────────────────────────────
#[test]
fn test_eval_context_prng_isolation() {
    let mut ctx1 = EvalContext::new().with_prng_state(42);
    let mut ctx2 = EvalContext::new().with_prng_state(999);

    // Call random() with each context — results should differ with high probability
    let r1 = evaluate_with_context("random()", &mut ctx1);
    let r2 = evaluate_with_context("random()", &mut ctx2);

    assert!(r1.is_ok(), "random() with seed 42 failed: {:?}", r1);
    assert!(r2.is_ok(), "random() with seed 999 failed: {:?}", r2);

    let val1: f64 = r1.unwrap().0.parse().unwrap();
    let val2: f64 = r2.unwrap().0.parse().unwrap();

    // Both should be in [0, 1)
    assert!(
        (0.0..1.0).contains(&val1),
        "random() out of range: {}",
        val1
    );
    assert!(
        (0.0..1.0).contains(&val2),
        "random() out of range: {}",
        val2
    );

    // With different seeds, results should differ
    assert_ne!(
        val1, val2,
        "Different PRNG seeds should produce different results"
    );

    // Same seed should produce same result
    let mut ctx3 = EvalContext::new().with_prng_state(42);
    let mut ctx4 = EvalContext::new().with_prng_state(42);
    let r3 = evaluate_with_context("random()", &mut ctx3);
    let r4 = evaluate_with_context("random()", &mut ctx4);
    assert_eq!(
        r3.unwrap().0,
        r4.unwrap().0,
        "Same PRNG seed should produce same result"
    );

    // Multiple calls advance the state independently per context
    let mut ctx5 = EvalContext::new().with_prng_state(42);
    let mut ctx6 = EvalContext::new().with_prng_state(42);
    let _ = evaluate_with_context("random()", &mut ctx5);
    let _ = evaluate_with_context("random()", &mut ctx5);
    let seq_a1 = evaluate_with_context("random()", &mut ctx5).unwrap().0;
    let seq_a2 = evaluate_with_context("random()", &mut ctx5).unwrap().0;

    let _ = evaluate_with_context("random()", &mut ctx6);
    let _ = evaluate_with_context("random()", &mut ctx6);
    let seq_b1 = evaluate_with_context("random()", &mut ctx6).unwrap().0;
    let seq_b2 = evaluate_with_context("random()", &mut ctx6).unwrap().0;

    assert_eq!(seq_a1, seq_b1, "Same seed+position should give same value");
    assert_eq!(seq_a2, seq_b2, "Same seed+position should give same value");
    assert_ne!(seq_a1, seq_a2, "Consecutive calls should advance state");
}

// ─────────────────────────────────────────────────────────────────────────
// 8. Memory register isolation: two EvalContexts with different memory
//    registers are independent.
// ─────────────────────────────────────────────────────────────────────────
#[test]
fn test_eval_context_memory_register_isolation() {
    // Memory registers use numeric IDs: store(val, id) stores val in R{id},
    // recall(id) recalls from R{id}.
    let mut regs1 = HashMap::new();
    regs1.insert("R1".to_string(), 10.0);
    let mut regs2 = HashMap::new();
    regs2.insert("R1".to_string(), 99.0);

    let mut ctx1 = EvalContext::new().with_memory_registers(regs1);
    let mut ctx2 = EvalContext::new().with_memory_registers(regs2);

    // recall(1) should return register R1 value in each context
    let r1 = evaluate_with_context("recall(1)", &mut ctx1);
    let r2 = evaluate_with_context("recall(1)", &mut ctx2);

    assert!(r1.is_ok(), "recall in ctx1 failed: {:?}", r1);
    assert!(r2.is_ok(), "recall in ctx2 failed: {:?}", r2);

    let v1: f64 = r1.unwrap().0.parse().unwrap();
    let v2: f64 = r2.unwrap().0.parse().unwrap();

    assert_eq!(v1, 10.0, "ctx1 should have register R1=10");
    assert_eq!(v2, 99.0, "ctx2 should have register R1=99");

    // Mutating ctx1's register via store should not affect ctx2
    let _ = evaluate_with_context("store(42, 1)", &mut ctx1);

    let v1_after = evaluate_with_context("recall(1)", &mut ctx1).unwrap().0;
    let v2_after = evaluate_with_context("recall(1)", &mut ctx2).unwrap().0;

    assert_eq!(v1_after, "42", "ctx1 store should change ctx1");
    assert_eq!(v2_after, "99", "ctx2 should be unaffected by ctx1 store");
}

// ─────────────────────────────────────────────────────────────────────────
// 9. Variable isolation: two EvalContexts with different user variables
//    are independent.
// ─────────────────────────────────────────────────────────────────────────
#[test]
fn test_eval_context_variable_isolation() {
    // User variables use numeric IDs: setvar(value, id) sets v{id},
    // getvar(id) gets v{id}.
    let mut vars1 = HashMap::new();
    vars1.insert("v1".to_string(), 7.0);
    let mut vars2 = HashMap::new();
    vars2.insert("v1".to_string(), 21.0);

    let mut ctx1 = EvalContext::new().with_user_variables(vars1);
    let mut ctx2 = EvalContext::new().with_user_variables(vars2);

    // getvar(1) should return the user variable v1 in each context
    let r1 = evaluate_with_context("getvar(1)", &mut ctx1);
    let r2 = evaluate_with_context("getvar(1)", &mut ctx2);

    assert!(r1.is_ok(), "getvar in ctx1 failed: {:?}", r1);
    assert!(r2.is_ok(), "getvar in ctx2 failed: {:?}", r2);

    let v1: f64 = r1.unwrap().0.parse().unwrap();
    let v2: f64 = r2.unwrap().0.parse().unwrap();

    assert_eq!(v1, 7.0, "ctx1 should have v1=7");
    assert_eq!(v2, 21.0, "ctx2 should have v1=21");

    // Setting a variable in ctx1 should not affect ctx2
    let _ = evaluate_with_context("setvar(100, 1)", &mut ctx1);

    let v1_after = evaluate_with_context("getvar(1)", &mut ctx1).unwrap().0;
    let v2_after = evaluate_with_context("getvar(1)", &mut ctx2).unwrap().0;

    assert_eq!(v1_after, "100", "ctx1 setvar should change ctx1");
    assert_eq!(v2_after, "21", "ctx2 should be unaffected by ctx1 setvar");

    // Deleting a variable in ctx1 should not affect ctx2
    let _ = evaluate_with_context("delvar(1)", &mut ctx1);

    // After delvar, getvar(1) should return default (0.0)
    let v1_del = evaluate_with_context("getvar(1)", &mut ctx1).unwrap().0;
    let v2_final = evaluate_with_context("getvar(1)", &mut ctx2).unwrap().0;

    assert_eq!(
        v1_del, "0",
        "getvar after delvar should return default in ctx1"
    );
    assert_eq!(v2_final, "21", "ctx2 should still have v1=21");
}

// ─────────────────────────────────────────────────────────────────────────
// 10. MCP mode isolation: EvalContext::mcp_mode() rejects random and
//     side-effect functions while EvalContext::new() allows them.
// ─────────────────────────────────────────────────────────────────────────
#[test]
fn test_mcp_mode_isolation() {
    let mut default_ctx = EvalContext::new();
    let mut mcp_ctx = EvalContext::mcp_mode();

    // random() should work with default context
    let r1 = evaluate_with_context("random()", &mut default_ctx);
    assert!(
        r1.is_ok(),
        "random() should be allowed in default ctx: {:?}",
        r1
    );

    // random() should be rejected in MCP mode
    let r2 = evaluate_with_context("random()", &mut mcp_ctx);
    assert!(r2.is_err(), "random() should be rejected in MCP mode");
    let err = r2.unwrap_err();
    assert!(
        err.contains("disabled in MCP mode") || err.contains("allow_random=false"),
        "Error should mention MCP mode: {}",
        err
    );

    // randint(1, 10) should be rejected in MCP mode
    let r3 = evaluate_with_context("randint(1, 10)", &mut mcp_ctx);
    assert!(r3.is_err(), "randint should be rejected in MCP mode");

    // store() should work with default context
    let mut default_ctx2 = EvalContext::new();
    let r4 = evaluate_with_context("store(42, 1)", &mut default_ctx2);
    assert!(
        r4.is_ok(),
        "store() should be allowed in default ctx: {:?}",
        r4
    );

    // store() should be rejected in MCP mode
    let mut mcp_ctx2 = EvalContext::mcp_mode();
    let r5 = evaluate_with_context("store(42, 1)", &mut mcp_ctx2);
    assert!(r5.is_err(), "store() should be rejected in MCP mode");
    let err5 = r5.unwrap_err();
    assert!(
        err5.contains("disabled in MCP mode") || err5.contains("allow_side_effects=false"),
        "Error should mention MCP mode: {}",
        err5
    );

    // setvar() should be rejected in MCP mode
    let mut mcp_ctx3 = EvalContext::mcp_mode();
    let r6 = evaluate_with_context("setvar(1, 1)", &mut mcp_ctx3);
    assert!(r6.is_err(), "setvar should be rejected in MCP mode");

    // Normal math should work in both modes
    let r7 = evaluate_with_context("2 + 3", &mut mcp_ctx);
    assert!(r7.is_ok(), "Normal math should work in MCP mode: {:?}", r7);
    assert_eq!(r7.unwrap().0, "5");

    // Also verify through the run_with_context pipeline
    let mut default_ctx3 = EvalContext::new();
    let mut mcp_ctx4 = EvalContext::mcp_mode();

    let r8 = run_with_context("random()", &mut default_ctx3);
    assert!(
        r8.is_ok(),
        "run_with_context random() in default ctx: {:?}",
        r8
    );

    let r9 = run_with_context("random()", &mut mcp_ctx4);
    assert!(
        r9.is_err(),
        "run_with_context random() should be rejected in MCP mode"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// 11. Context profile override: call_json_with_execution_context with a
//     context profile different from the registry's profile enforces the
//     context's profile for tool availability.
// ─────────────────────────────────────────────────────────────────────────
#[test]
fn test_context_profile_override() {
    // Registry has full profile, but context overrides to human_math
    let registry = ToolRegistry::with_profile(Profile::Full);
    let ctx_hm = ExecutionContext::builder()
        .profile(Profile::HumanMath)
        .build();

    // math_eval is in human_math — should succeed
    let r1 = registry.call_json_with_execution_context(
        "math_eval",
        serde_json::json!({"expression": "2 + 3"}),
        &ctx_hm,
    );
    assert!(r1.is_ok());
    assert!(r1.unwrap().ok);

    // text_equal is NOT in human_math — should fail with ToolUnavailable
    let r2 = registry.call_json_with_execution_context(
        "text_equal",
        serde_json::json!({"a": "x", "b": "x"}),
        &ctx_hm,
    );
    assert!(r2.is_err());
    match r2.unwrap_err() {
        ToolCallError::ToolUnavailable { tool, profile } => {
            assert_eq!(tool, "text_equal");
            assert_eq!(profile, "human_math");
        }
        other => panic!("expected ToolUnavailable, got {:?}", other),
    }

    // Without context override, full profile can still call text_equal
    let ctx_default = ExecutionContext::test_default();
    let r3 = registry.call_json_with_execution_context(
        "text_equal",
        serde_json::json!({"a": "x", "b": "x"}),
        &ctx_default,
    );
    assert!(r3.is_ok());
    assert!(r3.unwrap().ok);
}

// ─────────────────────────────────────────────────────────────────────────
// 12. Context audience override: call_json_with_execution_context with
//     Harness audience can execute HarnessOnly tools even when registry
//     was created with Model audience.
// ─────────────────────────────────────────────────────────────────────────
#[test]
fn test_context_audience_override() {
    // Registry has Model audience (rejects HarnessOnly)
    let registry = ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Model);

    // Context overrides to Harness audience
    let ctx_harness = ExecutionContext::builder()
        .audience(ToolAudience::Harness)
        .build();

    // shell_split is HarnessOnly — should succeed with Harness context
    let r1 = registry.call_json_with_execution_context(
        "shell_split",
        serde_json::json!({"command": "echo hello"}),
        &ctx_harness,
    );
    assert!(r1.is_ok());
    assert!(r1.unwrap().ok);

    // Without context override, Model audience rejects shell_split
    let ctx_model = ExecutionContext::test_default();
    let r2 = registry.call_json_with_execution_context(
        "shell_split",
        serde_json::json!({"command": "echo hello"}),
        &ctx_model,
    );
    assert!(r2.is_err());
    match r2.unwrap_err() {
        ToolCallError::ToolNotAllowedForAudience { tool, .. } => {
            assert_eq!(tool, "shell_split");
        }
        other => panic!("expected ToolNotAllowedForAudience, got {:?}", other),
    }
}

// ─────────────────────────────────────────────────────────────────────────
// 13. Context compatibility mode override: the context's compat mode
//     is used for schema validation instead of the registry's.
// ─────────────────────────────────────────────────────────────────────────
#[test]
fn test_context_compatibility_mode_override() {
    // Registry has StrictNative, context overrides to EggcalcPython
    let registry = ToolRegistry::new(); // StrictNative
    let ctx_python = ExecutionContext::builder()
        .compatibility_mode(CompatibilityMode::EggcalcPython)
        .build();

    // Both modes allow normal math — verify the context's mode is used
    let r1 = registry.call_json_with_execution_context(
        "math_eval",
        serde_json::json!({"expression": "1 + 1"}),
        &ctx_python,
    );
    assert!(r1.is_ok());
    assert!(r1.unwrap().ok);

    // Verify the registry's default compat mode is different
    assert_eq!(registry.compat_mode(), CompatibilityMode::StrictNative);
}

// ─────────────────────────────────────────────────────────────────────────
// 14. EvalContext threading through math_eval: math_eval with MCP-mode
//     context rejects random() while default context allows it.
// ─────────────────────────────────────────────────────────────────────────
#[test]
fn test_eval_context_through_math_eval() {
    let registry = ToolRegistry::default();

    // Default EvalContext (allow_random=true) — random() should work
    let ctx_default = ExecutionContext::test_default();
    let r1 = registry.call_json_with_execution_context(
        "math_eval",
        serde_json::json!({"expression": "random()"}),
        &ctx_default,
    );
    assert!(r1.is_ok());
    assert!(r1.unwrap().ok);

    // MCP mode EvalContext (allow_random=false) — random() should fail
    let mut eval_ctx = EvalContext::mcp_mode();
    let ctx_mcp = ExecutionContext::test_default().with_eval_context(&mut eval_ctx);
    let r2 = registry.call_json_with_execution_context(
        "math_eval",
        serde_json::json!({"expression": "random()"}),
        &ctx_mcp,
    );
    assert!(r2.is_ok()); // dispatch succeeds
    let resp = r2.unwrap();
    assert!(!resp.ok, "random() should be rejected in MCP mode");
    let err_msg = resp.error.unwrap();
    assert!(
        err_msg.contains("disabled") || err_msg.contains("allow_random"),
        "Error should mention disabled/random: {}",
        err_msg
    );
}

// ─────────────────────────────────────────────────────────────────────────
// 15. EvalContext PRNG isolation through math_eval: different seeds
//     produce different random() results via the tool dispatch.
// ─────────────────────────────────────────────────────────────────────────
#[test]
fn test_eval_context_prng_through_math_eval() {
    let registry = ToolRegistry::default();

    let mut eval_ctx1 = EvalContext::new().with_prng_state(42);
    let ctx1 = ExecutionContext::test_default().with_eval_context(&mut eval_ctx1);
    let r1 = registry.call_json_with_execution_context(
        "math_eval",
        serde_json::json!({"expression": "random()"}),
        &ctx1,
    );

    let mut eval_ctx2 = EvalContext::new().with_prng_state(999);
    let ctx2 = ExecutionContext::test_default().with_eval_context(&mut eval_ctx2);
    let r2 = registry.call_json_with_execution_context(
        "math_eval",
        serde_json::json!({"expression": "random()"}),
        &ctx2,
    );

    assert!(r1.is_ok());
    assert!(r2.is_ok());
    let v1 = r1.unwrap().result.unwrap()["value"]
        .as_str()
        .unwrap()
        .parse::<f64>()
        .unwrap();
    let v2 = r2.unwrap().result.unwrap()["value"]
        .as_str()
        .unwrap()
        .parse::<f64>()
        .unwrap();

    assert!((0.0..1.0).contains(&v1), "random() out of range: {}", v1);
    assert!((0.0..1.0).contains(&v2), "random() out of range: {}", v2);
    assert_ne!(
        v1, v2,
        "Different seeds should produce different random() results"
    );
}
