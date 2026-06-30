use eggsact::text::glob::glob_match;

// ─── glob_match ──────────────────────────────────────────────────────

#[test]
fn test_glob_match_exact() {
    let result = glob_match("hello.txt", "hello.txt", "posix", true);
    assert!(result.matches);
}

#[test]
fn test_glob_match_no_match() {
    let result = glob_match("hello.txt", "world.txt", "posix", true);
    assert!(!result.matches);
}

#[test]
fn test_glob_match_star() {
    let result = glob_match("*.txt", "hello.txt", "posix", true);
    assert!(result.matches);
}

#[test]
fn test_glob_match_star_no_match() {
    let result = glob_match("*.txt", "hello.rs", "posix", true);
    assert!(!result.matches);
}

#[test]
fn test_glob_match_question_mark() {
    let result = glob_match("?.txt", "a.txt", "posix", true);
    assert!(result.matches);
}

#[test]
fn test_glob_match_question_mark_multiple() {
    let result = glob_match("?.txt", "ab.txt", "posix", true);
    assert!(!result.matches);
}

#[test]
fn test_glob_match_double_star() {
    let result = glob_match("**/*.txt", "src/main.txt", "posix", true);
    assert!(result.matches);
}

#[test]
fn test_glob_match_double_star_deep() {
    let result = glob_match("**/*.txt", "a/b/c/d.txt", "posix", true);
    assert!(result.matches);
}

#[test]
fn test_glob_match_directory() {
    let result = glob_match("src/*", "src/main.rs", "posix", true);
    assert!(result.matches);
}

#[test]
fn test_glob_match_case_sensitive() {
    let result = glob_match("Hello.txt", "hello.txt", "posix", true);
    assert!(!result.matches);
}

#[test]
fn test_glob_match_case_insensitive() {
    let result = glob_match("Hello.txt", "hello.txt", "posix", false);
    assert!(result.matches);
}

#[test]
fn test_glob_match_double_star_case_insensitive() {
    let result = glob_match("**/*.TXT", "src/main.txt", "posix", false);
    assert!(result.matches);
}

#[test]
fn test_glob_match_multiple_double_stars_use_current_path_position() {
    let result = glob_match("**/x/y/**/z.txt", "a/b/x/y/c/z.txt", "posix", true);
    assert!(result.matches);
}

#[test]
fn test_glob_match_empty_pattern() {
    let result = glob_match("", "", "posix", true);
    assert!(result.matches);
}

#[test]
fn test_glob_match_empty_path() {
    let result = glob_match("*.txt", "", "posix", true);
    assert!(!result.matches);
}

#[test]
fn test_glob_match_bracket_expr() {
    let result = glob_match("[abc].txt", "b.txt", "posix", true);
    assert!(result.matches);
}

#[test]
fn test_glob_match_bracket_expr_no_match() {
    let result = glob_match("[abc].txt", "d.txt", "posix", true);
    assert!(!result.matches);
}

#[test]
fn test_glob_match_negated_bracket() {
    let result = glob_match("[!abc].txt", "d.txt", "posix", true);
    assert!(result.matches);
}

#[test]
fn test_glob_match_range() {
    let result = glob_match("[a-z].txt", "m.txt", "posix", true);
    assert!(result.matches);
}

#[test]
fn test_glob_match_invalid_range_does_not_panic() {
    let result = glob_match("[z-a].txt", "m.txt", "posix", true);
    assert!(!result.matches);
}

// ─── UNC path tests ─────────────────────────────────────────────────

#[test]
fn test_glob_match_unc_path_forward_slash() {
    // Forward-slash UNC paths work with posix-style splitting
    let result = glob_match(
        "//server/share/*.txt",
        "//server/share/file.txt",
        "posix",
        true,
    );
    assert!(result.matches);
}

#[test]
fn test_glob_match_unc_path_no_match() {
    let result = glob_match(
        "//server/share/*.txt",
        "//other/share/file.txt",
        "posix",
        true,
    );
    assert!(!result.matches);
}

#[test]
fn test_glob_match_unc_path_double_star() {
    let result = glob_match(
        "//server/share/**/*.txt",
        "//server/share/a/b/file.txt",
        "posix",
        true,
    );
    assert!(result.matches);
}

// ─── Regression: literal plus sign ───────────────────────────────────

#[test]
fn test_glob_match_literal_plus() {
    let result = glob_match("a+b", "a+b", "posix", true);
    assert!(result.matches);
    let result = glob_match("a+b", "aab", "posix", true);
    assert!(!result.matches);
    let result = glob_match("a+b", "ab", "posix", true);
    assert!(!result.matches);
}
