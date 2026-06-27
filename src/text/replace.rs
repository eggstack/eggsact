use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

const MAX_TEXT_LENGTH: usize = 100_000;
const MAX_PREVIEW_CHARS: usize = 2000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionInfo {
    pub codepoint_index: usize,
    pub byte_start: usize,
    pub byte_end: usize,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextReplaceCheckResult {
    pub match_count: usize,
    pub unique_match: bool,
    pub expected_count_met: bool,
    pub would_change: bool,
    pub positions: Vec<PositionInfo>,
    pub changed_text_fingerprint: String,
    pub newline_style_before: String,
    pub newline_style_after: String,
    pub preview_before: String,
    pub preview_after: String,
    pub findings: Vec<Finding>,
}

fn normalize_for_match(s: &str, mode: &str) -> String {
    match mode {
        "nfc" => s.nfc().collect(),
        "nfkc" => s.nfkc().collect(),
        "casefold" => s.to_lowercase(),
        "whitespace_collapse" => collapse_whitespace(s),
        _ => s.to_string(),
    }
}

fn collapse_whitespace(s: &str) -> String {
    let re = fancy_regex::Regex::new(r"\s+").unwrap();
    re.replace_all(s, " ").to_string()
}

fn detect_newline_style(text: &str) -> String {
    let crlf_count = text.matches("\r\n").count();
    let lf_only = text.matches('\n').count() - crlf_count;
    let cr_only = text.matches('\r').count() - crlf_count;

    let has_crlf = crlf_count > 0;
    let has_lf = lf_only > 0;
    let has_cr = cr_only > 0;

    if has_crlf && (has_lf || has_cr) {
        "mixed".to_string()
    } else if has_crlf {
        "CRLF".to_string()
    } else if has_lf {
        "LF".to_string()
    } else if has_cr {
        "CR".to_string()
    } else {
        "LF".to_string()
    }
}

fn codepoint_index_to_line_column(
    text: &str,
    codepoint_index: usize,
    line_base: usize,
    column_base: usize,
) -> (usize, usize) {
    let mut current_pos = 0;
    let mut current_line = line_base;
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '\r' {
            // Count \r\n as a single line ending
            let line_len = i - current_pos;
            if current_pos + line_len > codepoint_index {
                let offset_in_line = codepoint_index - current_pos;
                let prefix: String = chars[current_pos..current_pos + offset_in_line]
                    .iter()
                    .collect();
                let current_col = column_base + prefix.chars().count();
                return (current_line, current_col);
            }
            current_line += 1;
            i += 1;
            // Skip \n after \r (treat \r\n as single line ending)
            if i < len && chars[i] == '\n' {
                i += 1;
            }
            current_pos = i;
        } else if chars[i] == '\n' {
            let line_len = i - current_pos;
            if current_pos + line_len > codepoint_index {
                let offset_in_line = codepoint_index - current_pos;
                let prefix: String = chars[current_pos..current_pos + offset_in_line]
                    .iter()
                    .collect();
                let current_col = column_base + prefix.chars().count();
                return (current_line, current_col);
            }
            current_line += 1;
            i += 1;
            current_pos = i;
        } else {
            i += 1;
        }
    }

    // Handle remaining text after last line ending
    if current_pos <= codepoint_index && codepoint_index <= len {
        let offset_in_line = codepoint_index - current_pos;
        let prefix: String = chars[current_pos..current_pos + offset_in_line]
            .iter()
            .collect();
        let current_col = column_base + prefix.chars().count();
        return (current_line, current_col);
    }

    (current_line, column_base)
}

fn get_byte_offsets(text: &str, codepoint_index: usize, old_len: usize) -> (usize, usize) {
    let mut char_indices = text.char_indices();
    let mut cp_idx = 0;
    let mut byte_start = text.len();

    while let Some((byte_offset, _)) = char_indices.next() {
        if cp_idx == codepoint_index {
            byte_start = byte_offset;
            break;
        }
        cp_idx += 1;
    }

    let mut char_indices = text.char_indices();
    let mut cp_idx = 0;
    let mut byte_end = text.len();

    while let Some((byte_offset, _)) = char_indices.next() {
        if cp_idx == codepoint_index + old_len {
            byte_end = byte_offset;
            break;
        }
        cp_idx += 1;
    }

    (byte_start, byte_end)
}

fn get_text_at_codepoint_range(text: &str, start_cp: usize, end_cp: usize) -> String {
    let text_chars = text.chars().count();
    if start_cp >= text_chars {
        return String::new();
    }
    let actual_end = end_cp.min(text_chars);
    if actual_end <= start_cp {
        return String::new();
    }
    text.chars()
        .skip(start_cp)
        .take(actual_end - start_cp)
        .collect()
}

pub fn text_replace_check(
    text: &str,
    old: &str,
    new: &str,
    mode: &str,
    expected_count: Option<usize>,
    allow_multiple: bool,
    newline_policy: &str,
    return_preview: bool,
    max_preview_chars: usize,
) -> Result<TextReplaceCheckResult, String> {
    if text.len() > MAX_TEXT_LENGTH {
        return Err(format!(
            "Input length {} exceeds MAX_TEXT_LENGTH {}",
            text.len(),
            MAX_TEXT_LENGTH
        ));
    }

    let valid_modes = ["exact", "nfc", "nfkc", "casefold", "whitespace_collapse"];
    if !valid_modes.contains(&mode) {
        return Err(format!(
            "Invalid mode: {}. Use one of: {}",
            mode,
            valid_modes.join(", ")
        ));
    }

    let valid_newline = ["preserve", "normalize_lf", "normalize_crlf"];
    if !valid_newline.contains(&newline_policy) {
        return Err(format!(
            "Invalid newline_policy: {}. Use one of: {}",
            newline_policy,
            valid_newline.join(", ")
        ));
    }

    let mut findings: Vec<Finding> = Vec::new();

    let text_norm = normalize_for_match(text, mode);
    let old_norm = normalize_for_match(old, mode);

    let old_chars = old.chars().count();

    let mut positions: Vec<PositionInfo> = Vec::new();
    let mut search_start = 0;

    // For exact mode, search text directly for correct positions.
    // For normalized modes, search text_norm to preserve normalized matching,
    // then map positions back to text via codepoint index.
    if mode == "exact" {
        while search_start <= text.len() {
            if let Some(idx) = text[special_min(search_start, text.len())..].find(old) {
                let byte_idx = special_min(search_start, text.len()) + idx;
                let cp_idx = text[..byte_idx].chars().count();
                let (byte_start, byte_end) = get_byte_offsets(text, cp_idx, old_chars);
                let (line, column) = codepoint_index_to_line_column(text, cp_idx, 1, 1);
                positions.push(PositionInfo {
                    codepoint_index: cp_idx,
                    byte_start,
                    byte_end,
                    line,
                    column,
                });
                search_start = byte_idx + if !old.is_empty() { old.len() } else { 1 };
            } else {
                break;
            }
        }
    } else {
        while search_start <= text_norm.len() {
            if let Some(idx) =
                text_norm[special_min(search_start, text_norm.len())..].find(&old_norm)
            {
                let byte_idx = special_min(search_start, text_norm.len()) + idx;
                let cp_idx = text_norm[..byte_idx].chars().count();
                let (byte_start, byte_end) = get_byte_offsets(text, cp_idx, old_chars);
                let (line, column) = codepoint_index_to_line_column(text, cp_idx, 1, 1);
                positions.push(PositionInfo {
                    codepoint_index: cp_idx,
                    byte_start,
                    byte_end,
                    line,
                    column,
                });
                search_start = byte_idx
                    + if !old_norm.is_empty() {
                        old_norm.len()
                    } else {
                        1
                    };
            } else {
                break;
            }
        }
    }

    let match_count = positions.len();
    let unique_match = match_count == 1;
    let would_change = match_count > 0;

    let expected_count_met = if let Some(expected) = expected_count {
        if match_count != expected {
            false
        } else {
            true
        }
    } else {
        true
    };

    if let Some(expected) = expected_count {
        if match_count != expected {
            if match_count == 0 {
                findings.push(Finding {
                    kind: "no_match".to_string(),
                    message: format!("Expected {} match(es) but found 0", expected),
                });
            } else {
                findings.push(Finding {
                    kind: "count_mismatch".to_string(),
                    message: format!("Expected {} match(es) but found {}", expected, match_count),
                });
            }
        }
    }

    if !allow_multiple && match_count > 1 {
        findings.push(Finding {
            kind: "ambiguous_replacement".to_string(),
            message: format!(
                "Found {} matches but allow_multiple is false; replacement is ambiguous",
                match_count
            ),
        });
    }

    if match_count == 0 {
        findings.push(Finding {
            kind: "no_match".to_string(),
            message: "No matches found; replacement would not change text".to_string(),
        });
    }

    let changed_text_built = if would_change {
        let old_chars = old.chars().count();
        let mut parts: Vec<String> = Vec::new();
        let mut last_cp = 0;
        for pos in &positions {
            let mid_text = get_text_at_codepoint_range(text, last_cp, pos.codepoint_index);
            parts.push(mid_text);
            parts.push(new.to_string());
            last_cp = pos.codepoint_index + old_chars;
        }
        parts.push(get_text_at_codepoint_range(
            text,
            last_cp,
            text.chars().count(),
        ));
        parts.join("")
    } else {
        text.to_string()
    };

    let mut hasher = Sha256::new();
    hasher.update(changed_text_built.as_bytes());
    let after_fp = format!("{:x}", hasher.finalize());
    let after_fp = after_fp[..16.min(after_fp.len())].to_string();

    let newline_before = detect_newline_style(text);
    let newline_after = detect_newline_style(&changed_text_built);

    let preview_before;
    let preview_after;

    if return_preview {
        let cap = max_preview_chars.min(MAX_PREVIEW_CHARS);
        let text_chars: Vec<char> = text.chars().collect();
        let changed_chars: Vec<char> = changed_text_built.chars().collect();
        preview_before = text_chars[..text_chars.len().min(cap)].iter().collect();
        preview_after = changed_chars[..changed_chars.len().min(cap)]
            .iter()
            .collect();
        if text_chars.len() > cap {
            findings.push(Finding {
                kind: "preview_truncated".to_string(),
                message: format!("Preview before truncated at {} characters", cap),
            });
        }
        if changed_chars.len() > cap {
            findings.push(Finding {
                kind: "preview_truncated".to_string(),
                message: format!("Preview after truncated at {} characters", cap),
            });
        }
    } else {
        preview_before = String::new();
        preview_after = String::new();
    }

    Ok(TextReplaceCheckResult {
        match_count,
        unique_match,
        expected_count_met,
        would_change,
        positions,
        changed_text_fingerprint: after_fp,
        newline_style_before: newline_before,
        newline_style_after: newline_after,
        preview_before,
        preview_after,
        findings,
    })
}

fn special_min(a: usize, b: usize) -> usize {
    if a < b {
        a
    } else {
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_replace_check_exact() {
        let result = text_replace_check(
            "hello world",
            "world",
            "rust",
            "exact",
            None,
            false,
            "preserve",
            false,
            2000,
        )
        .unwrap();
        assert_eq!(result.match_count, 1);
        assert!(result.unique_match);
        assert!(result.would_change);
        assert_eq!(result.newline_style_before, "LF");
    }

    #[test]
    fn test_text_replace_check_no_match() {
        let result = text_replace_check(
            "hello world",
            "foo",
            "bar",
            "exact",
            None,
            false,
            "preserve",
            false,
            2000,
        )
        .unwrap();
        assert_eq!(result.match_count, 0);
        assert!(!result.would_change);
        assert!(!result.findings.is_empty());
    }

    #[test]
    fn test_text_replace_check_multiple_with_allow_multiple_false() {
        let result = text_replace_check(
            "foo foo foo",
            "foo",
            "bar",
            "exact",
            None,
            false,
            "preserve",
            false,
            2000,
        )
        .unwrap();
        assert_eq!(result.match_count, 3);
        assert!(!result.unique_match);
        let has_ambiguous = result
            .findings
            .iter()
            .any(|f| f.kind == "ambiguous_replacement");
        assert!(has_ambiguous);
    }

    #[test]
    fn test_text_replace_check_with_preview() {
        let result = text_replace_check(
            "hello world",
            "world",
            "rust",
            "exact",
            None,
            false,
            "preserve",
            true,
            2000,
        )
        .unwrap();
        assert!(!result.preview_before.is_empty());
        assert!(!result.preview_after.is_empty());
    }

    #[test]
    fn test_text_replace_check_casefold() {
        let result = text_replace_check(
            "Hello WORLD",
            "hello world",
            "hi there",
            "casefold",
            None,
            false,
            "preserve",
            false,
            2000,
        )
        .unwrap();
        assert_eq!(result.match_count, 1);
        assert!(result.would_change);
    }

    #[test]
    fn test_text_replace_check_nfc() {
        let result = text_replace_check(
            "café",
            "cafe\u{0301}",
            "coffee",
            "nfc",
            None,
            false,
            "preserve",
            false,
            2000,
        )
        .unwrap();
        assert_eq!(result.match_count, 1);
    }

    #[test]
    fn test_text_replace_check_expected_count_met() {
        let result = text_replace_check(
            "foo bar foo",
            "foo",
            "baz",
            "exact",
            Some(2),
            false,
            "preserve",
            false,
            2000,
        )
        .unwrap();
        assert!(result.expected_count_met);
    }

    #[test]
    fn test_text_replace_check_expected_count_not_met() {
        let result = text_replace_check(
            "foo bar",
            "foo",
            "baz",
            "exact",
            Some(5),
            false,
            "preserve",
            false,
            2000,
        )
        .unwrap();
        assert!(!result.expected_count_met);
    }
}
