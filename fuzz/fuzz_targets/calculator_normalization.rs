//! Fuzz expression normalization/token-preprocessing.
//!
//! Properties: normalization is deterministic, idempotent where applicable,
//! output is valid UTF-8, output length bounded.

use libfuzzer_sys::fuzz_target;
use eggsact::calc::normalize::normalize;

const MAX_EXPR_LEN: usize = 10_000;

fuzz_target!(|data: &[u8]| {
    let Ok(expr) = std::str::from_utf8(data) else { return };
    if expr.len() > MAX_EXPR_LEN { return; }

    if let Ok(norm1) = normalize(expr) {
        // Deterministic
        let norm2 = normalize(expr).unwrap();
        assert_eq!(norm1, norm2);

        // Valid UTF-8 (guaranteed by &str return, but assert anyway)
        assert!(norm1.is_utf8());

        // Idempotent: normalizing the normalized form should not change it
        if let Ok(norm3) = normalize(&norm1) {
            assert_eq!(norm1, norm3);
        }

        // Output bounded
        assert!(norm1.len() <= expr.len() * 100 + 1000);
    }
});
