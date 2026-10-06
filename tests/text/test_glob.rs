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

// ─── Windows separator handling ─────────────────────────────────────

#[test]
fn test_glob_match_windows_backslash_pattern() {
    // A backslash is a separator on Windows, exactly like it is in the path.
    assert!(glob_match(r"src\*.rs", r"src\b.rs", "windows", true).matches);
    assert!(glob_match(r"src\**\*.rs", r"src\main.rs", "windows", true).matches);
    assert!(!glob_match(r"src\*.rs", r"src\b.txt", "windows", true).matches);
}

#[test]
fn test_glob_match_windows_pattern_against_itself() {
    let literal = r"C:\proj\b.rs";
    assert!(
        glob_match(literal, literal, "windows", true).matches,
        "a windows pattern must match the identical path string"
    );
}

#[test]
fn test_glob_match_windows_drive_letter_and_unc_prefixes() {
    assert!(glob_match(r"C:\proj\*", r"C:\proj\b.rs", "windows", true).matches);
    assert!(
        glob_match(
            r"\\server\share\*.txt",
            r"\\server\share\a.txt",
            "windows",
            true
        )
        .matches
    );
    assert!(
        !glob_match(
            r"\\server\share\*.txt",
            r"\\other\share\a.txt",
            "windows",
            true
        )
        .matches
    );
}

#[test]
fn test_glob_match_posix_backslash_is_still_a_literal() {
    // On posix `\` is not a separator; it must stay a literal, not split.
    assert!(glob_match(r"src\*.rs", r"src\a.rs", "posix", true).matches);
    assert!(!glob_match(r"src\*.rs", "src/a.rs", "posix", true).matches);
}

// ─── Recursion depth guard ──────────────────────────────────────────

#[test]
fn test_glob_match_deep_double_star_pattern_does_not_overflow() {
    // 3000 `**` segments (9 KB) recursed the matcher out of the stack and
    // aborted the process before the depth guard and run collapsing existed.
    // Adjacent `**` segments are redundant, so this is now one segment and
    // answers correctly instead of crashing.
    let pattern = format!("{}*", "**/".repeat(3_000));
    assert!(glob_match(&pattern, "a", "posix", true).matches);

    // Separated by real segments they cannot collapse, so the guard applies.
    let deep = "**/a/".repeat(5_000);
    let result = glob_match(&deep, "a", "posix", true);
    assert!(!result.matches);
    assert!(
        result.summary.contains("`**` segments"),
        "unexpected summary: {}",
        result.summary
    );
}

#[test]
fn test_glob_match_double_star_depth_limit_boundary() {
    let limit = eggsact::text::glob::MAX_DOUBLE_STAR_SEGMENTS;
    let unit = "**/a/";
    let at_limit = unit.repeat(limit);
    let over_limit = unit.repeat(limit + 1);

    // At the limit the pattern is still evaluated, and still correct.
    let path: String = std::iter::repeat_n("a", limit)
        .collect::<Vec<_>>()
        .join("/");
    let at = glob_match(&at_limit, &path, "posix", true);
    assert!(
        at.matches,
        "a pattern at the limit must still be evaluated correctly; got: {}",
        at.summary
    );

    // One past the limit is rejected rather than recursed.
    let over = glob_match(&over_limit, &path, "posix", true);
    assert!(!over.matches);
    assert!(
        over.summary.contains("`**` segments"),
        "unexpected summary: {}",
        over.summary
    );
    assert_eq!(
        eggsact::text::glob::double_star_segment_count(&at_limit, "posix"),
        limit
    );
    assert_eq!(
        eggsact::text::glob::double_star_segment_count(&over_limit, "posix"),
        limit + 1
    );
}

#[test]
fn test_glob_match_redundant_double_star_runs_are_collapsed() {
    // `**/**` matches exactly what `**` matches, so a long run of them is one
    // segment and stays under the limit.
    let pattern = "**/".repeat(3_000);
    assert_eq!(
        eggsact::text::glob::double_star_segment_count(&pattern, "posix"),
        1
    );
    assert!(glob_match(&format!("{}x", pattern), "a/b/x", "posix", true).matches);
}

#[test]
fn test_glob_match_interleaved_double_stars_still_match() {
    assert!(glob_match("**/x/**/y", "a/x/b/c/y", "posix", true).matches);
    assert!(!glob_match("**/x/**/y", "a/x/b/z", "posix", true).matches);
}

// ─── Backtracking bound ─────────────────────────────────────────────

#[test]
fn double_star_backtracking_is_bounded_and_reported() {
    // Each `**` tries every split point, so `**/*` repeated several times
    // against a deep path is super-polynomial. It was 36 s in release mode at
    // 48 segments and 58 s at the 100 KB input cap. The match must now finish
    // quickly and say the effort limit was reached — never answer a plain "no
    // match" for work it did not actually finish.
    let pattern = "**/*/**/*/**/*/**/*/z";
    let path = vec!["a"; 49_000].join("/");
    let result = glob_match(pattern, &path, "posix", true);

    assert!(!result.matches);
    assert!(
        result.summary.contains("effort"),
        "unexpected summary: {}",
        result.summary
    );
}

#[test]
fn deep_matching_globs_still_succeed_within_the_budget() {
    // The bound must not reject a pattern that genuinely matches.
    let deep = vec!["x"; 49_000].join("/");

    let hit = glob_match("**/*/**/*/**/*/**/*/*", &deep, "posix", true);
    assert!(hit.matches, "summary was: {}", hit.summary);

    let tail_hit = glob_match("**/z", &format!("{deep}/z"), "posix", true);
    assert!(tail_hit.matches, "summary was: {}", tail_hit.summary);
    assert_eq!(tail_hit.summary, "Pattern matches path");
}

#[test]
fn ordinary_globs_are_unaffected_by_the_matching_bound() {
    for (pattern, path, expected) in [
        ("**/*.rs", "a/b/c/d.rs", true),
        ("src/**/*.rs", "src/a/b/c/main.rs", true),
        ("src/**/*.rs", "src/a/b/c/main.py", false),
        ("**/target/z", "src/target/z", true),
        ("**/target/z", "src/src/src/z", false),
        ("**/target/z", "src/src/src", false),
        ("*.rs", "a/b/c.rs", false),
        ("**", "any/deep/path", true),
    ] {
        let result = glob_match(pattern, path, "posix", true);
        assert_eq!(result.matches, expected, "pattern={pattern} path={path}");
        assert!(
            !result.summary.contains("effort"),
            "ordinary glob hit the effort limit: pattern={pattern} path={path}"
        );
    }
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
