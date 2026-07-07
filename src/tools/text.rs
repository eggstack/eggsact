use crate::mcp::machine_codes;
use crate::mcp::response::{disposition, finding, severity, verdict, ToolResponse};
use crate::text::measure::{char_category_metrics, word_metrics};
use crate::text::position::{TextPositionResult, TextWindowPosition, TextWindowResult};
use crate::text::primitives::byte_offset_to_char_index;
use crate::text::transform::{
    TextFingerprintResult, TextHashResult, TextTransformResult, UnescapeTextResult,
};
use crate::text::unicode_tools::{
    detect_mixed_scripts as unicode_detect_mixed_scripts, detect_newline_style,
    find_invisibles as unicode_find_invisibles,
};
use crate::text::{count_graphemes, text_fingerprint};
use crate::tools::helpers::*;
use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;
use unicode_normalization::UnicodeNormalization;

// ---------------------------------------------------------------------------
// prompt_input_inspect static regex patterns
// ---------------------------------------------------------------------------

static MARKDOWN_LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[([^\]]{1,2000})\]\(([^)]{1,2000})\)").unwrap());

static HTML_COMMENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<!--(.*?)-->").unwrap());

static ANSI_ESCAPE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\x1b\[[0-9;]*[A-Za-z]").unwrap());

static TERMINAL_CONTROL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[\x00-\x08\x0e-\x1f\x7f]|\x1b[()][AB012]|\x1b[=>78]").unwrap());

static DEFAULT_INSTRUCTION_RE: LazyLock<Regex> = LazyLock::new(|| {
    let escaped: Vec<String> = DEFAULT_INSTRUCTION_PHRASES
        .iter()
        .map(|s| regex::escape(s))
        .collect();
    let combined = escaped.join("|");
    Regex::new(&format!("(?i){}", combined)).unwrap()
});

static DEFAULT_INSTRUCTION_PHRASES: &[&str] = &[
    "ignore previous",
    "ignore all previous",
    "disregard previous",
    "disregard all previous",
    "forget everything",
    "new instructions",
    "override instructions",
    "system prompt",
    "you are now",
    "act as",
    "pretend you are",
    "roleplay as",
    "do not follow",
    "ignore the above",
    "ignore the following",
    "disregard the above",
    "disregard the following",
    "override safety",
    "bypass safety",
    "jailbreak",
    "do anything now",
    " DAN",
];

const MAX_FINDINGS: usize = 1000;

// ---------------------------------------------------------------------------
// prompt_input_inspect helpers
// ---------------------------------------------------------------------------

fn _pi_char_span(index: usize, length: usize) -> serde_json::Value {
    serde_json::json!({"char_start": index, "char_end": index + length})
}

fn _pi_hidden_char_display(c: char) -> &'static str {
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

fn _pi_hidden_char_category(c: char) -> &'static str {
    let cp = c as u32;
    match cp {
        0x00..=0x1F | 0x7F | 0x80..=0x9F => "Cc",
        0x034F => "Mn",
        0x200B..=0x200F | 0x2060..=0x2069 | 0xFEFF => "Cf",
        0x2028 => "Zl",
        0x2029 => "Zp",
        0xFE00..=0xFE0F => "Mn",
        0xFFF0..=0xFFFD => "Cn",
        _ => "Cf",
    }
}

fn _pi_find_unicode_hidden(text: &str) -> Vec<serde_json::Value> {
    let mut findings = Vec::new();
    for (i, c) in text.chars().enumerate() {
        let cp = c as u32;
        let (found, name, severity): (bool, String, &str) = match cp {
            // C0 controls except TAB(09), LF(0A), CR(0D)
            0x00..=0x08 | 0x0B..=0x0C | 0x0E..=0x1F => (true, "CONTROL".to_string(), "warn"),
            0x7F => (true, "CONTROL".to_string(), "warn"),
            0x80..=0x9F => (true, "CONTROL".to_string(), "warn"),
            // Zero-width characters — high severity
            0x200B => (true, "ZERO WIDTH SPACE".to_string(), "error"),
            0x200C => (true, "ZERO WIDTH NON-JOINER".to_string(), "error"),
            0x200D => (true, "ZERO WIDTH JOINER".to_string(), "error"),
            0x200E => (true, "LEFT-TO-RIGHT MARK".to_string(), "warn"),
            0x200F => (true, "RIGHT-TO-LEFT MARK".to_string(), "warn"),
            0x2028 => (true, "LINE SEPARATOR".to_string(), "warn"),
            0x2029 => (true, "PARAGRAPH SEPARATOR".to_string(), "warn"),
            0x2060 => (true, "WORD JOINER".to_string(), "error"),
            0x202A..=0x202E | 0x2066..=0x2069 => (
                true,
                unicode_names2::name(c)
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "BIDI CONTROL".to_string()),
                "warn",
            ),
            0x2061..=0x2065 => (true, "INVISIBLE FORMAT".to_string(), "warn"),
            // Variation selectors
            0xFE00..=0xFE0F => (true, "VARIATION SELECTOR".to_string(), "warn"),
            // Specials
            0xFFF0..=0xFFFD => (true, "SPECIALS".to_string(), "warn"),
            0xFEFF => (true, "BOM/ZWNBSP".to_string(), "warn"),
            _ => (false, String::new(), "info"),
        };
        if found {
            let display = _pi_hidden_char_display(c);
            let category = _pi_hidden_char_category(c);
            findings.push(serde_json::json!({
                "code": "HIDDEN_CHAR",
                "severity": severity,
                "message": format!("Hidden character: {} (U+{:04X}) at position {}", name, cp, i),
                "span": _pi_char_span(i, 1),
                "details": {
                    "codepoint": format!("U+{:04X}", cp),
                    "name": name,
                    "category": category,
                    "display": display,
                },
            }));
        }
    }
    findings
}

fn _pi_find_bidi(text: &str) -> Vec<serde_json::Value> {
    let bidi_names: &[(u32, &str)] = &[
        (0x202A, "LEFT-TO-RIGHT EMBEDDING (LRE)"),
        (0x202B, "RIGHT-TO-LEFT EMBEDDING (RLE)"),
        (0x202C, "POP DIRECTIONAL FORMATTING (PDF)"),
        (0x202D, "LEFT-TO-RIGHT OVERRIDE (LRO)"),
        (0x202E, "RIGHT-TO-LEFT OVERRIDE (RLO)"),
        (0x2066, "LEFT-TO-RIGHT ISOLATE (LRI)"),
        (0x2067, "RIGHT-TO-LEFT ISOLATE (RLI)"),
        (0x2068, "FIRST STRONG ISOLATE (FSI)"),
        (0x2069, "POP DIRECTIONAL ISOLATE (PDI)"),
        (0x200E, "LEFT-TO-RIGHT MARK (LRM)"),
        (0x200F, "RIGHT-TO-LEFT MARK (RLM)"),
    ];
    let mut findings = Vec::new();
    for (i, c) in text.chars().enumerate() {
        let cp = c as u32;
        if let Some(&(_, name)) = bidi_names.iter().find(|&&(cp_id, _)| cp_id == cp) {
            findings.push(serde_json::json!({
                "code": "BIDI_CONTROL",
                "severity": "warn",
                "message": format!("Bidi control character: {} at position {}", name, i),
                "span": _pi_char_span(i, 1),
                "details": {
                    "codepoint": format!("U+{:04X}", cp),
                    "name": name,
                },
            }));
        }
    }
    findings
}

fn _pi_find_html_comments(text: &str) -> Vec<serde_json::Value> {
    let mut findings = Vec::new();
    for m in HTML_COMMENT_RE.captures_iter(text) {
        let full_match = m.get(0).unwrap();
        let char_start =
            byte_offset_to_char_index(text, full_match.start()).unwrap_or(text.chars().count());
        let char_end =
            byte_offset_to_char_index(text, full_match.end()).unwrap_or(text.chars().count());
        let content = m
            .get(1)
            .map(|c| c.as_str().trim().to_string())
            .unwrap_or_default();
        let severity = if content.is_empty() { "info" } else { "warn" };
        let truncated = if content.chars().count() > 100 {
            let preview: String = content.chars().take(100).collect();
            format!("{}...", preview)
        } else {
            content.clone()
        };
        let message = if content.is_empty() {
            format!("HTML comment at position {}", char_start)
        } else {
            format!("HTML comment at position {}: {}", char_start, truncated)
        };
        let content_preview: String = content.chars().take(500).collect();
        findings.push(serde_json::json!({
            "code": "HTML_COMMENT",
            "severity": severity,
            "message": message,
            "span": {"char_start": char_start, "char_end": char_end},
            "details": {"content": content_preview},
        }));
    }
    findings
}

fn _pi_find_markdown_links(text: &str) -> Vec<serde_json::Value> {
    let mut findings = Vec::new();
    for m in MARKDOWN_LINK_RE.captures_iter(text) {
        let full_match = m.get(0).unwrap();
        let char_start =
            byte_offset_to_char_index(text, full_match.start()).unwrap_or(text.chars().count());
        let char_end =
            byte_offset_to_char_index(text, full_match.end()).unwrap_or(text.chars().count());
        let link_text = m.get(1).map(|c| c.as_str()).unwrap_or("");
        let link_target = m.get(2).map(|c| c.as_str()).unwrap_or("");
        let mut severity = "info";
        let mut details = serde_json::json!({
            "text": link_text,
            "target": link_target,
        });

        if (link_target.starts_with("http://")
            || link_target.starts_with("https://")
            || link_target.starts_with("ftp://"))
            && (link_text.contains("http://") || link_text.contains("https://"))
        {
            severity = "warn";
            details["mismatch"] = serde_json::json!("text contains URL while target is also a URL");
        }
        if link_target.starts_with("data:") {
            severity = "warn";
            details["mismatch"] = serde_json::json!("data URI target");
        }

        let display_text: String = if link_text.chars().count() > 50 {
            link_text.chars().take(50).collect()
        } else {
            link_text.to_string()
        };
        let display_target: String = if link_target.chars().count() > 80 {
            link_target.chars().take(80).collect()
        } else {
            link_target.to_string()
        };
        findings.push(serde_json::json!({
            "code": "MARKDOWN_LINK",
            "severity": severity,
            "message": format!(
                "Markdown link at position {}: [{}]({})",
                char_start,
                display_text,
                display_target
            ),
            "span": {"char_start": char_start, "char_end": char_end},
            "details": details,
        }));
    }
    findings
}

fn _pi_python_repr(s: &str) -> String {
    let mut out = String::from("'");
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\x1b' => out.push_str("\\x1b"),
            c if c.is_control() => out.push_str(&format!("\\x{:02x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('\'');
    out
}

fn _pi_find_ansi_escapes(text: &str) -> Vec<serde_json::Value> {
    let mut findings = Vec::new();
    for m in ANSI_ESCAPE_RE.find_iter(text) {
        let char_start = byte_offset_to_char_index(text, m.start()).unwrap_or(text.chars().count());
        let char_end = byte_offset_to_char_index(text, m.end()).unwrap_or(text.chars().count());
        findings.push(serde_json::json!({
            "code": "ANSI_ESCAPE",
            "severity": "warn",
            "message": format!("ANSI escape sequence at position {}", char_start),
            "span": {"char_start": char_start, "char_end": char_end},
            "details": {"sequence": _pi_python_repr(m.as_str())},
        }));
    }
    findings
}

fn _pi_find_terminal_controls(text: &str) -> Vec<serde_json::Value> {
    let mut findings = Vec::new();
    for m in TERMINAL_CONTROL_RE.find_iter(text) {
        let char_start = byte_offset_to_char_index(text, m.start()).unwrap_or(text.chars().count());
        let char_end = byte_offset_to_char_index(text, m.end()).unwrap_or(text.chars().count());
        let first_char = m.as_str().chars().next().unwrap();
        let cp = format!("U+{:04X}", first_char as u32);
        let name = "CONTROL".to_string();
        findings.push(serde_json::json!({
            "code": "TERMINAL_CONTROL",
            "severity": "info",
            "message": format!(
                "Terminal control character {} ({}) at position {}",
                name,
                cp,
                char_start
            ),
            "span": {"char_start": char_start, "char_end": char_end},
            "details": {"codepoint": cp, "name": name},
        }));
    }
    findings
}

fn _pi_find_base64_like_blobs(text: &str) -> Vec<serde_json::Value> {
    let mut findings = Vec::new();
    let re = match Regex::new(r"(?:[A-Za-z0-9+/]{4}){16,}(?:[A-Za-z0-9+/]{0,3})?(?:=){0,2}") {
        Ok(r) => r,
        Err(_) => return findings,
    };
    for m in re.find_iter(text) {
        let s = m.as_str();
        if s.chars().count() < 64 {
            continue;
        }
        let has_upper = s.chars().any(|c| c.is_uppercase());
        let has_lower = s.chars().any(|c| c.is_lowercase());
        let has_digit = s.chars().any(|c| c.is_ascii_digit());
        if has_upper && has_lower && has_digit {
            let preview: String = s.chars().take(100).collect();
            findings.push(serde_json::json!({
                "code": "BASE64_BLOB",
                "severity": "warn",
                "message": format!("Base64-like blob ({} chars) at position {}", s.chars().count(), byte_offset_to_char_index(text, m.start()).unwrap_or(text.chars().count())),
                "span": {"char_start": byte_offset_to_char_index(text, m.start()).unwrap_or(text.chars().count()), "char_end": byte_offset_to_char_index(text, m.end()).unwrap_or(text.chars().count())},
                "details": {"length": s.chars().count(), "preview": preview},
            }));
        }
    }
    findings
}

fn _pi_find_long_minified_lines(text: &str) -> Vec<serde_json::Value> {
    let mut findings = Vec::new();
    let mut offset = 0usize;
    for line in text.split(['\n', '\r']) {
        let line_len = line.chars().count();
        if line_len > 1000 {
            findings.push(serde_json::json!({
                "code": "LONG_LINE",
                "severity": "info",
                "message": format!("Very long line ({} chars) at position {}", line_len, offset),
                "span": {"char_start": offset, "char_end": offset + line_len},
                "details": {"length": line_len},
            }));
        }
        offset += line.chars().count() + 1;
    }
    findings
}

fn _pi_find_instruction_phrases(
    text: &str,
    phrase_patterns: Option<&[String]>,
) -> Vec<serde_json::Value> {
    let re = match phrase_patterns {
        Some(custom) if !custom.is_empty() => {
            let escaped: Vec<String> = custom.iter().map(|p| regex::escape(p)).collect();
            let combined = escaped.join("|");
            match Regex::new(&format!("(?i){}", combined)) {
                Ok(r) => r,
                Err(_) => return Vec::new(),
            }
        }
        Some(_) | None => DEFAULT_INSTRUCTION_RE.clone(),
    };

    let mut findings = Vec::new();
    for m in re.find_iter(text) {
        let char_start = byte_offset_to_char_index(text, m.start()).unwrap_or(text.chars().count());
        let char_end = byte_offset_to_char_index(text, m.end()).unwrap_or(text.chars().count());
        findings.push(serde_json::json!({
            "code": "INSTRUCTION_PHRASE",
            "severity": "warn",
            "message": format!("Instruction-like phrase at position {}: '{}'", char_start, m.as_str()),
            "span": {"char_start": char_start, "char_end": char_end},
            "details": {"phrase": m.as_str()},
        }));
    }
    findings
}

fn _pi_compute_risk_score(findings: &[serde_json::Value]) -> i64 {
    let mut score: i64 = 0;
    for f in findings {
        let sev = f.get("severity").and_then(|v| v.as_str()).unwrap_or("info");
        score += match sev {
            "error" => 5,
            "warn" => 3,
            _ => 1,
        };
    }
    score
}

fn _pi_build_summary(findings: &[serde_json::Value], risk_score: i64) -> String {
    if findings.is_empty() {
        return "No red flags detected in the input text.".to_string();
    }

    let mut code_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut sev_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for f in findings {
        let code = f
            .get("code")
            .and_then(|v| v.as_str())
            .unwrap_or("UNKNOWN")
            .to_string();
        *code_counts.entry(code).or_insert(0) += 1;
        let sev = f
            .get("severity")
            .and_then(|v| v.as_str())
            .unwrap_or("info")
            .to_string();
        *sev_counts.entry(sev).or_insert(0) += 1;
    }

    let mut codes: Vec<String> = code_counts.keys().cloned().collect();
    codes.sort();
    let parts: Vec<String> = codes
        .iter()
        .map(|code| format!("{} {}", code_counts.get(code).copied().unwrap_or(0), code))
        .collect();

    let mut sev_parts = Vec::new();
    for sev in &["error", "warn", "info"] {
        if let Some(&count) = sev_counts.get(*sev) {
            sev_parts.push(format!("{} {}", count, sev));
        }
    }

    format!(
        "{} finding(s): {}. Severity: {}. Risk score: {}.",
        findings.len(),
        parts.join(", "),
        sev_parts.join(", "),
        risk_score
    )
}

fn _pi_recommend_next_tool(findings: &[serde_json::Value]) -> Option<serde_json::Value> {
    if findings.is_empty() {
        return None;
    }

    let codes: std::collections::HashSet<String> = findings
        .iter()
        .filter_map(|f| f.get("code").and_then(|v| v.as_str()).map(String::from))
        .collect();

    let mut recommendations: Vec<String> = Vec::new();

    if codes.contains("HIDDEN_CHAR") || codes.contains("BIDI_CONTROL") {
        recommendations.push("text_inspect".to_string());
    }
    if codes.contains("ANSI_ESCAPE") || codes.contains("TERMINAL_CONTROL") {
        recommendations.push("text_transform".to_string());
    }
    if codes.contains("BASE64_BLOB") {
        recommendations.push("text_inspect".to_string());
    }
    if codes.contains("HTML_COMMENT") || codes.contains("MARKDOWN_LINK") {
        recommendations.push("markdown_structure".to_string());
    }
    if codes.contains("INSTRUCTION_PHRASE") {
        recommendations.push("text_inspect".to_string());
    }

    if recommendations.len() == 1 {
        Some(serde_json::Value::String(
            recommendations.into_iter().next().unwrap(),
        ))
    } else if recommendations.is_empty() {
        None
    } else {
        Some(serde_json::Value::Array(
            recommendations
                .into_iter()
                .map(serde_json::Value::String)
                .collect(),
        ))
    }
}

// ---------------------------------------------------------------------------
// text_measure
// ---------------------------------------------------------------------------

pub fn text_measure(args: &Value) -> ToolResponse {
    let text = match _require_str(args, "text", "text_measure") {
        Ok(s) => s,
        Err(e) => return *e,
    };
    let detail = args
        .get("detail")
        .and_then(|v| v.as_str())
        .unwrap_or("normal");

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("text_measure"),
        );
    }

    let valid_details = ["summary", "normal", "full"];
    if !valid_details.contains(&detail) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported detail level: {}", detail),
            Some(vec![format!("Use one of: {}", valid_details.join(", "))]),
            Some("text_measure"),
        );
    }

    let bytes_utf8 = text.len();
    let codepoints = text.chars().count();
    let graphemes = crate::text::count_graphemes(text);
    let word_stats = word_metrics(text);
    let words = word_stats.words;
    let char_stats = char_category_metrics(text);
    let text_lines = split_lines(text);
    let lines = text_lines.len();
    let nonempty_lines = text_lines.iter().filter(|l| !l.is_empty()).count();
    let blank_lines = text_lines.iter().filter(|l| l.is_empty()).count();

    let mut warnings: Vec<String> = Vec::new();
    let combining_marks = char_stats.combining_marks;
    if combining_marks > 0 {
        warnings.push(format!("Text contains {} combining mark(s) - codepoint count diverges from user-perceived characters", combining_marks));
    }

    // Detect special sequences (matching Python's _detect_special_sequences)
    let mut zwj_count = 0;
    let mut variation_selector_count = 0;
    let mut regional_indicator_pairs = 0;
    let mut emoji_modifier_count = 0;
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let cp = chars[i] as u32;
        if cp == 0x200D {
            // ZWJ
            zwj_count += 1;
        } else if (0xFE00..=0xFE0F).contains(&cp) {
            // variation selector
            variation_selector_count += 1;
        } else if (0x1F1E6..=0x1F1FF).contains(&cp) {
            // regional indicator
            if i + 1 < chars.len() {
                let next_cp = chars[i + 1] as u32;
                if (0x1F1E6..=0x1F1FF).contains(&next_cp) {
                    regional_indicator_pairs += 1;
                    i += 1; // skip the pair
                }
            }
        } else if (0x1F3FB..=0x1F3FF).contains(&cp) {
            // emoji modifier
            emoji_modifier_count += 1;
        }
        i += 1;
    }
    if zwj_count > 0 {
        warnings.push(format!(
            "Text contains {} zero-width joiner sequence(s) - sequences may affect display",
            zwj_count
        ));
    }
    if variation_selector_count > 0 {
        warnings.push(format!(
            "Text contains {} variation selector(s) - display may differ",
            variation_selector_count
        ));
    }
    if regional_indicator_pairs > 0 {
        warnings.push(format!(
            "Text contains {} regional indicator pair(s) - these render as flag emoji",
            regional_indicator_pairs
        ));
    }
    if emoji_modifier_count > 0 {
        warnings.push(format!(
            "Text contains {} emoji modifier(s) - modifies base emoji appearance",
            emoji_modifier_count
        ));
    }

    if detail == "summary" {
        let ascii = text.chars().filter(|c| c.is_ascii()).count();
        let non_ascii = codepoints - ascii;
        let result = serde_json::json!({
            "codepoints": codepoints,
            "graphemes": graphemes,
            "words": words,
            "bytes_utf8": bytes_utf8,
            "ascii": ascii,
            "non_ascii": non_ascii,
            "warnings": warnings,
        });
        return ToolResponse::success(result, Some("text_measure")).with_tool("text_measure");
    }

    // normal/full detail
    let unique_words_casefolded: usize = word_stats.unique_words_casefolded;
    let max_line_length_codepoints = text_lines
        .iter()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(0);
    let chars_no_whitespace = text.chars().filter(|c| !c.is_whitespace()).count();
    let ascii = text.chars().filter(|c| c.is_ascii()).count();
    let non_ascii = codepoints - ascii;
    let letters = char_stats.letters;
    let digits = char_stats.digits;
    let punctuation = char_stats.punctuation;
    let symbols = char_stats.symbols;
    let spaces = char_stats.spaces;
    let control_chars = char_stats.control_chars;
    let all_invisibles = unicode_find_invisibles(text);
    let invisible_chars = all_invisibles.len();

    let newline_style = detect_newline_style(text);
    let ends_with_newline = text.ends_with('\n') || text.ends_with('\r');

    let normalization = serde_json::json!({
        "is_nfc": text.nfc().eq(text.chars()),
        "is_nfd": text.nfd().eq(text.chars()),
        "is_nfkc": text.nfkc().eq(text.chars()),
        "is_nfkd": text.nfkd().eq(text.chars()),
    });

    let contains_invisibles = invisible_chars > 0;
    let contains_bidi_controls = all_invisibles
        .iter()
        .any(|inv| inv.display.contains("BIDI"));
    let mixed_script_result = unicode_detect_mixed_scripts(text);
    let scripts = mixed_script_result.scripts;
    let mixed_scripts = mixed_script_result.mixed_scripts;
    let unicode_risks = serde_json::json!({
        "contains_invisibles": contains_invisibles,
        "contains_bidi_controls": contains_bidi_controls,
        "mixed_scripts": mixed_scripts,
        "scripts": scripts,
    });

    let result = serde_json::json!({
        "bytes_utf8": bytes_utf8,
        "codepoints": codepoints,
        "graphemes": graphemes,
        "words": words,
        "unique_words_casefolded": unique_words_casefolded,
        "lines": lines,
        "nonempty_lines": nonempty_lines,
        "blank_lines": blank_lines,
        "max_line_length_codepoints": max_line_length_codepoints,
        "chars_no_whitespace": chars_no_whitespace,
        "ascii": ascii,
        "non_ascii": non_ascii,
        "letters": letters,
        "digits": digits,
        "punctuation": punctuation,
        "symbols": symbols,
        "spaces": spaces,
        "control_chars": control_chars,
        "combining_marks": combining_marks,
        "invisible_chars": invisible_chars,
        "newline_style": newline_style,
        "ends_with_newline": ends_with_newline,
        "normalization": normalization,
        "unicode_risks": unicode_risks,
        "warnings": warnings,
    });

    ToolResponse::success(result, Some("text_measure")).with_tool("text_measure")
}

// ---------------------------------------------------------------------------
// text_equal
// ---------------------------------------------------------------------------

pub fn text_equal(args: &Value) -> ToolResponse {
    let a = match _require_str(args, "a", "text_equal") {
        Ok(s) => s,
        Err(e) => return *e,
    };
    let b = match _require_str(args, "b", "text_equal") {
        Ok(s) => s,
        Err(e) => return *e,
    };
    let casefold = args
        .get("casefold")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let normalization = args
        .get("normalization")
        .and_then(|v| v.as_str())
        .unwrap_or("raw");
    let trim = args.get("trim").and_then(|v| v.as_bool()).unwrap_or(false);
    let ignore_newline_style = args
        .get("ignore_newline_style")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let ignore_trailing_whitespace = args
        .get("ignore_trailing_whitespace")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let ignore_final_newline = args
        .get("ignore_final_newline")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if !["raw", "NFC", "NFD", "NFKC", "NFKD"].contains(&normalization) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported normalization form: {}", normalization),
            Some(vec![format!("Use one of: raw, NFC, NFD, NFKC, NFKD")]),
            Some("text_equal"),
        );
    }

    let mut a_work = a.to_string();
    let mut b_work = b.to_string();

    if ignore_final_newline {
        while a_work.ends_with('\n') || a_work.ends_with('\r') {
            a_work.pop();
        }
        while b_work.ends_with('\n') || b_work.ends_with('\r') {
            b_work.pop();
        }
    }

    if ignore_trailing_whitespace {
        let lines_a: Vec<String> = split_lines(&a_work);
        let lines_b: Vec<String> = split_lines(&b_work);
        a_work = lines_a
            .iter()
            .map(|l| l.trim_end().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        b_work = lines_b
            .iter()
            .map(|l| l.trim_end().to_string())
            .collect::<Vec<_>>()
            .join("\n");
    }

    if ignore_newline_style {
        a_work = a_work.replace("\r\n", "\n").replace("\r", "\n");
        b_work = b_work.replace("\r\n", "\n").replace("\r", "\n");
    }

    if trim {
        a_work = a_work.trim().to_string();
        b_work = b_work.trim().to_string();
    }

    let raw_equal = a_work == b_work;
    let casefold_equal = unicode_casefold(&a_work) == unicode_casefold(&b_work);
    let byte_equal = a_work.as_bytes() == b_work.as_bytes();
    let a_nfc: String = a_work.nfc().collect();
    let b_nfc: String = b_work.nfc().collect();
    let nfc_equal: bool = a_nfc == b_nfc;
    let a_nfd: String = a_work.nfd().collect();
    let b_nfd: String = b_work.nfd().collect();
    let nfd_equal: bool = a_nfd == b_nfd;
    let a_nfkc: String = a_work.nfkc().collect();
    let b_nfkc: String = b_work.nfkc().collect();
    let nfkc_equal: bool = a_nfkc == b_nfkc;
    let a_nfkd: String = a_work.nfkd().collect();
    let b_nfkd: String = b_work.nfkd().collect();
    let nfkd_equal: bool = a_nfkd == b_nfkd;

    let equal = if casefold {
        casefold_equal
    } else if normalization == "raw" {
        raw_equal
    } else {
        let a_norm: String = match normalization {
            "NFC" => a_work.nfc().collect(),
            "NFD" => a_work.nfd().collect(),
            "NFKC" => a_work.nfkc().collect(),
            "NFKD" => a_work.nfkd().collect(),
            _ => a_work.clone(),
        };
        let b_norm: String = match normalization {
            "NFC" => b_work.nfc().collect(),
            "NFD" => b_work.nfd().collect(),
            "NFKC" => b_work.nfkc().collect(),
            "NFKD" => b_work.nfkd().collect(),
            _ => b_work.clone(),
        };
        a_norm == b_norm
    };

    // Find first difference (always compute from post-trim raw values, matching Python)
    let mut a_chars = a_work.chars();
    let mut b_chars = b_work.chars();
    let mut a_idx = 0;
    let mut b_idx = 0;
    let mut found: Option<serde_json::Value> = None;
    loop {
        let a_ch = a_chars.next();
        let b_ch = b_chars.next();
        match (a_ch, b_ch) {
            (Some(ac), Some(bc)) => {
                if ac != bc {
                    found = Some(serde_json::json!({
                        "a_index": a_idx,
                        "b_index": b_idx,
                        "a_char": ac.to_string(),
                        "b_char": bc.to_string(),
                        "a_codepoint": format!("U+{:04X}", ac as u32),
                        "b_codepoint": format!("U+{:04X}", bc as u32),
                        "a_visible": build_safe_repr(&ac.to_string()),
                        "b_visible": build_safe_repr(&bc.to_string()),
                    }));
                    break;
                }
                a_idx += 1;
                b_idx += 1;
            }
            (None, None) => break,
            (Some(_), None) | (None, Some(_)) => {
                found = Some(serde_json::json!({
                    "a_index": a_idx,
                    "b_index": b_idx,
                    "a_char": a_ch.map(|c| c.to_string()).unwrap_or_default(),
                    "b_char": b_ch.map(|c| c.to_string()).unwrap_or_default(),
                    "a_codepoint": a_ch.map(|c| format!("U+{:04X}", c as u32)).unwrap_or_default(),
                    "b_codepoint": b_ch.map(|c| format!("U+{:04X}", c as u32)).unwrap_or_default(),
                    "a_visible": a_ch.map(|c| c.to_string()).unwrap_or_default(),
                    "b_visible": b_ch.map(|c| c.to_string()).unwrap_or_default(),
                }));
                break;
            }
        }
    }

    let a_chars_work: Vec<char> = a_work.chars().collect();
    let b_chars_work: Vec<char> = b_work.chars().collect();
    let length_diff = a_chars_work.len() != b_chars_work.len();
    let invisibles_detected = a_chars_work.iter().any(|c| is_invisible_char(*c))
        || b_chars_work.iter().any(|c| is_invisible_char(*c));

    let classification = classify_difference(
        raw_equal,
        nfc_equal,
        casefold_equal,
        byte_equal,
        length_diff,
        invisibles_detected,
    );

    ToolResponse::success(
        serde_json::json!({
            "equal": equal,
            "mode": {
                "normalization": normalization,
                "casefold": casefold,
                "trim": trim,
                "ignore_newline_style": ignore_newline_style,
                "ignore_trailing_whitespace": ignore_trailing_whitespace,
                "ignore_final_newline": ignore_final_newline,
            },
            "raw_equal": raw_equal,
            "nfc_equal": nfc_equal,
            "nfd_equal": nfd_equal,
            "nfkc_equal": nfkc_equal,
            "nfkd_equal": nfkd_equal,
            "casefold_equal": casefold_equal,
            "byte_equal": byte_equal,
            "lengths": {
                "a_codepoints": a_work.chars().count(),
                "b_codepoints": b_work.chars().count(),
                "a_bytes_utf8": a_work.len(),
                "b_bytes_utf8": b_work.len(),
            },
            "first_difference": found,
            "classification": classification,
        }),
        Some("text_equal"),
    )
    .with_tool("text_equal")
}

// ---------------------------------------------------------------------------
// text_diff_explain
// ---------------------------------------------------------------------------

pub fn text_diff_explain(args: &Value) -> ToolResponse {
    let budget_ctx = crate::mcp::budget::for_handler(crate::mcp::budget::ToolBudget::MODERATE);

    let a = match _require_str(args, "a", "text_diff_explain") {
        Ok(s) => s,
        Err(e) => return *e,
    };
    let b = match _require_str(args, "b", "text_diff_explain") {
        Ok(s) => s,
        Err(e) => return *e,
    };
    let max_diffs = args.get("max_diffs").and_then(|v| v.as_i64()).unwrap_or(20);
    if max_diffs < 0 {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("max_diffs must be non-negative, got {}", max_diffs),
            None,
            Some("text_diff_explain"),
        );
    }
    let max_diffs = max_diffs as usize;
    let include_codepoints = args
        .get("include_codepoints")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let include_context = args
        .get("include_context")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let detail = args
        .get("detail")
        .and_then(|v| v.as_str())
        .unwrap_or("normal");

    if max_diffs > 10000 {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("max_diffs {} exceeds 10000", max_diffs),
            None,
            Some("text_diff_explain"),
        );
    }

    if a.chars().count() > MAX_TEXT_LENGTH || b.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!("Input exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("text_diff_explain"),
        );
    }

    let valid_details = ["summary", "normal", "full"];
    if !valid_details.contains(&detail) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported detail level: {}", detail),
            Some(vec![format!("Use one of: {}", valid_details.join(", "))]),
            Some("text_diff_explain"),
        );
    }

    let max_diffs_to_use = if detail == "summary" {
        max_diffs.min(5)
    } else {
        max_diffs
    };

    let raw_equal = a == b;
    let byte_equal = raw_equal;

    let a_nfc: String = a.nfc().collect();
    let b_nfc: String = b.nfc().collect();
    let nfc_equal = a_nfc == b_nfc;

    let a_nfkc: String = a.nfkc().collect();
    let b_nfkc: String = b.nfkc().collect();
    let nfkc_equal = a_nfkc == b_nfkc;

    let casefold_equal = unicode_casefold(a) == unicode_casefold(b);

    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let same_length_codepoints = a_chars.len() == b_chars.len();
    let length_diff = !same_length_codepoints;

    let a_invisibles = a_chars.iter().any(|c| is_invisible_char(*c));
    let b_invisibles = b_chars.iter().any(|c| is_invisible_char(*c));
    let invisibles_detected = a_invisibles || b_invisibles;

    let mut classification = classify_difference(
        raw_equal,
        nfc_equal,
        casefold_equal,
        byte_equal,
        length_diff,
        invisibles_detected,
    );
    if classification == "ordinary_text_difference" && nfkc_equal {
        classification = "compatibility_normalization_only".to_string();
    }

    let distance = if raw_equal {
        0
    } else {
        crate::text::levenshtein_distance(a, b)
    };

    let all_spans = crate::text::diff_spans(a, b, max_diffs_to_use);
    let truncated = all_spans.len() >= max_diffs_to_use;
    let max_diffs_applied = max_diffs_to_use;

    if budget_ctx.should_stop() {
        return budget_ctx
            .check_should_stop("text_diff_explain")
            .unwrap_err();
    }

    let prefix_len = common_prefix_len(a, b);
    let suffix_len = common_suffix_len(a, b);

    let equal_char_limit = if detail == "summary" { 50 } else { 200 };

    let mut diffs_output: Vec<serde_json::Value> = Vec::new();
    for s in &all_spans {
        let (a_text_show, b_text_show, _truncated_flag) =
            if s.kind == "equal" && s.a_text.chars().count() > equal_char_limit {
                let truncated_a: String = s.a_text.chars().take(equal_char_limit).collect();
                let truncated_b: String = s.b_text.chars().take(equal_char_limit).collect();
                (
                    format!("{}...", truncated_a),
                    format!("{}...", truncated_b),
                    true,
                )
            } else {
                (s.a_text.clone(), s.b_text.clone(), false)
            };
        let a_visible = build_safe_repr(&a_text_show);
        let b_visible = build_safe_repr(&b_text_show);

        let note = if s.kind == "equal" {
            "Matching text".to_string()
        } else if s.a_text.chars().count() != s.b_text.chars().count() {
            format!(
                "Length difference: {} vs {} codepoints",
                s.a_text.chars().count(),
                s.b_text.chars().count()
            )
        } else if nfc_equal {
            "Different raw codepoints, equal after NFC normalization".to_string()
        } else {
            "Different codepoints".to_string()
        };

        diffs_output.push(serde_json::json!({
            "kind": s.kind,
            "a_span": [s.a_start, s.a_end],
            "b_span": [s.b_start, s.b_end],
            "a_text": a_text_show,
            "b_text": b_text_show,
            "a_visible": a_visible,
            "b_visible": b_visible,
            "a_codepoints": if include_codepoints {
                s.a_text.chars().map(|c| {
                    serde_json::json!({
                        "char": format!("{}", c),
                        "codepoint": format!("U+{:04X}", c as u32),
                        "name": unicode_name_char(c),
                    })
                }).collect::<Vec<_>>()
            } else {
                vec![]
            },
            "b_codepoints": if include_codepoints {
                s.b_text.chars().map(|c| {
                    serde_json::json!({
                        "char": format!("{}", c),
                        "codepoint": format!("U+{:04X}", c as u32),
                        "name": unicode_name_char(c),
                    })
                }).collect::<Vec<_>>()
            } else {
                vec![]
            },
            "note": if include_context { note } else { String::new() },
        }));
    }

    let mut security_findings: Vec<serde_json::Value> = Vec::new();

    let a_invisible_count = a_chars.iter().filter(|c| is_invisible_char(**c)).count();
    let b_invisible_count = b_chars.iter().filter(|c| is_invisible_char(**c)).count();
    if a_invisible_count > 0 || b_invisible_count > 0 {
        security_findings.push(serde_json::json!({
            "kind": "invisible_characters",
            "a_count": a_invisible_count,
            "b_count": b_invisible_count,
        }));
    }

    let mut a_confusable_count = 0;
    let mut b_confusable_count = 0;
    for c in a.chars() {
        let key = format!("U+{:04X}", c as u32);
        if crate::text::CONFUSABLES.get(key.as_str()).is_some() {
            a_confusable_count += 1;
        }
    }
    for c in b.chars() {
        let key = format!("U+{:04X}", c as u32);
        if crate::text::CONFUSABLES.get(key.as_str()).is_some() {
            b_confusable_count += 1;
        }
    }
    if a_confusable_count > 0 || b_confusable_count > 0 {
        security_findings.push(serde_json::json!({
            "kind": "confusables",
            "a_count": a_confusable_count,
            "b_count": b_confusable_count,
        }));
    }

    let agent_instruction =
        generate_agent_instruction(&classification, raw_equal, nfc_equal, byte_equal);

    let equal = raw_equal;

    let a_metrics = serde_json::json!({
        "bytes_utf8": a.len(),
        "codepoints": a_chars.len(),
    });

    let b_metrics = serde_json::json!({
        "bytes_utf8": b.len(),
        "codepoints": b_chars.len(),
    });

    let summary = serde_json::json!({
        "raw_equal": raw_equal,
        "byte_equal": byte_equal,
        "nfc_equal": nfc_equal,
        "nfkc_equal": nfkc_equal,
        "casefold_equal": casefold_equal,
        "same_length_codepoints": same_length_codepoints,
        "edit_distance": distance,
        "common_prefix_len": prefix_len,
        "common_suffix_len": suffix_len,
        "truncated": truncated,
        "max_diffs_applied": max_diffs_applied,
    });

    let result = serde_json::json!({
        "equal": equal,
        "classification": classification,
        "summary": summary,
        "a_metrics": a_metrics,
        "b_metrics": b_metrics,
        "diffs": diffs_output,
        "security_findings": security_findings,
        "agent_instruction": agent_instruction,
    });

    ToolResponse::success(result, Some("text_diff_explain")).with_tool("text_diff_explain")
}

// ---------------------------------------------------------------------------
// text_inspect
// ---------------------------------------------------------------------------

pub fn text_inspect(args: &Value) -> ToolResponse {
    let text = match _require_str(args, "text", "text_inspect") {
        Ok(s) => s,
        Err(e) => return *e,
    };
    let include_codepoints = args
        .get("include_codepoints")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let include_confusables = args
        .get("include_confusables")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let detail = args
        .get("detail")
        .and_then(|v| v.as_str())
        .unwrap_or("normal");
    let normalize_form = args
        .get("normalize")
        .and_then(|v| v.as_str())
        .unwrap_or("none");
    let compare_normalized = args
        .get("compare_normalized")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let _ = include_codepoints; // codepoints are always included in invisibles/bidi items (matches Python)

    let valid_details = ["summary", "normal", "full"];
    if !valid_details.contains(&detail) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported detail level: {}", detail),
            Some(vec![format!("Use one of: {}", valid_details.join(", "))]),
            Some("text_inspect"),
        );
    }

    let valid_norms = ["none", "NFC", "NFD", "NFKC", "NFKD"];
    if !valid_norms.contains(&normalize_form) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported normalization form: {}", normalize_form),
            Some(vec![format!("Use one of: {}", valid_norms.join(", "))]),
            Some("text_inspect"),
        );
    }

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("text_inspect"),
        );
    }

    let mut limits_applied: Vec<String> = Vec::new();

    // --- Metrics ---
    let bytes_utf8 = text.len();
    let codepoints = text.chars().count();
    let graphemes = count_graphemes(text);
    let word_stats = word_metrics(text);
    let words = word_stats.words;
    let text_lines = split_lines(text);
    let lines = text_lines.len();
    let nonempty_lines = text_lines.iter().filter(|l| !l.is_empty()).count();
    let blank_lines = text_lines.iter().filter(|l| l.is_empty()).count();

    let mut ascii_count = 0usize;
    let mut non_ascii = 0usize;
    let mut zwj_sequences = 0usize;
    let mut regional_indicator_pairs = 0usize;
    let mut emoji_modifiers = 0usize;
    let chars_vec: Vec<char> = text.chars().collect();
    let char_stats = char_category_metrics(text);
    let all_invisibles = unicode_find_invisibles(text);

    for (i, c) in chars_vec.iter().enumerate() {
        let cp = *c as u32;
        if cp <= 0x7F {
            ascii_count += 1;
        } else {
            non_ascii += 1;
        }
        if cp == 0x200D {
            zwj_sequences += 1;
        }
        if (0x1F1E6..=0x1F1FF).contains(&cp)
            && i + 1 < chars_vec.len()
            && (0x1F1E6..=0x1F1FF).contains(&(chars_vec[i + 1] as u32))
        {
            regional_indicator_pairs += 1;
        }
        if (0x1F3FB..=0x1F3FF).contains(&cp) {
            emoji_modifiers += 1;
        }
    }

    let unique_words_casefolded: usize = word_stats.unique_words_casefolded;
    let max_line_length_codepoints = text_lines
        .iter()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(0);
    let chars_no_whitespace = text.chars().filter(|c| !c.is_whitespace()).count();
    let letters = char_stats.letters;
    let digits = char_stats.digits;
    let punctuation = char_stats.punctuation;
    let symbols = char_stats.symbols;
    let spaces = char_stats.spaces;
    let control_chars = char_stats.control_chars;
    let combining_marks = char_stats.combining_marks;
    let invisible_chars = all_invisibles.len();

    let newline_style = detect_newline_style(text);
    let ends_with_newline = text.ends_with('\n') || text.ends_with('\r');

    // --- Normalization check ---
    let is_nfc: String = text.nfc().collect();
    let is_nfd: String = text.nfd().collect();
    let is_nfkc: String = text.nfkc().collect();
    let is_nfkd: String = text.nfkd().collect();
    let normalization_is_nfc = text == is_nfc;
    let normalization_is_nfd = text == is_nfd;
    let normalization_is_nfkc = text == is_nfkc;
    let normalization_is_nfkd = text == is_nfkd;
    let normalization_diff = !normalization_is_nfc;

    let safe_repr = build_safe_repr(text);
    let normals_repr = if normalization_diff {
        Some(is_nfc.clone())
    } else {
        None
    };

    // --- Invisibles and Bidi Controls ---
    let mut invisibles: Vec<serde_json::Value> = Vec::new();
    let mut bidi_controls: Vec<serde_json::Value> = Vec::new();

    for inv in &all_invisibles {
        let item = serde_json::json!({
            "index": inv.index,
            "char": format!("{}", inv.char),
            "codepoint": inv.codepoint,
            "name": inv.name,
            "category": inv.category,
            "display": inv.display,
        });
        if item
            .get("display")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .contains("BIDI")
        {
            bidi_controls.push(item);
        } else {
            invisibles.push(item);
        }
    }

    // --- Confusables ---
    let confusables: Vec<serde_json::Value> = if include_confusables {
        text.chars()
            .enumerate()
            .filter_map(|(i, c)| {
                let key = format!("U+{:04X}", c as u32);
                crate::text::CONFUSABLES.get(key.as_str()).map(|sub| {
                    let name = unicode_names2::name(c)
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| "<unknown>".to_string());
                    let confusable_chars: String = sub
                        .split_whitespace()
                        .filter_map(|cp| cp.strip_prefix("U+"))
                        .filter_map(|hex| u32::from_str_radix(hex, 16).ok())
                        .filter_map(char::from_u32)
                        .collect();
                    let confusable_name: String = confusable_chars
                        .chars()
                        .map(|ch| {
                            unicode_names2::name(ch)
                                .map(|n| n.to_string())
                                .unwrap_or_else(|| ch.to_string())
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    serde_json::json!({
                        "index": i,
                        "char": format!("{}", c),
                        "codepoint": key,
                        "name": name,
                        "confusable_with": confusable_chars,
                        "confusable_name": confusable_name,
                    })
                })
            })
            .collect()
    } else {
        vec![]
    };

    // --- Mixed Scripts ---
    let mixed_script_result = unicode_detect_mixed_scripts(text);
    let scripts = mixed_script_result.scripts.clone();
    let script_positions: Vec<serde_json::Value> = mixed_script_result
        .positions
        .iter()
        .map(|pos| {
            serde_json::json!({
                "index": pos.index,
                "char": pos.char.to_string(),
                "script": pos.script,
                "codepoint": pos.codepoint,
            })
        })
        .collect();
    let mixed_scripts = mixed_script_result.mixed_scripts;

    // --- Apply detail limits (must run before warnings, since Python iterates over truncated lists) ---
    let max_items = inspect_max_items(detail);
    let invisibles_limited = apply_detail_limit(&invisibles, max_items);
    let confusables_limited = apply_detail_limit(&confusables, max_items);

    if invisibles.len() > max_items {
        limits_applied.push(format!("invisibles_limited={}", max_items));
    }
    if confusables.len() > max_items {
        limits_applied.push(format!("confusables_limited={}", max_items));
    }

    // --- Compute limits_applied_info (Python: computed once, then appended in loop AND at end) ---
    let mut limits_applied_info: Vec<String> = Vec::new();
    let total_invisibles_omitted = invisibles.len() - invisibles_limited.len();
    let total_confusables_omitted = confusables.len() - confusables_limited.len();
    if total_invisibles_omitted > 0 {
        limits_applied_info.push(format!("invisibles_omitted={}", total_invisibles_omitted));
    }
    if total_confusables_omitted > 0 {
        limits_applied_info.push(format!("confusables_omitted={}", total_confusables_omitted));
    }

    // --- Warnings ---
    let mut warnings: Vec<serde_json::Value> = Vec::new();
    for inv in &invisibles_limited {
        let name_str = inv.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let idx = inv.get("index").and_then(|v| v.as_u64()).unwrap_or(0);
        warnings.push(serde_json::json!({
            "severity": "warning",
            "kind": "invisible_character",
            "message": format!("Text contains {} at index {}", name_str, idx),
            "codepoint": inv.get("codepoint").cloned().unwrap_or(serde_json::Value::Null),
        }));
    }
    for bc in &bidi_controls {
        let name_str = bc.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let idx = bc.get("index").and_then(|v| v.as_u64()).unwrap_or(0);
        warnings.push(serde_json::json!({
            "severity": "danger",
            "kind": "bidi_control",
            "message": format!("Text contains bidirectional control character {} at index {}", name_str, idx),
            "codepoint": bc.get("codepoint").cloned().unwrap_or(serde_json::Value::Null),
        }));
    }
    if mixed_scripts {
        warnings.push(serde_json::json!({
            "severity": "info",
            "kind": "mixed_scripts",
            "message": format!("Text contains mixed scripts: {}", scripts.join(", ")),
        }));
    }
    for conf in &confusables_limited {
        let char_str = conf.get("char").and_then(|v| v.as_str()).unwrap_or("");
        let confusable_str = conf
            .get("confusable_with")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        warnings.push(serde_json::json!({
            "severity": "warning",
            "kind": "confusable",
            "message": format!("Text contains confusable character '{}' (looks like '{}')", char_str, confusable_str),
            "codepoint": conf.get("codepoint").cloned().unwrap_or(serde_json::Value::Null),
        }));
    }

    // --- In summary mode, surface omitted counts as info warnings and append to limits_applied (matches Python) ---
    if !limits_applied_info.is_empty() && detail == "summary" {
        for msg in &limits_applied_info {
            warnings.push(serde_json::json!({
                "severity": "info",
                "kind": "limits_applied",
                "message": msg,
            }));
            limits_applied.push(msg.clone());
        }
    }

    // --- Replicate Python's `limits_applied + limits_applied_info` at final return ---
    let final_limits_applied: Vec<String> = limits_applied;

    // --- metrics.warnings (string list, matches Python's MeasureTextResult.warnings) ---
    let mut metrics_warnings: Vec<String> = Vec::new();
    if combining_marks > 0 {
        metrics_warnings.push(format!(
            "Text contains {} combining mark(s) - codepoint count diverges from user-perceived characters",
            combining_marks
        ));
    }
    if zwj_sequences > 0 {
        metrics_warnings.push(format!(
            "Text contains {} zero-width joiner sequence(s) - sequences may affect display",
            zwj_sequences
        ));
    }
    if regional_indicator_pairs > 0 {
        metrics_warnings.push(format!(
            "Text contains {} regional indicator pair(s) - these render as flag emoji",
            regional_indicator_pairs
        ));
    }
    if emoji_modifiers > 0 {
        metrics_warnings.push(format!(
            "Text contains {} emoji modifier(s) - modifies base emoji appearance",
            emoji_modifiers
        ));
    }

    // --- Unicode Risks ---
    let contains_invisibles = !invisibles.is_empty();
    let contains_bidi_controls = !bidi_controls.is_empty();

    // --- Normalization Findings ---
    // Python only populates normalization_findings when compare_normalized=True AND normalize != "none"
    let mut normalization_findings: Vec<serde_json::Value> = Vec::new();

    // --- Normalized analysis (if compare_normalized) ---
    let normalized_output: Option<serde_json::Value> =
        if compare_normalized && normalize_form != "none" {
            let norm_text: String = match normalize_form {
                "NFC" => text.nfc().collect(),
                "NFD" => text.nfd().collect(),
                "NFKC" => text.nfkc().collect(),
                "NFKD" => text.nfkd().collect(),
                _ => text.to_string(),
            };
            let changed = text != norm_text;

            // Populate normalization_findings (matching Python behavior)
            match normalize_form {
                "NFKC" => {
                    normalization_findings.push(serde_json::json!({
                        "kind": "compatibility_fold",
                        "message": "NFKC changes fullwidth character to ASCII",
                    }));
                }
                "NFC" => {
                    if changed {
                        normalization_findings.push(serde_json::json!({
                            "kind": "canonical_composition",
                            "message": "NFC composes combining characters",
                        }));
                    }
                }
                "NFD" => {
                    if changed {
                        normalization_findings.push(serde_json::json!({
                            "kind": "canonical_decomposition",
                            "message": "NFD decomposes combined characters",
                        }));
                    }
                }
                "NFKD" => {
                    normalization_findings.push(serde_json::json!({
                        "kind": "compatibility_decomposition",
                        "message": "NFKD decomposes and converts compatibility characters",
                    }));
                }
                _ => {}
            }

            // Build per-character diff entries (matching Python's format)
            let mut diff_entries: Vec<serde_json::Value> = Vec::new();
            if changed {
                for (i, (c1, c2)) in text.chars().zip(norm_text.chars()).enumerate() {
                    if c1 != c2 {
                        diff_entries.push(serde_json::json!({
                            "index": i,
                            "original": format!("{}", c1),
                            "normalized": format!("{}", c2),
                            "original_codepoint": format!("U+{:04X}", c1 as u32),
                            "normalized_codepoint": format!("U+{:04X}", c2 as u32),
                        }));
                    }
                }
            }

            let norm_safe_repr = build_safe_repr(&norm_text);

            Some(serde_json::json!({
                "form": normalize_form,
                "text": norm_text,
                "safe_repr": norm_safe_repr,
                "changed": changed,
                "diff": diff_entries,
            }))
        } else {
            None
        };

    // --- Original dict (Python uses truncated invisibles/confusables) ---
    let original_dict = serde_json::json!({
        "safe_repr": safe_repr,
        "confusables": confusables_limited,
        "invisibles": invisibles_limited,
    });

    // --- Apply detail limits already computed above ---

    // --- Build result ---
    let result = serde_json::json!({
        "safe_repr": safe_repr,
        "metrics": {
            "bytes_utf8": bytes_utf8,
            "codepoints": codepoints,
            "graphemes": graphemes,
            "words": words,
            "unique_words_casefolded": unique_words_casefolded,
            "lines": lines,
            "nonempty_lines": nonempty_lines,
            "blank_lines": blank_lines,
            "max_line_length_codepoints": max_line_length_codepoints,
            "chars_no_whitespace": chars_no_whitespace,
            "ascii": ascii_count,
            "non_ascii": non_ascii,
            "letters": letters,
            "digits": digits,
            "punctuation": punctuation,
            "symbols": symbols,
            "spaces": spaces,
            "control_chars": control_chars,
            "combining_marks": combining_marks,
            "invisible_chars": invisible_chars,
            "newline_style": newline_style,
            "ends_with_newline": ends_with_newline,
            "normalization": {
                "is_nfc": normalization_is_nfc,
                "is_nfd": normalization_is_nfd,
                "is_nfkc": normalization_is_nfkc,
                "is_nfkd": normalization_is_nfkd,
            },
            "unicode_risks": {
                "contains_invisibles": contains_invisibles,
                "contains_bidi_controls": contains_bidi_controls,
                "mixed_scripts": mixed_scripts,
                "scripts": scripts,
            },
            "warnings": metrics_warnings,
        },
        "normalization": {
            "is_nfc": normalization_is_nfc,
            "is_nfkc": normalization_is_nfkc,
        },
        "normalization_diff": normalization_diff,
        "normals_repr": normals_repr,
        "invisibles": invisibles_limited,
        "bidi_controls": bidi_controls.clone(),
        "mixed_scripts": {
            "mixed_scripts": mixed_scripts,
            "scripts": scripts,
            "positions": script_positions,
        },
        "confusables": confusables_limited,
        "warnings": warnings,
        "limits_applied": final_limits_applied,
        "normalize": normalize_form,
        "compare_normalized": compare_normalized,
        "original": original_dict,
        "normalized": normalized_output,
        "normalization_findings": normalization_findings,
    });

    // --- Envelope findings (per-character, matching Python) ---
    let mut findings: Vec<serde_json::Value> = Vec::new();
    let mut machine_code: Option<String> = None;

    for inv in &invisibles_limited {
        let idx = inv.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let name_str = inv
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        findings.push(serde_json::json!({
            "code": "INVISIBLE_CHAR",
            "severity": "warn",
            "message": format!("Invisible character: {} at index {}", name_str, idx),
            "span": {"char_start": idx, "char_end": idx + 1},
            "details": {"codepoint": inv.get("codepoint"), "category": inv.get("category")},
        }));
    }
    for conf in &confusables_limited {
        let idx = conf.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        findings.push(serde_json::json!({
            "code": "CONFUSABLE_CHAR",
            "severity": "warn",
            "message": format!("Confusable character at index {}", idx),
            "span": {"char_start": idx, "char_end": idx + 1},
            "details": {"original": conf.get("char"), "confusable": conf.get("confusable_with")},
        }));
    }
    for bidi in &bidi_controls {
        let idx = bidi.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let name_str = bidi
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        findings.push(serde_json::json!({
            "code": "BIDI_CONTROL",
            "severity": "warn",
            "message": format!("Bidirectional control character: {} at index {}", name_str, idx),
            "span": {"char_start": idx, "char_end": idx + 1},
            "details": {"codepoint": bidi.get("codepoint")},
        }));
    }

    // Determine machine_code with fixed priority: CONFUSABLES > BIDI > INVISIBLES
    if !findings.is_empty() {
        let codes: std::collections::HashSet<&str> = findings
            .iter()
            .filter_map(|f| f.get("code").and_then(|c| c.as_str()))
            .collect();
        if codes.contains("CONFUSABLE_CHAR") {
            machine_code = Some(machine_codes::CONFUSABLES_DETECTED.to_string());
        } else if codes.contains("BIDI_CONTROL") {
            machine_code = Some(machine_codes::BIDI_DETECTED.to_string());
        } else if codes.contains("INVISIBLE_CHAR") {
            machine_code = Some(machine_codes::INVISIBLES_DETECTED.to_string());
        }
    }

    let mut resp = ToolResponse::success(result, Some("text_inspect"));

    if !findings.is_empty() {
        resp = resp.with_findings(findings);
    }
    if let Some(code) = machine_code {
        resp = resp.with_machine_code(&code);
    }
    resp
}

// ---------------------------------------------------------------------------
// text_count
// ---------------------------------------------------------------------------

pub fn text_count(args: &Value) -> ToolResponse {
    let text = match _require_str(args, "text", "text_count") {
        Ok(s) => s,
        Err(e) => return *e,
    };
    let target = match args.get("target") {
        Some(v) => match v.as_str() {
            Some(s) => Some(s),
            None => {
                return ToolResponse::error_with_code(
                    "invalid_arguments",
                    machine_codes::INVALID_ARGUMENTS,
                    &format!("target must be a string, got {}", json_type_name(v)),
                    None,
                    Some("text_count"),
                )
            }
        },
        None => None,
    };
    let count_mode = args
        .get("count_mode")
        .and_then(|v| v.as_str())
        .unwrap_or("codepoint");
    let normalization = args
        .get("normalization")
        .and_then(|v| v.as_str())
        .unwrap_or("raw");

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("text_count"),
        );
    }

    let valid_count_modes = ["codepoint", "grapheme", "byte", "substring"];
    if !valid_count_modes.contains(&count_mode) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported count_mode: {}", count_mode),
            Some(vec![format!(
                "Use one of: {}",
                valid_count_modes.join(", ")
            )]),
            Some("text_count"),
        );
    }

    if !["raw", "NFC", "NFKC"].contains(&normalization) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported normalization form: {}", normalization),
            Some(vec![format!("Use one of: raw, NFC, NFKC")]),
            Some("text_count"),
        );
    }

    let work_text = normalize_text_count_input(text, normalization);

    if let Some(target) = target {
        const MAX_TARGET_LENGTH: usize = 1000;
        if target.chars().count() > MAX_TARGET_LENGTH {
            return ToolResponse::error_with_code(
                "input_too_large",
                machine_codes::INPUT_TOO_LARGE,
                &format!(
                    "target length {} exceeds {}",
                    target.chars().count(),
                    MAX_TARGET_LENGTH
                ),
                None,
                Some("text_count"),
            );
        }

        let work_target = normalize_text_count_input(target, normalization);
        if let Some(error) = validate_text_count_target(target, &work_target, count_mode) {
            return error;
        }

        let (count, positions) = text_count_matches(&work_text, &work_target, count_mode);

        let text_length_codepoints = match count_mode {
            "grapheme" => crate::text::count_graphemes(&work_text),
            _ => work_text.chars().count(),
        };

        ToolResponse::success(
            serde_json::json!({
                "count": count,
                "target": target,
                "normalization": normalization,
                "positions": positions,
                "text_length_codepoints": text_length_codepoints,
            }),
            Some("text_count"),
        )
        .with_tool("text_count")
    } else {
        let freq = crate::text::char_frequency(&work_text);
        let freq_value =
            serde_json::to_value(freq).unwrap_or(serde_json::Value::Object(Default::default()));
        ToolResponse::success(freq_value, Some("text_count")).with_tool("text_count")
    }
}

// ---------------------------------------------------------------------------
// text_truncate
// ---------------------------------------------------------------------------

pub fn text_truncate(args: &Value) -> ToolResponse {
    let text = match args.get("text") {
        Some(v) => match v.as_str() {
            Some(s) => s,
            None => {
                return ToolResponse::error_with_code(
                    "invalid_arguments",
                    machine_codes::INVALID_ARGUMENTS,
                    &format!("text must be a string, got {}", json_type_name(v)),
                    None,
                    Some("text_truncate"),
                )
            }
        },
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "text must be a string, got NoneType",
                None,
                Some("text_truncate"),
            )
        }
    };
    let max_graphemes = match args.get("max_graphemes").and_then(|v| v.as_i64()) {
        Some(n) => n,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'max_graphemes' parameter",
                None,
                Some("text_truncate"),
            )
        }
    };

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!(
                "text length {} exceeds {}",
                text.chars().count(),
                MAX_TEXT_LENGTH
            ),
            Some(vec![format!(
                "Maximum input length is {} characters",
                MAX_TEXT_LENGTH
            )]),
            Some("text_truncate"),
        );
    }

    if max_graphemes < 0 {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("max_graphemes must be non-negative, got {}", max_graphemes),
            Some(vec!["Set max_graphemes to 0 or higher".to_string()]),
            Some("text_truncate"),
        );
    }

    let original_graphemes = crate::text::count_graphemes(text) as i64;

    if original_graphemes <= max_graphemes {
        return ToolResponse::success(
            serde_json::json!({
                "original_graphemes": original_graphemes,
                "truncated_graphemes": original_graphemes,
                "truncated": false,
                "text": text,
            }),
            Some("text_truncate"),
        )
        .with_tool("text_truncate");
    }

    let truncated_text = crate::text::truncate_to_grapheme(text, max_graphemes as usize);
    ToolResponse::success(
        serde_json::json!({
            "original_graphemes": original_graphemes,
            "truncated_graphemes": max_graphemes,
            "truncated": true,
            "text": truncated_text,
        }),
        Some("text_truncate"),
    )
    .with_tool("text_truncate")
}

// ---------------------------------------------------------------------------
// text_fingerprint_tool
// ---------------------------------------------------------------------------

pub fn text_fingerprint_tool(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'text' parameter",
                None,
                Some("text_fingerprint"),
            )
        }
    };
    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("text_fingerprint"),
        );
    }

    let unicode = args
        .get("unicode")
        .and_then(|v| v.as_str())
        .unwrap_or("raw");
    let newline = args
        .get("newline")
        .and_then(|v| v.as_str())
        .unwrap_or("raw");
    let trim_final_newline = args
        .get("trim_final_newline")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let casefold = args
        .get("casefold")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let valid_unicode = ["raw", "NFC", "NFD", "NFKC", "NFKD"];
    if !valid_unicode.contains(&unicode) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported unicode normalization: {}", unicode),
            Some(vec![format!("Use one of: {}", valid_unicode.join(", "))]),
            Some("text_fingerprint"),
        );
    }

    let valid_newline = ["raw", "LF"];
    if !valid_newline.contains(&newline) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported newline normalization: {}", newline),
            Some(vec![format!("Use one of: {}", valid_newline.join(", "))]),
            Some("text_fingerprint"),
        );
    }

    let result: TextFingerprintResult =
        text_fingerprint(text, unicode, newline, trim_final_newline, casefold);
    ToolResponse::success(
        serde_json::json!({
            "sha256": result.sha256,
            "bytes_utf8": result.bytes_utf8,
            "codepoints": result.codepoints,
            "graphemes": result.graphemes,
            "newline_style": result.newline_style,
            "normalization": result.normalization,
            "summary": result.summary,
        }),
        Some("text_fingerprint"),
    )
    .with_tool("text_fingerprint")
}

// ---------------------------------------------------------------------------
// text_hash
// ---------------------------------------------------------------------------

pub fn text_hash(args: &Value) -> ToolResponse {
    let text = match _require_str(args, "text", "text_hash") {
        Ok(s) => s,
        Err(e) => return *e,
    };
    let algorithms = match args.get("algorithms") {
        Some(Value::Array(arr)) => {
            if arr.len() > 10 {
                return ToolResponse::error_with_code(
                    "invalid_arguments",
                    machine_codes::INVALID_ARGUMENTS,
                    &format!("algorithms list length {} exceeds 10", arr.len()),
                    None,
                    Some("text_hash"),
                );
            }
            let non_str_indices: Vec<usize> = arr
                .iter()
                .enumerate()
                .filter(|(_, v)| !v.is_string())
                .map(|(i, _)| i)
                .collect();
            if !non_str_indices.is_empty() {
                return ToolResponse::error_with_code(
                    "invalid_arguments",
                    machine_codes::INVALID_ARGUMENTS,
                    "All algorithms must be strings",
                    Some(vec![format!(
                        "Non-string items at indices: {:?}",
                        &non_str_indices[..5.min(non_str_indices.len())]
                    )]),
                    Some("text_hash"),
                );
            }
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        }
        None => vec!["sha256".to_string()],
        _ => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                &format!(
                    "algorithms must be a list, got {}",
                    json_type_name(args.get("algorithms").unwrap_or(&Value::Null))
                ),
                None,
                Some("text_hash"),
            )
        }
    };
    let encoding = args
        .get("encoding")
        .and_then(|v| v.as_str())
        .unwrap_or("utf-8");
    let detail = args
        .get("detail")
        .and_then(|v| v.as_str())
        .unwrap_or("normal");

    if !["summary", "normal", "full"].contains(&detail) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported detail level: {}", detail),
            Some(vec!["Use one of: summary, normal, full".to_string()]),
            Some("text_hash"),
        );
    }

    let valid_encodings = [
        "utf-8",
        "utf8",
        "ascii",
        "us-ascii",
        "latin-1",
        "latin1",
        "iso-8859-1",
        "utf-16",
        "utf16",
        "utf-16-le",
        "utf-16-be",
        "utf-32",
        "utf32",
        "utf-32-le",
        "utf-32-be",
        "cp1252",
        "windows-1252",
        "shift_jis",
        "shiftjis",
        "cp932",
        "euc-jp",
        "euc_jp",
        "gb2312",
        "gbk",
        "gb18030",
        "big5",
        "euc-kr",
        "euc_kr",
        "iso-8859-2",
        "iso-8859-3",
        "iso-8859-4",
        "iso-8859-5",
        "iso-8859-6",
        "iso-8859-7",
        "iso-8859-8",
        "iso-8859-9",
        "iso-8859-10",
        "iso-8859-13",
        "iso-8859-14",
        "iso-8859-15",
        "iso-8859-16",
        "koi8-r",
        "koi8_u",
        "cp866",
        "cp874",
    ];
    let enc_lower = encoding.to_lowercase().replace(['-', '_'], "");
    let known = valid_encodings
        .iter()
        .any(|e| e.replace(['-', '_'], "") == enc_lower);
    if !known {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Invalid encoding: {}", encoding),
            Some(vec![
                "Use a valid encoding name like 'utf-8', 'ascii', 'latin-1'".to_string(),
            ]),
            Some("text_hash"),
        );
    }

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!(
                "text length {} exceeds MAX_TEXT_LENGTH {}",
                text.chars().count(),
                MAX_TEXT_LENGTH
            ),
            Some(vec![format!(
                "Maximum input length is {} characters",
                MAX_TEXT_LENGTH
            )]),
            Some("text_hash"),
        );
    }

    let result: TextHashResult = crate::text::text_hash(text, &algorithms, encoding);

    if detail == "summary" {
        ToolResponse::success(
            serde_json::json!({
                "summary": result.summary,
            }),
            Some("text_hash"),
        )
        .with_tool("text_hash")
    } else {
        ToolResponse::success(
            serde_json::json!({
                "encoding": result.encoding,
                "bytes": result.bytes,
                "codepoints": result.codepoints,
                "hashes": result.hashes,
                "warnings": result.warnings,
                "summary": result.summary,
            }),
            Some("text_hash"),
        )
        .with_tool("text_hash")
    }
}

// ---------------------------------------------------------------------------
// text_position
// ---------------------------------------------------------------------------

pub fn text_position(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'text' parameter",
                None,
                Some("text_position"),
            )
        }
    };
    let byte_offset = args
        .get("byte_offset")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let codepoint_index = args
        .get("codepoint_index")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let line = args
        .get("line")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let column = args
        .get("column")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let utf16_offset = args
        .get("utf16_offset")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let line_base = args.get("line_base").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
    let column_base = args
        .get("column_base")
        .and_then(|v| v.as_u64())
        .unwrap_or(1) as usize;
    let detail = args
        .get("detail")
        .and_then(|v| v.as_str())
        .unwrap_or("normal");

    let valid_details = ["summary", "normal", "full"];
    if !valid_details.contains(&detail) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported detail level: {}", detail),
            Some(vec![format!("Use one of: {}", valid_details.join(", "))]),
            Some("text_position"),
        );
    }

    if line_base != 0 && line_base != 1 {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("line_base must be 0 or 1, got {}", line_base),
            Some(vec!["Set line_base to 0 or 1".to_string()]),
            Some("text_position"),
        );
    }
    if column_base != 0 && column_base != 1 {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("column_base must be 0 or 1, got {}", column_base),
            Some(vec!["Set column_base to 0 or 1".to_string()]),
            Some("text_position"),
        );
    }

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!(
                "text length {} exceeds MAX_TEXT_LENGTH {}",
                text.chars().count(),
                MAX_TEXT_LENGTH
            ),
            Some(vec![format!(
                "Maximum input length is {} characters",
                MAX_TEXT_LENGTH
            )]),
            Some("text_position"),
        );
    }

    let result: TextPositionResult = crate::text::position::text_position(
        text,
        byte_offset,
        codepoint_index,
        line,
        column,
        utf16_offset,
        line_base,
        column_base,
    );

    if !result.valid {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            result.error.as_deref().unwrap_or("Invalid position"),
            None,
            Some("text_position"),
        );
    }

    let output = if detail == "summary" {
        serde_json::json!({
            "summary": result.summary,
        })
    } else {
        serde_json::json!({
            "valid": result.valid,
            "byte_offset": result.byte_offset,
            "codepoint_index": result.codepoint_index,
            "utf16_offset": result.utf16_offset,
            "line": result.line,
            "column": result.column,
            "line_base": result.line_base,
            "column_base": result.column_base,
            "char": result.char,
            "codepoint": result.codepoint,
            "name": result.name,
            "line_text_preview": result.line_text_preview,
            "error": result.error,
            "summary": result.summary,
        })
    };

    ToolResponse::success(output, Some("text_position")).with_tool("text_position")
}

// ---------------------------------------------------------------------------
// text_window
// ---------------------------------------------------------------------------

pub fn text_window(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'text' parameter",
                None,
                Some("text_window"),
            )
        }
    };
    let position = match args.get("position").and_then(|v| v.as_object()) {
        Some(obj) => obj.clone(),
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'position' parameter",
                None,
                Some("text_window"),
            )
        }
    };
    let context_lines = match args.get("context_lines") {
        Some(v) => {
            if let Some(n) = v.as_i64() {
                if n < 0 {
                    return ToolResponse::error_with_code(
                        "invalid_arguments",
                        machine_codes::INVALID_ARGUMENTS,
                        &format!("context_lines must be non-negative, got {}", n),
                        Some(vec!["Set context_lines to 0 or higher".to_string()]),
                        Some("text_window"),
                    );
                }
                if n > 10000 {
                    return ToolResponse::error_with_code(
                        "invalid_arguments",
                        machine_codes::INVALID_ARGUMENTS,
                        &format!("context_lines {} exceeds 10000", n),
                        None,
                        Some("text_window"),
                    );
                }
                n as usize
            } else {
                return ToolResponse::error_with_code(
                    "invalid_arguments",
                    machine_codes::INVALID_ARGUMENTS,
                    &format!(
                        "context_lines must be an integer, got {}",
                        json_type_name(v)
                    ),
                    None,
                    Some("text_window"),
                );
            }
        }
        None => 2,
    };
    let include_visible_repr = args
        .get("include_visible_repr")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!(
                "text length {} exceeds MAX_TEXT_LENGTH {}",
                text.chars().count(),
                MAX_TEXT_LENGTH
            ),
            Some(vec![format!(
                "Maximum input length is {} characters",
                MAX_TEXT_LENGTH
            )]),
            Some("text_window"),
        );
    }

    // Validate position kind
    let valid_kinds = [
        "byte_offset",
        "codepoint_index",
        "grapheme_index",
        "line_column",
    ];
    let pos_kind = position
        .get("kind")
        .and_then(|v| v.as_str())
        .unwrap_or("codepoint_index");
    if !valid_kinds.contains(&pos_kind) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unknown position kind: {}", pos_kind),
            Some(vec![format!("Use one of: {}", valid_kinds.join(", "))]),
            Some("text_window"),
        );
    }

    // Validate integer fields in position
    let max_pos = MAX_TEXT_LENGTH * 16;
    for key in &[
        "value",
        "byte_offset",
        "codepoint_index",
        "grapheme_index",
        "line",
        "column",
    ] {
        if let Some(v) = position.get(*key) {
            if let Some(n) = v.as_i64() {
                if n < 0 || n as usize > max_pos {
                    return ToolResponse::error_with_code(
                        "invalid_arguments",
                        machine_codes::INVALID_ARGUMENTS,
                        &format!("position.{}={} out of range [0, {}]", key, n, max_pos),
                        None,
                        Some("text_window"),
                    );
                }
            } else if v.is_boolean() {
                return ToolResponse::error_with_code(
                    "invalid_arguments",
                    machine_codes::INVALID_ARGUMENTS,
                    &format!(
                        "position.{} must be an integer, got {}",
                        key,
                        if v.as_bool().unwrap() {
                            "boolean (true)"
                        } else {
                            "boolean (false)"
                        }
                    ),
                    None,
                    Some("text_window"),
                );
            } else {
                return ToolResponse::error_with_code(
                    "invalid_arguments",
                    machine_codes::INVALID_ARGUMENTS,
                    &format!(
                        "position.{} must be an integer, got {}",
                        key,
                        json_type_name(v)
                    ),
                    None,
                    Some("text_window"),
                );
            }
        }
    }

    let pos_value = position
        .get("value")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let pos_byte_offset = position
        .get("byte_offset")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let pos_codepoint_index = position
        .get("codepoint_index")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let pos_grapheme_index = position
        .get("grapheme_index")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let pos_line = position
        .get("line")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let pos_column = position
        .get("column")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let pos_line_base = position
        .get("line_base")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let pos_column_base = position
        .get("column_base")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);

    let text_position = TextWindowPosition {
        kind: pos_kind.to_string(),
        value: pos_value,
        byte_offset: pos_byte_offset,
        codepoint_index: pos_codepoint_index,
        grapheme_index: pos_grapheme_index,
        line: pos_line,
        column: pos_column,
        line_base: pos_line_base,
        column_base: pos_column_base,
    };

    let result: TextWindowResult = crate::text::position::text_window(
        text,
        &text_position,
        context_lines,
        include_visible_repr,
    );

    ToolResponse::success(
        serde_json::json!({
            "position": result.position,
            "line_text": result.line_text,
            "line_visible_repr": result.line_visible_repr,
            "before": result.before,
            "after": result.after,
            "newline_style": result.newline_style,
            "at_codepoint": result.at_codepoint,
            "warnings": result.warnings,
        }),
        Some("text_window"),
    )
    .with_tool("text_window")
}

// ---------------------------------------------------------------------------
// text_transform
// ---------------------------------------------------------------------------

pub fn text_transform(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'text' parameter",
                None,
                Some("text_transform"),
            )
        }
    };
    let operations = match args.get("operations").and_then(|v| v.as_array()) {
        Some(arr) => {
            let mut ops = Vec::new();
            for (i, v) in arr.iter().enumerate() {
                match v.as_str() {
                    Some(s) => ops.push(s.to_string()),
                    None => {
                        return ToolResponse::error_with_code(
                            "invalid_arguments",
                            machine_codes::INVALID_ARGUMENTS,
                            &format!(
                                "operations list items must be strings, operation {} is {}",
                                i,
                                json_type_name(v)
                            ),
                            None,
                            Some("text_transform"),
                        )
                    }
                }
            }
            ops
        }
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'operations' parameter",
                None,
                Some("text_transform"),
            )
        }
    };
    let detail = args
        .get("detail")
        .and_then(|v| v.as_str())
        .unwrap_or("normal");

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!(
                "text length {} exceeds MAX_TEXT_LENGTH {}",
                text.chars().count(),
                MAX_TEXT_LENGTH
            ),
            Some(vec![format!(
                "Maximum input length is {} characters",
                MAX_TEXT_LENGTH
            )]),
            Some("text_transform"),
        );
    }

    if operations.len() > 100 {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!(
                "operations list too large ({} items, max 100)",
                operations.len()
            ),
            Some(vec!["Maximum 100 operations allowed per call".to_string()]),
            Some("text_transform"),
        );
    }

    let valid_operations = [
        "normalize_nfc",
        "normalize_nfd",
        "normalize_nfkc",
        "normalize_nfkd",
        "casefold",
        "trim",
        "trim_trailing_whitespace",
        "normalize_newlines_lf",
        "ensure_final_newline",
        "strip_final_newline",
        "remove_zero_width",
        "remove_bidi_controls",
        "visible_repr",
    ];
    let mut unknown_ops: Vec<String> = Vec::new();
    for op in &operations {
        if !valid_operations.contains(&op.to_lowercase().as_str()) {
            unknown_ops.push(op.clone());
        }
    }
    if !unknown_ops.is_empty() {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unknown operation(s): {}", unknown_ops.join(", ")),
            Some(vec![format!(
                "Valid operations: {}",
                valid_operations.join(", ")
            )]),
            Some("text_transform"),
        );
    }

    if !["summary", "normal", "full"].contains(&detail) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported detail level: {}", detail),
            Some(vec![format!("Use one of: summary, normal, full")]),
            Some("text_transform"),
        );
    }

    let result: TextTransformResult = crate::text::text_transform(text, &operations);

    let output = match detail {
        "summary" => serde_json::json!({
            "changed": result.changed,
            "text": result.text,
            "operations_applied": result.operations_applied,
            "warnings": result.warnings,
            "summary": result.summary,
        }),
        _ => serde_json::json!({
            "changed": result.changed,
            "text": result.text,
            "operations_applied": result.operations_applied,
            "removed": result.removed,
            "warnings": result.warnings,
            "summary": result.summary,
        }),
    };

    ToolResponse::success(output, Some("text_transform")).with_tool("text_transform")
}

// ---------------------------------------------------------------------------
// text_replace_check
// ---------------------------------------------------------------------------

pub fn text_replace_check_tool(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'text' parameter",
                None,
                Some("text_replace_check"),
            )
        }
    };
    let old = match args.get("old").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'old' parameter",
                None,
                Some("text_replace_check"),
            )
        }
    };
    let new = match args.get("new").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'new' parameter",
                None,
                Some("text_replace_check"),
            )
        }
    };
    let mode = args.get("mode").and_then(|v| v.as_str()).unwrap_or("exact");
    let expected_count = args
        .get("expected_count")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize);
    let allow_multiple = args
        .get("allow_multiple")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let newline_policy = args
        .get("newline_policy")
        .and_then(|v| v.as_str())
        .unwrap_or("preserve");
    let return_preview = args
        .get("return_preview")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let max_preview_chars = if let Some(v) = args.get("max_preview_chars") {
        if let Some(n) = v.as_i64() {
            if n < 0 {
                return ToolResponse::error_with_code(
                    "invalid_arguments",
                    machine_codes::INVALID_ARGUMENTS,
                    &format!("max_preview_chars must be non-negative, got {}", n),
                    None,
                    Some("text_replace_check"),
                );
            }
            n as usize
        } else {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "max_preview_chars must be an integer",
                None,
                Some("text_replace_check"),
            );
        }
    } else {
        2000
    };

    const MAX_PREVIEW_CHARS: usize = 100_000;
    if max_preview_chars > MAX_PREVIEW_CHARS {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!(
                "max_preview_chars {} exceeds {}",
                max_preview_chars, MAX_PREVIEW_CHARS
            ),
            None,
            Some("text_replace_check"),
        );
    }

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("text_replace_check"),
        );
    }

    if old.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!("old exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("text_replace_check"),
        );
    }

    if new.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!("new exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("text_replace_check"),
        );
    }

    let valid_modes = ["exact", "nfc", "nfkc", "casefold", "whitespace_collapse"];
    if !valid_modes.contains(&mode) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported mode: {}", mode),
            Some(vec![format!("Use one of: {}", valid_modes.join(", "))]),
            Some("text_replace_check"),
        );
    }

    let valid_newline_policies = ["preserve", "normalize_lf", "normalize_crlf"];
    if !valid_newline_policies.contains(&newline_policy) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported newline_policy: {}", newline_policy),
            Some(vec![format!(
                "Use one of: {}",
                valid_newline_policies.join(", ")
            )]),
            Some("text_replace_check"),
        );
    }

    match crate::text::replace::text_replace_check(
        text,
        old,
        new,
        mode,
        expected_count,
        allow_multiple,
        newline_policy,
        return_preview,
        max_preview_chars,
    ) {
        Ok(result) => ToolResponse::success(
            serde_json::json!({
                "match_count": result.match_count,
                "unique_match": result.unique_match,
                "expected_count_met": result.expected_count_met,
                "would_change": result.would_change,
                "positions": result.positions,
                "changed_text_fingerprint": result.changed_text_fingerprint,
                "newline_style_before": result.newline_style_before,
                "newline_style_after": result.newline_style_after,
                "preview_before": result.preview_before,
                "preview_after": result.preview_after,
                "findings": result.findings,
            }),
            Some("text_replace_check"),
        )
        .with_tool("text_replace_check"),
        Err(e) => ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &e,
            None,
            Some("text_replace_check"),
        ),
    }
}

// ---------------------------------------------------------------------------
// text_security_inspect
// ---------------------------------------------------------------------------

pub fn text_security_inspect(args: &Value) -> ToolResponse {
    let budget_ctx = crate::mcp::budget::for_handler(crate::mcp::budget::ToolBudget::HEAVY);

    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'text' parameter",
                None,
                Some("text_security_inspect"),
            )
        }
    };
    let policy = args
        .get("policy")
        .and_then(|v| v.as_str())
        .unwrap_or("default");
    let normalize = args
        .get("normalize")
        .and_then(|v| v.as_str())
        .unwrap_or("none");
    let compare_normalized = args
        .get("compare_normalized")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let detail = args
        .get("detail")
        .and_then(|v| v.as_str())
        .unwrap_or("summary");

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("text_security_inspect"),
        );
    }

    let valid_policies = ["default", "source_code", "prompt", "markdown", "identifier"];
    if !valid_policies.contains(&policy) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("policy must be one of: {}", valid_policies.join(", ")),
            None,
            Some("text_security_inspect"),
        );
    }

    let valid_normalizations = ["none", "NFC", "NFD", "NFKC", "NFKD"];
    if !valid_normalizations.contains(&normalize) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported normalize form: {}", normalize),
            Some(vec![format!(
                "Use one of: {}",
                valid_normalizations.join(", ")
            )]),
            Some("text_security_inspect"),
        );
    }

    let valid_details = ["summary", "normal", "full"];
    if !valid_details.contains(&detail) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported detail level: {}", detail),
            Some(vec![format!("Use one of: {}", valid_details.join(", "))]),
            Some("text_security_inspect"),
        );
    }

    let mut subresults: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
    let mut all_findings: Vec<serde_json::Value> = Vec::new();
    let mut code_list: Vec<String> = Vec::new();

    // Helper to safely call sub-tools and record errors (matching Python's try/except pattern)
    let store_subresult = |subresults: &mut serde_json::Map<String, serde_json::Value>,
                           key: &str,
                           result: &Option<serde_json::Value>,
                           err: &Option<String>| {
        if let Some(ref r) = result {
            subresults.insert(key.to_string(), r.clone());
        } else if let Some(ref e) = err {
            subresults.insert(key.to_string(), serde_json::json!({"error": e}));
        }
    };

    // 1. Always call text_inspect (pass detail, normalize, compare_normalized)
    let text_inspect_args = serde_json::json!({
        "text": text,
        "detail": detail,
        "normalize": normalize,
        "compare_normalized": compare_normalized,
    });
    let ti_result = text_inspect(&text_inspect_args);
    store_subresult(
        &mut subresults,
        "text_inspect",
        &ti_result.result,
        &ti_result.error,
    );
    if let Some(ref r) = ti_result.result {
        // Check warnings from text_inspect
        if let Some(warnings) = r.get("warnings").and_then(|v| v.as_array()) {
            for w in warnings {
                if !w.is_null() {
                    all_findings.push(finding(
                        "TEXT_INSPECT_WARNING",
                        severity::MEDIUM,
                        w.as_str().unwrap_or("text inspection warning"),
                        Some(disposition::CAUTION),
                        None,
                    ));
                }
            }
        }
        // Check invisibles (Python extracts inv from result["invisibles"])
        if let Some(inv) = r.get("invisibles").and_then(|v| v.as_array()) {
            if !inv.is_empty() {
                if !code_list.contains(&machine_codes::UNICODE_RISK.to_string()) {
                    code_list.push(machine_codes::UNICODE_RISK.to_string());
                }
                all_findings.push(finding(
                    "HIDDEN_CHARS",
                    severity::MEDIUM,
                    &format!("Found {} invisible character(s)", inv.len()),
                    Some(disposition::CAUTION),
                    None,
                ));
            }
        }
        // Check confusables
        if let Some(conf) = r.get("confusables").and_then(|v| v.as_array()) {
            if !conf.is_empty() {
                if !code_list.contains(&machine_codes::UNICODE_RISK.to_string()) {
                    code_list.push(machine_codes::UNICODE_RISK.to_string());
                }
                all_findings.push(finding(
                    "CONFUSABLES",
                    severity::MEDIUM,
                    &format!("Found {} confusable character(s)", conf.len()),
                    Some(disposition::CAUTION),
                    None,
                ));
            }
        }
    }

    if budget_ctx.should_stop() {
        return budget_ctx
            .check_should_stop("text_security_inspect")
            .unwrap_err();
    }

    // 2. Map policy to unicode_policy_check policy (matching Python behavior)
    // Python: upolicy = "source_code" if policy == "source_code" else "human_text"
    let uc_policy = if policy == "source_code" {
        "source_code"
    } else {
        "human_text"
    };
    let uc_args = serde_json::json!({"text": text, "policy": uc_policy});
    let uc_result = crate::tools::unicode_policy_check(&uc_args);
    store_subresult(
        &mut subresults,
        "unicode_policy_check",
        &uc_result.result,
        &uc_result.error,
    );
    // Iterate individual findings (matching Python behavior)
    if let Some(ref r) = uc_result.result {
        if let Some(up_findings) = r.get("findings").and_then(|v| v.as_array()) {
            for f in up_findings {
                let raw_sev = f.get("severity").and_then(|v| v.as_str()).unwrap_or("info");
                let sev = match raw_sev {
                    "error" | "critical" => severity::HIGH,
                    "warn" | "warning" => severity::MEDIUM,
                    "danger" => severity::HIGH,
                    "info" => severity::INFO,
                    other => other,
                };
                let disp = match sev {
                    severity::HIGH => Some(disposition::BLOCKING),
                    severity::MEDIUM => Some(disposition::CAUTION),
                    _ => Some(disposition::INFORMATIONAL),
                };
                let code = f
                    .get("code")
                    .and_then(|v| v.as_str())
                    .unwrap_or("UNICODE_POLICY");
                let msg = f.get("message").and_then(|v| v.as_str()).unwrap_or("");
                all_findings.push(finding(code, sev, msg, disp, None));
                if raw_sev == "error"
                    && !code_list.contains(&machine_codes::UNICODE_RISK.to_string())
                {
                    code_list.push(machine_codes::UNICODE_RISK.to_string());
                }
            }
        }
    }

    if budget_ctx.should_stop() {
        return budget_ctx
            .check_should_stop("text_security_inspect")
            .unwrap_err();
    }

    // 3. If normalize != "none", use unicodedata.normalize directly (matching Python)
    if normalize != "none" {
        let normalized: String = match normalize {
            "NFC" => text.nfc().collect(),
            "NFD" => text.nfd().collect(),
            "NFKC" => text.nfkc().collect(),
            "NFKD" => text.nfkd().collect(),
            _ => text.to_string(),
        };
        let changed = normalized != text;
        subresults.insert(
            "canonicalize_text".to_string(),
            serde_json::json!({
                "changed": changed,
                "form": normalize,
            }),
        );
        if changed && !code_list.contains(&machine_codes::NORMALIZATION_DIFF.to_string()) {
            code_list.push(machine_codes::NORMALIZATION_DIFF.to_string());
        }
    }
    // 4. If policy is "prompt", "markdown", or "default", call prompt_input_inspect
    if matches!(policy, "prompt" | "markdown" | "default") {
        let pi_args = serde_json::json!({"text": text});
        let pi_result = prompt_input_inspect_tool(&pi_args);
        store_subresult(
            &mut subresults,
            "prompt_input_inspect",
            &pi_result.result,
            &pi_result.error,
        );
        // Iterate individual findings (matching Python behavior)
        if let Some(ref r) = pi_result.result {
            if let Some(pi_findings) = r.get("findings").and_then(|v| v.as_array()) {
                for f in pi_findings {
                    let code = f
                        .get("code")
                        .and_then(|v| v.as_str())
                        .unwrap_or("PROMPT_RISK");
                    let raw_sev = f.get("severity").and_then(|v| v.as_str()).unwrap_or("warn");
                    let sev = match raw_sev {
                        "error" | "critical" => severity::HIGH,
                        "warn" | "warning" => severity::MEDIUM,
                        "danger" => severity::HIGH,
                        "info" => severity::INFO,
                        other => other,
                    };
                    let disp = match sev {
                        severity::HIGH => Some(disposition::BLOCKING),
                        severity::MEDIUM => Some(disposition::CAUTION),
                        _ => Some(disposition::INFORMATIONAL),
                    };
                    let msg = f.get("message").and_then(|v| v.as_str()).unwrap_or("");
                    all_findings.push(finding(code, sev, msg, disp, None));
                }
                if pi_findings.iter().any(|f| {
                    let sev = f.get("severity").and_then(|v| v.as_str()).unwrap_or("");
                    sev == "warn" || sev == "error"
                }) && !code_list.contains(&machine_codes::PROMPT_INJECTION_RISK.to_string())
                {
                    code_list.push(machine_codes::PROMPT_INJECTION_RISK.to_string());
                }
            }
        }
    }

    if budget_ctx.should_stop() {
        return budget_ctx
            .check_should_stop("text_security_inspect")
            .unwrap_err();
    }

    // 5. If policy is "identifier" or "default", call identifier_inspect
    //    Filter words with is_identifier check (matching Python's .isidentifier())
    if matches!(policy, "identifier" | "default") {
        let words: Vec<String> = text
            .split_whitespace()
            .filter(|w| !w.is_empty())
            .filter(|w| {
                // Match Python's str.isidentifier() behavior:
                // Must start with underscore or letter, rest must be alphanumeric/underscore
                let chars: Vec<char> = w.chars().collect();
                if chars.is_empty() {
                    return false;
                }
                let first = chars[0];
                if first != '_' && !first.is_alphabetic() {
                    return false;
                }
                chars[1..].iter().all(|c| c.is_alphanumeric() || *c == '_')
            })
            .map(|w| w.to_string())
            .collect();
        if !words.is_empty() {
            let id_args = serde_json::json!({"identifiers": words});
            let id_result = crate::tools::identifier_inspect(&id_args);
            store_subresult(
                &mut subresults,
                "identifier_inspect",
                &id_result.result,
                &id_result.error,
            );
            if let Some(id_findings) = id_result.findings.as_deref() {
                if !id_findings.is_empty() {
                    for f in id_findings {
                        let code = f
                            .get("code")
                            .and_then(|v| v.as_str())
                            .unwrap_or("IDENTIFIER_RISK");
                        let raw_sev = f.get("severity").and_then(|v| v.as_str()).unwrap_or("warn");
                        let sev = match raw_sev {
                            "error" | "critical" => severity::HIGH,
                            "warn" | "warning" => severity::MEDIUM,
                            "danger" => severity::HIGH,
                            "info" => severity::INFO,
                            other => other,
                        };
                        let disp = match sev {
                            severity::HIGH => Some(disposition::BLOCKING),
                            severity::MEDIUM => Some(disposition::CAUTION),
                            _ => Some(disposition::INFORMATIONAL),
                        };
                        let msg = f.get("message").and_then(|v| v.as_str()).unwrap_or("");
                        all_findings.push(finding(code, sev, msg, disp, None));
                    }
                    if !code_list.contains(&machine_codes::IDENTIFIER_COLLISION_RISK.to_string()) {
                        code_list.push(machine_codes::IDENTIFIER_COLLISION_RISK.to_string());
                    }
                }
            }
        }
    }

    if budget_ctx.should_stop() {
        return budget_ctx
            .check_should_stop("text_security_inspect")
            .unwrap_err();
    }

    // 6. Determine verdict
    let has_error = all_findings
        .iter()
        .any(|f| f.get("severity").and_then(|v| v.as_str()) == Some(severity::HIGH));
    let has_warn = all_findings
        .iter()
        .any(|f| f.get("severity").and_then(|v| v.as_str()) == Some(severity::MEDIUM));
    let response_verdict = if has_error {
        verdict::BLOCK
    } else if has_warn {
        verdict::REVIEW
    } else {
        verdict::ALLOW
    };

    // Deduplicate machine codes
    let mut unique_machine_codes: Vec<String> = Vec::new();
    for mc in &code_list {
        if !unique_machine_codes.contains(mc) {
            unique_machine_codes.push(mc.clone());
        }
    }
    let primary_machine_code = if unique_machine_codes.is_empty() {
        machine_codes::TEXT_SECURITY_OK.to_string()
    } else {
        unique_machine_codes[0].clone()
    };

    // Build summary
    let n_findings = all_findings.len();
    let summary = if response_verdict == verdict::ALLOW {
        format!("No security issues found ({} findings).", n_findings)
    } else if response_verdict == verdict::REVIEW {
        format!(
            "Review recommended: {} finding(s) require attention.",
            n_findings
        )
    } else {
        format!("Block: {} finding(s) indicate security risk.", n_findings)
    };

    let recommended_action = if response_verdict == verdict::ALLOW {
        "allow".to_string()
    } else if response_verdict == verdict::REVIEW {
        "review content for hidden instructions".to_string()
    } else {
        "do not trust this text without manual inspection".to_string()
    };

    let next_tool = if response_verdict != verdict::ALLOW {
        Some(ToolResponse::next_tool(
            "text_diff_explain",
            &recommended_action,
            None,
        ))
    } else {
        None
    };

    let normalized_changed = subresults
        .get("canonicalize_text")
        .and_then(|v| v.get("changed"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let mut result = serde_json::json!({
        "verdict": response_verdict,
        "policy": policy,
        "findings": all_findings,
        "machine_code": primary_machine_code,
        "normalized_changed": normalized_changed,
        "recommended_action": recommended_action,
        "summary": summary,
    });

    // Only include subresults when detail is "normal" or "full" (matching Python)
    if detail == "normal" || detail == "full" {
        result["subresults"] = serde_json::Value::Object(subresults);
    }

    let mut resp = ToolResponse::success(result, Some("text_security_inspect"))
        .with_tool("text_security_inspect");
    resp = resp
        .with_machine_code(&primary_machine_code)
        .with_verdict(response_verdict);
    if !all_findings.is_empty() {
        resp = resp.with_findings(all_findings);
    }
    if let Some(nt) = next_tool {
        resp = resp.with_recommended_next_tool(nt);
    }
    resp
}

// ---------------------------------------------------------------------------
// escape_text
// ---------------------------------------------------------------------------

pub fn escape_text(args: &Value) -> ToolResponse {
    let text = match _require_str(args, "text", "escape_text") {
        Ok(s) => s,
        Err(e) => return *e,
    };
    let mode = match _require_str(args, "mode", "escape_text") {
        Ok(s) => s,
        Err(e) => return *e,
    };
    let detail = args
        .get("detail")
        .and_then(|v| v.as_str())
        .unwrap_or("normal");

    if !["summary", "normal", "full"].contains(&detail) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported detail level: {}", detail),
            Some(vec!["Use one of: summary, normal, full".to_string()]),
            Some("escape_text"),
        );
    }

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!(
                "text length {} exceeds MAX_TEXT_LENGTH {}",
                text.chars().count(),
                MAX_TEXT_LENGTH
            ),
            Some(vec![format!(
                "Maximum input length is {} characters",
                MAX_TEXT_LENGTH
            )]),
            Some("escape_text"),
        );
    }

    let valid_modes = [
        "html_text",
        "json_string",
        "markdown_code_block",
        "markdown_inline_code",
        "posix_shell_single",
        "python_string",
        "regex_literal",
        "rust_string",
        "url_component",
    ];
    if !valid_modes.contains(&mode) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported escape mode: {}", mode),
            Some(vec![format!("Valid modes: {}", valid_modes.join(", "))]),
            Some("escape_text"),
        );
    }

    match crate::text::escape_text(text, mode) {
        Ok(result) => {
            if detail == "summary" {
                ToolResponse::success(
                    serde_json::json!({
                        "mode": result.mode,
                        "changed": result.changed,
                        "summary": result.summary,
                    }),
                    Some("escape_text"),
                )
                .with_tool("escape_text")
            } else {
                ToolResponse::success(
                    serde_json::json!({
                        "mode": result.mode,
                        "escaped": result.escaped,
                        "changed": result.changed,
                        "summary": result.summary,
                    }),
                    Some("escape_text"),
                )
                .with_tool("escape_text")
            }
        }
        Err(e) => ToolResponse::error_with_code(
            "internal_error",
            machine_codes::INTERNAL_ERROR,
            &e,
            None,
            Some("escape_text"),
        ),
    }
}

// ---------------------------------------------------------------------------
// unescape_text
// ---------------------------------------------------------------------------

pub fn unescape_text(args: &Value) -> ToolResponse {
    let text = match _require_str(args, "text", "unescape_text") {
        Ok(s) => s,
        Err(e) => return *e,
    };
    let mode = match _require_str(args, "mode", "unescape_text") {
        Ok(s) => s,
        Err(e) => return *e,
    };
    let detail = args
        .get("detail")
        .and_then(|v| v.as_str())
        .unwrap_or("normal");

    if !["summary", "normal", "full"].contains(&detail) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported detail level: {}", detail),
            Some(vec!["Use one of: summary, normal, full".to_string()]),
            Some("unescape_text"),
        );
    }

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!(
                "text length {} exceeds MAX_TEXT_LENGTH {}",
                text.chars().count(),
                MAX_TEXT_LENGTH
            ),
            Some(vec![format!(
                "Maximum input length is {} characters",
                MAX_TEXT_LENGTH
            )]),
            Some("unescape_text"),
        );
    }

    let valid_modes = [
        "json_string",
        "python_string",
        "unicode_escape",
        "url_component",
    ];
    if !valid_modes.contains(&mode) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported unescape mode: {}", mode),
            Some(vec![format!("Valid modes: {}", valid_modes.join(", "))]),
            Some("unescape_text"),
        );
    }

    let result: UnescapeTextResult = crate::text::unescape_text(text, mode);
    if detail == "summary" {
        ToolResponse::success(
            serde_json::json!({
                "mode": result.mode,
                "changed": result.changed,
                "error": result.error,
                "summary": result.summary,
            }),
            Some("unescape_text"),
        )
        .with_tool("unescape_text")
    } else {
        ToolResponse::success(
            serde_json::json!({
                "mode": result.mode,
                "unescaped": result.unescaped,
                "changed": result.changed,
                "error": result.error,
                "summary": result.summary,
            }),
            Some("unescape_text"),
        )
        .with_tool("unescape_text")
    }
}

// ---------------------------------------------------------------------------
// line_range_extract_tool
// ---------------------------------------------------------------------------

pub fn line_range_extract_tool(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                &format!(
                    "text must be a string, got {}",
                    json_type_name(args.get("text").unwrap_or(&Value::Null))
                ),
                None,
                Some("line_range_extract"),
            )
        }
    };
    let start_line = match require_non_negative_int_arg(args, "start_line", "line_range_extract") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let end_line = match require_non_negative_int_arg(args, "end_line", "line_range_extract") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if let Err(response) = validate_line_range_order(start_line, end_line, "line_range_extract") {
        return *response;
    }
    let line_base = args.get("line_base").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
    let include_line_numbers = args
        .get("include_line_numbers")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let include_fingerprint = args
        .get("include_fingerprint")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let char_count = text.chars().count();
    if char_count > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!(
                "text length {} exceeds MAX_TEXT_LENGTH {}",
                char_count, MAX_TEXT_LENGTH
            ),
            Some(vec![format!(
                "Maximum input length is {} characters",
                MAX_TEXT_LENGTH
            )]),
            Some("line_range_extract"),
        );
    }

    match crate::text::line_range::line_range_extract(
        text,
        start_line,
        end_line,
        line_base,
        include_line_numbers,
        include_fingerprint,
    ) {
        Ok(result) => ToolResponse::success(
            serde_json::json!({
                "line_count_total": result.line_count_total,
                "start_line": result.start_line,
                "end_line": result.end_line,
                "valid_range": result.valid_range,
                "text": result.text,
                "lines": result.lines,
                "byte_start": result.byte_start,
                "byte_end": result.byte_end,
                "char_start": result.char_start,
                "char_end": result.char_end,
                "newline_style": result.newline_style,
                "ends_with_newline": result.ends_with_newline,
                "fingerprint": result.fingerprint,
                "findings": result.findings,
            }),
            Some("line_range_extract"),
        )
        .with_tool("line_range_extract"),
        Err(e) => ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &e,
            None,
            Some("line_range_extract"),
        ),
    }
}

// ---------------------------------------------------------------------------
// line_range_compare_tool
// ---------------------------------------------------------------------------

pub fn line_range_compare_tool(args: &Value) -> ToolResponse {
    let left_text = match args.get("left_text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                &format!(
                    "left_text must be a string, got {}",
                    json_type_name(args.get("left_text").unwrap_or(&Value::Null))
                ),
                None,
                Some("line_range_compare"),
            )
        }
    };
    let right_text = match args.get("right_text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                &format!(
                    "right_text must be a string, got {}",
                    json_type_name(args.get("right_text").unwrap_or(&Value::Null))
                ),
                None,
                Some("line_range_compare"),
            )
        }
    };
    let start_line = match require_non_negative_int_arg(args, "start_line", "line_range_compare") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let end_line = match require_non_negative_int_arg(args, "end_line", "line_range_compare") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if let Err(response) = validate_line_range_order(start_line, end_line, "line_range_compare") {
        return *response;
    }
    let line_base = args.get("line_base").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
    let comparison_mode = args
        .get("comparison_mode")
        .and_then(|v| v.as_str())
        .unwrap_or("exact");

    for (label, t) in [("left_text", left_text), ("right_text", right_text)] {
        let char_count = t.chars().count();
        if char_count > MAX_TEXT_LENGTH {
            return ToolResponse::error_with_code(
                "input_too_large",
                machine_codes::INPUT_TOO_LARGE,
                &format!(
                    "{} length {} exceeds MAX_TEXT_LENGTH {}",
                    label, char_count, MAX_TEXT_LENGTH
                ),
                Some(vec![format!(
                    "Maximum input length is {} characters",
                    MAX_TEXT_LENGTH
                )]),
                Some("line_range_compare"),
            );
        }
    }

    let valid_modes = ["exact", "ignore_trailing_whitespace", "normalize_newlines"];
    if !valid_modes.contains(&comparison_mode) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported comparison_mode: {}", comparison_mode),
            Some(vec![format!("Use one of: {}", valid_modes.join(", "))]),
            Some("line_range_compare"),
        );
    }

    match crate::text::line_range::line_range_compare(
        left_text,
        right_text,
        start_line,
        end_line,
        line_base,
        comparison_mode,
    ) {
        Ok(result) => ToolResponse::success(
            serde_json::json!({
                "equal": result.equal,
                "left_fingerprint": result.left_fingerprint,
                "right_fingerprint": result.right_fingerprint,
                "diff_summary": result.diff_summary,
                "first_difference": result.first_difference,
            }),
            Some("line_range_compare"),
        )
        .with_tool("line_range_compare"),
        Err(e) => ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &e,
            None,
            Some("line_range_compare"),
        ),
    }
}

// ---------------------------------------------------------------------------
// prompt_input_inspect_tool
// ---------------------------------------------------------------------------

pub fn prompt_input_inspect_tool(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'text' parameter",
                None,
                Some("prompt_input_inspect"),
            )
        }
    };

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!(
                "Input length {} exceeds MAX_TEXT_LENGTH {}",
                text.chars().count(),
                MAX_TEXT_LENGTH
            ),
            None,
            Some("prompt_input_inspect"),
        );
    }

    // Parse checks parameter
    let valid_check_names: &[&str] = &[
        "unicode_hidden",
        "bidi",
        "html_comments",
        "markdown_links",
        "ansi_escapes",
        "terminal_controls",
        "base64_like_blobs",
        "instruction_phrases",
        "long_minified_lines",
    ];
    let all_check_set: std::collections::HashSet<&str> =
        valid_check_names.iter().copied().collect();

    let active_checks: std::collections::HashSet<String> = match args.get("checks") {
        Some(Value::Array(arr)) => {
            let mut set = std::collections::HashSet::new();
            let mut invalid: Vec<&str> = Vec::new();
            for v in arr {
                if let Some(s) = v.as_str() {
                    if !all_check_set.contains(s) {
                        invalid.push(s);
                    } else {
                        set.insert(s.to_string());
                    }
                }
            }
            if !invalid.is_empty() {
                return ToolResponse::error_with_code(
                    "invalid_arguments",
                    machine_codes::INVALID_ARGUMENTS,
                    &format!("Unknown check(s): {}", invalid.join(", ")),
                    Some(vec![format!(
                        "Valid checks: {}",
                        valid_check_names.join(", ")
                    )]),
                    Some("prompt_input_inspect"),
                );
            }
            set
        }
        None => all_check_set.iter().map(|s| s.to_string()).collect(),
        _ => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "checks must be a list of strings",
                None,
                Some("prompt_input_inspect"),
            );
        }
    };

    // Parse phrase_patterns parameter
    let phrase_patterns: Option<Vec<String>> = match args.get("phrase_patterns") {
        Some(Value::Array(arr)) => {
            let patterns: Vec<String> = arr
                .iter()
                .map(|v| match v.as_str() {
                    Some(s) => s.to_string(),
                    None => v.to_string(),
                })
                .collect();
            if patterns.len() > MAX_LIST_ITEMS {
                return ToolResponse::error_with_code(
                    "input_too_large",
                    machine_codes::INPUT_TOO_LARGE,
                    &format!(
                        "phrase_patterns count {} exceeds MAX_LIST_ITEMS {}",
                        patterns.len(),
                        MAX_LIST_ITEMS
                    ),
                    None,
                    Some("prompt_input_inspect"),
                );
            }
            Some(patterns)
        }
        None => None,
        _ => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                &format!(
                    "phrase_patterns must be a list, got {}",
                    json_type_name(args.get("phrase_patterns").unwrap())
                ),
                None,
                Some("prompt_input_inspect"),
            );
        }
    };

    // Run checks
    let mut findings: Vec<serde_json::Value> = Vec::new();

    if active_checks.contains("unicode_hidden") {
        findings.extend(_pi_find_unicode_hidden(text));
    }
    if active_checks.contains("bidi") {
        findings.extend(_pi_find_bidi(text));
    }
    if active_checks.contains("html_comments") {
        findings.extend(_pi_find_html_comments(text));
    }
    if active_checks.contains("markdown_links") {
        findings.extend(_pi_find_markdown_links(text));
    }
    if active_checks.contains("ansi_escapes") {
        findings.extend(_pi_find_ansi_escapes(text));
    }
    if active_checks.contains("terminal_controls") {
        findings.extend(_pi_find_terminal_controls(text));
    }
    if active_checks.contains("base64_like_blobs") {
        findings.extend(_pi_find_base64_like_blobs(text));
    }
    if active_checks.contains("instruction_phrases") {
        findings.extend(_pi_find_instruction_phrases(
            text,
            phrase_patterns.as_deref(),
        ));
    }
    if active_checks.contains("long_minified_lines") {
        findings.extend(_pi_find_long_minified_lines(text));
    }

    // Deduplicate by (position, codepoint)
    let mut seen: std::collections::HashSet<(i64, String)> = std::collections::HashSet::new();
    let mut deduped: Vec<serde_json::Value> = Vec::new();
    for f in &findings {
        let pos = f
            .get("span")
            .and_then(|s| s.get("char_start"))
            .and_then(|v| v.as_i64())
            .unwrap_or(-1);
        let codepoint = f
            .get("details")
            .and_then(|d| d.get("codepoint"))
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| f.get("code").and_then(|v| v.as_str()).unwrap_or("UNKNOWN"));
        let key = (pos, codepoint.to_string());
        if seen.insert(key) {
            deduped.push(f.clone());
        }
    }
    findings = deduped;

    // Truncate if needed (sort by severity first so high-severity findings are kept)
    let findings_truncated = findings.len() > MAX_FINDINGS;
    if findings_truncated {
        let severity_order = |f: &serde_json::Value| -> u8 {
            match f.get("severity").and_then(|v| v.as_str()).unwrap_or("info") {
                "error" => 0,
                "warn" => 1,
                _ => 2,
            }
        };
        findings.sort_by_key(|f| severity_order(f));
        findings.truncate(MAX_FINDINGS);
    }

    let risk_score = _pi_compute_risk_score(&findings);
    let summary = _pi_build_summary(&findings, risk_score);
    let mut checks_run: Vec<String> = active_checks.iter().cloned().collect();
    checks_run.sort();

    let recommended_next_tool_json =
        _pi_recommend_next_tool(&findings).unwrap_or(serde_json::Value::Null);

    let result = serde_json::json!({
        "findings": findings.clone(),
        "summary": summary,
        "risk_score": risk_score,
        "recommended_next_tool": recommended_next_tool_json,
        "text_length": text.chars().count(),
        "checks_run": checks_run,
        "findings_truncated": findings_truncated,
    });

    // Determine machine_code and findings for envelope
    let has_findings = !findings.is_empty();
    let codes: std::collections::HashSet<String> = findings
        .iter()
        .filter_map(|f| f.get("code").and_then(|v| v.as_str()).map(String::from))
        .collect();

    let mut machine_code: Option<String> = None;
    if has_findings {
        if codes
            .iter()
            .any(|c| c == "HIDDEN_CHAR" || c == "BIDI_CONTROL" || c == "ANSI_ESCAPE")
        {
            machine_code = Some(machine_codes::PROMPT_HIDDEN_CONTENT.to_string());
        } else {
            machine_code = Some(machine_codes::PROMPT_HAS_FLAGS.to_string());
        }
    }

    let envelope_findings: Vec<serde_json::Value> = findings
        .iter()
        .map(|f| {
            serde_json::json!({
                "code": f.get("code").and_then(|v| v.as_str()).unwrap_or("UNKNOWN"),
                "severity": f.get("severity").and_then(|v| v.as_str()).unwrap_or("info"),
                "message": f.get("message").and_then(|v| v.as_str()).unwrap_or(""),
                "span": f.get("span"),
                "details": f.get("details"),
            })
        })
        .collect();

    let mut resp = ToolResponse::success(result.clone(), Some("prompt_input_inspect"))
        .with_tool("prompt_input_inspect");

    if !envelope_findings.is_empty() {
        resp = resp.with_findings(envelope_findings);
    }
    if let Some(code) = machine_code {
        resp = resp.with_machine_code(&code);
    }
    if let Some(rec) = _pi_recommend_next_tool(&findings) {
        resp = resp.with_recommended_next_tool(rec);
    }
    resp
}
