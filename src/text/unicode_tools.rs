use serde::{Deserialize, Serialize};
use unicode_general_category::get_general_category;

/// Unicode helper tools: script detection, invisible char detection,
/// combining mark detection, safe representation, and character name lookup.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvisibleCharInfo {
    pub index: usize,
    pub char: char,
    pub codepoint: String,
    pub name: String,
    pub category: String,
    pub display: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptInfo {
    pub index: usize,
    pub char: char,
    pub script: String,
    pub codepoint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixedScriptsResult {
    pub mixed_scripts: bool,
    pub scripts: Vec<String>,
    pub positions: Vec<ScriptInfo>,
}

pub fn unicode_casefold(s: &str) -> String {
    caseless::default_case_fold_str(s)
}

pub fn is_invisible_char(c: char) -> bool {
    let cp = c as u32;
    matches!(cp,
        0x200b | 0x200c | 0x200d | 0x200e | 0x200f |
        0xfeff | 0x00a0 | 0x2028 | 0x2029 |
        0x2060 | 0x00ad | 0x180e | 0x034f |
        0x202a..=0x202e | 0x2066..=0x2069 |
        0xfe00..=0xfe0f
    )
}

pub fn is_known_invisible_char(c: char) -> bool {
    matches!(
        c,
        '\u{200b}'
            | '\u{200c}'
            | '\u{200d}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{feff}'
            | '\u{00a0}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{2060}'
            | '\u{00ad}'
            | '\u{180e}'
            | '\u{034f}'
            | '\u{202a}'
            | '\u{202b}'
            | '\u{202c}'
            | '\u{202d}'
            | '\u{202e}'
            | '\u{2066}'
            | '\u{2067}'
            | '\u{2068}'
            | '\u{2069}'
    )
}

pub fn is_combining_mark(c: char) -> bool {
    get_general_category(c).abbreviation().starts_with('M')
}

pub fn invisible_display_name(c: char) -> &'static str {
    match c {
        '\u{200b}' => "ZWSP",
        '\u{200c}' => "ZWNJ",
        '\u{200d}' => "ZWJ",
        '\u{200e}' => "LRM",
        '\u{200f}' => "RLM",
        '\u{feff}' => "BOM",
        '\u{00a0}' => "NBSP",
        '\u{2028}' => "LINE SEP",
        '\u{2029}' => "PARA SEP",
        '\u{2060}' => "WORD JOINER",
        '\u{00ad}' => "SHY",
        '\u{180e}' => "MVS",
        '\u{034f}' => "CGJ",
        '\u{202a}' => "LRE",
        '\u{202b}' => "RLE",
        '\u{202c}' => "PDF",
        '\u{202d}' => "LRO",
        '\u{202e}' => "RLO",
        '\u{2066}' => "LRI",
        '\u{2067}' => "RLI",
        '\u{2068}' => "FSI",
        '\u{2069}' => "PDI",
        _ => "CTRL",
    }
}

pub fn bidi_display_name(c: char) -> &'static str {
    match c {
        '\u{202a}' => "LRE",
        '\u{202b}' => "RLE",
        '\u{202c}' => "PDF",
        '\u{202d}' => "LRO",
        '\u{202e}' => "RLO",
        '\u{2066}' => "LRI",
        '\u{2067}' => "RLI",
        '\u{2068}' => "FSI",
        '\u{2069}' => "PDI",
        _ => "BIDI",
    }
}

pub fn unicode_name_char(c: char) -> String {
    unicode_names2::name(c)
        .map(|name| name.to_string())
        .unwrap_or_else(|| "<unknown>".to_string())
}

pub fn find_invisibles(text: &str) -> Vec<InvisibleCharInfo> {
    let mut result = Vec::new();

    for (index, c) in text.chars().enumerate() {
        let cp = c as u32;
        let category = get_general_category(c).abbreviation().to_string();

        let (name, display) = if is_known_invisible_char(c) {
            (
                match c {
                    '\u{200b}' => "ZERO WIDTH SPACE".to_string(),
                    '\u{200c}' => "ZERO WIDTH NON-JOINER".to_string(),
                    '\u{200d}' => "ZERO WIDTH JOINER".to_string(),
                    '\u{200e}' => "LEFT-TO-RIGHT MARK".to_string(),
                    '\u{200f}' => "RIGHT-TO-LEFT MARK".to_string(),
                    '\u{feff}' => "ZERO WIDTH NO-BREAK SPACE".to_string(),
                    '\u{00a0}' => "NO-BREAK SPACE".to_string(),
                    '\u{2028}' => "LINE SEPARATOR".to_string(),
                    '\u{2029}' => "PARAGRAPH SEPARATOR".to_string(),
                    '\u{2060}' => "WORD JOINER".to_string(),
                    '\u{202a}' => "LEFT-TO-RIGHT EMBEDDING".to_string(),
                    '\u{202b}' => "RIGHT-TO-LEFT EMBEDDING".to_string(),
                    '\u{202c}' => "POP DIRECTIONAL FORMATTING".to_string(),
                    '\u{202d}' => "LEFT-TO-RIGHT OVERRIDE".to_string(),
                    '\u{202e}' => "RIGHT-TO-LEFT OVERRIDE".to_string(),
                    '\u{2066}' => "LEFT-TO-RIGHT ISOLATE".to_string(),
                    '\u{2067}' => "RIGHT-TO-LEFT ISOLATE".to_string(),
                    '\u{2068}' => "FIRST STRONG ISOLATE".to_string(),
                    '\u{2069}' => "POP DIRECTIONAL ISOLATE".to_string(),
                    '\u{00ad}' => "SOFT HYPHEN".to_string(),
                    '\u{180e}' => "MONGOLIAN VOWEL SEPARATOR".to_string(),
                    '\u{034f}' => "COMBINING GRAPHEME JOINER".to_string(),
                    _ => unicode_name_char(c),
                },
                invisible_display_name(c).to_string(),
            )
        } else if (0xFE00..=0xFE0F).contains(&cp) {
            ("VARIATION SELECTOR".to_string(), "VS".to_string())
        } else if (0x2061..=0x2065).contains(&cp) {
            let display = match cp {
                0x2061 => "FUNCTION APPLICATION",
                0x2062 => "INVISIBLE TIMES",
                0x2063 => "INVISIBLE SEPARATOR",
                0x2064 => "INVISIBLE PLUS",
                0x2065 => "INVISIBLE",
                _ => "INVISIBLE",
            };
            (unicode_name_char(c), display.to_string())
        } else if (0x206A..=0x206F).contains(&cp) {
            (
                unicode_name_char(c),
                format!("BIDI:{}", bidi_display_name(c)),
            )
        } else if category.starts_with('M') {
            (unicode_name_char(c), "CM".to_string())
        } else if category.starts_with('C') && c != '\n' && c != '\t' && c != '\r' {
            (unicode_name_char(c), "CTRL".to_string())
        } else {
            continue;
        };

        result.push(InvisibleCharInfo {
            index,
            char: c,
            codepoint: format!("U+{:04X}", cp),
            name,
            category,
            display,
        });
    }

    result
}

pub fn build_safe_repr(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            ' ' => result.push('\u{2420}'),
            '\t' => result.push('\u{2409}'),
            '\n' => result.push('\u{240A}'),
            '\r' => result.push('\u{240D}'),
            _ if (0xFE00..=0xFE0F).contains(&(c as u32)) => {
                result.push_str("\u{27E6}VS\u{27E7}");
            }
            _ if matches!(c as u32, 0x202a..=0x202e | 0x2066..=0x2069) => {
                let name = bidi_display_name(c);
                result.push_str(&format!("\u{27E6}{}\u{27E7}", name));
            }
            _ if is_combining_mark(c) => {
                result.push('\u{25CC}');
                result.push(c);
            }
            _ if is_known_invisible_char(c) => {
                let display = invisible_display_name(c);
                result.push_str(&format!("\u{27E6}{}\u{27E7}", display));
            }
            _ => result.push(c),
        }
    }
    result
}

pub fn script_name(c: char) -> String {
    let cp = c as u32;
    if get_general_category(c).abbreviation().starts_with('M') {
        "Inherited".to_string()
    } else if (0x3000..=0x303F).contains(&cp) {
        "CJK".to_string()
    } else if (0x0041..=0x024F).contains(&cp) || (0x1E00..=0x1EFF).contains(&cp) {
        "Latin".to_string()
    } else if (0x0400..=0x04FF).contains(&cp) || (0x0500..=0x052F).contains(&cp) {
        "Cyrillic".to_string()
    } else if (0x0370..=0x03FF).contains(&cp) {
        "Greek".to_string()
    } else if (0x0590..=0x05FF).contains(&cp) {
        "Hebrew".to_string()
    } else if (0x0600..=0x06FF).contains(&cp) || (0x0750..=0x077F).contains(&cp) {
        "Arabic".to_string()
    } else if (0x0900..=0x097F).contains(&cp) {
        "Devanagari".to_string()
    } else if (0x0E00..=0x0E7F).contains(&cp) {
        "Thai".to_string()
    } else if (0x3040..=0x309F).contains(&cp) {
        "Hiragana".to_string()
    } else if (0x30A0..=0x30FF).contains(&cp) {
        "Katakana".to_string()
    } else if (0x4E00..=0x9FFF).contains(&cp) || (0x3400..=0x4DBF).contains(&cp) {
        "Han".to_string()
    } else if (0xAC00..=0xD7AF).contains(&cp) {
        "Hangul".to_string()
    } else if (0x10A0..=0x10FF).contains(&cp) {
        "Georgian".to_string()
    } else if (0x0530..=0x058F).contains(&cp) {
        "Armenian".to_string()
    } else if (0x13A0..=0x13FF).contains(&cp) {
        "Cherokee".to_string()
    } else if (0x1400..=0x167F).contains(&cp) {
        "Canadian_Aboriginal".to_string()
    } else {
        "Other".to_string()
    }
}

pub fn detect_mixed_scripts(text: &str) -> MixedScriptsResult {
    let mut scripts = std::collections::BTreeSet::new();
    let mut positions = Vec::new();

    for (index, c) in text.chars().enumerate() {
        let script = script_name(c);
        if script != "Common" && script != "Inherited" && script != "Other" {
            scripts.insert(script.clone());
            positions.push(ScriptInfo {
                index,
                char: c,
                script,
                codepoint: format!("U+{:04X}", c as u32),
            });
        }
    }

    MixedScriptsResult {
        mixed_scripts: scripts.len() > 1,
        scripts: scripts.into_iter().collect(),
        positions,
    }
}

pub fn detect_newline_style(text: &str) -> String {
    let has_crlf = text.contains("\r\n");
    let standalone_cr = text.matches('\r').count() - text.matches("\r\n").count();
    let standalone_lf = text.matches('\n').count() - text.matches("\r\n").count();

    if (has_crlf && (standalone_cr > 0 || standalone_lf > 0))
        || (standalone_cr > 0 && standalone_lf > 0)
    {
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

pub fn unicode_scripts(s: &str) -> Vec<String> {
    s.chars().map(script_name).collect()
}

pub fn confusables_count(s: &str) -> usize {
    use crate::text::confusables::CONFUSABLES;
    s.chars()
        .filter(|c| {
            let key = format!("U+{:04X}", *c as u32);
            CONFUSABLES.get(key.as_str()).is_some()
        })
        .count()
}

pub fn reverse_confusables(ch: char) -> Result<Vec<String>, String> {
    use crate::text::confusables::CONFUSABLES;
    use std::collections::HashMap;
    use std::sync::LazyLock;

    static REVERSE_INDEX: LazyLock<HashMap<String, Vec<String>>> = LazyLock::new(|| {
        let mut index: HashMap<String, Vec<String>> = HashMap::new();
        for (&source_cp, &target_cps_str) in CONFUSABLES.iter() {
            for target_cp in target_cps_str.split_whitespace() {
                index
                    .entry(target_cp.to_string())
                    .or_default()
                    .push(source_cp.to_string());
            }
        }
        index
    });

    let target_cp = format!("U+{:04X}", ch as u32);
    Ok(REVERSE_INDEX
        .get(&target_cp)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|cp| {
            let hex = cp.strip_prefix("U+")?;
            let code = u32::from_str_radix(hex, 16).ok()?;
            char::from_u32(code).map(|c| c.to_string())
        })
        .collect())
}
