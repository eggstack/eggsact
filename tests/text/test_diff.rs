use eggsact::text::levenshtein_distance;

#[test]
fn test_levenshtein_identical() {
    assert_eq!(levenshtein_distance("hello", "hello"), 0);
    assert_eq!(levenshtein_distance("", ""), 0);
}

#[test]
fn test_levenshtein_single_char_diff() {
    assert_eq!(levenshtein_distance("hello", "hallo"), 1);
    assert_eq!(levenshtein_distance("abc", "abd"), 1);
}

#[test]
fn test_levenshtein_insertion() {
    assert_eq!(levenshtein_distance("abc", "abcd"), 1);
    assert_eq!(levenshtein_distance("", "a"), 1);
}

#[test]
fn test_levenshtein_deletion() {
    assert_eq!(levenshtein_distance("abcd", "abc"), 1);
    assert_eq!(levenshtein_distance("a", ""), 1);
}

#[test]
fn test_levenshtein_substitution() {
    assert_eq!(levenshtein_distance("abc", "xyz"), 3);
}

#[test]
fn test_levenshtein_empty_strings() {
    assert_eq!(levenshtein_distance("", "hello"), 5);
    assert_eq!(levenshtein_distance("hello", ""), 5);
}

#[test]
fn test_levenshtein_complete_diff() {
    assert_eq!(levenshtein_distance("Saturday", "Sunday"), 3);
}

#[test]
fn test_levenshtein_large_input_limit() {
    let long_a = "a".repeat(20_000);
    let long_b = "b".repeat(20_000);
    let result = levenshtein_distance(&long_a, &long_b);
    assert!(result <= 20_000);
}
