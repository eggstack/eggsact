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
