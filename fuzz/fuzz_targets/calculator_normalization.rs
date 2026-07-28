#![no_main]

//! Fuzz expression normalization/token-preprocessing.
//!
//! Properties: normalization is deterministic, idempotent where applicable,
//! output is valid UTF-8, output length bounded.
//!
//! The target invokes `run()` twice under `catch_unwind` to distinguish:
//! - production panic (first or second call);
//! - deterministic `Ok`;
//! - deterministic `Err`;
//! - `Ok` then `Err` / `Err` then `Ok` (non-determinism);
//! - differing successful values or error messages.

use libfuzzer_sys::fuzz_target;
use eggsact::calc::run;

const MAX_EXPR_LEN: usize = 10_000;

fuzz_target!(|data: &[u8]| {
    let Ok(expr) = std::str::from_utf8(data) else { return };
    if expr.len() > MAX_EXPR_LEN { return; }

    let first = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(expr)));
    let second = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(expr)));

    assert!(first.is_ok(), "first run panicked");
    assert!(second.is_ok(), "second run panicked");

    let first = first.unwrap();
    let second = second.unwrap();

    match (first, second) {
        (Ok(a), Ok(b)) => {
            assert_eq!(a, b);
            // Valid UTF-8 (guaranteed by &str return, but assert anyway)
            assert!(std::str::from_utf8(a.0.as_bytes()).is_ok());
            // Output bounded: small inputs can produce large numeric results
            // (e.g. 946! has ~2400 digits), so use a generous bound that still
            // catches pathological expansion while allowing legitimate large outputs.
            assert!(a.0.len() <= expr.len() * 1000 + 10_000);
        }
        (Err(a), Err(b)) => {
            assert_eq!(std::mem::discriminant(&a), std::mem::discriminant(&b));
            assert_eq!(a.to_string(), b.to_string());
        }
        (left, right) => {
            panic!("non-deterministic calculator outcome: {left:?} vs {right:?}");
        }
    }
});
