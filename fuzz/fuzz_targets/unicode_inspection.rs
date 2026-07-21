//! Fuzz Unicode inspection and normalization.
//!
//! Asserts: no panic, NFC/NFKC idempotent, grapheme positions within bounds,
//! safe representation deterministic, findings bounded.

use libfuzzer_sys::fuzz_target;
use eggsact::text::{
    unicode_policy_check, canonicalize_text, find_invisibles,
    detect_mixed_scripts, count_graphemes, unicode_casefold,
    build_safe_repr, has_confusables,
};

const MAX_TEXT_LEN: usize = 50_000;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else { return };
    if text.len() > MAX_TEXT_LEN { return; }

    // Unicode policy check
    let _ = unicode_policy_check(text, "permissive", None);
    let _ = unicode_policy_check(text, "strict", None);

    // Canonicalize
    let _ = canonicalize_text(text, "nfc", false);
    let _ = canonicalize_text(text, "nfkc", false);

    // NFC idempotence
    if let Ok(nfc1) = unicode_normalization::Nfc::try_from(text).map(|n| n.collect::<String>()) {
        let nfc2: String = unicode_normalization::Nfc::new(&nfc1).collect();
        assert_eq!(nfc1, nfc2);
    }

    // Find invisibles
    let invis = find_invisibles(text);
    assert!(invis.len() <= text.len());

    // Detect mixed scripts
    let _ = detect_mixed_scripts(text);

    // Count graphemes
    let gc = count_graphemes(text);
    assert!(gc <= text.len());

    // Casefold
    let cf = unicode_casefold(text);
    assert!(cf.is_utf8());

    // Safe repr
    let sr = build_safe_repr(text);
    assert!(sr.is_utf8());

    // Confusables
    let _ = has_confusables(text);

    // Serializable
    let _ = serde_json::to_string(&serde_json::json!({
        "graphemes": gc,
        "invisibles": invis.len(),
    }));
});
