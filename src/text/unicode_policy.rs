use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicyFinding {
    pub rule: String,
    pub severity: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnicodePolicyCheckResult {
    pub pass: bool,
    pub policy: String,
    pub normalized_form: String,
    pub findings: Vec<PolicyFinding>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalizeResult {
    pub text: String,
    pub changed: bool,
    pub operations_applied: Vec<String>,
    pub fingerprint_before: String,
    pub fingerprint_after: String,
    pub findings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalizeResultWithMapping {
    #[serde(flatten)]
    pub base: CanonicalizeResult,
    pub mapping: Option<Vec<CharMapping>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharMapping {
    pub position: usize,
    pub original: Option<String>,
    pub original_codepoint: Option<String>,
    pub canonical: Option<String>,
    pub canonical_codepoint: Option<String>,
}

const VALID_POLICIES: &[&str] = &[
    "identifier_strict",
    "filename_safe",
    "source_code",
    "human_text",
    "json_key",
    "domain_like",
];

const WINDOWS_RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

const BIDI_CHARS: &[char] = &[
    '\u{200e}', '\u{200f}', '\u{202a}', '\u{202b}', '\u{202c}', '\u{202d}', '\u{202e}', '\u{2066}',
    '\u{2067}', '\u{2068}', '\u{2069}',
];

const ZERO_WIDTH_CHARS: &[char] = &['\u{200b}', '\u{200c}', '\u{200d}', '\u{2060}'];

const WIN_FORBIDDEN: &[char] = &['\\', '/', ':', '*', '?', '"', '<', '>', '|'];

/// Count occurrences of zero-width characters in text (matching Python's
/// `[c for c in normalized if c in _ZERO_WIDTH_CHARS]` which counts every
/// occurrence, not just distinct types).
fn count_zero_width_occurrences(text: &str, exclude_word_joiner: bool) -> usize {
    text.chars()
        .filter(|c| {
            if exclude_word_joiner && *c == '\u{2060}' {
                return false;
            }
            ZERO_WIDTH_CHARS.contains(c)
        })
        .count()
}

fn default_normalization(policy: &str) -> &'static str {
    match policy {
        "identifier_strict" => "NFC",
        "filename_safe" => "NFC",
        "source_code" => "NFC",
        "human_text" => "NFC",
        "json_key" => "NFC",
        "domain_like" => "NFKC",
        _ => "NFC",
    }
}

fn normalize_unicode(text: &str, form: &str) -> Result<String, String> {
    match form {
        "NFC" => Ok(text.nfc().collect()),
        "NFD" => Ok(text.nfd().collect()),
        "NFKC" => Ok(text.nfkc().collect()),
        "NFKD" => Ok(text.nfkd().collect()),
        "raw" | "" => Ok(text.to_string()),
        _ => Err(format!("Invalid normalization form: {}", form)),
    }
}

fn get_unicode_category(c: char) -> &'static str {
    let cp = c as u32;
    if cp <= 0x001F || (0x007F..=0x009F).contains(&cp) {
        return "Cc";
    }
    if (0x0300..=0x036F).contains(&cp)
        || (0x1DC0..=0x1DFF).contains(&cp)
        || (0xFE20..=0xFE2F).contains(&cp)
    {
        return "Mn";
    }
    if (0x0600..=0x06FF).contains(&cp) || (0x0750..=0x077F).contains(&cp) {
        return "Mc";
    }
    if (0x0900..=0x0DFF).contains(&cp) {
        return "Mc";
    }
    if cp == 0x200C || cp == 0x200D {
        return "Cf";
    }
    if cp == 0x200B || cp == 0x2060 {
        return "Cf";
    }
    "Lo"
}

fn find_invisibles(text: &str) -> Vec<char> {
    let invisible_chars: HashSet<char> = [
        '\u{200b}', '\u{200c}', '\u{200d}', '\u{2060}', '\u{200e}', '\u{200f}', '\u{2028}',
        '\u{2029}', '\u{202a}', '\u{202b}', '\u{202c}', '\u{202d}', '\u{202e}', '\u{2066}',
        '\u{2067}', '\u{2068}', '\u{2069}', '\u{feff}', '\u{180e}', '\u{034f}', '\u{206a}',
        '\u{206b}', '\u{206c}', '\u{206d}', '\u{206e}', '\u{206f}',
    ]
    .iter()
    .cloned()
    .collect();

    text.chars()
        .filter(|c| invisible_chars.contains(c))
        .collect()
}

fn get_script(cp: u32) -> &'static str {
    if (0x0041..=0x005A).contains(&cp) || (0x0061..=0x007A).contains(&cp) {
        return "Latin";
    }
    if (0x0030..=0x0039).contains(&cp) {
        return "Common";
    }
    if (0x0400..=0x04FF).contains(&cp) {
        return "Cyrillic";
    }
    if (0x0530..=0x058F).contains(&cp) {
        return "Armenian";
    }
    if (0x0600..=0x06FF).contains(&cp) {
        return "Arabic";
    }
    if (0x0900..=0x097F).contains(&cp) {
        return "Devanagari";
    }
    if (0x3040..=0x309F).contains(&cp) {
        return "Hiragana";
    }
    if (0x30A0..=0x30FF).contains(&cp) {
        return "Katakana";
    }
    if (0x4E00..=0x9FFF).contains(&cp) || (0x3400..=0x4DBF).contains(&cp) {
        return "Han";
    }
    if (0xAC00..=0xD7AF).contains(&cp) {
        return "Hangul";
    }
    if (0x1F300..=0x1F9FF).contains(&cp) || (0x1F600..=0x1F64F).contains(&cp) {
        return "Common";
    }
    if cp == 0x200C || cp == 0x200D {
        return "Inherited";
    }
    if (0x0300..=0x036F).contains(&cp) {
        return "Inherited";
    }
    "Unknown"
}

fn detect_mixed_scripts(text: &str) -> (bool, Vec<String>) {
    let mut scripts: HashSet<&'static str> = HashSet::new();
    for c in text.chars() {
        let cp = c as u32;
        if c.is_whitespace() || c == '\u{200b}' || c == '\u{200c}' || c == '\u{200d}' {
            continue;
        }
        let cat = get_unicode_category(c);
        if cat.starts_with('M') || cat == "Cf" {
            continue;
        }
        let script = get_script(cp);
        if matches!(script, "Unknown" | "Common" | "Inherited" | "Other") {
            continue;
        }
        scripts.insert(script);
    }

    let has_multiple = scripts.len() > 1;
    let mut script_list: Vec<String> = scripts.into_iter().map(String::from).collect();
    script_list.sort();
    (has_multiple, script_list)
}

fn detect_confusables(text: &str) -> Vec<(char, &'static str)> {
    use crate::text::confusables::CONFUSABLES;

    text.chars()
        .filter_map(|c| {
            let key = format!("U+{:04X}", c as u32);
            CONFUSABLES.get(key.as_str()).map(|sub| (c, *sub))
        })
        .collect()
}

pub fn unicode_policy_check(
    text: &str,
    policy: &str,
    normalization: Option<&str>,
) -> UnicodePolicyCheckResult {
    if !VALID_POLICIES.contains(&policy) {
        let valid = VALID_POLICIES.join(", ");
        return UnicodePolicyCheckResult {
            pass: false,
            policy: policy.to_string(),
            normalized_form: String::new(),
            findings: vec![PolicyFinding {
                rule: "invalid_policy".to_string(),
                severity: "error".to_string(),
                message: format!("Unknown policy: {}. Valid policies: {}", policy, valid),
            }],
            summary: format!("Invalid policy: {}", policy),
        };
    }

    if text.len() > 100_000 {
        return UnicodePolicyCheckResult {
            pass: false,
            policy: policy.to_string(),
            normalized_form: String::new(),
            findings: vec![PolicyFinding {
                rule: "input_too_large".to_string(),
                severity: "error".to_string(),
                message: format!("Input length {} exceeds maximum 100000", text.len()),
            }],
            summary: "Input too large".to_string(),
        };
    }

    let norm_form = if let Some(n) = normalization {
        if n == "raw" {
            ""
        } else {
            n
        }
    } else {
        default_normalization(policy)
    };

    let normalized = if norm_form.is_empty() {
        text.to_string()
    } else {
        match normalize_unicode(text, norm_form) {
            Ok(n) => n,
            Err(e) => {
                return UnicodePolicyCheckResult {
                    pass: false,
                    policy: policy.to_string(),
                    normalized_form: String::new(),
                    findings: vec![PolicyFinding {
                        rule: "invalid_normalization".to_string(),
                        severity: "error".to_string(),
                        message: e,
                    }],
                    summary: format!("Invalid normalization: {}", norm_form),
                };
            }
        }
    };

    let mut findings: Vec<PolicyFinding> = Vec::new();

    match policy {
        "identifier_strict" => findings.extend(check_identifier_strict(text, &normalized)),
        "filename_safe" => findings.extend(check_filename_safe(text, &normalized)),
        "source_code" => findings.extend(check_source_code(text, &normalized)),
        "human_text" => findings.extend(check_human_text(text, &normalized)),
        "json_key" => findings.extend(check_json_key(text, &normalized)),
        "domain_like" => findings.extend(check_domain_like(text, &normalized)),
        _ => {}
    }

    let errors: Vec<_> = findings.iter().filter(|f| f.severity == "error").collect();
    let pass = errors.is_empty();

    let mut summary_parts: Vec<String> = Vec::new();
    if pass {
        summary_parts.push(format!("PASS ({})", policy));
    } else {
        summary_parts.push(format!("FAIL ({})", policy));
        summary_parts.push(format!("{} error(s)", errors.len()));
    }

    let warnings: Vec<_> = findings
        .iter()
        .filter(|f| f.severity == "warning")
        .collect();
    if !warnings.is_empty() {
        summary_parts.push(format!("{} warning(s)", warnings.len()));
    }

    UnicodePolicyCheckResult {
        pass,
        policy: policy.to_string(),
        normalized_form: normalized,
        findings,
        summary: summary_parts.join("; "),
    }
}

fn check_identifier_strict(_text: &str, normalized: &str) -> Vec<PolicyFinding> {
    let mut findings: Vec<PolicyFinding> = Vec::new();

    let (mixed, scripts) = detect_mixed_scripts(normalized);
    if mixed {
        findings.push(PolicyFinding {
            rule: "mixed_scripts".to_string(),
            severity: "error".to_string(),
            message: format!("Mixed scripts detected: {}", scripts.join(", ")),
        });
    }

    let bidi_found: Vec<char> = BIDI_CHARS
        .iter()
        .filter(|c| normalized.contains(**c))
        .cloned()
        .collect();
    if !bidi_found.is_empty() {
        findings.push(PolicyFinding {
            rule: "bidi_controls".to_string(),
            severity: "error".to_string(),
            message: format!("Bidi control characters found: {}", bidi_found.len()),
        });
    }

    let zw_count = count_zero_width_occurrences(normalized, false);
    if zw_count > 0 {
        findings.push(PolicyFinding {
            rule: "zero_width_characters".to_string(),
            severity: "error".to_string(),
            message: format!("Zero-width characters found: {}", zw_count),
        });
    }

    let confusables = detect_confusables(normalized);
    if !confusables.is_empty() {
        findings.push(PolicyFinding {
            rule: "confusables".to_string(),
            severity: "error".to_string(),
            message: format!("Confusable characters found: {}", confusables.len()),
        });
    }

    // Normalization instability (NFC != NFD form)
    // `normalized` is already NFC-normalized at this point.
    // Check if the NFD form differs — meaning the text contains precomposed
    // characters that have decomposed equivalents.
    {
        let nfd_form: String = normalized.nfd().collect();
        if nfd_form != *normalized {
            findings.push(PolicyFinding {
                rule: "normalization_instability".to_string(),
                severity: "warning".to_string(),
                message: "Text has different forms under NFC vs NFD normalization".to_string(),
            });
        }
    }

    let invisibles = find_invisibles(normalized);
    if !invisibles.is_empty() {
        findings.push(PolicyFinding {
            rule: "invisible_characters".to_string(),
            severity: "error".to_string(),
            message: format!("Invisible characters found: {}", invisibles.len()),
        });
    }

    findings
}

fn check_filename_safe(_text: &str, normalized: &str) -> Vec<PolicyFinding> {
    let mut findings: Vec<PolicyFinding> = Vec::new();

    for (i, c) in normalized.chars().enumerate() {
        let cat = get_unicode_category(c);
        if cat.starts_with('C') && c != '\n' && c != '\t' && c != '\r' {
            findings.push(PolicyFinding {
                rule: "control_characters".to_string(),
                severity: "error".to_string(),
                message: format!("Control character at position {}: U+{:04X}", i, c as u32),
            });
        }
    }

    let forbidden_found: Vec<char> = WIN_FORBIDDEN
        .iter()
        .filter(|c| normalized.contains(**c))
        .cloned()
        .collect();
    if !forbidden_found.is_empty() {
        let unique: Vec<char> = forbidden_found
            .iter()
            .cloned()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        findings.push(PolicyFinding {
            rule: "path_separators".to_string(),
            severity: "error".to_string(),
            message: format!(
                "Forbidden path characters found: {}",
                unique
                    .iter()
                    .map(|c| format!("{:?}", c))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        });
    }

    let bidi_found: Vec<char> = BIDI_CHARS
        .iter()
        .filter(|c| normalized.contains(**c))
        .cloned()
        .collect();
    if !bidi_found.is_empty() {
        findings.push(PolicyFinding {
            rule: "bidi_controls".to_string(),
            severity: "error".to_string(),
            message: format!("Bidi control characters found: {}", bidi_found.len()),
        });
    }

    let zw_count = count_zero_width_occurrences(normalized, false);
    if zw_count > 0 {
        findings.push(PolicyFinding {
            rule: "zero_width_characters".to_string(),
            severity: "error".to_string(),
            message: format!("Zero-width characters found: {}", zw_count),
        });
    }

    let stem = normalized.split('.').next().unwrap_or("").to_uppercase();
    if WINDOWS_RESERVED.contains(&stem.as_str()) {
        findings.push(PolicyFinding {
            rule: "reserved_windows_name".to_string(),
            severity: "error".to_string(),
            message: format!("Reserved Windows device name: {}", stem),
        });
    }

    findings
}

fn check_source_code(_text: &str, normalized: &str) -> Vec<PolicyFinding> {
    let mut findings: Vec<PolicyFinding> = Vec::new();

    let bidi_found: Vec<char> = BIDI_CHARS
        .iter()
        .filter(|c| normalized.contains(**c))
        .cloned()
        .collect();
    if !bidi_found.is_empty() {
        findings.push(PolicyFinding {
            rule: "bidi_controls".to_string(),
            severity: "error".to_string(),
            message: format!("Bidi control characters found: {}", bidi_found.len()),
        });
    }

    let zw_count = count_zero_width_occurrences(normalized, true);
    if zw_count > 0 {
        findings.push(PolicyFinding {
            rule: "zero_width_characters".to_string(),
            severity: "error".to_string(),
            message: format!("Zero-width characters found: {}", zw_count),
        });
    }

    let confusables = detect_confusables(normalized);
    if !confusables.is_empty() {
        findings.push(PolicyFinding {
            rule: "confusables".to_string(),
            severity: "warning".to_string(),
            message: format!("Confusable characters found: {}", confusables.len()),
        });
    }

    findings
}

fn check_human_text(_text: &str, normalized: &str) -> Vec<PolicyFinding> {
    let mut findings: Vec<PolicyFinding> = Vec::new();

    let bidi_found: Vec<char> = BIDI_CHARS
        .iter()
        .filter(|c| normalized.contains(**c))
        .cloned()
        .collect();
    if !bidi_found.is_empty() {
        findings.push(PolicyFinding {
            rule: "bidi_controls".to_string(),
            severity: "warning".to_string(),
            message: format!("Bidi control characters found: {}", bidi_found.len()),
        });
    }

    let zw_count = count_zero_width_occurrences(normalized, false);
    if zw_count > 0 {
        findings.push(PolicyFinding {
            rule: "zero_width_characters".to_string(),
            severity: "warning".to_string(),
            message: format!("Zero-width characters found: {}", zw_count),
        });
    }

    let (mixed, scripts) = detect_mixed_scripts(normalized);
    if mixed {
        findings.push(PolicyFinding {
            rule: "mixed_scripts".to_string(),
            severity: "warning".to_string(),
            message: format!("Mixed scripts detected: {}", scripts.join(", ")),
        });
    }

    let confusables = detect_confusables(normalized);
    if !confusables.is_empty() {
        findings.push(PolicyFinding {
            rule: "confusables".to_string(),
            severity: "warning".to_string(),
            message: format!("Confusable characters found: {}", confusables.len()),
        });
    }

    findings
}

fn check_json_key(_text: &str, normalized: &str) -> Vec<PolicyFinding> {
    let mut findings: Vec<PolicyFinding> = Vec::new();

    let bidi_found: Vec<char> = BIDI_CHARS
        .iter()
        .filter(|c| normalized.contains(**c))
        .cloned()
        .collect();
    if !bidi_found.is_empty() {
        findings.push(PolicyFinding {
            rule: "bidi_controls".to_string(),
            severity: "error".to_string(),
            message: format!("Bidi control characters found: {}", bidi_found.len()),
        });
    }

    let zw_count = count_zero_width_occurrences(normalized, false);
    if zw_count > 0 {
        findings.push(PolicyFinding {
            rule: "zero_width_characters".to_string(),
            severity: "error".to_string(),
            message: format!("Zero-width characters found: {}", zw_count),
        });
    }

    let confusables = detect_confusables(normalized);
    if !confusables.is_empty() {
        findings.push(PolicyFinding {
            rule: "confusables".to_string(),
            severity: "warning".to_string(),
            message: format!("Confusable characters found: {}", confusables.len()),
        });
    }

    for (i, c) in normalized.chars().enumerate() {
        let cat = get_unicode_category(c);
        if cat.starts_with('C') && c != '\n' && c != '\t' && c != '\r' {
            findings.push(PolicyFinding {
                rule: "control_characters".to_string(),
                severity: "error".to_string(),
                message: format!("Control character at position {}: U+{:04X}", i, c as u32),
            });
        }
    }

    findings
}

fn check_domain_like(_text: &str, normalized: &str) -> Vec<PolicyFinding> {
    let mut findings: Vec<PolicyFinding> = Vec::new();

    let (mixed, scripts) = detect_mixed_scripts(normalized);
    if mixed {
        findings.push(PolicyFinding {
            rule: "mixed_scripts".to_string(),
            severity: "error".to_string(),
            message: format!("Mixed scripts detected: {}", scripts.join(", ")),
        });
    }

    let confusables = detect_confusables(normalized);
    if !confusables.is_empty() {
        findings.push(PolicyFinding {
            rule: "confusables".to_string(),
            severity: "error".to_string(),
            message: format!("Confusable characters found: {}", confusables.len()),
        });
    }

    let bidi_found: Vec<char> = BIDI_CHARS
        .iter()
        .filter(|c| normalized.contains(**c))
        .cloned()
        .collect();
    if !bidi_found.is_empty() {
        findings.push(PolicyFinding {
            rule: "bidi_controls".to_string(),
            severity: "error".to_string(),
            message: format!("Bidi control characters found: {}", bidi_found.len()),
        });
    }

    let zw_count = count_zero_width_occurrences(normalized, false);
    if zw_count > 0 {
        findings.push(PolicyFinding {
            rule: "zero_width_characters".to_string(),
            severity: "error".to_string(),
            message: format!("Zero-width characters found: {}", zw_count),
        });
    }

    findings
}

const VALID_PROFILES: &[&str] = &[
    "source_file_identity",
    "identifier_compare",
    "human_label_compare",
    "json_key_compare",
    "path_segment_compare",
];

fn fingerprint(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn canonicalize_text(
    text: &str,
    profile: &str,
    return_mapping: bool,
) -> CanonicalizeResultWithMapping {
    if !VALID_PROFILES.contains(&profile) {
        let valid = VALID_PROFILES.join(", ");
        return CanonicalizeResultWithMapping {
            base: CanonicalizeResult {
                text: text.to_string(),
                changed: false,
                operations_applied: vec![],
                fingerprint_before: String::new(),
                fingerprint_after: String::new(),
                findings: vec![format!(
                    "Invalid profile: {}. Valid profiles: {}",
                    profile, valid
                )],
            },
            mapping: None,
        };
    }

    let fp_before = fingerprint(text);

    let (current_text, operations, findings) = match profile {
        "source_file_identity" => canonicalize_source_file_identity(text),
        "identifier_compare" => canonicalize_identifier_compare(text),
        "human_label_compare" => canonicalize_human_label_compare(text),
        "json_key_compare" => canonicalize_json_key_compare(text),
        "path_segment_compare" => canonicalize_path_segment_compare(text),
        _ => (text.to_string(), vec![], vec![]),
    };

    let fp_after = fingerprint(&current_text);
    let changed = current_text != text;

    let mapping = if return_mapping && changed {
        Some(build_char_mapping(text, &current_text))
    } else {
        None
    };

    CanonicalizeResultWithMapping {
        base: CanonicalizeResult {
            text: current_text,
            changed,
            operations_applied: operations,
            fingerprint_before: fp_before,
            fingerprint_after: fp_after,
            findings,
        },
        mapping,
    }
}

fn build_char_mapping(original: &str, canonical: &str) -> Vec<CharMapping> {
    let mut mapping: Vec<CharMapping> = Vec::new();
    let max_len = original.chars().count().max(canonical.chars().count());

    let orig_chars: Vec<char> = original.chars().collect();
    let canon_chars: Vec<char> = canonical.chars().collect();

    for i in 0..max_len {
        let orig_char = orig_chars.get(i).copied();
        let canon_char = canon_chars.get(i).copied();

        if orig_char != canon_char {
            let entry = CharMapping {
                position: i,
                original: orig_char.map(|c| c.to_string()),
                original_codepoint: orig_char.map(|c| format!("U+{:04X}", c as u32)),
                canonical: canon_char.map(|c| c.to_string()),
                canonical_codepoint: canon_char.map(|c| format!("U+{:04X}", c as u32)),
            };
            mapping.push(entry);
        }
    }

    mapping
}

fn canonicalize_source_file_identity(text: &str) -> (String, Vec<String>, Vec<String>) {
    let mut ops: Vec<String> = Vec::new();
    let findings: Vec<String> = Vec::new();
    let mut current = text.to_string();

    let nfc = current.nfc().collect::<String>();
    if nfc != current {
        current = nfc;
        ops.push("NFC".to_string());
    }

    let lf = current.replace("\r\n", "\n").replace("\r", "\n");
    if lf != current {
        current = lf;
        ops.push("LF_newlines".to_string());
    }

    let lines: Vec<&str> = current.split('\n').collect();
    let stripped: Vec<String> = lines.iter().map(|l| l.trim_end().to_string()).collect();
    let new_text = stripped.join("\n");
    if new_text != current {
        current = new_text;
        ops.push("strip_trailing_whitespace".to_string());
    }

    if !current.ends_with('\n') {
        current.push('\n');
        ops.push("ensure_final_newline".to_string());
    } else {
        while current.ends_with("\n\n") {
            current.pop();
        }
        if !current.ends_with('\n') {
            current.push('\n');
        }
    }

    (current, ops, findings)
}

fn canonicalize_identifier_compare(text: &str) -> (String, Vec<String>, Vec<String>) {
    let mut ops: Vec<String> = Vec::new();
    let findings: Vec<String> = Vec::new();
    let mut current = text.to_string();

    let nfc = current.nfc().collect::<String>();
    if nfc != current {
        current = nfc;
        ops.push("NFC".to_string());
    }

    let folded = current.to_lowercase();
    if folded != current {
        current = folded;
        ops.push("casefold".to_string());
    }

    (current, ops, findings)
}

fn canonicalize_human_label_compare(text: &str) -> (String, Vec<String>, Vec<String>) {
    let mut ops: Vec<String> = Vec::new();
    let mut findings: Vec<String> = Vec::new();
    let mut current = text.to_string();

    let nfc = current.nfc().collect::<String>();
    if nfc != current {
        current = nfc;
        ops.push("NFC".to_string());
    }

    let folded = current.to_lowercase();
    if folded != current {
        current = folded;
        ops.push("casefold".to_string());
    }

    let trimmed = current.trim().to_string();
    if trimmed != current {
        current = trimmed;
        ops.push("trim".to_string());
    }

    let collapsed = current.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed != current {
        current = collapsed;
        ops.push("collapse_whitespace".to_string());
        findings.push("Whitespace sequences collapsed to single space".to_string());
    }

    (current, ops, findings)
}

fn canonicalize_json_key_compare(text: &str) -> (String, Vec<String>, Vec<String>) {
    let mut ops: Vec<String> = Vec::new();
    let findings: Vec<String> = Vec::new();
    let mut current = text.to_string();

    let nfc = current.nfc().collect::<String>();
    if nfc != current {
        current = nfc;
        ops.push("NFC".to_string());
    }

    let folded = current.to_lowercase();
    if folded != current {
        current = folded;
        ops.push("casefold".to_string());
    }

    (current, ops, findings)
}

fn canonicalize_path_segment_compare(text: &str) -> (String, Vec<String>, Vec<String>) {
    let mut ops: Vec<String> = Vec::new();
    let findings: Vec<String> = Vec::new();
    let mut current = text.to_string();

    let nfc = current.nfc().collect::<String>();
    if nfc != current {
        current = nfc;
        ops.push("NFC".to_string());
    }

    let lowered = current.to_lowercase();
    if lowered != current {
        current = lowered;
        ops.push("lowercase".to_string());
    }

    let lf = current.replace("\r\n", "\n").replace("\r", "\n");
    if lf != current {
        current = lf;
        ops.push("LF_newlines".to_string());
    }

    (current, ops, findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identifier_strict_pass() {
        let result = unicode_policy_check("hello", "identifier_strict", None);
        assert!(result.pass);
        assert_eq!(result.policy, "identifier_strict");
    }

    #[test]
    fn test_identifier_strict_fail() {
        let result = unicode_policy_check("h\u{200b}ello", "identifier_strict", None);
        assert!(!result.pass);
    }

    #[test]
    fn test_filename_safe_pass() {
        let result = unicode_policy_check("myfile.txt", "filename_safe", None);
        assert!(result.pass);
    }

    #[test]
    fn test_filename_safe_fail() {
        let result = unicode_policy_check("file:t.txt", "filename_safe", None);
        assert!(!result.pass);
    }

    #[test]
    fn test_invalid_policy() {
        let result = unicode_policy_check("test", "invalid_policy", None);
        assert!(!result.pass);
        assert!(result.summary.contains("Invalid policy"));
    }

    #[test]
    fn test_canonicalize_identifier() {
        let result = canonicalize_text("Hello", "identifier_compare", false);
        assert!(result.base.changed);
        assert_eq!(result.base.text, "hello");
    }

    #[test]
    fn test_canonicalize_human_label() {
        let result = canonicalize_text("  Hello   World  ", "human_label_compare", false);
        assert!(result.base.changed);
        assert_eq!(result.base.text, "hello world");
    }

    #[test]
    fn test_canonicalize_path_segment() {
        let result = canonicalize_text("MyFile.TXT", "path_segment_compare", false);
        assert!(result.base.changed);
        assert_eq!(result.base.text, "myfile.txt");
    }

    #[test]
    fn test_invalid_profile() {
        let result = canonicalize_text("test", "invalid_profile", false);
        assert!(!result.base.changed);
        assert!(!result.base.findings.is_empty());
    }
}
