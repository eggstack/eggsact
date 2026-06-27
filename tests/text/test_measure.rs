use eggsact::text::{char_frequency, line_count, text_length, word_count};

#[test]
fn test_text_length() {
    assert_eq!(text_length("hello"), 5);
    assert_eq!(text_length(""), 0);
    assert_eq!(text_length("a b c"), 5);
    assert_eq!(text_length("こんにちは"), 5); // 5 Japanese characters
}

#[test]
fn test_word_count() {
    assert_eq!(word_count("hello world"), 2);
    assert_eq!(word_count(""), 0);
    assert_eq!(word_count("   "), 0);
    assert_eq!(word_count("one two three four five"), 5);
    assert_eq!(word_count("hello\tworld\n"), 2);
}

#[test]
fn test_line_count() {
    assert_eq!(line_count("hello"), 1);
    assert_eq!(line_count(""), 0);
    assert_eq!(line_count("hello\nworld"), 2);
    assert_eq!(line_count("line1\nline2\nline3"), 3);
    assert_eq!(line_count("a\n\nb"), 3); // empty lines still count
}

#[test]
fn test_char_frequency() {
    let freq = char_frequency("hello");
    assert_eq!(freq.get(&'h'), Some(&1));
    assert_eq!(freq.get(&'e'), Some(&1));
    assert_eq!(freq.get(&'l'), Some(&2));
    assert_eq!(freq.get(&'o'), Some(&1));
    assert_eq!(freq.len(), 4); // 4 unique characters
}

#[test]
fn test_char_frequency_empty() {
    let freq = char_frequency("");
    assert_eq!(freq.len(), 0);
}

#[test]
fn test_char_frequency_repeats() {
    let freq = char_frequency("aaaa");
    assert_eq!(freq.get(&'a'), Some(&4));
    assert_eq!(freq.len(), 1);
}

#[test]
fn test_char_frequency_unicode() {
    let freq = char_frequency("héllo");
    assert_eq!(freq.get(&'h'), Some(&1));
    assert_eq!(freq.get(&'é'), Some(&1));
    assert_eq!(freq.get(&'l'), Some(&2));
    assert_eq!(freq.get(&'o'), Some(&1));
}
