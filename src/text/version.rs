use fancy_regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub pre_release: Vec<String>,
    pub build: String,
    pub raw: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedConstraintComponent {
    pub operator: String,
    pub version: ParsedVersion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedConstraint {
    pub raw: String,
    pub scheme: String,
    pub components: Vec<ParsedConstraintComponent>,
    #[serde(rename = "type")]
    pub constraint_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionCompareResult {
    pub comparison: i32,
    pub valid: bool,
    pub scheme: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionConstraintResult {
    pub satisfies: bool,
    pub parsed_version: Option<ParsedVersion>,
    pub parsed_constraint: Option<ParsedConstraint>,
    pub scheme: String,
    pub explanation: String,
    pub findings: Vec<String>,
}

#[allow(dead_code)]
fn _get_pre_release_order(ident: &str) -> i32 {
    match ident.to_lowercase().as_str() {
        "dev" | "snapshot" | "pre" => -1,
        "alpha" | "a" => 0,
        "beta" | "b" => 1,
        "rc" | "c" => 2,
        _ => i32::MIN, // unknown label
    }
}

use std::sync::LazyLock;

static SEMVER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z\.\-]+))?(?:\+([0-9A-Za-z\.\-]+))?$").unwrap()
});
static SEMVER_LAX_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\d+)(?:\.(\d+))?(?:\.(\d+))?(?:-([0-9A-Za-z\.\-]+))?(?:\+([0-9A-Za-z\.\-]+))?$")
        .unwrap()
});

fn parse_pre_release_identifiers(ident: &str) -> Vec<String> {
    ident
        .split(['.', '-', ' '])
        .filter(|p| !p.is_empty())
        .map(String::from)
        .collect()
}

fn compare_pre_release(a: &[String], b: &[String]) -> i32 {
    if a.is_empty() && b.is_empty() {
        return 0;
    }
    if a.is_empty() {
        return 1;
    }
    if b.is_empty() {
        return -1;
    }

    for i in 0..std::cmp::min(a.len(), b.len()) {
        let ai = &a[i];
        let bi = &b[i];
        let ai_int = ai.chars().all(|c| c.is_ascii_digit());
        let bi_int = bi.chars().all(|c| c.is_ascii_digit());

        if ai_int && bi_int {
            let a_val = ai.parse::<u64>().unwrap_or(u64::MAX);
            let b_val = bi.parse::<u64>().unwrap_or(u64::MAX);
            let diff = (a_val as i128) - (b_val as i128);
            if diff != 0 {
                return if diff < 0 { -1 } else { 1 };
            }
        } else if ai_int {
            return -1;
        } else if bi_int {
            return 1;
        } else {
            let order_a = _get_pre_release_order(ai);
            let order_b = _get_pre_release_order(bi);
            // If both are known labels, use semantic ordering
            if order_a != i32::MIN && order_b != i32::MIN {
                return match order_a.cmp(&order_b) {
                    std::cmp::Ordering::Less => -1,
                    std::cmp::Ordering::Equal => 0,
                    std::cmp::Ordering::Greater => 1,
                };
            }
            // Unknown or mixed — fall back to lexicographic
            if ai < bi {
                return -1;
            } else if ai > bi {
                return 1;
            }
        }
    }

    if a.len() < b.len() {
        return -1;
    } else if a.len() > b.len() {
        return 1;
    }
    0
}

fn parse_semver_prefix(version: &str) -> Option<(u64, u64, u64)> {
    let trimmed = version.trim();
    let caps = SEMVER_RE.captures(trimmed).ok()??;
    let major = caps.get(1)?.as_str().parse().ok()?;
    let minor = caps.get(2)?.as_str().parse().ok()?;
    let patch = caps.get(3)?.as_str().parse().ok()?;
    Some((major, minor, patch))
}

fn python_semver_compare(a: &str, b: &str) -> VersionCompareResult {
    let parsed_a = parse_semver_prefix(a);
    let parsed_b = parse_semver_prefix(b);

    if parsed_a.is_none() {
        return VersionCompareResult {
            comparison: 0,
            valid: false,
            scheme: "semver".to_string(),
            summary: format!("Invalid semver: '{}'", a),
        };
    }
    if parsed_b.is_none() {
        return VersionCompareResult {
            comparison: 0,
            valid: false,
            scheme: "semver".to_string(),
            summary: format!("Invalid semver: '{}'", b),
        };
    }

    let (maj_a, min_a, pat_a) = parsed_a.unwrap();
    let (maj_b, min_b, pat_b) = parsed_b.unwrap();

    let (comparison, summary) = match (maj_a, min_a, pat_a).cmp(&(maj_b, min_b, pat_b)) {
        std::cmp::Ordering::Less => (-1, format!("{a} < {b}")),
        std::cmp::Ordering::Greater => (1, format!("{a} > {b}")),
        std::cmp::Ordering::Equal => (0, format!("{a} == {b}")),
    };

    VersionCompareResult {
        comparison,
        valid: true,
        scheme: "semver".to_string(),
        summary,
    }
}

fn python_loose_compare(a: &str, b: &str) -> VersionCompareResult {
    let re = regex::Regex::new(r"\d+").unwrap();
    let parts_a: Vec<u64> = re
        .find_iter(a)
        .filter_map(|m| m.as_str().parse::<u64>().ok())
        .collect();
    let parts_b: Vec<u64> = re
        .find_iter(b)
        .filter_map(|m| m.as_str().parse::<u64>().ok())
        .collect();

    let max_len = std::cmp::max(parts_a.len(), parts_b.len());
    for i in 0..max_len {
        let val_a = parts_a.get(i).copied().unwrap_or(0);
        let val_b = parts_b.get(i).copied().unwrap_or(0);
        if val_a < val_b {
            return VersionCompareResult {
                comparison: -1,
                valid: true,
                scheme: "loose".to_string(),
                summary: format!("{} < {}", a, b),
            };
        } else if val_a > val_b {
            return VersionCompareResult {
                comparison: 1,
                valid: true,
                scheme: "loose".to_string(),
                summary: format!("{} > {}", a, b),
            };
        }
    }

    VersionCompareResult {
        comparison: 0,
        valid: true,
        scheme: "loose".to_string(),
        summary: format!("{} == {}", a, b),
    }
}

#[allow(dead_code)]
fn _sort_pre_release_key(ident: &str) -> (i32, String) {
    if ident.chars().all(|c| c.is_ascii_digit()) {
        return (1, ident.to_string());
    }
    let order = _get_pre_release_order(ident);
    if order != -1 {
        return (0, order.to_string());
    }
    (2, ident.to_string())
}

fn version_less_than(a: &ParsedVersion, b: &ParsedVersion) -> bool {
    if a.major < b.major {
        return true;
    }
    if a.major > b.major {
        return false;
    }
    if a.minor < b.minor {
        return true;
    }
    if a.minor > b.minor {
        return false;
    }
    if a.patch < b.patch {
        return true;
    }
    if a.patch > b.patch {
        return false;
    }
    compare_pre_release(&a.pre_release, &b.pre_release) < 0
}

fn version_equal(a: &ParsedVersion, b: &ParsedVersion) -> bool {
    a.major == b.major && a.minor == b.minor && a.patch == b.patch && a.pre_release == b.pre_release
}

fn version_lte(a: &ParsedVersion, b: &ParsedVersion) -> bool {
    version_less_than(a, b) || version_equal(a, b)
}

fn version_gte(a: &ParsedVersion, b: &ParsedVersion) -> bool {
    !version_less_than(a, b)
}

fn version_gt(a: &ParsedVersion, b: &ParsedVersion) -> bool {
    !version_lte(a, b)
}

pub fn parse_version(version: &str) -> Option<ParsedVersion> {
    let version = version.trim();
    if let Ok(Some(caps)) = SEMVER_RE.captures(version) {
        let pre_release_str = caps.get(4).map(|m| m.as_str()).unwrap_or("");
        let build_str = caps.get(5).map(|m| m.as_str()).unwrap_or("");

        Some(ParsedVersion {
            major: caps.get(1)?.as_str().parse().ok()?,
            minor: caps.get(2)?.as_str().parse().ok()?,
            patch: caps.get(3)?.as_str().parse().ok()?,
            pre_release: if pre_release_str.is_empty() {
                Vec::new()
            } else {
                parse_pre_release_identifiers(pre_release_str)
            },
            build: build_str.to_string(),
            raw: version.to_string(),
        })
    } else {
        None
    }
}

fn parse_version_lax(version: &str) -> Option<ParsedVersion> {
    let version = version.trim();
    if let Ok(Some(caps)) = SEMVER_LAX_RE.captures(version) {
        let pre_release_str = caps.get(4).map(|m| m.as_str()).unwrap_or("");
        let build_str = caps.get(5).map(|m| m.as_str()).unwrap_or("");

        Some(ParsedVersion {
            major: caps.get(1)?.as_str().parse().ok()?,
            minor: caps
                .get(2)
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(0),
            patch: caps
                .get(3)
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(0),
            pre_release: if pre_release_str.is_empty() {
                Vec::new()
            } else {
                parse_pre_release_identifiers(pre_release_str)
            },
            build: build_str.to_string(),
            raw: version.to_string(),
        })
    } else {
        None
    }
}

fn parse_comparison_constraint(constraint: &str) -> (String, String) {
    let constraint = constraint.trim();
    let operators = [">=", "<=", "!=", ">", "<", "==", "="];

    for op in operators.iter() {
        if let Some(stripped) = constraint.strip_prefix(op) {
            let ver = stripped.trim();
            let actual_op = if *op == "=" || *op == "==" { "==" } else { *op };
            return (actual_op.to_string(), ver.to_string());
        }
    }
    ("=".to_string(), constraint.to_string())
}

fn cargo_caret_range(version: &ParsedVersion) -> (ParsedVersion, ParsedVersion) {
    let upper = if version.major != 0 {
        ParsedVersion {
            major: version.major + 1,
            minor: 0,
            patch: 0,
            pre_release: Vec::new(),
            build: String::new(),
            raw: String::new(),
        }
    } else if version.minor != 0 {
        ParsedVersion {
            major: 0,
            minor: version.minor + 1,
            patch: 0,
            pre_release: Vec::new(),
            build: String::new(),
            raw: String::new(),
        }
    } else if version.patch != 0 {
        ParsedVersion {
            major: 0,
            minor: 0,
            patch: version.patch + 1,
            pre_release: Vec::new(),
            build: String::new(),
            raw: String::new(),
        }
    } else {
        ParsedVersion {
            major: 0,
            minor: 0,
            patch: 1,
            pre_release: Vec::new(),
            build: String::new(),
            raw: String::new(),
        }
    };

    let lower = ParsedVersion {
        major: version.major,
        minor: version.minor,
        patch: version.patch,
        pre_release: version.pre_release.clone(),
        build: String::new(),
        raw: String::new(),
    };

    (lower, upper)
}

fn cargo_tilde_range(version: &ParsedVersion) -> (ParsedVersion, ParsedVersion) {
    let upper = if version.minor == 0 && version.patch == 0 && !version.pre_release.is_empty() {
        ParsedVersion {
            major: version.major,
            minor: version.minor + 1,
            patch: 0,
            pre_release: Vec::new(),
            build: String::new(),
            raw: String::new(),
        }
    } else if version.minor == 0 && version.patch == 0 {
        ParsedVersion {
            major: version.major,
            minor: 1,
            patch: 0,
            pre_release: Vec::new(),
            build: String::new(),
            raw: String::new(),
        }
    } else {
        ParsedVersion {
            major: version.major,
            minor: version.minor + 1,
            patch: 0,
            pre_release: Vec::new(),
            build: String::new(),
            raw: String::new(),
        }
    };

    let lower = ParsedVersion {
        major: version.major,
        minor: version.minor,
        patch: version.patch,
        pre_release: version.pre_release.clone(),
        build: String::new(),
        raw: format!("{}.{}.{}", version.major, version.minor, version.patch),
    };

    (lower, upper)
}

fn cargo_wildcard_range(constraint: &str) -> (Option<ParsedVersion>, Option<ParsedVersion>) {
    let constraint_trimmed = constraint.trim().trim_end_matches('.');
    let parts: Vec<&str> = constraint_trimmed.split('.').collect();
    let nums: Vec<u32> = parts.iter().filter_map(|p| p.parse().ok()).collect();

    if nums.len() == 1 {
        let lower = ParsedVersion {
            major: nums[0],
            minor: 0,
            patch: 0,
            pre_release: Vec::new(),
            build: String::new(),
            raw: constraint.to_string(),
        };
        let upper = ParsedVersion {
            major: nums[0] + 1,
            minor: 0,
            patch: 0,
            pre_release: Vec::new(),
            build: String::new(),
            raw: String::new(),
        };
        (Some(lower), Some(upper))
    } else if nums.len() == 2 {
        let lower = ParsedVersion {
            major: nums[0],
            minor: nums[1],
            patch: 0,
            pre_release: Vec::new(),
            build: String::new(),
            raw: constraint.to_string(),
        };
        let upper = ParsedVersion {
            major: nums[0],
            minor: nums[1] + 1,
            patch: 0,
            pre_release: Vec::new(),
            build: String::new(),
            raw: String::new(),
        };
        (Some(lower), Some(upper))
    } else {
        (None, None)
    }
}

fn evaluate_component(ver: &ParsedVersion, op: &str, bound: &ParsedVersion) -> bool {
    match op {
        ">=" => version_gte(ver, bound),
        ">" => version_gt(ver, bound),
        "<=" => version_lte(ver, bound),
        "<" => version_less_than(ver, bound),
        "==" => version_equal(ver, bound),
        "!=" => !version_equal(ver, bound),
        _ => false,
    }
}

pub fn version_compare(a: &str, b: &str, scheme: &str) -> VersionCompareResult {
    match scheme {
        "semver" => python_semver_compare(a, b),
        "pep440" => VersionCompareResult {
            comparison: 0,
            valid: false,
            scheme: "pep440".to_string(),
            summary: "PEP 440 not implemented (requires packaging library)".to_string(),
        },
        "loose" => python_loose_compare(a, b),
        _ => VersionCompareResult {
            comparison: 0,
            valid: false,
            scheme: scheme.to_string(),
            summary: format!("Unknown scheme: {}", scheme),
        },
    }
}

pub fn check_version_constraint(
    version: &str,
    constraint: &str,
    scheme: &str,
) -> VersionConstraintResult {
    let mut findings: Vec<String> = Vec::new();
    let parsed_ver = parse_version(version);

    if parsed_ver.is_none() {
        return VersionConstraintResult {
            satisfies: false,
            parsed_version: None,
            parsed_constraint: None,
            scheme: scheme.to_string(),
            explanation: format!("Invalid version: '{}'", version),
            findings: vec![format!(
                "Could not parse version string '{}' as semver",
                version
            )],
        };
    }

    let parsed_ver = parsed_ver.unwrap_or_else(|| unreachable!("checked is_none() above"));

    if scheme != "semver" && scheme != "cargo" {
        return VersionConstraintResult {
            satisfies: false,
            parsed_version: Some(parsed_ver),
            parsed_constraint: None,
            scheme: scheme.to_string(),
            explanation: format!("Unsupported scheme: '{}'", scheme),
            findings: vec![format!(
                "Scheme '{}' is not supported; use 'semver' or 'cargo'",
                scheme
            )],
        };
    }

    let constraint = constraint.trim();

    if constraint.contains('*') {
        let (lower, upper) = cargo_wildcard_range(constraint);
        if lower.is_none() || upper.is_none() {
            return VersionConstraintResult {
                satisfies: false,
                parsed_version: Some(parsed_ver),
                parsed_constraint: None,
                scheme: scheme.to_string(),
                explanation: format!("Invalid wildcard constraint: '{}'", constraint),
                findings,
            };
        }

        let lower = lower.unwrap_or_else(|| unreachable!("checked is_none() above"));
        let upper = upper.unwrap_or_else(|| unreachable!("checked is_none() above"));
        let satisfies = version_gte(&parsed_ver, &lower) && version_less_than(&parsed_ver, &upper);
        let pc = ParsedConstraint {
            raw: constraint.to_string(),
            scheme: scheme.to_string(),
            components: vec![
                ParsedConstraintComponent {
                    operator: ">=".to_string(),
                    version: lower,
                },
                ParsedConstraintComponent {
                    operator: "<".to_string(),
                    version: upper,
                },
            ],
            constraint_type: "wildcard".to_string(),
        };
        return VersionConstraintResult {
            satisfies,
            parsed_version: Some(parsed_ver),
            parsed_constraint: Some(pc),
            scheme: scheme.to_string(),
            explanation: if satisfies {
                format!("{} satisfies {}", version, constraint)
            } else {
                format!("{} does not satisfy {}", version, constraint)
            },
            findings,
        };
    }

    if let Some(stripped) = constraint.strip_prefix('^') {
        let ver_str = stripped.trim();
        let parsed_bound = parse_version(ver_str);

        if parsed_bound.is_none() {
            return VersionConstraintResult {
                satisfies: false,
                parsed_version: Some(parsed_ver),
                parsed_constraint: None,
                scheme: scheme.to_string(),
                explanation: format!("Invalid version in caret constraint: '{}'", ver_str),
                findings: vec![format!(
                    "Could not parse version '{}' in caret constraint",
                    ver_str
                )],
            };
        }

        let parsed_bound = parsed_bound.unwrap_or_else(|| unreachable!("checked is_none() above"));

        if parsed_bound.major == 0 && parsed_bound.minor == 0 && parsed_bound.patch == 0 {
            findings.push("Caret constraint ^0.0.0 matches only 0.0.0".to_string());
        }

        let (lower, upper) = cargo_caret_range(&parsed_bound);
        let satisfies = version_gte(&parsed_ver, &lower) && version_less_than(&parsed_ver, &upper);
        let pc = ParsedConstraint {
            raw: constraint.to_string(),
            scheme: "cargo".to_string(),
            components: vec![
                ParsedConstraintComponent {
                    operator: ">=".to_string(),
                    version: lower,
                },
                ParsedConstraintComponent {
                    operator: "<".to_string(),
                    version: upper,
                },
            ],
            constraint_type: "caret".to_string(),
        };
        return VersionConstraintResult {
            satisfies,
            parsed_version: Some(parsed_ver),
            parsed_constraint: Some(pc),
            scheme: "cargo".to_string(),
            explanation: if satisfies {
                format!("{} satisfies {}", version, constraint)
            } else {
                format!("{} does not satisfy {}", version, constraint)
            },
            findings,
        };
    }

    if let Some(stripped) = constraint.strip_prefix('~') {
        let ver_str = stripped.trim();
        let parsed_bound = parse_version_lax(ver_str);

        if parsed_bound.is_none() {
            return VersionConstraintResult {
                satisfies: false,
                parsed_version: Some(parsed_ver),
                parsed_constraint: None,
                scheme: scheme.to_string(),
                explanation: format!("Invalid version in tilde constraint: '{}'", ver_str),
                findings: vec![format!(
                    "Could not parse version '{}' in tilde constraint",
                    ver_str
                )],
            };
        }

        let parsed_bound = parsed_bound.unwrap_or_else(|| unreachable!("checked is_none() above"));
        let (lower, upper) = cargo_tilde_range(&parsed_bound);
        let satisfies = version_gte(&parsed_ver, &lower) && version_less_than(&parsed_ver, &upper);
        let pc = ParsedConstraint {
            raw: constraint.to_string(),
            scheme: "cargo".to_string(),
            components: vec![
                ParsedConstraintComponent {
                    operator: ">=".to_string(),
                    version: lower,
                },
                ParsedConstraintComponent {
                    operator: "<".to_string(),
                    version: upper,
                },
            ],
            constraint_type: "tilde".to_string(),
        };
        return VersionConstraintResult {
            satisfies,
            parsed_version: Some(parsed_ver),
            parsed_constraint: Some(pc),
            scheme: "cargo".to_string(),
            explanation: if satisfies {
                format!("{} satisfies {}", version, constraint)
            } else {
                format!("{} does not satisfy {}", version, constraint)
            },
            findings,
        };
    }

    if constraint.contains(',') {
        let parts: Vec<&str> = constraint
            .split(',')
            .map(|p| p.trim())
            .filter(|p| !p.is_empty())
            .collect();
        let mut all_satisfy = true;
        let mut components: Vec<ParsedConstraintComponent> = Vec::new();

        for part in parts {
            let (op, ver_str) = parse_comparison_constraint(part);
            let mut parsed_bound = parse_version(&ver_str);
            if parsed_bound.is_none() {
                parsed_bound = parse_version_lax(&ver_str);
            }

            if parsed_bound.is_none() {
                return VersionConstraintResult {
                    satisfies: false,
                    parsed_version: Some(parsed_ver),
                    parsed_constraint: None,
                    scheme: scheme.to_string(),
                    explanation: format!("Invalid version in constraint part: '{}'", ver_str),
                    findings: vec![format!(
                        "Could not parse version '{}' in constraint '{}'",
                        ver_str, part
                    )],
                };
            }

            let parsed_bound =
                parsed_bound.unwrap_or_else(|| unreachable!("checked is_none() above"));
            components.push(ParsedConstraintComponent {
                operator: op.clone(),
                version: parsed_bound.clone(),
            });

            if !evaluate_component(&parsed_ver, &op, &parsed_bound) {
                all_satisfy = false;
            }
        }

        let pc = ParsedConstraint {
            raw: constraint.to_string(),
            scheme: scheme.to_string(),
            components,
            constraint_type: "range".to_string(),
        };
        return VersionConstraintResult {
            satisfies: all_satisfy,
            parsed_version: Some(parsed_ver),
            parsed_constraint: Some(pc),
            scheme: scheme.to_string(),
            explanation: if all_satisfy {
                format!("{} satisfies {}", version, constraint)
            } else {
                format!("{} does not satisfy {}", version, constraint)
            },
            findings,
        };
    }

    let (op, ver_str) = parse_comparison_constraint(constraint);
    let mut parsed_bound = parse_version(&ver_str);
    if parsed_bound.is_none() {
        parsed_bound = parse_version_lax(&ver_str);
    }

    if parsed_bound.is_none() {
        return VersionConstraintResult {
            satisfies: false,
            parsed_version: Some(parsed_ver),
            parsed_constraint: None,
            scheme: scheme.to_string(),
            explanation: format!("Invalid version in constraint: '{}'", ver_str),
            findings: vec![format!(
                "Could not parse version '{}' in constraint",
                ver_str
            )],
        };
    }

    let parsed_bound = parsed_bound.unwrap_or_else(|| unreachable!("checked is_none() above"));
    let satisfies = evaluate_component(&parsed_ver, &op, &parsed_bound);
    let pc = ParsedConstraint {
        raw: constraint.to_string(),
        scheme: scheme.to_string(),
        components: vec![ParsedConstraintComponent {
            operator: op,
            version: parsed_bound,
        }],
        constraint_type: "comparison".to_string(),
    };
    VersionConstraintResult {
        satisfies,
        parsed_version: Some(parsed_ver),
        parsed_constraint: Some(pc),
        scheme: scheme.to_string(),
        explanation: if satisfies {
            format!("{} satisfies {}", version, constraint)
        } else {
            format!("{} does not satisfy {}", version, constraint)
        },
        findings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pr(s: &str) -> Vec<String> {
        parse_pre_release_identifiers(s)
    }

    #[test]
    fn test_prerelease_dev_before_alpha() {
        let result = compare_pre_release(&pr("dev"), &pr("alpha"));
        assert!(result < 0, "dev should be less than alpha, got {}", result);
    }

    #[test]
    fn test_prerelease_pre_before_rc() {
        let result = compare_pre_release(&pr("pre.1"), &pr("rc.1"));
        assert!(result < 0, "pre.1 should be less than rc.1, got {}", result);
    }

    #[test]
    fn test_prerelease_snapshot_before_beta() {
        let result = compare_pre_release(&pr("snapshot"), &pr("beta"));
        assert!(
            result < 0,
            "snapshot should be less than beta, got {}",
            result
        );
    }

    #[test]
    fn test_prerelease_dev_ordering_chain() {
        let dev = compare_pre_release(&pr("dev"), &pr("alpha"));
        let alpha = compare_pre_release(&pr("alpha"), &pr("beta"));
        let beta = compare_pre_release(&pr("beta"), &pr("rc"));
        assert!(dev < 0, "dev < alpha failed: got {}", dev);
        assert!(alpha < 0, "alpha < beta failed: got {}", alpha);
        assert!(beta < 0, "beta < rc failed: got {}", beta);
    }
}
