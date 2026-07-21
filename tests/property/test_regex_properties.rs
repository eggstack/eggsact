use eggsact::text::{classify_pattern, regex_finditer, regex_safety_check};

#[test]
fn classify_pattern_deterministic() {
    let patterns = ["^[a-z]+$", "(?=foo)bar", "(\\w+) \\1", "\\d{3}-\\d{4}"];
    for p in &patterns {
        let c1 = classify_pattern(p);
        let c2 = classify_pattern(p);
        assert_eq!(
            format!("{:?}", c1.preferred_engine),
            format!("{:?}", c2.preferred_engine)
        );
        assert_eq!(c1.features.len(), c2.features.len());
    }
}

#[test]
fn regex_safety_check_deterministic() {
    let patterns = ["^[a-z]+$", "(.+)+$", "a{100000}"];
    for p in &patterns {
        let r1 = regex_safety_check(p);
        let r2 = regex_safety_check(p);
        assert_eq!(r1.valid_pattern, r2.valid_pattern);
        assert_eq!(r1.risk, r2.risk);
    }
}

#[test]
fn regex_finditer_spans_within_bounds() {
    let cases = [
        ("\\d+", "abc123def456"),
        ("[a-z]+", "Hello World"),
        ("\\w+", "test@example.com"),
    ];
    for (p, t) in &cases {
        let result = regex_finditer(p, t, None, 100, false, false);
        for m in &result.matches {
            assert!(m.span.len() == 2, "Span should have 2 elements");
            assert!(m.span[0] >= 0, "Match start out of bounds");
            assert!(m.span[1] >= m.span[0], "Match end before start");
        }
    }
}

#[test]
fn regex_finditer_deterministic() {
    let result1 = regex_finditer("\\d+", "abc123def456", None, 100, false, false);
    let result2 = regex_finditer("\\d+", "abc123def456", None, 100, false, false);
    assert_eq!(result1.matches.len(), result2.matches.len());
    for (m1, m2) in result1.matches.iter().zip(result2.matches.iter()) {
        assert_eq!(m1.span, m2.span);
    }
}

#[test]
fn regex_finditer_max_matches_respected() {
    let result = regex_finditer("\\d+", "1234567890", None, 3, false, false);
    assert!(result.matches.len() <= 3);
}
