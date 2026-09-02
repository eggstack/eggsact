use unicode_general_category::get_general_category;
use unicode_segmentation::UnicodeSegmentation;

pub fn byte_offset_to_char_index(text: &str, byte_offset: usize) -> Result<usize, String> {
    if byte_offset > text.len() {
        return Err(format!(
            "Byte offset {} out of range (0-{})",
            byte_offset,
            text.len()
        ));
    }
    // Floor to previous char boundary if offset is mid-codepoint, so slicing
    // never panics and column reporting remains stable for multi-byte input.
    let mut valid_offset = byte_offset;
    while valid_offset > 0 && !text.is_char_boundary(valid_offset) {
        valid_offset -= 1;
    }
    Ok(text
        .char_indices()
        .take_while(|(idx, _)| *idx < valid_offset)
        .count())
}

pub fn codepoint_index_to_byte_offset(text: &str, codepoint_index: usize) -> Result<usize, String> {
    let char_count = text.chars().count();
    if codepoint_index > char_count {
        return Err(format!(
            "Codepoint index {} out of range (0-{})",
            codepoint_index, char_count
        ));
    }
    if let Some((byte_offset, _)) = text.char_indices().nth(codepoint_index) {
        Ok(byte_offset)
    } else {
        Ok(text.len())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CodepointInfo {
    pub index: usize,
    pub char: String,
    pub codepoint: String,
    pub name: String,
    pub category: String,
}

pub fn codepoints(s: &str) -> Vec<CodepointInfo> {
    s.chars()
        .enumerate()
        .map(|(index, ch)| {
            let codepoint = format!("U+{:04X}", ch as u32);
            let name = unicode_names2::name(ch)
                .map(|n| n.to_string())
                .unwrap_or_else(|| "<unknown>".to_string());
            let category = get_general_category(ch).abbreviation().to_string();
            CodepointInfo {
                index,
                char: ch.to_string(),
                codepoint,
                name,
                category,
            }
        })
        .collect()
}

pub fn count_graphemes(s: &str) -> usize {
    s.graphemes(true).count()
}

pub(crate) fn is_line_break(ch: char) -> bool {
    matches!(
        ch,
        '\n' | '\r'
            | '\x0b'
            | '\x0c'
            | '\x1c'
            | '\x1d'
            | '\x1e'
            | '\u{0085}'
            | '\u{2028}'
            | '\u{2029}'
    )
}

pub fn line_count(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    let mut count = 1;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if is_line_break(ch) {
            count += 1;
            if ch == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
        }
    }
    count
}

pub(crate) fn normalize_line_endings(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if is_line_break(ch) {
            if ch == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            normalized.push('\n');
        } else {
            normalized.push(ch);
        }
    }
    normalized
}

pub fn detect_newline_style(text: &str) -> &'static str {
    let crlf_count = text.matches("\r\n").count();
    let lf_only = text.matches('\n').count() - crlf_count;
    let cr_only = text.matches('\r').count() - crlf_count;

    let has_crlf = crlf_count > 0;
    let has_lf = lf_only > 0;
    let has_cr = cr_only > 0;

    if has_crlf && (has_lf || has_cr) {
        "mixed"
    } else if has_crlf {
        "CRLF"
    } else if has_lf {
        "LF"
    } else if has_cr {
        "CR"
    } else {
        "none"
    }
}

pub fn truncate_to_grapheme(s: &str, max_graphemes: usize) -> String {
    if max_graphemes == 0 || s.is_empty() {
        return String::new();
    }
    s.graphemes(true).take(max_graphemes).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_graphemes_simple() {
        assert_eq!(count_graphemes("abc"), 3);
        assert_eq!(count_graphemes(""), 0);
        assert_eq!(count_graphemes(" "), 1);
    }

    #[test]
    fn test_count_graphemes_emoji() {
        assert_eq!(count_graphemes("👋"), 1);
        assert_eq!(count_graphemes("🇺🇸"), 1);
    }

    #[test]
    fn test_truncate_to_grapheme_simple() {
        assert_eq!(truncate_to_grapheme("hello", 3), "hel");
        assert_eq!(truncate_to_grapheme("hello", 0), "");
        assert_eq!(truncate_to_grapheme("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_to_grapheme_emoji() {
        let s = "👋 hello";
        assert_eq!(truncate_to_grapheme(s, 1), "👋");
        assert_eq!(truncate_to_grapheme(s, 2), "👋 ");
        assert_eq!(truncate_to_grapheme(s, 3), "👋 h");
    }

    #[test]
    fn test_truncate_to_grapheme_empty() {
        assert_eq!(truncate_to_grapheme("", 5), "");
        assert_eq!(truncate_to_grapheme("hello", 0), "");
    }

    #[test]
    fn test_codepoint_index_to_byte_offset_ascii() {
        assert_eq!(codepoint_index_to_byte_offset("abc", 0).unwrap(), 0);
        assert_eq!(codepoint_index_to_byte_offset("abc", 1).unwrap(), 1);
        assert_eq!(codepoint_index_to_byte_offset("abc", 3).unwrap(), 3);
    }

    #[test]
    fn test_codepoint_index_to_byte_offset_multibyte() {
        // é is 2 bytes in UTF-8
        let s = "éa";
        assert_eq!(codepoint_index_to_byte_offset(s, 0).unwrap(), 0);
        assert_eq!(codepoint_index_to_byte_offset(s, 1).unwrap(), 2);
        assert_eq!(codepoint_index_to_byte_offset(s, 2).unwrap(), 3);
    }

    #[test]
    fn test_codepoint_index_to_byte_offset_emoji() {
        // 😀 is 4 bytes in UTF-8; "a😀b" = 1+4+1 = 6 bytes, 3 codepoints
        let s = "a😀b";
        assert_eq!(codepoint_index_to_byte_offset(s, 0).unwrap(), 0);
        assert_eq!(codepoint_index_to_byte_offset(s, 1).unwrap(), 1);
        assert_eq!(codepoint_index_to_byte_offset(s, 2).unwrap(), 5);
        assert_eq!(codepoint_index_to_byte_offset(s, 3).unwrap(), 6);
    }

    #[test]
    fn test_codepoint_index_to_byte_offset_out_of_range() {
        assert!(codepoint_index_to_byte_offset("abc", 4).is_err());
        assert!(codepoint_index_to_byte_offset("", 1).is_err());
    }
}
