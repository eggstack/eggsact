use crate::mcp::schemas::ToolResponse;
use crate::text::count_graphemes;

use serde_json::Value;
use std::sync::{mpsc, Condvar, LazyLock};
use std::time::Duration;
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub(crate) const MAX_TEXT_LENGTH: usize = 100_000;
pub(crate) const MAX_INSPECT_ITEMS: usize = 100;
pub(crate) const MAX_LIST_ITEMS: usize = 10_000;
pub(crate) const MAX_REGEX_SAMPLES: usize = 100;
pub(crate) const MAX_REGEX_SAMPLE_LENGTH: usize = 10_000;
pub(crate) const MAX_MATCHES_REGEX: usize = 100;
pub(crate) const MAX_MATCHES_HARD_CAP: usize = 1000;
pub(crate) const MAX_PATTERN_LENGTH: usize = 1000;
pub(crate) const MAX_SCHEMA_DEPTH: usize = 32;
pub(crate) const MAX_SCHEMA_ELEMENTS: usize = 10_000;
pub(crate) const REGEX_TIMEOUT_SECONDS: u64 = 5;
pub(crate) const MAX_CONCURRENT_SPAWNED: usize = 16;
pub(crate) const SPAWN_ACQUIRE_TIMEOUT: u64 = 10;
pub(crate) const MAX_EXPRESSION_LENGTH: usize = 10_000;

// ---------------------------------------------------------------------------
// run_with_timeout
// ---------------------------------------------------------------------------

pub(crate) fn run_with_timeout<T: Send + 'static>(
    timeout: Duration,
    f: impl FnOnce() -> T + Send + 'static,
) -> Result<T, ()> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(timeout).map_err(|_| ())
}

// ---------------------------------------------------------------------------
// split_lines
// ---------------------------------------------------------------------------

pub(crate) fn split_lines(text: &str) -> Vec<String> {
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

// ---------------------------------------------------------------------------
// SpawnState / SpawnSemaphore / SpawnPermit / try_acquire_spawn_permit
// ---------------------------------------------------------------------------

pub(crate) struct SpawnState {
    count: usize,
}

pub(crate) struct SpawnSemaphore {
    pub(crate) state: std::sync::Mutex<SpawnState>,
    pub(crate) cvar: Condvar,
}

static SPAWN_SEMAPHORE: LazyLock<SpawnSemaphore> = LazyLock::new(|| SpawnSemaphore {
    state: std::sync::Mutex::new(SpawnState { count: 0 }),
    cvar: Condvar::new(),
});

pub(crate) struct SpawnPermit {
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

pub(crate) fn try_acquire_spawn_permit() -> Option<SpawnPermit> {
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

// ---------------------------------------------------------------------------
// _require_str
// ---------------------------------------------------------------------------

pub(crate) fn _require_str<'a>(
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

// ---------------------------------------------------------------------------
// json_type_name
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// require_non_negative_int_arg
// ---------------------------------------------------------------------------

pub(crate) fn require_non_negative_int_arg(
    args: &Value,
    field: &str,
    tool: &'static str,
) -> Result<usize, Box<ToolResponse>> {
    let Some(value) = args.get(field) else {
        return Err(Box::new(ToolResponse::error(
            "invalid_arguments",
            &format!("Missing '{}' parameter", field),
            None,
            Some(tool),
        )));
    };

    let value_i64 = match value {
        Value::Number(n) if n.is_i64() => match n.as_i64() {
            Some(value) => value,
            None => {
                return Err(Box::new(ToolResponse::error(
                    "invalid_arguments",
                    &format!("{} must be an int, got {}", field, json_type_name(value)),
                    None,
                    Some(tool),
                )));
            }
        },
        Value::Bool(_) => {
            return Err(Box::new(ToolResponse::error(
                "invalid_arguments",
                &format!("{} must be an int, got bool", field),
                None,
                Some(tool),
            )));
        }
        value => {
            return Err(Box::new(ToolResponse::error(
                "invalid_arguments",
                &format!("{} must be an int, got {}", field, json_type_name(value)),
                None,
                Some(tool),
            )));
        }
    };

    if value_i64 < 0 {
        return Err(Box::new(ToolResponse::error(
            "invalid_arguments",
            &format!("{} must be non-negative, got {}", field, value_i64),
            None,
            Some(tool),
        )));
    }

    Ok(value_i64 as usize)
}

// ---------------------------------------------------------------------------
// validate_line_range_order
// ---------------------------------------------------------------------------

pub(crate) fn validate_line_range_order(
    start_line: usize,
    end_line: usize,
    tool: &'static str,
) -> Result<(), Box<ToolResponse>> {
    if start_line > end_line {
        return Err(Box::new(ToolResponse::error(
            "invalid_arguments",
            &format!(
                "start_line ({}) must be <= end_line ({})",
                start_line, end_line
            ),
            None,
            Some(tool),
        )));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// require_array_arg
// ---------------------------------------------------------------------------

pub(crate) fn require_array_arg<'a>(
    args: &'a Value,
    field: &str,
    tool: &'static str,
) -> Result<&'a Vec<Value>, Box<ToolResponse>> {
    match args.get(field).and_then(|v| v.as_array()) {
        Some(arr) => Ok(arr),
        None => Err(Box::new(ToolResponse::error(
            "invalid_arguments",
            &format!(
                "{} must be a list, got {}",
                field,
                json_type_name(args.get(field).unwrap_or(&Value::Null))
            ),
            None,
            Some(tool),
        ))),
    }
}

// ---------------------------------------------------------------------------
// require_list_compare_args
// ---------------------------------------------------------------------------

pub(crate) fn require_list_compare_args(
    args: &Value,
) -> Result<(&Vec<Value>, &Vec<Value>), Box<ToolResponse>> {
    let type_name = |field: &str| args.get(field).map(json_type_name).unwrap_or("NoneType");

    let a = match args.get("a").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => {
            return Err(Box::new(ToolResponse::error(
                "invalid_arguments",
                &format!(
                    "a and b must be lists, got {} and {}",
                    type_name("a"),
                    type_name("b")
                ),
                None,
                Some("list_compare"),
            )));
        }
    };

    let b = match args.get("b").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => {
            return Err(Box::new(ToolResponse::error(
                "invalid_arguments",
                &format!(
                    "a and b must be lists, got {} and {}",
                    type_name("a"),
                    type_name("b")
                ),
                None,
                Some("list_compare"),
            )));
        }
    };

    Ok((a, b))
}

// ---------------------------------------------------------------------------
// unicode_casefold
// ---------------------------------------------------------------------------

pub(crate) fn unicode_casefold(s: &str) -> String {
    caseless::default_case_fold_str(s)
}

// ---------------------------------------------------------------------------
// normalize_text_count_input
// ---------------------------------------------------------------------------

pub(crate) fn normalize_text_count_input(text: &str, normalization: &str) -> String {
    match normalization {
        "NFKC" => text.nfkc().collect(),
        "NFC" => text.nfc().collect(),
        _ => text.to_string(),
    }
}

// ---------------------------------------------------------------------------
// validate_text_count_target
// ---------------------------------------------------------------------------

pub(crate) fn validate_text_count_target(
    target: &str,
    normalized_target: &str,
    count_mode: &str,
) -> Option<ToolResponse> {
    if target.is_empty() {
        return Some(ToolResponse::error(
            "invalid_arguments",
            "target must not be empty",
            Some(vec!["Provide a non-empty target".to_string()]),
            Some("text_count"),
        ));
    }

    match count_mode {
        "codepoint" => {
            if normalized_target.chars().count() != 1 {
                return Some(ToolResponse::error(
                    "invalid_arguments",
                    "target must be a single codepoint for count_mode='codepoint' after normalization",
                    Some(vec!["Provide a target that normalizes to one codepoint".to_string()]),
                    Some("text_count"),
                ));
            }
        }
        "grapheme" => {
            if count_graphemes(normalized_target) != 1 {
                return Some(ToolResponse::error(
                    "invalid_arguments",
                    "target must be a single grapheme for count_mode='grapheme' after normalization",
                    Some(vec!["Provide a target that normalizes to one grapheme".to_string()]),
                    Some("text_count"),
                ));
            }
        }
        "byte" if normalized_target.len() != 1 || !normalized_target.is_ascii() => {
            return Some(ToolResponse::error(
                "invalid_arguments",
                "target must be a single byte for count_mode='byte' after normalization",
                Some(vec![
                    "Provide a target that normalizes to one ASCII byte".to_string()
                ]),
                Some("text_count"),
            ));
        }
        _ => {}
    }

    None
}

// ---------------------------------------------------------------------------
// text_count_matches
// ---------------------------------------------------------------------------

pub(crate) fn text_count_matches(
    work_text: &str,
    work_target: &str,
    count_mode: &str,
) -> (usize, Vec<usize>) {
    match count_mode {
        "byte" => {
            let target_bytes = work_target.as_bytes();
            let width = target_bytes.len();
            let positions: Vec<usize> = work_text
                .as_bytes()
                .windows(width)
                .enumerate()
                .filter_map(|(index, window)| (window == target_bytes).then_some(index))
                .collect();
            (positions.len(), positions)
        }
        "codepoint" => {
            let Some(target_char) = work_target.chars().next() else {
                return (0, Vec::new());
            };
            let positions: Vec<usize> = work_text
                .chars()
                .enumerate()
                .filter_map(|(index, c)| (c == target_char).then_some(index))
                .collect();
            (positions.len(), positions)
        }
        "grapheme" => {
            let positions: Vec<usize> = work_text
                .grapheme_indices(true)
                .filter_map(|(index, grapheme)| (grapheme == work_target).then_some(index))
                .collect();
            (positions.len(), positions)
        }
        "substring" => {
            let positions: Vec<usize> = work_text
                .match_indices(work_target)
                .map(|(index, _)| index)
                .collect();
            (positions.len(), positions)
        }
        _ => (0, Vec::new()),
    }
}

// ---------------------------------------------------------------------------
// contains_true_division
// ---------------------------------------------------------------------------

pub(crate) fn contains_true_division(expr: &str) -> bool {
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

// ---------------------------------------------------------------------------
// common_prefix_len / common_suffix_len
// ---------------------------------------------------------------------------

pub(crate) fn common_prefix_len(a: &str, b: &str) -> usize {
    let mut count = 0;
    for (ca, cb) in a.chars().zip(b.chars()) {
        if ca != cb {
            break;
        }
        count += 1;
    }
    count
}

pub(crate) fn common_suffix_len(a: &str, b: &str) -> usize {
    let mut count = 0;
    for (ca, cb) in a.chars().rev().zip(b.chars().rev()) {
        if ca != cb {
            break;
        }
        count += 1;
    }
    count
}

// ---------------------------------------------------------------------------
// classify_difference
// ---------------------------------------------------------------------------

pub(crate) fn classify_difference(
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

// ---------------------------------------------------------------------------
// generate_agent_instruction
// ---------------------------------------------------------------------------

pub(crate) fn generate_agent_instruction(
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

// ---------------------------------------------------------------------------
// is_invisible_char
// ---------------------------------------------------------------------------

pub(crate) fn is_invisible_char(c: char) -> bool {
    let cp = c as u32;
    matches!(cp,
        0x200b | 0x200c | 0x200d | 0x200e | 0x200f |
        0xfeff | 0x00a0 | 0x2028 | 0x2029 |
        0x2060 | 0x00ad | 0x180e | 0x034f |
        0x202a..=0x202e | 0x2066..=0x2069 |
        0xfe00..=0xfe0f
    )
}

// ---------------------------------------------------------------------------
// invisible_display_name / bidi_display_name / unicode_name_char
// ---------------------------------------------------------------------------

pub(crate) fn invisible_display_name(c: char) -> &'static str {
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

pub(crate) fn bidi_display_name(c: char) -> &'static str {
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

pub(crate) fn unicode_name_char(c: char) -> String {
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

// ---------------------------------------------------------------------------
// is_combining_mark
// ---------------------------------------------------------------------------

pub(crate) fn is_combining_mark(c: char) -> bool {
    let cp = c as u32;
    matches!(cp,
        0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF |
        0x20D0..=0x20FF | 0xFE20..=0xFE2F
    )
}

// ---------------------------------------------------------------------------
// build_safe_repr
// ---------------------------------------------------------------------------

pub(crate) fn build_safe_repr(text: &str) -> String {
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

// ---------------------------------------------------------------------------
// apply_detail_limit / inspect_max_items
// ---------------------------------------------------------------------------

pub(crate) fn apply_detail_limit(
    arr: &[serde_json::Value],
    max_items: usize,
) -> Vec<serde_json::Value> {
    if arr.len() > max_items {
        arr.iter().take(max_items).cloned().collect()
    } else {
        arr.to_vec()
    }
}

pub(crate) fn inspect_max_items(detail: &str) -> usize {
    if detail == "summary" {
        10
    } else {
        MAX_INSPECT_ITEMS
    }
}

// ---------------------------------------------------------------------------
// build_extract_summary / json_value_preview / get_json_type
// get_json_type_detail / get_python_json_type
// ---------------------------------------------------------------------------

pub(crate) fn build_extract_summary(v: &serde_json::Value) -> String {
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

pub(crate) fn json_value_preview(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

pub(crate) fn get_json_type(v: &serde_json::Value) -> &str {
    match v {
        serde_json::Value::Object(_) => "object",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Null => "null",
    }
}

pub(crate) fn get_json_type_detail(v: &serde_json::Value) -> &str {
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
pub(crate) fn get_python_json_type(v: &serde_json::Value) -> &str {
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

// ---------------------------------------------------------------------------
// compare_json_values (and supporting types)
// ---------------------------------------------------------------------------

pub(crate) struct JsonDiff {
    pub(crate) path: String,
    pub(crate) kind: String,
    pub(crate) a_type: Option<String>,
    pub(crate) b_type: Option<String>,
    pub(crate) a_preview: Option<String>,
    pub(crate) b_preview: Option<String>,
}

#[derive(Clone, Copy)]
pub(crate) struct JsonCompareOptions {
    pub(crate) ignore_object_order: bool,
    pub(crate) ignore_array_order: bool,
    pub(crate) numeric_string_equivalence: bool,
    pub(crate) casefold_keys: bool,
    pub(crate) treat_missing_null_as_equal: bool,
    pub(crate) max_diffs: usize,
}

pub(crate) struct JsonCompareState {
    pub(crate) diffs: Vec<JsonDiff>,
    pub(crate) type_match: bool,
}

pub(crate) fn compare_json_values(
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

                    let a_keys_vec: Vec<&String> = obj_a.keys().collect();
                    let b_keys_vec: Vec<&String> = obj_b.keys().collect();

                    for i in 0..min_len {
                        if a_key_order[i] != b_key_order[i] {
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

// ---------------------------------------------------------------------------
// json_canonicalize_invalid_response
// ---------------------------------------------------------------------------

pub(crate) fn json_canonicalize_invalid_response(error: serde_json::Error) -> ToolResponse {
    ToolResponse::success(
        serde_json::json!({
            "valid": false,
            "canonical": null,
            "minified": null,
            "sha256": null,
            "duplicate_keys": [],
            "top_level_type": null,
            "top_level_keys": null,
            "error": error.to_string(),
            "line": error.line(),
            "column": error.column(),
        }),
        Some("json_canonicalize"),
    )
    .with_tool("json_canonicalize")
}

// ---------------------------------------------------------------------------
// detect_duplicates_in_json
// ---------------------------------------------------------------------------

pub(crate) fn detect_duplicates_in_json(text: &str, duplicates: &mut Vec<String>) {
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

// ---------------------------------------------------------------------------
// sort_json_keys
// ---------------------------------------------------------------------------

pub(crate) fn sort_json_keys(v: &serde_json::Value) -> serde_json::Value {
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

// ---------------------------------------------------------------------------
// escape_ascii
// ---------------------------------------------------------------------------

pub(crate) fn escape_ascii(s: &str) -> String {
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
