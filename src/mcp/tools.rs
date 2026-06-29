use crate::text::primitives::byte_offset_to_char_index;
use regex::Regex;
use serde::Serialize;
use std::sync::{mpsc, Condvar, LazyLock};
use std::time::Duration;
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

/// Run a closure on a dedicated thread with a timeout.
/// Returns `Ok(T)` if the closure completes within `timeout`,
/// or `Err(())` on timeout (the background thread continues to completion).
fn run_with_timeout<T: Send + 'static>(
    timeout: Duration,
    f: impl FnOnce() -> T + Send + 'static,
) -> Result<T, ()> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(timeout).map_err(|_| ())
}

use crate::calc::units::{
    convert_temperature, get_conversion_factor, get_unit_info, is_unit, PHYSICAL_CONSTANTS,
};
use crate::calc::{run, RunError};
use crate::mcp::schemas::ToolResponse;
use crate::text::glob::glob_match;
use crate::text::measure::{char_category_metrics, word_metrics};
use crate::text::position::{TextPositionResult, TextWindowPosition, TextWindowResult};
use crate::text::transform::{
    TextFingerprintResult, TextHashResult, TextTransformResult, UnescapeTextResult,
};
use crate::text::unicode_tools::{
    detect_mixed_scripts as unicode_detect_mixed_scripts, detect_newline_style,
    find_invisibles as unicode_find_invisibles,
};
use crate::text::{
    count_graphemes, path::path_normalize, regex_safety_check, text_fingerprint,
    CheckBracketsResult, ValidateJsonResult,
};
use serde_json::Value;

const MAX_TEXT_LENGTH: usize = 100_000;
const MAX_INSPECT_ITEMS: usize = 100;
const MAX_LIST_ITEMS: usize = 10_000;
const MAX_REGEX_SAMPLES: usize = 100;
const MAX_REGEX_SAMPLE_LENGTH: usize = 10_000;
const MAX_MATCHES_REGEX: usize = 100;
const MAX_MATCHES_HARD_CAP: usize = 1000;
const MAX_PATTERN_LENGTH: usize = 1000;
const MAX_SCHEMA_DEPTH: usize = 32;
const MAX_SCHEMA_ELEMENTS: usize = 10_000;
const REGEX_TIMEOUT_SECONDS: u64 = 5;
const MAX_CONCURRENT_SPAWNED: usize = 16;
const SPAWN_ACQUIRE_TIMEOUT: u64 = 10; // seconds to wait for a spawn slot
const MAX_EXPRESSION_LENGTH: usize = 10_000;

/// Split text into lines, treating `\r`, `\n`, `\r\n`, and Unicode line separators
/// as line separators. Matches Python's `str.splitlines()` behavior for standard
/// line endings, including returning no lines for an empty string.
fn split_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }

    let mut result = Vec::new();
    let mut current = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\n' => {
                result.push(std::mem::take(&mut current));
            }
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                result.push(std::mem::take(&mut current));
            }
            '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' => {
                result.push(std::mem::take(&mut current));
            }
            '\u{0085}' | '\u{2028}' | '\u{2029}' => {
                result.push(std::mem::take(&mut current));
            }
            _ => {
                current.push(c);
            }
        }
    }
    result.push(current);
    result
}

// Concurrency limiter for spawned threads (matching Python's _SPAWN_SEMAPHORE)
struct SpawnState {
    count: usize,
}

struct SpawnSemaphore {
    state: std::sync::Mutex<SpawnState>,
    cvar: Condvar,
}

static SPAWN_SEMAPHORE: LazyLock<SpawnSemaphore> = LazyLock::new(|| SpawnSemaphore {
    state: std::sync::Mutex::new(SpawnState { count: 0 }),
    cvar: Condvar::new(),
});

struct SpawnPermit {
    _private: (),
}

impl Drop for SpawnPermit {
    fn drop(&mut self) {
        if let Ok(mut s) = SPAWN_SEMAPHORE.state.lock() {
            s.count = s.count.saturating_sub(1);
        }
        SPAWN_SEMAPHORE.cvar.notify_one();
    }
}

fn try_acquire_spawn_permit() -> Option<SpawnPermit> {
    let mut state = SPAWN_SEMAPHORE.state.lock().ok()?;
    let deadline =
        std::time::Instant::now() + std::time::Duration::from_secs(SPAWN_ACQUIRE_TIMEOUT);
    loop {
        if state.count < MAX_CONCURRENT_SPAWNED {
            state.count += 1;
            return Some(SpawnPermit { _private: () });
        }
        let now = std::time::Instant::now();
        if now >= deadline {
            return None;
        }
        state = SPAWN_SEMAPHORE
            .cvar
            .wait_timeout(state, deadline - now)
            .ok()?
            .0;
    }
}

fn _require_str<'a>(
    args: &'a Value,
    field: &str,
    tool: &str,
) -> Result<&'a str, Box<ToolResponse>> {
    match args.get(field) {
        Some(v) => match v.as_str() {
            Some(s) => {
                let codepoint_len = s.chars().count();
                if codepoint_len > MAX_TEXT_LENGTH {
                    return Err(Box::new(ToolResponse::error(
                        "input_too_large",
                        &format!(
                            "{} length {} exceeds {}",
                            field, codepoint_len, MAX_TEXT_LENGTH
                        ),
                        None,
                        Some(tool),
                    )));
                }
                Ok(s)
            }
            None => Err(Box::new(ToolResponse::error(
                "invalid_arguments",
                &format!("{} must be a string, got {}", field, json_type_name(v)),
                None,
                Some(tool),
            ))),
        },
        None => Err(Box::new(ToolResponse::error(
            "invalid_arguments",
            &format!("{} must be a string, got NoneType", field),
            None,
            Some(tool),
        ))),
    }
}

pub(crate) fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "NoneType",
        Value::Bool(_) => "bool",
        Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                "int"
            } else {
                "float"
            }
        }
        Value::String(_) => "str",
        Value::Array(_) => "list",
        Value::Object(_) => "dict",
    }
}

fn unicode_casefold(s: &str) -> String {
    caseless::default_case_fold_str(s)
}

pub fn math_eval(args: &Value) -> ToolResponse {
    let expression = match _require_str(args, "expression", "math_eval") {
        Ok(s) => s,
        Err(e) => return *e,
    };

    if expression.chars().count() > MAX_EXPRESSION_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!(
                "Expression length {} exceeds maximum {}",
                expression.chars().count(),
                MAX_EXPRESSION_LENGTH
            ),
            Some(vec!["Try a shorter expression".to_string()]),
            Some("math_eval"),
        );
    }

    // Detect true division (/) so we can match Python's float-division semantics.
    // Floor division (//) is excluded. Power (**) is handled by the evaluator.
    let has_true_division = contains_true_division(expression);

    // Run evaluation on a dedicated thread with timeout.
    // Using std::thread + mpsc avoids the deadlock the old tokio-based
    // handle.block_on/spawn_blocking pattern caused under concurrent load.
    let _permit = match try_acquire_spawn_permit() {
        Some(p) => p,
        None => {
            return ToolResponse::error(
                "timeout",
                &format!(
                    "Could not acquire spawn slot after {}s (all {} slots busy)",
                    SPAWN_ACQUIRE_TIMEOUT, MAX_CONCURRENT_SPAWNED
                ),
                None,
                Some("math_eval"),
            )
        }
    };
    let expr_owned = expression.to_string();
    let eval_result = match run_with_timeout(Duration::from_secs(30), move || run(&expr_owned)) {
        Ok(r) => r,
        Err(_timeout) => {
            return ToolResponse::error(
                "timeout",
                "Expression evaluation timed out after 30 seconds",
                Some(vec!["Try a simpler expression".to_string()]),
                Some("math_eval"),
            )
        }
    };

    match eval_result {
        Ok((result, result_type)) => {
            // Parse unit from result string (e.g., "60.48 m" -> value="60.48", unit="m")
            let (value_str, unit, display) = if let Some(space_pos) = result.rfind(' ') {
                let numeric_part = &result[..space_pos];
                let unit_part = &result[space_pos + 1..];
                // Check if numeric_part is a valid number
                if numeric_part.parse::<f64>().is_ok() {
                    (
                        numeric_part.to_string(),
                        Some(unit_part.to_string()),
                        Some(result.clone()),
                    )
                } else {
                    (result.clone(), None, None)
                }
            } else {
                (result.clone(), None, None)
            };

            let obj = if let Some(ref u) = unit {
                let numeric_type = if let Ok(_int_val) = value_str.parse::<i64>() {
                    // Check if the original string represents an integer
                    if value_str.contains('.') || value_str.contains('e') || value_str.contains('E')
                    {
                        "float"
                    } else {
                        "int"
                    }
                } else {
                    "float"
                };
                serde_json::json!({
                    "value": value_str,
                    "type": numeric_type,
                    "unit": u,
                    "display": display,
                })
            } else {
                // Python parity: '/' is true division, so the result is always a float.
                // If the input contained '/' and the evaluator produced an int-formatted
                // result (e.g., "25" for 100/4), promote to float ("25.0").
                let (out_value, out_type) = if has_true_division && result_type == "int" {
                    (format!("{}.0", value_str), "float".to_string())
                } else {
                    (value_str, result_type.to_string())
                };
                serde_json::json!({"value": out_value, "type": out_type})
            };
            ToolResponse::success(obj, Some("math_eval"))
        }
        Err(e) => {
            let (error_type, suggestions) = match &e {
                RunError::Evaluation(_) => (
                    "evaluation_error",
                    Some(vec!["Check expression syntax".to_string()]),
                ),
                RunError::Internal(_) => (
                    "internal_error",
                    Some(vec!["Check expression syntax".to_string()]),
                ),
            };
            ToolResponse::error(error_type, &e.to_string(), suggestions, Some("math_eval"))
        }
    }
}

/// Detects true division (`/`) in the input expression while excluding
/// floor division (`//`) and power (`**`). The check scans the raw
/// characters so it works before any normalization runs.
fn contains_true_division(expr: &str) -> bool {
    let bytes = expr.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'/' {
            let prev_is_slash = i > 0 && bytes[i - 1] == b'/';
            let next_is_slash = i + 1 < bytes.len() && bytes[i + 1] == b'/';
            if !prev_is_slash && !next_is_slash {
                return true;
            }
        }
        i += 1;
    }
    false
}

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
        return ToolResponse::error(
            "input_too_large",
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("text_measure"),
        );
    }

    let valid_details = ["summary", "normal", "full"];
    if !valid_details.contains(&detail) {
        return ToolResponse::error(
            "invalid_arguments",
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
        return ToolResponse::error(
            "invalid_arguments",
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

fn common_prefix_len(a: &str, b: &str) -> usize {
    let mut count = 0;
    for (ca, cb) in a.chars().zip(b.chars()) {
        if ca != cb {
            break;
        }
        count += 1;
    }
    count
}

fn common_suffix_len(a: &str, b: &str) -> usize {
    let mut count = 0;
    for (ca, cb) in a.chars().rev().zip(b.chars().rev()) {
        if ca != cb {
            break;
        }
        count += 1;
    }
    count
}

fn classify_difference(
    raw_equal: bool,
    nfc_equal: bool,
    casefold_equal: bool,
    byte_equal: bool,
    length_diff: bool,
    invisibles_detected: bool,
) -> String {
    if raw_equal {
        return "exact_match".to_string();
    }
    if nfc_equal {
        if byte_equal {
            return "exact_match".to_string();
        }
        if !casefold_equal {
            return "accent_or_diacritic_difference".to_string();
        }
        return "unicode_normalization_only".to_string();
    }
    if casefold_equal {
        return "case_only".to_string();
    }
    if length_diff {
        return "length_only".to_string();
    }
    if invisibles_detected {
        return "invisible_character".to_string();
    }
    "ordinary_text_difference".to_string()
}

fn generate_agent_instruction(
    classification: &str,
    raw_equal: bool,
    _nfc_equal: bool,
    byte_equal: bool,
) -> String {
    if raw_equal {
        return "Strings are identical.".to_string();
    }
    match classification {
        "unicode_normalization_only" => "Treat these strings as equivalent only if NFC normalization is acceptable. They are not byte-identical.".to_string(),
        "case_only" => "Strings differ only by case. Case-insensitive comparison should treat them as equal.".to_string(),
        "accent_or_diacritic_difference" => "Strings differ by accents or diacritics only (same letters, different marks). NFC normalization will make them equal.".to_string(),
        "compatibility_normalization_only" => "Strings differ in compatibility normalization (NFKC). Treat as equivalent if compatibility normalization is acceptable.".to_string(),
        _ if !byte_equal => "Strings are not byte-identical and differ in Unicode normalization. Choose appropriate normalization for your use case.".to_string(),
        _ => "Strings differ. Review diff details for specifics.".to_string(),
    }
}

fn is_invisible_char(c: char) -> bool {
    let cp = c as u32;
    matches!(cp,
        0x200b | 0x200c | 0x200d | 0x200e | 0x200f |
        0xfeff | 0x00a0 | 0x2028 | 0x2029 |
        0x2060 | 0x00ad | 0x180e | 0x034f |
        0x202a..=0x202e | 0x2066..=0x2069 |
        0xfe00..=0xfe0f
    )
}

pub fn text_diff_explain(args: &Value) -> ToolResponse {
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
        return ToolResponse::error(
            "invalid_arguments",
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
        return ToolResponse::error(
            "invalid_arguments",
            &format!("max_diffs {} exceeds 10000", max_diffs),
            None,
            Some("text_diff_explain"),
        );
    }

    if a.chars().count() > MAX_TEXT_LENGTH || b.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Input exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("text_diff_explain"),
        );
    }

    let valid_details = ["summary", "normal", "full"];
    if !valid_details.contains(&detail) {
        return ToolResponse::error(
            "invalid_arguments",
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

fn invisible_display_name(c: char) -> &'static str {
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
        _ => "CTRL",
    }
}

fn bidi_display_name(c: char) -> &'static str {
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

fn unicode_name_char(c: char) -> String {
    if let Some(name) = unicode_names2::name(c) {
        return name.to_string();
    }
    match c as u32 {
        0x200b => "ZERO WIDTH SPACE".to_string(),
        0x200c => "ZERO WIDTH NON-JOINER".to_string(),
        0x200d => "ZERO WIDTH JOINER".to_string(),
        0x200e => "LEFT-TO-RIGHT MARK".to_string(),
        0x200f => "RIGHT-TO-LEFT MARK".to_string(),
        0xfeff => "ZERO WIDTH NO-BREAK SPACE".to_string(),
        0x00a0 => "NO-BREAK SPACE".to_string(),
        0x2028 => "LINE SEPARATOR".to_string(),
        0x2029 => "PARAGRAPH SEPARATOR".to_string(),
        0x2060 => "WORD JOINER".to_string(),
        0x00ad => "SOFT HYPHEN".to_string(),
        0x180e => "MONGOLIAN VOWEL SEPARATOR".to_string(),
        0x034f => "COMBINING GRAPHEME JOINER".to_string(),
        0x202a => "LEFT-TO-RIGHT EMBEDDING".to_string(),
        0x202b => "RIGHT-TO-LEFT EMBEDDING".to_string(),
        0x202c => "POP DIRECTIONAL FORMATTING".to_string(),
        0x202d => "LEFT-TO-RIGHT OVERRIDE".to_string(),
        0x202e => "RIGHT-TO-LEFT OVERRIDE".to_string(),
        0x2066 => "LEFT-TO-RIGHT ISOLATE".to_string(),
        0x2067 => "RIGHT-TO-LEFT ISOLATE".to_string(),
        0x2068 => "FIRST STRONG ISOLATE".to_string(),
        0x2069 => "POP DIRECTIONAL ISOLATE".to_string(),
        _ => format!("U+{:04X}", c as u32),
    }
}

fn is_combining_mark(c: char) -> bool {
    let cp = c as u32;
    matches!(cp,
        0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF |
        0x20D0..=0x20FF | 0xFE20..=0xFE2F
    )
}

fn build_safe_repr(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            ' ' => result.push('\u{2420}'),
            '\t' => result.push('\u{2409}'),
            '\n' => result.push('\u{240A}'),
            '\r' => result.push('\u{240D}'),
            _ if matches!(
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
            ) =>
            {
                let display = invisible_display_name(c);
                result.push_str(&format!("\u{27E6}{}\u{27E7}", display));
            }
            _ if (0xfe00..=0xfe0f).contains(&(c as u32)) => {
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
            _ => result.push(c),
        }
    }
    result
}

fn apply_detail_limit(arr: &[serde_json::Value], max_items: usize) -> Vec<serde_json::Value> {
    if arr.len() > max_items {
        arr.iter().take(max_items).cloned().collect()
    } else {
        arr.to_vec()
    }
}

fn inspect_max_items(detail: &str) -> usize {
    if detail == "summary" {
        10
    } else {
        MAX_INSPECT_ITEMS
    }
}

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
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported detail level: {}", detail),
            Some(vec![format!("Use one of: {}", valid_details.join(", "))]),
            Some("text_inspect"),
        );
    }

    let valid_norms = ["none", "NFC", "NFD", "NFKC", "NFKD"];
    if !valid_norms.contains(&normalize_form) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported normalization form: {}", normalize_form),
            Some(vec![format!("Use one of: {}", valid_norms.join(", "))]),
            Some("text_inspect"),
        );
    }

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
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
            machine_code = Some("CONFUSABLES_DETECTED".to_string());
        } else if codes.contains("BIDI_CONTROL") {
            machine_code = Some("BIDI_DETECTED".to_string());
        } else if codes.contains("INVISIBLE_CHAR") {
            machine_code = Some("INVISIBLES_DETECTED".to_string());
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

pub fn text_count(args: &Value) -> ToolResponse {
    let text = match _require_str(args, "text", "text_count") {
        Ok(s) => s,
        Err(e) => return *e,
    };
    let target = match args.get("target") {
        Some(v) => match v.as_str() {
            Some(s) => Some(s),
            None => {
                return ToolResponse::error(
                    "invalid_arguments",
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
        return ToolResponse::error(
            "input_too_large",
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("text_count"),
        );
    }

    let valid_count_modes = ["codepoint", "grapheme", "byte", "substring"];
    if !valid_count_modes.contains(&count_mode) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported count_mode: {}", count_mode),
            Some(vec![format!(
                "Use one of: {}",
                valid_count_modes.join(", ")
            )]),
            Some("text_count"),
        );
    }

    if !["raw", "NFC", "NFKC"].contains(&normalization) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported normalization form: {}", normalization),
            Some(vec![format!("Use one of: raw, NFC, NFKC")]),
            Some("text_count"),
        );
    }

    let mut work_text = text.to_string();
    if normalization != "raw" {
        work_text = match normalization {
            "NFKC" => work_text.nfkc().collect::<String>(),
            _ => work_text.nfc().collect::<String>(),
        };
    }

    if let Some(target) = target {
        // Validate target length
        const MAX_TARGET_LENGTH: usize = 1000;
        if target.chars().count() > MAX_TARGET_LENGTH {
            return ToolResponse::error(
                "input_too_large",
                &format!(
                    "target length {} exceeds {}",
                    target.chars().count(),
                    MAX_TARGET_LENGTH
                ),
                None,
                Some("text_count"),
            );
        }

        // Validate target cardinality based on count_mode
        match count_mode {
            "codepoint" => {
                if target.chars().count() != 1 {
                    return ToolResponse::error(
                        "invalid_arguments",
                        "target must be a single codepoint for count_mode='codepoint'",
                        Some(vec!["Provide a single-codepoint target".to_string()]),
                        Some("text_count"),
                    );
                }
            }
            "grapheme" => {
                if count_graphemes(target) != 1 {
                    return ToolResponse::error(
                        "invalid_arguments",
                        "target must be a single grapheme for count_mode='grapheme'",
                        Some(vec!["Provide a single-grapheme target".to_string()]),
                        Some("text_count"),
                    );
                }
            }
            "byte" if (target.len() != 1 || !target.is_ascii()) => {
                return ToolResponse::error(
                    "invalid_arguments",
                    "target must be a single byte for count_mode='byte'",
                    Some(vec!["Provide a single-byte target".to_string()]),
                    Some("text_count"),
                );
            }
            _ => {}
        }

        let work_target = match normalization {
            "NFKC" => target.nfkc().collect::<String>(),
            "NFC" => target.nfc().collect::<String>(),
            _ => target.to_string(),
        };

        let count = match count_mode {
            "byte" => {
                if work_target.is_empty() {
                    0
                } else {
                    let text_bytes = work_text.as_bytes();
                    let target_bytes = work_target.as_bytes();
                    let tw = target_bytes.len();
                    if tw == 0 {
                        0
                    } else {
                        text_bytes
                            .windows(tw)
                            .filter(|w| *w == target_bytes)
                            .count()
                    }
                }
            }
            "codepoint" => {
                if work_target.is_empty() {
                    0
                } else {
                    let target_char = work_target.chars().next().unwrap();
                    work_text.chars().filter(|c| *c == target_char).count()
                }
            }
            "grapheme" => {
                if work_target.is_empty() {
                    0
                } else {
                    work_text
                        .graphemes(true)
                        .filter(|g| *g == work_target.as_str())
                        .count()
                }
            }
            "substring" => work_text.matches(&work_target).count(),
            _ => 0,
        };

        // Collect positions
        let positions: Vec<usize> = match count_mode {
            "byte" => {
                if work_target.is_empty() {
                    vec![]
                } else {
                    let text_bytes = work_text.as_bytes();
                    let target_bytes = work_target.as_bytes();
                    let tw = target_bytes.len();
                    if tw == 0 {
                        vec![]
                    } else {
                        text_bytes
                            .windows(tw)
                            .enumerate()
                            .filter(|(_, w)| *w == target_bytes)
                            .map(|(i, _)| i)
                            .collect()
                    }
                }
            }
            "codepoint" => {
                if work_target.is_empty() {
                    vec![]
                } else {
                    let target_char = work_target.chars().next().unwrap();
                    work_text
                        .chars()
                        .enumerate()
                        .filter(|(_, c)| *c == target_char)
                        .map(|(i, _)| i)
                        .collect()
                }
            }
            "grapheme" => {
                if work_target.is_empty() {
                    vec![]
                } else {
                    work_text
                        .grapheme_indices(true)
                        .filter(|(_, g)| *g == work_target.as_str())
                        .map(|(i, _)| i)
                        .collect()
                }
            }
            "substring" => work_text
                .match_indices(&work_target)
                .map(|(i, _)| i)
                .collect(),
            _ => vec![],
        };

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

pub fn validate_brackets(args: &Value) -> ToolResponse {
    let text = match _require_str(args, "text", "validate_brackets") {
        Ok(s) => s,
        Err(e) => return *e,
    };
    let pairs_val = args.get("pairs");

    // Build pairs map from args or use defaults
    let pairs: std::collections::HashMap<char, char> = if let Some(pairs_obj) =
        pairs_val.and_then(|v| v.as_object())
    {
        // Validate pairs: max 64 entries, keys/values must be strings, length <= 16
        const MAX_PAIRS: usize = 64;
        const MAX_PAIR_LEN: usize = 16;
        if pairs_obj.len() > MAX_PAIRS {
            return ToolResponse::error(
                "input_too_large",
                &format!(
                    "pairs dict length {} exceeds maximum of 64",
                    pairs_obj.len()
                ),
                None,
                Some("validate_brackets"),
            );
        }
        let mut map = std::collections::HashMap::new();
        for (k, v) in pairs_obj {
            let val_str = match v.as_str() {
                Some(s) => s,
                None => {
                    return ToolResponse::error(
                        "invalid_arguments",
                        &format!(
                            "pairs keys and values must be strings, got String -> {}",
                            json_type_name(v)
                        ),
                        None,
                        Some("validate_brackets"),
                    );
                }
            };
            if k.len() > MAX_PAIR_LEN || val_str.len() > MAX_PAIR_LEN {
                return ToolResponse::error(
                    "invalid_arguments",
                    &format!(
                        "pairs key/value length must be <= 16, got {}/{}",
                        k.len(),
                        val_str.len()
                    ),
                    None,
                    Some("validate_brackets"),
                );
            }
            if k.chars().count() > 1 || val_str.chars().count() > 1 {
                return ToolResponse::error(
                        "invalid_arguments",
                        &format!(
                            "pairs key/value must be single characters, got key '{}' ({} chars) and value '{}' ({} chars)",
                            k, k.chars().count(), val_str, val_str.chars().count()
                        ),
                        None,
                        Some("validate_brackets"),
                    );
            }
            if let (Some(key_ch), Some(val_ch)) = (k.chars().next(), val_str.chars().next()) {
                map.insert(key_ch, val_ch);
            }
        }
        map
    } else if let Some(v) = pairs_val {
        // pairs was provided but is not a dict — return type error like Python
        return ToolResponse::error(
            "invalid_arguments",
            &format!("pairs must be a dict or None, got {}", json_type_name(v)),
            None,
            Some("validate_brackets"),
        );
    } else {
        [('(', ')'), ('[', ']'), ('{', '}'), ('<', '>')]
            .iter()
            .cloned()
            .collect()
    };

    match crate::text::validate_brackets_with_pairs(text, &pairs) {
        Ok(result) => {
            let result: CheckBracketsResult = result;
            ToolResponse::success(
                serde_json::json!({
                    "balanced": result.balanced,
                    "unmatched_openers": result.unmatched_openers,
                    "unmatched_closers": result.unmatched_closers,
                }),
                Some("validate_brackets"),
            )
            .with_tool("validate_brackets")
        }
        Err(e) => ToolResponse::error("invalid_arguments", &e, None, Some("validate_brackets")),
    }
}

pub fn validate_json(args: &Value) -> ToolResponse {
    let text = match _require_str(args, "text", "validate_json") {
        Ok(s) => s,
        Err(e) => return *e,
    };

    match crate::text::validate_json(text) {
        Ok(result) => {
            let result: ValidateJsonResult = result;
            let findings = if !result.valid {
                let error_msg = result
                    .error
                    .clone()
                    .unwrap_or_else(|| "Invalid JSON".to_string());
                let span = if result.line.is_some() || result.column.is_some() {
                    let mut s = serde_json::json!({});
                    if let Some(line) = result.line {
                        s["line"] = serde_json::json!(line);
                    }
                    if let Some(col) = result.column {
                        s["column"] = serde_json::json!(col);
                    }
                    s
                } else {
                    serde_json::Value::Null
                };
                vec![serde_json::json!({
                    "code": "JSON_PARSE_ERROR",
                    "severity": "error",
                    "message": error_msg,
                    "span": span,
                    "details": {"position": result.position},
                })]
            } else {
                vec![]
            };
            let machine_code = if !result.valid {
                Some("JSON_INVALID".to_string())
            } else {
                None
            };
            let mut resp = ToolResponse::success(
                serde_json::json!({
                    "valid": result.valid,
                    "error": result.error,
                    "line": result.line,
                    "column": result.column,
                    "position": result.position,
                    "type": result.json_type,
                    "top_level_keys": result.top_level_keys,
                }),
                Some("validate_json"),
            )
            .with_tool("validate_json");
            if !findings.is_empty() {
                resp = resp.with_findings(findings);
            }
            if let Some(code) = machine_code {
                resp = resp.with_machine_code(&code);
            }
            resp
        }
        Err(e) => ToolResponse::error("invalid_arguments", &e, None, Some("validate_json")),
    }
}

pub fn validate_regex(args: &Value) -> ToolResponse {
    let pattern = match args.get("pattern") {
        Some(v) => match v.as_str() {
            Some(s) => s,
            None => {
                return ToolResponse::error(
                    "invalid_arguments",
                    &format!("pattern must be a string, got {}", json_type_name(v)),
                    None,
                    Some("validate_regex"),
                )
            }
        },
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "pattern must be a string, got NoneType",
                None,
                Some("validate_regex"),
            )
        }
    };
    let samples = match args.get("samples") {
        Some(Value::Array(arr)) => arr,
        Some(v) => {
            return ToolResponse::error(
                "invalid_arguments",
                &format!("samples must be a list, got {}", json_type_name(v)),
                None,
                Some("validate_regex"),
            )
        }
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "samples must be a list, got NoneType",
                None,
                Some("validate_regex"),
            )
        }
    };
    let flags = match args.get("flags") {
        Some(Value::Array(arr)) => {
            // Validate all flags are strings
            let non_str_flags: Vec<usize> = arr
                .iter()
                .enumerate()
                .filter(|(_, v)| !v.is_string())
                .map(|(i, _)| i)
                .collect();
            if !non_str_flags.is_empty() {
                return ToolResponse::error(
                    "invalid_arguments",
                    "All flags must be strings",
                    Some(vec![format!(
                        "Non-string items at indices: {:?}",
                        &non_str_flags[..5.min(non_str_flags.len())]
                    )]),
                    Some("validate_regex"),
                );
            }
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        }
        Some(v) => {
            return ToolResponse::error(
                "invalid_arguments",
                &format!("flags must be a list, got {}", json_type_name(v)),
                None,
                Some("validate_regex"),
            )
        }
        None => Vec::new(),
    };
    let ignore_case = args
        .get("ignore_case")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let multiline = args
        .get("multiline")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let dotall = args
        .get("dotall")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let ascii = args.get("ascii").and_then(|v| v.as_bool()).unwrap_or(false);

    if samples.len() > MAX_REGEX_SAMPLES {
        return ToolResponse::error(
            "input_too_large",
            &format!(
                "Number of samples {} exceeds MAX_REGEX_SAMPLES {}",
                samples.len(),
                MAX_REGEX_SAMPLES
            ),
            Some(vec![format!(
                "Maximum {} samples allowed",
                MAX_REGEX_SAMPLES
            )]),
            Some("validate_regex"),
        );
    }

    if pattern.chars().count() > MAX_PATTERN_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!(
                "Pattern length {} exceeds MAX_PATTERN_LENGTH {}",
                pattern.chars().count(),
                MAX_PATTERN_LENGTH
            ),
            Some(vec![format!(
                "Maximum pattern length is {} characters",
                MAX_PATTERN_LENGTH
            )]),
            Some("validate_regex"),
        );
    }

    let mut total_chars: usize = 0;
    let non_str_indices: Vec<usize> = samples
        .iter()
        .enumerate()
        .filter(|(_, v)| !v.is_string())
        .map(|(i, _)| i)
        .collect();
    if !non_str_indices.is_empty() {
        return ToolResponse::error(
            "invalid_arguments",
            "All samples must be strings",
            Some(vec![format!(
                "Non-string items at indices: {:?}",
                &non_str_indices[..5.min(non_str_indices.len())]
            )]),
            Some("validate_regex"),
        );
    }
    let long_samples: Vec<usize> = samples
        .iter()
        .enumerate()
        .filter(|(_, v)| {
            v.as_str()
                .is_some_and(|s| s.chars().count() > MAX_REGEX_SAMPLE_LENGTH)
        })
        .map(|(i, _)| i)
        .collect();
    if !long_samples.is_empty() {
        return ToolResponse::error(
            "input_too_large",
            &format!(
                "Sample(s) at indices {:?} exceed MAX_REGEX_SAMPLE_LENGTH {}",
                long_samples, MAX_REGEX_SAMPLE_LENGTH
            ),
            Some(vec![format!(
                "Maximum {} characters per sample",
                MAX_REGEX_SAMPLE_LENGTH
            )]),
            Some("validate_regex"),
        );
    }
    let sample_strs: Vec<String> = samples
        .iter()
        .map(|v| {
            let s = v.as_str().unwrap_or("");
            total_chars += s.chars().count();
            s.to_string()
        })
        .collect();

    if total_chars > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!(
                "Total sample size {} characters exceeds MAX_TEXT_LENGTH {}",
                total_chars, MAX_TEXT_LENGTH
            ),
            Some(vec![format!(
                "Maximum total {} characters across all samples",
                MAX_TEXT_LENGTH
            )]),
            Some("validate_regex"),
        );
    }

    let safety = regex_safety_check(pattern);
    if safety.risk == "medium" || safety.risk == "high" {
        return ToolResponse::error(
            "unsafe_pattern",
            &format!(
                "Pattern has {} risk of catastrophic backtracking",
                safety.risk
            ),
            Some(vec![
                "Try a simpler pattern or break it into smaller parts".to_string(),
                "Use the regex_safety_check tool for detailed analysis and suggestions".to_string(),
            ]),
            Some("validate_regex"),
        );
    }

    let flags_clone: Option<Vec<String>> = if flags.is_empty() { None } else { Some(flags) };
    let pattern_owned = pattern.to_string();
    let samples_owned: Vec<String> = sample_strs;
    let result = match run_with_timeout(Duration::from_secs(REGEX_TIMEOUT_SECONDS), move || {
        let refs: Vec<&str> = samples_owned.iter().map(|s| s.as_str()).collect();
        crate::text::regex_test(
            &pattern_owned,
            &refs,
            flags_clone.as_ref(),
            ignore_case,
            multiline,
            dotall,
            ascii,
        )
    }) {
        Ok(r) => r,
        Err(_timeout) => {
            return ToolResponse::error(
                "timeout",
                "Regex execution exceeded time limit (possible ReDoS)",
                Some(vec!["Try a simpler pattern or fewer samples".to_string()]),
                Some("validate_regex"),
            )
        }
    };

    let flags_used = serde_json::json!({
        "ignore_case": ignore_case,
        "multiline": multiline,
        "dotall": dotall,
        "ascii": ascii,
    });

    let mut result_value = serde_json::json!({
        "valid_pattern": result.valid_pattern,
        "results": result.results,
        "flags_used": flags_used,
    });
    if let Some(ref err) = result.error {
        result_value["error"] = serde_json::json!(err);
    }

    ToolResponse::success(result_value, Some("validate_regex")).with_tool("validate_regex")
}

pub fn list_compare(args: &Value) -> ToolResponse {
    let a = match args.get("a") {
        Some(v) => match v.as_array() {
            Some(arr) => arr,
            None => {
                return ToolResponse::error(
                    "invalid_arguments",
                    &format!(
                        "a and b must be lists, got {} and {}",
                        json_type_name(v),
                        args.get("b")
                            .map(|bv| json_type_name(bv))
                            .unwrap_or("NoneType")
                    ),
                    None,
                    Some("list_compare"),
                )
            }
        },
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "a and b must be lists, got NoneType and NoneType",
                None,
                Some("list_compare"),
            )
        }
    };
    let b = match args.get("b") {
        Some(v) => match v.as_array() {
            Some(arr) => arr,
            None => {
                return ToolResponse::error(
                    "invalid_arguments",
                    &format!(
                        "a and b must be lists, got {} and {}",
                        json_type_name(args.get("a").unwrap()),
                        json_type_name(v)
                    ),
                    None,
                    Some("list_compare"),
                )
            }
        },
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "a and b must be lists, got NoneType and NoneType",
                None,
                Some("list_compare"),
            )
        }
    };

    if a.len() > MAX_LIST_ITEMS || b.len() > MAX_LIST_ITEMS {
        return ToolResponse::error(
            "input_too_large",
            &format!("List length exceeds MAX_LIST_ITEMS {}", MAX_LIST_ITEMS),
            Some(vec![format!("Maximum {} items per list", MAX_LIST_ITEMS)]),
            Some("list_compare"),
        );
    }

    // Validate all elements are strings
    let mut total_chars = 0usize;
    let mut errors: Vec<String> = Vec::new();
    for (i, item) in a.iter().enumerate() {
        if !item.is_string() {
            errors.push(format!("[{}] is {}, not string", i, json_type_name(item)));
        } else {
            total_chars += item.as_str().unwrap_or("").chars().count();
        }
    }
    for (i, item) in b.iter().enumerate() {
        if !item.is_string() {
            errors.push(format!("[{}] is {}, not string", i, json_type_name(item)));
        } else {
            total_chars += item.as_str().unwrap_or("").chars().count();
        }
    }
    if !errors.is_empty() {
        return ToolResponse::error(
            "invalid_arguments",
            "All list elements must be strings",
            Some(errors.into_iter().take(10).collect()),
            Some("list_compare"),
        );
    }

    let max_total_chars = MAX_TEXT_LENGTH * 2;
    if total_chars > max_total_chars {
        return ToolResponse::error(
            "input_too_large",
            &format!("Total string length {} exceeds maximum", total_chars),
            Some(vec![format!(
                "Maximum combined string length is {} characters",
                max_total_chars
            )]),
            Some("list_compare"),
        );
    }

    let mode = args.get("mode").and_then(|v| v.as_str()).unwrap_or("set");
    let casefold = args
        .get("casefold")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let normalization = args
        .get("normalization")
        .and_then(|v| v.as_str())
        .unwrap_or("NFC");
    let trim = args.get("trim").and_then(|v| v.as_bool()).unwrap_or(false);
    let include_near_matches = args
        .get("include_near_matches")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let near_match_threshold = args
        .get("near_match_threshold")
        .and_then(|v| v.as_f64())
        .unwrap_or(2.0);
    // Match Python: ignore_order defaults to mode != "ordered" when not provided
    let _ignore_order = args
        .get("ignore_order")
        .and_then(|v| v.as_bool())
        .unwrap_or_else(|| mode != "ordered");
    // Match Python: treat_as_multiset defaults to mode == "multiset" when not provided
    let treat_as_multiset = args
        .get("treat_as_multiset")
        .and_then(|v| v.as_bool())
        .unwrap_or_else(|| mode == "multiset");

    let valid_modes = ["ordered", "set", "multiset"];
    if !valid_modes.contains(&mode) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported mode: {}", mode),
            Some(vec![format!("Use one of: {}", valid_modes.join(", "))]),
            Some("list_compare"),
        );
    }

    let valid_normalizations = ["raw", "NFC", "NFD", "NFKC", "NFKD"];
    if !valid_normalizations.contains(&normalization) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported normalization form: {}", normalization),
            Some(vec![format!(
                "Use one of: {}",
                valid_normalizations.join(", ")
            )]),
            Some("list_compare"),
        );
    }

    if near_match_threshold < 0.0 {
        return ToolResponse::error(
            "invalid_arguments",
            &format!(
                "near_match_threshold must be non-negative, got {}",
                near_match_threshold
            ),
            Some(vec!["Set near_match_threshold to 0 or higher".to_string()]),
            Some("list_compare"),
        );
    }

    let treat_as_multiset_val = if mode == "multiset" {
        true
    } else {
        treat_as_multiset
    };
    let ignore_order_val = if let Some(v) = args.get("ignore_order").and_then(|v| v.as_bool()) {
        v
    } else {
        mode != "ordered"
    };

    let transform = |v: &Value| -> String {
        let mut result = match v.as_str() {
            Some(s) => s.to_string(),
            None => v.to_string(),
        };
        if trim {
            result = result.trim().to_string();
        }
        if normalization != "raw" {
            result = match normalization {
                "NFC" => result.nfc().to_string(),
                "NFD" => result.nfd().to_string(),
                "NFKC" => result.nfkc().to_string(),
                "NFKD" => result.nfkd().to_string(),
                _ => result,
            };
        }
        if casefold {
            result = unicode_casefold(&result);
        }
        result
    };

    let a_transformed: Vec<String> = a.iter().map(&transform).collect();
    let b_transformed: Vec<String> = b.iter().map(&transform).collect();

    use std::collections::HashMap;
    let mut a_counts: HashMap<String, usize> = HashMap::new();
    let mut b_counts: HashMap<String, usize> = HashMap::new();
    for x in &a_transformed {
        *a_counts.entry(x.clone()).or_insert(0) += 1;
    }
    for x in &b_transformed {
        *b_counts.entry(x.clone()).or_insert(0) += 1;
    }

    let a_set: std::collections::HashSet<String> = a_transformed.iter().cloned().collect();
    let b_set: std::collections::HashSet<String> = b_transformed.iter().cloned().collect();

    // In set mode, only_in_a/only_in_b use set membership (items not in other set at all)
    // In multiset mode, use count comparison (items where count_a > count_b)
    let only_a_orig: Vec<Value> = if treat_as_multiset_val {
        // multiset: use count comparison - items where a count > b count
        a.iter()
            .enumerate()
            .filter(|(i, _)| {
                let t = &a_transformed[*i];
                a_counts.get(t).copied().unwrap_or(0) > b_counts.get(t).copied().unwrap_or(0)
            })
            .map(|(_, v)| v.clone())
            .collect()
    } else {
        // set: use set membership - items not present in b at all
        a.iter()
            .filter(|v| !b_set.contains(&transform(v)))
            .cloned()
            .collect()
    };
    let only_b_orig: Vec<Value> = if treat_as_multiset_val {
        // multiset: use count comparison - items where b count > a count
        b.iter()
            .enumerate()
            .filter(|(i, _)| {
                let t = &b_transformed[*i];
                b_counts.get(t).copied().unwrap_or(0) > a_counts.get(t).copied().unwrap_or(0)
            })
            .map(|(_, v)| v.clone())
            .collect()
    } else {
        // set: use set membership - items not present in a at all
        b.iter()
            .filter(|v| !a_set.contains(&transform(v)))
            .cloned()
            .collect()
    };

    let duplicates_a: Vec<String>;
    let duplicates_b: Vec<String>;
    {
        duplicates_a = a_counts
            .iter()
            .filter(|(_, c)| **c > 1)
            .map(|(k, _)| k.clone())
            .collect();
        duplicates_b = b_counts
            .iter()
            .filter(|(_, c)| **c > 1)
            .map(|(k, _)| k.clone())
            .collect();
    }

    let mut near_matches: Vec<serde_json::Value> = Vec::new();
    if include_near_matches && near_match_threshold > 0.0 {
        let threshold_int = near_match_threshold.round() as usize;
        if threshold_int > 0 {
            let mut seen_pairs: std::collections::HashSet<(String, String)> =
                std::collections::HashSet::new();
            for (i, a_item) in a.iter().enumerate() {
                let a_t = &a_transformed[i];
                for (j, b_item) in b.iter().enumerate() {
                    let b_t = &b_transformed[j];
                    if a_t == b_t {
                        continue;
                    }
                    let dist = crate::text::levenshtein_distance(a_t, b_t);
                    if dist > 0 && dist <= threshold_int {
                        let a_str = a_item.as_str().unwrap_or("");
                        let b_str = b_item.as_str().unwrap_or("");
                        let pair = if a_str <= b_str {
                            (a_str.to_string(), b_str.to_string())
                        } else {
                            (b_str.to_string(), a_str.to_string())
                        };
                        if !seen_pairs.contains(&pair) {
                            seen_pairs.insert(pair);
                            near_matches.push(serde_json::json!({
                                "a": a_item,
                                "b": b_item,
                                "distance": dist,
                                "classification": "fuzzy"
                            }));
                        }
                        break;
                    }
                }
            }
        }
    }

    let same_ordered = ignore_order_val || (a_transformed == b_transformed);

    let same_unordered = if treat_as_multiset_val {
        a_counts == b_counts
    } else {
        a_set == b_set
    };

    let equal = match mode {
        "ordered" => same_ordered,
        "set" => same_unordered && only_a_orig.is_empty() && only_b_orig.is_empty(),
        _ => same_unordered,
    };

    let only_in_a: Vec<Value> = only_a_orig.to_vec();
    let only_in_b: Vec<Value> = only_b_orig.to_vec();

    if mode == "ordered" {
        let mut aligned: Vec<serde_json::Value> = Vec::new();
        let max_len = std::cmp::max(a.len(), b.len());
        let mut first_diff_index: Option<usize> = None;

        for i in 0..max_len {
            let a_item = if i < a.len() {
                Some(a[i].clone())
            } else {
                None
            };
            let b_item = if i < b.len() {
                Some(b[i].clone())
            } else {
                None
            };

            let op = match (&a_item, &b_item) {
                (Some(_), None) => "delete",
                (None, Some(_)) => "insert",
                (Some(a_val), Some(b_val)) if a_transformed.get(i) == b_transformed.get(i) => {
                    "equal"
                }
                _ => "replace",
            };

            if first_diff_index.is_none() && op != "equal" {
                first_diff_index = Some(i);
            }

            let mut entry = serde_json::json!({"op": op});
            if let Some(ref v) = a_item {
                entry["a"] = v.clone();
                entry["a_index"] = Value::from(i);
            }
            if let Some(ref v) = b_item {
                entry["b"] = v.clone();
                entry["b_index"] = Value::from(i);
            }
            aligned.push(entry);
        }

        let equal_prefix_length = first_diff_index.unwrap_or(a.len());

        ToolResponse::success(
            serde_json::json!({
                "equal": equal,
                "first_diff_index": first_diff_index,
                "equal_prefix_length": equal_prefix_length,
                "aligned": aligned,
                "only_in_a": only_in_a,
                "only_in_b": only_in_b,
                "missing_in_a": only_in_b,
                "missing_in_b": only_in_a,
                "duplicates_in_a": duplicates_a,
                "duplicates_in_b": duplicates_b,
                "near_matches": near_matches,
            }),
            Some("list_compare"),
        )
        .with_tool("list_compare")
    } else if mode == "set" {
        ToolResponse::success(
            serde_json::json!({
                "equal": equal,
                "only_in_a": only_in_a,
                "only_in_b": only_in_b,
                "missing_in_a": only_in_b,
                "missing_in_b": only_in_a,
                "duplicates_in_a": duplicates_a,
            "duplicates_in_b": duplicates_b,
            "near_matches": near_matches,
            }),
            Some("list_compare"),
        )
        .with_tool("list_compare")
    } else {
        let mut count_deltas: serde_json::Map<String, Value> = serde_json::Map::new();
        let all_keys: std::collections::HashSet<String> =
            a_counts.keys().chain(b_counts.keys()).cloned().collect();
        for k in all_keys {
            let delta =
                *a_counts.get(&k).unwrap_or(&0) as i64 - *b_counts.get(&k).unwrap_or(&0) as i64;
            if delta != 0 {
                count_deltas.insert(k, Value::Number(delta.into()));
            }
        }

        ToolResponse::success(
            serde_json::json!({
                "equal": equal,
                "count_deltas": Value::Object(count_deltas),
                "missing_in_a": only_in_b,
                "missing_in_b": only_in_a,
                "duplicates_in_a": duplicates_a,
                "duplicates_in_b": duplicates_b,
                "only_in_a": only_in_a,
                "only_in_b": only_in_b,
                "near_matches": near_matches,
            }),
            Some("list_compare"),
        )
        .with_tool("list_compare")
    }
}

pub fn text_truncate(args: &Value) -> ToolResponse {
    let text = match args.get("text") {
        Some(v) => match v.as_str() {
            Some(s) => s,
            None => {
                return ToolResponse::error(
                    "invalid_arguments",
                    &format!("text must be a string, got {}", json_type_name(v)),
                    None,
                    Some("text_truncate"),
                )
            }
        },
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "text must be a string, got NoneType",
                None,
                Some("text_truncate"),
            )
        }
    };
    let max_graphemes = match args.get("max_graphemes").and_then(|v| v.as_i64()) {
        Some(n) => n,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'max_graphemes' parameter",
                None,
                Some("text_truncate"),
            )
        }
    };

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
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
        return ToolResponse::error(
            "invalid_arguments",
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

pub fn unit_convert(args: &Value) -> ToolResponse {
    // Reject booleans explicitly (Python rejects booleans as value)
    if let Some(Value::Bool(_)) = args.get("value") {
        return ToolResponse::error(
            "invalid_arguments",
            &format!(
                "value must be a finite number, got {}",
                json_type_name(args.get("value").unwrap())
            ),
            None,
            Some("unit_convert"),
        );
    }
    let value = match args.get("value").and_then(|v| v.as_f64()) {
        Some(v) => v,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'value' parameter",
                None,
                Some("unit_convert"),
            )
        }
    };
    // Reject NaN and Infinity
    if !value.is_finite() {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Value must be a finite number, got {}", value),
            None,
            Some("unit_convert"),
        );
    }
    let from_unit = match args.get("from_unit").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'from_unit' parameter",
                None,
                Some("unit_convert"),
            )
        }
    };
    let to_unit = match args.get("to_unit").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'to_unit' parameter",
                None,
                Some("unit_convert"),
            )
        }
    };

    if !is_unit(from_unit) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unknown unit: {}", from_unit),
            None,
            Some("unit_convert"),
        );
    }
    if !is_unit(to_unit) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unknown unit: {}", to_unit),
            None,
            Some("unit_convert"),
        );
    }

    // Cross-category rejection (matching Python behavior)
    let from_info = get_unit_info(from_unit);
    let to_info = get_unit_info(to_unit);

    // If either unit is not recognized by get_unit_info(), return an error
    // instead of silently skipping the cross-category check.
    let ((_, from_cat), (_, to_cat)) = match (&from_info, &to_info) {
        (Some(f), Some(t)) => (f, t),
        _ => {
            return ToolResponse::error(
                "conversion_error",
                &format!(
                    "Cannot determine category for unit(s): from='{}', to='{}'",
                    from_unit, to_unit
                ),
                None,
                Some("unit_convert"),
            );
        }
    };
    if from_cat != to_cat {
        return ToolResponse::error(
            "conversion_error",
            &format!(
                "Cannot convert between incompatible categories: {} ({}) -> {} ({})",
                from_cat, from_unit, to_cat, to_unit
            ),
            None,
            Some("unit_convert"),
        );
    }

    // Temperature special-case: offset-based conversions (matching Python behavior)
    if *from_cat == "temperature" && *to_cat == "temperature" {
        match convert_temperature(value, from_unit, to_unit) {
            Ok(result) => {
                if !result.is_finite() {
                    return ToolResponse::error(
                        "conversion_error",
                        &format!("Conversion result is not finite: {}", result),
                        None,
                        Some("unit_convert"),
                    );
                }
                return ToolResponse::success(
                    serde_json::json!({
                        "value": result,
                        "from_unit": from_unit,
                        "to_unit": to_unit,
                        "factor": null,
                    }),
                    Some("unit_convert"),
                )
                .with_tool("unit_convert");
            }
            Err(e) => {
                return ToolResponse::error("conversion_error", &e, None, Some("unit_convert"));
            }
        }
    }

    match get_conversion_factor(from_unit, to_unit) {
        Ok(factor) => {
            let result = value * factor;
            if !result.is_finite() {
                return ToolResponse::error(
                    "conversion_error",
                    &format!("Conversion result is not finite: {}", result),
                    None,
                    Some("unit_convert"),
                );
            }
            ToolResponse::success(
                serde_json::json!({
                    "value": result,
                    "from_unit": from_unit,
                    "to_unit": to_unit,
                    "factor": factor,
                }),
                Some("unit_convert"),
            )
            .with_tool("unit_convert")
        }
        Err(e) => ToolResponse::error("conversion_error", &e, None, Some("unit_convert")),
    }
}

pub fn unit_info(args: &Value) -> ToolResponse {
    let unit = match args.get("unit").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'unit' parameter",
                None,
                Some("unit_info"),
            )
        }
    };

    if let Some((canonical, category)) = get_unit_info(unit) {
        ToolResponse::success(
            serde_json::json!({
                "unit": unit,
                "canonical": canonical,
                "category": category,
                "is_valid": true,
            }),
            Some("unit_info"),
        )
        .with_tool("unit_info")
    } else {
        ToolResponse::error(
            "invalid_arguments",
            &format!("Unknown unit: {}", unit),
            None,
            Some("unit_info"),
        )
    }
}

pub fn constant_lookup(args: &Value) -> ToolResponse {
    let name = match args.get("name").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'name' parameter",
                None,
                Some("constant_lookup"),
            )
        }
    };

    // Always lowercase first to match Python behavior (key = name.lower()).
    let constant = PHYSICAL_CONSTANTS.get(name.to_lowercase().as_str());
    if let Some(constant) = constant {
        ToolResponse::success(
            serde_json::json!({
                "name": name,
                "value": constant.value,
                "symbol": constant.symbol,
                "display_name": constant.display_name,
            }),
            Some("constant_lookup"),
        )
        .with_tool("constant_lookup")
    } else {
        ToolResponse::error(
            "invalid_arguments",
            &format!("Unknown constant: {}", name),
            None,
            Some("constant_lookup"),
        )
    }
}

pub fn path_normalize_tool(args: &Value) -> ToolResponse {
    let path = match args.get("path").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'path' parameter",
                None,
                Some("path_normalize"),
            )
        }
    };
    let platform = args
        .get("platform")
        .and_then(|v| v.as_str())
        .unwrap_or("posix");
    let collapse_dot_segments = args
        .get("collapse_dot_segments")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let preserve_trailing_separator = args
        .get("preserve_trailing_separator")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let valid_platforms = ["posix", "windows"];
    if !valid_platforms.contains(&platform) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported platform: {}", platform),
            Some(vec![format!("Use one of: {}", valid_platforms.join(", "))]),
            Some("path_normalize"),
        );
    }

    let result = path_normalize(
        path,
        platform,
        collapse_dot_segments,
        preserve_trailing_separator,
    );
    ToolResponse::success(
        serde_json::json!({
            "normalized": result.normalized,
            "is_absolute": result.is_absolute,
            "components": result.components,
            "warnings": result.warnings,
        }),
        Some("path_normalize"),
    )
    .with_tool("path_normalize")
}

pub fn text_fingerprint_tool(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'text' parameter",
                None,
                Some("text_fingerprint"),
            )
        }
    };
    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
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
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported unicode normalization: {}", unicode),
            Some(vec![format!("Use one of: {}", valid_unicode.join(", "))]),
            Some("text_fingerprint"),
        );
    }

    let valid_newline = ["raw", "LF"];
    if !valid_newline.contains(&newline) {
        return ToolResponse::error(
            "invalid_arguments",
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

pub fn validate_toml_tool(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'text' parameter",
                None,
                Some("validate_toml"),
            )
        }
    };
    let detail = args
        .get("detail")
        .and_then(|v| v.as_str())
        .unwrap_or("normal");

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("validate_toml"),
        );
    }

    let valid_details = ["summary", "normal", "full"];
    if !valid_details.contains(&detail) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported detail level: {}", detail),
            Some(vec![format!("Use one of: {}", valid_details.join(", "))]),
            Some("validate_toml"),
        );
    }

    match crate::text::toml::validate_toml(text) {
        Ok(result) => {
            if detail == "summary" {
                ToolResponse::success(
                    serde_json::json!({
                        "valid": result.valid,
                        "error": result.error,
                    }),
                    Some("validate_toml"),
                )
                .with_tool("validate_toml")
            } else {
                ToolResponse::success(
                    serde_json::json!({
                        "valid": result.valid,
                        "error": result.error,
                        "line": result.line,
                        "column": result.column,
                        "position": result.position,
                        "type": result.toml_type,
                        "top_level_keys": result.top_level_keys,
                        "tables": result.tables,
                    }),
                    Some("validate_toml"),
                )
                .with_tool("validate_toml")
            }
        }
        Err(e) => ToolResponse::error("invalid_arguments", &e, None, Some("validate_toml")),
    }
}

pub fn toml_shape_tool(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'text' parameter",
                None,
                Some("toml_shape"),
            )
        }
    };
    let max_tables = match args.get("max_tables") {
        Some(v) => {
            if v.is_boolean() || !v.is_number() {
                return ToolResponse::error(
                    "invalid_arguments",
                    &format!(
                        "max_tables must be an integer, got {}",
                        match v {
                            Value::Bool(_) => "bool",
                            Value::Null => "null",
                            Value::String(_) =>
                                return ToolResponse::error(
                                    "invalid_arguments",
                                    "max_tables must be an integer, got string",
                                    None,
                                    Some("toml_shape")
                                ),
                            Value::Array(_) => "array",
                            Value::Object(_) => "object",
                            _ => "unknown",
                        }
                    ),
                    None,
                    Some("toml_shape"),
                );
            }
            if v.as_i64().unwrap_or(0) < 0 {
                return ToolResponse::error(
                    "invalid_arguments",
                    "max_tables must be a non-negative integer",
                    None,
                    Some("toml_shape"),
                );
            }
            v.as_u64().unwrap_or(100) as usize
        }
        None => 100,
    };
    if max_tables == 0 {
        return ToolResponse::error(
            "invalid_arguments",
            "max_tables must be a positive integer",
            None,
            Some("toml_shape"),
        );
    }
    let detail = args
        .get("detail")
        .and_then(|v| v.as_str())
        .unwrap_or("normal");

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("toml_shape"),
        );
    }

    let valid_details = ["summary", "normal", "full"];
    if !valid_details.contains(&detail) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported detail level: {}", detail),
            Some(vec![format!("Use one of: {}", valid_details.join(", "))]),
            Some("toml_shape"),
        );
    }

    match crate::text::toml::toml_shape(text, max_tables) {
        Ok(result) => {
            if detail == "summary" {
                ToolResponse::success(
                    serde_json::json!({
                        "valid": result.valid,
                        "truncated": result.truncated,
                        "summary": result.summary,
                    }),
                    Some("toml_shape"),
                )
                .with_tool("toml_shape")
            } else {
                ToolResponse::success(
                    serde_json::json!({
                        "valid": result.valid,
                        "top_level_keys": result.top_level_keys,
                        "tables": result.tables,
                        "truncated": result.truncated,
                        "summary": result.summary,
                    }),
                    Some("toml_shape"),
                )
                .with_tool("toml_shape")
            }
        }
        Err(e) => ToolResponse::error("invalid_arguments", &e, None, Some("toml_shape")),
    }
}

pub fn version_compare_tool(args: &Value) -> ToolResponse {
    let a = match args.get("a").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'a' parameter",
                None,
                Some("version_compare"),
            )
        }
    };
    let b = match args.get("b").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'b' parameter",
                None,
                Some("version_compare"),
            )
        }
    };
    let scheme = args
        .get("scheme")
        .and_then(|v| v.as_str())
        .unwrap_or("semver");

    if a.chars().count() > MAX_TEXT_LENGTH || b.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Version string exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("version_compare"),
        );
    }

    let valid_schemes = ["semver", "pep440", "loose"];
    if !valid_schemes.contains(&scheme) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported scheme: {}", scheme),
            Some(vec![format!("Use one of: {}", valid_schemes.join(", "))]),
            Some("version_compare"),
        );
    }

    let result = crate::text::version::version_compare(a, b, scheme);
    ToolResponse::success(
        serde_json::json!({
            "comparison": result.comparison,
            "valid": result.valid,
            "scheme": result.scheme,
            "summary": result.summary,
        }),
        Some("version_compare"),
    )
    .with_tool("version_compare")
}

pub fn line_range_extract_tool(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                &format!(
                    "text must be a string, got {}",
                    json_type_name(args.get("text").unwrap_or(&Value::Null))
                ),
                None,
                Some("line_range_extract"),
            )
        }
    };
    // Validate start_line and end_line are integers (not bools)
    let start_line_i64 = match args.get("start_line") {
        Some(Value::Number(n)) if n.is_i64() => n.as_i64().unwrap(),
        Some(Value::Bool(_)) => {
            return ToolResponse::error(
                "invalid_arguments",
                "start_line must be an int, got bool",
                None,
                Some("line_range_extract"),
            );
        }
        Some(v) => {
            return ToolResponse::error(
                "invalid_arguments",
                &format!("start_line must be an int, got {}", json_type_name(v)),
                None,
                Some("line_range_extract"),
            );
        }
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'start_line' parameter",
                None,
                Some("line_range_extract"),
            );
        }
    };
    if start_line_i64 < 0 {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("start_line must be non-negative, got {}", start_line_i64),
            None,
            Some("line_range_extract"),
        );
    }
    let start_line = start_line_i64 as usize;
    let end_line_i64 = match args.get("end_line") {
        Some(Value::Number(n)) if n.is_i64() => n.as_i64().unwrap(),
        Some(Value::Bool(_)) => {
            return ToolResponse::error(
                "invalid_arguments",
                "end_line must be an int, got bool",
                None,
                Some("line_range_extract"),
            );
        }
        Some(v) => {
            return ToolResponse::error(
                "invalid_arguments",
                &format!("end_line must be an int, got {}", json_type_name(v)),
                None,
                Some("line_range_extract"),
            );
        }
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'end_line' parameter",
                None,
                Some("line_range_extract"),
            );
        }
    };
    if end_line_i64 < 0 {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("end_line must be non-negative, got {}", end_line_i64),
            None,
            Some("line_range_extract"),
        );
    }
    let end_line = end_line_i64 as usize;
    if start_line > end_line {
        return ToolResponse::error(
            "invalid_arguments",
            &format!(
                "start_line ({}) must be <= end_line ({})",
                start_line, end_line
            ),
            None,
            Some("line_range_extract"),
        );
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
        return ToolResponse::error(
            "input_too_large",
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
        Err(e) => ToolResponse::error("invalid_arguments", &e, None, Some("line_range_extract")),
    }
}

pub fn line_range_compare_tool(args: &Value) -> ToolResponse {
    let left_text = match args.get("left_text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
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
            return ToolResponse::error(
                "invalid_arguments",
                &format!(
                    "right_text must be a string, got {}",
                    json_type_name(args.get("right_text").unwrap_or(&Value::Null))
                ),
                None,
                Some("line_range_compare"),
            )
        }
    };
    let start_line_i64 = match args.get("start_line") {
        Some(Value::Number(n)) if n.is_i64() => n.as_i64().unwrap(),
        Some(Value::Bool(_)) => {
            return ToolResponse::error(
                "invalid_arguments",
                "start_line must be an int, got bool",
                None,
                Some("line_range_compare"),
            );
        }
        Some(v) => {
            return ToolResponse::error(
                "invalid_arguments",
                &format!("start_line must be an int, got {}", json_type_name(v)),
                None,
                Some("line_range_compare"),
            );
        }
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'start_line' parameter",
                None,
                Some("line_range_compare"),
            );
        }
    };
    if start_line_i64 < 0 {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("start_line must be non-negative, got {}", start_line_i64),
            None,
            Some("line_range_compare"),
        );
    }
    let start_line = start_line_i64 as usize;
    let end_line_i64 = match args.get("end_line") {
        Some(Value::Number(n)) if n.is_i64() => n.as_i64().unwrap(),
        Some(Value::Bool(_)) => {
            return ToolResponse::error(
                "invalid_arguments",
                "end_line must be an int, got bool",
                None,
                Some("line_range_compare"),
            );
        }
        Some(v) => {
            return ToolResponse::error(
                "invalid_arguments",
                &format!("end_line must be an int, got {}", json_type_name(v)),
                None,
                Some("line_range_compare"),
            );
        }
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'end_line' parameter",
                None,
                Some("line_range_compare"),
            );
        }
    };
    if end_line_i64 < 0 {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("end_line must be non-negative, got {}", end_line_i64),
            None,
            Some("line_range_compare"),
        );
    }
    let end_line = end_line_i64 as usize;
    if start_line > end_line {
        return ToolResponse::error(
            "invalid_arguments",
            &format!(
                "start_line ({}) must be <= end_line ({})",
                start_line, end_line
            ),
            None,
            Some("line_range_compare"),
        );
    }
    let line_base = args.get("line_base").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
    let comparison_mode = args
        .get("comparison_mode")
        .and_then(|v| v.as_str())
        .unwrap_or("exact");

    for (label, t) in [("left_text", left_text), ("right_text", right_text)] {
        let char_count = t.chars().count();
        if char_count > MAX_TEXT_LENGTH {
            return ToolResponse::error(
                "input_too_large",
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
        return ToolResponse::error(
            "invalid_arguments",
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
        Err(e) => ToolResponse::error("invalid_arguments", &e, None, Some("line_range_compare")),
    }
}

pub fn regex_safety_check_tool(args: &Value) -> ToolResponse {
    let pattern = match args.get("pattern").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'pattern' parameter",
                None,
                Some("regex_safety_check"),
            )
        }
    };

    if pattern.chars().count() > MAX_PATTERN_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Pattern exceeds {} chars", MAX_PATTERN_LENGTH),
            Some(vec![format!(
                "Maximum pattern length is {} characters",
                MAX_PATTERN_LENGTH
            )]),
            Some("regex_safety_check"),
        );
    }

    let result = crate::text::regex_safety::regex_safety_check(pattern);
    let risk = result.risk.clone();
    let findings_list = result.findings.clone();
    let pattern_length = pattern.chars().count();

    let envelope_findings: Vec<serde_json::Value> = findings_list
        .iter()
        .map(|f| {
            serde_json::json!({
                "code": f.kind.to_uppercase(),
                "severity": "warn",
                "message": f.message.clone(),
                "details": {"pattern_length": pattern_length}
            })
        })
        .collect();

    let machine_code = if risk == "medium" || risk == "high" {
        Some("REGEX_UNSAFE".to_string())
    } else {
        None
    };

    let mut resp = ToolResponse::success(
        serde_json::json!({
            "valid_pattern": result.valid_pattern,
            "risk": result.risk,
            "findings": result.findings,
        }),
        Some("regex_safety_check"),
    )
    .with_tool("regex_safety_check");

    if !envelope_findings.is_empty() {
        resp = resp.with_findings(envelope_findings);
    }
    if let Some(ref code) = machine_code {
        resp = resp.with_machine_code(code);
    }
    resp
}

pub fn text_replace_check_tool(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'text' parameter",
                None,
                Some("text_replace_check"),
            )
        }
    };
    let old = match args.get("old").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'old' parameter",
                None,
                Some("text_replace_check"),
            )
        }
    };
    let new = match args.get("new").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
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
                return ToolResponse::error(
                    "invalid_arguments",
                    &format!("max_preview_chars must be non-negative, got {}", n),
                    None,
                    Some("text_replace_check"),
                );
            }
            n as usize
        } else {
            return ToolResponse::error(
                "invalid_arguments",
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
        return ToolResponse::error(
            "invalid_arguments",
            &format!(
                "max_preview_chars {} exceeds {}",
                max_preview_chars, MAX_PREVIEW_CHARS
            ),
            None,
            Some("text_replace_check"),
        );
    }

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("text_replace_check"),
        );
    }

    if old.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("old exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("text_replace_check"),
        );
    }

    if new.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("new exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("text_replace_check"),
        );
    }

    let valid_modes = ["exact", "nfc", "nfkc", "casefold", "whitespace_collapse"];
    if !valid_modes.contains(&mode) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported mode: {}", mode),
            Some(vec![format!("Use one of: {}", valid_modes.join(", "))]),
            Some("text_replace_check"),
        );
    }

    let valid_newline_policies = ["preserve", "normalize_lf", "normalize_crlf"];
    if !valid_newline_policies.contains(&newline_policy) {
        return ToolResponse::error(
            "invalid_arguments",
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
        Err(e) => ToolResponse::error("invalid_arguments", &e, None, Some("text_replace_check")),
    }
}

pub fn text_transform(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
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
                        return ToolResponse::error(
                            "invalid_arguments",
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
            return ToolResponse::error(
                "invalid_arguments",
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
        return ToolResponse::error(
            "input_too_large",
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
        return ToolResponse::error(
            "invalid_arguments",
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
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unknown operation(s): {}", unknown_ops.join(", ")),
            Some(vec![format!(
                "Valid operations: {}",
                valid_operations.join(", ")
            )]),
            Some("text_transform"),
        );
    }

    if !["summary", "normal", "full"].contains(&detail) {
        return ToolResponse::error(
            "invalid_arguments",
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

pub fn json_shape_tool(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'text' parameter",
                None,
                Some("json_shape"),
            )
        }
    };
    let max_depth = args.get("max_depth").and_then(|v| v.as_u64()).unwrap_or(4) as usize;
    let max_keys = args.get("max_keys").and_then(|v| v.as_u64()).unwrap_or(100) as usize;
    let max_array_items = args
        .get("max_array_items")
        .and_then(|v| v.as_u64())
        .unwrap_or(5) as usize;

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("json_shape"),
        );
    }

    if max_depth < 1 {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("max_depth must be at least 1, got {}", max_depth),
            Some(vec!["Set max_depth to 1 or higher".to_string()]),
            Some("json_shape"),
        );
    }

    if max_keys < 1 {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("max_keys must be at least 1, got {}", max_keys),
            Some(vec!["Set max_keys to 1 or higher".to_string()]),
            Some("json_shape"),
        );
    }

    if max_array_items < 1 {
        return ToolResponse::error(
            "invalid_arguments",
            &format!(
                "max_array_items must be at least 1, got {}",
                max_array_items
            ),
            Some(vec!["Set max_array_items to 1 or higher".to_string()]),
            Some("json_shape"),
        );
    }

    const MAX_SHAPE_DEPTH: usize = 32;
    const MAX_SHAPE_KEYS: usize = 10_000;
    const MAX_SHAPE_ARRAY_ITEMS: usize = 10_000;

    if max_depth > MAX_SHAPE_DEPTH {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("max_depth {} exceeds {}", max_depth, MAX_SHAPE_DEPTH),
            None,
            Some("json_shape"),
        );
    }

    if max_keys > MAX_SHAPE_KEYS {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("max_keys {} exceeds {}", max_keys, MAX_SHAPE_KEYS),
            None,
            Some("json_shape"),
        );
    }

    if max_array_items > MAX_SHAPE_ARRAY_ITEMS {
        return ToolResponse::error(
            "invalid_arguments",
            &format!(
                "max_array_items {} exceeds {}",
                max_array_items, MAX_SHAPE_ARRAY_ITEMS
            ),
            None,
            Some("json_shape"),
        );
    }

    match crate::text::validate::json_shape(text, max_depth, max_keys, max_array_items) {
        Ok(result) => ToolResponse::success(
            serde_json::json!({
                "valid": result.valid,
                "shape": result.shape,
                "truncated": result.truncated,
                "summary": result.summary,
            }),
            Some("json_shape"),
        )
        .with_tool("json_shape"),
        Err(e) => ToolResponse::error("invalid_arguments", &e, None, Some("json_shape")),
    }
}

pub fn regex_finditer_tool(args: &Value) -> ToolResponse {
    let pattern = match _require_str(args, "pattern", "regex_finditer") {
        Ok(s) => s,
        Err(e) => return *e,
    };
    let text = match _require_str(args, "text", "regex_finditer") {
        Ok(s) => s,
        Err(e) => return *e,
    };
    let flags = match args.get("flags") {
        Some(Value::Array(arr)) => {
            let non_str_flags: Vec<usize> = arr
                .iter()
                .enumerate()
                .filter(|(_, v)| !v.is_string())
                .map(|(i, _)| i)
                .collect();
            if !non_str_flags.is_empty() {
                return ToolResponse::error(
                    "invalid_arguments",
                    "All flags must be strings",
                    Some(vec![format!(
                        "Non-string items at indices: {:?}",
                        &non_str_flags[..5.min(non_str_flags.len())]
                    )]),
                    Some("regex_finditer"),
                );
            }
            Some(
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect::<Vec<_>>(),
            )
        }
        Some(v) => {
            return ToolResponse::error(
                "invalid_arguments",
                &format!("flags must be a list, got {}", json_type_name(v)),
                None,
                Some("regex_finditer"),
            )
        }
        None => None,
    };
    let max_matches = args
        .get("max_matches")
        .and_then(|v| v.as_u64())
        .unwrap_or(MAX_MATCHES_REGEX as u64) as usize;
    if max_matches < 1 {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("max_matches must be at least 1, got {}", max_matches),
            Some(vec!["Set max_matches to 1 or higher".to_string()]),
            Some("regex_finditer"),
        );
    }
    if max_matches > MAX_MATCHES_HARD_CAP {
        return ToolResponse::error(
            "invalid_arguments",
            &format!(
                "max_matches {} exceeds maximum of {}",
                max_matches, MAX_MATCHES_HARD_CAP
            ),
            Some(vec![format!(
                "Set max_matches to {} or lower",
                MAX_MATCHES_HARD_CAP
            )]),
            Some("regex_finditer"),
        );
    }
    let max_matches = max_matches.min(MAX_MATCHES_HARD_CAP);
    let include_line_column = args
        .get("include_line_column")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let include_groups = args
        .get("include_groups")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let char_count = text.chars().count();
    if char_count > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            Some(vec![format!(
                "Maximum input length is {} characters",
                MAX_TEXT_LENGTH
            )]),
            Some("regex_finditer"),
        );
    }

    if pattern.chars().count() > MAX_PATTERN_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!(
                "Pattern length {} exceeds MAX_PATTERN_LENGTH {}",
                pattern.chars().count(),
                MAX_PATTERN_LENGTH
            ),
            Some(vec![format!(
                "Maximum pattern length is {} characters",
                MAX_PATTERN_LENGTH
            )]),
            Some("regex_finditer"),
        );
    }

    let safety = regex_safety_check(pattern);
    if safety.risk == "medium" || safety.risk == "high" {
        return ToolResponse::error(
            "unsafe_pattern",
            &format!(
                "Pattern has {} risk of catastrophic backtracking",
                safety.risk
            ),
            Some(vec![
                "Try a simpler pattern or break it into smaller parts".to_string(),
                "Use the regex_safety_check tool for detailed analysis and suggestions".to_string(),
            ]),
            Some("regex_finditer"),
        );
    }

    let pattern_owned = pattern.to_string();
    let text_owned = text.to_string();
    let flags_owned: Option<Vec<String>> = flags;
    let result = match run_with_timeout(Duration::from_secs(REGEX_TIMEOUT_SECONDS), move || {
        crate::text::validate::regex_finditer(
            &pattern_owned,
            &text_owned,
            flags_owned.as_ref(),
            max_matches,
            include_line_column,
            include_groups,
        )
    }) {
        Ok(r) => r,
        Err(_timeout) => {
            return ToolResponse::error(
                "timeout",
                "Regex execution exceeded time limit (possible ReDoS)",
                Some(vec![
                    "Try a simpler pattern or reduce max_matches".to_string()
                ]),
                Some("regex_finditer"),
            )
        }
    };

    let matches: Vec<serde_json::Value> = result
        .matches
        .iter()
        .map(|m| {
            let mut obj = serde_json::json!({
                "match": m.m,
                "span": m.span,
                "groups": m.groups,
                "groupdict": m.group_dict,
            });
            if let (Some(line), Some(column)) = (m.line, m.column) {
                obj["line"] = serde_json::json!(line);
                obj["column"] = serde_json::json!(column);
            }
            obj
        })
        .collect();

    ToolResponse::success(
        serde_json::json!({
            "valid_pattern": result.valid_pattern,
            "matches": matches,
            "truncated": result.truncated,
            "match_count": result.match_count,
            "error": result.error,
        }),
        Some("regex_finditer"),
    )
    .with_tool("regex_finditer")
}

pub fn validate_schema_light_tool(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'text' parameter",
                None,
                Some("validate_schema_light"),
            )
        }
    };
    let detail = args
        .get("detail")
        .and_then(|v| v.as_str())
        .unwrap_or("normal");
    let valid_details = ["summary", "normal", "full"];
    if !valid_details.contains(&detail) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported detail level: {}", detail),
            Some(vec![format!("Use one of: {}", valid_details.join(", "))]),
            Some("validate_schema_light"),
        );
    }

    let schema_val = args.get("schema");
    let schema = match schema_val.and_then(|v| v.as_object()) {
        Some(o) => o.clone(),
        None => {
            let type_name = match schema_val {
                Some(v) => json_type_name(v),
                None => "NoneType",
            };
            return ToolResponse::error(
                "invalid_arguments",
                &format!("schema must be a dict, got {}", type_name),
                None,
                Some("validate_schema_light"),
            );
        }
    };

    const MAX_SCHEMA_SIZE: usize = 100_000;
    let schema_json =
        serde_json::to_string(&serde_json::Value::Object(schema.clone())).unwrap_or_default();
    if schema_json.len() > MAX_SCHEMA_SIZE {
        return ToolResponse::error(
            "input_too_large",
            &format!(
                "Schema JSON size {} bytes exceeds limit of {} bytes",
                schema_json.len(),
                MAX_SCHEMA_SIZE
            ),
            None,
            Some("validate_schema_light"),
        );
    }

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!(
                "text length {} exceeds {}",
                text.chars().count(),
                MAX_TEXT_LENGTH
            ),
            None,
            Some("validate_schema_light"),
        );
    }

    let data: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            return ToolResponse::error(
                "invalid_arguments",
                &format!("Invalid JSON: {}", e),
                Some(vec!["Provide valid JSON".to_string()]),
                Some("validate_schema_light"),
            )
        }
    };

    let schema_value = serde_json::Value::Object(schema);
    let mut violations: Vec<serde_json::Value> = Vec::new();
    const MAX_SCHEMA_VIOLATIONS: usize = 100;

    fn get_type_name(value: &serde_json::Value) -> &str {
        match value {
            serde_json::Value::Null => "null",
            serde_json::Value::Bool(_) => "boolean",
            serde_json::Value::Number(n) => {
                if n.is_i64() || n.is_u64() {
                    "integer"
                } else {
                    "number"
                }
            }
            serde_json::Value::String(_) => "string",
            serde_json::Value::Array(_) => "array",
            serde_json::Value::Object(_) => "object",
        }
    }

    fn add_violation(
        violations: &mut Vec<serde_json::Value>,
        path: &str,
        message: &str,
        value_type: Option<&str>,
        expected_type: Option<&str>,
    ) {
        if violations.len() < MAX_SCHEMA_VIOLATIONS {
            violations.push(serde_json::json!({
                "path": path,
                "message": message,
                "value_type": value_type,
                "expected_type": expected_type,
            }));
        }
    }

    fn validate(
        path: &str,
        value: &serde_json::Value,
        schema_def: &serde_json::Value,
        violations: &mut Vec<serde_json::Value>,
        depth: usize,
        elements: &mut usize,
    ) {
        if violations.len() >= MAX_SCHEMA_VIOLATIONS {
            return;
        }
        if depth > MAX_SCHEMA_DEPTH {
            add_violation(violations, path, "schema depth limit exceeded", None, None);
            return;
        }
        *elements += 1;
        if *elements > MAX_SCHEMA_ELEMENTS {
            add_violation(
                violations,
                path,
                "schema element limit exceeded",
                None,
                None,
            );
            return;
        }

        let schema_obj = match schema_def.as_object() {
            Some(o) => o,
            None => return,
        };

        let expected_type = schema_obj.get("type").and_then(|v| v.as_str());

        if let Some(exp_type) = expected_type {
            let actual_type = get_type_name(value);
            let type_matches = match exp_type {
                "object" => value.is_object(),
                "array" => value.is_array(),
                "string" => value.is_string(),
                "number" => value.is_number(),
                "integer" => value.is_i64() || value.is_u64(),
                "boolean" => value.is_boolean(),
                "null" => value.is_null(),
                _ => false,
            };

            if !type_matches {
                let msg = format!("expected {}, got {}", exp_type, actual_type);
                add_violation(violations, path, &msg, Some(actual_type), Some(exp_type));
                return;
            }
        }

        if let (Some(exp_type), serde_json::Value::Object(obj_val)) = (expected_type, value) {
            if exp_type == "object" {
                let required = schema_obj.get("required").and_then(|v| v.as_array());
                if let Some(req_arr) = required {
                    for req_item in req_arr {
                        if let Some(req_key) = req_item.as_str() {
                            if !obj_val.contains_key(req_key) {
                                let full_path = if path.is_empty() {
                                    format!("/{}", req_key)
                                } else {
                                    format!("{}/{}", path, req_key)
                                };
                                add_violation(
                                    violations,
                                    &full_path,
                                    &format!("missing required key '{}'", req_key),
                                    None,
                                    Some("object"),
                                );
                            }
                        }
                    }
                }

                if let Some(add_props) = schema_obj
                    .get("additional_properties")
                    .or_else(|| schema_obj.get("additionalProperties"))
                {
                    if add_props.as_bool() == Some(false) {
                        let props = schema_obj.get("properties").and_then(|v| v.as_object());
                        let allowed_keys: std::collections::HashSet<_> = props
                            .map(|p| p.keys().cloned().collect())
                            .unwrap_or_default();
                        for key in obj_val.keys() {
                            if !allowed_keys.contains(key) {
                                let full_path = if path.is_empty() {
                                    format!("/{}", key)
                                } else {
                                    format!("{}/{}", path, key)
                                };
                                add_violation(
                                    violations,
                                    &full_path,
                                    &format!("additional property '{}' not allowed", key),
                                    Some("string"),
                                    None,
                                );
                            }
                        }
                    }
                }

                if let Some(properties) = schema_obj.get("properties").and_then(|v| v.as_object()) {
                    for (prop_name, prop_schema) in properties {
                        if let Some(prop_value) = obj_val.get(prop_name) {
                            let full_path = if path.is_empty() {
                                format!("/{}", prop_name)
                            } else {
                                format!("{}/{}", path, prop_name)
                            };
                            validate(
                                &full_path,
                                prop_value,
                                prop_schema,
                                violations,
                                depth + 1,
                                elements,
                            );
                        }
                    }
                }
            }
        } else if let (Some(exp_type), serde_json::Value::Array(arr_val)) = (expected_type, value) {
            if exp_type == "array" {
                if let Some(min_items) = schema_obj
                    .get("min_items")
                    .or_else(|| schema_obj.get("minItems"))
                    .and_then(|v| v.as_u64())
                {
                    if (arr_val.len() as u64) < min_items {
                        add_violation(
                            violations,
                            path,
                            &format!(
                                "array has {} items, minimum is {}",
                                arr_val.len(),
                                min_items
                            ),
                            Some("array"),
                            None,
                        );
                    }
                }

                if let Some(max_items) = schema_obj
                    .get("max_items")
                    .or_else(|| schema_obj.get("maxItems"))
                    .and_then(|v| v.as_u64())
                {
                    if arr_val.len() as u64 > max_items {
                        add_violation(
                            violations,
                            path,
                            &format!(
                                "array has {} items, maximum is {}",
                                arr_val.len(),
                                max_items
                            ),
                            Some("array"),
                            None,
                        );
                    }
                }

                if let Some(items_schema) = schema_obj.get("items") {
                    for (i, item) in arr_val.iter().enumerate() {
                        let item_path = format!("{}/[{}]", path, i);
                        validate(
                            &item_path,
                            item,
                            items_schema,
                            violations,
                            depth + 1,
                            elements,
                        );
                    }
                }
            }
        } else if let (Some(exp_type), serde_json::Value::String(str_val)) = (expected_type, value)
        {
            if exp_type == "string" {
                if let Some(min_len) = schema_obj.get("min_length").and_then(|v| v.as_u64()) {
                    if (str_val.chars().count() as u64) < min_len {
                        add_violation(
                            violations,
                            path,
                            &format!(
                                "string has length {}, minimum is {}",
                                str_val.chars().count(),
                                min_len
                            ),
                            Some("string"),
                            None,
                        );
                    }
                }

                if let Some(max_len) = schema_obj.get("max_length").and_then(|v| v.as_u64()) {
                    if (str_val.chars().count() as u64) > max_len {
                        add_violation(
                            violations,
                            path,
                            &format!(
                                "string has length {}, maximum is {}",
                                str_val.chars().count(),
                                max_len
                            ),
                            Some("string"),
                            None,
                        );
                    }
                }

                if let Some(pattern) = schema_obj.get("pattern").and_then(|v| v.as_str()) {
                    if let Ok(re) = regex::Regex::new(pattern) {
                        // Use find with start offset 0 to match Python's re.match behavior
                        // (match at start of string only)
                        let matched = re.find(str_val).map(|m| m.start() == 0).unwrap_or(false);
                        if !matched {
                            let display_val = if str_val.chars().count() > 20 {
                                let truncated: String = str_val.chars().take(20).collect();
                                format!("{}...", truncated)
                            } else {
                                str_val.clone()
                            };
                            add_violation(
                                violations,
                                path,
                                &format!(
                                    "string '{}' does not match pattern '{}'",
                                    display_val, pattern
                                ),
                                Some("string"),
                                None,
                            );
                        }
                    }
                }
            }
        }

        if let Some(enum_values) = schema_obj.get("enum") {
            if let Some(arr) = enum_values.as_array() {
                if !arr.contains(value) {
                    fn fmt_enum_value(v: &serde_json::Value) -> String {
                        match v {
                            serde_json::Value::String(s) => format!("'{}'", s),
                            other => format!("{}", other),
                        }
                    }
                    let value_str = fmt_enum_value(value);
                    let enum_str: Vec<String> = arr.iter().map(fmt_enum_value).collect();
                    add_violation(
                        violations,
                        path,
                        &format!(
                            "value {} is not in enum [{}]",
                            value_str,
                            enum_str.join(", ")
                        ),
                        Some(get_type_name(value)),
                        None,
                    );
                }
            }
        }
    }

    // Pre-check schema depth (matching Python: return error immediately if too deep)
    fn check_schema_depth(o: &serde_json::Value, d: usize) -> Result<usize, String> {
        if d > MAX_SCHEMA_DEPTH {
            return Err("schema too deeply nested".to_string());
        }
        match o {
            serde_json::Value::Object(obj) => {
                if obj.is_empty() {
                    Ok(d)
                } else {
                    obj.values()
                        .map(|v| check_schema_depth(v, d + 1))
                        .max()
                        .unwrap_or(Ok(d))
                }
            }
            serde_json::Value::Array(arr) => {
                if arr.is_empty() {
                    Ok(d)
                } else {
                    arr.iter()
                        .map(|v| check_schema_depth(v, d + 1))
                        .max()
                        .unwrap_or(Ok(d))
                }
            }
            _ => Ok(d),
        }
    }
    if let Err(e) = check_schema_depth(&schema_value, 0) {
        return ToolResponse::error(
            "input_too_large",
            &format!("schema nesting too deep (max {}): {}", MAX_SCHEMA_DEPTH, e),
            None,
            Some("validate_schema_light"),
        );
    }

    validate("", &data, &schema_value, &mut violations, 0, &mut 0usize);

    let truncated = violations.len() >= MAX_SCHEMA_VIOLATIONS;
    let valid = violations.is_empty();

    let summary = if violations.is_empty() {
        "Data is valid".to_string()
    } else if truncated {
        format!(
            "Schema violations detected (truncated, {} shown)",
            violations.len()
        )
    } else {
        let issue = if violations.len() == 1 {
            "issue"
        } else {
            "issues"
        };
        format!("Schema violations detected: {} {}", violations.len(), issue)
    };

    let output = if detail == "summary" {
        serde_json::json!({
            "valid": valid,
            "summary": summary,
        })
    } else {
        serde_json::json!({
            "valid": valid,
            "violations": violations,
            "truncated": truncated,
            "summary": summary,
        })
    };

    ToolResponse::success(output, Some("validate_schema_light")).with_tool("validate_schema_light")
}

// ---------------------------------------------------------------------------
// prompt_input_inspect helpers
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

pub fn prompt_input_inspect_tool(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'text' parameter",
                None,
                Some("prompt_input_inspect"),
            )
        }
    };

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
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
                return ToolResponse::error(
                    "invalid_arguments",
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
            return ToolResponse::error(
                "invalid_arguments",
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
                return ToolResponse::error(
                    "input_too_large",
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
            return ToolResponse::error(
                "invalid_arguments",
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
            machine_code = Some("PROMPT_HIDDEN_CONTENT".to_string());
        } else {
            machine_code = Some("PROMPT_HAS_FLAGS".to_string());
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

pub fn text_position(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
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
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported detail level: {}", detail),
            Some(vec![format!("Use one of: {}", valid_details.join(", "))]),
            Some("text_position"),
        );
    }

    if line_base != 0 && line_base != 1 {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("line_base must be 0 or 1, got {}", line_base),
            Some(vec!["Set line_base to 0 or 1".to_string()]),
            Some("text_position"),
        );
    }
    if column_base != 0 && column_base != 1 {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("column_base must be 0 or 1, got {}", column_base),
            Some(vec!["Set column_base to 0 or 1".to_string()]),
            Some("text_position"),
        );
    }

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
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
        return ToolResponse::error(
            "invalid_arguments",
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

pub fn text_hash(args: &Value) -> ToolResponse {
    let text = match _require_str(args, "text", "text_hash") {
        Ok(s) => s,
        Err(e) => return *e,
    };
    let algorithms = match args.get("algorithms") {
        Some(Value::Array(arr)) => {
            if arr.len() > 10 {
                return ToolResponse::error(
                    "invalid_arguments",
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
                return ToolResponse::error(
                    "invalid_arguments",
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
            return ToolResponse::error(
                "invalid_arguments",
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
        return ToolResponse::error(
            "invalid_arguments",
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
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Invalid encoding: {}", encoding),
            Some(vec![
                "Use a valid encoding name like 'utf-8', 'ascii', 'latin-1'".to_string(),
            ]),
            Some("text_hash"),
        );
    }

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
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
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported detail level: {}", detail),
            Some(vec!["Use one of: summary, normal, full".to_string()]),
            Some("escape_text"),
        );
    }

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
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
        return ToolResponse::error(
            "invalid_arguments",
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
        Err(e) => ToolResponse::error("internal_error", &e, None, Some("escape_text")),
    }
}

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
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported detail level: {}", detail),
            Some(vec!["Use one of: summary, normal, full".to_string()]),
            Some("unescape_text"),
        );
    }

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
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
        return ToolResponse::error(
            "invalid_arguments",
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

pub fn text_window(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'text' parameter",
                None,
                Some("text_window"),
            )
        }
    };
    let position = match args.get("position").and_then(|v| v.as_object()) {
        Some(obj) => obj.clone(),
        None => {
            return ToolResponse::error(
                "invalid_arguments",
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
                    return ToolResponse::error(
                        "invalid_arguments",
                        &format!("context_lines must be non-negative, got {}", n),
                        Some(vec!["Set context_lines to 0 or higher".to_string()]),
                        Some("text_window"),
                    );
                }
                if n > 10000 {
                    return ToolResponse::error(
                        "invalid_arguments",
                        &format!("context_lines {} exceeds 10000", n),
                        None,
                        Some("text_window"),
                    );
                }
                n as usize
            } else {
                return ToolResponse::error(
                    "invalid_arguments",
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
        return ToolResponse::error(
            "input_too_large",
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
        return ToolResponse::error(
            "invalid_arguments",
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
                    return ToolResponse::error(
                        "invalid_arguments",
                        &format!("position.{}={} out of range [0, {}]", key, n, max_pos),
                        None,
                        Some("text_window"),
                    );
                }
            } else if v.is_boolean() {
                return ToolResponse::error(
                    "invalid_arguments",
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
                return ToolResponse::error(
                    "invalid_arguments",
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

pub fn json_extract(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'text' parameter",
                None,
                Some("json_extract"),
            )
        }
    };
    let pointer = match args.get("pointer") {
        Some(v) => match v.as_str() {
            Some(s) => s,
            None => {
                return ToolResponse::error(
                    "invalid_arguments",
                    &format!("pointer must be a string, got {}", json_type_name(v)),
                    None,
                    Some("json_extract"),
                )
            }
        },
        None => "",
    };
    let max_output_chars = match args.get("max_output_chars") {
        Some(v) => {
            if let Some(n) = v.as_i64() {
                if n < 0 {
                    return ToolResponse::error(
                        "invalid_arguments",
                        &format!("max_output_chars must be non-negative, got {}", n),
                        None,
                        Some("json_extract"),
                    );
                }
                n as usize
            } else {
                return ToolResponse::error(
                    "invalid_arguments",
                    &format!(
                        "max_output_chars must be a non-negative integer, got {}",
                        json_type_name(v)
                    ),
                    None,
                    Some("json_extract"),
                );
            }
        }
        None => 4000,
    };
    let detail = args
        .get("detail")
        .and_then(|v| v.as_str())
        .unwrap_or("normal");

    if !["summary", "normal", "full"].contains(&detail) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported detail level: {}", detail),
            Some(vec!["Use one of: summary, normal, full".to_string()]),
            Some("json_extract"),
        );
    }

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("json_extract"),
        );
    }

    if pointer.len() > 4096 {
        return ToolResponse::error(
            "input_too_large",
            &format!("pointer length {} exceeds 4096", pointer.len()),
            None,
            Some("json_extract"),
        );
    }

    if max_output_chars > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "invalid_arguments",
            &format!(
                "max_output_chars {} exceeds {}",
                max_output_chars, MAX_TEXT_LENGTH
            ),
            None,
            Some("json_extract"),
        );
    }

    let parsed = match serde_json::from_str::<serde_json::Value>(text) {
        Ok(v) => v,
        Err(e) => {
            return ToolResponse::success(
                serde_json::json!({
                    "valid_json": false,
                    "found": false,
                    "pointer": pointer,
                    "value_type": null,
                    "value": null,
                    "preview": null,
                    "child_keys": null,
                    "array_length": null,
                    "truncated": false,
                    "missing_at": null,
                    "reason": null,
                    "available_keys": null,
                    "error": e.to_string(),
                    "line": e.line(),
                    "column": e.column(),
                    "summary": format!("Invalid JSON: {}", e),
                }),
                Some("json_extract"),
            )
            .with_tool("json_extract");
        }
    };

    if pointer.is_empty() {
        let full_preview = match &parsed {
            serde_json::Value::String(s) => s.clone(),
            other => serde_json::to_string(other).unwrap_or_default(),
        };
        let child_keys = match &parsed {
            serde_json::Value::Object(map) => Some(map.keys().cloned().collect::<Vec<_>>()),
            _ => None,
        };
        let summary = build_extract_summary(&parsed);
        let truncated = full_preview.chars().count() > max_output_chars;
        let preview: String = full_preview.chars().take(max_output_chars).collect();
        let mut result = serde_json::json!({
            "valid_json": true,
            "found": true,
            "pointer": pointer,
            "value": parsed,
            "value_type": get_json_type(&parsed),
            "preview": preview,
            "child_keys": child_keys,
            "array_length": null,
            "truncated": truncated,
            "missing_at": null,
            "reason": null,
            "available_keys": null,
            "error": null,
            "line": null,
            "column": null,
            "summary": summary,
        });
        if let serde_json::Value::Array(ref arr) = parsed {
            result["array_length"] = serde_json::json!(arr.len());
        }
        if detail == "summary" {
            result = serde_json::json!({
                "valid_json": true,
                "found": true,
                "summary": summary,
            });
        }
        return ToolResponse::success(result, Some("json_extract")).with_tool("json_extract");
    }

    let raw_tokens: Vec<&str> = pointer.split('/').collect();
    let tokens: Vec<&str> = if raw_tokens.first() == Some(&"") {
        raw_tokens[1..].to_vec()
    } else {
        raw_tokens
    };
    let mut current = &parsed;

    for token in tokens {
        let decoded = token.replace("~1", "/").replace("~0", "~");
        match current {
            serde_json::Value::Object(map) => match map.get(&decoded) {
                Some(v) => current = v,
                None => {
                    let missing_at = format!("/{}", token);
                    return ToolResponse::success(
                        serde_json::json!({
                            "valid_json": true,
                            "found": false,
                            "pointer": pointer,
                            "value_type": null,
                            "value": null,
                            "preview": null,
                            "child_keys": null,
                            "array_length": null,
                            "truncated": false,
                            "missing_at": missing_at,
                            "reason": "key_not_found",
                            "available_keys": map.keys().collect::<Vec<_>>(),
                            "error": null,
                            "line": null,
                            "column": null,
                            "summary": format!("Key '{}' not found in object at /{}", token, token),
                        }),
                        Some("json_extract"),
                    )
                    .with_tool("json_extract");
                }
            },
            serde_json::Value::Array(arr) => match decoded.parse::<usize>() {
                Ok(idx) if idx < arr.len() => current = &arr[idx],
                _ => {
                    return ToolResponse::success(
                        serde_json::json!({
                            "valid_json": true,
                            "found": false,
                            "pointer": pointer,
                            "value_type": null,
                            "value": null,
                            "preview": null,
                            "child_keys": null,
                            "array_length": arr.len(),
                            "truncated": false,
                            "missing_at": format!("/{}", token),
                            "reason": "index_out_of_range",
                            "available_keys": null,
                            "error": null,
                            "line": null,
                            "column": null,
                            "summary": null,
                        }),
                        Some("json_extract"),
                    )
                    .with_tool("json_extract");
                }
            },
            _ => {
                return ToolResponse::success(
                    serde_json::json!({
                        "valid_json": true,
                        "found": false,
                        "pointer": pointer,
                        "value_type": null,
                        "value": null,
                        "preview": null,
                        "child_keys": null,
                        "array_length": null,
                        "truncated": false,
                        "missing_at": null,
                        "reason": "invalid_pointer_syntax",
                        "available_keys": null,
                        "error": null,
                        "line": null,
                        "column": null,
                        "summary": null,
                    }),
                    Some("json_extract"),
                )
                .with_tool("json_extract");
            }
        }
    }

    let full_preview = match current {
        serde_json::Value::String(s) => s.clone(),
        other => serde_json::to_string(other).unwrap_or_default(),
    };
    let char_count = full_preview.chars().count();
    let truncated = char_count > max_output_chars;
    let preview: String = full_preview.chars().take(max_output_chars).collect();
    let child_keys = match current {
        serde_json::Value::Object(map) => Some(map.keys().cloned().collect::<Vec<_>>()),
        _ => None,
    };
    let summary = build_extract_summary(current);
    let mut result = serde_json::json!({
        "valid_json": true,
        "found": true,
        "pointer": pointer,
        "value": *current,
        "value_type": get_json_type(current),
        "preview": preview,
        "child_keys": child_keys,
        "array_length": null,
        "truncated": truncated,
        "missing_at": null,
        "reason": null,
        "available_keys": null,
        "error": null,
        "line": null,
        "column": null,
        "summary": summary,
    });
    if let serde_json::Value::Array(arr) = current {
        result["array_length"] = serde_json::json!(arr.len());
    }
    if detail == "summary" {
        result = serde_json::json!({
            "valid_json": true,
            "found": true,
            "summary": summary,
        });
    }
    ToolResponse::success(result, Some("json_extract")).with_tool("json_extract")
}

fn build_extract_summary(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Object(map) => format!("Object with {} keys", map.len()),
        serde_json::Value::Array(arr) => format!("Array of {} elements", arr.len()),
        serde_json::Value::String(s) => {
            let preview: String = s.chars().take(50).collect();
            let truncated = if s.chars().count() > 50 { "..." } else { "" };
            format!("String: \"{}{}\"", preview, truncated)
        }
        serde_json::Value::Number(n) => format!("Number: {}", n),
        serde_json::Value::Bool(b) => format!("Boolean: {}", b),
        serde_json::Value::Null => "null".to_string(),
    }
}

fn json_value_preview(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

fn get_json_type(v: &serde_json::Value) -> &str {
    match v {
        serde_json::Value::Object(_) => "object",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Null => "null",
    }
}

fn get_json_type_detail(v: &serde_json::Value) -> &str {
    match v {
        serde_json::Value::Object(_) => "object",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                "integer"
            } else {
                "float"
            }
        }
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Null => "null",
    }
}

/// Python-style type names matching `type(parsed).__name__` for JSON scalars.
fn get_python_json_type(v: &serde_json::Value) -> &str {
    match v {
        serde_json::Value::Object(_) => "object",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::String(_) => "str",
        serde_json::Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                "int"
            } else {
                "float"
            }
        }
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Null => "NoneType",
    }
}

pub fn json_compare(args: &Value) -> ToolResponse {
    let a = match args.get("a").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'a' parameter",
                None,
                Some("json_compare"),
            )
        }
    };
    let b = match args.get("b").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'b' parameter",
                None,
                Some("json_compare"),
            )
        }
    };
    let ignore_object_order = args
        .get("ignore_object_order")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let ignore_array_order = args
        .get("ignore_array_order")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let numeric_string_equivalence = args
        .get("numeric_string_equivalence")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let casefold_keys = args
        .get("casefold_keys")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let treat_missing_null_as_equal = args
        .get("treat_missing_null_as_equal")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let max_diffs = args.get("max_diffs").and_then(|v| v.as_i64()).unwrap_or(50);
    let detail = args
        .get("detail")
        .and_then(|v| v.as_str())
        .unwrap_or("normal");

    if !["summary", "normal", "full"].contains(&detail) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported detail level: {}", detail),
            Some(vec!["Use one of: summary, normal, full".to_string()]),
            Some("json_compare"),
        );
    }

    if max_diffs < 0 {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("max_diffs must be non-negative, got {}", max_diffs),
            None,
            Some("json_compare"),
        );
    }
    if max_diffs > 10_000 {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("max_diffs {} exceeds 10000", max_diffs),
            None,
            Some("json_compare"),
        );
    }
    let max_diffs = max_diffs as usize;

    let parsed_a: Result<serde_json::Value, _> = serde_json::from_str(a);
    let parsed_b: Result<serde_json::Value, _> = serde_json::from_str(b);

    let valid_a = parsed_a.is_ok();
    let valid_b = parsed_b.is_ok();

    if !valid_a || !valid_b {
        let mut diffs: Vec<serde_json::Value> = Vec::new();
        if let Err(ref e) = parsed_a {
            let line = e.line();
            let col = e.column();
            let raw = e.to_string();
            let msg = if let Some(idx) = raw.rfind(" at line ") {
                raw[..idx].to_string()
            } else {
                raw
            };
            diffs.push(serde_json::json!({
                "path": "",
                "kind": "parse_error_a",
                "a_type": Value::Null,
                "b_type": Value::Null,
                "a_preview": format!("Line {}, Col {}: {}", line, col, msg),
                "b_preview": Value::Null,
            }));
        }
        if let Err(ref e) = parsed_b {
            let line = e.line();
            let col = e.column();
            let raw = e.to_string();
            let msg = if let Some(idx) = raw.rfind(" at line ") {
                raw[..idx].to_string()
            } else {
                raw
            };
            diffs.push(serde_json::json!({
                "path": "",
                "kind": "parse_error_b",
                "a_type": Value::Null,
                "b_type": Value::Null,
                "a_preview": Value::Null,
                "b_preview": format!("Line {}, Col {}: {}", line, col, msg),
            }));
        }
        let diff_count = diffs.len();
        return ToolResponse::success(
            serde_json::json!({
                "valid_json_a": valid_a,
                "valid_json_b": valid_b,
                "equal": false,
                "same_type": false,
                "diff_count": diff_count,
                "diffs": diffs,
                "truncated": false,
                "summary": "One or both inputs are not valid JSON",
            }),
            Some("json_compare"),
        )
        .with_tool("json_compare");
    }

    let parsed_a = parsed_a.unwrap();
    let parsed_b = parsed_b.unwrap();

    let options = JsonCompareOptions {
        ignore_object_order,
        ignore_array_order,
        numeric_string_equivalence,
        casefold_keys,
        treat_missing_null_as_equal,
        max_diffs,
    };
    let (equal, type_match, diffs) = compare_json_values(&parsed_a, &parsed_b, options);

    let truncated = diffs.len() >= max_diffs;
    let diff_count = diffs.len();
    let summary = if equal {
        "JSON documents are equal".to_string()
    } else {
        format!(
            "JSON documents differ at {} path{}",
            diff_count,
            if diff_count != 1 { "s" } else { "" }
        )
    };

    let diffs_json: Vec<serde_json::Value> = diffs
        .iter()
        .map(|d| {
            serde_json::json!({
                "path": d.path,
                "kind": d.kind,
                "a_type": d.a_type,
                "b_type": d.b_type,
                "a_preview": d.a_preview,
                "b_preview": d.b_preview,
            })
        })
        .collect();

    let result = if detail == "summary" {
        serde_json::json!({
            "valid_json_a": true,
            "valid_json_b": true,
            "equal": equal,
            "same_type": type_match,
            "diff_count": diff_count,
            "summary": summary,
        })
    } else {
        serde_json::json!({
            "valid_json_a": true,
            "valid_json_b": true,
            "equal": equal,
            "same_type": type_match,
            "diff_count": diff_count,
            "diffs": diffs_json,
            "truncated": truncated,
            "summary": summary,
        })
    };

    ToolResponse::success(result, Some("json_compare")).with_tool("json_compare")
}

#[derive(Debug, serde::Serialize)]
struct JsonDiff {
    path: String,
    kind: String,
    a_type: Option<String>,
    b_type: Option<String>,
    a_preview: Option<String>,
    b_preview: Option<String>,
}

#[derive(Clone, Copy)]
struct JsonCompareOptions {
    ignore_object_order: bool,
    ignore_array_order: bool,
    numeric_string_equivalence: bool,
    casefold_keys: bool,
    treat_missing_null_as_equal: bool,
    max_diffs: usize,
}

struct JsonCompareState {
    diffs: Vec<JsonDiff>,
    type_match: bool,
}

fn compare_json_values(
    a: &serde_json::Value,
    b: &serde_json::Value,
    options: JsonCompareOptions,
) -> (bool, bool, Vec<JsonDiff>) {
    let mut state = JsonCompareState {
        diffs: Vec::new(),
        type_match: true,
    };

    fn compare_rec(
        a: &serde_json::Value,
        b: &serde_json::Value,
        path: &str,
        options: JsonCompareOptions,
        state: &mut JsonCompareState,
    ) {
        if state.diffs.len() >= options.max_diffs {
            return;
        }

        if options.treat_missing_null_as_equal && (a.is_null() || b.is_null()) {
            return;
        }

        let type_a = get_json_type_detail(a);
        let type_b = get_json_type_detail(b);

        if options.numeric_string_equivalence {
            if let (serde_json::Value::String(s), serde_json::Value::Number(n)) = (a, b) {
                if let Ok(num) = s.parse::<f64>() {
                    if num == n.as_f64().unwrap_or(f64::NAN) {
                        return;
                    }
                }
            }
            if let (serde_json::Value::Number(n), serde_json::Value::String(s)) = (a, b) {
                if let Ok(num) = s.parse::<f64>() {
                    if num == n.as_f64().unwrap_or(f64::NAN) {
                        return;
                    }
                }
            }
            if let (serde_json::Value::String(s1), serde_json::Value::String(s2)) = (a, b) {
                if let (Ok(n1), Ok(n2)) = (s1.parse::<f64>(), s2.parse::<f64>()) {
                    if n1 == n2 {
                        return;
                    }
                }
            }
        }

        if type_a != type_b {
            state.type_match = false;
            state.diffs.push(JsonDiff {
                path: path.to_string(),
                kind: "type_changed".to_string(),
                a_type: Some(type_a.to_string()),
                b_type: Some(type_b.to_string()),
                a_preview: Some(json_value_preview(a)),
                b_preview: Some(json_value_preview(b)),
            });
            return;
        }

        match (a, b) {
            (serde_json::Value::Object(obj_a), serde_json::Value::Object(obj_b)) => {
                if options.ignore_object_order {
                    // Order-insensitive: build casefolded → original key maps
                    let keys_a_map: std::collections::HashMap<String, String> = obj_a
                        .keys()
                        .map(|k| {
                            (
                                if options.casefold_keys {
                                    unicode_casefold(k)
                                } else {
                                    k.clone()
                                },
                                k.clone(),
                            )
                        })
                        .collect();
                    let keys_b_map: std::collections::HashMap<String, String> = obj_b
                        .keys()
                        .map(|k| {
                            (
                                if options.casefold_keys {
                                    unicode_casefold(k)
                                } else {
                                    k.clone()
                                },
                                k.clone(),
                            )
                        })
                        .collect();
                    let keys_a_set: std::collections::HashSet<&String> =
                        keys_a_map.keys().collect();
                    let keys_b_set: std::collections::HashSet<&String> =
                        keys_b_map.keys().collect();

                    // Report keys only in A (missing in B)
                    let mut only_a: Vec<&String> =
                        keys_a_set.difference(&keys_b_set).copied().collect();
                    only_a.sort();
                    for cf_key in only_a {
                        let orig_key = &keys_a_map[cf_key];
                        let new_path = if path.is_empty() {
                            format!("/{}", orig_key)
                        } else {
                            format!("{}/{}", path, orig_key)
                        };
                        let a_val = &obj_a[orig_key];
                        state.diffs.push(JsonDiff {
                            path: new_path,
                            kind: "key_missing_in_b".to_string(),
                            a_type: Some(get_json_type(a_val).to_string()),
                            b_type: None,
                            a_preview: Some(json_value_preview(a_val)),
                            b_preview: None,
                        });
                    }

                    // Report keys only in B (missing in A)
                    let mut only_b: Vec<&String> =
                        keys_b_set.difference(&keys_a_set).copied().collect();
                    only_b.sort();
                    for cf_key in only_b {
                        let orig_key = &keys_b_map[cf_key];
                        let new_path = if path.is_empty() {
                            format!("/{}", orig_key)
                        } else {
                            format!("{}/{}", path, orig_key)
                        };
                        let b_val = &obj_b[orig_key];
                        state.diffs.push(JsonDiff {
                            path: new_path,
                            kind: "key_missing_in_a".to_string(),
                            a_type: None,
                            b_type: Some(get_json_type(b_val).to_string()),
                            a_preview: None,
                            b_preview: Some(json_value_preview(b_val)),
                        });
                    }

                    // Compare common keys
                    let common: std::collections::HashSet<&String> =
                        keys_a_set.intersection(&keys_b_set).copied().collect();
                    let mut common_sorted: Vec<&String> = common.into_iter().collect();
                    common_sorted.sort();
                    for cf_key in common_sorted {
                        let orig_key_a = &keys_a_map[cf_key];
                        let orig_key_b = &keys_b_map[cf_key];
                        let new_path = if path.is_empty() {
                            if orig_key_a == orig_key_b {
                                format!("/{}", orig_key_a)
                            } else {
                                format!("/{}->{}", orig_key_a, orig_key_b)
                            }
                        } else {
                            if orig_key_a == orig_key_b {
                                format!("{}/{}", path, orig_key_a)
                            } else {
                                format!("{}/{}->{}", path, orig_key_a, orig_key_b)
                            }
                        };
                        compare_rec(
                            &obj_a[orig_key_a],
                            &obj_b[orig_key_b],
                            &new_path,
                            options,
                            state,
                        );
                    }
                } else {
                    // Order-sensitive: compare by positional order of keys
                    let a_key_order: Vec<String> = obj_a
                        .keys()
                        .map(|k| {
                            if options.casefold_keys {
                                unicode_casefold(k)
                            } else {
                                k.clone()
                            }
                        })
                        .collect();
                    let b_key_order: Vec<String> = obj_b
                        .keys()
                        .map(|k| {
                            if options.casefold_keys {
                                unicode_casefold(k)
                            } else {
                                k.clone()
                            }
                        })
                        .collect();
                    let min_len = a_key_order.len().min(b_key_order.len());

                    // Pre-collect original keys for O(1) indexed access
                    let a_keys_vec: Vec<&String> = obj_a.keys().collect();
                    let b_keys_vec: Vec<&String> = obj_b.keys().collect();

                    // Compare keys at matching positions
                    for i in 0..min_len {
                        if a_key_order[i] != b_key_order[i] {
                            // Key at position i differs — report as key_missing_in_b for the A key
                            let orig_key_a = a_keys_vec[i];
                            let new_path = if path.is_empty() {
                                format!("/{}", orig_key_a)
                            } else {
                                format!("{}/{}", path, orig_key_a)
                            };
                            let a_val = &obj_a[orig_key_a];
                            state.diffs.push(JsonDiff {
                                path: new_path,
                                kind: "key_missing_in_b".to_string(),
                                a_type: Some(get_json_type(a_val).to_string()),
                                b_type: None,
                                a_preview: Some(json_value_preview(a_val)),
                                b_preview: None,
                            });
                            continue;
                        }
                        // Keys match at this position — recurse into value
                        let orig_key_a = a_keys_vec[i];
                        let orig_key_b = b_keys_vec[i];
                        let new_path = if path.is_empty() {
                            format!("/{}", orig_key_a)
                        } else {
                            format!("{}/{}", path, orig_key_a)
                        };
                        compare_rec(
                            &obj_a[orig_key_a],
                            &obj_b[orig_key_b],
                            &new_path,
                            options,
                            state,
                        );
                    }

                    // Report length mismatch
                    if a_key_order.len() != b_key_order.len() {
                        let actual_path = if path.is_empty() {
                            "".to_string()
                        } else {
                            path.to_string()
                        };
                        state.type_match = false;
                        state.diffs.push(JsonDiff {
                            path: actual_path,
                            kind: "object_length_changed".to_string(),
                            a_type: Some("object".to_string()),
                            b_type: Some("object".to_string()),
                            a_preview: Some(format!("{} keys", a_key_order.len())),
                            b_preview: Some(format!("{} keys", b_key_order.len())),
                        });
                    }
                }
            }
            (serde_json::Value::Array(arr_a), serde_json::Value::Array(arr_b)) => {
                if arr_a.len() != arr_b.len() {
                    state.type_match = false;
                    state.diffs.push(JsonDiff {
                        path: path.to_string(),
                        kind: "array_length_changed".to_string(),
                        a_type: Some(type_a.to_string()),
                        b_type: Some(type_b.to_string()),
                        a_preview: Some(format!("{} items", arr_a.len())),
                        b_preview: Some(format!("{} items", arr_b.len())),
                    });
                    return;
                }

                if options.ignore_array_order {
                    let mut a_sorted: Vec<_> = arr_a.iter().collect();
                    let mut b_sorted: Vec<_> = arr_b.iter().collect();
                    let cmp = |a: &&serde_json::Value, b: &&serde_json::Value| {
                        serde_json::to_string(*a)
                            .unwrap_or_default()
                            .cmp(&serde_json::to_string(*b).unwrap_or_default())
                    };
                    a_sorted.sort_by(cmp);
                    b_sorted.sort_by(cmp);
                    for (i, (va, vb)) in a_sorted.iter().zip(b_sorted.iter()).enumerate() {
                        compare_rec(va, vb, &format!("{}/{}", path, i), options, state);
                    }
                } else {
                    for (i, (va, vb)) in arr_a.iter().zip(arr_b.iter()).enumerate() {
                        compare_rec(va, vb, &format!("{}/{}", path, i), options, state);
                    }
                }
            }
            _ => {
                if a != b {
                    state.diffs.push(JsonDiff {
                        path: path.to_string(),
                        kind: "value_changed".to_string(),
                        a_type: Some(type_a.to_string()),
                        b_type: Some(type_b.to_string()),
                        a_preview: Some(json_value_preview(a)),
                        b_preview: Some(json_value_preview(b)),
                    });
                }
            }
        }
    }

    compare_rec(a, b, "", options, &mut state);

    let equal = state.diffs.is_empty();
    (equal, state.type_match, state.diffs)
}

pub fn json_canonicalize(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'text' parameter",
                None,
                Some("json_canonicalize"),
            )
        }
    };
    let sort_keys = args
        .get("sort_keys")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let indent_raw = args.get("indent").and_then(|v| v.as_i64());
    let indent = match indent_raw {
        Some(v) if v < 0 => {
            return ToolResponse::error(
                "invalid_arguments",
                &format!("indent must be 0-100 or None, got {}", v),
                Some(vec![
                    "Use a value between 0-100 or None for minified".to_string()
                ]),
                Some("json_canonicalize"),
            );
        }
        Some(v) => Some(v as usize),
        None => None,
    };
    let _ensure_ascii = args
        .get("ensure_ascii")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let detect_duplicate_keys = args
        .get("detect_duplicate_keys")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let trailing_newline = args
        .get("trailing_newline")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("json_canonicalize"),
        );
    }

    if let Some(indent_val) = indent {
        if indent_val > 100 {
            return ToolResponse::error(
                "invalid_arguments",
                &format!("indent must be 0-100 or None, got {}", indent_val),
                Some(vec![
                    "Use a value between 0-100 or None for minified".to_string()
                ]),
                Some("json_canonicalize"),
            );
        }
    }

    let mut duplicate_keys: Vec<String> = Vec::new();

    let parsed = if detect_duplicate_keys {
        match serde_json::from_str::<serde_json::Value>(text) {
            Ok(v) => {
                detect_duplicates_in_json(text, &mut duplicate_keys);
                Some(v)
            }
            Err(e) => {
                return ToolResponse::success(
                    serde_json::json!({
                        "valid": false,
                        "canonical": null,
                        "minified": null,
                        "sha256": null,
                        "duplicate_keys": [],
                        "top_level_type": null,
                        "top_level_keys": null,
                        "error": e.to_string(),
                        "line": e.line(),
                        "column": e.column(),
                    }),
                    Some("json_canonicalize"),
                )
                .with_tool("json_canonicalize");
            }
        }
    } else {
        match serde_json::from_str::<serde_json::Value>(text) {
            Ok(v) => Some(v),
            Err(e) => {
                return ToolResponse::success(
                    serde_json::json!({
                        "valid": false,
                        "canonical": null,
                        "minified": null,
                        "sha256": null,
                        "duplicate_keys": [],
                        "top_level_type": null,
                        "top_level_keys": null,
                        "error": e.to_string(),
                        "line": e.line(),
                        "column": e.column(),
                    }),
                    Some("json_canonicalize"),
                )
                .with_tool("json_canonicalize");
            }
        }
    };

    let parsed = parsed.unwrap();

    let (top_level_type, top_level_keys) = match &parsed {
        serde_json::Value::Object(map) => (
            "object".to_string(),
            Some(map.keys().cloned().collect::<Vec<_>>()),
        ),
        serde_json::Value::Array(_) => ("array".to_string(), None),
        other => (get_python_json_type(other).to_string(), None),
    };

    let canonical_data = if sort_keys {
        sort_json_keys(&parsed)
    } else {
        parsed.clone()
    };
    let ensure_ascii = _ensure_ascii;
    let canonical = if let Some(indent_val) = indent {
        let indent_str = " ".repeat(indent_val);
        let formatter = serde_json::ser::PrettyFormatter::with_indent(indent_str.as_bytes());
        let mut buf = Vec::new();
        {
            let mut serializer = serde_json::Serializer::with_formatter(&mut buf, formatter);
            if let Err(e) = canonical_data.serialize(&mut serializer) {
                return ToolResponse::error(
                    "serialization_error",
                    &e.to_string(),
                    None,
                    Some("json_canonicalize"),
                );
            }
        }
        match String::from_utf8(buf) {
            Ok(s) => s,
            Err(e) => {
                return ToolResponse::error(
                    "serialization_error",
                    &format!("invalid UTF-8 output: {}", e),
                    None,
                    Some("json_canonicalize"),
                )
            }
        }
    } else {
        // Use Python-style default separators: ", " and ": " (with spaces)
        // Python's json.dumps(indent=None) uses separators=(', ', ': ')
        struct PythonStyleFormatter;
        impl serde_json::ser::Formatter for PythonStyleFormatter {
            fn begin_array_value<W: std::io::Write + ?Sized>(
                &mut self,
                writer: &mut W,
                first: bool,
            ) -> std::io::Result<()> {
                if first {
                    Ok(())
                } else {
                    writer.write_all(b", ")
                }
            }
            fn begin_object_key<W: std::io::Write + ?Sized>(
                &mut self,
                writer: &mut W,
                first: bool,
            ) -> std::io::Result<()> {
                if first {
                    Ok(())
                } else {
                    writer.write_all(b", ")
                }
            }
            fn begin_object_value<W: std::io::Write + ?Sized>(
                &mut self,
                writer: &mut W,
            ) -> std::io::Result<()> {
                writer.write_all(b": ")
            }
        }
        let mut buf = Vec::new();
        {
            let mut serializer =
                serde_json::Serializer::with_formatter(&mut buf, PythonStyleFormatter);
            if let Err(e) = canonical_data.serialize(&mut serializer) {
                return ToolResponse::error(
                    "serialization_error",
                    &e.to_string(),
                    None,
                    Some("json_canonicalize"),
                );
            }
        }
        match String::from_utf8(buf) {
            Ok(s) => s,
            Err(e) => {
                return ToolResponse::error(
                    "serialization_error",
                    &format!("invalid UTF-8 output: {}", e),
                    None,
                    Some("json_canonicalize"),
                )
            }
        }
    };
    let canonical = if ensure_ascii {
        escape_ascii(&canonical)
    } else {
        canonical
    };
    let mut canonical_out = canonical.clone();
    if trailing_newline {
        canonical_out.push('\n');
    }

    let minified_raw = if indent.is_some() {
        // When indent is set, Python uses the same pretty-printed format for minified
        canonical.clone()
    } else {
        let compact_formatter = serde_json::ser::CompactFormatter;
        let mut buf = Vec::new();
        {
            let mut serializer =
                serde_json::Serializer::with_formatter(&mut buf, compact_formatter);
            if let Err(e) = canonical_data.serialize(&mut serializer) {
                return ToolResponse::error(
                    "serialization_error",
                    &e.to_string(),
                    None,
                    Some("json_canonicalize"),
                );
            }
        }
        match String::from_utf8(buf) {
            Ok(s) => s,
            Err(e) => {
                return ToolResponse::error(
                    "serialization_error",
                    &format!("invalid UTF-8 output: {}", e),
                    None,
                    Some("json_canonicalize"),
                )
            }
        }
    };
    let minified = if ensure_ascii {
        escape_ascii(&minified_raw)
    } else {
        minified_raw
    };

    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(canonical_out.as_bytes());
    let sha256_hash = format!("{:x}", hasher.finalize());

    ToolResponse::success(
        serde_json::json!({
            "valid": true,
            "canonical": canonical_out,
            "minified": minified,
            "sha256": sha256_hash,
            "duplicate_keys": duplicate_keys,
            "top_level_type": top_level_type,
            "top_level_keys": top_level_keys,
            "error": Value::Null,
            "line": Value::Null,
            "column": Value::Null,
        }),
        Some("json_canonicalize"),
    )
    .with_tool("json_canonicalize")
}

fn detect_duplicates_in_json(text: &str, duplicates: &mut Vec<String>) {
    let trimmed = text.trim();
    if trimmed.is_empty() || (!trimmed.starts_with('{') && !trimmed.starts_with('[')) {
        return;
    }
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape = false;
    let mut string_start: usize = 0;
    let mut keys_at_depth: Vec<std::collections::HashSet<String>> = Vec::new();

    let bytes: Vec<u8> = trimmed.bytes().collect();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if escape {
            escape = false;
            i += 1;
            continue;
        }
        if in_string {
            if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                in_string = false;
                let string_end = i;
                let mut is_key = false;
                let mut k = i + 1;
                while k < bytes.len() {
                    match bytes[k] {
                        b' ' | b'\t' | b'\n' | b'\r' => k += 1,
                        b':' => {
                            is_key = true;
                            break;
                        }
                        _ => break,
                    }
                }
                if is_key && depth > 0 {
                    let key = String::from_utf8_lossy(&bytes[string_start..string_end]).to_string();
                    let idx = (depth - 1) as usize;
                    if idx < keys_at_depth.len() && !keys_at_depth[idx].insert(key.clone()) {
                        duplicates.push(key);
                    }
                }
            }
            i += 1;
            continue;
        }
        match b {
            b'"' => {
                in_string = true;
                string_start = i + 1;
            }
            b'{' => {
                depth += 1;
                keys_at_depth.push(std::collections::HashSet::new());
            }
            b'[' => {
                depth += 1;
                keys_at_depth.push(std::collections::HashSet::new());
            }
            b'}' | b']' if depth > 0 => {
                depth -= 1;
                keys_at_depth.pop();
            }
            _ => {}
        }
        i += 1;
    }
}

fn sort_json_keys(v: &serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Object(map) => {
            let mut sorted: std::collections::BTreeMap<_, _> = std::collections::BTreeMap::new();
            for (k, val) in map.iter() {
                sorted.insert(k.clone(), sort_json_keys(val));
            }
            serde_json::Value::Object(sorted.into_iter().collect())
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(sort_json_keys).collect())
        }
        _ => v.clone(),
    }
}

fn escape_ascii(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii() {
            result.push(c);
        } else {
            for utf16_unit in c.encode_utf16(&mut [0u16; 2]) {
                result.push_str(&format!("\\u{:04x}", utf16_unit));
            }
        }
    }
    result
}

pub fn json_query(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'text' parameter",
                None,
                Some("json_query"),
            )
        }
    };
    let pointer = args.get("pointer").and_then(|v| v.as_str()).unwrap_or("");

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("json_query"),
        );
    }

    let parsed = match serde_json::from_str::<serde_json::Value>(text) {
        Ok(v) => v,
        Err(e) => {
            return ToolResponse::success(
                serde_json::json!({
                    "found": false,
                    "pointer": pointer,
                    "reason": "invalid_json",
                    "error": e.to_string(),
                    "line": e.line(),
                    "column": e.column(),
                }),
                Some("json_query"),
            )
            .with_tool("json_query")
            .with_warnings(vec![
                "json_query is deprecated; use json_extract instead".to_string()
            ])
            .with_recommended_next_tool(serde_json::json!("json_extract"));
        }
    };

    if pointer.is_empty() {
        return ToolResponse::success(
            serde_json::json!({
                "found": true,
                "pointer": pointer,
                "value": parsed,
                "type": get_json_type(&parsed),
            }),
            Some("json_query"),
        )
        .with_tool("json_query")
        .with_warnings(vec![
            "json_query is deprecated; use json_extract instead".to_string()
        ])
        .with_recommended_next_tool(serde_json::json!("json_extract"));
    }

    let tokens: Vec<&str> = pointer.split('/').filter(|s| !s.is_empty()).collect();
    let mut current = &parsed;

    for token in tokens {
        let decoded = token.replace("~1", "/").replace("~0", "~");
        match current {
            serde_json::Value::Object(map) => match map.get(&decoded) {
                Some(v) => current = v,
                None => {
                    return ToolResponse::success(
                        serde_json::json!({
                            "found": false,
                            "pointer": pointer,
                            "missing_at": format!("/{}", token),
                            "reason": "key_not_found",
                        }),
                        Some("json_query"),
                    )
                    .with_tool("json_query")
                    .with_warnings(vec![
                        "json_query is deprecated; use json_extract instead".to_string()
                    ])
                    .with_recommended_next_tool(serde_json::json!("json_extract"));
                }
            },
            serde_json::Value::Array(arr) => match decoded.parse::<usize>() {
                Ok(idx) if idx < arr.len() => current = &arr[idx],
                _ => {
                    return ToolResponse::success(
                        serde_json::json!({
                            "found": false,
                            "pointer": pointer,
                            "reason": "index_out_of_range",
                        }),
                        Some("json_query"),
                    )
                    .with_tool("json_query")
                    .with_warnings(vec![
                        "json_query is deprecated; use json_extract instead".to_string()
                    ])
                    .with_recommended_next_tool(serde_json::json!("json_extract"));
                }
            },
            _ => {
                return ToolResponse::success(
                    serde_json::json!({
                        "found": false,
                        "pointer": pointer,
                        "reason": "invalid_pointer_syntax",
                    }),
                    Some("json_query"),
                )
                .with_tool("json_query")
                .with_warnings(vec![
                    "json_query is deprecated; use json_extract instead".to_string()
                ])
                .with_recommended_next_tool(serde_json::json!("json_extract"));
            }
        }
    }

    ToolResponse::success(
        serde_json::json!({
            "found": true,
            "pointer": pointer,
            "value": *current,
            "type": get_json_type(current),
        }),
        Some("json_query"),
    )
    .with_tool("json_query")
    .with_warnings(vec![
        "json_query is deprecated; use json_extract instead".to_string()
    ])
    .with_recommended_next_tool(serde_json::json!("json_extract"))
}

pub fn list_dedupe(args: &Value) -> ToolResponse {
    let items = match args.get("items").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                &format!(
                    "items must be a list, got {}",
                    json_type_name(args.get("items").unwrap_or(&Value::Null))
                ),
                None,
                Some("list_dedupe"),
            )
        }
    };
    if items.len() > MAX_LIST_ITEMS {
        return ToolResponse::error(
            "input_too_large",
            &format!("items length {} exceeds {}", items.len(), MAX_LIST_ITEMS),
            None,
            Some("list_dedupe"),
        );
    }
    let normalization = args
        .get("normalization")
        .and_then(|v| v.as_str())
        .unwrap_or("NFC");
    let casefold = args
        .get("casefold")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let stable = args.get("stable").and_then(|v| v.as_bool()).unwrap_or(true);

    // Validate all items are strings
    let non_str_indices: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, v)| !v.is_string())
        .map(|(i, _)| i)
        .collect();
    if !non_str_indices.is_empty() {
        return ToolResponse::error(
            "invalid_arguments",
            "All items elements must be strings",
            Some(vec![format!(
                "Non-string items at indices: {:?}",
                &non_str_indices[..5.min(non_str_indices.len())]
            )]),
            Some("list_dedupe"),
        );
    }

    let oversized_indices: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, v)| {
            v.as_str()
                .is_some_and(|s| s.chars().count() > MAX_TEXT_LENGTH)
        })
        .map(|(i, _)| i)
        .collect();
    if !oversized_indices.is_empty() {
        return ToolResponse::error(
            "input_too_large",
            &format!("items exceed max length {}", MAX_TEXT_LENGTH),
            Some(vec![format!(
                "Oversized items at indices: {:?}",
                &oversized_indices[..5.min(oversized_indices.len())]
            )]),
            Some("list_dedupe"),
        );
    }

    let valid_normalizations = ["raw", "NFC", "NFD", "NFKC", "NFKD"];
    if !valid_normalizations.contains(&normalization) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported normalization form: {}", normalization),
            Some(vec![format!(
                "Use one of: {}",
                valid_normalizations.join(", ")
            )]),
            Some("list_dedupe"),
        );
    }

    let original_count = items.len();

    let normalize_item = |item: &Value| -> String {
        let s = match item.as_str() {
            Some(st) => st.to_string(),
            None => item.to_string(),
        };
        let mut compare_val = if casefold { unicode_casefold(&s) } else { s };
        if normalization != "raw" {
            compare_val = match normalization {
                "NFD" => compare_val.nfd().collect::<String>(),
                "NFKC" => compare_val.nfkc().collect::<String>(),
                "NFKD" => compare_val.nfkd().collect::<String>(),
                _ => compare_val.nfc().collect::<String>(),
            };
        }
        compare_val
    };

    let result: Vec<serde_json::Value> = if stable {
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut out: Vec<serde_json::Value> = Vec::new();
        for item in items {
            let key = normalize_item(item);
            if seen.insert(key) {
                out.push(item.clone());
            }
        }
        out
    } else {
        let mut unique_map: std::collections::HashMap<String, serde_json::Value> =
            std::collections::HashMap::new();
        for item in items {
            let key = normalize_item(item);
            unique_map.entry(key).or_insert_with(|| item.clone());
        }
        unique_map.into_values().collect()
    };

    let deduped_count = result.len();
    ToolResponse::success(
        serde_json::json!({
            "items": result,
            "original_count": original_count,
            "deduped_count": deduped_count,
            "duplicates_removed": original_count - deduped_count,
        }),
        Some("list_dedupe"),
    )
    .with_tool("list_dedupe")
}

pub fn list_sort(args: &Value) -> ToolResponse {
    let items = match args.get("items").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                &format!(
                    "items must be a list, got {}",
                    json_type_name(args.get("items").unwrap_or(&Value::Null))
                ),
                None,
                Some("list_sort"),
            )
        }
    };
    if items.len() > MAX_LIST_ITEMS {
        return ToolResponse::error(
            "input_too_large",
            &format!("items length {} exceeds {}", items.len(), MAX_LIST_ITEMS),
            None,
            Some("list_sort"),
        );
    }
    let normalization = args
        .get("normalization")
        .and_then(|v| v.as_str())
        .unwrap_or("NFC");
    let casefold = args
        .get("casefold")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let reverse = args
        .get("reverse")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let stable = args.get("stable").and_then(|v| v.as_bool()).unwrap_or(true);

    // Validate all items are strings
    let non_str_indices: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, v)| !v.is_string())
        .map(|(i, _)| i)
        .collect();
    if !non_str_indices.is_empty() {
        return ToolResponse::error(
            "invalid_arguments",
            "All items elements must be strings",
            Some(vec![format!(
                "Non-string items at indices: {:?}",
                &non_str_indices[..5.min(non_str_indices.len())]
            )]),
            Some("list_sort"),
        );
    }

    let oversized_indices: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, v)| {
            v.as_str()
                .is_some_and(|s| s.chars().count() > MAX_TEXT_LENGTH)
        })
        .map(|(i, _)| i)
        .collect();
    if !oversized_indices.is_empty() {
        return ToolResponse::error(
            "input_too_large",
            &format!("items exceed max length {}", MAX_TEXT_LENGTH),
            Some(vec![format!(
                "Oversized items at indices: {:?}",
                &oversized_indices[..5.min(oversized_indices.len())]
            )]),
            Some("list_sort"),
        );
    }

    let valid_normalizations = ["raw", "NFC", "NFD", "NFKC", "NFKD"];
    if !valid_normalizations.contains(&normalization) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported normalization form: {}", normalization),
            Some(vec![format!(
                "Use one of: {}",
                valid_normalizations.join(", ")
            )]),
            Some("list_sort"),
        );
    }

    let mut paired: Vec<(String, serde_json::Value)> = Vec::new();
    for item in items {
        let s = match item.as_str() {
            Some(st) => st.to_string(),
            None => item.to_string(),
        };
        let mut key = if casefold { unicode_casefold(&s) } else { s };
        if normalization != "raw" {
            key = match normalization {
                "NFD" => key.nfd().collect::<String>(),
                "NFKC" => key.nfkc().collect::<String>(),
                "NFKD" => key.nfkd().collect::<String>(),
                _ => key.nfc().collect::<String>(),
            };
        }
        paired.push((key, item.clone()));
    }

    if stable {
        paired.sort_by(|a, b| a.0.cmp(&b.0));
    } else {
        paired.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    }
    if reverse {
        paired.reverse();
    }

    let original_count = items.len();
    let sorted_items: Vec<serde_json::Value> = paired.into_iter().map(|(_, v)| v).collect();
    let sorted_count = sorted_items.len();

    ToolResponse::success(
        serde_json::json!({
            "items": sorted_items,
            "original_count": original_count,
            "sorted_count": sorted_count,
        }),
        Some("list_sort"),
    )
    .with_tool("list_sort")
}

pub fn glob_match_tool(args: &Value) -> ToolResponse {
    let pattern = match args.get("pattern").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'pattern' parameter",
                None,
                Some("glob_match"),
            )
        }
    };
    let path = match args.get("path").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'path' parameter",
                None,
                Some("glob_match"),
            )
        }
    };
    let platform = args
        .get("platform")
        .and_then(|v| v.as_str())
        .unwrap_or("posix");
    let case_sensitive = args
        .get("case_sensitive")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    if pattern.chars().count() > MAX_TEXT_LENGTH || path.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            "Pattern or path exceeds maximum length",
            None,
            Some("glob_match"),
        );
    }

    let valid_platforms = ["posix", "windows"];
    if !valid_platforms.contains(&platform) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported platform: {}", platform),
            Some(vec![format!("Use one of: {}", valid_platforms.join(", "))]),
            Some("glob_match"),
        );
    }

    let result = glob_match(pattern, path, platform, case_sensitive);

    ToolResponse::success(
        serde_json::json!({
            "matches": result.matches,
            "normalized_pattern": result.normalized_pattern,
            "normalized_path": result.normalized_path,
            "matched_segment": result.matched_segment,
            "unmatched_segment": result.unmatched_segment,
            "summary": result.summary,
        }),
        Some("glob_match"),
    )
    .with_tool("glob_match")
}

// Tier 2 Tools

pub fn identifier_analyze(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'text' parameter",
                None,
                Some("identifier_analyze"),
            )
        }
    };
    let languages = args.get("languages").and_then(|v| v.as_array()).map(|arr| {
        arr.iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect::<Vec<_>>()
    });
    let detail = args
        .get("detail")
        .and_then(|v| v.as_str())
        .unwrap_or("normal");

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("identifier_analyze"),
        );
    }

    let valid_details = ["summary", "normal", "full"];
    if !valid_details.contains(&detail) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported detail level: {}", detail),
            Some(vec![format!("Use one of: {}", valid_details.join(", "))]),
            Some("identifier_analyze"),
        );
    }

    if let Some(ref langs) = languages {
        let valid_languages = ["python", "rust", "javascript", "env"];
        for lang in langs {
            if !valid_languages.contains(&lang.as_str()) {
                return ToolResponse::error(
                    "invalid_arguments",
                    &format!("Unsupported language: {}", lang),
                    Some(vec![format!("Use one of: {}", valid_languages.join(", "))]),
                    Some("identifier_analyze"),
                );
            }
        }
    }

    let lang_refs: Option<Vec<&str>> = languages
        .as_ref()
        .map(|v| v.iter().map(|s| s.as_str()).collect());
    let result = crate::text::identifier_analyze(text, lang_refs);

    if detail == "summary" {
        ToolResponse::success(
            serde_json::json!({
                "text": result.text,
                "classification": result.classification,
                "python_valid": result.python_valid,
                "python_keyword": result.python_keyword,
                "env_valid": result.env_valid,
                "summary": result.summary,
            }),
            Some("identifier_analyze"),
        )
        .with_tool("identifier_analyze")
    } else {
        ToolResponse::success(
            serde_json::json!({
                "text": result.text,
                "classification": result.classification,
                "python_valid": result.python_valid,
                "python_keyword": result.python_keyword,
                "rust_valid": result.rust_valid,
                "javascript_valid": result.javascript_valid,
                "env_valid": result.env_valid,
                "suggestions": result.suggestions,
                "warnings": result.warnings,
                "summary": result.summary,
            }),
            Some("identifier_analyze"),
        )
        .with_tool("identifier_analyze")
    }
}

pub fn identifier_inspect(args: &Value) -> ToolResponse {
    let identifiers_val = args.get("identifiers");
    let identifiers = match identifiers_val.and_then(|v| v.as_array()) {
        Some(arr) => {
            // Validate all elements are strings
            for item in arr.iter() {
                if !item.is_string() {
                    return ToolResponse::error(
                        "invalid_arguments",
                        &format!(
                            "Each identifier must be a string, got {}",
                            json_type_name(item)
                        ),
                        None,
                        Some("identifier_inspect"),
                    );
                }
            }
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect::<Vec<_>>()
        }
        None => {
            let type_name = match identifiers_val {
                Some(v) => json_type_name(v),
                None => "NoneType",
            };
            return ToolResponse::error(
                "invalid_arguments",
                &format!("identifiers must be a list, got {}", type_name),
                None,
                Some("identifier_inspect"),
            );
        }
    };
    let language = args
        .get("language")
        .and_then(|v| v.as_str())
        .unwrap_or("generic");
    let normalization = args
        .get("normalization")
        .and_then(|v| v.as_str())
        .unwrap_or("NFC");
    let casefold = args
        .get("casefold")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let check_confusables = args
        .get("check_confusables")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    if identifiers.len() > MAX_LIST_ITEMS {
        return ToolResponse::error(
            "input_too_large",
            &format!(
                "Number of identifiers {} exceeds MAX_LIST_ITEMS {}",
                identifiers.len(),
                MAX_LIST_ITEMS
            ),
            None,
            Some("identifier_inspect"),
        );
    }

    for ident in &identifiers {
        if ident.chars().count() > MAX_TEXT_LENGTH {
            return ToolResponse::error(
                "input_too_large",
                &format!(
                    "Identifier length {} exceeds MAX_TEXT_LENGTH {}",
                    ident.chars().count(),
                    MAX_TEXT_LENGTH
                ),
                Some(vec![format!(
                    "Maximum identifier length is {}",
                    MAX_TEXT_LENGTH
                )]),
                Some("identifier_inspect"),
            );
        }
    }

    let valid_languages = [
        "generic",
        "python",
        "rust",
        "javascript",
        "typescript",
        "json_key",
    ];
    if !valid_languages.contains(&language) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported language: {}", language),
            Some(vec![format!("Use one of: {}", valid_languages.join(", "))]),
            Some("identifier_inspect"),
        );
    }

    let valid_normalizations = ["raw", "NFC", "NFD", "NFKC", "NFKD"];
    if !valid_normalizations.contains(&normalization) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported normalization form: {}", normalization),
            Some(vec![format!(
                "Use one of: {}",
                valid_normalizations.join(", ")
            )]),
            Some("identifier_inspect"),
        );
    }

    let result = crate::text::identifier_inspect(
        &identifiers,
        language,
        normalization,
        casefold,
        check_confusables,
    );

    let has_collisions = !result.collisions.is_empty();
    let has_invalid = result.identifiers.iter().any(|id| !id.valid);

    let mut envelope_findings: Vec<serde_json::Value> = Vec::new();
    for ident_info in &result.identifiers {
        for warning in &ident_info.warnings {
            envelope_findings.push(serde_json::json!({
                "code": "IDENT_WARNING",
                "severity": "warn",
                "message": warning,
                "details": {"identifier": ident_info.raw},
            }));
        }
    }
    for collision in &result.collisions {
        envelope_findings.push(serde_json::json!({
            "code": "IDENT_COLLISION",
            "severity": "warn",
            "message": format!("{}: '{}' collides with '{}'", collision.kind, collision.a, collision.b),
            "details": {"kind": collision.kind, "a": collision.a, "b": collision.b},
        }));
    }

    let mut resp = ToolResponse::success(
        serde_json::json!({
            "identifiers": result.identifiers,
            "collisions": result.collisions,
        }),
        Some("identifier_inspect"),
    )
    .with_tool("identifier_inspect");

    if !envelope_findings.is_empty() {
        resp = resp.with_findings(envelope_findings);
    }

    if has_collisions {
        resp = resp.with_machine_code("IDENT_COLLISIONS");
    } else if has_invalid {
        resp = resp.with_machine_code("IDENT_INVALID");
    }
    resp
}

pub fn path_analyze(args: &Value) -> ToolResponse {
    let path = match args.get("path").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'path' parameter",
                None,
                Some("path_analyze"),
            )
        }
    };
    let style = args.get("style").and_then(|v| v.as_str()).unwrap_or("auto");
    let detail = args
        .get("detail")
        .and_then(|v| v.as_str())
        .unwrap_or("normal");

    if path.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!(
                "Path length {} exceeds MAX_TEXT_LENGTH {}",
                path.chars().count(),
                MAX_TEXT_LENGTH
            ),
            None,
            Some("path_analyze"),
        );
    }

    let valid_styles = ["auto", "posix", "windows"];
    if !valid_styles.contains(&style) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported style: {}", style),
            Some(vec![format!("Use one of: {}", valid_styles.join(", "))]),
            Some("path_analyze"),
        );
    }

    let valid_details = ["summary", "normal", "full"];
    if !valid_details.contains(&detail) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported detail level: {}", detail),
            Some(vec![format!("Use one of: {}", valid_details.join(", "))]),
            Some("path_analyze"),
        );
    }

    let result = crate::text::path_analyze(path, style);

    let has_traversal = result.has_traversal;
    let is_hidden = result.hidden;
    let mut envelope_findings: Vec<serde_json::Value> = Vec::new();
    if has_traversal {
        envelope_findings.push(serde_json::json!({
            "code": "PATH_TRAVERSAL",
            "severity": "warn",
            "message": "Path contains parent directory traversal (..)",
            "details": {"normalized_lexical": result.normalized_lexical},
        }));
    }
    if is_hidden {
        envelope_findings.push(serde_json::json!({
            "code": "PATH_HIDDEN",
            "severity": "info",
            "message": "Path starts with a dot (hidden file/directory)",
        }));
    }
    let machine_code = if has_traversal {
        Some("PATH_HAS_TRAVERSAL")
    } else if is_hidden {
        Some("PATH_IS_HIDDEN")
    } else {
        None
    };

    if detail == "summary" {
        let mut resp = ToolResponse::success(
            serde_json::json!({
                "summary": result.summary,
                "style": result.style,
                "absolute": result.absolute,
                "hidden": result.hidden,
                "has_traversal": result.has_traversal,
                "warnings": result.warnings,
            }),
            Some("path_analyze"),
        )
        .with_tool("path_analyze");
        if !envelope_findings.is_empty() {
            resp = resp.with_findings(envelope_findings.clone());
        }
        if let Some(code) = machine_code {
            resp = resp.with_machine_code(code);
        }
        resp
    } else {
        let mut resp = ToolResponse::success(
            serde_json::json!({
                "input": result.input,
                "style": result.style,
                "absolute": result.absolute,
                "has_traversal": result.has_traversal,
                "components": result.components,
                "parent": result.parent,
                "name": result.name,
                "stem": result.stem,
                "suffix": result.suffix,
                "suffixes": result.suffixes,
                "hidden": result.hidden,
                "normalized_lexical": result.normalized_lexical,
                "warnings": result.warnings,
                "summary": result.summary,
            }),
            Some("path_analyze"),
        )
        .with_tool("path_analyze");
        if !envelope_findings.is_empty() {
            resp = resp.with_findings(envelope_findings);
        }
        if let Some(code) = machine_code {
            resp = resp.with_machine_code(code);
        }
        resp
    }
}

pub fn path_compare(args: &Value) -> ToolResponse {
    let left = match args.get("left").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'left' parameter",
                None,
                Some("path_compare"),
            )
        }
    };
    let right = match args.get("right").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'right' parameter",
                None,
                Some("path_compare"),
            )
        }
    };
    let platform = args
        .get("platform")
        .and_then(|v| v.as_str())
        .unwrap_or("posix");
    let case_sensitive = args
        .get("case_sensitive")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let normalize_separators = args
        .get("normalize_separators")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let collapse_dot_segments = args
        .get("collapse_dot_segments")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    if left.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            "Left path exceeds MAX_TEXT_LENGTH",
            None,
            Some("path_compare"),
        );
    }
    if right.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            "Right path exceeds MAX_TEXT_LENGTH",
            None,
            Some("path_compare"),
        );
    }

    let valid_platforms = ["posix", "windows"];
    if !valid_platforms.contains(&platform) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported platform: {}", platform),
            Some(vec![format!("Use one of: {}", valid_platforms.join(", "))]),
            Some("path_compare"),
        );
    }

    let result = crate::text::path_compare(
        left,
        right,
        platform,
        case_sensitive,
        normalize_separators,
        collapse_dot_segments,
    );

    ToolResponse::success(
        serde_json::json!({
            "equal": result.equal,
            "left_normalized": result.left_normalized,
            "right_normalized": result.right_normalized,
            "differences": result.differences,
            "findings": result.findings,
        }),
        Some("path_compare"),
    )
    .with_tool("path_compare")
}

pub fn path_scope_check(args: &Value) -> ToolResponse {
    let root = match args.get("root").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'root' parameter",
                None,
                Some("path_scope_check"),
            )
        }
    };
    let target = match args.get("target").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'target' parameter",
                None,
                Some("path_scope_check"),
            )
        }
    };
    let platform = args
        .get("platform")
        .and_then(|v| v.as_str())
        .unwrap_or("posix");
    let case_sensitive = args
        .get("case_sensitive")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    if root.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            "Root path exceeds MAX_TEXT_LENGTH",
            None,
            Some("path_scope_check"),
        );
    }
    if target.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            "Target path exceeds MAX_TEXT_LENGTH",
            None,
            Some("path_scope_check"),
        );
    }

    let valid_platforms = ["posix", "windows"];
    if !valid_platforms.contains(&platform) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported platform: {}", platform),
            Some(vec![format!("Use one of: {}", valid_platforms.join(", "))]),
            Some("path_scope_check"),
        );
    }

    let result = crate::text::path_scope_check(root, target, platform, case_sensitive);

    ToolResponse::success(
        serde_json::json!({
            "inside_root": result.inside_root,
            "root_normalized": result.root_normalized,
            "target_normalized": result.target_normalized,
            "relative_path": result.relative_path,
            "escapes_via_dotdot": result.escapes_via_dotdot,
            "absolute_target": result.absolute_target,
            "findings": result.findings,
        }),
        Some("path_scope_check"),
    )
    .with_tool("path_scope_check")
}

pub fn shell_split(args: &Value) -> ToolResponse {
    let command = match args.get("command").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'command' parameter",
                None,
                Some("shell_split"),
            )
        }
    };
    let shell = args
        .get("shell")
        .and_then(|v| v.as_str())
        .unwrap_or("posix");
    let detect_risky_features = args
        .get("detect_risky_features")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    if command.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Command exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("shell_split"),
        );
    }

    let valid_shells = ["posix"];
    if !valid_shells.contains(&shell) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported shell: {}", shell),
            Some(vec!["Use one of: posix".to_string()]),
            Some("shell_split"),
        );
    }

    let result = crate::text::shell_split(command, shell, detect_risky_features);

    ToolResponse::success(
        serde_json::json!({
            "parse_ok": result.parse_ok,
            "argv": result.argv,
            "argc": result.argc,
            "features": {
                "has_pipe": result.features.has_pipe,
                "has_redirection": result.features.has_redirection,
                "has_command_substitution": result.features.has_command_substitution,
                "has_variable_expansion": result.features.has_variable_expansion,
                "has_glob_pattern": result.features.has_glob_pattern,
                "has_control_operator": result.features.has_control_operator,
                "has_unbalanced_quotes": result.features.has_unbalanced_quotes,
            },
            "findings": result.findings,
        }),
        Some("shell_split"),
    )
    .with_tool("shell_split")
}

pub fn shell_quote_join(args: &Value) -> ToolResponse {
    let argv_raw = match args.get("argv").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'argv' parameter",
                None,
                Some("shell_quote_join"),
            )
        }
    };
    let shell = args
        .get("shell")
        .and_then(|v| v.as_str())
        .unwrap_or("posix");

    if argv_raw.len() > MAX_LIST_ITEMS {
        return ToolResponse::error(
            "input_too_large",
            &format!("argv length {} exceeds MAX_LIST_ITEMS", argv_raw.len()),
            None,
            Some("shell_quote_join"),
        );
    }

    let non_str_indices: Vec<usize> = argv_raw
        .iter()
        .enumerate()
        .filter(|(_, v)| !v.is_string())
        .map(|(i, _)| i)
        .collect();
    if !non_str_indices.is_empty() {
        return ToolResponse::error(
            "invalid_arguments",
            "All argv elements must be strings",
            Some(vec![format!(
                "Non-string items at indices: {:?}",
                &non_str_indices[..5.min(non_str_indices.len())]
            )]),
            Some("shell_quote_join"),
        );
    }
    let oversized_indices: Vec<usize> = argv_raw
        .iter()
        .enumerate()
        .filter(|(_, v)| {
            v.as_str()
                .is_some_and(|s| s.chars().count() > MAX_TEXT_LENGTH)
        })
        .map(|(i, _)| i)
        .collect();
    if !oversized_indices.is_empty() {
        return ToolResponse::error(
            "input_too_large",
            &format!("argv items exceed max length {}", MAX_TEXT_LENGTH),
            Some(vec![format!(
                "Oversized items at indices: {:?}",
                &oversized_indices[..5.min(oversized_indices.len())]
            )]),
            Some("shell_quote_join"),
        );
    }
    let mut argv: Vec<String> = Vec::with_capacity(argv_raw.len());
    for v in argv_raw.iter() {
        if let Some(s) = v.as_str() {
            argv.push(s.to_string());
        }
    }

    let valid_shells = ["posix"];
    if !valid_shells.contains(&shell) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported shell: {}", shell),
            Some(vec![format!("Use one of: {}", valid_shells.join(", "))]),
            Some("shell_quote_join"),
        );
    }

    let result = crate::text::shell_quote_join(&argv, shell);

    ToolResponse::success(
        serde_json::json!({
            "command": result.command,
            "roundtrip_ok": result.roundtrip_ok,
            "findings": result.findings,
        }),
        Some("shell_quote_join"),
    )
    .with_tool("shell_quote_join")
}

pub fn argv_compare(args: &Value) -> ToolResponse {
    let left_command = args.get("left_command").and_then(|v| v.as_str());
    let right_command = args.get("right_command").and_then(|v| v.as_str());
    let left_argv = match args.get("left_argv").and_then(|v| v.as_array()) {
        Some(arr) => {
            if arr.len() > MAX_LIST_ITEMS {
                return ToolResponse::error(
                    "input_too_large",
                    &format!("left_argv length {} exceeds {}", arr.len(), MAX_LIST_ITEMS),
                    None,
                    Some("argv_compare"),
                );
            }
            let non_str: Vec<usize> = arr
                .iter()
                .enumerate()
                .filter(|(_, v)| !v.is_string())
                .map(|(i, _)| i)
                .collect();
            if !non_str.is_empty() {
                return ToolResponse::error(
                    "invalid_arguments",
                    "All left_argv elements must be strings",
                    Some(vec![format!(
                        "Non-string items at indices: {:?}",
                        &non_str[..5.min(non_str.len())]
                    )]),
                    Some("argv_compare"),
                );
            }
            let oversized: Vec<usize> = arr
                .iter()
                .enumerate()
                .filter(|(_, v)| {
                    v.as_str()
                        .is_some_and(|s| s.chars().count() > MAX_TEXT_LENGTH)
                })
                .map(|(i, _)| i)
                .collect();
            if !oversized.is_empty() {
                return ToolResponse::error(
                    "input_too_large",
                    &format!("left_argv items exceed max length {}", MAX_TEXT_LENGTH),
                    Some(vec![format!(
                        "Oversized items at indices: {:?}",
                        &oversized[..5.min(oversized.len())]
                    )]),
                    Some("argv_compare"),
                );
            }
            Some(
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect::<Vec<_>>(),
            )
        }
        None => None,
    };
    let right_argv = match args.get("right_argv").and_then(|v| v.as_array()) {
        Some(arr) => {
            if arr.len() > MAX_LIST_ITEMS {
                return ToolResponse::error(
                    "input_too_large",
                    &format!("right_argv length {} exceeds {}", arr.len(), MAX_LIST_ITEMS),
                    None,
                    Some("argv_compare"),
                );
            }
            let non_str: Vec<usize> = arr
                .iter()
                .enumerate()
                .filter(|(_, v)| !v.is_string())
                .map(|(i, _)| i)
                .collect();
            if !non_str.is_empty() {
                return ToolResponse::error(
                    "invalid_arguments",
                    "All right_argv elements must be strings",
                    Some(vec![format!(
                        "Non-string items at indices: {:?}",
                        &non_str[..5.min(non_str.len())]
                    )]),
                    Some("argv_compare"),
                );
            }
            let oversized: Vec<usize> = arr
                .iter()
                .enumerate()
                .filter(|(_, v)| {
                    v.as_str()
                        .is_some_and(|s| s.chars().count() > MAX_TEXT_LENGTH)
                })
                .map(|(i, _)| i)
                .collect();
            if !oversized.is_empty() {
                return ToolResponse::error(
                    "input_too_large",
                    &format!("right_argv items exceed max length {}", MAX_TEXT_LENGTH),
                    Some(vec![format!(
                        "Oversized items at indices: {:?}",
                        &oversized[..5.min(oversized.len())]
                    )]),
                    Some("argv_compare"),
                );
            }
            Some(
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect::<Vec<_>>(),
            )
        }
        None => None,
    };
    let shell = args
        .get("shell")
        .and_then(|v| v.as_str())
        .unwrap_or("posix");

    let valid_shells = ["posix"];
    if !valid_shells.contains(&shell) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported shell: {}", shell),
            Some(vec![format!("Use one of: {}", valid_shells.join(", "))]),
            Some("argv_compare"),
        );
    }

    // XOR validation: each side must be either a *_command OR an *_argv, not both (and not neither).
    let left_both = left_command.is_some() == left_argv.is_some();
    let right_both = right_command.is_some() == right_argv.is_some();
    if left_both {
        let msg = if left_command.is_some() && left_argv.is_some() {
            "Provide exactly one of left_command or left_argv, not both"
        } else {
            "Provide exactly one of left_command or left_argv"
        };
        return ToolResponse::error("invalid_arguments", msg, None, Some("argv_compare"));
    }
    if right_both {
        let msg = if right_command.is_some() && right_argv.is_some() {
            "Provide exactly one of right_command or right_argv, not both"
        } else {
            "Provide exactly one of right_command or right_argv"
        };
        return ToolResponse::error("invalid_arguments", msg, None, Some("argv_compare"));
    }

    if let Some(cmd) = left_command {
        if cmd.chars().count() > MAX_TEXT_LENGTH {
            return ToolResponse::error(
                "input_too_large",
                "Left command exceeds MAX_TEXT_LENGTH",
                None,
                Some("argv_compare"),
            );
        }
    }
    if let Some(cmd) = right_command {
        if cmd.chars().count() > MAX_TEXT_LENGTH {
            return ToolResponse::error(
                "input_too_large",
                "Right command exceeds MAX_TEXT_LENGTH",
                None,
                Some("argv_compare"),
            );
        }
    }

    let left_ref = left_command;
    let right_ref = right_command;
    let left_argv_ref = left_argv.as_deref();
    let right_argv_ref = right_argv.as_deref();

    let result =
        crate::text::argv_compare(left_ref, right_ref, left_argv_ref, right_argv_ref, shell);

    ToolResponse::success(
        serde_json::json!({
            "argv_equal": result.argv_equal,
            "left_argv": result.left_argv,
            "right_argv": result.right_argv,
            "first_difference": result.first_difference,
            "findings": result.findings,
        }),
        Some("argv_compare"),
    )
    .with_tool("argv_compare")
}

pub fn markdown_structure(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'text' parameter",
                None,
                Some("markdown_structure"),
            )
        }
    };
    let include_sections = args
        .get("include_sections")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let include_links = args
        .get("include_links")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let include_code_fences = args
        .get("include_code_fences")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let include_html_comments = args
        .get("include_html_comments")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("markdown_structure"),
        );
    }

    let result = crate::text::markdown_structure(
        text,
        include_sections,
        include_links,
        include_code_fences,
        include_html_comments,
    );

    ToolResponse::success(
        serde_json::json!({
            "headings": result.headings,
            "code_fences": result.code_fences,
            "links": result.links,
            "html_comments": result.html_comments,
            "frontmatter": result.frontmatter,
            "tables_detected": result.tables_detected,
            "findings": result.findings,
        }),
        Some("markdown_structure"),
    )
    .with_tool("markdown_structure")
}

pub fn code_fence_extract(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'text' parameter",
                None,
                Some("code_fence_extract"),
            )
        }
    };
    let language = args.get("language").and_then(|v| v.as_str());
    let include_content = args
        .get("include_content")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("code_fence_extract"),
        );
    }

    let result = crate::text::code_fence_extract(text, language, include_content);

    ToolResponse::success(
        serde_json::json!({
            "blocks": result.blocks,
            "unclosed_fences": result.unclosed_fences,
            "findings": result.findings,
        }),
        Some("code_fence_extract"),
    )
    .with_tool("code_fence_extract")
}

pub fn dotenv_validate(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'text' parameter",
                None,
                Some("dotenv_validate"),
            )
        }
    };
    let allow_export = args
        .get("allow_export")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let key_pattern = args
        .get("key_pattern")
        .and_then(|v| v.as_str())
        .unwrap_or("^[A-Za-z_][A-Za-z0-9_]*$");
    let duplicate_policy = args
        .get("duplicate_policy")
        .and_then(|v| v.as_str())
        .unwrap_or("warn");

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("dotenv_validate"),
        );
    }

    let valid_policies = ["warn", "error", "allow"];
    if !valid_policies.contains(&duplicate_policy) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported duplicate_policy: {}", duplicate_policy),
            Some(vec![format!("Use one of: {}", valid_policies.join(", "))]),
            Some("dotenv_validate"),
        );
    }

    if key_pattern.len() > 1000 {
        return ToolResponse::error(
            "input_too_large",
            "key_pattern exceeds 1000 chars",
            None,
            Some("dotenv_validate"),
        );
    }

    let safety = crate::text::regex_safety::regex_safety_check(key_pattern);
    if !safety.valid_pattern {
        return ToolResponse::error(
            "invalid_arguments",
            "key_pattern is not a valid regular expression",
            Some(vec!["Fix the regex syntax in key_pattern".to_string()]),
            Some("dotenv_validate"),
        );
    }
    if safety.risk == "medium" || safety.risk == "high" {
        return ToolResponse::error(
            "unsafe_pattern",
            &format!(
                "key_pattern has {} risk of catastrophic backtracking",
                safety.risk
            ),
            Some(vec![
                "Use a simpler key_pattern or break it into smaller parts".to_string(),
                "Use the regex_safety_check tool for detailed analysis and suggestions".to_string(),
            ]),
            Some("dotenv_validate"),
        );
    }

    // Reject inline flags in pattern (e.g., (?s), (?i), (?x))
    let inline_flag_re = regex::Regex::new(r"\(\?([aiLmsux]+)\)").unwrap();
    if let Some(m) = inline_flag_re.find(key_pattern) {
        return ToolResponse::error(
    "unsafe_pattern",
    &format!("key_pattern contains inline flags '{}'; use the explicit boolean parameters instead", m.as_str()),
    Some(vec!["Remove inline flags and use ignore_case, multiline, dotall parameters".to_string()]),
    Some("dotenv_validate")
);
    }

    // Run validation on a dedicated thread with timeout to prevent ReDoS from
    // hanging the server (matching Python's multiprocessing.Process isolation).
    let text_owned = text.to_string();
    let key_pattern_owned = key_pattern.to_string();
    let duplicate_policy_owned = duplicate_policy.to_string();
    let result = match run_with_timeout(Duration::from_secs(REGEX_TIMEOUT_SECONDS), move || {
        crate::text::dotenv_validate(
            &text_owned,
            allow_export,
            &key_pattern_owned,
            &duplicate_policy_owned,
        )
    }) {
        Ok(r) => r,
        Err(_timeout) => {
            return ToolResponse::error(
                "timeout",
                "Regex execution exceeded time limit (possible ReDoS)",
                Some(vec!["Try a simpler key_pattern or shorter text".to_string()]),
                Some("dotenv_validate"),
            )
        }
    };

    ToolResponse::success(
        serde_json::json!({
            "parse_ok": result.parse_ok,
            "entries": result.entries,
            "duplicates": result.duplicates,
            "invalid_lines": result.invalid_lines,
            "requires_quoting": result.requires_quoting,
            "contains_expansion_syntax": result.contains_expansion_syntax,
            "findings": result.findings,
        }),
        Some("dotenv_validate"),
    )
    .with_tool("dotenv_validate")
}

pub fn ini_validate(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'text' parameter",
                None,
                Some("ini_validate"),
            )
        }
    };
    let duplicate_policy = args
        .get("duplicate_policy")
        .and_then(|v| v.as_str())
        .unwrap_or("warn");

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("ini_validate"),
        );
    }

    let valid_policies = ["warn", "error", "allow"];
    if !valid_policies.contains(&duplicate_policy) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported duplicate_policy: {}", duplicate_policy),
            Some(vec![format!("Use one of: {}", valid_policies.join(", "))]),
            Some("ini_validate"),
        );
    }

    let result = crate::text::ini_validate(text, duplicate_policy);

    ToolResponse::success(
        serde_json::json!({
            "parse_ok": result.parse_ok,
            "sections": result.sections,
            "keys_by_section": result.keys_by_section,
            "duplicates": result.duplicates,
            "invalid_lines": result.invalid_lines,
            "findings": result.findings,
        }),
        Some("ini_validate"),
    )
    .with_tool("ini_validate")
}

pub fn patch_apply_check(args: &Value) -> ToolResponse {
    let original_text_val = args.get("original_text");
    let original_text = match original_text_val.and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            let type_name = match original_text_val {
                Some(v) => json_type_name(v),
                None => "NoneType",
            };
            return ToolResponse::error(
                "invalid_arguments",
                &format!("original_text must be a string, got {}", type_name),
                None,
                Some("patch_apply_check"),
            );
        }
    };
    let patch_text_val = args.get("patch_text");
    let patch_text = match patch_text_val.and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            let type_name = match patch_text_val {
                Some(v) => json_type_name(v),
                None => "NoneType",
            };
            return ToolResponse::error(
                "invalid_arguments",
                &format!("patch_text must be a string, got {}", type_name),
                None,
                Some("patch_apply_check"),
            );
        }
    };
    let strict = args.get("strict").and_then(|v| v.as_bool()).unwrap_or(true);
    let return_result_fingerprint = args
        .get("return_result_fingerprint")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let return_result_text = args
        .get("return_result_text")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    const MAX_ORIGINAL_LENGTH: usize = 200_000;
    const MAX_PATCH_LENGTH: usize = 100_000;

    if original_text.chars().count() > MAX_ORIGINAL_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!(
                "Original text length {} exceeds maximum of {}",
                original_text.chars().count(),
                MAX_ORIGINAL_LENGTH
            ),
            Some(vec![format!(
                "Maximum original text length is {}",
                MAX_ORIGINAL_LENGTH
            )]),
            Some("patch_apply_check"),
        );
    }
    if patch_text.chars().count() > MAX_PATCH_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!(
                "Patch text length {} exceeds maximum of {}",
                patch_text.chars().count(),
                MAX_PATCH_LENGTH
            ),
            Some(vec![format!(
                "Maximum patch text length is {}",
                MAX_PATCH_LENGTH
            )]),
            Some("patch_apply_check"),
        );
    }

    let result = crate::text::patch_apply_check(
        original_text,
        patch_text,
        strict,
        return_result_fingerprint,
        return_result_text,
    );

    ToolResponse::success(
        serde_json::json!({
            "patch_parse_ok": result.patch_parse_ok,
            "applies": result.applies,
            "hunks_total": result.hunks_total,
            "hunks_applied": result.hunks_applied,
            "hunks_failed": result.hunks_failed,
            "failed_hunks": result.failed_hunks,
            "affected_line_ranges": result.affected_line_ranges,
            "newline_style_before": result.newline_style_before,
            "newline_style_after": result.newline_style_after,
            "result_fingerprint": result.result_fingerprint,
            "result_text": result.result_text,
            "findings": result.findings,
        }),
        Some("patch_apply_check"),
    )
    .with_tool("patch_apply_check")
}

pub fn patch_summary(args: &Value) -> ToolResponse {
    let patch_text_val = args.get("patch_text");
    let patch_text = match patch_text_val.and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            let type_name = match patch_text_val {
                Some(v) => json_type_name(v),
                None => "NoneType",
            };
            return ToolResponse::error(
                "invalid_arguments",
                &format!("patch_text must be a string, got {}", type_name),
                None,
                Some("patch_summary"),
            );
        }
    };

    const MAX_PATCH_LENGTH: usize = 100_000;
    if patch_text.chars().count() > MAX_PATCH_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!(
                "Patch text length {} exceeds maximum of {}",
                patch_text.chars().count(),
                MAX_PATCH_LENGTH
            ),
            None,
            Some("patch_summary"),
        );
    }

    let result = crate::text::patch_summary(patch_text);

    ToolResponse::success(
        serde_json::json!({
            "files_changed": result.files_changed,
            "hunks_total": result.hunks_total,
            "additions": result.additions,
            "deletions": result.deletions,
            "renames_detected": result.renames_detected,
            "binary_patch_detected": result.binary_patch_detected,
            "line_ranges_by_file": result.line_ranges_by_file,
            "findings": result.findings,
        }),
        Some("patch_summary"),
    )
    .with_tool("patch_summary")
}

pub fn unicode_policy_check(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'text' parameter",
                None,
                Some("unicode_policy_check"),
            )
        }
    };
    let policy = match args.get("policy").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'policy' parameter",
                None,
                Some("unicode_policy_check"),
            )
        }
    };
    let normalization = args.get("normalization").and_then(|v| v.as_str());

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("unicode_policy_check"),
        );
    }

    let valid_policies = [
        "identifier_strict",
        "filename_safe",
        "source_code",
        "human_text",
        "json_key",
        "domain_like",
    ];
    if !valid_policies.contains(&policy) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported policy: {}", policy),
            Some(vec![format!("Use one of: {}", valid_policies.join(", "))]),
            Some("unicode_policy_check"),
        );
    }

    if let Some(ref n) = normalization {
        let valid_normalizations = ["raw", "NFC", "NFD", "NFKC", "NFKD"];
        if !valid_normalizations.contains(n) {
            return ToolResponse::error(
                "invalid_arguments",
                &format!("Unsupported normalization form: {}", n),
                Some(vec![format!(
                    "Use one of: {}",
                    valid_normalizations.join(", ")
                )]),
                Some("unicode_policy_check"),
            );
        }
    }

    let result = crate::text::unicode_policy_check(text, policy, normalization);

    ToolResponse::success(
        serde_json::json!({
            "pass_": result.pass,
            "policy": result.policy,
            "normalized_form": result.normalized_form,
            "findings": result.findings,
            "summary": result.summary,
        }),
        Some("unicode_policy_check"),
    )
    .with_tool("unicode_policy_check")
}

pub fn canonicalize_text(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'text' parameter",
                None,
                Some("canonicalize_text"),
            )
        }
    };
    let profile = match args.get("profile").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'profile' parameter",
                None,
                Some("canonicalize_text"),
            )
        }
    };
    let return_mapping = args
        .get("return_mapping")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("canonicalize_text"),
        );
    }

    let valid_profiles = [
        "source_file_identity",
        "identifier_compare",
        "human_label_compare",
        "json_key_compare",
        "path_segment_compare",
    ];
    if !valid_profiles.contains(&profile) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported profile: {}", profile),
            Some(vec![format!("Use one of: {}", valid_profiles.join(", "))]),
            Some("canonicalize_text"),
        );
    }

    let result = crate::text::canonicalize_text(text, profile, return_mapping);

    ToolResponse::success(
        serde_json::json!({
            "text": result.base.text,
            "changed": result.base.changed,
            "operations_applied": result.base.operations_applied,
            "fingerprint_before": result.base.fingerprint_before,
            "fingerprint_after": result.base.fingerprint_after,
            "findings": result.base.findings,
            "mapping": result.mapping,
        }),
        Some("canonicalize_text"),
    )
    .with_tool("canonicalize_text")
}

// Tier 3 Tools

pub fn identifier_table_inspect(args: &Value) -> ToolResponse {
    let identifiers = match args.get("identifiers").and_then(|v| v.as_array()) {
        Some(arr) => {
            // Validate each entry (matching Python's behavior)
            let mut bad_entries: Vec<String> = Vec::new();
            let mut valid_entries: Vec<crate::text::TableIdentifierEntry> = Vec::new();
            for (i, v) in arr.iter().enumerate() {
                match v.as_object() {
                    Some(obj) => {
                        match obj.get("name") {
                            Some(name_val) => {
                                match name_val.as_str() {
                                    Some(name_str) => {
                                        if name_str.chars().count() > MAX_TEXT_LENGTH {
                                            bad_entries.push(format!(
                                                "[{}] 'name' length {} exceeds MAX_TEXT_LENGTH {}",
                                                i,
                                                name_str.chars().count(),
                                                MAX_TEXT_LENGTH
                                            ));
                                        } else {
                                            // Validate optional fields
                                            if let Some(kind_val) = obj.get("kind") {
                                                if kind_val.as_str().is_none() {
                                                    bad_entries.push(format!(
                                                        "[{}] 'kind' must be a string, got {}",
                                                        i,
                                                        json_type_name(kind_val)
                                                    ));
                                                    continue;
                                                }
                                            }
                                            if let Some(file_val) = obj.get("file") {
                                                if file_val.as_str().is_none() {
                                                    bad_entries.push(format!(
                                                        "[{}] 'file' must be a string, got {}",
                                                        i,
                                                        json_type_name(file_val)
                                                    ));
                                                    continue;
                                                }
                                            }
                                            if let Some(line_val) = obj.get("line") {
                                                if line_val.as_i64().is_none()
                                                    || line_val.is_boolean()
                                                    || line_val.as_i64().unwrap_or(0) < 0
                                                {
                                                    bad_entries.push(format!("[{}] 'line' must be a non-negative integer, got {}", i, json_type_name(line_val)));
                                                    continue;
                                                }
                                            }
                                            if let Some(lang_val) = obj.get("language") {
                                                if lang_val.as_str().is_none() {
                                                    bad_entries.push(format!(
                                                        "[{}] 'language' must be a string, got {}",
                                                        i,
                                                        json_type_name(lang_val)
                                                    ));
                                                    continue;
                                                }
                                            }
                                            let kind = obj
                                                .get("kind")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("")
                                                .to_string();
                                            let file = obj
                                                .get("file")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("")
                                                .to_string();
                                            let line = obj
                                                .get("line")
                                                .and_then(|v| v.as_i64())
                                                .unwrap_or(0)
                                                .min(i32::MAX as i64)
                                                as i32;
                                            valid_entries.push(crate::text::TableIdentifierEntry {
                                                name: name_str.to_string(),
                                                kind,
                                                file,
                                                line,
                                            });
                                        }
                                    }
                                    None => {
                                        bad_entries.push(format!(
                                            "[{}] 'name' must be a string, got {}",
                                            i,
                                            json_type_name(name_val)
                                        ));
                                    }
                                }
                            }
                            None => {
                                bad_entries.push(format!("[{}] missing required 'name' field", i));
                            }
                        }
                    }
                    None => {
                        bad_entries.push(format!("[{}] is {}, not dict", i, json_type_name(v)));
                    }
                }
            }
            if !bad_entries.is_empty() {
                return ToolResponse::error(
                    "invalid_arguments",
                    "Malformed identifier entries",
                    Some(bad_entries.into_iter().take(10).collect()),
                    Some("identifier_table_inspect"),
                );
            }
            valid_entries
        }
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'identifiers' parameter",
                None,
                Some("identifier_table_inspect"),
            )
        }
    };
    let language = args
        .get("language")
        .and_then(|v| v.as_str())
        .unwrap_or("python");
    let valid_checks = [
        "casefold",
        "normalization",
        "confusable",
        "style",
        "reserved",
        "mixed_style",
    ];
    let checks = if let Some(arr) = args.get("checks").and_then(|v| v.as_array()) {
        let invalid: Vec<&str> = arr
            .iter()
            .filter_map(|v| v.as_str())
            .filter(|c| !valid_checks.contains(c))
            .collect();
        if !invalid.is_empty() {
            return ToolResponse::error(
                "invalid_arguments",
                &format!("Unknown check(s): {}", invalid.join(", ")),
                Some(vec![format!("Valid checks: {}", valid_checks.join(", "))]),
                Some("identifier_table_inspect"),
            );
        }
        Some(
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(String::from)
                .collect::<Vec<_>>(),
        )
    } else {
        None
    };

    if identifiers.len() > MAX_LIST_ITEMS {
        return ToolResponse::error(
            "input_too_large",
            &format!(
                "Number of identifiers {} exceeds MAX_LIST_ITEMS",
                identifiers.len()
            ),
            None,
            Some("identifier_table_inspect"),
        );
    }

    let valid_languages = [
        "generic",
        "python",
        "rust",
        "javascript",
        "typescript",
        "json_key",
    ];
    if !valid_languages.contains(&language) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported language: {}", language),
            Some(vec![format!("Use one of: {}", valid_languages.join(", "))]),
            Some("identifier_table_inspect"),
        );
    }

    let checks_ref = checks
        .as_ref()
        .map(|v| v.iter().map(|s| s.as_str()).collect::<Vec<_>>());
    let result = crate::text::identifier_table_inspect(&identifiers, language, checks_ref);

    let has_reserved = !result.reserved_keyword_hits.is_empty();
    let has_collisions = !result.collisions.is_empty();

    let mut envelope_findings: Vec<serde_json::Value> = Vec::new();
    for collision in &result.collisions {
        envelope_findings.push(serde_json::json!({
            "code": format!("COLLISION_{}", collision.kind.to_uppercase()),
            "severity": "warn",
            "message": collision.detail,
            "details": {"names": collision.names},
        }));
    }
    for hit in &result.reserved_keyword_hits {
        let mut details = serde_json::json!({});
        if !hit.file.is_empty() {
            details["file"] = serde_json::json!(hit.file);
        }
        if hit.line > 0 {
            details["line"] = serde_json::json!(hit.line);
        }
        envelope_findings.push(serde_json::json!({
            "code": "RESERVED_KEYWORD",
            "severity": "warn",
            "message": format!("'{}' is a reserved keyword in {}", hit.name, hit.language),
            "details": details,
        }));
    }
    for group in &result.mixed_style_groups {
        envelope_findings.push(serde_json::json!({
            "code": "MIXED_STYLE",
            "severity": "info",
            "message": format!("Mixed styles for '{}': {}", group.stripped, group.styles.join(", ")),
            "details": {"names": group.names},
        }));
    }

    let mut resp = ToolResponse::success(
        serde_json::json!({
            "count": result.count,
            "collisions": result.collisions,
            "reserved_keyword_hits": result.reserved_keyword_hits,
            "mixed_style_groups": result.mixed_style_groups,
            "findings": result.findings,
        }),
        Some("identifier_table_inspect"),
    )
    .with_tool("identifier_table_inspect");

    if !envelope_findings.is_empty() {
        resp = resp.with_findings(envelope_findings);
    }

    if has_reserved {
        resp = resp.with_machine_code("RESERVED_KEYWORDS");
    } else if has_collisions {
        resp = resp.with_machine_code("IDENT_COLLISIONS");
    }
    resp
}

pub fn version_constraint_check(args: &Value) -> ToolResponse {
    let version = match args.get("version").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'version' parameter",
                None,
                Some("version_constraint_check"),
            )
        }
    };
    let constraint = match args.get("constraint").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'constraint' parameter",
                None,
                Some("version_constraint_check"),
            )
        }
    };
    let scheme = args
        .get("scheme")
        .and_then(|v| v.as_str())
        .unwrap_or("semver");

    let valid_schemes = ["semver", "cargo"];
    if !valid_schemes.contains(&scheme) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported scheme: {}", scheme),
            Some(vec![format!("Use one of: {}", valid_schemes.join(", "))]),
            Some("version_constraint_check"),
        );
    }

    if version.trim().is_empty() {
        return ToolResponse::error(
            "invalid_arguments",
            "Version string is empty",
            Some(vec![
                "Provide a valid version string like '1.2.3'".to_string()
            ]),
            Some("version_constraint_check"),
        );
    }
    if constraint.trim().is_empty() {
        return ToolResponse::error(
            "invalid_arguments",
            "Constraint string is empty",
            Some(vec![
                "Provide a valid constraint like '>=1.0' or '^1.2.3'".to_string()
            ]),
            Some("version_constraint_check"),
        );
    }
    if version.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Input exceeds maximum length of {}", MAX_TEXT_LENGTH),
            None,
            Some("version_constraint_check"),
        );
    }
    if constraint.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Input exceeds maximum length of {}", MAX_TEXT_LENGTH),
            None,
            Some("version_constraint_check"),
        );
    }

    let result = crate::text::check_version_constraint(version, constraint, scheme);

    let satisfies = result.satisfies;
    let has_note = !result.findings.is_empty();

    // Create envelope findings from result findings
    let envelope_findings: Vec<serde_json::Value> = result
        .findings
        .iter()
        .map(|f| {
            serde_json::json!({
                "code": "CONSTRAINT_NOTE",
                "severity": "info",
                "message": f,
            })
        })
        .collect();

    let mut resp = ToolResponse::success(
        serde_json::json!({
            "satisfies": result.satisfies,
            "parsed_version": result.parsed_version,
            "parsed_constraint": result.parsed_constraint,
            "scheme": result.scheme,
            "explanation": result.explanation,
            "findings": result.findings,
        }),
        Some("version_constraint_check"),
    )
    .with_tool("version_constraint_check");

    if !envelope_findings.is_empty() {
        resp = resp.with_findings(envelope_findings);
    }
    if !satisfies {
        resp = resp.with_machine_code("CONSTRAINT_NOT_SATISFIED");
    } else if has_note {
        resp = resp.with_machine_code("CONSTRAINT_NOTE");
    }
    resp
}

pub fn cargo_toml_inspect(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'text' parameter",
                None,
                Some("cargo_toml_inspect"),
            )
        }
    };
    let check_workspace = args
        .get("check_workspace")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let check_dependencies = args
        .get("check_dependencies")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("cargo_toml_inspect"),
        );
    }

    let result = crate::text::cargo_toml_inspect(text, check_workspace, check_dependencies);

    let parse_ok = result.parse_ok;
    let has_findings = !result.findings.is_empty();

    // Create envelope findings with classified codes
    let envelope_findings: Vec<serde_json::Value> = result
        .findings
        .iter()
        .map(|msg| {
            let (severity, code) = {
                let lower = msg.to_lowercase();
                if lower.contains("parse error") || lower.contains("not a table") {
                    ("error", "CARGO_PARSE_ERROR")
                } else if lower.contains("missing") {
                    ("warn", "CARGO_MISSING_FIELD")
                } else if lower.contains("confusable") {
                    ("warn", "CARGO_CONFUSABLE_NAMES")
                } else if lower.contains("suspicious") {
                    ("warn", "CARGO_SUSPICIOUS_NAME")
                } else if lower.contains("unrecognized") {
                    ("warn", "CARGO_UNRECOGNIZED_VALUE")
                } else {
                    ("info", "CARGO_NOTE")
                }
            };
            serde_json::json!({
                "code": code,
                "severity": severity,
                "message": msg,
            })
        })
        .collect();

    let mut resp = ToolResponse::success(
    serde_json::json!({
        "parse_ok": result.parse_ok,
        "package": result.package,
        "workspace": result.workspace,
        "dependencies": result.dependencies,
        "path_dependencies": result.path_dependencies,
        "suspicious_dependency_names": result.suspicious_dependency_names,
        "duplicate_or_confusable_dependency_names": result.duplicate_or_confusable_dependency_names,
        "findings": result.findings,
    }),
    Some("cargo_toml_inspect")
    ).with_tool("cargo_toml_inspect");

    if !envelope_findings.is_empty() {
        resp = resp.with_findings(envelope_findings);
    }
    if !parse_ok {
        resp = resp.with_machine_code("CARGO_PARSE_FAILED");
    } else if has_findings {
        resp = resp.with_machine_code("CARGO_HAS_FINDINGS");
    }
    resp
}

pub fn text_security_inspect(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
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
        return ToolResponse::error(
            "input_too_large",
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("text_security_inspect"),
        );
    }

    let valid_policies = ["default", "source_code", "prompt", "markdown", "identifier"];
    if !valid_policies.contains(&policy) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("policy must be one of: {}", valid_policies.join(", ")),
            None,
            Some("text_security_inspect"),
        );
    }

    let valid_normalizations = ["none", "NFC", "NFD", "NFKC", "NFKD"];
    if !valid_normalizations.contains(&normalize) {
        return ToolResponse::error(
            "invalid_arguments",
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
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported detail level: {}", detail),
            Some(vec![format!("Use one of: {}", valid_details.join(", "))]),
            Some("text_security_inspect"),
        );
    }

    let mut subresults: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
    let mut all_findings: Vec<serde_json::Value> = Vec::new();
    let mut machine_codes: Vec<String> = Vec::new();

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
                    all_findings.push(serde_json::json!({
                        "code": "TEXT_INSPECT_WARNING",
                        "severity": "warn",
                        "message": w,
                    }));
                }
            }
        }
        // Check invisibles (Python extracts inv from result["invisibles"])
        if let Some(inv) = r.get("invisibles").and_then(|v| v.as_array()) {
            if !inv.is_empty() {
                if !machine_codes.contains(&"UNICODE_RISK".to_string()) {
                    machine_codes.push("UNICODE_RISK".to_string());
                }
                all_findings.push(serde_json::json!({
                    "code": "HIDDEN_CHARS",
                    "severity": "warn",
                    "message": format!("Found {} invisible character(s)", inv.len()),
                }));
            }
        }
        // Check confusables
        if let Some(conf) = r.get("confusables").and_then(|v| v.as_array()) {
            if !conf.is_empty() {
                if !machine_codes.contains(&"UNICODE_RISK".to_string()) {
                    machine_codes.push("UNICODE_RISK".to_string());
                }
                all_findings.push(serde_json::json!({
                    "code": "CONFUSABLES",
                    "severity": "warn",
                    "message": format!("Found {} confusable character(s)", conf.len()),
                }));
            }
        }
    }

    // 2. Map policy to unicode_policy_check policy (matching Python behavior)
    // Python: upolicy = "source_code" if policy == "source_code" else "human_text"
    let uc_policy = if policy == "source_code" {
        "source_code"
    } else {
        "human_text"
    };
    let uc_args = serde_json::json!({"text": text, "policy": uc_policy});
    let uc_result = unicode_policy_check(&uc_args);
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
                let sev = f.get("severity").and_then(|v| v.as_str()).unwrap_or("info");
                let code = f
                    .get("code")
                    .and_then(|v| v.as_str())
                    .unwrap_or("UNICODE_POLICY");
                let msg = f.get("message").and_then(|v| v.as_str()).unwrap_or("");
                all_findings.push(serde_json::json!({
                    "code": code,
                    "severity": sev,
                    "message": msg,
                }));
                if sev == "error" && !machine_codes.contains(&"UNICODE_RISK".to_string()) {
                    machine_codes.push("UNICODE_RISK".to_string());
                }
            }
        }
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
        if changed && !machine_codes.contains(&"NORMALIZATION_DIFF".to_string()) {
            machine_codes.push("NORMALIZATION_DIFF".to_string());
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
                    let sev = f.get("severity").and_then(|v| v.as_str()).unwrap_or("warn");
                    let msg = f.get("message").and_then(|v| v.as_str()).unwrap_or("");
                    all_findings.push(serde_json::json!({
                        "code": code,
                        "severity": sev,
                        "message": msg,
                    }));
                }
                if pi_findings.iter().any(|f| {
                    let sev = f.get("severity").and_then(|v| v.as_str()).unwrap_or("");
                    sev == "warn" || sev == "error"
                }) && !machine_codes.contains(&"PROMPT_INJECTION_RISK".to_string())
                {
                    machine_codes.push("PROMPT_INJECTION_RISK".to_string());
                }
            }
        }
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
            let id_result = identifier_inspect(&id_args);
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
                        let sev = f.get("severity").and_then(|v| v.as_str()).unwrap_or("warn");
                        let msg = f.get("message").and_then(|v| v.as_str()).unwrap_or("");
                        all_findings.push(serde_json::json!({
                            "code": code,
                            "severity": sev,
                            "message": msg,
                        }));
                    }
                    if !machine_codes.contains(&"IDENTIFIER_COLLISION_RISK".to_string()) {
                        machine_codes.push("IDENTIFIER_COLLISION_RISK".to_string());
                    }
                }
            }
        }
    }

    // 6. Determine verdict
    let has_error = all_findings
        .iter()
        .any(|f| f.get("severity").and_then(|v| v.as_str()) == Some("error"));
    let has_warn = all_findings
        .iter()
        .any(|f| f.get("severity").and_then(|v| v.as_str()) == Some("warn"));
    let verdict = if has_error {
        "block"
    } else if has_warn {
        "review"
    } else {
        "allow"
    };

    // Deduplicate machine codes
    let mut unique_machine_codes: Vec<String> = Vec::new();
    for mc in &machine_codes {
        if !unique_machine_codes.contains(mc) {
            unique_machine_codes.push(mc.clone());
        }
    }
    let primary_machine_code = if unique_machine_codes.is_empty() {
        "TEXT_SECURITY_OK".to_string()
    } else {
        unique_machine_codes[0].clone()
    };

    // Build summary
    let n_findings = all_findings.len();
    let summary = if verdict == "allow" {
        format!("No security issues found ({} findings).", n_findings)
    } else if verdict == "review" {
        format!(
            "Review recommended: {} finding(s) require attention.",
            n_findings
        )
    } else {
        format!("Block: {} finding(s) indicate security risk.", n_findings)
    };

    let recommended_action = if verdict == "allow" {
        "allow".to_string()
    } else if verdict == "review" {
        "review content for hidden instructions".to_string()
    } else {
        "do not trust this text without manual inspection".to_string()
    };

    let normalized_changed = subresults
        .get("canonicalize_text")
        .and_then(|v| v.get("changed"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let mut result = serde_json::json!({
        "verdict": verdict,
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
    resp = resp.with_machine_code(&primary_machine_code);
    if !all_findings.is_empty() {
        resp = resp.with_findings(all_findings);
    }
    resp
}

pub fn edit_preflight(args: &Value) -> ToolResponse {
    let original = match args.get("original").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'original' parameter",
                None,
                Some("edit_preflight"),
            )
        }
    };
    let replacement_mode = args
        .get("replacement_mode")
        .and_then(|v| v.as_str())
        .unwrap_or("literal");
    let strict = args.get("strict").and_then(|v| v.as_bool()).unwrap_or(true);
    let expected_fingerprint = args.get("expected_fingerprint").and_then(|v| v.as_str());

    if original.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Original text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("edit_preflight"),
        );
    }

    let valid_modes = ["literal", "patch", "line_range"];
    if !valid_modes.contains(&replacement_mode) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!(
                "replacement_mode must be one of: {}",
                valid_modes.join(", ")
            ),
            None,
            Some("edit_preflight"),
        );
    }

    let mut subresults = serde_json::Map::new();
    let mut findings: Vec<serde_json::Value> = Vec::new();

    match replacement_mode {
        "literal" => {
            let old = match args.get("old").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => {
                    return ToolResponse::error(
                        "invalid_arguments",
                        "literal mode requires both 'old' and 'new'",
                        None,
                        Some("edit_preflight"),
                    )
                }
            };
            let new = match args.get("new").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => {
                    return ToolResponse::error(
                        "invalid_arguments",
                        "literal mode requires both 'old' and 'new'",
                        None,
                        Some("edit_preflight"),
                    )
                }
            };
            let tr_args = serde_json::json!({
                "text": original,
                "old": old,
                "new": new,
                "mode": "exact",
            });
            let tr_result = text_replace_check_tool(&tr_args);
            if let Some(ref r) = tr_result.result {
                subresults.insert("text_replace_check".to_string(), r.clone());
                let match_count = r.get("match_count").and_then(|v| v.as_u64()).unwrap_or(0);
                if match_count == 0 {
                    findings.push(serde_json::json!({
                        "code": "NO_MATCH",
                        "severity": "error",
                        "message": "old text not found in original",
                    }));
                } else if match_count > 1 {
                    findings.push(serde_json::json!({
                        "code": "MULTIPLE_MATCHES",
                        "severity": "warn",
                        "message": format!("Found {} matches; use allow_multiple=true", match_count),
                    }));
                }
            }
        }
        "patch" => {
            let patch_text = match args.get("patch").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => {
                    return ToolResponse::error(
                        "invalid_arguments",
                        "patch mode requires 'patch'",
                        None,
                        Some("edit_preflight"),
                    )
                }
            };
            let pa_args = serde_json::json!({
                "original_text": original,
                "patch_text": patch_text,
                "strict": strict,
                "return_result_fingerprint": true,
                "return_result_text": false,
            });
            let pa_result = patch_apply_check(&pa_args);
            match pa_result {
                ToolResponse {
                    error: Some(ref e), ..
                } => {
                    findings.push(serde_json::json!({
                        "code": "PATCH_ERROR",
                        "severity": "error",
                        "message": e,
                    }));
                }
                ToolResponse {
                    result: Some(ref r),
                    ..
                } => {
                    subresults.insert("patch_apply_check".to_string(), r.clone());
                    if let Some(applies) = r.get("applies").and_then(|v| v.as_bool()) {
                        if !applies {
                            findings.push(serde_json::json!({
                                "code": "PATCH_FAILED",
                                "severity": "error",
                                "message": "Patch does not apply cleanly",
                            }));
                        }
                    }
                }
                _ => {}
            }
        }
        "line_range" => {
            let start_line = match args.get("start_line").and_then(|v| v.as_u64()) {
                Some(n) => n as usize,
                None => {
                    return ToolResponse::error(
                        "invalid_arguments",
                        "line_range mode requires 'start_line' and 'end_line'",
                        None,
                        Some("edit_preflight"),
                    )
                }
            };
            let end_line = match args.get("end_line").and_then(|v| v.as_u64()) {
                Some(n) => n as usize,
                None => {
                    return ToolResponse::error(
                        "invalid_arguments",
                        "line_range mode requires 'start_line' and 'end_line'",
                        None,
                        Some("edit_preflight"),
                    )
                }
            };
            let lr_args = serde_json::json!({
                "text": original,
                "start_line": start_line,
                "end_line": end_line,
            });
            let lr_result = line_range_extract_tool(&lr_args);
            if let Some(ref r) = lr_result.result {
                subresults.insert("line_range_extract".to_string(), r.clone());
                if let Some(valid_range) = r.get("valid_range").and_then(|v| v.as_bool()) {
                    if !valid_range {
                        findings.push(serde_json::json!({
                            "code": "INVALID_RANGE",
                            "severity": "error",
                            "message": "Invalid line range",
                        }));
                    }
                }
            }
        }
        _ => unreachable!(),
    }

    // Check expected_fingerprint if provided (matching Python per-mode behavior)
    // Python:
    //   literal mode: fingerprints original text
    //   patch mode:   fingerprints result_fingerprint from patch_apply_check
    //   line_range mode: fingerprints fingerprint from line_range_extract
    if let Some(fp) = expected_fingerprint {
        let (actual_fp, fp_source) = if replacement_mode == "patch" {
            // Use result_fingerprint from patch_apply_check subresult
            let fp_val = subresults
                .get("patch_apply_check")
                .and_then(|r| r.get("result_fingerprint"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            (fp_val.to_string(), "patch_apply_check")
        } else if replacement_mode == "line_range" {
            // Use fingerprint from line_range_extract subresult
            let fp_val = subresults
                .get("line_range_extract")
                .and_then(|r| r.get("fingerprint"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            (fp_val.to_string(), "line_range_extract")
        } else {
            // literal mode: fingerprint original text
            let fp_args = serde_json::json!({"text": original});
            let fp_result = text_fingerprint_tool(&fp_args);
            let fp_val = fp_result
                .result
                .as_ref()
                .and_then(|r| r.get("sha256"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            subresults.insert(
                "text_fingerprint".to_string(),
                fp_result.result.unwrap_or(serde_json::Value::Null),
            );
            (fp_val, "text_fingerprint")
        };
        if actual_fp != fp {
            findings.push(serde_json::json!({
                "code": "FINGERPRINT_MISMATCH",
                "severity": "warn",
                "message": format!("Expected {}, got {} (from {})", fp, actual_fp, fp_source),
            }));
        }
    }

    let has_error = findings
        .iter()
        .any(|f| f.get("severity").and_then(|v| v.as_str()) == Some("error"));
    let ok_to_apply = !has_error;

    // Determine machine_code and recommended_next_tool (matching Python's first-inserted-wins)
    let mut machine_codes: Vec<String> = Vec::new();
    let mut recommended_next: Option<String> = None;

    let has_no_match = findings
        .iter()
        .any(|f| f.get("code").and_then(|v| v.as_str()) == Some("NO_MATCH"));
    let has_multiple = findings
        .iter()
        .any(|f| f.get("code").and_then(|v| v.as_str()) == Some("MULTIPLE_MATCHES"));
    let has_patch_fail = findings
        .iter()
        .any(|f| f.get("code").and_then(|v| v.as_str()) == Some("PATCH_FAILED"));
    let has_patch_error = findings
        .iter()
        .any(|f| f.get("code").and_then(|v| v.as_str()) == Some("PATCH_ERROR"));
    let has_fingerprint = findings
        .iter()
        .any(|f| f.get("code").and_then(|v| v.as_str()) == Some("FINGERPRINT_MISMATCH"));
    let has_invalid_range = findings
        .iter()
        .any(|f| f.get("code").and_then(|v| v.as_str()) == Some("INVALID_RANGE"));

    if has_no_match || has_multiple {
        machine_codes.push("AMBIGUOUS_REPLACEMENT".to_string());
        recommended_next = Some("text_diff_explain".to_string());
    }
    if (has_patch_fail || has_patch_error) && !machine_codes.contains(&"PATCH_FAILED".to_string()) {
        machine_codes.push("PATCH_FAILED".to_string());
    }
    if has_invalid_range && !machine_codes.contains(&"LINE_RANGE_INVALID".to_string()) {
        machine_codes.push("LINE_RANGE_INVALID".to_string());
    }
    if has_fingerprint {
        if !machine_codes.contains(&"FINGERPRINT_MISMATCH".to_string()) {
            machine_codes.push("FINGERPRINT_MISMATCH".to_string());
        }
        if recommended_next.is_none() {
            recommended_next = Some("text_diff_explain".to_string());
        }
    }
    if has_error && machine_codes.is_empty() {
        machine_codes.push("EDIT_FAILED".to_string());
    }
    if machine_codes.is_empty() {
        machine_codes.push("EDIT_OK".to_string());
    }
    let machine_code_str = machine_codes[0].clone();

    // Build summary
    let summary = if ok_to_apply {
        format!("Edit OK ({} mode)", replacement_mode)
    } else {
        format!("Edit blocked ({} mode)", replacement_mode)
    };
    let summary = if findings.is_empty() {
        summary
    } else {
        format!("{}; {} finding(s)", summary, findings.len())
    };

    let mut result = serde_json::json!({
        "ok_to_apply": ok_to_apply,
        "mode": replacement_mode,
        "findings": findings,
        "machine_code": machine_code_str,
        "recommended_next_tool": recommended_next,
        "summary": summary,
    });
    if !subresults.is_empty() {
        result["subresults"] = serde_json::Value::Object(subresults);
    }

    let mut resp =
        ToolResponse::success(result, Some("edit_preflight")).with_tool("edit_preflight");
    resp = resp.with_machine_code(&machine_code_str);
    if !findings.is_empty() {
        resp = resp.with_findings(findings.clone());
    }
    if let Some(ref next) = recommended_next {
        let next_val: serde_json::Value = serde_json::Value::String(next.clone());
        resp = resp.with_recommended_next_tool(next_val);
    }
    resp
}

pub fn command_preflight(args: &Value) -> ToolResponse {
    let command = match args.get("command").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'command' parameter",
                None,
                Some("command_preflight"),
            )
        }
    };
    let platform = args
        .get("platform")
        .and_then(|v| v.as_str())
        .unwrap_or("posix");
    let policy = args
        .get("policy")
        .and_then(|v| v.as_str())
        .unwrap_or("default");
    let _working_directory = args.get("working_directory").and_then(|v| v.as_str());

    if command.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Command exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("command_preflight"),
        );
    }

    let valid_platforms = ["posix", "windows", "auto"];
    if !valid_platforms.contains(&platform) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported platform: {}", platform),
            Some(vec![format!("Use one of: {}", valid_platforms.join(", "))]),
            Some("command_preflight"),
        );
    }

    let valid_policies = ["default", "strict", "permissive"];
    if !valid_policies.contains(&policy) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported policy: {}", policy),
            Some(vec![format!("Use one of: {}", valid_policies.join(", "))]),
            Some("command_preflight"),
        );
    }

    let mut subresults = serde_json::Map::new();
    let mut findings: Vec<serde_json::Value> = Vec::new();
    let mut machine_codes: Vec<String> = Vec::new();

    // 1. Always call shell_split
    // Only "posix" shell splitting is currently supported; "windows" is not implemented.
    if platform == "windows" {
        return ToolResponse::error(
            "unsupported_platform",
            "Windows shell splitting is not supported; only 'posix' is available",
            Some(vec!["Use platform='posix' or platform='auto'".to_string()]),
            Some("command_preflight"),
        );
    }
    let shell = "posix";
    let ss_args = serde_json::json!({"command": command, "shell": shell});
    let ss_result = shell_split(&ss_args);
    if let Some(ref r) = ss_result.result {
        subresults.insert(
            "shell_split".to_string(),
            serde_json::json!({
                "argv": r.get("argv").cloned().unwrap_or(serde_json::json!([])),
                "features": r.get("features").cloned().unwrap_or(serde_json::json!({})),
            }),
        );
        // Check for risky features from the features map
        if let Some(features) = r.get("features") {
            if let Some(obj) = features.as_object() {
                let risky: Vec<&String> = obj
                    .iter()
                    .filter(|(_, v)| v.as_bool() == Some(true))
                    .map(|(k, _)| k)
                    .collect();
                for rf in &risky {
                    let sev = if policy == "strict" { "error" } else { "warn" };
                    findings.push(serde_json::json!({
                        "code": "RISKY_SHELL_FEATURE",
                        "severity": sev,
                        "message": rf,
                    }));
                }
                if !risky.is_empty() && !machine_codes.contains(&"SHELL_RISK".to_string()) {
                    machine_codes.push("SHELL_RISK".to_string());
                }
            }
        }
    } else if let Some(ref e) = ss_result.error {
        machine_codes.push("SHELL_PARSE_ERROR".to_string());
        findings.push(serde_json::json!({
            "code": "SHELL_PARSE_ERROR",
            "severity": "error",
            "message": e,
        }));
    }

    // 2. Check for regex-like args in the command (matching Python's case sensitivity)
    // Python: "grep" in command or "sed" in command or "awk" in command or "regex" in command.lower()
    let looks_like_regex = command.contains("grep")
        || command.contains("sed")
        || command.contains("awk")
        || command.to_lowercase().contains("regex");
    let argv: Vec<String> = subresults
        .get("shell_split")
        .and_then(|r| r.get("argv"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    if looks_like_regex && !argv.is_empty() {
        // Check arguments that look like patterns (non-empty, non-flag, contain regex metachars)
        // Note: includes argv[0] (command name), matching Python behavior
        let regex_metachars: std::collections::HashSet<char> = ".*+?[]|^$\\(){}".chars().collect();
        let regex_args: Vec<&String> = argv
            .iter()
            .filter(|arg| {
                !arg.starts_with('-')
                    && !arg.is_empty()
                    && arg.chars().any(|c| regex_metachars.contains(&c))
            })
            .collect();
        for pattern in &regex_args {
            let rs_args = serde_json::json!({"pattern": pattern.as_str()});
            let rs_result = regex_safety_check_tool(&rs_args);
            if let Some(ref r) = rs_result.result {
                let risk = r.get("risk").and_then(|v| v.as_str()).unwrap_or("none");
                let mut has_rs_findings = false;
                if let Some(findings_arr) = r.get("findings").and_then(|v| v.as_array()) {
                    has_rs_findings = !findings_arr.is_empty();
                    for f in findings_arr {
                        let sev = if risk != "none" { "warn" } else { "info" };
                        let kind = f
                            .get("kind")
                            .and_then(|v| v.as_str())
                            .unwrap_or("REGEX_RISK");
                        findings.push(serde_json::json!({
                            "code": kind.to_uppercase(),
                            "severity": sev,
                            "message": f.get("message").and_then(|v| v.as_str()).unwrap_or(""),
                        }));
                    }
                }
                if has_rs_findings
                    && risk != "none"
                    && !machine_codes.contains(&"REGEX_RISK".to_string())
                {
                    machine_codes.push("REGEX_RISK".to_string());
                }
                subresults.entry("regex_safety_check".to_string())
                    .or_insert_with(|| serde_json::json!([]))
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({
                        "pattern": pattern.as_str(),
                        "findings_count": r.get("findings").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0),
                        "risk": risk,
                    }));
            }
        }
    }

    // 3. Determine verdict
    let has_error = findings
        .iter()
        .any(|f| f.get("severity").and_then(|v| v.as_str()) == Some("error"));
    let has_warn = findings
        .iter()
        .any(|f| f.get("severity").and_then(|v| v.as_str()) == Some("warn"));
    let verdict = if has_error {
        "block"
    } else if has_warn {
        "review"
    } else {
        "allow"
    };

    // Build primary machine_code (matching Python: first code from ordered unique list)
    let unique_codes: Vec<String> = machine_codes.into_iter().fold(Vec::new(), |mut acc, c| {
        if !acc.contains(&c) {
            acc.push(c);
        }
        acc
    });
    let primary_code = unique_codes
        .first()
        .cloned()
        .unwrap_or_else(|| "COMMAND_OK".to_string());

    let summary = format!("Command {} ({} finding(s))", verdict, findings.len());

    let mut result = serde_json::json!({
        "verdict": verdict,
        "command": command,
        "platform": platform,
        "policy": policy,
        "findings": findings,
        "machine_code": primary_code,
        "summary": summary,
    });
    if let Some(wd) = _working_directory {
        result["working_directory"] = serde_json::json!(wd);
    }
    if !subresults.is_empty() {
        result["subresults"] = serde_json::Value::Object(subresults);
    }

    let mut resp =
        ToolResponse::success(result, Some("command_preflight")).with_tool("command_preflight");
    resp = resp.with_machine_code(&primary_code);
    if !findings.is_empty() {
        resp = resp.with_findings(findings);
    }
    resp
}

pub fn config_preflight(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'text' parameter",
                None,
                Some("config_preflight"),
            )
        }
    };
    let format = args
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("auto");
    let schema = args.get("schema");
    let strict = args
        .get("strict")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("config_preflight"),
        );
    }

    let valid_formats = ["auto", "json", "toml", "dotenv", "ini", "cargo_toml"];
    if !valid_formats.contains(&format) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported format: {}", format),
            Some(vec![format!("Use one of: {}", valid_formats.join(", "))]),
            Some("config_preflight"),
        );
    }

    // Auto-detect format
    let detected_format = if format == "auto" {
        let stripped = text.trim();
        if stripped.starts_with('{') || stripped.starts_with('[') {
            // Could be JSON or TOML; try JSON first
            let vj_result = validate_json(&serde_json::json!({"text": text}));
            let is_json = vj_result
                .result
                .as_ref()
                .and_then(|r| r.get("valid"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if is_json {
                "json"
            } else {
                "toml"
            }
        } else if stripped.contains('=') && !stripped.starts_with('{') {
            // Heuristic: if contains = and doesn't look like JSON object
            "dotenv"
        } else {
            "json"
        }
    } else {
        format
    };

    let mut subresults = serde_json::Map::new();
    let mut findings: Vec<serde_json::Value> = Vec::new();
    let mut machine_codes: Vec<String> = Vec::new();
    let mut verdict = "valid";

    match detected_format {
        "json" => {
            let vj_result = validate_json(&serde_json::json!({"text": text}));
            if let Some(ref r) = vj_result.result {
                subresults.insert("validate_json".to_string(), r.clone());
                let valid = r.get("valid").and_then(|v| v.as_bool()).unwrap_or(false);
                if !valid {
                    verdict = "invalid";
                    machine_codes.push("CONFIG_PARSE_FAILED".to_string());
                    findings.push(serde_json::json!({
                        "code": "JSON_PARSE_ERROR",
                        "severity": "error",
                        "message": r.get("error").and_then(|v| v.as_str()).unwrap_or("Invalid JSON"),
                    }));
                } else if let Some(sch) = schema {
                    let vs_args = serde_json::json!({"text": text, "schema": sch});
                    let vs_result = validate_schema_light_tool(&vs_args);
                    if let Some(ref vr) = vs_result.result {
                        subresults.insert("validate_schema_light".to_string(), vr.clone());
                        let vs_valid = vr.get("valid").and_then(|v| v.as_bool()).unwrap_or(true);
                        if !vs_valid {
                            machine_codes.push("CONFIG_SCHEMA_MISMATCH".to_string());
                            verdict = "valid_with_warnings";
                            if let Some(violations) =
                                vr.get("violations").and_then(|v| v.as_array())
                            {
                                for violation in violations {
                                    let msg = violation
                                        .get("message")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("Schema violation");
                                    findings.push(serde_json::json!({
                                        "code": "SCHEMA_ERROR",
                                        "severity": if strict { "error" } else { "warn" },
                                        "message": msg,
                                    }));
                                }
                            } else {
                                findings.push(serde_json::json!({
                                    "code": "SCHEMA_ERROR",
                                    "severity": if strict { "error" } else { "warn" },
                                    "message": "Schema validation failed",
                                }));
                            }
                        }
                    }
                }
                // Optionally canonicalize
                if verdict != "invalid" {
                    let jc_result = json_canonicalize(&serde_json::json!({"text": text}));
                    if let Some(ref r) = jc_result.result {
                        let canonical = r.get("canonical").and_then(|v| v.as_str());
                        let changed = canonical.is_some_and(|c| c != text);
                        subresults.insert(
                            "json_canonicalize".to_string(),
                            serde_json::json!({
                                "changed": changed,
                            }),
                        );
                    }
                }
            } else if let Some(ref e) = vj_result.error {
                machine_codes.push("CONFIG_PARSE_FAILED".to_string());
                findings.push(serde_json::json!({
                    "code": "CONFIG_ERROR",
                    "severity": "error",
                    "message": e,
                }));
            }
        }
        "toml" => {
            let vt_result = validate_toml_tool(&serde_json::json!({"text": text}));
            if let Some(ref r) = vt_result.result {
                subresults.insert("validate_toml".to_string(), r.clone());
                let valid = r
                    .get("valid")
                    .or_else(|| r.get("parse_ok"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if !valid {
                    verdict = "invalid";
                    machine_codes.push("CONFIG_PARSE_FAILED".to_string());
                    findings.push(serde_json::json!({
                        "code": "TOML_PARSE_ERROR",
                        "severity": "error",
                        "message": r.get("error").and_then(|v| v.as_str()).unwrap_or("Invalid TOML"),
                    }));
                } else {
                    let ts_result = toml_shape_tool(&serde_json::json!({"text": text}));
                    if let Some(ref r) = ts_result.result {
                        subresults.insert("toml_shape".to_string(), r.clone());
                    }
                }
            } else if let Some(ref e) = vt_result.error {
                machine_codes.push("CONFIG_PARSE_FAILED".to_string());
                findings.push(serde_json::json!({
                    "code": "CONFIG_ERROR",
                    "severity": "error",
                    "message": e,
                }));
            }
        }
        "dotenv" => {
            let dv_result = dotenv_validate(&serde_json::json!({"text": text}));
            if let Some(ref r) = dv_result.result {
                subresults.insert("dotenv_validate".to_string(), r.clone());
                let parse_ok = r.get("parse_ok").and_then(|v| v.as_bool()).unwrap_or(false);
                if !parse_ok {
                    verdict = "invalid";
                    machine_codes.push("CONFIG_PARSE_FAILED".to_string());
                    if let Some(dv_findings) = r.get("findings").and_then(|v| v.as_array()) {
                        for err in dv_findings {
                            findings.push(serde_json::json!({
                                "code": "DOTENV_ERROR",
                                "severity": "error",
                                "message": err.as_str().unwrap_or("Invalid dotenv format"),
                            }));
                        }
                    } else {
                        findings.push(serde_json::json!({
                            "code": "DOTENV_ERROR",
                            "severity": "error",
                            "message": "Invalid dotenv format",
                        }));
                    }
                }
            } else if let Some(ref e) = dv_result.error {
                machine_codes.push("CONFIG_PARSE_FAILED".to_string());
                findings.push(serde_json::json!({
                    "code": "CONFIG_ERROR",
                    "severity": "error",
                    "message": e,
                }));
            }
        }
        "ini" => {
            let iv_result = ini_validate(&serde_json::json!({"text": text}));
            if let Some(ref r) = iv_result.result {
                subresults.insert("ini_validate".to_string(), r.clone());
                let parse_ok = r.get("parse_ok").and_then(|v| v.as_bool()).unwrap_or(false);
                if !parse_ok {
                    verdict = "invalid";
                    machine_codes.push("CONFIG_PARSE_FAILED".to_string());
                    if let Some(iv_findings) = r.get("findings").and_then(|v| v.as_array()) {
                        for err in iv_findings {
                            findings.push(serde_json::json!({
                                "code": "INI_ERROR",
                                "severity": "error",
                                "message": err.as_str().unwrap_or("Invalid INI format"),
                            }));
                        }
                    } else {
                        findings.push(serde_json::json!({
                            "code": "INI_ERROR",
                            "severity": "error",
                            "message": "Invalid INI format",
                        }));
                    }
                }
            } else if let Some(ref e) = iv_result.error {
                machine_codes.push("CONFIG_PARSE_FAILED".to_string());
                findings.push(serde_json::json!({
                    "code": "CONFIG_ERROR",
                    "severity": "error",
                    "message": e,
                }));
            }
        }
        "cargo_toml" => {
            let ct_result = cargo_toml_inspect(&serde_json::json!({"text": text}));
            if let Some(ref r) = ct_result.result {
                subresults.insert("cargo_toml_inspect".to_string(), r.clone());
                let parse_ok = r.get("parse_ok").and_then(|v| v.as_bool()).unwrap_or(false);
                if !parse_ok {
                    verdict = "invalid";
                    machine_codes.push("CONFIG_PARSE_FAILED".to_string());
                    findings.push(serde_json::json!({
                        "code": "CARGO_PARSE_ERROR",
                        "severity": "error",
                        "message": "Cargo.toml parse failed",
                    }));
                } else {
                    if let Some(ct_findings) = r.get("findings").and_then(|v| v.as_array()) {
                        for f in ct_findings {
                            findings.push(serde_json::json!({
                                "code": f.get("code").and_then(|v| v.as_str()).unwrap_or("CARGO_NOTE"),
                                "severity": f.get("severity").and_then(|v| v.as_str()).unwrap_or("info"),
                                "message": f.get("message").and_then(|v| v.as_str()).unwrap_or(""),
                            }));
                        }
                    }
                }
            } else if let Some(ref e) = ct_result.error {
                machine_codes.push("CONFIG_PARSE_FAILED".to_string());
                findings.push(serde_json::json!({
                    "code": "CONFIG_ERROR",
                    "severity": "error",
                    "message": e,
                }));
            }
        }
        _ => unreachable!(),
    }

    let parse_ok = verdict != "invalid";

    let machine_code = if !parse_ok {
        machine_codes
            .first()
            .cloned()
            .unwrap_or_else(|| "CONFIG_PARSE_FAILED".to_string())
    } else if !findings.is_empty() {
        machine_codes
            .first()
            .cloned()
            .unwrap_or_else(|| "CONFIG_HAS_WARNINGS".to_string())
    } else {
        "CONFIG_OK".to_string()
    };

    let summary = format!(
        "{} config: {} ({} finding(s))",
        detected_format,
        verdict,
        findings.len()
    );

    let mut result = serde_json::json!({
        "valid": parse_ok,
        "verdict": verdict,
        "format": detected_format,
        "findings": findings,
        "machine_code": machine_code,
        "summary": summary,
    });
    if !subresults.is_empty() {
        result["subresults"] = serde_json::Value::Object(subresults);
    }

    let mut resp =
        ToolResponse::success(result, Some("config_preflight")).with_tool("config_preflight");
    resp = resp.with_machine_code(&machine_code);
    if !findings.is_empty() {
        resp = resp.with_findings(findings);
    }
    resp
}

pub fn structured_data_compare(args: &Value) -> ToolResponse {
    let a = match args.get("a").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'a' parameter",
                None,
                Some("structured_data_compare"),
            )
        }
    };
    let b = match args.get("b").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'b' parameter",
                None,
                Some("structured_data_compare"),
            )
        }
    };
    let format = args
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("json");
    let ignore_object_order = args
        .get("ignore_object_order")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let ignore_array_order = args
        .get("ignore_array_order")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let max_diffs = args.get("max_diffs").and_then(|v| v.as_u64()).unwrap_or(50) as usize;

    if a.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Input 'a' exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("structured_data_compare"),
        );
    }
    if b.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Input 'b' exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("structured_data_compare"),
        );
    }

    let valid_formats = ["json"];
    if !valid_formats.contains(&format) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("format must be 'json' (got '{}')", format),
            None,
            Some("structured_data_compare"),
        );
    }

    let mut subresults = serde_json::Map::new();
    let mut findings: Vec<serde_json::Value> = Vec::new();

    // Validate both inputs
    let vj_a = validate_json(&serde_json::json!({"text": a}));
    let vj_b = validate_json(&serde_json::json!({"text": b}));

    let valid_a = vj_a
        .result
        .as_ref()
        .and_then(|r| r.get("valid"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let valid_b = vj_b
        .result
        .as_ref()
        .and_then(|r| r.get("valid"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Extract error messages before consuming results
    let error_a = vj_a
        .result
        .as_ref()
        .and_then(|r| r.get("error"))
        .and_then(|v| v.as_str())
        .unwrap_or("Invalid JSON in a")
        .to_string();
    let error_b = vj_b
        .result
        .as_ref()
        .and_then(|r| r.get("error"))
        .and_then(|v| v.as_str())
        .unwrap_or("Invalid JSON in b")
        .to_string();

    subresults.insert(
        "validate_a".to_string(),
        serde_json::json!({"valid": valid_a}),
    );
    subresults.insert(
        "validate_b".to_string(),
        serde_json::json!({"valid": valid_b}),
    );

    if !valid_a {
        findings.push(serde_json::json!({
            "code": "INVALID_JSON_A",
            "severity": "error",
            "message": error_a,
        }));
    }
    if !valid_b {
        findings.push(serde_json::json!({
            "code": "INVALID_JSON_B",
            "severity": "error",
            "message": error_b,
        }));
    }

    if !valid_a || !valid_b {
        let result = serde_json::json!({
            "equal": false,
            "valid_a": valid_a,
            "valid_b": valid_b,
            "findings": findings,
            "machine_code": "INVALID_INPUT",
            "summary": "One or both inputs are not valid JSON",
        });
        return ToolResponse::success(result, Some("structured_data_compare"))
            .with_tool("structured_data_compare")
            .with_findings(findings)
            .with_machine_code("INVALID_INPUT");
    }

    let equal = if valid_a && valid_b {
        let jc_result = json_compare(&serde_json::json!({
            "a": a,
            "b": b,
            "ignore_object_order": ignore_object_order,
            "ignore_array_order": ignore_array_order,
            "max_diffs": max_diffs,
        }));
        // Check if json_compare itself failed
        if jc_result.error.is_some() {
            findings.push(serde_json::json!({
                "code": "COMPARE_ERROR",
                "severity": "error",
                "message": jc_result.error.as_deref().unwrap_or("json_compare failed"),
            }));
            false
        } else {
            let jc_equal = jc_result
                .result
                .as_ref()
                .and_then(|r| r.get("equal"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            // Extract VALUE_DIFF findings from json_compare diffs
            if let Some(jc_res) = &jc_result.result {
                if let Some(diffs) = jc_res.get("diffs").and_then(|d| d.as_array()) {
                    for d in diffs.iter().take(max_diffs) {
                        let path = d.get("path").and_then(|v| v.as_str()).unwrap_or("/");
                        let kind = d.get("kind").and_then(|v| v.as_str()).unwrap_or("unknown");
                        findings.push(serde_json::json!({
                            "code": "VALUE_DIFF",
                            "severity": "info",
                            "message": format!("{}: {}", path, kind),
                        }));
                    }
                }
            }
            // jc_equal already reflects json_compare's equality verdict; no need for
            // a separate has_value_diff check since jc_equal is false iff diffs exist.
            let eq = jc_equal;
            subresults.insert(
                "json_compare".to_string(),
                serde_json::json!({
                    "equal": eq,
                    "diff_count": jc_result.result.as_ref()
                        .and_then(|r| r.get("diff_count"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0),
                }),
            );
            eq
        }
    } else {
        false
    };

    // Get shapes for both
    let shape_a = json_shape_tool(&serde_json::json!({"text": a}));
    let shape_b = json_shape_tool(&serde_json::json!({"text": b}));
    // Check shape type mismatch
    if let (Some(sa), Some(sb)) = (&shape_a.result, &shape_b.result) {
        let type_a = sa.get("type").and_then(|v| v.as_str());
        let type_b = sb.get("type").and_then(|v| v.as_str());
        if type_a.is_some() && type_b.is_some() && type_a != type_b {
            findings.push(serde_json::json!({
                "code": "TYPE_MISMATCH",
                "severity": "warn",
                "message": format!("Type mismatch: a={}, b={}", type_a.unwrap_or("?"), type_b.unwrap_or("?")),
            }));
        }
        subresults.insert(
            "shape_a".to_string(),
            shape_a.result.unwrap_or(serde_json::Value::Null),
        );
        subresults.insert(
            "shape_b".to_string(),
            shape_b.result.unwrap_or(serde_json::Value::Null),
        );
    }

    // Determine machine_code (matching Python behavior)
    let machine_code = if !valid_a || !valid_b {
        "INVALID_INPUT".to_string()
    } else if equal {
        "DATA_EQUAL".to_string()
    } else {
        "DATA_DIFF".to_string()
    };

    // Build summary
    let diff_count = findings
        .iter()
        .filter(|f| f.get("code").and_then(|v| v.as_str()).unwrap_or("") == "VALUE_DIFF")
        .count();
    let summary = if equal {
        "Equal".to_string()
    } else {
        format!(
            "Different ({} diff(s), {} finding(s))",
            diff_count,
            findings.len()
        )
    };

    let mut result = serde_json::json!({
        "equal": equal,
        "valid_a": valid_a,
        "valid_b": valid_b,
        "findings": findings,
        "machine_code": machine_code,
        "summary": summary,
    });
    if !subresults.is_empty() {
        result["subresults"] = serde_json::Value::Object(subresults);
    }

    let mut resp = ToolResponse::success(result, Some("structured_data_compare"))
        .with_tool("structured_data_compare");
    resp = resp.with_machine_code(&machine_code);
    if !findings.is_empty() {
        resp = resp.with_findings(findings);
    }
    resp
}

#[cfg(test)]
mod tests {
    use super::split_lines;

    #[test]
    fn split_lines_matches_python_for_empty_text() {
        assert!(split_lines("").is_empty());
    }

    #[test]
    fn split_lines_preserves_trailing_empty_line() {
        assert_eq!(split_lines("a\n"), vec!["a".to_string(), String::new()]);
    }

    #[test]
    fn split_lines_treats_crlf_as_one_separator() {
        assert_eq!(
            split_lines("a\r\nb\rc"),
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }
}
