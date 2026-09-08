//! Canonical patch analysis shared by patch-family tools.
//!
//! One parse per operation: [`analyze_patch`] owns a single
//! `parse_unified_diff` result and computes neutral facts once. The three
//! public tools project different answers from [`PatchAnalysis`]:
//!
//! - `patch_summary` is the neutral presentation;
//! - `patch_contract_check` applies contract policy (scope, categories,
//!   large deletions);
//! - `diff_risk_classify` applies review-routing policy.
//!
//! Path-role classification reuses the canonical repository classifier
//! (`super::repo`), so a manifest/lockfile/CI/generated/vendor path cannot
//! receive different bucket facts depending on which patch tool is called.
//! Policy differences stay intentional and commented at the call site: a
//! security-sensitive path is a shared fact; contract vs risk tools may
//! choose different verdicts from different policy goals.
//!
//! No `ToolResponse`, machine codes, verdicts, profiles, or MCP metadata.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::repo as repo_facts;

/// Per-file neutral facts for one changed path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchFileFacts {
    /// Canonical effective path (`new` unless deleted, else `old`).
    pub effective_path: String,
    pub old_path: String,
    pub new_path: String,
    /// Canonical bucket from [`repo_facts::classify_path`].
    pub bucket: String,
    pub is_manifest: bool,
    pub is_lockfile: bool,
    pub is_ci: bool,
    pub is_config: bool,
    pub is_generated: bool,
    pub is_vendor: bool,
    pub is_security_sensitive: bool,
    pub is_docs: bool,
    pub is_tests: bool,
    pub additions: usize,
    pub deletions: usize,
    pub hunks: usize,
    pub line_ranges: Vec<SimpleLineRange>,
}

/// Inclusive destination line range (mirrors `text::patch::LineRange`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimpleLineRange {
    pub start: usize,
    pub end: usize,
}

/// Rename pair (`from` -> `to`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenamePair {
    pub from: String,
    pub to: String,
}

/// Neutral patch facts computed once per operation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchAnalysis {
    pub ok: bool,
    pub error: Option<String>,
    pub files: Vec<PatchFileFacts>,
    pub files_changed: usize,
    pub hunks_total: usize,
    pub additions: usize,
    pub deletions: usize,
    pub renames_detected: Vec<RenamePair>,
    pub binary_patch_detected: bool,
    pub line_ranges_by_file: BTreeMap<String, Vec<SimpleLineRange>>,
}

fn line_range(start: usize, count: usize) -> SimpleLineRange {
    SimpleLineRange {
        start,
        end: if count == 0 {
            start
        } else {
            start.saturating_add(count - 1)
        },
    }
}

fn effective_path(old_file: &str, new_file: &str) -> String {
    if new_file.is_empty() || new_file == "/dev/null" {
        old_file.to_string()
    } else {
        new_file.to_string()
    }
}

/// Analyze `patch_text` once into neutral [`PatchAnalysis`] facts.
///
/// Deterministic and explicit-input only. Parse acceptance mirrors
/// `text::patch::parse_unified_diff`; neutral counts mirror
/// `text::patch::patch_summary` so projections cannot disagree about path
/// identity or add/delete totals.
pub fn analyze_patch(patch_text: &str) -> PatchAnalysis {
    const MAX_PATCH_LENGTH: usize = 200_000;
    if patch_text.len() > MAX_PATCH_LENGTH {
        return PatchAnalysis {
            ok: false,
            error: Some(format!(
                "Patch text exceeds maximum length of {MAX_PATCH_LENGTH}"
            )),
            files: Vec::new(),
            files_changed: 0,
            hunks_total: 0,
            additions: 0,
            deletions: 0,
            renames_detected: Vec::new(),
            binary_patch_detected: false,
            line_ranges_by_file: BTreeMap::new(),
        };
    }

    let parse = crate::text::patch::parse_unified_diff(patch_text);
    if !parse.ok {
        return PatchAnalysis {
            ok: false,
            error: parse.error,
            files: Vec::new(),
            files_changed: 0,
            hunks_total: 0,
            additions: 0,
            deletions: 0,
            renames_detected: Vec::new(),
            binary_patch_detected: false,
            line_ranges_by_file: BTreeMap::new(),
        };
    }

    let mut files = Vec::new();
    let mut hunks_total = 0usize;
    let mut additions = 0usize;
    let mut deletions = 0usize;
    let mut renames_detected = Vec::new();
    let mut line_ranges_by_file: BTreeMap<String, Vec<SimpleLineRange>> = BTreeMap::new();

    for f in &parse.files {
        let old_file = &f.old_file;
        let new_file = &f.new_file;
        if !old_file.is_empty() && !new_file.is_empty() && old_file != new_file {
            renames_detected.push(RenamePair {
                from: old_file.clone(),
                to: new_file.clone(),
            });
        }

        let eff = effective_path(old_file, new_file);
        let bucket = if eff.is_empty() {
            "source".to_string()
        } else {
            repo_facts::path_bucket(&eff)
        };

        let mut file_add = 0usize;
        let mut file_del = 0usize;
        let mut ranges = Vec::new();
        for h in &f.hunks {
            hunks_total += 1;
            let mut h_add = 0usize;
            let mut h_del = 0usize;
            for line in &h.lines {
                let normalized = line.trim_end_matches('\r');
                if normalized.starts_with('+') {
                    h_add += 1;
                } else if normalized.starts_with('-') {
                    h_del += 1;
                }
            }
            file_add += h_add;
            file_del += h_del;
            additions += h_add;
            deletions += h_del;
            ranges.push(line_range(h.new_start, h.new_count));
        }

        let file_key = if new_file.is_empty() {
            old_file.clone()
        } else {
            new_file.clone()
        };

        if !file_key.is_empty() {
            line_ranges_by_file.insert(file_key.clone(), ranges.clone());
        }

        // Skip pushing a facts record for empty keys (e.g. /dev/null-only
        // entries) to keep files/files_changed aligned with the summary.
        if eff.is_empty() && file_key.is_empty() {
            continue;
        }

        files.push(PatchFileFacts {
            effective_path: eff.clone(),
            old_path: old_file.clone(),
            new_path: new_file.clone(),
            is_manifest: bucket == "manifests",
            is_lockfile: bucket == "lockfiles",
            is_ci: bucket == "ci",
            is_config: bucket == "configs",
            is_generated: bucket == "generated",
            is_vendor: bucket == "vendor",
            is_security_sensitive: if eff.is_empty() {
                false
            } else {
                repo_facts::is_security_sensitive_path(&eff)
            },
            is_docs: bucket == "docs",
            is_tests: bucket == "tests",
            bucket,
            additions: file_add,
            deletions: file_del,
            hunks: f.hunks.len(),
            line_ranges: ranges,
        });
    }

    let binary_patch_detected =
        patch_text.contains("GIT binary patch") || patch_text.contains('\0');

    PatchAnalysis {
        ok: true,
        error: None,
        files_changed: parse.files.len(),
        files,
        hunks_total,
        additions,
        deletions,
        renames_detected,
        binary_patch_detected,
        line_ranges_by_file,
    }
}

impl PatchAnalysis {
    /// Total changed lines (additions + deletions).
    pub fn total_changes(&self) -> usize {
        self.additions + self.deletions
    }

    /// Per-file deletion counts for contract large-deletion policy.
    pub fn deletions_by_file(&self) -> Vec<(&str, usize)> {
        self.files
            .iter()
            .map(|f| (f.effective_path.as_str(), f.deletions))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_parse_neutral_counts() {
        let patch = "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-old\n+new\n";
        let a = analyze_patch(patch);
        assert!(a.ok);
        assert_eq!(a.files_changed, 1);
        assert_eq!(a.additions, 1);
        assert_eq!(a.deletions, 1);
        assert_eq!(a.files[0].effective_path, "src/lib.rs");
        assert_eq!(a.files[0].bucket, "source");
    }

    #[test]
    fn rejects_empty_patch() {
        let a = analyze_patch("");
        assert!(!a.ok);
        assert!(a.error.is_some());
    }

    #[test]
    fn classifies_manifest_ci_generated_vendor_once() {
        let patch = "--- a/Cargo.toml\n+++ b/Cargo.toml\n@@ -1 +1 @@\n-old\n+new\n--- a/.github/workflows/ci.yml\n+++ b/.github/workflows/ci.yml\n@@ -1 +1 @@\n-old\n+new\n";
        let a = analyze_patch(patch);
        assert!(a.ok);
        let manifest = a
            .files
            .iter()
            .find(|f| f.effective_path == "Cargo.toml")
            .unwrap();
        assert!(manifest.is_manifest);
        let ci = a
            .files
            .iter()
            .find(|f| f.effective_path == ".github/workflows/ci.yml")
            .unwrap();
        assert!(ci.is_ci);
    }

    #[test]
    fn security_fact_shared_with_repo() {
        let patch =
            "--- a/src/auth/handler.rs\n+++ b/src/auth/handler.rs\n@@ -1 +1 @@\n-old\n+new\n";
        let a = analyze_patch(patch);
        assert!(a.files[0].is_security_sensitive);
    }

    #[test]
    fn binary_and_rename_flags() {
        let patch = "--- a/old.txt\n+++ b/new.txt\n@@ -1 +1 @@\n-old\n+new\nGIT binary patch\n";
        let a = analyze_patch(patch);
        assert!(a.binary_patch_detected);
        assert_eq!(a.renames_detected.len(), 1);
    }

    #[test]
    fn neutral_counts_match_text_summary() {
        // Guard against drift between the canonical analysis and the
        // underlying text summary core.
        let patches = [
            "--- a/f\n+++ b/f\n@@ -1 +1 @@\n-old\n+new\n",
            "--- a/a.rs\n+++ b/a.rs\n@@ -1,3 +1,3 @@\n a\n-b\n-c\n+d\n+e\n f\n",
        ];
        for p in patches {
            let a = analyze_patch(p);
            let s = crate::text::patch_summary(p);
            assert_eq!(a.files_changed, s.files_changed);
            assert_eq!(a.hunks_total, s.hunks_total);
            assert_eq!(a.additions, s.additions);
            assert_eq!(a.deletions, s.deletions);
            assert_eq!(a.binary_patch_detected, s.binary_patch_detected);
            assert_eq!(a.renames_detected.len(), s.renames_detected.len());
        }
    }
}
