use eggsact::text::unicode_tools::{
    build_safe_repr, confusables_count, detect_mixed_scripts, detect_newline_style,
    find_invisibles, is_invisible_char, is_known_invisible_char, unicode_casefold,
    unicode_name_char, unicode_scripts,
};

// ─── is_invisible_char ───────────────────────────────────────────────

#[test]
fn test_invisible_char_zero_width_space() {
    assert!(is_invisible_char('\u{200b}'));
}

#[test]
fn test_invisible_char_word_joiner() {
    assert!(is_invisible_char('\u{2060}'));
}

#[test]
fn test_invisible_char_non_joiner() {
    assert!(is_invisible_char('\u{200c}'));
}

#[test]
fn test_invisible_char_regular_char() {
    assert!(!is_invisible_char('a'));
    assert!(!is_invisible_char(' '));
    assert!(!is_invisible_char('\n'));
}

// ─── is_known_invisible_char ─────────────────────────────────────────

#[test]
fn test_known_invisible_zwj() {
    assert!(is_known_invisible_char('\u{200d}'));
}

#[test]
fn test_known_invisible_regular() {
    assert!(!is_known_invisible_char('a'));
}

// ─── unicode_casefold ────────────────────────────────────────────────

#[test]
fn test_casefold_ascii() {
    assert_eq!(unicode_casefold("Hello"), "hello");
}

#[test]
fn test_casefold_german_eszett() {
    let result = unicode_casefold("Straße");
    assert!(result.contains("ss"));
}

#[test]
fn test_casefold_upper() {
    assert_eq!(unicode_casefold("WORLD"), "world");
}

#[test]
fn test_casefold_empty() {
    assert_eq!(unicode_casefold(""), "");
}

// ─── unicode_name_char ───────────────────────────────────────────────

#[test]
fn test_unicode_name_ascii() {
    let name = unicode_name_char('A');
    assert!(!name.is_empty());
}

#[test]
fn test_unicode_name_digit() {
    let name = unicode_name_char('0');
    assert!(!name.is_empty());
}

#[test]
fn test_unicode_name_emoji() {
    let name = unicode_name_char('\u{1F44B}');
    assert!(!name.is_empty());
}

// ─── unicode_scripts ─────────────────────────────────────────────────

#[test]
fn test_scripts_ascii() {
    let scripts = unicode_scripts("hello");
    assert!(scripts.contains(&"Latin".to_string()));
}

#[test]
fn test_scripts_cyrillic() {
    let scripts = unicode_scripts("\u{043F}\u{0440}\u{0438}\u{0432}\u{0435}\u{0442}");
    assert!(scripts.contains(&"Cyrillic".to_string()));
}

#[test]
fn test_scripts_mixed() {
    let scripts = unicode_scripts("hello \u{043F}\u{0440}\u{0438}\u{0432}\u{0435}\u{0442}");
    assert!(scripts.len() > 1);
}

#[test]
fn test_scripts_empty() {
    let scripts = unicode_scripts("");
    assert!(scripts.is_empty());
}

// ─── find_invisibles ─────────────────────────────────────────────────

#[test]
fn test_find_invisibles_none() {
    let result = find_invisibles("hello");
    assert!(result.is_empty());
}

#[test]
fn test_find_invisibles_zero_width_space() {
    let result = find_invisibles("hello\u{200b}world");
    assert!(!result.is_empty());
}

#[test]
fn test_find_invisibles_multiple() {
    let result = find_invisibles("\u{200b}\u{200c}\u{200d}");
    assert_eq!(result.len(), 3);
}

#[test]
fn test_find_invisibles_empty() {
    let result = find_invisibles("");
    assert!(result.is_empty());
}

// ─── detect_newline_style ────────────────────────────────────────────

#[test]
fn test_newline_lf() {
    assert_eq!(detect_newline_style("line1\nline2"), "LF");
}

#[test]
fn test_newline_crlf() {
    assert_eq!(detect_newline_style("line1\r\nline2"), "CRLF");
}

#[test]
fn test_newline_cr() {
    assert_eq!(detect_newline_style("line1\rline2"), "CR");
}

#[test]
fn test_newline_none() {
    assert_eq!(detect_newline_style("no newlines"), "none");
}

#[test]
fn test_newline_empty() {
    assert_eq!(detect_newline_style(""), "none");
}

// ─── detect_mixed_scripts ────────────────────────────────────────────

#[test]
fn test_mixed_scripts_pure_latin() {
    let result = detect_mixed_scripts("hello");
    assert!(!result.mixed_scripts);
}

#[test]
fn test_mixed_scripts_mixed() {
    let result = detect_mixed_scripts("hello \u{043F}\u{0440}\u{0438}\u{0432}\u{0435}\u{0442}");
    assert!(result.mixed_scripts);
    assert!(result.scripts.contains(&"Latin".to_string()));
    assert!(result.scripts.contains(&"Cyrillic".to_string()));
}

// ─── build_safe_repr ─────────────────────────────────────────────────

#[test]
fn test_safe_repr_ascii() {
    let result = build_safe_repr("hello");
    assert!(result.contains("hello"));
}

#[test]
fn test_safe_repr_invisible() {
    let result = build_safe_repr("hello\u{200b}world");
    // Should escape the invisible character
    assert!(result != "hello\u{200b}world" || result.contains("\\"));
}

#[test]
fn test_safe_repr_newline() {
    let result = build_safe_repr("line1\nline2");
    // Should contain some representation of the newline
    assert!(!result.is_empty());
}

// ─── confusables_count ───────────────────────────────────────────────

#[test]
fn test_confusables_count_none() {
    assert_eq!(confusables_count("hello"), 0);
}

#[test]
fn test_confusables_count_one() {
    assert_eq!(confusables_count("\u{0430}"), 1);
}

#[test]
fn test_confusables_count_multiple() {
    assert_eq!(confusables_count("\u{0430}\u{03B2}\u{03B3}"), 3);
}

#[test]
fn test_confusables_count_empty() {
    assert_eq!(confusables_count(""), 0);
}

// ─── L13: U+2061 should be classified as FUNCTION APPLICATION, not BIDI ──

#[test]
fn test_find_invisibles_u2061_classified_as_bidi() {
    let result = find_invisibles("a\u{2061}b");
    assert!(!result.is_empty(), "U+2061 should be found as invisible");
    let info = result.iter().find(|i| i.codepoint == "U+2061").unwrap();
    assert_eq!(
        info.display, "FUNCTION APPLICATION",
        "U+2061 display should be 'FUNCTION APPLICATION', got '{}'",
        info.display
    );
}

// ─── L17: unicode_scripts returns "Other" for unknown chars ───────────

#[test]
fn test_unicode_scripts_returns_other_for_unknown_chars() {
    let scripts = unicode_scripts("\u{2603}");
    assert!(
        scripts.contains(&"Other".to_string()),
        "Snowman U+2603 should map to 'Other' script, got {:?}",
        scripts
    );
}
