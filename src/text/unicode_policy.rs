use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use unicode_general_category::get_general_category;
use unicode_normalization::UnicodeNormalization;

use crate::text::unicode_tools::unicode_casefold;

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

/// Bidirectional controls, delegated to the authoritative typed classifier in
/// `unicode_tools` so policy code never maintains its own bidi table.
/// (Previously a duplicated hand-maintained list.)
use crate::text::unicode_tools::BIDI_CONTROLS as BIDI_CHARS;

const WIN_FORBIDDEN: &[char] = &['\\', '/', ':', '*', '?', '"', '<', '>', '|'];

/// Count occurrences of zero-width characters in text (matching Python's
/// `[c for c in normalized if c in _ZERO_WIDTH_CHARS]` which counts every
/// occurrence, not just distinct types).
fn count_zero_width_occurrences(text: &str, exclude_word_joiner: bool) -> usize {
    use crate::text::unicode_tools::is_zero_width_char;
    text.chars()
        .filter(|c| {
            if exclude_word_joiner && *c == '\u{2060}' {
                return false;
            }
            is_zero_width_char(*c)
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
    get_general_category(c).abbreviation()
}

fn find_invisibles(text: &str) -> Vec<char> {
    use crate::text::unicode_tools::has_security_invisible_hazard;
    text.chars()
        .filter(|c| has_security_invisible_hazard(*c))
        .collect()
}

/// Script identity with the policy-layer `"Unknown"` spelling.
///
/// Delegates to the authoritative [`crate::text::script`] source; emoji
/// (which the old table mapped to `"Common"`) now surface as `"Unknown"`
/// via [`crate::text::script::policy_script_of`] and are filtered by the
/// caller exactly as before, so wire behavior is preserved.
fn get_script(cp: u32) -> &'static str {
    crate::text::script::policy_script_of(cp)
}

fn detect_mixed_scripts(text: &str) -> (bool, Vec<String>) {
    let mut scripts: HashSet<&'static str> = HashSet::new();
    for c in text.chars() {
        if c.is_whitespace() || c == '\u{200b}' || c == '\u{200c}' || c == '\u{200d}' {
            continue;
        }
        let cat = get_unicode_category(c);
        if cat.starts_with('M') || cat == "Cf" {
            continue;
        }
        let script = get_script(c as u32);
        if matches!(script, "Unknown" | "Common" | "Inherited" | "Other") {
            continue;
        }
        scripts.insert(script);
    }

    let mut script_list: Vec<String> = scripts.iter().map(|s| s.to_string()).collect();
    script_list.sort();
    // UTS #39 §5.1 resolved-script verdict (authoritative); the observed
    // list above is preserved for diagnostics.
    let has_multiple = crate::text::script::is_mixed_script(text);
    (has_multiple, script_list)
}

fn detect_confusables(text: &str) -> Vec<(char, &'static str)> {
    use crate::text::confusables::find_confusables;
    find_confusables(text)
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

    let text_length = text.chars().count();
    if text_length > 100_000 {
        return UnicodePolicyCheckResult {
            pass: false,
            policy: policy.to_string(),
            normalized_form: String::new(),
            findings: vec![PolicyFinding {
                rule: "input_too_large".to_string(),
                severity: "error".to_string(),
                message: format!("Input length {} exceeds maximum 100000", text_length),
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

fn check_identifier_strict(text: &str, normalized: &str) -> Vec<PolicyFinding> {
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

    // Normalization instability: warn only for an actual input/profile
    // normalization change (or a further compatibility change), never merely
    // because a precomposed character has an NFD decomposition. Ordinary
    // NFC-stable input such as "caf\u{e9}" must not warn: NFC and NFD forms
    // of any non-empty string almost always differ, so NFC-vs-NFD inequality
    // is not instability.
    if text != normalized {
        findings.push(PolicyFinding {
            rule: "normalization_instability".to_string(),
            severity: "warning".to_string(),
            message: "Input changed under profile normalization; identifier may compare unequal across normalization forms".to_string(),
        });
    } else {
        let nfkc_form: String = normalized.nfkc().collect();
        if nfkc_form != *normalized {
            findings.push(PolicyFinding {
                rule: "normalization_instability".to_string(),
                severity: "warning".to_string(),
                message:
                    "Text changes under compatibility (NFKC) folding despite profile normalization"
                        .to_string(),
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

/// Character-level mapping between `original` and `canonical`.
///
/// `position` is a code-point position in `original`. The previous
/// implementation zipped both strings by raw index, so one one-to-many
/// transform (e.g. `ß -> ss` casefolding) shifted every later diagnostic
/// mapping. This implementation uses a bounded greedy alignment (lookahead
/// window of 16 code points): equal code points align with no entry, a
/// local expansion/contraction emits exactly one entry for the affected
/// span, and only truly unalignable tails fall back to positional pairing.
/// Later positions therefore stay aligned after a local expansion.
fn build_char_mapping(original: &str, canonical: &str) -> Vec<CharMapping> {
    const LOOKAHEAD: usize = 16;

    fn codepoints(chars: &[char]) -> Option<String> {
        if chars.is_empty() {
            None
        } else {
            Some(
                chars
                    .iter()
                    .map(|c| format!("U+{:04X}", *c as u32))
                    .collect::<Vec<_>>()
                    .join(" "),
            )
        }
    }

    fn text(chars: &[char]) -> Option<String> {
        if chars.is_empty() {
            None
        } else {
            Some(chars.iter().collect())
        }
    }

    let orig_chars: Vec<char> = original.chars().collect();
    let canon_chars: Vec<char> = canonical.chars().collect();
    let mut mapping: Vec<CharMapping> = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);

    while i < orig_chars.len() || j < canon_chars.len() {
        match (orig_chars.get(i).copied(), canon_chars.get(j).copied()) {
            (Some(o), Some(c)) if o == c => {
                i += 1;
                j += 1;
            }
            (Some(o), Some(c)) => {
                // Expansion? If the *next* original char appears within the
                // lookahead window ahead in canonical, the span between is
                // this char's local expansion (e.g. `ß -> ss`).
                let mut expanded_to: Option<usize> = None;
                if let Some(next_orig) = orig_chars.get(i + 1).copied() {
                    let end = (j + LOOKAHEAD + 1).min(canon_chars.len());
                    for k in (j + 1)..=end {
                        if canon_chars.get(k) == Some(&next_orig) {
                            expanded_to = Some(k);
                            break;
                        }
                    }
                } else {
                    // Last original char: consume a bounded trailing
                    // expansion only (avoids swallowing a long unrelated
                    // tail when canonical is much longer).
                    let remaining = canon_chars.len().saturating_sub(j);
                    if (2..=LOOKAHEAD).contains(&remaining) {
                        expanded_to = Some(canon_chars.len());
                    }
                }
                if let Some(k) = expanded_to {
                    let span = &canon_chars[j..k.min(canon_chars.len())];
                    if !span.is_empty() {
                        mapping.push(CharMapping {
                            position: i,
                            original: Some(o.to_string()),
                            original_codepoint: Some(format!("U+{:04X}", o as u32)),
                            canonical: text(span),
                            canonical_codepoint: codepoints(span),
                        });
                        i += 1;
                        j = k.min(canon_chars.len());
                        continue;
                    }
                }
                // Contraction? If the current canonical char appears within
                // the lookahead window ahead in original, the span between
                // is a local contraction onto it.
                let mut contracted_to: Option<usize> = None;
                let oend = (i + LOOKAHEAD + 1).min(orig_chars.len());
                for k in (i + 1)..=oend {
                    if orig_chars.get(k) == Some(&c) {
                        contracted_to = Some(k);
                        break;
                    }
                }
                if let Some(k) = contracted_to {
                    let span = &orig_chars[i..k.min(orig_chars.len())];
                    if !span.is_empty() {
                        mapping.push(CharMapping {
                            position: i,
                            original: text(span),
                            original_codepoint: codepoints(span),
                            canonical: Some(c.to_string()),
                            canonical_codepoint: Some(format!("U+{:04X}", c as u32)),
                        });
                        i = k.min(orig_chars.len());
                        j += 1;
                        continue;
                    }
                }
                // Plain substitution fallback.
                mapping.push(CharMapping {
                    position: i,
                    original: Some(o.to_string()),
                    original_codepoint: Some(format!("U+{:04X}", o as u32)),
                    canonical: Some(c.to_string()),
                    canonical_codepoint: Some(format!("U+{:04X}", c as u32)),
                });
                i += 1;
                j += 1;
            }
            (None, Some(c)) => {
                mapping.push(CharMapping {
                    position: i,
                    original: None,
                    original_codepoint: None,
                    canonical: Some(c.to_string()),
                    canonical_codepoint: Some(format!("U+{:04X}", c as u32)),
                });
                j += 1;
            }
            (Some(o), None) => {
                mapping.push(CharMapping {
                    position: i,
                    original: Some(o.to_string()),
                    original_codepoint: Some(format!("U+{:04X}", o as u32)),
                    canonical: None,
                    canonical_codepoint: None,
                });
                i += 1;
            }
            (None, None) => break,
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

    let folded = unicode_casefold(&current);
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

    let folded = unicode_casefold(&current);
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

    let folded = unicode_casefold(&current);
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

    #[test]
    fn test_normalization_instability_ignores_ordinary_precomposed() {
        // "café" (U+00E9) is NFC-stable: NFC-vs-NFD inequality alone must not
        // warn. The old predicate flagged every precomposed character.
        let result = unicode_policy_check("café", "identifier_strict", None);
        assert!(
            !result
                .findings
                .iter()
                .any(|f| f.rule == "normalization_instability"),
            "NFC-stable input must not warn: {:?}",
            result.findings
        );
    }

    #[test]
    fn test_normalization_instability_fires_on_profile_change() {
        // Decomposed "cafe\u{301}" changes under the NFC profile: genuine
        // instability. Compatibility "ﬁ" (U+FB01) survives NFC but folds
        // under NFKC: also instability.
        let decomposed = unicode_policy_check("cafe\u{301}", "identifier_strict", None);
        assert!(
            decomposed
                .findings
                .iter()
                .any(|f| f.rule == "normalization_instability"),
            "decomposed input must warn: {:?}",
            decomposed.findings
        );
        let compat = unicode_policy_check("\u{FB01}", "identifier_strict", None);
        assert!(
            compat
                .findings
                .iter()
                .any(|f| f.rule == "normalization_instability"),
            "compatibility input must warn: {:?}",
            compat.findings
        );
        let plain = unicode_policy_check("hello", "identifier_strict", None);
        assert!(!plain
            .findings
            .iter()
            .any(|f| f.rule == "normalization_instability"));
    }

    #[test]
    fn test_char_mapping_no_cascade_after_expansion() {
        // "ßtest" casefolds to "sstest": exactly one local expansion entry,
        // with later positions still aligned (no cascade).
        let result = canonicalize_text("ßtest", "identifier_compare", true);
        let mapping = result.mapping.expect("changed input must map");
        assert_eq!(mapping.len(), 1, "expansion must not cascade: {mapping:?}");
        assert_eq!(mapping[0].position, 0);
        assert_eq!(mapping[0].original.as_deref(), Some("ß"));
        assert_eq!(mapping[0].canonical.as_deref(), Some("ss"));
    }

    #[test]
    fn test_legitimate_japanese_mixture_not_flagged() {
        let ja = unicode_policy_check("日本語テスト漢字", "human_text", None);
        assert!(
            !ja.findings.iter().any(|f| f.rule == "mixed_scripts"),
            "legitimate Japanese mixture must not warn: {:?}",
            ja.findings
        );
        let spoof = unicode_policy_check("hello привеt", "human_text", None);
        assert!(
            spoof.findings.iter().any(|f| f.rule == "mixed_scripts"),
            "Latin/Cyrillic mixture must warn: {:?}",
            spoof.findings
        );
    }
}
