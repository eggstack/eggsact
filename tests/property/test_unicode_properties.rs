use eggsact::text::unicode_tools::{find_invisibles, unicode_casefold};
use eggsact::text::{canonicalize_text, count_graphemes, has_confusables, unicode_policy_check};

#[test]
fn unicode_policy_deterministic() {
    let inputs = ["hello", "🌍", "\u{202e}test"];
    for input in &inputs {
        let r1 = unicode_policy_check(input, "permissive", None);
        let r2 = unicode_policy_check(input, "permissive", None);
        assert_eq!(r1, r2);
    }
}

#[test]
fn canonicalize_text_idempotent() {
    let inputs = ["hello", "café", "\u{0065}\u{0301}", "naïve"];
    for input in &inputs {
        let r1 = canonicalize_text(input, "nfc", false);
        let r2 = canonicalize_text(&r1.base.text, "nfc", false);
        assert_eq!(
            r1.base.text, r2.base.text,
            "NFC canonicalize not idempotent for: {:?}",
            input
        );
    }
}

#[test]
fn count_graphemes_within_bounds() {
    let inputs = ["", "hello", "é", "🌍", "👨‍👩‍👧‍👦", "\u{0301}\u{0302}\u{0303}"];
    for input in &inputs {
        let gc = count_graphemes(input);
        assert!(gc <= input.len(), "Grapheme count exceeds byte length");
    }
}

#[test]
fn casefold_preserves_content() {
    let inputs = ["Hello", "CAFÉ", "straße", "Ωμέγα"];
    for input in &inputs {
        let cf = unicode_casefold(input);
        assert!(!cf.is_empty());
        assert!(
            cf.len() >= input.len() / 2,
            "Casefold drastically reduced content"
        );
    }
}

#[test]
fn find_invisibles_bounded() {
    let inputs = ["", "hello", "\u{200b}test\u{200c}", "\u{202e}rtl"];
    for input in &inputs {
        let inv = find_invisibles(input);
        assert!(inv.len() <= input.len());
    }
}

#[test]
fn has_confusables_deterministic() {
    let inputs = ["hello", "α", "a", "Ελληνικά"];
    for input in &inputs {
        let h1 = has_confusables(input);
        let h2 = has_confusables(input);
        assert_eq!(h1, h2);
    }
}
