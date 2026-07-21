use eggsact::text::glob::glob_match;
use eggsact::text::path::{path_analyze, path_normalize};

#[test]
fn glob_match_deterministic() {
    let cases = [
        ("*.rs", "src/main.rs"),
        ("**/*.txt", "a/b/c.txt"),
        ("?", "a"),
        ("[abc].txt", "a.txt"),
    ];
    for (pattern, path) in &cases {
        let m1 = glob_match(pattern, path, "posix", true);
        let m2 = glob_match(pattern, path, "posix", true);
        assert_eq!(m1.matches, m2.matches);
    }
}

#[test]
fn path_analyze_deterministic() {
    let paths = ["/usr/local/bin", "src/main.rs", "../foo", "."];
    for path in &paths {
        let a1 = path_analyze(path, "posix");
        let a2 = path_analyze(path, "posix");
        assert_eq!(a1.absolute, a2.absolute);
        assert_eq!(a1.components, a2.components);
    }
}

#[test]
fn path_normalize_idempotent() {
    let paths = ["/usr//local/./bin", "src/../src/main.rs", "a/b/../c"];
    for path in &paths {
        let r1 = path_normalize(path, "posix", true, false);
        let r2 = path_normalize(&r1.normalized, "posix", true, false);
        assert_eq!(
            r1.normalized, r2.normalized,
            "Path normalization not idempotent for: {}",
            path
        );
    }
}

#[test]
fn path_normalize_preserves_content() {
    let paths = ["/usr/local", "src/main.rs", "a/b/c"];
    for path in &paths {
        let result = path_normalize(path, "posix", true, false);
        assert!(!result.normalized.is_empty());
        let filename = path.rsplit('/').next().unwrap_or(path);
        assert!(
            result.normalized.contains(filename),
            "Normalized path missing original filename '{}' in '{}'",
            filename,
            result.normalized
        );
    }
}
