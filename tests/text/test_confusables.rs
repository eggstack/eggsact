use eggsact::text::{find_confusables, has_confusables, CONFUSABLES};

#[test]
fn test_confusables_loaded() {
    assert_eq!(
        CONFUSABLES.len(),
        6565,
        "Confusables count should match confusables.txt (UTS #39)"
    );
}

#[test]
fn test_bug_008_confusable_entry() {
    // BUG-008: U+05AD (HEBREW ACCENT TIPEHA) should confusable-map to
    // U+0596 (HEBREW ACCENT ETNAHTA)
    assert_eq!(
        CONFUSABLES.get("U+05AD"),
        Some(&"U+0596"),
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
    assert_eq!(CONFUSABLES.get("U+0430"), Some(&"U+0061"));
    // Greek 'α' (U+03B1) maps to Latin 'a' (U+0061)
    assert_eq!(CONFUSABLES.get("U+03B1"), Some(&"U+0061"));
}

#[test]
fn test_confusables_multiple_chars_in_string() {
    // "аβγ" - all look like Latin letters
    let result = find_confusables("аβγ");
    assert_eq!(result.len(), 3);
}
