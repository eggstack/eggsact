use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const ZERO_WIDTH_CHARS: &[(char, &str)] = &[
    ('\u{200b}', "ZERO WIDTH SPACE"),
    ('\u{200c}', "ZERO WIDTH NON-JOINER"),
    ('\u{200d}', "ZERO WIDTH JOINER"),
    ('\u{2060}', "WORD JOINER"),
];

const BIDI_CONTROL_CHARS: &[(char, &str)] = &[
    ('\u{202a}', "LEFT-TO-RIGHT EMBEDDING"),
    ('\u{202b}', "RIGHT-TO-LEFT EMBEDDING"),
    ('\u{202c}', "POP DIRECTIONAL FORMATTING"),
    ('\u{202d}', "LEFT-TO-RIGHT OVERRIDE"),
    ('\u{202e}', "RIGHT-TO-LEFT OVERRIDE"),
    ('\u{2066}', "LEFT-TO-RIGHT ISOLATE"),
    ('\u{2067}', "RIGHT-TO-LEFT ISOLATE"),
    ('\u{2068}', "FIRST STRONG ISOLATE"),
    ('\u{2069}', "POP DIRECTIONAL ISOLATE"),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemovedChar {
    pub index: usize,
    #[serde(rename = "char")]
    pub ch: char,
    pub codepoint: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextTransformResult {
    pub changed: bool,
    pub text: String,
    pub operations_applied: Vec<String>,
    pub removed: Vec<RemovedChar>,
    pub warnings: Vec<String>,
    pub summary: String,
}

pub fn text_transform(text: &str, operations: &[String]) -> TextTransformResult {
    if operations.is_empty() {
        return TextTransformResult {
            changed: false,
            text: text.to_string(),
            operations_applied: vec![],
            removed: vec![],
            warnings: vec![],
            summary: "No operations requested".to_string(),
        };
    }

    let mut current_text = text.to_string();
    let mut ops_applied: Vec<String> = vec![];
    let mut all_removed: Vec<RemovedChar> = vec![];
    let mut all_warnings: Vec<String> = vec![];

    for op in operations {
        let op_lower = op.to_lowercase();
        match op_lower.as_str() {
            "normalize_nfc" => {
                let normalized = unescape_chars::normalize_nfc(&current_text);
                if normalized != current_text {
                    current_text = normalized;
                    ops_applied.push("normalize_nfc".to_string());
                }
            }
            "normalize_nfd" => {
                let normalized = unescape_chars::normalize_nfd(&current_text);
                if normalized != current_text {
                    current_text = normalized;
                    ops_applied.push("normalize_nfd".to_string());
                }
            }
            "normalize_nfkc" => {
                let normalized = unescape_chars::normalize_nfkc(&current_text);
                if normalized != current_text {
                    current_text = normalized;
                    ops_applied.push("normalize_nfkc".to_string());
                }
            }
            "normalize_nfkd" => {
                let normalized = unescape_chars::normalize_nfkd(&current_text);
                if normalized != current_text {
                    current_text = normalized;
                    ops_applied.push("normalize_nfkd".to_string());
                }
            }
            "casefold" => {
                let casefolded = crate::text::unicode_tools::unicode_casefold(&current_text);
                if casefolded != current_text {
                    current_text = casefolded;
                    ops_applied.push("casefold".to_string());
                }
            }
            "trim" => {
                let trimmed = current_text.trim();
                if trimmed != current_text {
                    current_text = trimmed.to_string();
                    ops_applied.push("trim".to_string());
                }
            }
            "trim_trailing_whitespace" => {
                let lines: Vec<&str> = current_text.split('\n').collect();
                let trimmed_lines: Vec<String> =
                    lines.iter().map(|l| l.trim_end().to_string()).collect();
                let new_text = trimmed_lines.join("\n");
                if new_text != current_text {
                    current_text = new_text;
                    ops_applied.push("trim_trailing_whitespace".to_string());
                }
            }
            "normalize_newlines_lf" | "normalize_newlines" => {
                let normalized = current_text.replace("\r\n", "\n").replace("\r", "\n");
                if normalized != current_text {
                    current_text = normalized;
                    ops_applied.push(op_lower.clone());
                }
            }
            "ensure_final_newline" => {
                if !current_text.ends_with('\n') {
                    current_text.push('\n');
                    ops_applied.push("ensure_final_newline".to_string());
                }
            }
            "strip_final_newline" => {
                if current_text.ends_with('\n') {
                    current_text.pop();
                    ops_applied.push("strip_final_newline".to_string());
                }
            }
            "remove_zero_width" => {
                let (result_text, removed, warnings) =
                    remove_chars(&current_text, ZERO_WIDTH_CHARS, "zero-width");
                if result_text != current_text {
                    current_text = result_text;
                    all_removed.extend(removed);
                    all_warnings.extend(warnings);
                    ops_applied.push("remove_zero_width".to_string());
                }
            }
            "remove_bidi_controls" | "remove_bidi" => {
                let (result_text, removed, warnings) =
                    remove_chars(&current_text, BIDI_CONTROL_CHARS, "bidi");
                if result_text != current_text {
                    current_text = result_text;
                    all_removed.extend(removed);
                    all_warnings.extend(warnings);
                    ops_applied.push(op_lower.clone());
                }
            }
            "visible_repr" => {
                current_text = visible_repr(&current_text);
                ops_applied.push("visible_repr".to_string());
            }
            "trim_lines" => {
                let lines: Vec<&str> = current_text.lines().collect();
                let trimmed_lines: Vec<String> =
                    lines.iter().map(|l| l.trim().to_string()).collect();
                let new_text = trimmed_lines.join("\n");
                if new_text != current_text {
                    current_text = new_text;
                    ops_applied.push("trim_lines".to_string());
                }
            }
            "title_case" => {
                let new_text: String = current_text
                    .split_whitespace()
                    .map(|word| {
                        let mut chars = word.chars();
                        match chars.next() {
                            Some(first) => {
                                let upper: String = first.to_uppercase().collect();
                                let rest: String = chars.collect::<String>().to_lowercase();
                                format!("{}{}", upper, rest)
                            }
                            None => String::new(),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                if new_text != current_text {
                    current_text = new_text;
                    ops_applied.push("title_case".to_string());
                }
            }
            "upper" => {
                let new_text = current_text.to_uppercase();
                if new_text != current_text {
                    current_text = new_text;
                    ops_applied.push("upper".to_string());
                }
            }
            "lower" => {
                let new_text = current_text.to_lowercase();
                if new_text != current_text {
                    current_text = new_text;
                    ops_applied.push("lower".to_string());
                }
            }
            _ => {}
        }
    }

    let changed = current_text != text;

    let ops_str = ops_applied.join(", ");
    let summary = if !ops_applied.is_empty() {
        if changed {
            format!(
                "Applied {} operation(s): {}; text changed",
                ops_applied.len(),
                ops_str
            )
        } else {
            format!(
                "Applied {} operation(s): {}; text unchanged",
                ops_applied.len(),
                ops_str
            )
        }
    } else {
        "No recognized operations applied".to_string()
    };

    TextTransformResult {
        changed,
        text: current_text,
        operations_applied: ops_applied,
        removed: all_removed,
        warnings: all_warnings,
        summary,
    }
}

fn remove_chars(
    text: &str,
    chars_to_remove: &[(char, &str)],
    operation_name: &str,
) -> (String, Vec<RemovedChar>, Vec<String>) {
    let mut result = String::new();
    let mut removed: Vec<RemovedChar> = vec![];
    let char_map: HashMap<char, &str> = chars_to_remove
        .iter()
        .map(|(c, name)| (*c, *name))
        .collect();

    for (index, ch) in text.char_indices() {
        if let Some(name) = char_map.get(&ch) {
            removed.push(RemovedChar {
                index,
                ch,
                codepoint: format!("U+{:04X}", ch as u32),
                name: name.to_string(),
            });
        } else {
            result.push(ch);
        }
    }

    let mut warnings: Vec<String> = vec![];
    if !removed.is_empty() {
        let count = removed.len();
        let names: std::collections::HashSet<_> = removed.iter().map(|r| r.name.clone()).collect();
        let names_str = names.into_iter().collect::<Vec<_>>().join(", ");
        warnings.push(format!(
            "Removed {} invisible/{} character(s): {}",
            count, operation_name, names_str
        ));
    }

    (result, removed, warnings)
}

fn visible_repr(text: &str) -> String {
    let mut result = String::new();
    for ch in text.chars() {
        match ch {
            ' ' => result.push('\u{2420}'),  // ␠ SPACE SYMBOL
            '\t' => result.push('\u{2409}'), // ␉ SYMBOL FOR HORIZONTAL TABULATION
            '\n' => result.push('\u{240A}'), // ␊ SYMBOL FOR LINE FEED
            '\r' => result.push('\u{240D}'), // ␍ SYMBOL FOR CARRIAGE RETURN
            '\u{200b}' => result.push_str("\u{27E6}ZWSP\u{27E7}"),
            '\u{200c}' => result.push_str("\u{27E6}ZWNJ\u{27E7}"),
            '\u{200d}' => result.push_str("\u{27E6}ZWJ\u{27E7}"),
            '\u{2060}' => result.push_str("\u{27E6}WJ\u{27E7}"),
            '\u{fe00}'..='\u{fe0f}' => result.push_str("\u{27E6}VS\u{27E7}"),
            c if (c as u32) < 32 || c as u32 == 0x7f || c == '\\' => {
                result.push_str(&format!("\u{27E6}U+{:04X}\u{27E7}", c as u32));
            }
            c => result.push(c),
        }
    }
    result
}

mod unescape_chars {
    use unicode_normalization::UnicodeNormalization;

    pub fn normalize_nfc(s: &str) -> String {
        s.nfc().collect()
    }

    pub fn normalize_nfd(s: &str) -> String {
        s.nfd().collect()
    }

    pub fn normalize_nfkc(s: &str) -> String {
        s.nfkc().collect()
    }

    pub fn normalize_nfkd(s: &str) -> String {
        s.nfkd().collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscapeTextResult {
    pub mode: String,
    pub escaped: String,
    pub changed: bool,
    pub summary: String,
}

pub fn escape_text(text: &str, mode: &str) -> Result<EscapeTextResult, String> {
    let original_text = text.to_string();

    let escaped = match mode {
        "json_string" => serde_json::to_string(text).map_err(|e| e.to_string())?,
        "python_string" => {
            let mut result = String::from("'");
            for ch in text.chars() {
                match ch {
                    '\\' => result.push_str("\\\\"),
                    '\'' => result.push_str("\\'"),
                    '\n' => result.push_str("\\n"),
                    '\r' => result.push_str("\\r"),
                    '\t' => result.push_str("\\t"),
                    '\0' => result.push_str("\\x00"),
                    c if (c as u32) < 32 => {
                        result.push_str(&format!("\\x{:02x}", c as u32));
                    }
                    c => result.push(c),
                }
            }
            result.push('\'');
            result
        }
        "rust_string" => {
            let mut result = String::from("\"");
            for ch in text.chars() {
                match ch {
                    '\\' => result.push_str("\\\\"),
                    '"' => result.push_str("\\\""),
                    '\n' => result.push_str("\\n"),
                    '\r' => result.push_str("\\r"),
                    '\t' => result.push_str("\\t"),
                    c => result.push(c),
                }
            }
            result.push('"');
            result
        }
        "posix_shell_single" => {
            let escaped = text.replace("'", "'\\''");
            format!("'{}'", escaped)
        }
        "regex_literal" => regex_literal_escape(text),
        "markdown_inline_code" => {
            if text.contains('`') {
                format!("`` {} ``", text)
            } else {
                format!("`{}`", text)
            }
        }
        "markdown_code_block" => {
            format!("```\n{}\n```", text)
        }
        "html_text" => {
            let mut result = String::new();
            for ch in text.chars() {
                match ch {
                    '&' => result.push_str("&amp;"),
                    '<' => result.push_str("&lt;"),
                    '>' => result.push_str("&gt;"),
                    '"' => result.push_str("&quot;"),
                    '\'' => result.push_str("&#39;"),
                    c => result.push(c),
                }
            }
            result
        }
        "url_component" => urlencoding::encode(text).to_string(),
        _ => return Err(format!("Unsupported escape mode: {}", mode)),
    };

    let changed = escaped != original_text;

    let mode_names: HashMap<&str, &str> = [
        ("json_string", "JSON string literal"),
        ("python_string", "Python string literal"),
        ("rust_string", "Rust string literal"),
        ("posix_shell_single", "POSIX shell single-quoted string"),
        ("regex_literal", "regex literal"),
        ("markdown_inline_code", "inline markdown code"),
        ("markdown_code_block", "markdown code block"),
        ("html_text", "HTML text"),
        ("url_component", "URL component"),
    ]
    .iter()
    .cloned()
    .collect();

    let summary = format!(
        "Escaped text as {}",
        mode_names.get(mode).copied().unwrap_or(mode)
    );

    Ok(EscapeTextResult {
        mode: mode.to_string(),
        escaped,
        changed,
        summary,
    })
}

fn regex_literal_escape(text: &str) -> String {
    let mut result = String::new();
    for ch in text.chars() {
        match ch {
            '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|' => {
                result.push('\\');
                result.push(ch)
            }
            c => result.push(c),
        }
    }
    result
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnescapeTextResult {
    pub mode: String,
    pub unescaped: String,
    pub changed: bool,
    pub error: Option<String>,
    pub summary: String,
}

pub fn unescape_text(text: &str, mode: &str) -> UnescapeTextResult {
    let original_text = text.to_string();
    let mut error: Option<String> = None;

    let unescaped = match mode {
        "json_string" => {
            if !text.starts_with('"') || !text.ends_with('"') {
                error = Some(
                    "Invalid JSON string literal: must be wrapped in double quotes".to_string(),
                );
                text.to_string()
            } else {
                match serde_json::from_str::<String>(text) {
                    Ok(s) => s,
                    Err(e) => {
                        error = Some(format!("Invalid JSON string literal: {}", e));
                        text.to_string()
                    }
                }
            }
        }
        "python_string" => {
            if (!text.starts_with('\'') || !text.ends_with('\''))
                && (!text.starts_with('"') || !text.ends_with('"'))
            {
                error = Some(
                    "Invalid Python string literal: must be wrapped in single or double quotes"
                        .to_string(),
                );
                text.to_string()
            } else {
                let inner = &text[1..text.len() - 1];
                let simple = inner
                    .replace("\\'", "'")
                    .replace("\\\"", "\"")
                    .replace("\\n", "\n")
                    .replace("\\t", "\t")
                    .replace("\\r", "\r")
                    .replace("\\0", "\0")
                    .replace("\\a", "\x07")
                    .replace("\\b", "\x08")
                    .replace("\\f", "\x0C")
                    .replace("\\v", "\x0B")
                    .replace("\\\\", "\\");
                let mut result = String::new();
                let mut chars = simple.chars();
                while let Some(c) = chars.next() {
                    if c == '\\' {
                        if let Some(next) = chars.next() {
                            match next {
                                'x' => {
                                    let hex: String = chars.by_ref().take(2).collect();
                                    if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                                        result.push(byte as char);
                                    } else {
                                        result.push('\\');
                                        result.push('x');
                                        result.push_str(&hex);
                                    }
                                }
                                'u' => {
                                    let hex: String = chars.by_ref().take(4).collect();
                                    if let Ok(cp) = u32::from_str_radix(&hex, 16) {
                                        if let Some(c) = char::from_u32(cp) {
                                            result.push(c);
                                        } else {
                                            result.push('\\');
                                            result.push('u');
                                            result.push_str(&hex);
                                        }
                                    } else {
                                        result.push('\\');
                                        result.push('u');
                                        result.push_str(&hex);
                                    }
                                }
                                'U' => {
                                    let hex: String = chars.by_ref().take(8).collect();
                                    if let Ok(cp) = u32::from_str_radix(&hex, 16) {
                                        if let Some(c) = char::from_u32(cp) {
                                            result.push(c);
                                        } else {
                                            result.push('\\');
                                            result.push('U');
                                            result.push_str(&hex);
                                        }
                                    } else {
                                        result.push('\\');
                                        result.push('U');
                                        result.push_str(&hex);
                                    }
                                }
                                _ => {
                                    result.push('\\');
                                    result.push(next);
                                }
                            }
                        } else {
                            result.push('\\');
                        }
                    } else {
                        result.push(c);
                    }
                }
                result
            }
        }
        "unicode_escape" => {
            let re1 = regex::Regex::new(r"\\u([0-9a-fA-F]{4})").unwrap();
            let re2 = regex::Regex::new(r"\\U([0-9a-fA-F]{8})").unwrap();
            let mut result = text.to_string();
            result = re1
                .replace_all(&result, |caps: &regex::Captures| {
                    let code = u32::from_str_radix(&caps[1], 16).unwrap_or(0);
                    char::from_u32(code).unwrap_or('\u{FFFD}').to_string()
                })
                .to_string();
            result = re2
                .replace_all(&result, |caps: &regex::Captures| {
                    let code = u32::from_str_radix(&caps[1], 16).unwrap_or(0);
                    char::from_u32(code).unwrap_or('\u{FFFD}').to_string()
                })
                .to_string();
            result
        }
        "url_component" => match urlencoding::decode(text) {
            Ok(s) => s.to_string(),
            Err(e) => {
                error = Some(format!("Invalid URL component: {}", e));
                text.to_string()
            }
        },
        _ => {
            error = Some(format!("Unsupported unescape mode: {}", mode));
            text.to_string()
        }
    };

    let changed = unescaped != original_text;

    let mode_names: HashMap<&str, &str> = [
        ("json_string", "JSON string literal"),
        ("python_string", "Python string literal"),
        ("unicode_escape", "Unicode escape sequences"),
        ("url_component", "URL component"),
    ]
    .iter()
    .cloned()
    .collect();

    let summary = if let Some(ref err) = error {
        format!(
            "Failed to unescape {}: {}",
            mode_names.get(mode).copied().unwrap_or(mode),
            err
        )
    } else {
        format!(
            "Unescaped {}",
            mode_names.get(mode).copied().unwrap_or(mode)
        )
    };

    UnescapeTextResult {
        mode: mode.to_string(),
        unescaped,
        changed,
        error,
        summary,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextHashResult {
    pub encoding: String,
    pub bytes: usize,
    pub codepoints: usize,
    pub hashes: HashMap<String, String>,
    pub warnings: Vec<String>,
    pub summary: String,
}

pub fn text_hash(text: &str, algorithms: &[String], encoding: &str) -> TextHashResult {
    let mut warnings: Vec<String> = vec![];

    let codepoint_count = text.chars().count();

    // Encode to the requested encoding
    let (encoded, byte_count) = match encoding.to_lowercase().as_str() {
        "utf-8" | "" | "utf8" => {
            let bytes = text.as_bytes().to_vec();
            let len = bytes.len();
            (bytes, len)
        }
        "utf-16" | "utf16" => {
            let bytes: Vec<u8> = text.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
            let len = bytes.len();
            (bytes, len)
        }
        "utf-16be" | "utf-16-be" | "utf16be" => {
            let bytes: Vec<u8> = text.encode_utf16().flat_map(|u| u.to_be_bytes()).collect();
            let len = bytes.len();
            (bytes, len)
        }
        "utf-16le" | "utf-16-le" | "utf16le" => {
            let bytes: Vec<u8> = text.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
            let len = bytes.len();
            (bytes, len)
        }
        "ascii" => {
            let bytes: Vec<u8> = text
                .bytes()
                .map(|b| if b > 127 { b'?' } else { b })
                .collect();
            let len = bytes.len();
            (bytes, len)
        }
        "latin-1" | "latin1" | "iso-8859-1" => {
            let bytes: Vec<u8> = text
                .chars()
                .map(|c| {
                    let cp = c as u32;
                    if cp <= 0xFF {
                        cp as u8
                    } else {
                        b'?'
                    }
                })
                .collect();
            let len = bytes.len();
            (bytes, len)
        }
        _ => {
            warnings.push(format!(
                "Unknown encoding '{}', falling back to UTF-8",
                encoding
            ));
            let bytes = text.as_bytes().to_vec();
            let len = bytes.len();
            (bytes, len)
        }
    };

    let mut hashes: HashMap<String, String> = HashMap::new();

    for algo in algorithms {
        match algo.to_lowercase().as_str() {
            "sha256" => {
                use sha2::{Digest, Sha256};
                let mut hasher = Sha256::new();
                hasher.update(&encoded);
                hashes.insert("sha256".to_string(), format!("{:x}", hasher.finalize()));
            }
            "sha1" => {
                use sha1::{Digest, Sha1};
                let mut hasher = Sha1::new();
                hasher.update(&encoded);
                hashes.insert("sha1".to_string(), format!("{:x}", hasher.finalize()));
            }
            "md5" => {
                let digest = md5::compute(&encoded);
                hashes.insert("md5".to_string(), format!("{:x}", digest));
                if !warnings
                    .iter()
                    .any(|w| w == "MD5 is non-cryptographic and provided for compatibility only")
                {
                    warnings.push(
                        "MD5 is non-cryptographic and provided for compatibility only".to_string(),
                    );
                }
            }
            "crc32" => {
                let crc = crc32fast::hash(&encoded);
                hashes.insert("crc32".to_string(), format!("{:08x}", crc));
            }
            _ => {
                let supported = "crc32, md5, sha1, sha256";
                warnings.push(format!(
                    "Unknown algorithm '{}', skipping (supported: {})",
                    algo, supported
                ));
            }
        }
    }

    let algo_count = hashes.len();
    let summary = if algo_count == 1 {
        let algo_name = hashes.keys().next().unwrap().to_uppercase();
        format!(
            "{} computed for {} {} bytes",
            algo_name, byte_count, encoding
        )
    } else {
        format!(
            "Computed {} hashes for {} {} bytes",
            algo_count, byte_count, encoding
        )
    };

    TextHashResult {
        encoding: encoding.to_string(),
        bytes: byte_count,
        codepoints: codepoint_count,
        hashes,
        warnings,
        summary,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextFingerprintResult {
    pub sha256: String,
    pub bytes_utf8: usize,
    pub codepoints: usize,
    pub graphemes: usize,
    pub newline_style: String,
    pub normalization: HashMap<String, serde_json::Value>,
    pub summary: String,
}

pub fn text_fingerprint(
    text: &str,
    unicode_norm: &str,
    newline_style: &str,
    trim_final_newline: bool,
    casefold: bool,
) -> TextFingerprintResult {
    let mut canonical = text.to_string();

    if unicode_norm != "raw" {
        canonical = match unicode_norm {
            "NFC" => unescape_chars::normalize_nfc(&canonical),
            "NFD" => unescape_chars::normalize_nfd(&canonical),
            "NFKC" => unescape_chars::normalize_nfkc(&canonical),
            "NFKD" => unescape_chars::normalize_nfkd(&canonical),
            _ => canonical,
        };
    }

    if newline_style == "LF" {
        canonical = canonical.replace("\r\n", "\n").replace("\r", "\n");
    }

    if trim_final_newline && canonical.ends_with('\n') {
        canonical.pop();
    }

    if casefold {
        canonical = caseless::default_case_fold_str(&canonical).to_string();
    }

    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    let sha256_hash = format!("{:x}", hasher.finalize());

    let is_nfc = unescape_chars::normalize_nfc(text) == text;

    let nl_style = {
        let crlf_count = text.matches("\r\n").count();
        let lf_only = text.matches('\n').count() - crlf_count;
        let cr_only = text.matches('\r').count() - crlf_count;
        if crlf_count > 0 && (lf_only > 0 || cr_only > 0) {
            "mixed"
        } else if crlf_count > 0 {
            "CRLF"
        } else if cr_only > 0 {
            "CR"
        } else if lf_only > 0 {
            "LF"
        } else {
            "none"
        }
    };

    let mut norm_map: HashMap<String, serde_json::Value> = HashMap::new();
    norm_map.insert("input_is_nfc".to_string(), serde_json::json!(is_nfc));
    norm_map.insert("applied".to_string(), serde_json::json!(unicode_norm));

    let summary = format!(
        "SHA-256 fingerprint computed for {} codepoints",
        canonical.chars().count()
    );

    TextFingerprintResult {
        sha256: sha256_hash,
        bytes_utf8: canonical.len(),
        codepoints: canonical.chars().count(),
        graphemes: crate::text::count_graphemes(&canonical),
        newline_style: nl_style.to_string(),
        normalization: norm_map,
        summary,
    }
}
