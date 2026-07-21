use eggsact::text::{common_prefix_suffix, first_diff, levenshtein_distance, patch_summary};

#[test]
fn levenshtein_self_distance_zero() {
    let inputs = ["", "hello", "test string", "αβγ"];
    for s in &inputs {
        assert_eq!(levenshtein_distance(s, s), 0);
    }
}

#[test]
fn levenshtein_symmetric() {
    let pairs = [("abc", "def"), ("hello", "helo"), ("", "abc"), ("abc", "")];
    for (a, b) in &pairs {
        assert_eq!(levenshtein_distance(a, b), levenshtein_distance(b, a));
    }
}

#[test]
fn levenshtein_triangle_inequality() {
    let triples = [("abc", "def", "ghi"), ("hello", "world", "foo")];
    for (a, b, c) in &triples {
        let ab = levenshtein_distance(a, b);
        let bc = levenshtein_distance(b, c);
        let ac = levenshtein_distance(a, c);
        assert!(ac <= ab + bc, "Triangle inequality violated");
    }
}

#[test]
fn first_diff_deterministic() {
    let pairs = [("abc", "axc"), ("hello", "world"), ("same", "same")];
    for (a, b) in &pairs {
        let d1 = first_diff(a, b);
        let d2 = first_diff(a, b);
        assert_eq!(d1, d2);
    }
}

#[test]
fn first_diff_self_is_none() {
    let inputs = ["", "hello", "test"];
    for s in &inputs {
        assert!(first_diff(s, s).is_none());
    }
}

#[test]
fn common_prefix_suffix_consistency() {
    let pairs = [("abcde", "abcxy"), ("hello", "world"), ("", "abc")];
    for (a, b) in &pairs {
        let cps = common_prefix_suffix(a, b);
        assert!(cps.common_prefix_len <= a.len());
        assert!(cps.common_prefix_len <= b.len());
        assert!(cps.common_suffix_len <= a.len());
        assert!(cps.common_suffix_len <= b.len());
        assert!(cps.common_prefix_len + cps.common_suffix_len <= a.len());
    }
}

#[test]
fn patch_summary_deterministic() {
    let patches = [
        "--- a/f\n+++ b/f\n@@ -1 +1 @@\n-old\n+new",
        "",
        "--- a/f\n+++ b/f\n@@ -0,0 +1 @@\n+line",
    ];
    for p in &patches {
        let s1 = patch_summary(p);
        let s2 = patch_summary(p);
        assert_eq!(s1, s2);
    }
}
