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
fn test_path_analyze_windows_single_multibyte_component() {
    let result = path_analyze("͌", "windows");
    assert_eq!(result.name.as_deref(), Some("͌"));
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

// ─── UNC share boundary protection ────────────────────────────────────

#[test]
fn test_unc_dir_dotdot_file_stays_in_share() {
    // \\host\share\dir\..\file -> \\host\share\file (dir collapsed, share intact)
    let result = path_normalize("\\\\host\\share\\dir\\..\\file", "windows", true, false);
    assert_eq!(result.normalized, "\\\\host\\share\\file");
}

#[test]
fn test_unc_dotdot_above_share_clamped() {
    // \\host\share\.. -> \\host\share (.. clamped at UNC share boundary)
    let result = path_normalize("\\\\host\\share\\..", "windows", true, false);
    assert_eq!(result.normalized, "\\\\host\\share");
    // has_dot_dot is still true (raw path had ..)
    assert!(result.warnings.iter().any(|w| w.contains("dot-dot")));
}

#[test]
fn test_unc_double_dotdot_above_share() {
    // \\host\share\dir\..\..\secret -> \\host\share\secret
    // dir pops, then .. at share boundary is clamped
    let result = path_normalize(
        "\\\\host\\share\\dir\\..\\..\\secret",
        "windows",
        true,
        false,
    );
    assert_eq!(result.normalized, "\\\\host\\share\\secret");
}

#[test]
fn test_unc_forward_slash_equivalent() {
    // //host/share/dir/../file -> \\host\share\file
    let result = path_normalize("//host/share/dir/../file", "windows", true, false);
    assert_eq!(result.normalized, "\\\\host\\share\\file");
}

#[test]
fn test_unc_arbitrary_names_not_sentinel() {
    // \\documents\photos\..\secret -> \\documents\photos\secret
    // (.. is clamped at the UNC share boundary; "photos" is the share name)
    let result = path_normalize("\\\\documents\\photos\\..\\secret", "windows", true, false);
    assert_eq!(result.normalized, "\\\\documents\\photos\\secret");
}

#[test]
fn test_unc_share_root_valid() {
    let result = path_normalize("\\\\host\\share", "windows", true, false);
    assert_eq!(result.normalized, "\\\\host\\share");
    assert!(result.is_absolute);
}

#[test]
fn test_unc_incomplete_host_only() {
    // \\host is an incomplete UNC prefix
    let result = path_normalize("\\\\host", "windows", true, false);
    assert_eq!(result.normalized, "\\\\host");
    assert!(result.is_absolute);
}

#[test]
fn test_drive_relative_not_absolute() {
    let result = path_normalize("C:foo", "windows", true, false);
    assert!(!result.is_absolute);
}

#[test]
fn test_drive_rooted_absolute() {
    let result = path_normalize("C:\\foo", "windows", true, false);
    assert!(result.is_absolute);
}

#[test]
fn test_drive_rooted_forward_slash() {
    let result = path_normalize("C:/foo", "windows", true, false);
    assert!(result.is_absolute);
}

#[test]
fn test_drive_dotdot_collapse() {
    // C:/foo/../bar -> C:\bar
    let result = path_normalize("C:/foo/../bar", "windows", true, false);
    assert_eq!(result.normalized, "C:\\bar");
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

// ─── UNC scope check ─────────────────────────────────────────────────

#[test]
fn test_unc_scope_check_inside() {
    let result = path_scope_check(
        "\\\\host\\share",
        "\\\\host\\share\\dir\\file.txt",
        "windows",
        true,
    );
    assert!(result.inside_root);
}

#[test]
fn test_unc_scope_check_escape() {
    let result = path_scope_check(
        "\\\\host\\share",
        "\\\\host\\share\\..\\secret",
        "windows",
        true,
    );
    assert!(!result.inside_root);
    assert!(result.escapes_via_dotdot);
}

#[test]
fn test_unc_scope_check_different_share() {
    let result = path_scope_check(
        "\\\\host\\share",
        "\\\\host\\other\\file.txt",
        "windows",
        true,
    );
    assert!(!result.inside_root);
}

// ─── WS3 drive-relative classification tests ────────────────────────────

#[test]
fn test_analyze_drive_relative_is_relative() {
    let result = path_analyze("C:foo", "windows");
    assert!(!result.absolute, "C:foo must be relative");
}

#[test]
fn test_analyze_drive_relative_bare() {
    let result = path_analyze("C:", "windows");
    assert!(!result.absolute, "C: must be relative");
}

#[test]
fn test_analyze_drive_rooted_backslash_is_absolute() {
    let result = path_analyze("C:\\foo", "windows");
    assert!(result.absolute, "C:\\foo must be absolute");
}

#[test]
fn test_analyze_drive_rooted_forward_slash_is_absolute() {
    let result = path_analyze("C:/foo", "windows");
    assert!(result.absolute, "C:/foo must be absolute");
}

#[test]
fn test_normalize_drive_relative_preserves_drive() {
    let result = path_normalize("C:foo", "windows", true, false);
    assert!(!result.is_absolute);
    assert!(result.normalized.starts_with("C:"));
}

#[test]
fn test_scope_check_drive_relative_c_drive_root() {
    // C:foo under C:\work must NOT be inside (drive-relative can't be
    // resolved lexically).
    let result = path_scope_check("C:\\work", "C:foo", "windows", true);
    assert!(!result.inside_root);
}

#[test]
fn test_scope_check_drive_relative_d_drive_root() {
    // C:foo under D:\work must NOT be inside.
    let result = path_scope_check("D:\\work", "C:foo", "windows", true);
    assert!(!result.inside_root);
}

#[test]
fn test_scope_check_drive_relative_dotdot() {
    // C:..\secret under C:\work must NOT be inside.
    let result = path_scope_check("C:\\work", "C:..\\secret", "windows", true);
    assert!(!result.inside_root);
}

#[test]
fn test_scope_check_ordinary_relative_still_resolves() {
    // src\main.rs under C:\work must still be inside.
    let result = path_scope_check("C:\\work", "src\\main.rs", "windows", true);
    assert!(result.inside_root);
}

#[test]
fn test_scope_check_drive_relative_finding_message() {
    // The result should contain a finding explaining the ambiguity.
    let result = path_scope_check("C:\\work", "C:foo", "windows", true);
    assert!(
        result.findings.iter().any(|f| f.contains("Drive-relative")),
        "expected a Drive-relative finding, got: {:?}",
        result.findings
    );
}

#[test]
fn test_scope_check_drive_relative_finding_uses_actual_drive() {
    let result = path_scope_check("C:\\work", "D:foo", "windows", true);
    assert!(!result.inside_root);
    assert!(result.findings.iter().any(|f| f.contains("D:foo")));
    assert!(!result.findings.iter().any(|f| f.contains("C:foo")));
}

#[test]
fn test_analyze_drive_relative_warning() {
    // path_analyze should emit a warning for drive-relative paths.
    let result = path_analyze("C:foo", "windows");
    assert!(
        result.warnings.iter().any(|w| w.contains("Drive-relative")),
        "expected a Drive-relative warning, got: {:?}",
        result.warnings
    );
}
