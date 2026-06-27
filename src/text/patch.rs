use regex::Regex;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

const MAX_PATCH_LENGTH: usize = 200_000;
const MAX_ORIGINAL_LENGTH: usize = 200_000;

#[derive(Debug, Clone, Serialize)]
pub struct PatchHunk {
    pub old_start: usize,
    pub old_count: usize,
    pub new_start: usize,
    pub new_count: usize,
    pub header_line: String,
    pub lines: Vec<String>,
    pub raw: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PatchFile {
    pub old_file: String,
    pub new_file: String,
    pub hunks: Vec<PatchHunk>,
    pub raw: String,
}

#[derive(Debug, Serialize)]
pub struct PatchParseResult {
    pub ok: bool,
    pub files: Vec<PatchFile>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FailedHunk {
    pub hunk_index: usize,
    pub old_start: usize,
    pub old_count: usize,
    pub expected_context: Vec<String>,
    pub actual_context: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct PatchApplyCheckResult {
    pub patch_parse_ok: bool,
    pub applies: bool,
    pub hunks_total: usize,
    pub hunks_applied: usize,
    pub hunks_failed: usize,
    pub failed_hunks: Vec<FailedHunk>,
    pub affected_line_ranges: Vec<LineRange>,
    pub newline_style_before: String,
    pub newline_style_after: String,
    pub result_fingerprint: String,
    pub result_text: Option<String>,
    pub findings: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct LineRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Serialize)]
pub struct PatchSummaryResult {
    pub files_changed: usize,
    pub hunks_total: usize,
    pub additions: usize,
    pub deletions: usize,
    pub renames_detected: Vec<RenamePair>,
    pub binary_patch_detected: bool,
    pub line_ranges_by_file: HashMap<String, Vec<LineRange>>,
    pub findings: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct RenamePair {
    pub from: String,
    pub to: String,
}

fn detect_newline_style(text: &str) -> String {
    let has_crlf = text.contains("\r\n");
    let standalone_cr = text.matches('\r').count() - text.matches("\r\n").count();
    let standalone_lf = text.matches('\n').count() - text.matches("\r\n").count();

    if has_crlf && (standalone_cr > 0 || standalone_lf > 0) {
        "mixed".to_string()
    } else if standalone_cr > 0 && standalone_lf > 0 {
        "mixed".to_string()
    } else if has_crlf {
        "CRLF".to_string()
    } else if standalone_cr > 0 {
        "CR".to_string()
    } else if standalone_lf > 0 {
        "LF".to_string()
    } else {
        "none".to_string()
    }
}

fn parse_hunk_header(line: &str) -> Option<(usize, usize, usize, usize)> {
    let re = Regex::new(r"^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@").ok()?;
    let caps = re.captures(line)?;
    let old_start: usize = caps.get(1)?.as_str().parse().ok()?;
    let old_count: usize = caps
        .get(2)
        .map_or(Some(1), |m| m.as_str().parse().ok())
        .unwrap_or(1);
    let new_start: usize = caps.get(3)?.as_str().parse().ok()?;
    let new_count: usize = caps
        .get(4)
        .map_or(Some(1), |m| m.as_str().parse().ok())
        .unwrap_or(1);
    Some((old_start, old_count, new_start, new_count))
}

pub fn parse_unified_diff(patch_text: &str) -> PatchParseResult {
    if patch_text.trim().is_empty() {
        return PatchParseResult {
            ok: false,
            files: vec![],
            error: Some("Empty patch text".to_string()),
        };
    }

    let mut files: Vec<PatchFile> = vec![];
    let lines: Vec<&str> = patch_text.split('\n').collect();
    let mut i: usize = 0;
    let mut current_old_file = String::new();
    let mut current_new_file = String::new();
    let mut current_hunks: Vec<PatchHunk> = vec![];
    let mut current_hunk_lines: Vec<String> = vec![];
    let mut current_hunk_header = String::new();
    let mut current_hunk_info: Option<(usize, usize, usize, usize)> = None;
    let mut in_hunk = false;

    while i < lines.len() {
        let line = lines[i];

        if line.starts_with("--- ") || line.starts_with("+++ ") {
            if in_hunk {
                if let Some((old_s, old_c, new_s, new_c)) = current_hunk_info {
                    let raw = format!("{}\n{}", current_hunk_header, current_hunk_lines.join("\n"));
                    current_hunks.push(PatchHunk {
                        old_start: old_s,
                        old_count: old_c,
                        new_start: new_s,
                        new_count: new_c,
                        header_line: current_hunk_header.clone(),
                        lines: current_hunk_lines.clone(),
                        raw,
                    });
                    in_hunk = false;
                    current_hunk_lines = vec![];
                    current_hunk_info = None;
                }
            }

            if let Some(rest) = line.strip_prefix("--- ") {
                current_old_file = rest.trim().to_string();
                if current_old_file == "/dev/null" {
                    current_old_file = String::new();
                }
            } else if let Some(rest) = line.strip_prefix("+++ ") {
                current_new_file = rest.trim().to_string();
                if current_new_file == "/dev/null" {
                    current_new_file = String::new();
                }
            }
        } else if line.starts_with("@@ ") {
            if in_hunk {
                if let Some((old_s, old_c, new_s, new_c)) = current_hunk_info {
                    let raw = format!("{}\n{}", current_hunk_header, current_hunk_lines.join("\n"));
                    current_hunks.push(PatchHunk {
                        old_start: old_s,
                        old_count: old_c,
                        new_start: new_s,
                        new_count: new_c,
                        header_line: current_hunk_header.clone(),
                        lines: current_hunk_lines.clone(),
                        raw,
                    });
                    current_hunk_lines = vec![];
                }
            }

            if let Some(parsed) = parse_hunk_header(line) {
                current_hunk_info = Some(parsed);
                current_hunk_header = line.to_string();
                in_hunk = true;
            } else if in_hunk {
                current_hunk_lines.push(line.to_string());
            }
        } else if in_hunk {
            current_hunk_lines.push(line.to_string());
        }

        i += 1;
    }

    if in_hunk {
        if let Some((old_s, old_c, new_s, new_c)) = current_hunk_info {
            let raw = format!("{}\n{}", current_hunk_header, current_hunk_lines.join("\n"));
            current_hunks.push(PatchHunk {
                old_start: old_s,
                old_count: old_c,
                new_start: new_s,
                new_count: new_c,
                header_line: current_hunk_header,
                lines: current_hunk_lines,
                raw,
            });
        }
    }

    if !current_old_file.is_empty() || !current_new_file.is_empty() || !current_hunks.is_empty() {
        files.push(PatchFile {
            old_file: current_old_file,
            new_file: current_new_file,
            hunks: current_hunks,
            raw: patch_text.to_string(),
        });
    }

    if files.is_empty() {
        return PatchParseResult {
            ok: false,
            files: vec![],
            error: Some(
                "No unified diff headers found (-- a/... / +++ b/... or @@ ... @@)".to_string(),
            ),
        };
    }

    PatchParseResult {
        ok: true,
        files,
        error: None,
    }
}

fn text_to_lines(text: &str) -> Vec<String> {
    let mut result = text.to_string();
    if result.ends_with('\n') {
        result.pop();
    }
    if result.ends_with('\r') {
        result.pop();
    }
    result.split('\n').map(|s| s.to_string()).collect()
}

fn lines_to_text(lines: &[String]) -> String {
    lines.join("\n")
}

fn normalize_line(line: &str) -> String {
    line.trim_end_matches('\r').to_string()
}

fn strip_line_prefix(line: &str) -> String {
    if line.starts_with('+') {
        line[1..].to_string()
    } else if line.starts_with('-') {
        line[1..].to_string()
    } else if line.starts_with(' ') {
        line[1..].to_string()
    } else {
        line.to_string()
    }
}

fn fingerprint(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn apply_hunk(
    original_lines: &[String],
    hunk: &PatchHunk,
    strict: bool,
) -> (Option<Vec<String>>, Option<String>) {
    let old_start = hunk.old_start.saturating_sub(1);
    let old_count = hunk.old_count;

    if old_start > original_lines.len() {
        return (
            None,
            Some(format!("Invalid hunk start: {}", hunk.old_start)),
        );
    }

    let mut expected_context: Vec<String> = vec![];
    for hline in &hunk.lines {
        let normalized = normalize_line(hline);
        if normalized.starts_with(' ') || normalized.starts_with('-') {
            expected_context.push(strip_line_prefix(&normalized));
        }
    }

    let actual_end = old_start.saturating_add(old_count);
    let actual_context: Vec<String> = if actual_end > original_lines.len() {
        if strict {
            return (
                None,
                Some(format!(
                    "Hunk references lines {}-{} but original has only {} lines",
                    hunk.old_start,
                    hunk.old_start.saturating_add(old_count).saturating_sub(1),
                    original_lines.len()
                )),
            );
        }
        original_lines[old_start..].to_vec()
    } else {
        original_lines[old_start..actual_end].to_vec()
    };

    if strict && expected_context.len() != actual_context.len() {
        return (
            None,
            Some(format!(
                "Context length mismatch: hunk expects {} lines, actual has {} lines",
                expected_context.len(),
                actual_context.len()
            )),
        );
    }

    if strict {
        for (idx, (expected, actual)) in expected_context
            .iter()
            .zip(actual_context.iter())
            .enumerate()
        {
            if normalize_line(expected) != normalize_line(actual) {
                return (
                    None,
                    Some(format!(
                        "Context mismatch at line {}: expected {:?}, got {:?}",
                        hunk.old_start + idx,
                        normalize_line(expected),
                        normalize_line(actual)
                    )),
                );
            }
        }
    }

    let mut new_lines: Vec<String> = vec![];
    let mut new_idx = 0;
    let mut hunk_idx = 0;

    while hunk_idx < hunk.lines.len() {
        let hline = normalize_line(&hunk.lines[hunk_idx]);
        if hline.starts_with(' ') {
            if new_idx < original_lines.len() {
                new_lines.push(original_lines[new_idx].clone());
            } else {
                new_lines.push(strip_line_prefix(&hline));
            }
            new_idx += 1;
            hunk_idx += 1;
        } else if hline.starts_with('-') {
            new_idx += 1;
            hunk_idx += 1;
        } else if hline.starts_with('+') {
            new_lines.push(strip_line_prefix(&hline));
            hunk_idx += 1;
        } else if hline.starts_with('\\') {
            hunk_idx += 1;
        } else {
            hunk_idx += 1;
        }
    }

    while new_idx < original_lines.len() {
        new_lines.push(original_lines[new_idx].clone());
        new_idx += 1;
    }

    (Some(new_lines), None)
}

pub fn patch_apply_check(
    original_text: &str,
    patch_text: &str,
    strict: bool,
    return_result_fingerprint: bool,
    return_result_text: bool,
) -> PatchApplyCheckResult {
    let mut findings: Vec<String> = vec![];
    let mut failed_hunks: Vec<FailedHunk> = vec![];
    let mut affected_line_ranges: Vec<LineRange> = vec![];

    if original_text.len() > MAX_ORIGINAL_LENGTH {
        return PatchApplyCheckResult {
            patch_parse_ok: false,
            applies: false,
            hunks_total: 0,
            hunks_applied: 0,
            hunks_failed: 0,
            failed_hunks: vec![],
            affected_line_ranges: vec![],
            newline_style_before: detect_newline_style(original_text),
            newline_style_after: detect_newline_style(original_text),
            result_fingerprint: String::new(),
            result_text: None,
            findings: vec![format!(
                "Original text exceeds maximum length of {}",
                MAX_ORIGINAL_LENGTH
            )],
        };
    }

    if patch_text.len() > MAX_PATCH_LENGTH {
        return PatchApplyCheckResult {
            patch_parse_ok: false,
            applies: false,
            hunks_total: 0,
            hunks_applied: 0,
            hunks_failed: 0,
            failed_hunks: vec![],
            affected_line_ranges: vec![],
            newline_style_before: detect_newline_style(original_text),
            newline_style_after: detect_newline_style(original_text),
            result_fingerprint: String::new(),
            result_text: None,
            findings: vec![format!(
                "Patch text exceeds maximum length of {}",
                MAX_PATCH_LENGTH
            )],
        };
    }

    let newline_before = detect_newline_style(original_text);

    let parse_result = parse_unified_diff(patch_text);
    if !parse_result.ok {
        return PatchApplyCheckResult {
            patch_parse_ok: false,
            applies: false,
            hunks_total: 0,
            hunks_applied: 0,
            hunks_failed: 0,
            failed_hunks: vec![],
            affected_line_ranges: vec![],
            newline_style_before: newline_before.clone(),
            newline_style_after: newline_before.clone(),
            result_fingerprint: String::new(),
            result_text: None,
            findings: vec![format!(
                "Failed to parse patch: {}",
                parse_result.error.unwrap_or_default()
            )],
        };
    }

    let original_lines = text_to_lines(original_text);
    let mut all_hunks: Vec<&PatchHunk> = vec![];
    for file_entry in &parse_result.files {
        for hunk in &file_entry.hunks {
            all_hunks.push(hunk);
        }
    }

    let hunks_total = all_hunks.len();
    if hunks_total == 0 {
        return PatchApplyCheckResult {
            patch_parse_ok: true,
            applies: true,
            hunks_total: 0,
            hunks_applied: 0,
            hunks_failed: 0,
            failed_hunks: vec![],
            affected_line_ranges: vec![],
            newline_style_before: newline_before.clone(),
            newline_style_after: newline_before.clone(),
            result_fingerprint: if return_result_fingerprint {
                fingerprint(original_text)
            } else {
                String::new()
            },
            result_text: if return_result_text {
                Some(original_text.to_string())
            } else {
                None
            },
            findings: vec!["No hunks found in patch".to_string()],
        };
    }

    let mut current_lines = original_lines.clone();
    let mut hunks_applied = 0;
    let mut hunks_failed = 0;

    for (hunk_idx, hunk) in all_hunks.iter().enumerate() {
        let (result, error) = apply_hunk(&current_lines, hunk, strict);
        if let Some(new_lines) = result {
            current_lines = new_lines;
            hunks_applied += 1;
            affected_line_ranges.push(LineRange {
                start: hunk.new_start,
                end: hunk
                    .new_start
                    .saturating_add(hunk.new_count)
                    .saturating_sub(1),
            });
        } else {
            hunks_failed += 1;
            let expected_ctx: Vec<String> = hunk
                .lines
                .iter()
                .filter(|line| {
                    let normalized = normalize_line(line);
                    normalized.starts_with(' ') || normalized.starts_with('-')
                })
                .map(|line| strip_line_prefix(&normalize_line(line)))
                .collect();
            let actual_end = std::cmp::min(
                hunk.old_start
                    .saturating_sub(1)
                    .saturating_add(hunk.old_count),
                current_lines.len(),
            );
            let actual_ctx: Vec<String> = if hunk.old_start.saturating_sub(1) < current_lines.len()
            {
                current_lines[hunk.old_start.saturating_sub(1)..actual_end].to_vec()
            } else {
                vec![]
            };

            failed_hunks.push(FailedHunk {
                hunk_index: hunk_idx,
                old_start: hunk.old_start,
                old_count: hunk.old_count,
                expected_context: expected_ctx,
                actual_context: actual_ctx,
                reason: error.unwrap_or_else(|| "Unknown error".to_string()),
            });
        }
    }

    let applies = hunks_failed == 0;
    let result_text = if return_result_text {
        Some(lines_to_text(&current_lines))
    } else {
        None
    };
    let result_text_ref = result_text.as_deref().unwrap_or(original_text);
    let newline_after = detect_newline_style(result_text_ref);

    if hunks_failed > 0 {
        findings.push(format!(
            "{} of {} hunks failed to apply",
            hunks_failed, hunks_total
        ));
    }

    let result_fingerprint = if return_result_fingerprint {
        fingerprint(result_text_ref)
    } else {
        String::new()
    };

    PatchApplyCheckResult {
        patch_parse_ok: true,
        applies,
        hunks_total,
        hunks_applied,
        hunks_failed,
        failed_hunks,
        affected_line_ranges,
        newline_style_before: newline_before,
        newline_style_after: newline_after,
        result_fingerprint,
        result_text,
        findings,
    }
}

pub fn patch_summary(patch_text: &str) -> PatchSummaryResult {
    let mut findings: Vec<String> = vec![];

    if patch_text.len() > MAX_PATCH_LENGTH {
        return PatchSummaryResult {
            files_changed: 0,
            hunks_total: 0,
            additions: 0,
            deletions: 0,
            renames_detected: vec![],
            binary_patch_detected: false,
            line_ranges_by_file: HashMap::new(),
            findings: vec![format!(
                "Patch text exceeds maximum length of {}",
                MAX_PATCH_LENGTH
            )],
        };
    }

    let parse_result = parse_unified_diff(patch_text);

    if !parse_result.ok {
        return PatchSummaryResult {
            files_changed: 0,
            hunks_total: 0,
            additions: 0,
            deletions: 0,
            renames_detected: vec![],
            binary_patch_detected: false,
            line_ranges_by_file: HashMap::new(),
            findings: vec![format!(
                "Failed to parse patch: {}",
                parse_result.error.unwrap_or_default()
            )],
        };
    }

    let files_changed = parse_result.files.len();
    let mut hunks_total = 0;
    let mut additions = 0;
    let mut deletions = 0;
    let mut renames_detected: Vec<RenamePair> = vec![];
    let mut binary_patch_detected = false;
    let mut line_ranges_by_file: HashMap<String, Vec<LineRange>> = HashMap::new();

    for file_entry in &parse_result.files {
        let old_file = &file_entry.old_file;
        let new_file = &file_entry.new_file;

        if !old_file.is_empty() && !new_file.is_empty() && old_file != new_file {
            renames_detected.push(RenamePair {
                from: old_file.clone(),
                to: new_file.clone(),
            });
        }

        let file_key = if new_file.is_empty() {
            old_file.clone()
        } else {
            new_file.clone()
        };
        let mut file_ranges: Vec<LineRange> = vec![];

        for hunk in &file_entry.hunks {
            hunks_total += 1;
            let mut hunk_additions = 0;
            let mut hunk_deletions = 0;

            for hline in &hunk.lines {
                let normalized = normalize_line(hline);
                if normalized.starts_with('+') {
                    hunk_additions += 1;
                } else if normalized.starts_with('-') {
                    hunk_deletions += 1;
                }
            }

            additions += hunk_additions;
            deletions += hunk_deletions;

            file_ranges.push(LineRange {
                start: hunk.new_start,
                end: hunk
                    .new_start
                    .saturating_add(hunk.new_count)
                    .saturating_sub(1),
            });
        }

        if !file_key.is_empty() {
            line_ranges_by_file.insert(file_key, file_ranges);
        }
    }

    if patch_text.contains("GIT binary patch") || patch_text.contains('\0') {
        binary_patch_detected = true;
        findings.push("Binary patch content detected".to_string());
    }

    if parse_result.files.is_empty() {
        findings.push("No file headers found in patch".to_string());
    }

    PatchSummaryResult {
        files_changed,
        hunks_total,
        additions,
        deletions,
        renames_detected,
        binary_patch_detected,
        line_ranges_by_file,
        findings,
    }
}
