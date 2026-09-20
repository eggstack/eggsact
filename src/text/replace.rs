use std::sync::LazyLock;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

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

#[derive(Debug, Clone)]
pub struct TextReplaceCheckOptions<'a> {
    pub mode: &'a str,
    pub expected_count: Option<usize>,
    pub allow_multiple: bool,
    pub newline_policy: &'a str,
    pub return_preview: bool,
    pub max_preview_chars: usize,
}

impl Default for TextReplaceCheckOptions<'static> {
    fn default() -> Self {
        Self {
            mode: "exact",
            expected_count: None,
            allow_multiple: false,
            newline_policy: "preserve",
            return_preview: false,
            max_preview_chars: MAX_PREVIEW_CHARS,
        }
    }
}

fn normalize_for_match(s: &str, mode: &str) -> String {
    match mode {
        "nfc" => s.nfc().collect(),
        "nfkc" => s.nfkc().collect(),
        "casefold" => crate::text::unicode_tools::unicode_casefold(s),
        "whitespace_collapse" => collapse_whitespace(s),
        _ => s.to_string(),
    }
}

static WHITESPACE_RE: LazyLock<fancy_regex::Regex> =
    LazyLock::new(|| fancy_regex::Regex::new(r"\s+").unwrap());

fn collapse_whitespace(s: &str) -> String {
    WHITESPACE_RE.replace_all(s, " ").to_string()
}

struct NormalizedChunk {
    normalized_end: usize,
    original_start: usize,
    original_end: usize,
}

fn normalize_with_map(text: &str, mode: &str) -> (String, Vec<NormalizedChunk>) {
    let graphemes: Vec<&str> = text.graphemes(true).collect();
    let mut normalized = String::new();
    let mut chunks = Vec::new();
    let mut original_index = 0;
    let mut grapheme_index = 0;

    while grapheme_index < graphemes.len() {
        let original_start = original_index;
        let is_whitespace = mode == "whitespace_collapse"
            && graphemes[grapheme_index]
                .chars()
                .all(|ch| ch.is_whitespace());
        if is_whitespace {
            while grapheme_index < graphemes.len()
                && graphemes[grapheme_index]
                    .chars()
                    .all(|ch| ch.is_whitespace())
            {
                original_index += graphemes[grapheme_index].chars().count();
                grapheme_index += 1;
            }
            normalized.push(' ');
        } else {
            normalized.push_str(&normalize_for_match(graphemes[grapheme_index], mode));
            original_index += graphemes[grapheme_index].chars().count();
            grapheme_index += 1;
        }
        chunks.push(NormalizedChunk {
            normalized_end: normalized.len(),
            original_start,
            original_end: original_index,
        });
    }

    debug_assert_eq!(normalized, normalize_for_match(text, mode));
    (normalized, chunks)
}

fn original_index_for_normalized_boundary(
    chunks: &[NormalizedChunk],
    normalized_offset: usize,
) -> usize {
    let chunk_index = chunks.partition_point(|chunk| chunk.normalized_end <= normalized_offset);
    chunks
        .get(chunk_index)
        .map(|chunk| chunk.original_start)
        .or_else(|| chunks.last().map(|chunk| chunk.original_end))
        .unwrap_or(0)
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
        "none".to_string()
    }
}

fn apply_newline_policy(text: &str, policy: &str) -> String {
    match policy {
        "normalize_lf" => normalize_newlines_to_lf(text),
        "normalize_crlf" => {
            let lf_text = normalize_newlines_to_lf(text);
            lf_text.replace('\n', "\r\n")
        }
        _ => text.to_string(),
    }
}

fn normalize_newlines_to_lf(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                normalized.push('\n');
            }
            _ => normalized.push(ch),
        }
    }

    normalized
}

struct SourceIndex {
    byte_offsets: Vec<usize>,
    positions: Vec<(usize, usize)>,
}

impl SourceIndex {
    fn new(text: &str) -> Self {
        let mut byte_offsets = Vec::with_capacity(text.chars().count() + 1);
        let mut positions = Vec::with_capacity(byte_offsets.capacity());
        let mut line = 1;
        let mut column = 1;
        let mut after_cr = false;
        byte_offsets.push(0);
        positions.push((line, column));

        for (byte, ch) in text.char_indices() {
            debug_assert_eq!(byte_offsets.last().copied(), Some(byte));
            match ch {
                '\r' => {
                    line += 1;
                    column = 1;
                    after_cr = true;
                }
                '\n' => {
                    if !after_cr {
                        line += 1;
                    }
                    column = 1;
                    after_cr = false;
                }
                _ => {
                    after_cr = false;
                }
            }
            // Historical position semantics identify a newline itself with
            // the beginning of the following line.  Keep that behavior for
            // CR, LF, and both codepoints of CRLF while still building the
            // complete index in one pass.
            if matches!(ch, '\r' | '\n') {
                if let Some(position) = positions.last_mut() {
                    *position = (line, column);
                }
            } else {
                column += 1;
            }
            byte_offsets.push(byte + ch.len_utf8());
            positions.push((line, column));
        }

        Self {
            byte_offsets,
            positions,
        }
    }

    fn byte_offset(&self, codepoint_index: usize) -> usize {
        self.byte_offsets
            .get(codepoint_index)
            .copied()
            .unwrap_or_else(|| *self.byte_offsets.last().unwrap_or(&0))
    }

    fn codepoint_index_at_byte(&self, byte_offset: usize) -> usize {
        self.byte_offsets
            .partition_point(|offset| *offset <= byte_offset)
            .saturating_sub(1)
    }

    fn position(&self, codepoint_index: usize) -> (usize, usize) {
        self.positions
            .get(codepoint_index)
            .copied()
            .unwrap_or_else(|| *self.positions.last().unwrap_or(&(1, 1)))
    }

    fn build_position(&self, codepoint_index: usize, match_len: usize) -> PositionInfo {
        PositionInfo {
            codepoint_index,
            byte_start: self.byte_offset(codepoint_index),
            byte_end: self.byte_offset(codepoint_index.saturating_add(match_len)),
            line: self.position(codepoint_index).0,
            column: self.position(codepoint_index).1,
        }
    }
}

#[allow(clippy::too_many_arguments)]
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
    text_replace_check_with_options(
        text,
        old,
        new,
        TextReplaceCheckOptions {
            mode,
            expected_count,
            allow_multiple,
            newline_policy,
            return_preview,
            max_preview_chars,
        },
    )
}

pub fn text_replace_check_with_options(
    text: &str,
    old: &str,
    new: &str,
    options: TextReplaceCheckOptions<'_>,
) -> Result<TextReplaceCheckResult, String> {
    let TextReplaceCheckOptions {
        mode,
        expected_count,
        allow_multiple,
        newline_policy,
        return_preview,
        max_preview_chars,
    } = options;

    let source_index = SourceIndex::new(text);
    let text_length = source_index.positions.len().saturating_sub(1);
    if text_length > MAX_TEXT_LENGTH {
        return Err(format!(
            "Input length {} exceeds MAX_TEXT_LENGTH {}",
            text_length, MAX_TEXT_LENGTH
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

    let (text_norm, normalized_chunks) = normalize_with_map(text, mode);
    let old_norm = normalize_for_match(old, mode);

    let old_chars = old.chars().count();

    let mut positions: Vec<PositionInfo> = Vec::new();
    let mut match_ranges: Vec<(usize, usize)> = Vec::new();
    let mut search_start = 0;

    if old.is_empty() {
        for cp_idx in 0..=text_length {
            positions.push(source_index.build_position(cp_idx, 0));
            match_ranges.push((cp_idx, cp_idx));
        }
    } else if mode == "exact" {
        // Search text directly for correct positions.
        while search_start <= text.len() {
            let search_from = search_start.min(text.len());
            if let Some(idx) = text[search_from..].find(old) {
                let byte_idx = search_from + idx;
                let cp_idx = source_index.codepoint_index_at_byte(byte_idx);
                positions.push(source_index.build_position(cp_idx, old_chars));
                match_ranges.push((cp_idx, cp_idx + old_chars));
                search_start = byte_idx + old.len();
            } else {
                break;
            }
        }
    } else {
        // Search text_norm to preserve normalized matching, then map each hit
        // back to the original grapheme/codepoint span that produced it.
        while search_start <= text_norm.len() {
            let search_from = search_start.min(text_norm.len());
            if let Some(idx) = text_norm[search_from..].find(&old_norm) {
                let byte_idx = search_from + idx;
                let normalized_end = byte_idx + old_norm.len();
                let cp_start = original_index_for_normalized_boundary(&normalized_chunks, byte_idx);
                let cp_end =
                    original_index_for_normalized_boundary(&normalized_chunks, normalized_end);
                positions.push(source_index.build_position(cp_start, cp_end - cp_start));
                match_ranges.push((cp_start, cp_end));
                search_start = byte_idx + old_norm.len();
            } else {
                break;
            }
        }
    }

    let match_count = positions.len();
    let unique_match = match_count == 1;
    let would_change = match_count > 0;

    let expected_count_met = if let Some(expected) = expected_count {
        match_count == expected
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

    let replaced_text = if would_change {
        let mut replaced = String::with_capacity(
            text.len()
                .saturating_add(match_count.saturating_mul(new.len().saturating_sub(old.len()))),
        );
        let mut last_byte = 0;
        for (pos, &(_, match_end)) in positions.iter().zip(match_ranges.iter()) {
            let match_start_byte = source_index.byte_offset(pos.codepoint_index);
            let match_end_byte = source_index.byte_offset(match_end);
            replaced.push_str(&text[last_byte..match_start_byte]);
            replaced.push_str(new);
            last_byte = match_end_byte;
        }
        replaced.push_str(&text[last_byte..]);
        replaced
    } else {
        text.to_string()
    };
    let changed_text_built = apply_newline_policy(&replaced_text, newline_policy);

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
        assert_eq!(result.newline_style_before, "none");
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
    fn test_text_replace_check_casefold_unicode() {
        let result = text_replace_check(
            "Straße", "Strasse", "Street", "casefold", None, false, "preserve", false, 2000,
        )
        .unwrap();
        assert_eq!(result.match_count, 1);
        assert!(result.would_change);
    }

    #[test]
    fn test_text_replace_check_nfkc_expansion_positions_and_replacement() {
        let result = text_replace_check(
            "aﬁb", "fi", "X", "nfkc", None, false, "preserve", true, 2000,
        )
        .unwrap();

        assert_eq!(result.match_count, 1);
        assert_eq!(result.positions[0].codepoint_index, 1);
        assert_eq!(result.positions[0].byte_start, 1);
        assert_eq!(result.positions[0].byte_end, 4);
        assert_eq!(result.preview_after, "aXb");
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
