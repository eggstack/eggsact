use fancy_regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::LazyLock;

use crate::text::confusables::find_confusables;
use crate::text::diff::levenshtein_distance;

static PYTHON_IDENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[\p{XID_Start}_][\p{XID_Continue}_]*$").unwrap());

static RUST_IDENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_]*$").unwrap());

static JS_IDENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-zA-Z_$][a-zA-Z0-9_$]*$").unwrap());

static ENV_IDENT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Z_][A-Z0-9_]*$").unwrap());

static STRIP_STYLE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[_\-]").unwrap());

const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while",
];

const JS_KEYWORDS: &[&str] = &[
    "break",
    "case",
    "catch",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "import",
    "in",
    "instanceof",
    "let",
    "new",
    "null",
    "return",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "yield",
];

const TS_KEYWORDS: &[&str] = &[
    // JS base keywords
    "break",
    "case",
    "catch",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "import",
    "in",
    "instanceof",
    "let",
    "new",
    "null",
    "return",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "yield",
    // TS-specific keywords
    "any",
    "boolean",
    "constructor",
    "declare",
    "get",
    "module",
    "require",
    "number",
    "set",
    "string",
    "symbol",
    "type",
    "from",
    "of",
    "readonly",
    "abstract",
    "as",
    "async",
    "await",
    "implements",
    "interface",
    "is",
    "keyof",
    "namespace",
    "package",
    "private",
    "protected",
    "public",
    "override",
];

const PYTHON_KEYWORDS: &[&str] = &[
    "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class", "continue",
    "def", "del", "elif", "else", "except", "finally", "for", "from", "global", "if", "import",
    "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while",
    "with", "yield",
];

const INVISIBLE_CHARS: &[char] = &[
    '\u{200b}', '\u{200c}', '\u{200d}', '\u{200e}', '\u{200f}', '\u{feff}', '\u{00a0}', '\u{2028}',
    '\u{2029}', '\u{202a}', '\u{202b}', '\u{202c}', '\u{202d}', '\u{202e}', '\u{2066}', '\u{2067}',
    '\u{2068}', '\u{2069}', '\u{2060}',
];

const SCRIPT_RANGES: &[(u32, u32, &str)] = &[
    (0x0041, 0x005a, "Latin"),
    (0x0061, 0x007a, "Latin"),
    (0x00c0, 0x00ff, "Latin"),
    (0x0100, 0x017f, "Latin"),
    (0x0180, 0x024f, "Latin"),
    (0x0400, 0x04ff, "Cyrillic"),
    (0x0500, 0x052f, "Cyrillic"),
    (0x0370, 0x03ff, "Greek"),
    (0x1f00, 0x1fff, "Greek"),
    (0x4e00, 0x9fff, "Han"),
    (0x3000, 0x303f, "CJK"),
    (0x3040, 0x309f, "Hiragana"),
    (0x30a0, 0x30ff, "Katakana"),
    (0x0600, 0x06ff, "Arabic"),
    (0x0590, 0x05ff, "Hebrew"),
    (0x0900, 0x097f, "Devanagari"),
    (0x0e00, 0x0e7f, "Thai"),
    (0xac00, 0xd7af, "Hangul"),
    (0x10a0, 0x10ff, "Georgian"),
    (0x0530, 0x058f, "Armenian"),
    (0x13a0, 0x13ff, "Cherokee"),
    (0x1400, 0x167f, "Canadian_Aboriginal"),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentifierAnalyzeResult {
    pub text: String,
    pub classification: String,
    pub python_valid: bool,
    pub python_keyword: bool,
    pub rust_valid: Option<bool>,
    pub javascript_valid: Option<bool>,
    pub env_valid: bool,
    pub suggestions: HashMap<String, String>,
    pub warnings: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentifierInfo {
    pub raw: String,
    pub normalized: String,
    pub valid: bool,
    pub scripts: Vec<String>,
    pub has_invisibles: bool,
    pub has_confusables: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollisionInfo {
    pub kind: String,
    pub a: String,
    pub b: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentifierInspectResult {
    pub identifiers: Vec<IdentifierInfo>,
    pub collisions: Vec<CollisionInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableCollisionInfo {
    pub kind: String,
    pub names: Vec<String>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReservedKeywordHit {
    pub name: String,
    pub language: String,
    pub file: String,
    pub line: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixedStyleGroup {
    pub stripped: String,
    pub names: Vec<String>,
    pub styles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentifierTableInspectResult {
    pub count: i32,
    pub collisions: Vec<TableCollisionInfo>,
    pub reserved_keyword_hits: Vec<ReservedKeywordHit>,
    pub mixed_style_groups: Vec<MixedStyleGroup>,
    pub findings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableIdentifierEntry {
    pub name: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub file: String,
    #[serde(default)]
    pub line: i32,
}

fn _is_valid_ident_chars(text: &str, extra_chars: &str) -> bool {
    for c in text.chars() {
        if !c.is_alphanumeric() && c != '_' && !extra_chars.contains(c) {
            return false;
        }
    }
    true
}

fn _is_snake_case(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    if !text.contains('_') {
        return false;
    }
    if !_is_valid_ident_chars(text, "") {
        return false;
    }
    for part in text.split('_') {
        if !part.is_empty() && !part.chars().all(|c| c.is_lowercase()) {
            return false;
        }
    }
    true
}

fn _is_camel_case(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    if text
        .chars()
        .next()
        .map(|c| c.is_uppercase())
        .unwrap_or(false)
    {
        return false;
    }
    if text.contains('_') || text.contains('-') {
        return false;
    }
    if !is_valid_rust_identifier(text) {
        return false;
    }
    text.chars().any(|c| c.is_uppercase())
}

fn _is_pascal_case(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    if text
        .chars()
        .next()
        .map(|c| c.is_lowercase())
        .unwrap_or(false)
    {
        return false;
    }
    if text.contains('_') || text.contains('-') {
        return false;
    }
    if !is_valid_rust_identifier(text) {
        return false;
    }
    text.chars().any(|c| c.is_uppercase())
}

fn _is_kebab_case(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    if !text.contains('-') {
        return false;
    }
    if !_is_valid_ident_chars(text, "-") {
        return false;
    }
    for part in text.split('-') {
        if !part.is_empty() && !part.chars().all(|c| c.is_lowercase()) {
            return false;
        }
    }
    true
}

fn _is_screaming_snake_case(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    if !_is_valid_ident_chars(text, "") {
        return false;
    }
    for part in text.split('_') {
        if !part.is_empty() && !part.chars().all(|c| c.is_uppercase()) {
            return false;
        }
    }
    true
}

fn classify_style(text: &str) -> String {
    if text.is_empty() {
        return "invalid".to_string();
    }

    if (text.starts_with('_') || text.starts_with(char::is_lowercase))
        && text.contains('_')
        && !text.contains('-')
    {
        let parts: Vec<&str> = text.split('_').filter(|p| !p.is_empty()).collect();
        if !parts.is_empty() && parts.iter().all(|p| p.chars().all(|c| c.is_lowercase())) {
            return "snake_case".to_string();
        }
    }

    if text.contains('_') && !text.contains('-') {
        let parts: Vec<&str> = text.split('_').filter(|p| !p.is_empty()).collect();
        if !parts.is_empty() && parts.iter().all(|p| p.chars().all(|c| c.is_uppercase())) {
            return "SCREAMING_SNAKE_CASE".to_string();
        }
    }

    if text.starts_with(char::is_uppercase)
        && !text.contains('_')
        && !text.contains('-')
        && is_valid_rust_identifier(text)
        && text.chars().any(|c| c.is_uppercase())
    {
        return "PascalCase".to_string();
    }

    if text.contains('-') && !text.contains('_') {
        let parts: Vec<&str> = text.split('-').collect();
        if parts
            .iter()
            .all(|p| p.is_empty() || p.chars().all(|c| c.is_lowercase()))
        {
            return "kebab-case".to_string();
        }
    }

    if text.starts_with(char::is_lowercase)
        && !text.contains('_')
        && !text.contains('-')
        && is_valid_rust_identifier(text)
        && text.chars().any(|c| c.is_uppercase())
    {
        return "camelCase".to_string();
    }

    if is_valid_rust_identifier(text) {
        return "mixed".to_string();
    }

    "invalid".to_string()
}

fn to_snake_case(text: &str) -> String {
    let mut result = String::new();
    let mut prev_upper = false;
    let mut prev_underscore = false;
    let chars: Vec<char> = text.chars().collect();

    for (i, c) in chars.iter().enumerate() {
        if *c == '_' || *c == '-' {
            prev_underscore = true;
            continue;
        }
        if c.is_uppercase() {
            if !result.is_empty()
                && !prev_underscore
                && (prev_upper || (i + 1 < chars.len() && chars[i + 1].is_uppercase()))
            {
                result.push('_');
            }
            result.push(c.to_ascii_lowercase());
            prev_upper = true;
        } else {
            result.push(*c);
            prev_upper = false;
        }
        prev_underscore = false;
    }
    result
}

fn to_pascal_case(text: &str) -> String {
    let snake = to_snake_case(text);
    let parts: Vec<&str> = snake.split('_').collect();
    let mut result = String::new();
    for part in parts {
        if !part.is_empty() {
            let mut chars = part.chars();
            if let Some(first) = chars.next() {
                result.push(first.to_ascii_uppercase());
                for c in chars {
                    result.push(c.to_ascii_lowercase());
                }
            }
        }
    }
    result
}

fn to_camel_case(text: &str) -> String {
    let pascal = to_pascal_case(text);
    if !pascal.is_empty() {
        let mut chars = pascal.chars();
        if let Some(first) = chars.next() {
            return first.to_ascii_lowercase().to_string() + &chars.collect::<String>();
        }
    }
    pascal
}

fn to_kebab_case(text: &str) -> String {
    to_snake_case(text).replace('_', "-")
}

fn to_screaming_snake_case(text: &str) -> String {
    to_snake_case(text).to_uppercase()
}

fn is_valid_python_identifier(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    PYTHON_IDENT_RE.is_match(text).unwrap_or(false)
}

fn is_valid_rust_identifier(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    RUST_IDENT_RE.is_match(text).unwrap_or(false)
}

fn is_valid_js_identifier(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    JS_IDENT_RE.is_match(text).unwrap_or(false)
}

fn is_env_valid(text: &str) -> bool {
    ENV_IDENT_RE.is_match(text).unwrap_or(false)
}

fn is_python_keyword(text: &str) -> bool {
    PYTHON_KEYWORDS.contains(&text)
}

fn is_rust_keyword(text: &str) -> bool {
    RUST_KEYWORDS.contains(&text)
}

fn is_js_keyword(text: &str) -> bool {
    JS_KEYWORDS.contains(&text)
}

fn _is_ts_keyword(text: &str) -> bool {
    TS_KEYWORDS.contains(&text)
}

fn get_script_heuristic(c: char) -> String {
    let cp = c as u32;

    if cp == 0x200B
        || cp == 0x200C
        || cp == 0x200D
        || cp == 0x200E
        || cp == 0x200F
        || cp == 0xFEFF
        || cp == 0x00A0
        || cp == 0x2028
        || cp == 0x2029
        || cp == 0x202A
        || cp == 0x202B
        || cp == 0x202C
        || cp == 0x202D
        || cp == 0x202E
        || cp == 0x2066
        || cp == 0x2067
        || cp == 0x2068
        || cp == 0x2069
        || cp == 0x2060
    {
        return "Common".to_string();
    }

    for &(start, end, name) in SCRIPT_RANGES {
        if start <= cp && cp <= end {
            return name.to_string();
        }
    }

    "Other".to_string()
}

fn get_scripts(text: &str) -> Vec<String> {
    let mut scripts: std::collections::HashSet<String> = std::collections::HashSet::new();
    for c in text.chars() {
        let script = get_script_heuristic(c);
        if !["Common", "Inherited", "Unknown", "Other"].contains(&script.as_str()) {
            scripts.insert(script);
        }
    }
    let mut sorted: Vec<String> = scripts.into_iter().collect();
    sorted.sort();
    sorted
}

fn has_invisibles(text: &str) -> bool {
    text.chars().any(|c| INVISIBLE_CHARS.contains(&c))
}

fn normalize_text(text: &str, normalization: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    match normalization {
        "NFC" => text.nfc().collect(),
        "NFD" => text.nfd().collect(),
        "NFKC" => text.nfkc().collect(),
        "NFKD" => text.nfkd().collect(),
        _ => text.to_string(),
    }
}

pub fn identifier_analyze(text: &str, languages: Option<Vec<&str>>) -> IdentifierAnalyzeResult {
    let languages = languages.unwrap_or_else(|| vec!["python", "rust", "javascript", "env"]);

    let classification = classify_style(text);

    let python_valid;
    let python_keyword;
    if languages.contains(&"python") {
        python_valid = is_valid_python_identifier(text);
        python_keyword = python_valid && is_python_keyword(text);
    } else {
        python_valid = false;
        python_keyword = false;
    }

    let rust_valid = if languages.contains(&"rust") {
        if is_valid_rust_identifier(text) {
            Some(!is_rust_keyword(text))
        } else {
            Some(false)
        }
    } else {
        None
    };

    let javascript_valid = if languages.contains(&"javascript") {
        Some(is_valid_js_identifier(text))
    } else {
        None
    };

    let env_valid = if languages.contains(&"env") {
        is_env_valid(text)
    } else {
        false
    };

    let mut warnings: Vec<String> = vec![];
    if python_keyword {
        warnings.push("Python keyword - cannot be used as identifier in Python".to_string());
    }
    if rust_valid == Some(false) && languages.contains(&"rust") {
        warnings.push("Rust keyword - cannot be used as identifier in Rust".to_string());
    }
    if classification == "mixed" {
        warnings.push("Identifier has mixed naming convention".to_string());
    }
    if text.starts_with('_') {
        warnings.push(
            "Identifier starts with underscore - typically reserved for private/use-only"
                .to_string(),
        );
    }

    let mut suggestions = HashMap::new();
    suggestions.insert("snake_case".to_string(), to_snake_case(text));
    suggestions.insert("kebab_case".to_string(), to_kebab_case(text));
    suggestions.insert("pascal_case".to_string(), to_pascal_case(text));
    suggestions.insert("camel_case".to_string(), to_camel_case(text));
    suggestions.insert(
        "screaming_snake_case".to_string(),
        to_screaming_snake_case(text),
    );

    let mut summary_parts: Vec<String> = vec![];
    if classification != "invalid" {
        summary_parts.push(format!("Style: {}", classification));
    } else {
        summary_parts.push("Invalid identifier".to_string());
    }

    let mut valid_langs: Vec<&str> = vec![];
    if python_valid && !python_keyword {
        valid_langs.push("Python");
    }
    if rust_valid == Some(true) {
        valid_langs.push("Rust");
    }
    if javascript_valid == Some(true) {
        valid_langs.push("JavaScript");
    }
    if env_valid {
        valid_langs.push("env");
    }

    if !valid_langs.is_empty() {
        summary_parts.push(format!("Valid in: {}", valid_langs.join(", ")));
    }

    if python_keyword {
        summary_parts.push("Python: reserved keyword".to_string());
    }

    let summary = summary_parts.join(". ");

    IdentifierAnalyzeResult {
        text: text.to_string(),
        classification,
        python_valid,
        python_keyword,
        rust_valid,
        javascript_valid,
        env_valid,
        suggestions,
        warnings,
        summary,
    }
}

pub fn identifier_inspect(
    identifiers: &[String],
    language: &str,
    normalization: &str,
    casefold: bool,
    check_confusables: bool,
) -> IdentifierInspectResult {
    let mut id_infos: Vec<IdentifierInfo> = vec![];
    let mut normalized_ids: Vec<String> = vec![];
    let mut collisions: Vec<CollisionInfo> = vec![];
    let mut collision_pairs: std::collections::HashSet<(String, String)> =
        std::collections::HashSet::new();

    for raw_id in identifiers {
        let mut normalized = raw_id.clone();
        if normalization != "raw" {
            normalized = normalize_text(raw_id, normalization);
        }

        let scripts = get_scripts(&normalized);
        let has_invis = has_invisibles(raw_id);

        let confusables_found = if check_confusables {
            find_confusables(&normalized)
        } else {
            vec![]
        };

        let has_confusables = !confusables_found.is_empty();

        let mut valid = true;
        let mut warnings: Vec<String> = vec![];

        match language {
            "python" => {
                valid = is_valid_python_identifier(&normalized) && !is_python_keyword(&normalized);
                if !valid {
                    warnings.push("Invalid Python identifier".to_string());
                }
            }
            "javascript" | "typescript" => {
                valid = is_valid_js_identifier(&normalized) && !is_js_keyword(&normalized);
                if !valid {
                    warnings.push(format!("Invalid {} identifier", language));
                }
            }
            _ => {}
        }

        if has_invis {
            warnings.push("Contains invisible characters".to_string());
        }

        if has_confusables {
            warnings.push("Contains confusable characters".to_string());
        }

        if scripts.len() > 1 {
            warnings.push("Mixed script identifier".to_string());
        }

        id_infos.push(IdentifierInfo {
            raw: raw_id.clone(),
            normalized: normalized.clone(),
            valid,
            scripts,
            has_invisibles: has_invis,
            has_confusables,
            warnings,
        });
        normalized_ids.push(normalized);
    }

    if check_confusables {
        for i in 0..identifiers.len() {
            for j in (i + 1)..identifiers.len() {
                let a_raw = &identifiers[i];
                let b_raw = &identifiers[j];
                let a_norm = &normalized_ids[i];
                let b_norm = &normalized_ids[j];

                let a_confusables = find_confusables(a_norm);
                let b_confusables = find_confusables(b_norm);

                if !a_confusables.is_empty() && !b_confusables.is_empty() {
                    let a_targets: std::collections::HashSet<String> = a_confusables
                        .iter()
                        .map(|(_, target)| (*target).to_string())
                        .collect();
                    let b_targets: std::collections::HashSet<String> = b_confusables
                        .iter()
                        .map(|(_, target)| (*target).to_string())
                        .collect();
                    let shared: std::collections::HashSet<_> =
                        a_targets.intersection(&b_targets).collect();
                    if !shared.is_empty() {
                        let pair = if a_raw <= b_raw {
                            (a_raw.clone(), b_raw.clone())
                        } else {
                            (b_raw.clone(), a_raw.clone())
                        };
                        if !collision_pairs.contains(&pair) {
                            collision_pairs.insert(pair.clone());
                            collisions.push(CollisionInfo {
                                kind: "confusable".to_string(),
                                a: pair.0,
                                b: pair.1,
                            });
                        }
                        continue;
                    }
                }

                for (_, target) in &a_confusables {
                    if b_norm.contains(&target.to_string()) {
                        let pair = if a_raw <= b_raw {
                            (a_raw.clone(), b_raw.clone())
                        } else {
                            (b_raw.clone(), a_raw.clone())
                        };
                        if !collision_pairs.contains(&pair) {
                            collision_pairs.insert(pair.clone());
                            collisions.push(CollisionInfo {
                                kind: "confusable".to_string(),
                                a: pair.0,
                                b: pair.1,
                            });
                        }
                        break;
                    }
                }

                for (_, target) in &b_confusables {
                    if a_norm.contains(&target.to_string()) {
                        let pair = if a_raw <= b_raw {
                            (a_raw.clone(), b_raw.clone())
                        } else {
                            (b_raw.clone(), a_raw.clone())
                        };
                        if !collision_pairs.contains(&pair) {
                            collision_pairs.insert(pair.clone());
                            collisions.push(CollisionInfo {
                                kind: "confusable".to_string(),
                                a: pair.0,
                                b: pair.1,
                            });
                        }
                        break;
                    }
                }
            }
        }
    }

    if casefold {
        let mut casefold_map: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for (i, raw) in identifiers.iter().enumerate() {
            let cf_key = caseless::default_case_fold_str(&normalized_ids[i]);
            casefold_map.entry(cf_key).or_default().push(raw.clone());
        }

        for (_, items) in casefold_map.iter().filter(|(_, v)| v.len() > 1) {
            for i in 0..items.len() {
                for j in (i + 1)..items.len() {
                    let pair = if items[i] <= items[j] {
                        (items[i].clone(), items[j].clone())
                    } else {
                        (items[j].clone(), items[i].clone())
                    };
                    if !collision_pairs.contains(&pair) {
                        collision_pairs.insert(pair.clone());
                        collisions.push(CollisionInfo {
                            kind: "casefold".to_string(),
                            a: pair.0,
                            b: pair.1,
                        });
                    }
                }
            }
        }
    }

    if normalization != "raw" {
        let mut norm_map: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for (i, raw) in identifiers.iter().enumerate() {
            norm_map
                .entry(normalized_ids[i].clone())
                .or_default()
                .push(raw.clone());
        }

        for (_, items) in norm_map.iter().filter(|(_, v)| v.len() > 1) {
            let originals: Vec<String> = items.to_vec();
            for i in 0..originals.len() {
                for j in (i + 1)..originals.len() {
                    let pair = if originals[i] <= originals[j] {
                        (originals[i].clone(), originals[j].clone())
                    } else {
                        (originals[j].clone(), originals[i].clone())
                    };
                    if !collision_pairs.contains(&pair) {
                        collision_pairs.insert(pair.clone());
                        collisions.push(CollisionInfo {
                            kind: "normalization".to_string(),
                            a: pair.0,
                            b: pair.1,
                        });
                    }
                }
            }
        }
    }

    IdentifierInspectResult {
        identifiers: id_infos,
        collisions,
    }
}

fn strip_style(name: &str) -> String {
    STRIP_STYLE_RE.replace_all(name, "").to_lowercase()
}

fn get_lang_keywords(language: &str) -> Vec<&'static str> {
    match language {
        "python" => PYTHON_KEYWORDS.to_vec(),
        "rust" => RUST_KEYWORDS.to_vec(),
        "javascript" => JS_KEYWORDS.to_vec(),
        "typescript" => TS_KEYWORDS.to_vec(),
        _ => vec![],
    }
}

pub fn identifier_table_inspect(
    identifiers: &[TableIdentifierEntry],
    language: &str,
    checks: Option<Vec<&str>>,
) -> IdentifierTableInspectResult {
    let default_checks = vec![
        "casefold",
        "normalization",
        "confusable",
        "style",
        "reserved",
        "mixed_style",
    ];
    let checks = checks.unwrap_or(default_checks);
    let valid_checks = [
        "casefold",
        "normalization",
        "confusable",
        "style",
        "reserved",
        "mixed_style",
    ];
    let active_checks: Vec<&str> = checks
        .iter()
        .filter(|c| valid_checks.contains(c))
        .cloned()
        .collect();

    let count = identifiers.len() as i32;
    let mut collisions: Vec<TableCollisionInfo> = vec![];
    let mut reserved_hits: Vec<ReservedKeywordHit> = vec![];
    let mut mixed_style_groups: Vec<MixedStyleGroup> = vec![];
    let mut findings: Vec<String> = vec![];

    let names: Vec<String> = identifiers.iter().map(|e| e.name.clone()).collect();

    if active_checks.contains(&"casefold") {
        let mut cf_map: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for name in &names {
            let cf_key = name.to_lowercase();
            cf_map.entry(cf_key).or_default().push(name.clone());
        }
        for (_, group) in cf_map.iter().filter(|(_, v)| v.len() > 1) {
            collisions.push(TableCollisionInfo {
                kind: "casefold".to_string(),
                names: group.clone(),
                detail: format!("Casefold collision: {}", group.join(", ")),
            });
        }
        if !cf_map.is_empty() && cf_map.values().any(|g| g.len() > 1) {
            findings.push("Casefold collisions detected".to_string());
        }
    }

    if active_checks.contains(&"normalization") {
        let mut nfc_map: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for name in &names {
            let nfc_key = normalize_text(name, "NFC");
            nfc_map.entry(nfc_key).or_default().push(name.clone());
        }
        for (nfc_key, group) in nfc_map.iter().filter(|(_, v)| {
            let unique: std::collections::HashSet<_> = v.iter().cloned().collect();
            unique.len() > 1
        }) {
            let originals: Vec<String> = group
                .iter()
                .cloned()
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();
            collisions.push(TableCollisionInfo {
                kind: "normalization".to_string(),
                names: originals.clone(),
                detail: format!(
                    "Normalization collision (NFC '{}'): {}",
                    nfc_key,
                    originals.join(", ")
                ),
            });
        }
        if !nfc_map.is_empty()
            && nfc_map.values().any(|g| {
                let unique: std::collections::HashSet<_> = g.iter().cloned().collect();
                unique.len() > 1
            })
        {
            findings.push("Normalization collisions detected".to_string());
        }
    }

    if active_checks.contains(&"confusable") {
        let mut checked_pairs: std::collections::HashSet<(String, String)> =
            std::collections::HashSet::new();
        for i in 0..identifiers.len() {
            for j in (i + 1)..identifiers.len() {
                let name_a = &names[i];
                let name_b = &names[j];
                let pair = if name_a <= name_b {
                    (name_a.clone(), name_b.clone())
                } else {
                    (name_b.clone(), name_a.clone())
                };
                if checked_pairs.contains(&pair) {
                    continue;
                }

                let confusables_a = find_confusables(name_a);
                let confusables_b = find_confusables(name_b);

                let mut is_confusable = false;

                if !confusables_a.is_empty() && !confusables_b.is_empty() {
                    let a_targets: std::collections::HashSet<String> = confusables_a
                        .iter()
                        .map(|(_, t)| (*t).to_string())
                        .collect();
                    let b_targets: std::collections::HashSet<String> = confusables_b
                        .iter()
                        .map(|(_, t)| (*t).to_string())
                        .collect();
                    if !a_targets.is_disjoint(&b_targets) {
                        is_confusable = true;
                    }
                }

                if !is_confusable {
                    for (_, target) in &confusables_a {
                        if name_b.contains(&target.to_string()) {
                            is_confusable = true;
                            break;
                        }
                    }
                }

                if !is_confusable {
                    for (_, target) in &confusables_b {
                        if name_a.contains(&target.to_string()) {
                            is_confusable = true;
                            break;
                        }
                    }
                }

                if !is_confusable {
                    let dist = levenshtein_distance(name_a, name_b);
                    let max_len = std::cmp::max(name_a.len(), name_b.len());
                    if max_len > 0 && dist <= 1 && name_a != name_b {
                        is_confusable = true;
                    }
                }

                if is_confusable {
                    checked_pairs.insert(pair.clone());
                    collisions.push(TableCollisionInfo {
                        kind: "confusable".to_string(),
                        names: vec![name_a.clone(), name_b.clone()],
                        detail: format!("Confusable/near-collision: '{}' and '{}'", name_a, name_b),
                    });
                }
            }
        }
        if !checked_pairs.is_empty() {
            findings.push("Confusable characters or near-collisions detected".to_string());
        }
    }

    if active_checks.contains(&"style") {
        let mut style_map: std::collections::HashMap<String, Vec<(String, String)>> =
            std::collections::HashMap::new();
        for name in &names {
            let stripped = strip_style(name);
            if stripped.is_empty() {
                continue;
            }
            let style = classify_style(name);
            style_map
                .entry(stripped)
                .or_default()
                .push((name.clone(), style));
        }
        for (stripped, entries) in style_map.iter().filter(|(_, entries)| {
            let styles: std::collections::HashSet<_> = entries.iter().map(|(_, s)| s).collect();
            styles.len() > 1
        }) {
            let group_names: Vec<String> = entries.iter().map(|(n, _)| n.clone()).collect();
            let styles_present: Vec<String> = entries
                .iter()
                .map(|(_, s)| s.clone())
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();
            collisions.push(TableCollisionInfo {
                kind: "style_variant".to_string(),
                names: group_names,
                detail: format!(
                    "Style variants for '{}': {}",
                    stripped,
                    styles_present.join(", ")
                ),
            });
        }
        if style_map.values().any(|entries| {
            let styles: std::collections::HashSet<_> = entries.iter().map(|(_, s)| s).collect();
            styles.len() > 1
        }) {
            findings.push("Style variant collisions detected".to_string());
        }
    }

    if active_checks.contains(&"reserved") {
        let kw_set: std::collections::HashSet<&str> =
            get_lang_keywords(language).into_iter().collect();
        for (i, entry) in identifiers.iter().enumerate() {
            if kw_set.contains(names[i].as_str()) {
                reserved_hits.push(ReservedKeywordHit {
                    name: names[i].clone(),
                    language: language.to_string(),
                    file: entry.file.clone(),
                    line: entry.line,
                });
            }
        }
        if !reserved_hits.is_empty() {
            findings.push(format!(
                "{} reserved keyword hit(s) in {}",
                reserved_hits.len(),
                language
            ));
        }
    }

    if active_checks.contains(&"mixed_style") {
        let mut style_map2: std::collections::HashMap<String, Vec<(String, String)>> =
            std::collections::HashMap::new();
        for name in &names {
            let stripped = strip_style(name);
            if stripped.is_empty() {
                continue;
            }
            let style = classify_style(name);
            style_map2
                .entry(stripped)
                .or_default()
                .push((name.clone(), style));
        }
        for (stripped, entries) in style_map2.iter().filter(|(_, entries)| {
            let styles: std::collections::HashSet<_> = entries.iter().map(|(_, s)| s).collect();
            styles.len() > 1
        }) {
            let group_names: Vec<String> = entries.iter().map(|(n, _)| n.clone()).collect();
            let styles_present: Vec<String> = entries
                .iter()
                .map(|(_, s)| s.clone())
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();
            mixed_style_groups.push(MixedStyleGroup {
                stripped: stripped.clone(),
                names: group_names,
                styles: styles_present,
            });
        }
        if !mixed_style_groups.is_empty() {
            findings.push(format!(
                "{} mixed-style group(s) detected",
                mixed_style_groups.len()
            ));
        }
    }

    IdentifierTableInspectResult {
        count,
        collisions,
        reserved_keyword_hits: reserved_hits,
        mixed_style_groups,
        findings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_snake_case() {
        assert_eq!(classify_style("hello_world"), "snake_case");
    }

    #[test]
    fn test_classify_camel_case() {
        assert_eq!(classify_style("helloWorld"), "camelCase");
    }

    #[test]
    fn test_classify_pascal_case() {
        assert_eq!(classify_style("HelloWorld"), "PascalCase");
    }

    #[test]
    fn test_classify_kebab_case() {
        assert_eq!(classify_style("hello-world"), "kebab-case");
    }

    #[test]
    fn test_classify_screaming_snake() {
        assert_eq!(classify_style("HELLO_WORLD"), "SCREAMING_SNAKE_CASE");
    }

    #[test]
    fn test_analyze_snake_case() {
        let result = identifier_analyze("hello_world", None);
        assert_eq!(result.classification, "snake_case");
        assert!(result.python_valid);
    }

    #[test]
    fn test_analyze_python_keyword() {
        let result = identifier_analyze("for", None);
        assert!(result.python_keyword);
        assert!(result.python_valid);
    }

    #[test]
    fn test_analyze_env_valid() {
        let result = identifier_analyze("MY_VAR", None);
        assert!(result.env_valid);
    }

    #[test]
    fn test_rust_identifier_rejects_unicode() {
        let result = identifier_analyze("café", Some(vec!["rust"]));
        assert_eq!(result.rust_valid, Some(false));
    }

    #[test]
    fn test_rust_identifier_accepts_underscore_start() {
        let result = identifier_analyze("_private", Some(vec!["rust"]));
        assert_eq!(result.rust_valid, Some(true));
    }

    #[test]
    fn test_js_identifier_accepts_dollar() {
        let result = identifier_analyze("$element", Some(vec!["javascript"]));
        assert_eq!(result.javascript_valid, Some(true));
    }

    #[test]
    fn test_js_identifier_accepts_dollar_middle() {
        let result = identifier_analyze("foo$bar", Some(vec!["javascript"]));
        assert_eq!(result.javascript_valid, Some(true));
    }

    #[test]
    fn test_python_identifier_accepts_unicode() {
        let result = identifier_analyze("café", Some(vec!["python"]));
        assert!(result.python_valid);
    }
}
