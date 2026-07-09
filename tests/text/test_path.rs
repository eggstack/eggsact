use eggsact::text::path::{path_analyze, path_compare, path_normalize, path_scope_check};

// ─── path_analyze ────────────────────────────────────────────────────

#[test]
fn test_path_analyze_relative() {
    let result = path_analyze("src/main.rs", "posix");
    assert!(!result.absolute);
    assert!(!result.components.is_empty());
}

#[test]
fn test_path_analyze_absolute_posix() {
    let result = path_analyze("/usr/local/bin", "posix");
    assert!(result.absolute);
}

#[test]
fn test_path_analyze_dot_segments() {
    let result = path_analyze("src/../src/main.rs", "posix");
    assert!(result.has_traversal);
}

#[test]
fn test_path_analyze_trailing_separator() {
    let result = path_analyze("src/main/", "posix");
    assert_eq!(result.components, vec!["src", "main"]);
    assert!(!result.has_traversal);
    assert_eq!(result.name.as_deref(), Some("main"));
}

#[test]
fn test_path_analyze_empty() {
    let result = path_analyze("", "posix");
    assert!(result.components.is_empty());
    assert!(!result.absolute);
    assert!(!result.has_traversal);
    assert!(result.name.is_none());
}

#[test]
fn test_path_analyze_root() {
    let result = path_analyze("/", "posix");
    assert!(result.absolute);
}

#[test]
fn test_path_analyze_windows() {
    let result = path_analyze("C:\\Users\\test", "windows");
    assert!(result.absolute);
}

#[test]
fn test_path_analyze_unc() {
    let result = path_analyze("\\\\server\\share", "windows");
    assert!(result.absolute);
}

#[test]
fn test_path_analyze_components() {
    let result = path_analyze("src/main.rs", "posix");
    assert!(!result.components.is_empty());
}

#[test]
fn test_path_analyze_name() {
    let result = path_analyze("src/main.rs", "posix");
    assert!(result.name.is_some());
    assert_eq!(result.name.as_deref(), Some("main.rs"));
}

#[test]
fn test_path_analyze_suffix() {
    let result = path_analyze("src/main.rs", "posix");
    assert_eq!(result.suffix.as_deref(), Some(".rs"));
}

// ─── path_compare ────────────────────────────────────────────────────

#[test]
fn test_path_compare_identical() {
    let result = path_compare("src/main.rs", "src/main.rs", "posix", true, true, true);
    assert!(result.equal);
}

#[test]
fn test_path_compare_different() {
    let result = path_compare("src/main.rs", "src/lib.rs", "posix", true, true, true);
    assert!(!result.equal);
}

#[test]
fn test_path_compare_case_insensitive() {
    let result = path_compare("Src/Main.Rs", "src/main.rs", "posix", false, true, true);
    assert!(result.equal);
}

#[test]
fn test_path_compare_case_sensitive() {
    let result = path_compare("Src/Main.Rs", "src/main.rs", "posix", true, true, true);
    assert!(!result.equal);
}

#[test]
fn test_path_compare_with_dot_segments() {
    let result = path_compare("src/./main.rs", "src/main.rs", "posix", true, true, true);
    assert!(result.equal);
}

#[test]
fn test_path_compare_without_collapse() {
    let result = path_compare("src/./main.rs", "src/main.rs", "posix", true, true, false);
    assert!(!result.equal);
}

#[test]
fn test_path_compare_empty() {
    let result = path_compare("", "", "posix", true, true, true);
    assert!(result.equal);
}

// ─── path_normalize ──────────────────────────────────────────────────

#[test]
fn test_path_normalize_posix() {
    let result = path_normalize("src/./main.rs", "posix", true, false);
    assert!(result.normalized.contains("src"));
    assert!(result.normalized.contains("main.rs"));
    assert!(!result.normalized.contains("/./"));
}

#[test]
fn test_path_normalize_collapse_dot_segments() {
    let result = path_normalize("a/b/../c", "posix", true, false);
    assert!(!result.normalized.contains(".."));
}

#[test]
fn test_path_normalize_preserve_trailing() {
    let result = path_normalize("src/main.rs/", "posix", true, true);
    assert!(result.normalized.ends_with('/'));
}

#[test]
fn test_path_normalize_no_trailing() {
    let result = path_normalize("src/main.rs/", "posix", true, false);
    assert!(!result.normalized.ends_with('/'));
}

#[test]
fn test_path_normalize_empty() {
    let result = path_normalize("", "posix", true, false);
    assert!(result.normalized.is_empty());
}

#[test]
fn test_path_normalize_root() {
    let result = path_normalize("/", "posix", true, false);
    assert_eq!(result.normalized, "/");
}

// ─── path_normalize Windows mixed-separator handling (BUG-004) ───────

#[test]
fn test_path_normalize_windows_forward_slash_drive_letter() {
    let result = path_normalize("C:/foo/../bar", "windows", true, false);
    assert_eq!(result.normalized, "C:\\bar");
}

#[test]
fn test_path_normalize_windows_mixed_slashes_drive_letter() {
    let result = path_normalize("C:\\foo/../bar", "windows", true, false);
    assert_eq!(result.normalized, "C:\\bar");
}

#[test]
fn test_path_normalize_windows_unc_with_forward_slashes() {
    let result = path_normalize("//server/share/dir/../file", "windows", true, false);
    assert_eq!(result.normalized, "\\\\server\\share\\file");
}

// ─── path_scope_check ────────────────────────────────────────────────

#[test]
fn test_path_scope_check_inside() {
    let result = path_scope_check("/home/user", "/home/user/docs/file.txt", "posix", true);
    assert!(result.inside_root);
}

#[test]
fn test_path_scope_check_outside() {
    let result = path_scope_check("/home/user", "/etc/passwd", "posix", true);
    assert!(!result.inside_root);
}

#[test]
fn test_path_scope_check_same_path() {
    let result = path_scope_check("/home/user", "/home/user", "posix", true);
    assert!(result.inside_root);
}

#[test]
fn test_path_scope_check_relative() {
    let result = path_scope_check("src", "src/main.rs", "posix", true);
    assert!(result.inside_root);
}

#[test]
fn test_path_scope_check_traversal() {
    let result = path_scope_check("/home/user", "/home/user/../other", "posix", true);
    assert!(!result.inside_root);
    assert!(result.escapes_via_dotdot);
}

#[test]
fn test_path_scope_check_dotdot_in_filename_not_traversal() {
    // A filename containing ".." should NOT be flagged as traversal
    let result = path_scope_check("/home/user", "/home/user/file..txt", "posix", true);
    assert!(result.inside_root);
}

#[test]
fn test_path_scope_check_real_traversal() {
    // Real traversal with .. as path component
    let result = path_scope_check("/home/user", "/home/user/../other", "posix", true);
    assert!(!result.inside_root);
}

#[test]
fn test_path_scope_check_case_insensitive() {
    let result = path_scope_check("/Home/User", "/home/user/docs", "posix", false);
    assert!(result.inside_root);
}
