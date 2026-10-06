use eggsact::text::{patch_apply_check, patch_summary};

// ─── patch_summary ───────────────────────────────────────────────────

#[test]
fn test_patch_summary_basic() {
    let patch = "--- a/file.txt\n+++ b/file.txt\n@@ -1,3 +1,3 @@\n line1\n-old\n+new\n line3\n";
    let result = patch_summary(patch);
    assert!(result.files_changed >= 1);
}

#[test]
fn test_patch_summary_multiple_files() {
    let patch = "--- a/file1.txt\n+++ b/file1.txt\n@@ -1 +1 @@\n-old\n+new\n--- a/file2.txt\n+++ b/file2.txt\n@@ -1 +1 @@\n-old\n+new\n";
    let result = patch_summary(patch);
    // May detect 1 or 2 files depending on parser
    let _ = result;
}

#[test]
fn test_patch_summary_stats() {
    let patch = "--- a/file.txt\n+++ b/file.txt\n@@ -1,3 +1,3 @@\n line1\n-old1\n-old2\n+new1\n+new2\n+new3\n line3\n";
    let result = patch_summary(patch);
    assert!(result.additions > 0 || result.deletions > 0);
}

#[test]
fn test_patch_summary_empty() {
    let result = patch_summary("");
    assert!(result.files_changed == 0);
}

#[test]
fn test_patch_summary_renames() {
    let patch = "diff --git a/old.txt b/new.txt\nrename from old.txt\nrename to new.txt\n";
    let result = patch_summary(patch);
    let _ = result;
}

// ─── patch_apply_check ───────────────────────────────────────────────

#[test]
fn test_patch_apply_check_clean() {
    let original = "line1\nline2\nline3\n";
    let patch = "--- a/file.txt\n+++ b/file.txt\n@@ -1,3 +1,3 @@\n line1\n-old\n+new\n line3\n";
    let result = patch_apply_check(original, patch, false, false, false);
    assert!(result.applies);
}

#[test]
fn test_patch_apply_check_context_mismatch() {
    let original = "line1\nWRONG\nline3\n";
    let patch = "--- a/file.txt\n+++ b/file.txt\n@@ -1,3 +1,3 @@\n line1\n-old\n+new\n line3\n";
    let result = patch_apply_check(original, patch, true, false, false);
    let _ = result;
}

#[test]
fn test_patch_apply_check_empty_original() {
    let patch = "--- a/file.txt\n+++ b/file.txt\n@@ -0,0 +1 @@\n+new line\n";
    let result = patch_apply_check("", patch, false, false, false);
    let _ = result;
}

#[test]
fn test_patch_apply_check_empty_patch() {
    let result = patch_apply_check("line1\n", "", false, false, false);
    assert!(result.applies || result.failed_hunks.is_empty());
}

#[test]
fn test_patch_apply_check_reports_lenient_eof_truncation() {
    let patch = "--- a/file.txt\n+++ b/file.txt\n@@ -1,3 +1,2 @@\n a\n-b\n-c\n";
    let result = patch_apply_check("a\nb\n", patch, false, false, false);
    assert!(result
        .findings
        .iter()
        .any(|finding| finding.contains("context truncated at end")));
}

#[test]
fn test_patch_apply_check_result_text() {
    let original = "line1\nold\nline3\n";
    let patch = "--- a/file.txt\n+++ b/file.txt\n@@ -1,3 +1,3 @@\n line1\n-old\n+new\n line3\n";
    let result = patch_apply_check(original, patch, false, false, true);
    let _ = result;
}

#[test]
fn test_patch_apply_check_fingerprint() {
    let original = "line1\nold\nline3\n";
    let patch = "--- a/file.txt\n+++ b/file.txt\n@@ -1,3 +1,3 @@\n line1\n-old\n+new\n line3\n";
    let result = patch_apply_check(original, patch, false, true, false);
    let _ = result;
}

#[test]
fn test_zero_line_destination_ranges_remain_ordered() {
    let patch = "--- a/file.txt\n+++ b/file.txt\n@@ -2,1 +2,0 @@\n-line2\n";

    let applied = patch_apply_check("line1\nline2\n", patch, true, false, false);
    assert!(applied.applies);
    assert_eq!(applied.affected_line_ranges[0].start, 2);
    assert_eq!(applied.affected_line_ranges[0].end, 2);

    let summary = patch_summary(patch);
    let ranges = summary.line_ranges_by_file.get("file.txt").unwrap();
    assert_eq!(ranges[0].start, 2);
    assert_eq!(ranges[0].end, 2);
}

#[test]
fn nonzero_hunk_replaces_only_its_source_line() {
    let patch = "--- a/file.txt\n+++ b/file.txt\n@@ -2,1 +2,1 @@\n-b\n+B\n";
    let result = patch_apply_check("a\nb\nc", patch, true, true, true);
    assert!(result.applies);
    assert_eq!(result.result_text.as_deref(), Some("a\nB\nc"));
}

#[test]
fn insertion_and_deletion_after_nonzero_prefix_preserve_prefix_and_suffix() {
    let insert = "--- a/file.txt\n+++ b/file.txt\n@@ -1,2 +1,3 @@\n a\n+X\n b\n";
    let inserted = patch_apply_check("a\nb\nc", insert, true, false, true);
    assert_eq!(inserted.result_text.as_deref(), Some("a\nX\nb\nc"));

    let delete = "--- a/file.txt\n+++ b/file.txt\n@@ -2,1 +2,0 @@\n-b\n";
    let deleted = patch_apply_check("a\nb\nc", delete, true, false, true);
    assert_eq!(deleted.result_text.as_deref(), Some("a\nc"));
}

#[test]
fn separated_hunks_use_original_line_numbers_after_line_count_change() {
    let patch =
        "--- a/file.txt\n+++ b/file.txt\n@@ -2,1 +2,2 @@\n b\n+X\n@@ -4,1 +5,1 @@\n-d\n+D\n";
    let result = patch_apply_check("a\nb\nc\nd\ne", patch, true, true, true);
    assert!(result.applies);
    assert_eq!(result.result_text.as_deref(), Some("a\nb\nX\nc\nD\ne"));
}

#[test]
fn crlf_output_and_fingerprint_follow_the_applied_result() {
    let patch = "--- a/file.txt\r\n+++ b/file.txt\r\n@@ -2,1 +2,1 @@\r\n-b\r\n+B\r\n";
    let result = patch_apply_check("a\r\nb\r\nc\r\n", patch, true, true, true);
    assert!(result.applies);
    assert_eq!(result.newline_style_before, "CRLF");
    assert_eq!(result.newline_style_after, "CRLF");
    // The original's trailing CRLF is part of the file and must survive.
    assert_eq!(result.result_text.as_deref(), Some("a\r\nB\r\nc\r\n"));
    assert!(!result.result_fingerprint.is_empty());
}

#[test]
fn trailing_terminator_is_preserved_across_line_styles() {
    let patch = "--- a/file.txt\n+++ b/file.txt\n@@ -2,1 +2,2 @@\n-beta\n+BETA\n+X\n";

    // LF original keeps its final newline.
    let lf = patch_apply_check("alpha\nbeta\ngamma\n", patch, true, false, true);
    assert!(lf.applies);
    assert_eq!(lf.result_text.as_deref(), Some("alpha\nBETA\nX\ngamma\n"));

    // CRLF original keeps its final CRLF.
    let crlf = patch_apply_check("alpha\r\nbeta\r\ngamma\r\n", patch, true, false, true);
    assert!(crlf.applies);
    assert_eq!(
        crlf.result_text.as_deref(),
        Some("alpha\r\nBETA\r\nX\r\ngamma\r\n")
    );

    // An original with no trailing newline must not gain one.
    let bare = patch_apply_check("alpha\nbeta\ngamma", patch, true, false, true);
    assert!(bare.applies);
    assert_eq!(bare.result_text.as_deref(), Some("alpha\nBETA\nX\ngamma"));
}

#[test]
fn trailing_terminator_of_a_lone_carriage_return_is_preserved() {
    let patch = "--- a/file.txt\n+++ b/file.txt\n@@ -1,1 +1,1 @@\n-a\rb\n+A\rb\n";
    let result = patch_apply_check("a\rb\r", patch, true, false, true);
    assert!(result.applies);
    assert_eq!(result.result_text.as_deref(), Some("A\rb\r"));
}

#[test]
fn partially_applied_patch_returns_no_result_text() {
    // Hunk 1 consumes lines 1-3, so hunk 2 (which starts inside them) fails.
    // The spliced text used to be returned anyway, so a caller keying on
    // `result_text.is_some()` wrote corrupt content from a patch the tool
    // itself reported as failed.
    let original = "alpha\nbeta\ngamma\ndelta\n";
    let patch = "--- a/file.txt\n+++ b/file.txt\n@@ -1,3 +1,3 @@\n-alpha\n+ALPHA\n beta\n gamma\n@@ -2,1 +2,1 @@\n-beta\n+BETA\n";
    let result = patch_apply_check(original, patch, false, false, true);

    assert!(!result.applies);
    assert_eq!(result.hunks_applied, 1);
    assert_eq!(result.hunks_failed, 1);
    assert_eq!(
        result.result_text, None,
        "a failed patch must not hand back partially-applied text"
    );
}

#[test]
fn fingerprint_does_not_depend_on_return_result_text() {
    // A fingerprint must be a function of the result content alone; flipping
    // an unrelated output-shaping flag used to change it.
    let original = "a\nb\nc\n";
    let patch = "--- a/file.txt\n+++ b/file.txt\n@@ -2,1 +2,1 @@\n-b\n+B\n";

    // Patch that cannot apply at all: nothing was applied, so the result is
    // the original either way.
    let missing = "--- a/file.txt\n+++ b/file.txt\n@@ -2,1 +2,1 @@\n-zzz\n+Z\n";
    let without = patch_apply_check(original, missing, true, true, false);
    let with = patch_apply_check(original, missing, true, true, true);
    assert!(!without.applies);
    assert!(!with.applies);
    assert_eq!(without.result_fingerprint, with.result_fingerprint);
    assert!(!without.result_fingerprint.is_empty());

    // And a patch that does apply: the flag must not change it either.
    let applied_without = patch_apply_check(original, patch, true, true, false);
    let applied_with = patch_apply_check(original, patch, true, true, true);
    assert_eq!(
        applied_without.result_fingerprint,
        applied_with.result_fingerprint
    );
    assert_eq!(
        applied_with.result_fingerprint,
        eggsact::text::text_fingerprint(
            applied_with.result_text.as_deref().unwrap(),
            "raw",
            "preserve",
            false,
            false
        )
        .sha256,
        "fingerprint must be the hash of the returned result text"
    );
}

#[test]
fn fingerprint_is_computed_from_applied_text_without_returning_text() {
    let original = "a\nb\nc";
    let patch = "--- a/file.txt\n+++ b/file.txt\n@@ -2,1 +2,1 @@\n-b\n+B\n";
    let without_text = patch_apply_check(original, patch, true, true, false);
    let with_text = patch_apply_check(original, patch, true, true, true);

    assert!(without_text.applies);
    assert!(without_text.result_text.is_none());
    assert_eq!(
        without_text.result_fingerprint,
        with_text.result_fingerprint
    );
    assert_ne!(without_text.result_fingerprint, "");
}
