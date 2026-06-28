use serde::{Deserialize, Serialize};

use crate::text::primitives::byte_offset_to_char_index;
use unicode_general_category::get_general_category;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextPositionResult {
    pub valid: bool,
    pub byte_offset: Option<usize>,
    pub codepoint_index: Option<usize>,
    pub utf16_offset: Option<usize>,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub line_base: usize,
    pub column_base: usize,
    #[serde(rename = "char")]
    pub char: Option<String>,
    pub codepoint: Option<String>,
    pub name: Option<String>,
    pub line_text_preview: Option<String>,
    pub error: Option<String>,
    pub summary: String,
}

fn utf16_offset_to_codepoint_index(text: &str, utf16_offset: usize) -> usize {
    let mut utf16_count = 0;
    for (i, char) in text.chars().enumerate() {
        let cp = char as u32;
        if cp <= 0xFFFF {
            utf16_count += 1;
        } else {
            utf16_count += 2;
        }
        if utf16_count > utf16_offset {
            return i;
        }
        if utf16_count == utf16_offset {
            return i + 1;
        }
    }
    text.chars().count()
}

fn codepoint_index_to_utf16_offset(text: &str, codepoint_index: usize) -> usize {
    let mut utf16_offset = 0;
    for (i, char) in text.chars().enumerate() {
        if i >= codepoint_index {
            break;
        }
        let cp = char as u32;
        if cp <= 0xFFFF {
            utf16_offset += 1;
        } else {
            utf16_offset += 2;
        }
    }
    utf16_offset
}

fn split_lines_keepends(text: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = Vec::new();
    if text.is_empty() {
        return lines;
    }

    let mut start = 0usize;
    let mut i = 0usize;
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    while i < chars.len() {
        let (byte_idx, ch) = chars[i];
        let ch_len = ch.len_utf8();

        if ch == '\n' {
            lines.push(&text[start..byte_idx + ch_len]);
            start = byte_idx + ch_len;
        } else if ch == '\r' {
            if i + 1 < chars.len() && chars[i + 1].1 == '\n' {
                let end = chars[i + 1].0 + chars[i + 1].1.len_utf8();
                lines.push(&text[start..end]);
                start = end;
                i += 1;
            } else {
                lines.push(&text[start..byte_idx + ch_len]);
                start = byte_idx + ch_len;
            }
        }

        i += 1;
    }

    if start < text.len() {
        lines.push(&text[start..]);
    }

    lines
}

fn get_line_col(text: &str, codepoint_index: usize) -> (Vec<&str>, usize, usize) {
    let lines = split_lines_keepends(text);
    let chars: Vec<char> = text.chars().collect();
    let mut line_num = 0usize;
    let mut current_col = 0usize;

    for i in 0..chars.len() {
        if i == codepoint_index {
            return (lines, line_num, current_col);
        }

        match chars[i] {
            '\n' => {
                line_num += 1;
                current_col = 0;
            }
            '\r' => {
                if i + 1 < chars.len() && chars[i + 1] == '\n' {
                    continue;
                }
                line_num += 1;
                current_col = 0;
            }
            _ => {
                current_col += 1;
            }
        }
    }

    if codepoint_index >= chars.len() {
        return (lines, line_num, current_col);
    }

    (lines, line_num, current_col)
}

fn is_valid_byte_offset(text: &str, offset: usize) -> bool {
    let utf8_bytes = text.as_bytes();
    if offset > utf8_bytes.len() {
        return false;
    }
    if offset == utf8_bytes.len() {
        return true;
    }

    let byte = utf8_bytes[offset];

    if byte < 0x80 {
        return true;
    }

    if (0xC0..=0xDF).contains(&byte) {
        if offset + 1 >= utf8_bytes.len() {
            return false;
        }
        return (0x80..=0xBF).contains(&utf8_bytes[offset + 1]);
    }

    if (0xE0..=0xEF).contains(&byte) {
        if offset + 2 >= utf8_bytes.len() {
            return false;
        }
        return (0x80..=0xBF).contains(&utf8_bytes[offset + 1])
            && (0x80..=0xBF).contains(&utf8_bytes[offset + 2]);
    }

    if (0xF0..=0xF7).contains(&byte) {
        if offset + 3 >= utf8_bytes.len() {
            return false;
        }
        return (0x80..=0xBF).contains(&utf8_bytes[offset + 1])
            && (0x80..=0xBF).contains(&utf8_bytes[offset + 2])
            && (0x80..=0xBF).contains(&utf8_bytes[offset + 3]);
    }

    false
}

fn codepoint_index_to_byte_offset(text: &str, codepoint_index: usize) -> usize {
    text.char_indices()
        .nth(codepoint_index)
        .map(|(byte_idx, _)| byte_idx)
        .unwrap_or_else(|| text.len())
}

fn line_column_to_codepoint_index(
    text: &str,
    line: usize,
    column: usize,
    line_base: usize,
    column_base: usize,
) -> Option<usize> {
    let lines = split_lines_keepends(text);
    let line_index = line.checked_sub(line_base)?;
    if line_index >= lines.len() {
        return None;
    }

    let col_index = column.checked_sub(column_base)?;
    let line_text_raw = lines[line_index];
    let trimmed_len = line_text_raw.trim_end_matches(['\r', '\n']).chars().count();
    if col_index > trimmed_len {
        return None;
    }

    let mut codepoint_index = 0usize;
    for line_text in &lines[..line_index] {
        codepoint_index += line_text.chars().count();
    }

    Some(codepoint_index + col_index)
}

#[allow(clippy::too_many_arguments)]
pub fn text_position(
    text: &str,
    byte_offset: Option<usize>,
    codepoint_index: Option<usize>,
    line: Option<usize>,
    column: Option<usize>,
    utf16_offset: Option<usize>,
    line_base: usize,
    column_base: usize,
) -> TextPositionResult {
    let mut mode_parts = Vec::new();
    if byte_offset.is_some() {
        mode_parts.push("byte_offset");
    }
    if codepoint_index.is_some() {
        mode_parts.push("codepoint_index");
    }
    if line.is_some() || column.is_some() {
        mode_parts.push("line+column");
    }
    if utf16_offset.is_some() {
        mode_parts.push("utf16_offset");
    }

    if mode_parts.len() != 1 {
        return TextPositionResult {
            valid: false,
            byte_offset: None,
            codepoint_index: None,
            utf16_offset: None,
            line: None,
            column: None,
            line_base,
            column_base,
            char: None,
            codepoint: None,
            name: None,
            line_text_preview: None,
            error: Some(
                "Exactly one locator mode must be provided: byte_offset, codepoint_index, line+column, or utf16_offset".to_string(),
            ),
            summary: "Invalid: multiple or no locator modes provided".to_string(),
        };
    }

    if text.is_empty() {
        if byte_offset.is_some() && byte_offset != Some(0) {
            return TextPositionResult {
                valid: false,
                byte_offset: None,
                codepoint_index: None,
                utf16_offset: None,
                line: None,
                column: None,
                line_base,
                column_base,
                char: None,
                codepoint: None,
                name: None,
                line_text_preview: None,
                error: Some("Byte offset 0 is the only valid position for empty text".to_string()),
                summary: "Invalid position for empty text".to_string(),
            };
        }
        return TextPositionResult {
            valid: true,
            byte_offset: Some(0),
            codepoint_index: Some(0),
            utf16_offset: Some(0),
            line: Some(line_base),
            column: Some(column_base),
            line_base,
            column_base,
            char: Some(String::new()),
            codepoint: None,
            name: None,
            line_text_preview: Some(String::new()),
            error: None,
            summary: "Empty text at start position".to_string(),
        };
    }

    let total_codepoints = text.chars().count();
    let effective_codepoint_index: usize;

    if let Some(bo) = byte_offset {
        if bo > text.len() {
            return TextPositionResult {
                valid: false,
                byte_offset: None,
                codepoint_index: None,
                utf16_offset: None,
                line: None,
                column: None,
                line_base,
                column_base,
                char: None,
                codepoint: None,
                name: None,
                line_text_preview: None,
                error: Some("Byte offset exceeds text length".to_string()),
                summary: "Invalid byte offset: beyond text end".to_string(),
            };
        }
        if !is_valid_byte_offset(text, bo) {
            return TextPositionResult {
                valid: false,
                byte_offset: None,
                codepoint_index: None,
                utf16_offset: None,
                line: None,
                column: None,
                line_base,
                column_base,
                char: None,
                codepoint: None,
                name: None,
                line_text_preview: None,
                error: Some("Byte offset falls inside multibyte character".to_string()),
                summary: "Invalid byte offset: inside multibyte character".to_string(),
            };
        }
        effective_codepoint_index = byte_offset_to_char_index(text, bo).unwrap_or(total_codepoints);
    } else if let Some(cp) = codepoint_index {
        if cp > total_codepoints {
            return TextPositionResult {
                valid: false,
                byte_offset: None,
                codepoint_index: None,
                utf16_offset: None,
                line: None,
                column: None,
                line_base,
                column_base,
                char: None,
                codepoint: None,
                name: None,
                line_text_preview: None,
                error: Some("Codepoint index out of bounds".to_string()),
                summary: "Invalid codepoint_index: out of bounds".to_string(),
            };
        }
        effective_codepoint_index = cp;
    } else if let Some(u16) = utf16_offset {
        let cp_idx = utf16_offset_to_codepoint_index(text, u16);
        if cp_idx > total_codepoints {
            return TextPositionResult {
                valid: false,
                byte_offset: None,
                codepoint_index: None,
                utf16_offset: None,
                line: None,
                column: None,
                line_base,
                column_base,
                char: None,
                codepoint: None,
                name: None,
                line_text_preview: None,
                error: Some("UTF-16 offset exceeds text length".to_string()),
                summary: "Invalid utf16_offset: beyond text end".to_string(),
            };
        }
        effective_codepoint_index = cp_idx;
    } else {
        let ln = line.unwrap_or(1);
        let col = column.unwrap_or(1);

        let lines_keepends = split_lines_keepends(text);
        let line_count = lines_keepends.len();
        if ln < line_base || ln >= line_base + line_count {
            let max_line = line_base + line_count.saturating_sub(1);
            if ln < line_base {
                return TextPositionResult {
                    valid: false,
                    byte_offset: None,
                    codepoint_index: None,
                    utf16_offset: None,
                    line: None,
                    column: None,
                    line_base,
                    column_base,
                    char: None,
                    codepoint: None,
                    name: None,
                    line_text_preview: None,
                    error: Some(format!("Line {ln} is less than minimum line {line_base}")),
                    summary: "Invalid line: below valid range".to_string(),
                };
            }
            return TextPositionResult {
                valid: false,
                byte_offset: None,
                codepoint_index: None,
                utf16_offset: None,
                line: None,
                column: None,
                line_base,
                column_base,
                char: None,
                codepoint: None,
                name: None,
                line_text_preview: None,
                error: Some(format!("Line {ln} exceeds maximum line {max_line}")),
                summary: "Invalid line: beyond text end".to_string(),
            };
        }

        let lines = lines_keepends;

        if col < column_base {
            return TextPositionResult {
                valid: false,
                byte_offset: None,
                codepoint_index: None,
                utf16_offset: None,
                line: None,
                column: None,
                line_base,
                column_base,
                char: None,
                codepoint: None,
                name: None,
                line_text_preview: None,
                error: Some(format!(
                    "Column {col} is less than minimum column {column_base}"
                )),
                summary: "Invalid column: below valid range".to_string(),
            };
        }

        let line_index = ln.saturating_sub(line_base);
        let col_index = col.saturating_sub(column_base);

        if line_index < lines.len() {
            let line_text_raw = lines[line_index];
            let trimmed_len = line_text_raw.trim_end_matches(['\r', '\n']).chars().count();

            if col_index > trimmed_len {
                return TextPositionResult {
                    valid: false,
                    byte_offset: None,
                    codepoint_index: None,
                    utf16_offset: None,
                    line: None,
                    column: None,
                    line_base,
                    column_base,
                    char: None,
                    codepoint: None,
                    name: None,
                    line_text_preview: None,
                    error: Some(format!("Column {col} exceeds line length {trimmed_len}")),
                    summary: "Invalid column: beyond line length".to_string(),
                };
            }
        }

        effective_codepoint_index =
            line_column_to_codepoint_index(text, ln, col, line_base, column_base)
                .unwrap_or(total_codepoints);
    }

    let (lines, line_num, col) = get_line_col(text, effective_codepoint_index);

    let line_1based = line_num + line_base;
    let col_1based = col + column_base;

    let char_at_pos = text.chars().nth(effective_codepoint_index);

    let (char_val, codepoint_str, name_val) = if let Some(ch) = char_at_pos {
        let cp_str = format!("U+{:04X}", ch as u32);
        let name_val = unicodedata_name(ch);
        (Some(ch.to_string()), Some(cp_str), Some(name_val))
    } else {
        (None, None, None)
    };

    let line_preview = if line_num < lines.len() {
        Some(lines[line_num].trim_end_matches(['\r', '\n']).to_string())
    } else {
        None
    };

    let byte_offset_result = codepoint_index_to_byte_offset(text, effective_codepoint_index);
    let utf16_result = codepoint_index_to_utf16_offset(text, effective_codepoint_index);

    TextPositionResult {
        valid: true,
        byte_offset: Some(byte_offset_result),
        codepoint_index: Some(effective_codepoint_index),
        utf16_offset: Some(utf16_result),
        line: Some(line_1based),
        column: Some(col_1based),
        line_base,
        column_base,
        char: char_val,
        codepoint: codepoint_str,
        name: name_val,
        line_text_preview: line_preview,
        error: None,
        summary: format!("Line {line_1based}, column {col_1based}"),
    }
}

fn unicodedata_name(ch: char) -> String {
    unicode_names2::name(ch)
        .map(|name| name.to_string())
        .unwrap_or_else(|| "<unknown>".to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextWindowPosition {
    pub kind: String,
    pub value: Option<usize>,
    pub byte_offset: Option<usize>,
    pub codepoint_index: Option<usize>,
    pub grapheme_index: Option<usize>,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub line_base: Option<usize>,
    pub column_base: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionInfo {
    pub byte_offset: usize,
    pub codepoint_index: usize,
    pub grapheme_index: usize,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineInfo {
    pub line: usize,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtCodepointInfo {
    #[serde(rename = "char")]
    pub char: char,
    pub codepoint: String,
    pub name: String,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextWindowResult {
    pub position: PositionInfo,
    pub line_text: String,
    pub line_visible_repr: String,
    pub before: Vec<LineInfo>,
    pub after: Vec<LineInfo>,
    pub newline_style: String,
    pub at_codepoint: Option<AtCodepointInfo>,
    pub warnings: Vec<String>,
}

pub fn text_window(
    text: &str,
    position: &TextWindowPosition,
    context_lines: usize,
    include_visible_repr: bool,
) -> TextWindowResult {
    let mut warnings = Vec::new();

    let kind = &position.kind;
    let line_base = position.line_base.unwrap_or(1);
    let column_base = position.column_base.unwrap_or(1);
    let total_codepoints = text.chars().count();
    let graphemes: Vec<(usize, &str)> = text.grapheme_indices(true).collect();

    let mut codepoint_index: Option<usize> = None;

    match kind.as_str() {
        "byte_offset" => {
            let bo = position.value.or(position.byte_offset).unwrap_or(0);
            codepoint_index = Some(byte_offset_to_char_index(text, bo).unwrap_or(total_codepoints));
        }
        "codepoint_index" => {
            let cp = position.value.or(position.codepoint_index).unwrap_or(0);
            if cp <= total_codepoints {
                codepoint_index = Some(cp);
            }
        }
        "grapheme_index" => {
            let gi = position.value.or(position.grapheme_index).unwrap_or(0);
            if gi < graphemes.len() {
                let byte_idx = graphemes[gi].0;
                codepoint_index =
                    Some(byte_offset_to_char_index(text, byte_idx).unwrap_or(total_codepoints));
            } else {
                codepoint_index = Some(total_codepoints);
            }
        }
        "line_column" => {
            let ln = position.line.or(position.value).unwrap_or(1);
            let col = position.column.unwrap_or(1);
            codepoint_index = line_column_to_codepoint_index(text, ln, col, line_base, column_base);
        }
        _ => {
            codepoint_index = Some(0);
        }
    }

    let cp_idx = codepoint_index.unwrap_or(total_codepoints);
    let (line_num, column_num) = get_line_col_for_cp(text, cp_idx, line_base, column_base);

    let byte_offset = codepoint_index_to_byte_offset(text, cp_idx);

    let mut grapheme_index = 0;
    for (start_byte, _) in &graphemes {
        let start_cp = byte_offset_to_char_index(text, *start_byte).unwrap_or(total_codepoints);
        if start_cp < cp_idx {
            grapheme_index += 1;
        } else {
            break;
        }
    }

    let lines_all = split_lines_keepends(text);
    let line_index = line_num.saturating_sub(line_base);
    let line_text = if line_index < lines_all.len() {
        lines_all[line_index]
            .trim_end_matches(['\r', '\n'])
            .to_string()
    } else {
        String::new()
    };

    let line_visible_repr = if include_visible_repr {
        line_text.replace('\t', "→").replace(' ', "·")
    } else {
        String::new()
    };

    let newline_style = detect_newline_style(text);

    let before: Vec<LineInfo> = (0..context_lines)
        .filter_map(|offset| {
            let ln = line_num.saturating_sub(line_base + 1 + offset);
            if ln < lines_all.len() {
                Some(LineInfo {
                    line: ln + line_base,
                    text: lines_all[ln].trim_end_matches(['\r', '\n']).to_string(),
                })
            } else {
                None
            }
        })
        .collect();

    let after: Vec<LineInfo> = (0..context_lines)
        .filter_map(|offset| {
            let ln = line_num.saturating_sub(line_base) + 1 + offset;
            if ln < lines_all.len() {
                Some(LineInfo {
                    line: ln + line_base,
                    text: lines_all[ln].trim_end_matches(['\r', '\n']).to_string(),
                })
            } else {
                None
            }
        })
        .collect();

    let at_codepoint = if cp_idx < total_codepoints {
        let ch = text.chars().nth(cp_idx).unwrap();
        let codepoint_str = format!("U+{:04X}", ch as u32);
        let category = get_unicode_category(ch);
        let name_val = unicodedata_name(ch);
        Some(AtCodepointInfo {
            char: ch,
            codepoint: codepoint_str,
            name: name_val,
            category,
        })
    } else {
        None
    };

    if byte_offset < text.len() {
        let b = text.as_bytes()[byte_offset];
        if (0x80..0xC0).contains(&b) {
            warnings.push(
                "Position falls in middle of multi-byte sequence (byte is continuation byte)"
                    .to_string(),
            );
        }
    }

    TextWindowResult {
        position: PositionInfo {
            byte_offset,
            codepoint_index: cp_idx,
            grapheme_index,
            line: line_num,
            column: column_num,
        },
        line_text,
        line_visible_repr,
        before,
        after,
        newline_style,
        at_codepoint,
        warnings,
    }
}

fn get_line_col_for_cp(
    text: &str,
    codepoint_index: usize,
    line_base: usize,
    column_base: usize,
) -> (usize, usize) {
    let mut current_line = line_base;
    let mut current_col = column_base;
    let chars: Vec<char> = text.chars().collect();

    for i in 0..chars.len() {
        if i == codepoint_index {
            return (current_line, current_col);
        }

        match chars[i] {
            '\n' => {
                current_line += 1;
                current_col = column_base;
            }
            '\r' => {
                if i + 1 < chars.len() && chars[i + 1] == '\n' {
                    continue;
                }
                current_line += 1;
                current_col = column_base;
            }
            _ => {
                current_col += 1;
            }
        }
    }

    (current_line, current_col)
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

fn get_unicode_category(ch: char) -> String {
    get_general_category(ch).abbreviation().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_position_empty() {
        let result = text_position("", Some(0), None, None, None, None, 1, 1);
        assert!(result.valid);
        assert_eq!(result.byte_offset, Some(0));
    }

    #[test]
    fn test_text_position_simple() {
        let result = text_position("hello", None, Some(0), None, None, None, 1, 1);
        assert!(result.valid);
        assert_eq!(result.line, Some(1));
        assert_eq!(result.column, Some(1));
    }

    #[test]
    fn test_text_position_multiline() {
        let text = "line1\nline2\nline3";
        let result = text_position(text, None, Some(7), None, None, None, 1, 1);
        assert!(result.valid);
        assert_eq!(result.line, Some(2));
        assert_eq!(result.column, Some(2));
    }
}
