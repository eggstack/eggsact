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

/// Typed Unicode hazard classification for security decisions.
///
/// This is the single non-presentation source for hazard membership consumed
/// by `text_measure`, `inspect_text_security`, and Unicode policy paths.
/// Human-readable `display` and Unicode names must never be used as security
/// predicates; use [`is_bidi_control`] / [`classify_hazard`] instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnicodeHazard {
    /// Explicit bidirectional controls (UAX #9): LRM, RLM, LRE, RLE, PDF,
    /// LRO, RLO, LRI, RLI, FSI, PDI.
    BidiControl,
    /// Join controls: ZWNJ, ZWJ.
    JoinControl,
    /// Other invisible formatting: ZWSP, BOM/ZWNBSP, NBSP, line/paragraph
    /// separators, word joiner, soft hyphen, Mongolian vowel separator,
    /// combining grapheme joiner, invisible mathematical operators, and
    /// deprecated formatting controls.
    InvisibleFormat,
    /// Variation selectors (U+FE00..U+FE0F).
    VariationSelector,
    /// Combining marks (General_Category=M); inherit the base script.
    CombiningMark,
    /// Ordinary controls (General_Category=C, excluding \n \t \r).
    Control,
}

/// The 11 modern bidirectional controls (UAX #9).
pub const BIDI_CONTROLS: &[char] = &[
    '\u{200E}', '\u{200F}', '\u{202A}', '\u{202B}', '\u{202C}', '\u{202D}', '\u{202E}', '\u{2066}',
    '\u{2067}', '\u{2068}', '\u{2069}',
];

/// Join controls: zero-width non-joiner and zero-width joiner.
pub const JOIN_CONTROLS: &[char] = &['\u{200C}', '\u{200D}'];

/// Whether `c` is an explicit bidirectional control.
///
/// Exactly the 11 members of [`BIDI_CONTROLS`]: LRM, RLM, LRE, RLE, PDF,
/// LRO, RLO, LRI, RLI, FSI, PDI. Deprecated formatting controls
/// (U+206A..U+206F) and invisible mathematical operators (U+2061..U+2065)
/// are *not* bidi controls; they classify as [`UnicodeHazard::InvisibleFormat`].
pub fn is_bidi_control(c: char) -> bool {
    BIDI_CONTROLS.contains(&c)
}

/// Whether `c` is a join control (ZWNJ or ZWJ).
pub fn is_join_control(c: char) -> bool {
    JOIN_CONTROLS.contains(&c)
}

/// Typed hazard classification for one character.
///
/// Returns `None` for ordinary text (including newlines, tabs, and carriage
/// returns). Combining marks report [`UnicodeHazard::CombiningMark`] and
/// variation selectors report [`UnicodeHazard::VariationSelector`] so
/// consumers can retain those distinctions instead of collapsing everything
/// into a single "invisible" bucket.
pub fn classify_hazard(c: char) -> Option<UnicodeHazard> {
    let cp = c as u32;
    if is_bidi_control(c) {
        return Some(UnicodeHazard::BidiControl);
    }
    if is_join_control(c) {
        return Some(UnicodeHazard::JoinControl);
    }
    if (0xFE00..=0xFE0F).contains(&cp) {
        return Some(UnicodeHazard::VariationSelector);
    }
    // InvisibleFormat before CombiningMark: U+034F (CGJ, Mn) is a format
    // control and a Default_Ignorable, not an ordinary accent. Other Mn
    // (e.g. U+0301) still report CombiningMark below.
    if matches!(
        cp,
        0x200B | 0xFEFF | 0x00A0 | 0x2028 | 0x2029 | 0x2060 | 0x00AD | 0x180E | 0x034F
            | 0x2061..=0x2065
            | 0x206A..=0x206F
    ) {
        return Some(UnicodeHazard::InvisibleFormat);
    }
    if get_general_category(c).abbreviation().starts_with('M') {
        return Some(UnicodeHazard::CombiningMark);
    }
    if get_general_category(c).abbreviation().starts_with('C')
        && c != '\n'
        && c != '\t'
        && c != '\r'
    {
        return Some(UnicodeHazard::Control);
    }
    None
}

/// User-visible invisible hazard: BidiControl, JoinControl, InvisibleFormat,
/// or VariationSelector per the central [`classify_hazard`].
///
/// This is the single security-verdict predicate for "invisible" used by
/// policy and identifier paths. It is intentionally distinct from
/// `Default_Ignorable_Code_Point` (a Unicode property used by the skeleton);
/// see [`crate::text::unicode_properties::is_default_ignorable`].
pub fn has_security_invisible_hazard(c: char) -> bool {
    matches!(
        classify_hazard(c),
        Some(
            UnicodeHazard::BidiControl
                | UnicodeHazard::JoinControl
                | UnicodeHazard::InvisibleFormat
                | UnicodeHazard::VariationSelector
        )
    )
}

/// Zero-width characters for policy findings (single source).
pub fn is_zero_width_char(c: char) -> bool {
    matches!(c, '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{2060}')
}

pub fn is_invisible_char(c: char) -> bool {
    // Single source: the typed hazard classifier (presentation-adjacent
    // compatibility wrapper; security verdicts use
    // `has_security_invisible_hazard` directly).
    has_security_invisible_hazard(c)
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
            _ if (c as u32) < 32 || c as u32 == 0x7f || c == '\\' => {
                result.push_str(&format!("\u{27E6}U+{:04X}\u{27E7}", c as u32));
            }
            _ => result.push(c),
        }
    }
    result
}

/// Authoritative script identity for one character.
///
/// Delegates to [`crate::text::script::script_of`], the single source of
/// truth for script identity. Returns `"Common"`, `"Inherited"`, a script
/// name, or `"Other"`.
pub fn script_name(c: char) -> String {
    crate::text::script::script_of(c).to_string()
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

    // UTS #39 §5.1 resolved-script verdict (authoritative); the observed
    // per-character list above is preserved for diagnostics.
    let mixed_scripts = crate::text::script::is_mixed_script(text);

    MixedScriptsResult {
        mixed_scripts,
        scripts: scripts.into_iter().collect(),
        positions,
    }
}

pub fn detect_newline_style(text: &str) -> String {
    crate::text::primitives::detect_newline_style(text).to_string()
}

pub fn unicode_scripts(s: &str) -> Vec<String> {
    s.chars().map(script_name).collect()
}

pub fn confusables_count(s: &str) -> usize {
    use crate::text::confusables::lookup;
    s.chars().filter(|c| lookup(*c).is_some()).count()
}

/// Source characters whose confusable mapping is exactly the single target
/// character `ch`.
///
/// Only single-code-point mappings are indexed: a source such as `Æ`
/// (which maps to the two-code-point sequence `U+0041 U+0045`) is *not*
/// equivalent to `A` alone, so multi-code-point sources are excluded rather
/// than misrepresented as single-code-point equivalence. Whole-string
/// confusability must use [`crate::text::confusables::are_confusable`].
pub fn reverse_confusables(ch: char) -> Result<Vec<String>, String> {
    use crate::text::confusables::CONFUSABLES;
    use std::collections::HashMap;
    use std::sync::LazyLock;

    static REVERSE_INDEX: LazyLock<HashMap<String, Vec<String>>> = LazyLock::new(|| {
        let mut index: HashMap<String, Vec<String>> = HashMap::new();
        for &(source_cp, target_cps_str) in CONFUSABLES.iter() {
            // Single-code-point targets only; skip multi-code-point
            // sequences (component mapping is not equivalence).
            let mut parts = target_cps_str.split_whitespace();
            let (Some(only), None) = (parts.next(), parts.next()) else {
                continue;
            };
            index
                .entry(only.to_string())
                .or_default()
                .push(format!("U+{:04X}", source_cp));
        }
        index
    });

    let target_cp = format!("U+{:04X}", ch as u32);
    Ok(REVERSE_INDEX
        .get(&target_cp)
        .map(|v| {
            v.iter()
                .filter_map(|cp| {
                    let hex = cp.strip_prefix("U+")?;
                    let code = u32::from_str_radix(hex, 16).ok()?;
                    char::from_u32(code).map(|c| c.to_string())
                })
                .collect()
        })
        .unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bidi_membership_covers_all_supported_controls() {
        // Every bidi control named by Milestone 003 classifies as bidi.
        for c in [
            '\u{202A}', // LRE
            '\u{202B}', // RLE
            '\u{202C}', // PDF
            '\u{202D}', // LRO
            '\u{202E}', // RLO
            '\u{2066}', // LRI
            '\u{2067}', // RLI
            '\u{2068}', // FSI
            '\u{2069}', // PDI
            '\u{200E}', // LRM
            '\u{200F}', // RLM
        ] {
            assert!(
                is_bidi_control(c),
                "{c:?} (U+{:04X}) must be bidi",
                c as u32
            );
            assert_eq!(classify_hazard(c), Some(UnicodeHazard::BidiControl));
        }
    }

    #[test]
    fn bidi_membership_excludes_lookalikes() {
        // Ordinary RLO/LRO displays ("RLO", not "BIDI") were the reason
        // display-string predicates failed; the typed classifier must also
        // exclude neighboring invisible/format characters.
        for c in [
            'A', ' ', '\u{200B}', // ZWSP
            '\u{200C}', // ZWNJ (join control, not bidi)
            '\u{200D}', // ZWJ (join control, not bidi)
            '\u{2061}', // FUNCTION APPLICATION (math invisible)
            '\u{206A}', // deprecated formatting (invisible format, not bidi)
            '\u{FE00}', // variation selector
            '\u{0301}', // combining mark
            '\u{0001}', // ordinary control
        ] {
            assert!(!is_bidi_control(c), "{c:?} must not be bidi");
        }
        assert_eq!(
            classify_hazard('\u{200C}'),
            Some(UnicodeHazard::JoinControl)
        );
        assert_eq!(
            classify_hazard('\u{200D}'),
            Some(UnicodeHazard::JoinControl)
        );
        assert_eq!(
            classify_hazard('\u{FE00}'),
            Some(UnicodeHazard::VariationSelector)
        );
        assert_eq!(
            classify_hazard('\u{0301}'),
            Some(UnicodeHazard::CombiningMark)
        );
        assert_eq!(classify_hazard('\u{0001}'), Some(UnicodeHazard::Control));
        assert_eq!(
            classify_hazard('\u{2061}'),
            Some(UnicodeHazard::InvisibleFormat)
        );
        assert_eq!(classify_hazard('A'), None);
        assert_eq!(classify_hazard('\n'), None);
    }

    #[test]
    fn reverse_confusables_excludes_multi_code_point_sources() {
        // Æ maps to U+0041 U+0045: a component mapping, not equivalence with
        // 'A' alone, so it must not appear in reverse('A').
        let back = reverse_confusables('A').unwrap();
        assert!(
            back.iter().any(|s| s == "А"),
            "Cyrillic А must reverse to A"
        );
        assert!(
            !back.iter().any(|s| s == "Æ"),
            "Æ must not reverse to single 'A': {back:?}"
        );
    }
}
