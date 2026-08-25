use eggsact::text::{diff_spans, levenshtein_distance};

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

// PERF-102 regression: a skewed shape (a_len >> b_len) stays under the
// 4M-cell cap and must complete with flat DP storage instead of blowing up
// on per-row allocations.
#[test]
fn test_diff_spans_skewed_shape() {
    let long_a = format!("{}b", "a".repeat(2_000_000));
    let spans = diff_spans(&long_a, "b", 10);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].kind, "delete");
    assert_eq!((spans[0].a_start, spans[0].a_end), (0, 2_000_000));
    assert_eq!((spans[0].b_start, spans[0].b_end), (0, 0));

    let mirrored = diff_spans("b", &format!("{}b", "a".repeat(2_000_000)), 10);
    assert_eq!(mirrored.len(), 1);
    assert_eq!(mirrored[0].kind, "insert");
}
