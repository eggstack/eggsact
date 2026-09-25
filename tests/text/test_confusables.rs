use eggsact::text::{find_confusables, has_confusables, lookup, CONFUSABLES};

#[test]
fn test_confusables_loaded() {
    assert_eq!(
        CONFUSABLES.len(),
        6712,
        "Confusables count should match confusables.txt (UTS #39, Unicode 18.0.0)"
    );
}

// ─── Unicode 18 data-epoch regressions ───────────────────────────────
// Material mapping changes introduced by the 17.0.0 → 18.0.0 refresh,
// exercised as regression fixtures (Milestone 004 work package C).

#[test]
fn test_unicode18_added_inverted_exclamation_maps_to_i() {
    // New in 18.0.0: U+00A1 (¡) → U+0069 (i).
    assert_eq!(lookup('¡'), Some("U+0069"));
}

#[test]
fn test_unicode18_added_masculine_ordinal_maps_to_degree() {
    // New in 18.0.0: U+00BA (º) → U+00B0 (°).
    assert_eq!(lookup('º'), Some("U+00B0"));
}

#[test]
fn test_unicode18_changed_percent_skeleton() {
    // Changed in 18.0.0: U+0025 (%) maps to U+00B0 (was U+00BA) U+002F U+2080.
    assert_eq!(lookup('%'), Some("U+00B0 U+002F U+2080"));
}

#[test]
fn test_unicode18_removed_cedilla_mapping_keeps_skeleton_equality() {
    // Removed in 18.0.0 as redundant under a normalizing skeleton:
    // U+00C7 (Ç) is no longer a table source, but NFD still equates it
    // with "C" + U+0327, so the skeleton collision verdict is unchanged.
    // This is the mapping-vs-skeleton distinction made explicit.
    assert_eq!(lookup('Ç'), None);
    assert!(!has_confusables("Ç"));
    assert_eq!(
        eggsact::text::confusable_skeleton("Ç"),
        eggsact::text::confusable_skeleton("Ç")
    );
}

#[test]
fn test_unicode18_core_spoof_mappings_stable() {
    // The Milestone 003 acceptance fixtures survive the data refresh.
    assert_eq!(lookup('А'), Some("U+0041"));
    assert_eq!(lookup('а'), Some("U+0061"));
    assert_eq!(lookup('Æ'), Some("U+0041 U+0045"));
}

#[test]
fn test_bug_008_confusable_entry() {
    // BUG-008: U+05AD (HEBREW ACCENT TIPEHA) should confusable-map to
    // U+0596 (HEBREW ACCENT ETNAHTA)
    assert_eq!(
        lookup('\u{05AD}'),
        Some("U+0596"),
        "BUG-008: U+05AD should map to U+0596"
    );
}

#[test]
fn test_has_confusables_true() {
    // Cyrillic 'а' (U+0430) looks like Latin 'a'
    assert!(has_confusables("а"));
    // Greek lowercase 'α' (U+03B1) looks like Latin 'a'
    assert!(has_confusables("α"));
}

#[test]
fn test_has_confusables_false() {
    assert!(!has_confusables("hello"));
    assert!(!has_confusables(""));
}

#[test]
fn test_find_confusables_cyrillic() {
    let confusables = find_confusables("а");
    assert!(!confusables.is_empty());
    let (char, replacement) = confusables[0];
    assert_eq!(char, 'а');
    // Should map to Latin 'A' (U+0041) or 'a' (U+0061)
    assert!(replacement == "U+0041" || replacement == "U+0061");
}

#[test]
fn test_find_confusables_greek() {
    let confusables = find_confusables("α");
    assert!(!confusables.is_empty());
}

#[test]
fn test_find_confusables_none() {
    let confusables = find_confusables("hello");
    assert!(confusables.is_empty());
}

#[test]
fn test_find_confusables_mixed() {
    // String with both confusable and non-confusable chars
    let confusables = find_confusables("aα");
    assert_eq!(confusables.len(), 1);
    assert_eq!(confusables[0].0, 'α');
}

#[test]
fn test_confusables_lookup() {
    // Cyrillic 'а' (U+0430) maps to Latin 'a' (U+0061)
    assert_eq!(lookup('а'), Some("U+0061"));
    // Greek 'α' (U+03B1) maps to Latin 'a' (U+0061)
    assert_eq!(lookup('α'), Some("U+0061"));
}

#[test]
fn test_confusables_multiple_chars_in_string() {
    // "аβγ" - all look like Latin letters
    let result = find_confusables("аβγ");
    assert_eq!(result.len(), 3);
}
