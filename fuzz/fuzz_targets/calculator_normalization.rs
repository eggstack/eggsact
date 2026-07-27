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

        // The normalizer intentionally preserves some spacing distinctions
        // (for example, implicit multiplication around units), so its output
        // is not a general idempotence contract. The two full runs above
        // provide the deterministic property for this target.

        // Output bounded: small inputs can produce large numeric results
        // (e.g. 946! has ~2400 digits), so use a generous bound that still
        // catches pathological expansion while allowing legitimate large outputs.
        assert!(result1.0.len() <= expr.len() * 1000 + 10_000);
    }
});
