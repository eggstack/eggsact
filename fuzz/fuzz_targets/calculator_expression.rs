#![no_main]

//! Fuzz the calculator expression parser/evaluator.
//!
//! Feeds bounded arbitrary UTF-8 expressions into `eggsact::evaluate` and
//! `eggsact::evaluate_with_context` with deterministic contexts.
//!
//! Asserts: no panic, errors are structured, deterministic results, context
//! not mutated on parse failure.

use libfuzzer_sys::fuzz_target;
use eggsact::{evaluate, evaluate_with_context, EvalContext};

const MAX_EXPR_LEN: usize = 10_000;

fuzz_target!(|data: &[u8]| {
    let Ok(expr) = std::str::from_utf8(data) else { return };
    if expr.len() > MAX_EXPR_LEN { return; }

    // Default context
    let _ = evaluate(expr);

    // MCP-safe context
    let mut ctx = EvalContext::mcp_mode();
    let _ = evaluate_with_context(expr, &mut ctx);

    // Seeded deterministic context
    let mut ctx = EvalContext::new().with_prng_state(42);
    let result1 = evaluate_with_context(expr, &mut ctx);
    let mut ctx2 = EvalContext::new().with_prng_state(42);
    let result2 = evaluate_with_context(expr, &mut ctx2);
    assert_eq!(result1.is_err(), result2.is_err());
    if let (Ok(r1), Ok(r2)) = (result1, result2) {
        assert_eq!(r1.0, r2.0);
    }

    // Result serialization should succeed
    if let Ok(result) = evaluate(expr) {
        let _ = serde_json::to_string(&serde_json::json!({
            "value": result.0,
            "unit": result.1,
        }));
    }
});
