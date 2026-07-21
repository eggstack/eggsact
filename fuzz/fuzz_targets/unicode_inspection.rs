#![no_main]

//! Fuzz Unicode inspection and normalization.
//!
//! Asserts: no panic, NFC idempotent, grapheme positions within bounds,
//! safe representation deterministic, findings bounded.

use libfuzzer_sys::fuzz_target;
use eggsact::text::{
    unicode_policy_check, canonicalize_text, count_graphemes, has_confusables,
};
use eggsact::text::unicode_tools::{find_invisibles, unicode_casefold, build_safe_repr};

const MAX_TEXT_LEN: usize = 50_000;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else { return };
    if text.len() > MAX_TEXT_LEN { return; }

    // Unicode policy check
    let permissive = unicode_policy_check(text, "permissive", None);
    let strict = unicode_policy_check(text, "strict", None);
    // Bounded findings
    assert!(permissive.findings.len() <= 100);
    assert!(strict.findings.len() <= 100);

    // Deterministic
    let permissive2 = unicode_policy_check(text, "permissive", None);
    let strict2 = unicode_policy_check(text, "strict", None);
    let j1 = serde_json::to_value(&permissive).unwrap();
    let j2 = serde_json::to_value(&permissive2).unwrap();
    assert_eq!(j1, j2);
    let j1 = serde_json::to_value(&strict).unwrap();
    let j2 = serde_json::to_value(&strict2).unwrap();
    assert_eq!(j1, j2);

    // Canonicalize
    let _ = canonicalize_text(text, "nfc", false);
    let _ = canonicalize_text(text, "nfkc", false);

    // NFC idempotence
    let nfc1 = canonicalize_text(text, "nfc", false);
    let nfc2 = canonicalize_text(&nfc1.base.text, "nfc", false);
    assert_eq!(nfc1.base.text, nfc2.base.text);

    // Find invisibles
    let invis = find_invisibles(text);
    assert!(invis.len() <= text.len());

    // Count graphemes
    let gc = count_graphemes(text);
    assert!(gc <= text.len());

    // Casefold
    let cf = unicode_casefold(text);
    assert!(std::str::from_utf8(cf.as_bytes()).is_ok());

    // Safe repr
    let sr = build_safe_repr(text);
    assert!(std::str::from_utf8(sr.as_bytes()).is_ok());
    // Deterministic
    let sr2 = build_safe_repr(text);
    assert_eq!(sr, sr2);

    // Confusables
    let _ = has_confusables(text);

    // Serializable
    let _ = serde_json::json!({
        "graphemes": gc,
        "invisibles": invis.len(),
    });
});
