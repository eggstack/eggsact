use eggsact::text::primitives::byte_offset_to_char_index;
use eggsact::text::{count_graphemes, truncate_to_grapheme};

#[test]
fn test_count_graphemes_empty() {
    assert_eq!(count_graphemes(""), 0);
}

#[test]
fn test_count_graphemes_simple_ascii() {
    assert_eq!(count_graphemes("hello"), 5);
    assert_eq!(count_graphemes("Hello World"), 11);
}

#[test]
fn test_count_graphemes_with_spaces() {
    assert_eq!(count_graphemes("a b c"), 5);
    assert_eq!(count_graphemes("  "), 2);
}

#[test]
fn test_count_graphemes_emoji_single() {
    assert_eq!(count_graphemes("👋"), 1);
    assert_eq!(count_graphemes("🎉"), 1);
    assert_eq!(count_graphemes("🚀"), 1);
}

#[test]
fn test_count_graphemes_emoji_with_text() {
    assert_eq!(count_graphemes("hi 👋"), 4);
    assert_eq!(count_graphemes("hello 👨‍👩‍👧‍👦"), 7);
}

#[test]
fn test_count_graphemes_combining_chars() {
    let e_acute = "e\u{0301}";
    assert_eq!(count_graphemes(e_acute), 1);
    let combo = "a\u{0327}\u{0301}";
    assert_eq!(count_graphemes(combo), 1);
}

#[test]
fn test_truncate_to_grapheme_empty_string() {
    assert_eq!(truncate_to_grapheme("", 5), "");
    assert_eq!(truncate_to_grapheme("", 0), "");
}

#[test]
fn test_truncate_to_grapheme_zero_max() {
    assert_eq!(truncate_to_grapheme("hello", 0), "");
    assert_eq!(truncate_to_grapheme("👋👋👋", 0), "");
}

#[test]
fn test_truncate_to_grapheme_simple() {
    assert_eq!(truncate_to_grapheme("hello", 3), "hel");
    assert_eq!(truncate_to_grapheme("hello", 5), "hello");
    assert_eq!(truncate_to_grapheme("hello", 10), "hello");
}

#[test]
fn test_truncate_to_grapheme_emoji() {
    let text = "👋 hello";
    assert_eq!(truncate_to_grapheme(text, 1), "👋");
    assert_eq!(truncate_to_grapheme(text, 2), "👋 ");
    assert_eq!(truncate_to_grapheme(text, 3), "👋 h");
    assert_eq!(truncate_to_grapheme(text, 4), "👋 he");
    assert_eq!(truncate_to_grapheme(text, 5), "👋 hel");
    assert_eq!(truncate_to_grapheme(text, 6), "👋 hell");
    assert_eq!(truncate_to_grapheme(text, 7), "👋 hello");
}

#[test]
fn test_truncate_to_grapheme_emoji_whole() {
    let text = "👋👋👋";
    assert_eq!(truncate_to_grapheme(text, 1), "👋");
    assert_eq!(truncate_to_grapheme(text, 2), "👋👋");
    assert_eq!(truncate_to_grapheme(text, 3), "👋👋👋");
}

#[test]
fn test_truncate_to_grapheme_combining() {
    let e_acute = "e\u{0301}";
    assert_eq!(truncate_to_grapheme(e_acute, 1), e_acute);
    assert_eq!(truncate_to_grapheme(e_acute, 0), "");
}

#[test]
fn test_truncate_to_grapheme_mixed() {
    let text = "a\u{0301}b\u{0302}c";
    assert_eq!(truncate_to_grapheme(text, 1), "a\u{0301}");
    assert_eq!(truncate_to_grapheme(text, 2), "a\u{0301}b\u{0302}");
    assert_eq!(truncate_to_grapheme(text, 3), "a\u{0301}b\u{0302}c");
}

#[test]
fn test_byte_offset_to_char_index_out_of_bounds() {
    let result = byte_offset_to_char_index("hello", 100);
    assert!(
        result.is_err(),
        "Should return error for out-of-bounds offset"
    );
    let err = result.unwrap_err();
    assert!(
        err.contains("out of range"),
        "Error should mention out of range: {}",
        err
    );
}

#[test]
fn test_byte_offset_to_char_index_valid() {
    let result = byte_offset_to_char_index("hello", 3);
    assert_eq!(result.unwrap(), 3);
}

#[test]
fn test_byte_offset_to_char_index_at_end() {
    let result = byte_offset_to_char_index("hello", 5);
    assert_eq!(result.unwrap(), 5);
}
