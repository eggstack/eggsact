use eggsact::text::{check_version_constraint, version_compare};

// ─── version_compare ─────────────────────────────────────────────────

#[test]
fn test_version_compare_equal() {
    let result = version_compare("1.0.0", "1.0.0", "semver");
    assert_eq!(result.comparison, 0);
    assert!(result.valid);
}

#[test]
fn test_version_compare_less() {
    let result = version_compare("1.0.0", "2.0.0", "semver");
    assert_eq!(result.comparison, -1);
    assert!(result.valid);
}

#[test]
fn test_version_compare_greater() {
    let result = version_compare("2.0.0", "1.0.0", "semver");
    assert_eq!(result.comparison, 1);
    assert!(result.valid);
}

#[test]
fn test_version_compare_patch() {
    let result = version_compare("1.0.1", "1.0.0", "semver");
    assert_eq!(result.comparison, 1);

    let result = version_compare("1.0.0", "1.0.1", "semver");
    assert_eq!(result.comparison, -1);
}

#[test]
fn test_version_compare_minor() {
    let result = version_compare("1.1.0", "1.0.0", "semver");
    assert_eq!(result.comparison, 1);

    let result = version_compare("1.0.0", "1.1.0", "semver");
    assert_eq!(result.comparison, -1);
}

#[test]
fn test_version_compare_major() {
    let result = version_compare("2.0.0", "1.9.9", "semver");
    assert_eq!(result.comparison, 1);
}

#[test]
fn test_version_compare_invalid() {
    let result = version_compare("not-a-version", "1.0.0", "semver");
    assert!(!result.valid);
}

#[test]
fn test_version_compare_prerelease_ignored_by_semver() {
    // version_compare (semver scheme) only compares major.minor.patch;
    // pre-release identifiers are ignored for the comparison result
    let result = version_compare("1.0.0-alpha", "1.0.0", "semver");
    assert_eq!(result.comparison, 0);
    assert!(result.valid);

    let result = version_compare("1.0.0-alpha", "1.0.0-beta", "semver");
    assert_eq!(result.comparison, 0);
    assert!(result.valid);
}

// ─── check_version_constraint ────────────────────────────────────────

#[test]
fn test_constraintcaret_equal() {
    let result = check_version_constraint("1.0.0", "^1.0.0", "semver");
    assert!(result.satisfies);
}

#[test]
fn test_constraint_caret_minor_compatible() {
    let result = check_version_constraint("1.2.3", "^1.0.0", "semver");
    assert!(result.satisfies);
}

#[test]
fn test_constraint_caret_major_incompatible() {
    let result = check_version_constraint("2.0.0", "^1.0.0", "semver");
    assert!(!result.satisfies);
}

#[test]
fn test_constraint_tilde_patch_compatible() {
    let result = check_version_constraint("1.0.1", "~1.0.0", "semver");
    assert!(result.satisfies);
}

#[test]
fn test_constraint_tilde_minor_incompatible() {
    let result = check_version_constraint("1.1.0", "~1.0.0", "semver");
    assert!(!result.satisfies);
}

#[test]
fn test_constraint_range_inclusive() {
    let result = check_version_constraint("1.5.0", ">=1.0.0, <2.0.0", "semver");
    assert!(result.satisfies);
}

#[test]
fn test_constraint_range_exclusive() {
    let result = check_version_constraint("2.0.0", ">=1.0.0, <2.0.0", "semver");
    assert!(!result.satisfies);
}

#[test]
fn test_constraint_wildcard() {
    let result = check_version_constraint("1.2.3", "1.*", "semver");
    assert!(result.satisfies);
}

#[test]
fn test_constraint_wildcard_no_match() {
    let result = check_version_constraint("2.0.0", "1.*", "semver");
    assert!(!result.satisfies);
}

#[test]
fn test_constraint_invalid_version() {
    let result = check_version_constraint("not-a-version", "^1.0.0", "semver");
    assert!(!result.satisfies);
}

#[test]
fn test_constraint_greater_than() {
    let result = check_version_constraint("1.5.0", ">1.0.0", "semver");
    assert!(result.satisfies);

    let result = check_version_constraint("0.5.0", ">1.0.0", "semver");
    assert!(!result.satisfies);
}

#[test]
fn test_constraint_less_than() {
    let result = check_version_constraint("0.5.0", "<1.0.0", "semver");
    assert!(result.satisfies);

    let result = check_version_constraint("1.5.0", "<1.0.0", "semver");
    assert!(!result.satisfies);
}

#[test]
fn test_constraint_equal_or_greater() {
    let result = check_version_constraint("1.0.0", ">=1.0.0", "semver");
    assert!(result.satisfies);

    let result = check_version_constraint("0.9.0", ">=1.0.0", "semver");
    assert!(!result.satisfies);
}

#[test]
fn test_constraint_equal_or_less() {
    let result = check_version_constraint("1.0.0", "<=1.0.0", "semver");
    assert!(result.satisfies);

    let result = check_version_constraint("1.1.0", "<=1.0.0", "semver");
    assert!(!result.satisfies);
}

#[test]
fn test_constraint_explanation() {
    let result = check_version_constraint("1.5.0", "^1.0.0", "semver");
    assert!(!result.explanation.is_empty());
}

// ─── Pre-release ordering (dev/snapshot/pre) via constraints ────────
// compare_pre_release uses alphabetical string comparison for non-numeric
// Semantic pre-release ordering: dev/pre/snapshot < alpha < beta < rc

#[test]
fn test_prerelease_dev_satisfies_ge_alpha() {
    // dev (-1) < alpha (0), so 1.0.0-dev < 1.0.0-alpha
    let result = check_version_constraint("1.0.0-dev", ">=1.0.0-alpha", "semver");
    assert!(!result.satisfies);
}

#[test]
fn test_prerelease_dev_satisfies_ge_beta() {
    // dev (-1) < beta (1), so 1.0.0-dev < 1.0.0-beta
    let result = check_version_constraint("1.0.0-dev", ">=1.0.0-beta", "semver");
    assert!(!result.satisfies);
}

#[test]
fn test_prerelease_dev_less_than_rc() {
    // dev (-1) < rc (2), so 1.0.0-dev < 1.0.0-rc
    let result = check_version_constraint("1.0.0-dev", "<1.0.0-rc", "semver");
    assert!(result.satisfies);
}

#[test]
fn test_prerelease_snapshot_satisfies_ge_alpha() {
    // snapshot (-1) < alpha (0)
    let result = check_version_constraint("1.0.0-snapshot", ">=1.0.0-alpha", "semver");
    assert!(!result.satisfies);
}

#[test]
fn test_prerelease_snapshot_less_than_rc() {
    // snapshot (-1) < rc (2)
    let result = check_version_constraint("1.0.0-snapshot", "<1.0.0-rc", "semver");
    assert!(result.satisfies);
}

#[test]
fn test_prerelease_pre_satisfies_ge_alpha() {
    // pre (-1) < alpha (0)
    let result = check_version_constraint("1.0.0-pre", ">=1.0.0-alpha", "semver");
    assert!(!result.satisfies);
}

#[test]
fn test_prerelease_pre_less_than_rc() {
    // pre (-1) < rc (2)
    let result = check_version_constraint("1.0.0-pre", "<1.0.0-rc", "semver");
    assert!(result.satisfies);
}

#[test]
fn test_prerelease_numeric_before_alpha() {
    // numeric identifiers sort before non-numeric in compare_pre_release
    let result = check_version_constraint("1.0.0-1", "<1.0.0-alpha", "semver");
    assert!(result.satisfies);
}

#[test]
fn test_prerelease_numeric_ordering() {
    // numeric identifiers are compared numerically
    let result = check_version_constraint("1.0.0-1", "<1.0.0-2", "semver");
    assert!(result.satisfies);

    let result = check_version_constraint("1.0.0-10", ">1.0.0-2", "semver");
    assert!(result.satisfies);
}

#[test]
fn test_prerelease_longer_list_sorts_after_shorter() {
    // "alpha.1" splits to ["alpha", "1"], which is longer than ["alpha"]
    // when prefixes match, longer list sorts after shorter
    let result = check_version_constraint("1.0.0-alpha.1", ">1.0.0-alpha", "semver");
    assert!(result.satisfies);
}

#[test]
fn test_prerelease_same_identifier_equal() {
    let result = check_version_constraint("1.0.0-dev", ">=1.0.0-dev", "semver");
    assert!(result.satisfies);

    let result = check_version_constraint("1.0.0-dev", "<=1.0.0-dev", "semver");
    assert!(result.satisfies);
}

#[test]
fn test_prerelease_before_release() {
    // any pre-release sorts before the release version
    let result = check_version_constraint("1.0.0-alpha", "<1.0.0", "semver");
    assert!(result.satisfies);
}

#[test]
fn test_prerelease_all_ordering_chain() {
    // Verify semantic ordering through constraint checks:
    // dev/pre/snapshot < alpha < beta < rc
    let pairs = [
        ("dev", "alpha"),
        ("pre", "alpha"),
        ("snapshot", "alpha"),
        ("alpha", "beta"),
        ("beta", "rc"),
    ];
    for (a, b) in pairs {
        let ver_a = format!("1.0.0-{}", a);
        let constraint = format!(">=1.0.0-{}", b);
        let result = check_version_constraint(&ver_a, &constraint, "semver");
        assert!(!result.satisfies, "Expected {} < 1.0.0-{}", a, b);
    }
}

// ─── Tilde constraint with patch==0 ─────────────────────────────────

#[test]
fn test_tilde_patch_zero_matches_equal() {
    let result = check_version_constraint("1.0.0", "~1.0.0", "semver");
    assert!(result.satisfies);
}

#[test]
fn test_tilde_patch_zero_matches_patch_increment() {
    let result = check_version_constraint("1.0.1", "~1.0.0", "semver");
    assert!(result.satisfies);
}

#[test]
fn test_tilde_patch_zero_rejects_minor_bump() {
    let result = check_version_constraint("1.1.0", "~1.0.0", "semver");
    assert!(!result.satisfies);
}

#[test]
fn test_tilde_zero_zero_zero() {
    let result = check_version_constraint("0.0.0", "~0.0.0", "semver");
    assert!(result.satisfies);

    let result = check_version_constraint("0.0.5", "~0.0.0", "semver");
    assert!(result.satisfies);

    let result = check_version_constraint("0.1.0", "~0.0.0", "semver");
    assert!(!result.satisfies);
}

#[test]
fn test_tilde_two_component() {
    let result = check_version_constraint("1.0.5", "~1.0", "semver");
    assert!(result.satisfies);

    let result = check_version_constraint("1.1.0", "~1.0", "semver");
    assert!(!result.satisfies);
}

#[test]
fn test_tilde_with_prerelease() {
    let result = check_version_constraint("1.0.0-alpha", "~1.0.0-alpha", "semver");
    assert!(result.satisfies);

    let result = check_version_constraint("1.0.0", "~1.0.0-alpha", "semver");
    assert!(result.satisfies);

    let result = check_version_constraint("1.0.5", "~1.0.0-alpha", "semver");
    assert!(result.satisfies);

    let result = check_version_constraint("1.1.0", "~1.0.0-alpha", "semver");
    assert!(!result.satisfies);
}
