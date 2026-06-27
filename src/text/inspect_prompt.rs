use crate::text::primitives::byte_offset_to_char_index;
use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;

static MARKDOWN_LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[([^\]]{1,2000})\]\(([^)]{1,2000})\)").unwrap());

static HTML_COMMENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<!--(.*?)-->").unwrap());

static ANSI_ESCAPE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\x1b\[[0-9;]*[A-Za-z]").unwrap());

static TERMINAL_CONTROL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[\x00-\x08\x0e-\x1f\x7f]|\x1b[()][AB012]|\x1b[=>78]").unwrap());

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

fn _pi_char_span(index: usize, length: usize) -> Value {
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
        0x200B..=0x200F | 0x2060..=0x2069 | 0xFEFF => "Cf",
        0x2028 => "Zl",
        0x2029 => "Zp",
        0xFE00..=0xFE0F => "Cf",
        0xFFF0..=0xFFFC => "Cn",
        0xFFFD => "So",
        _ => "Lo",
    }
}

fn _pi_find_unicode_hidden(text: &str) -> Vec<Value> {
    let mut findings = Vec::new();
    for (i, c) in text.chars().enumerate() {
        let cp = c as u32;
        let (found, name, severity) = match cp {
            0x00..=0x08 | 0x0E..=0x1F => (true, "C0 CONTROL", "warn"),
            0x7F => (true, "DEL", "warn"),
            0x80..=0x9F => (true, "C1 CONTROL", "warn"),
            0x200B => (true, "ZERO WIDTH SPACE", "error"),
            0x200C => (true, "ZERO WIDTH NON-JOINER", "error"),
            0x200D => (true, "ZERO WIDTH JOINER", "error"),
            0x200E => (true, "LEFT-TO-RIGHT MARK", "warn"),
            0x200F => (true, "RIGHT-TO-LEFT MARK", "warn"),
            0x2028 => (true, "LINE SEPARATOR", "warn"),
            0x2029 => (true, "PARAGRAPH SEPARATOR", "warn"),
            0x2060 => (true, "WORD JOINER", "error"),
            0x2066..=0x2069 => (true, "INVISIBLE FORMAT", "warn"),
            0xFE00..=0xFE0F => (true, "VARIATION SELECTOR", "warn"),
            0xFFF0..=0xFFFD => (true, "SPECIALS", "warn"),
            0xFEFF => (true, "BOM/ZWNBSP", "warn"),
            _ => (false, "", "info"),
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

fn _pi_find_bidi(text: &str) -> Vec<Value> {
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

fn _pi_find_html_comments(text: &str) -> Vec<Value> {
    let mut findings = Vec::new();
    for m in HTML_COMMENT_RE.captures_iter(text) {
        let full_match = m.get(0).unwrap();
        let content = m
            .get(1)
            .map(|c| c.as_str().trim().to_string())
            .unwrap_or_default();
        let severity = if content.is_empty() { "info" } else { "warn" };
        let truncated = if content.chars().count() > 100 {
            format!("{}...", content.chars().take(100).collect::<String>())
        } else {
            content.clone()
        };
        let char_start =
            byte_offset_to_char_index(text, full_match.start()).unwrap_or(text.chars().count());
        let char_end =
            byte_offset_to_char_index(text, full_match.end()).unwrap_or(text.chars().count());
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

fn _pi_find_markdown_links(text: &str) -> Vec<Value> {
    let mut findings = Vec::new();
    for m in MARKDOWN_LINK_RE.captures_iter(text) {
        let full_match = m.get(0).unwrap();
        let link_text = m.get(1).map(|c| c.as_str()).unwrap_or("");
        let link_target = m.get(2).map(|c| c.as_str()).unwrap_or("");
        let severity;
        let mut details = serde_json::json!({
            "text": link_text,
            "target": link_target,
        });

        let label = link_text.trim();
        let target = link_target.trim();
        if label == target {
            details["mismatch"] = serde_json::json!(false);
            severity = "info";
        } else {
            details["mismatch"] = serde_json::json!(true);
            if link_target.starts_with("http://") || link_target.starts_with("https://") {
                details["kind"] = serde_json::json!("external");
                severity = "warn";
            } else if link_target.starts_with('#') {
                details["kind"] = serde_json::json!("anchor");
                severity = "warn";
            } else {
                details["kind"] = serde_json::json!("relative");
                severity = "info";
            }
        }

        let char_start =
            byte_offset_to_char_index(text, full_match.start()).unwrap_or(text.chars().count());
        let char_end =
            byte_offset_to_char_index(text, full_match.end()).unwrap_or(text.chars().count());
        findings.push(serde_json::json!({
            "code": "MARKDOWN_LINK",
            "severity": severity,
            "message": format!("Markdown link at position {}: label='{}', target='{}'",
                char_start, link_text, link_target),
            "span": {"char_start": char_start, "char_end": char_end},
            "details": details,
        }));
    }
    findings
}

fn _pi_find_ansi_escapes(text: &str) -> Vec<Value> {
    let mut findings = Vec::new();
    for m in ANSI_ESCAPE_RE.find_iter(text) {
        let char_start = byte_offset_to_char_index(text, m.start()).unwrap_or(text.chars().count());
        let char_end = byte_offset_to_char_index(text, m.end()).unwrap_or(text.chars().count());
        findings.push(serde_json::json!({
            "code": "ANSI_ESCAPE",
            "severity": "warn",
            "message": format!("ANSI escape sequence at position {} (length {})", char_start, char_end - char_start),
            "span": {"char_start": char_start, "char_end": char_end},
            "details": {
                "sequence": m.as_str().to_string(),
                "length": char_end - char_start,
            },
        }));
    }
    findings
}

fn _pi_find_terminal_controls(text: &str) -> Vec<Value> {
    let mut findings = Vec::new();
    for m in TERMINAL_CONTROL_RE.find_iter(text) {
        let char_start = byte_offset_to_char_index(text, m.start()).unwrap_or(text.chars().count());
        let char_end = byte_offset_to_char_index(text, m.end()).unwrap_or(text.chars().count());
        findings.push(serde_json::json!({
            "code": "TERMINAL_CONTROL",
            "severity": "warn",
            "message": format!("Terminal control character at position {} (length {})", char_start, char_end - char_start),
            "span": {"char_start": char_start, "char_end": char_end},
            "details": {
                "sequence": m.as_str().to_string(),
                "byte_value": m.as_str().as_bytes().first().copied().unwrap_or(0),
            },
        }));
    }
    findings
}

fn _pi_find_base64_like_blobs(text: &str) -> Vec<Value> {
    let mut findings = Vec::new();
    let re = Regex::new(r"[A-Za-z0-9+/]{40,}(?:=){0,2}").unwrap();
    for m in re.find_iter(text) {
        let blob = m.as_str();
        let entropy_estimate: f64 = {
            let mut char_counts = std::collections::HashMap::new();
            let total = blob.len().max(1);
            for b in blob.bytes() {
                *char_counts.entry(b).or_insert(0) += 1;
            }
            -char_counts
                .values()
                .map(|&c| {
                    let p = c as f64 / total as f64;
                    p * p.log2()
                })
                .sum::<f64>()
        };
        if entropy_estimate > 4.2 {
            let truncated = if blob.len() > 80 {
                format!("{}...", &blob[..80])
            } else {
                blob.to_string()
            };
            findings.push(serde_json::json!({
                "code": "BASE64_LIKE_BLOB",
                "severity": "warn",
                "message": format!("Potential base64 blob at position {} ({} chars)", byte_offset_to_char_index(text, m.start()).unwrap_or(text.chars().count()), blob.len()),
                "span": {"char_start": byte_offset_to_char_index(text, m.start()).unwrap_or(text.chars().count()), "char_end": byte_offset_to_char_index(text, m.end()).unwrap_or(text.chars().count())},
                "details": {
                    "preview": truncated,
                    "length": blob.len(),
                    "entropy": entropy_estimate,
                },
            }));
        }
    }
    findings
}

fn _pi_find_long_minified_lines(text: &str) -> Vec<Value> {
    let mut findings = Vec::new();
    for (line_idx, line) in text.lines().enumerate() {
        if line.chars().count() > 1000 {
            let preview = if line.chars().count() > 200 {
                format!("{}...", line.chars().take(200).collect::<String>())
            } else {
                line.to_string()
            };
            findings.push(serde_json::json!({
                "code": "LONG_MINIFIED_LINE",
                "severity": "info",
                "message": format!("Long line {}: {} chars", line_idx + 1, line.chars().count()),
                "span": {"line": line_idx + 1},
                "details": {
                    "length": line.chars().count(),
                    "preview": preview,
                },
            }));
        }
    }
    findings
}

fn _pi_find_instruction_phrases(text: &str, phrase_patterns: Option<&[String]>) -> Vec<Value> {
    let phrases: Vec<String> = if let Some(patterns) = phrase_patterns {
        patterns.iter().cloned().collect()
    } else {
        DEFAULT_INSTRUCTION_PHRASES
            .iter()
            .map(|s| s.to_string())
            .collect()
    };
    let mut findings = Vec::new();
    for phrase in &phrases {
        let pattern = format!("(?i){}", regex::escape(phrase));
        let re = Regex::new(&pattern).unwrap();
        if let Some(m) = re.find(text) {
            let char_start =
                byte_offset_to_char_index(text, m.start()).unwrap_or(text.chars().count());
            let char_end = byte_offset_to_char_index(text, m.end()).unwrap_or(text.chars().count());
            findings.push(serde_json::json!({
                "code": "INSTRUCTION_PHRASE",
                "severity": "warn",
                "message": format!("Instruction-like phrase '{}' at position {}", phrase, char_start),
                "span": {"char_start": char_start, "char_end": char_end},
                "details": {
                    "phrase": phrase,
                    "matched_lowercase": true,
                },
            }));
        }
    }
    findings
}

fn _pi_compute_risk_score(findings: &[Value]) -> i64 {
    let mut score: i64 = 0;
    for f in findings {
        match f.get("severity").and_then(|v| v.as_str()).unwrap_or("info") {
            "error" => score += 5,
            "warn" => score += 3,
            _ => score += 1,
        }
    }
    score
}

fn _pi_build_summary(findings: &[Value], risk_score: i64) -> String {
    if findings.is_empty() {
        return "No red flags detected.".to_string();
    }
    let error_count = findings
        .iter()
        .filter(|f| f.get("severity").and_then(|v| v.as_str()) == Some("error"))
        .count();
    let warn_count = findings
        .iter()
        .filter(|f| f.get("severity").and_then(|v| v.as_str()) == Some("warn"))
        .count();
    let info_count = findings
        .iter()
        .filter(|f| f.get("severity").and_then(|v| v.as_str()) == Some("info"))
        .count();
    format!(
        "Risk score: {}/100. {} error(s), {} warning(s), {} info(s).",
        risk_score, error_count, warn_count, info_count
    )
}

fn _pi_recommend_next_tool(findings: &[Value]) -> Option<String> {
    for f in findings {
        let code = f.get("code").and_then(|v| v.as_str()).unwrap_or("");
        match code {
            "HIDDEN_CHAR" | "BIDI_CONTROL" => {
                return Some("text_inspect".to_string());
            }
            "ANSI_ESCAPE" | "TERMINAL_CONTROL" => {
                return Some("text_transform".to_string());
            }
            _ => {}
        }
    }
    for f in findings {
        if f.get("code").and_then(|v| v.as_str()) == Some("MARKDOWN_LINK") {
            return Some("markdown_structure".to_string());
        }
    }
    None
}

/// Inspect text for prompt injection red flags.
/// Returns findings with code, severity, message, span, and details.
pub fn prompt_input_inspect(
    text: &str,
    checks: Option<&[String]>,
    phrase_patterns: Option<&[String]>,
) -> PromptInspectResult {
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

    let active_checks: Vec<String> = match checks {
        Some(list) if !list.is_empty() => list.to_vec(),
        _ => valid_check_names.iter().map(|s| s.to_string()).collect(),
    };

    let mut findings: Vec<Value> = Vec::new();

    if active_checks.iter().any(|c| c == "unicode_hidden") {
        findings.extend(_pi_find_unicode_hidden(text));
    }
    if active_checks.iter().any(|c| c == "bidi") {
        findings.extend(_pi_find_bidi(text));
    }
    if active_checks.iter().any(|c| c == "html_comments") {
        findings.extend(_pi_find_html_comments(text));
    }
    if active_checks.iter().any(|c| c == "markdown_links") {
        findings.extend(_pi_find_markdown_links(text));
    }
    if active_checks.iter().any(|c| c == "ansi_escapes") {
        findings.extend(_pi_find_ansi_escapes(text));
    }
    if active_checks.iter().any(|c| c == "instruction_phrases") {
        findings.extend(_pi_find_instruction_phrases(text, phrase_patterns));
    }
    if active_checks.iter().any(|c| c == "terminal_controls") {
        findings.extend(_pi_find_terminal_controls(text));
    }
    if active_checks.iter().any(|c| c == "base64_like_blobs") {
        findings.extend(_pi_find_base64_like_blobs(text));
    }
    if active_checks.iter().any(|c| c == "long_minified_lines") {
        findings.extend(_pi_find_long_minified_lines(text));
    }

    // Deduplicate by (position, codepoint)
    let mut seen: std::collections::HashSet<(i64, String)> = std::collections::HashSet::new();
    let mut deduped: Vec<Value> = Vec::new();
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

    // Truncate if needed
    let findings_truncated = findings.len() > MAX_FINDINGS;
    if findings_truncated {
        let severity_order = |f: &Value| -> u8 {
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
    let recommended_next_tool = _pi_recommend_next_tool(&findings);
    let mut checks_run: Vec<String> = active_checks.iter().cloned().collect();
    checks_run.sort();

    PromptInspectResult {
        findings,
        summary,
        risk_score,
        recommended_next_tool,
        findings_truncated,
        text_length: text.chars().count(),
        checks_run,
    }
}

pub struct PromptInspectResult {
    pub findings: Vec<Value>,
    pub summary: String,
    pub risk_score: i64,
    pub recommended_next_tool: Option<String>,
    pub findings_truncated: bool,
    pub text_length: usize,
    pub checks_run: Vec<String>,
}
