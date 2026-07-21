#![no_main]

//! Fuzz expression normalization/token-preprocessing.
//!
//! Properties: normalization is deterministic, idempotent where applicable,
//! output is valid UTF-8, output length bounded.

use libfuzzer_sys::fuzz_target;
use eggsact::calc::run;

const MAX_EXPR_LEN: usize = 10_000;

fuzz_target!(|data: &[u8]| {
    let Ok(expr) = std::str::from_utf8(data) else { return };
    if expr.len() > MAX_EXPR_LEN { return; }

    if let Ok(result1) = run(expr) {
        // Deterministic
        let result2 = run(expr).unwrap();
        assert_eq!(result1, result2);

        // Valid UTF-8 (guaranteed by &str return, but assert anyway)
        assert!(std::str::from_utf8(result1.0.as_bytes()).is_ok());

        // Idempotent: normalizing the normalized form should not change it
        if let Ok(result3) = run(&result1.0) {
            assert_eq!(result1.0, result3.0);
        }

        // Output bounded
        assert!(result1.0.len() <= expr.len() * 100 + 1000);
    }
});
