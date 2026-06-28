use fancy_regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::text::primitives::byte_offset_to_char_index;

/// Check if a pattern requires fancy_regex features (lookahead/lookbehind).
fn needs_fancy_regex(pattern: &str) -> bool {
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            chars.next();
            continue;
        }
        if c == '(' && chars.peek() == Some(&'?') {
            chars.next();
            if let Some(&next) = chars.peek() {
                if next == '=' || next == '!' {
                    return true;
                }
                if next == '<' {
                    // Check for lookbehind: (?<= or (?<!, but not (?<name>...)
                    if let Some(&next2) = chars.peek() {
                        if next2 == '=' || next2 == '!' {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

const MAX_PATTERN_LENGTH: usize = 1000;
const MAX_PATTERN_NESTING: usize = 5;
const MAX_SAMPLE_LENGTH: usize = 10_000;
const MAX_INPUT_LENGTH: usize = 100_000;
const MAX_SCHEMA_DEPTH: usize = 50;
const MAX_SCHEMA_ELEMENTS: usize = 100_000;
const MAX_SCHEMA_VIOLATIONS: usize = 100;
const MAX_LIST_ITEMS: usize = 10_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BracketError {
    pub char: String,
    pub index: i32,
    pub line: i32,
    pub column: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckBracketsResult {
    pub balanced: bool,
    pub unmatched_openers: Vec<BracketError>,
    pub unmatched_closers: Vec<BracketError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateJsonResult {
    pub valid: bool,
    pub error: Option<String>,
    pub line: Option<i32>,
    pub column: Option<i32>,
    pub position: Option<i32>,
    #[serde(rename = "type")]
    pub json_type: Option<String>,
    pub top_level_keys: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegexMatch {
    pub sample: String,
    pub matches: bool,
    pub fullmatch: bool,
    pub span: Option<Vec<i32>>,
    pub groups: Vec<String>,
    pub groupdict: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegexTestResult {
    pub valid_pattern: bool,
    pub results: Vec<RegexMatch>,
    pub error: Option<String>,
    pub flags_used: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonShapeKey {
    #[serde(rename = "type")]
    pub key_type: String,
    pub keys: Option<HashMap<String, JsonShapeKey>>,
    pub key_count: Option<usize>,
    pub item_types: Option<Vec<String>>,
    pub item_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonShapeResult {
    pub valid: bool,
    pub shape: Option<JsonShapeKey>,
    pub truncated: bool,
    pub summary: String,
}

fn get_line_column(text: &str, index: usize) -> (i32, i32) {
    let mut line = 1;
    let mut column = 1;
    for (i, c) in text.char_indices() {
        if i >= index {
            break;
        }
        if c == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}

pub fn validate_brackets(text: &str) -> Result<CheckBracketsResult, String> {
    if text.len() > MAX_INPUT_LENGTH {
        return Err(format!(
            "Input length {} exceeds MAX_INPUT_LENGTH {}",
            text.len(),
            MAX_INPUT_LENGTH
        ));
    }
    let pairs: HashMap<char, char> = [('(', ')'), ('[', ']'), ('{', '}'), ('<', '>')]
        .iter()
        .map(|(k, v)| (*k, *v))
        .collect();
    validate_brackets_with_pairs(text, &pairs)
}

pub fn validate_brackets_with_pairs(
    text: &str,
    pairs: &HashMap<char, char>,
) -> Result<CheckBracketsResult, String> {
    let openers: std::collections::HashSet<char> = pairs.keys().cloned().collect();
    let closers: std::collections::HashSet<char> = pairs.values().cloned().collect();

    let mut stack: Vec<(char, usize)> = Vec::new();
    let mut unmatched_openers: Vec<BracketError> = Vec::new();
    let mut unmatched_closers: Vec<BracketError> = Vec::new();

    for (index, c) in text.char_indices() {
        if openers.contains(&c) {
            stack.push((c, index));
        } else if closers.contains(&c) {
            if let Some((opener, opener_index)) = stack.pop() {
                if pairs.get(&opener) != Some(&c) {
                    let (line, column) = get_line_column(text, opener_index);
                    unmatched_openers.push(BracketError {
                        char: opener.to_string(),
                        index: opener_index as i32,
                        line,
                        column,
                    });
                    let (line, column) = get_line_column(text, index);
                    unmatched_closers.push(BracketError {
                        char: c.to_string(),
                        index: index as i32,
                        line,
                        column,
                    });
                }
            } else {
                let (line, column) = get_line_column(text, index);
                unmatched_closers.push(BracketError {
                    char: c.to_string(),
                    index: index as i32,
                    line,
                    column,
                });
            }
        }
    }

    for (opener, opener_index) in stack {
        let (line, column) = get_line_column(text, opener_index);
        unmatched_openers.push(BracketError {
            char: opener.to_string(),
            index: opener_index as i32,
            line,
            column,
        });
    }

    Ok(CheckBracketsResult {
        balanced: unmatched_openers.is_empty() && unmatched_closers.is_empty(),
        unmatched_openers,
        unmatched_closers,
    })
}

pub fn validate_json(text: &str) -> Result<ValidateJsonResult, String> {
    match serde_json::from_str::<serde_json::Value>(text) {
        Ok(parsed) => {
            let json_type = match &parsed {
                serde_json::Value::Object(_) => Some("object".to_string()),
                serde_json::Value::Array(_) => Some("array".to_string()),
                serde_json::Value::String(_) => Some("str".to_string()),
                serde_json::Value::Number(n) => {
                    if n.is_i64() || n.is_u64() {
                        Some("int".to_string())
                    } else {
                        Some("float".to_string())
                    }
                }
                serde_json::Value::Bool(_) => Some("bool".to_string()),
                serde_json::Value::Null => Some("NoneType".to_string()),
            };

            let top_level_keys = match &parsed {
                serde_json::Value::Object(m) => Some(m.keys().cloned().collect()),
                _ => None,
            };

            Ok(ValidateJsonResult {
                valid: true,
                error: None,
                line: None,
                column: None,
                position: None,
                json_type,
                top_level_keys,
            })
        }
        Err(e) => {
            let line = e.line() as i32;
            // serde_json column() returns 1-based byte offset within the line,
            // but returns 0 for empty strings. Handle both cases.
            let byte_col = e.column();
            let line_start_byte: usize = {
                let target_line = (line as usize).saturating_sub(1);
                let mut byte_pos = 0;
                let bytes = text.as_bytes();
                let mut lines_found = 0;
                while lines_found < target_line && byte_pos < bytes.len() {
                    match bytes[byte_pos] {
                        b'\r' => {
                            byte_pos += 1;
                            if byte_pos < bytes.len() && bytes[byte_pos] == b'\n' {
                                byte_pos += 1; // skip \r\n
                            } else {
                                byte_pos += 1; // skip \r
                            }
                        }
                        b'\n' => {
                            byte_pos += 1; // skip \n
                        }
                        _ => {
                            byte_pos += 1;
                            continue; // still on same line, don't increment lines_found
                        }
                    }
                    lines_found += 1;
                }
                byte_pos
            };
            let byte_pos_in_line = byte_col.saturating_sub(1); // 0-based byte offset within line
            let remaining = if line_start_byte < text.len() {
                &text[line_start_byte..]
            } else {
                ""
            };
            let char_col_in_line = byte_offset_to_char_index(remaining, byte_pos_in_line)
                .unwrap_or(remaining.chars().count());
            let column = (char_col_in_line + 1) as i32; // 1-based char column
                                                        // Compute character position from start of string (matches Python's JSONDecodeError.pos)
            let position = byte_offset_to_char_index(text, line_start_byte + byte_pos_in_line)
                .unwrap_or(text.chars().count()) as i32;
            // Match Python's json.JSONDecodeError.msg format. serde_json
            // strips to a raw message like "key must be a string", while
            // Python's json module emits descriptions like "Expecting
            // property name enclosed in double quotes". Map the most common
            // serde_json messages to their Python equivalents so parity
            // tests see the same string.
            let raw_msg = strip_serde_json_position(&e.to_string());
            let mapped_msg = map_json_error_to_python(text, &raw_msg, position);
            Ok(ValidateJsonResult {
                valid: false,
                error: Some(mapped_msg),
                line: Some(line),
                column: Some(column),
                position: Some(position),
                json_type: None,
                top_level_keys: None,
            })
        }
    }
}

fn strip_serde_json_position(s: &str) -> String {
    // serde_json formats errors as "<msg> at line N column M". Strip that
    // suffix so the message matches Python's json.JSONDecodeError.msg
    // (which is the raw message without any position info).
    if let Some(idx) = s.rfind(" at line ") {
        // After " at line " the format is "<N> column <M>". Verify by
        // checking that the rest of the string is "DIGITS column DIGITS"
        // (possibly with trailing punctuation that serde_json doesn't add,
        // but in practice the format ends at the column digits).
        let suffix = &s[idx + " at line ".len()..];
        if let Some(col_idx) = suffix.find(" column ") {
            let line_part = &suffix[..col_idx];
            let after_col = &suffix[col_idx + " column ".len()..];
            if !line_part.is_empty()
                && line_part.chars().all(|c| c.is_ascii_digit())
                && !after_col.is_empty()
                && after_col.chars().all(|c| c.is_ascii_digit())
            {
                return s[..idx].to_string();
            }
        }
    }
    s.to_string()
}

fn map_json_error_to_python(input: &str, raw_msg: &str, position: i32) -> String {
    // Map common serde_json error messages to the equivalent messages
    // produced by Python's json.JSONDecodeError. The strings below were
    // captured directly from CPython 3.12's json module.
    match raw_msg {
        "key must be a string" => "Expecting property name enclosed in double quotes",
        "EOF while parsing an object" => "Expecting property name enclosed in double quotes",
        "expected value" => "Expecting value",
        "EOF while parsing a list" => "Expecting value",
        "EOF while parsing a value" => "Expecting value",
        "EOF while parsing a string" => "Unterminated string starting at",
        "trailing characters" => "Extra data",
        "expected `:`" => "Expecting ':' delimiter",
        "expected `,` or `}`" => "Expecting ',' delimiter",
        "expected `,` or `]`" => "Expecting ',' delimiter",
        "expected ident" => "Expecting value",
        "expected `\"`" => "Expecting property name enclosed in double quotes",
        "control character (\\u0000-\\u001F) found while parsing a string" => {
            "Invalid control character at"
        }
        "invalid escape" => "Invalid \\escape",
        "invalid number" => "Expecting value",
        "number out of range" => "Number out of range",
        "lone leading surrogate in hex escape" => "Unterminated string starting at",
        "unexpected end of hex escape" => "Unterminated string starting at",
        "invalid unicode code point" => "Invalid \\uXXXX escape",
        "trailing comma" => {
            // Python distinguishes between object and list trailing commas.
            // We infer the context from the input by scanning backwards
            // from the error position to find the most recent unquoted
            // opening bracket.
            if context_at_position(input, position).starts_with('[') {
                "Illegal trailing comma before end of list"
            } else {
                "Illegal trailing comma before end of object"
            }
        }
        "recursion limit exceeded" => "Input string exceeds recursion limit",
        other => other,
    }
    .to_string()
}

fn context_at_position(input: &str, position: i32) -> String {
    // Returns the most recent non-whitespace character at or before the
    // given position, or empty string if position is out of range / negative.
    // Position is a character (codepoint) offset.
    if position < 0 {
        return String::new();
    }
    let char_pos = position as usize;
    let chars: Vec<char> = input.chars().collect();
    if char_pos > chars.len() {
        return String::new();
    }
    chars[..char_pos]
        .iter()
        .rev()
        .find(|c| !c.is_whitespace())
        .map(|c| c.to_string())
        .unwrap_or_default()
}

fn apply_flags(
    pattern: &str,
    flags: Option<&Vec<String>>,
    ignore_case: bool,
    multiline: bool,
    dotall: bool,
    _ascii: bool,
) -> String {
    let mut result = String::new();
    if ignore_case {
        result.push_str("(?i)");
    }
    if multiline {
        result.push_str("(?m)");
    }
    if dotall {
        result.push_str("(?s)");
    }
    if let Some(flag_list) = flags {
        for flag in flag_list {
            match flag.to_uppercase().as_str() {
                "IGNORECASE" | "I" => result.push_str("(?i)"),
                "MULTILINE" | "M" => result.push_str("(?m)"),
                "DOTALL" | "S" => result.push_str("(?s)"),
                "VERBOSE" | "X" => result.push_str("(?x)"),
                _ => {}
            }
        }
    }
    result.push_str(pattern);
    result
}

fn check_pattern_complexity(pattern: &str) -> Result<(), String> {
    let char_count = pattern.chars().count();
    if char_count > MAX_PATTERN_LENGTH {
        return Err(format!(
            "Pattern length {} exceeds maximum {}",
            char_count, MAX_PATTERN_LENGTH
        ));
    }

    let chars_vec: Vec<char> = pattern.chars().collect();
    let len = chars_vec.len();

    let mut nesting_depth: i32 = 0;
    let mut max_nesting: i32 = 0;
    let mut in_char_class = false;
    // Per-group state: whether a quantifier was seen directly in this group's content
    let mut group_stack: Vec<bool> = Vec::new();
    // Whether the immediately-preceding group had a quantifier in its content
    let mut prev_group_had_quantifier = false;

    let mut i: usize = 0;
    while i < len {
        let c = chars_vec[i];

        if c == '\\' && i + 1 < len {
            prev_group_had_quantifier = false;
            i += 2;
            continue;
        }

        if c == '[' {
            nesting_depth += 1;
            max_nesting = max_nesting.max(nesting_depth);
            in_char_class = true;
        } else if c == ']' {
            nesting_depth = nesting_depth.saturating_sub(1);
            in_char_class = false;
        } else if c == '(' && !in_char_class {
            nesting_depth += 1;
            max_nesting = max_nesting.max(nesting_depth);
            group_stack.push(false);
            prev_group_had_quantifier = false;
        } else if c == ')' && !in_char_class {
            nesting_depth -= 1;
            if let Some(inner_had_quantifier) = group_stack.pop() {
                // OR the inner group's state into the parent group
                if let Some(parent) = group_stack.last_mut() {
                    *parent = *parent || inner_had_quantifier;
                }
                prev_group_had_quantifier = inner_had_quantifier;
            } else {
                prev_group_had_quantifier = false;
            }
        } else if (c == '+' || c == '*' || c == '?') && !in_char_class {
            // ? after ( is group syntax ((?: ), (?= ), (?! ), (?<= ), (?<! )),
            // not a quantifier on a preceding element.
            if c == '?' && i > 0 && chars_vec[i - 1] == '(' {
                prev_group_had_quantifier = false;
            } else {
                // Check if previous char was also a quantifier (e.g., ++)
                if i > 0
                    && (chars_vec[i - 1] == '+'
                        || chars_vec[i - 1] == '*'
                        || chars_vec[i - 1] == '?')
                {
                    return Err(format!("Adjacent quantifiers detected at position {}", i));
                }
                // Check if a group with inner quantifier was just closed
                if prev_group_had_quantifier {
                    return Err(format!(
                        "Nested quantifiers detected at position {}: \
                         quantifier after group with internal quantifier",
                        i
                    ));
                }
                // Mark current group as having a quantifier
                if let Some(top) = group_stack.last_mut() {
                    *top = true;
                }
                prev_group_had_quantifier = false;
            }
        } else if c == '{' && !in_char_class {
            // Check if this is a {n,m} quantifier
            let j = i + 1;
            if j < len && chars_vec[j].is_ascii_digit() {
                let mut k = j;
                while k < len && chars_vec[k].is_ascii_digit() {
                    k += 1;
                }
                if k < len && chars_vec[k] == ',' {
                    k += 1;
                    while k < len && chars_vec[k].is_ascii_digit() {
                        k += 1;
                    }
                    if k < len && chars_vec[k] == '}' {
                        // This is a {n,m} quantifier
                        if prev_group_had_quantifier {
                            return Err(format!(
                                "Nested quantifiers detected at position {}: \
                                 {{n,m}} quantifier after group with internal quantifier",
                                i
                            ));
                        }
                        if let Some(top) = group_stack.last_mut() {
                            *top = true;
                        }
                        prev_group_had_quantifier = false;
                        i = k; // skip past the closing }
                    }
                } else if k < len && chars_vec[k] == '}' {
                    // {n} quantifier
                    if prev_group_had_quantifier {
                        return Err(format!(
                            "Nested quantifiers detected at position {}: \
                             {{n}} quantifier after group with internal quantifier",
                            i
                        ));
                    }
                    if let Some(top) = group_stack.last_mut() {
                        *top = true;
                    }
                    prev_group_had_quantifier = false;
                    i = k; // skip past the closing }
                } else {
                    prev_group_had_quantifier = false;
                }
            } else {
                prev_group_had_quantifier = false;
            }
        } else {
            prev_group_had_quantifier = false;
        }

        i += 1;
    }

    if max_nesting > MAX_PATTERN_NESTING as i32 {
        return Err(format!(
            "Pattern nesting depth {} exceeds maximum {}",
            max_nesting, MAX_PATTERN_NESTING
        ));
    }

    if nesting_depth < 0 {
        return Err("Unmatched closing parenthesis".to_string());
    }

    Ok(())
}

pub fn regex_test(
    pattern: &str,
    samples: &[&str],
    flags: Option<&Vec<String>>,
    ignore_case: bool,
    multiline: bool,
    dotall: bool,
    ascii: bool,
) -> RegexTestResult {
    if let Err(e) = check_pattern_complexity(pattern) {
        return RegexTestResult {
            valid_pattern: false,
            results: vec![],
            error: Some(e),
            flags_used: None,
        };
    }

    let full_pattern = apply_flags(pattern, flags, ignore_case, multiline, dotall, ascii);

    let re = match Regex::new(&full_pattern) {
        Ok(r) => r,
        Err(e) => {
            return RegexTestResult {
                valid_pattern: false,
                results: vec![],
                error: Some(e.to_string()),
                flags_used: None,
            }
        }
    };

    let mut results = Vec::new();

    for &sample in samples {
        let sample_chars = sample.chars().count();
        if sample_chars > MAX_SAMPLE_LENGTH {
            return RegexTestResult {
                valid_pattern: true,
                results: vec![],
                error: Some(format!(
                    "Sample length {} exceeds MAX_SAMPLE_LENGTH {}",
                    sample_chars, MAX_SAMPLE_LENGTH
                )),
                flags_used: None,
            };
        }
        let find_result = re.find(sample);
        let m = match find_result {
            Ok(m) => m,
            Err(_) => {
                results.push(RegexMatch {
                    sample: sample.to_string(),
                    matches: false,
                    fullmatch: false,
                    span: None,
                    groups: vec![],
                    groupdict: HashMap::new(),
                });
                continue;
            }
        };

        let m = match m {
            Some(m) => m,
            None => {
                results.push(RegexMatch {
                    sample: sample.to_string(),
                    matches: false,
                    fullmatch: false,
                    span: None,
                    groups: vec![],
                    groupdict: HashMap::new(),
                });
                continue;
            }
        };

        let is_fullmatch = m.start() == 0 && m.end() == sample.len();
        let span = Some(vec![
            byte_offset_to_char_index(sample, m.start()).unwrap_or(sample.chars().count()) as i32,
            byte_offset_to_char_index(sample, m.end()).unwrap_or(sample.chars().count()) as i32,
        ]);

        let caps_result = re.captures(sample);
        let caps = caps_result.unwrap_or_default();

        let mut groups = Vec::new();
        let mut groupdict = HashMap::new();

        if let Some(caps) = caps {
            for i in 1..caps.len() {
                if let Some(cap) = caps.get(i) {
                    groups.push(cap.as_str().to_string());
                } else {
                    groups.push(String::new());
                }
            }

            for name in re.capture_names().flatten() {
                if let Some(cap) = caps.name(name) {
                    groupdict.insert(name.to_string(), cap.as_str().to_string());
                }
            }
        }

        results.push(RegexMatch {
            sample: sample.to_string(),
            matches: true,
            fullmatch: is_fullmatch,
            span,
            groups,
            groupdict,
        });
    }

    let flags_used = serde_json::json!({
        "ignore_case": ignore_case,
        "multiline": multiline,
        "dotall": dotall,
        "ascii": ascii,
    });

    RegexTestResult {
        valid_pattern: true,
        results,
        error: None,
        flags_used: Some(flags_used),
    }
}

pub fn validate_regex(pattern: &str, text: &str) -> Result<bool, String> {
    check_pattern_complexity(pattern)?;
    let re = Regex::new(pattern).map_err(|e| e.to_string())?;
    match re.is_match(text) {
        Ok(b) => Ok(b),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_brackets_balanced() {
        let result = validate_brackets("(a + b)").unwrap();
        assert!(result.balanced);
        assert!(result.unmatched_openers.is_empty());
        assert!(result.unmatched_closers.is_empty());
    }

    #[test]
    fn test_validate_brackets_unbalanced() {
        let result = validate_brackets("(a + b").unwrap();
        assert!(!result.balanced);
        assert_eq!(result.unmatched_openers.len(), 1);
        assert_eq!(result.unmatched_openers[0].char, "(");
    }

    #[test]
    fn test_validate_brackets_with_position() {
        let result = validate_brackets("line1\nline2\n(a + b").unwrap();
        assert!(!result.balanced);
        let opener = &result.unmatched_openers[0];
        assert_eq!(opener.char, "(");
        assert!(opener.line >= 1);
        assert!(opener.column >= 1);
    }

    #[test]
    fn test_validate_json_valid() {
        let result = validate_json("{}").unwrap();
        assert!(result.valid);
        assert!(result.error.is_none());
        assert_eq!(result.json_type, Some("object".to_string()));
    }

    #[test]
    fn test_validate_json_valid_array() {
        let result = validate_json("[1, 2, 3]").unwrap();
        assert!(result.valid);
        assert_eq!(result.json_type, Some("array".to_string()));
    }

    #[test]
    fn test_validate_json_invalid() {
        let result = validate_json("{").unwrap();
        assert!(!result.valid);
        assert!(result.error.is_some());
        assert!(result.line.is_some());
        assert!(result.column.is_some());
    }

    #[test]
    fn test_validate_regex_match() {
        assert_eq!(validate_regex(r"\d+", "123"), Ok(true));
        assert_eq!(validate_regex(r"\w+", "hello"), Ok(true));
        assert_eq!(validate_regex(r"^hello", "hello world"), Ok(true));
    }

    #[test]
    fn test_validate_regex_no_match() {
        assert_eq!(validate_regex(r"\d+", "abc"), Ok(false));
        assert_eq!(validate_regex(r"^\d+$", "abc123def"), Ok(false));
    }

    #[test]
    fn test_validate_regex_invalid_pattern() {
        assert!(validate_regex("[", "text").is_err());
    }

    #[test]
    fn test_validate_regex_pattern_too_long() {
        let long_pattern = "a".repeat(1001);
        assert!(validate_regex(&long_pattern, "text").is_err());
    }

    #[test]
    fn test_validate_regex_nesting_too_deep() {
        let nested_pattern = "(".repeat(10).to_string() + "a" + &")".repeat(10);
        assert!(validate_regex(&nested_pattern, "text").is_err());
    }

    #[test]
    fn test_regex_test_basic() {
        let result = regex_test(r"\d+", &["123", "abc"], None, false, false, false, false);
        assert!(result.valid_pattern);
        assert!(result.error.is_none());
        assert_eq!(result.results.len(), 2);
        assert!(result.results[0].matches);
    }

    #[test]
    fn test_regex_test_with_groups() {
        let result = regex_test(
            r"(\d+)-(\d+)",
            &["123-456", "abc-def"],
            None,
            false,
            false,
            false,
            false,
        );
        assert!(result.valid_pattern);
        assert!(result.results[0].matches);
        assert_eq!(result.results[0].groups, vec!["123", "456"]);
    }

    #[test]
    fn test_regex_test_fullmatch() {
        let result = regex_test(r"\d+", &["123", "abc123"], None, false, false, false, false);
        assert!(result.results[0].fullmatch);
        assert!(!result.results[1].fullmatch);
    }

    #[test]
    fn test_regex_test_span() {
        let result = regex_test(r"\d+", &["abc123xyz"], None, false, false, false, false);
        assert!(result.results[0].matches);
        assert_eq!(result.results[0].span, Some(vec![3, 6]));
    }

    #[test]
    fn test_regex_test_with_flags() {
        let flags = vec!["IGNORECASE".to_string()];
        let result = regex_test(
            r"hello",
            &["HELLO", "hello"],
            Some(&flags),
            false,
            false,
            false,
            false,
        );
        assert!(result.valid_pattern);
        assert!(result.results[0].matches);
        assert!(result.results[1].matches);
    }

    #[test]
    fn test_regex_test_invalid_pattern() {
        let result = regex_test(r"[", &[], None, false, false, false, false);
        assert!(!result.valid_pattern);
        assert!(result.error.is_some());
    }

    #[test]
    fn test_regex_test_lookahead() {
        let result = regex_test(
            r"\d+(?=px)",
            &["100px", "200em"],
            None,
            false,
            false,
            false,
            false,
        );
        assert!(result.valid_pattern);
        assert!(result.results[0].matches);
        assert!(!result.results[1].matches);
    }

    #[test]
    fn test_regex_test_lookbehind() {
        let result = regex_test(
            r"(?<=\$)\d+",
            &["$100", "€200"],
            None,
            false,
            false,
            false,
            false,
        );
        assert!(result.valid_pattern);
        assert!(result.results[0].matches);
        assert!(!result.results[1].matches);
    }

    #[test]
    fn test_regex_test_complexity_rejected() {
        let long_pattern = "(".repeat(10).to_string() + "a" + &")".repeat(10);
        let result = regex_test(&long_pattern, &["test"], None, false, false, false, false);
        assert!(!result.valid_pattern);
        assert!(result.error.is_some());
    }

    #[test]
    fn test_regex_test_backtracking_nested() {
        let pattern = r"(a+)+b";
        let result = regex_test(pattern, &["aaaaaax"], None, false, false, false, false);
        assert!(!result.valid_pattern);
    }

    #[test]
    fn test_validate_json_integer_type() {
        let result = validate_json("42").unwrap();
        assert_eq!(result.json_type.as_deref(), Some("int"));
    }

    #[test]
    fn test_validate_json_float_type() {
        let result = validate_json("3.14").unwrap();
        assert_eq!(result.json_type.as_deref(), Some("float"));
    }

    #[test]
    fn test_regex_finditer_text_too_long_valid_pattern_false() {
        let long_text = "a".repeat(200_001);
        let result = regex_finditer("a", &long_text, None, 100, false, false);
        assert!(!result.valid_pattern);
        assert!(result.error.is_some());
    }
}

pub fn json_shape(
    text: &str,
    max_depth: usize,
    max_keys: usize,
    max_array_items: usize,
) -> Result<JsonShapeResult, String> {
    if text.len() > MAX_PATTERN_LENGTH * 100 {
        return Err(format!("Input length {} exceeds limit", text.len()));
    }

    let parsed: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            return Ok(JsonShapeResult {
                valid: false,
                shape: None,
                truncated: false,
                summary: format!("Invalid JSON: {}", e),
            });
        }
    };

    fn analyze_shape(
        value: &serde_json::Value,
        depth: usize,
        max_depth: usize,
        max_keys: usize,
        max_array_items: usize,
    ) -> JsonShapeKey {
        match value {
            serde_json::Value::Object(obj) => {
                let key_count = obj.len();
                if depth >= max_depth {
                    return JsonShapeKey {
                        key_type: "object".to_string(),
                        keys: None,
                        key_count: Some(key_count),
                        item_types: None,
                        item_count: None,
                    };
                }

                let mut keys = HashMap::new();
                let mut shown = 0;
                for (k, v) in obj.iter() {
                    if shown >= max_keys {
                        break;
                    }
                    keys.insert(
                        k.clone(),
                        analyze_shape(v, depth + 1, max_depth, max_keys, max_array_items),
                    );
                    shown += 1;
                }

                JsonShapeKey {
                    key_type: "object".to_string(),
                    keys: if keys.is_empty() { None } else { Some(keys) },
                    key_count: if key_count > shown {
                        Some(key_count)
                    } else {
                        None
                    },
                    item_types: None,
                    item_count: None,
                }
            }
            serde_json::Value::Array(arr) => {
                let item_count = arr.len();
                if depth >= max_depth {
                    return JsonShapeKey {
                        key_type: "array".to_string(),
                        keys: None,
                        key_count: None,
                        item_types: None,
                        item_count: Some(item_count),
                    };
                }

                let mut item_types = Vec::new();
                for item in arr.iter().take(max_array_items) {
                    item_types.push(
                        analyze_shape(item, depth + 1, max_depth, max_keys, max_array_items)
                            .key_type,
                    );
                }

                JsonShapeKey {
                    key_type: "array".to_string(),
                    keys: None,
                    key_count: None,
                    item_types: Some(item_types),
                    item_count: Some(item_count),
                }
            }
            serde_json::Value::String(_) => JsonShapeKey {
                key_type: "string".to_string(),
                keys: None,
                key_count: None,
                item_types: None,
                item_count: None,
            },
            serde_json::Value::Number(n) => {
                let key_type = if n.is_i64() || n.is_u64() {
                    "integer"
                } else {
                    "float"
                }
                .to_string();
                JsonShapeKey {
                    key_type,
                    keys: None,
                    key_count: None,
                    item_types: None,
                    item_count: None,
                }
            }
            serde_json::Value::Bool(_) => JsonShapeKey {
                key_type: "boolean".to_string(),
                keys: None,
                key_count: None,
                item_types: None,
                item_count: None,
            },
            serde_json::Value::Null => JsonShapeKey {
                key_type: "null".to_string(),
                keys: None,
                key_count: None,
                item_types: None,
                item_count: None,
            },
        }
    }

    let shape = analyze_shape(&parsed, 0, max_depth, max_keys, max_array_items);
    let summary = build_shape_summary(&shape);

    Ok(JsonShapeResult {
        valid: true,
        shape: Some(shape),
        truncated: false,
        summary,
    })
}

fn build_shape_summary(shape: &JsonShapeKey) -> String {
    if shape.key_type == "object" {
        let key_count = shape
            .key_count
            .unwrap_or_else(|| shape.keys.as_ref().map(|k| k.len()).unwrap_or(0));
        if let Some(keys) = &shape.keys {
            let mut sorted_keys: Vec<(&String, &JsonShapeKey)> = keys.iter().collect();
            sorted_keys.sort_by(|a, b| a.0.cmp(b.0));
            let mut sub_summaries = Vec::new();
            for (k, v) in sorted_keys.iter().take(3) {
                sub_summaries.push(format!("{}: {}", k, build_shape_summary(v)));
            }
            if keys.len() > 3 {
                return format!(
                    "object with {} keys ({{{}, ...}})",
                    key_count,
                    sub_summaries.join(", ")
                );
            }
            return format!(
                "object with {} keys ({{{}}})",
                key_count,
                sub_summaries.join(", ")
            );
        }
        format!("object with {} keys", key_count)
    } else if shape.key_type == "array" {
        let item_count = shape
            .item_count
            .unwrap_or_else(|| shape.item_types.as_ref().map(|t| t.len()).unwrap_or(0));
        if let Some(item_types) = &shape.item_types {
            let mut seen = std::collections::HashSet::new();
            let mut unique_types: Vec<String> = Vec::new();
            for t in item_types {
                if seen.insert(t.clone()) {
                    unique_types.push(t.clone());
                }
            }
            if unique_types.len() == 1 {
                return format!("array of {} with {} items", unique_types[0], item_count);
            }
            return format!(
                "array with {} items ([{}, ...])",
                item_count,
                unique_types.join(", ")
            );
        }
        format!("array with {} items", item_count)
    } else {
        shape.key_type.clone()
    }
}

const MAX_TEXT_LENGTH_REGEX: usize = 100_000;
const MAX_PATTERN_LENGTH_REGEX: usize = 1000;
const _MAX_MATCHES: usize = 100;
const _MAX_GROUPS: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegexFindIterMatch {
    pub m: String,
    pub span: Vec<i32>,
    pub line: Option<i32>,
    pub column: Option<i32>,
    pub groups: Vec<String>,
    #[serde(rename = "groupdict")]
    pub group_dict: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegexFindIterResult {
    pub valid_pattern: bool,
    pub matches: Vec<RegexFindIterMatch>,
    pub truncated: bool,
    pub match_count: i32,
    pub error: Option<String>,
}

fn get_line_column_for_index(text: &str, index: usize) -> (i32, i32) {
    let chars: Vec<char> = text.chars().collect();
    let mut line = 0usize;
    let mut column = 0usize;

    for i in 0..chars.len() {
        if i == index {
            return (line as i32 + 1, column as i32 + 1);
        }

        match chars[i] {
            '\n' => {
                line += 1;
                column = 0;
            }
            '\r' => {
                if i + 1 < chars.len() && chars[i + 1] == '\n' {
                    continue;
                }
                line += 1;
                column = 0;
            }
            _ => {
                column += 1;
            }
        }
    }

    if index >= chars.len() {
        return (line as i32 + 1, column as i32 + 1);
    }

    (line as i32 + 1, column as i32 + 1)
}

fn advance_to_next_char_boundary(text: &str, byte_offset: usize) -> usize {
    if byte_offset >= text.len() {
        return text.len();
    }
    match text[byte_offset..].chars().next() {
        Some(ch) => byte_offset + ch.len_utf8(),
        None => text.len(),
    }
}

pub fn regex_finditer(
    pattern: &str,
    text: &str,
    flags: Option<&Vec<String>>,
    max_matches: usize,
    include_line_column: bool,
    include_groups: bool,
) -> RegexFindIterResult {
    if text.chars().count() > MAX_TEXT_LENGTH_REGEX {
        return RegexFindIterResult {
            valid_pattern: false,
            matches: vec![],
            truncated: false,
            match_count: 0,
            error: Some(format!(
                "Text length {} exceeds MAX_TEXT_LENGTH_REGEX {}",
                text.chars().count(),
                MAX_TEXT_LENGTH_REGEX
            )),
        };
    }

    if pattern.chars().count() > MAX_PATTERN_LENGTH_REGEX {
        return RegexFindIterResult {
            valid_pattern: true,
            matches: vec![],
            truncated: false,
            match_count: 0,
            error: Some(format!(
                "Pattern length {} exceeds maximum {}",
                pattern.chars().count(),
                MAX_PATTERN_LENGTH_REGEX
            )),
        };
    }

    if let Err(e) = check_pattern_complexity(pattern) {
        return RegexFindIterResult {
            valid_pattern: false,
            matches: vec![],
            truncated: false,
            match_count: 0,
            error: Some(e),
        };
    }

    let mut case_insensitive = false;
    let mut multi_line = false;
    let mut dot_matches_new_line = false;

    if let Some(flag_list) = flags {
        for flag in flag_list {
            match flag.as_str() {
                "IGNORECASE" => case_insensitive = true,
                "MULTILINE" => multi_line = true,
                "DOTALL" => dot_matches_new_line = true,
                _ => {}
            }
        }
    }

    let pattern_with_flags = if case_insensitive || multi_line || dot_matches_new_line {
        let mut prefix = "(?".to_string();
        if case_insensitive {
            prefix.push('i');
        }
        if multi_line {
            prefix.push('m');
        }
        if dot_matches_new_line {
            prefix.push('s');
        }
        prefix.push(')');
        format!("{}{}", prefix, pattern)
    } else {
        pattern.to_string()
    };

    // Try standard regex crate first (handles \b natively), fall back to
    // fancy-regex only when the pattern uses lookaheads/lookbehinds.
    if !needs_fancy_regex(&pattern_with_flags) {
        let std_re = match regex::Regex::new(&pattern_with_flags) {
            Ok(r) => r,
            Err(e) => {
                return RegexFindIterResult {
                    valid_pattern: false,
                    matches: vec![],
                    truncated: false,
                    match_count: 0,
                    error: Some(format!("Invalid pattern: {}", e)),
                };
            }
        };

        let mut matches = Vec::new();
        let mut match_count = 0;
        let mut truncated = false;
        let mut start_pos = 0;

        while start_pos <= text.len() {
            let caps_opt = std_re.captures(&text[start_pos..]);
            let caps = match caps_opt {
                Some(c) => c,
                None => break,
            };
            match_count += 1;
            if matches.len() >= max_matches {
                truncated = true;
            }

            let first_match = caps.get(0).unwrap();
            let abs_start = start_pos + first_match.start();
            let abs_end = start_pos + first_match.end();
            start_pos = if abs_end == abs_start {
                advance_to_next_char_boundary(text, abs_start)
            } else {
                abs_end
            };

            if truncated {
                continue;
            }

            let mut groups = Vec::new();
            let mut group_dict = HashMap::new();

            if include_groups {
                for i in 1..caps.len() {
                    if let Some(cap) = caps.get(i) {
                        groups.push(cap.as_str().to_string());
                    }
                }
                for name in std_re.capture_names().flatten() {
                    if let Some(cap) = caps.name(name) {
                        group_dict.insert(name.to_string(), cap.as_str().to_string());
                    }
                }
            }

            let (line, column) = if include_line_column {
                let (l, c) = get_line_column_for_index(
                    text,
                    byte_offset_to_char_index(text, abs_start).unwrap_or(text.chars().count()),
                );
                (Some(l), Some(c))
            } else {
                (None, None)
            };

            matches.push(RegexFindIterMatch {
                m: first_match.as_str().to_string(),
                span: vec![
                    byte_offset_to_char_index(text, abs_start).unwrap_or(text.chars().count())
                        as i32,
                    byte_offset_to_char_index(text, abs_end).unwrap_or(text.chars().count()) as i32,
                ],
                line,
                column,
                groups,
                group_dict,
            });
        }

        return RegexFindIterResult {
            valid_pattern: true,
            matches,
            truncated,
            match_count,
            error: None,
        };
    }

    let compiled = match Regex::new(&pattern_with_flags) {
        Ok(c) => c,
        Err(e) => {
            return RegexFindIterResult {
                valid_pattern: false,
                matches: vec![],
                truncated: false,
                match_count: 0,
                error: Some(format!("Invalid pattern: {}", e)),
            };
        }
    };

    let mut matches = Vec::new();
    let mut match_count = 0;
    let mut truncated = false;
    let mut start_pos = 0;

    while start_pos <= text.len() {
        let caps_opt = match compiled.captures_from_pos(text, start_pos) {
            Ok(c) => c,
            Err(_) => break,
        };
        let caps = match caps_opt {
            Some(c) => c,
            None => break,
        };
        match_count += 1;
        if matches.len() >= max_matches {
            truncated = true;
        }

        let first_match = caps.get(0);
        let end_pos = first_match
            .map(|m| m.end())
            .unwrap_or_else(|| advance_to_next_char_boundary(text, start_pos));
        // Advance past this match; if it was zero-length, advance by 1
        start_pos = if end_pos == start_pos {
            advance_to_next_char_boundary(text, start_pos)
        } else {
            end_pos
        };

        if truncated {
            continue;
        }

        let mut groups = Vec::new();
        let mut group_dict = HashMap::new();

        if include_groups {
            for i in 1..caps.len() {
                if let Some(cap) = caps.get(i) {
                    groups.push(cap.as_str().to_string());
                }
            }
            for name in compiled.capture_names().flatten() {
                if let Some(cap) = caps.name(name) {
                    group_dict.insert(name.to_string(), cap.as_str().to_string());
                }
            }
        }

        let (line, column) = if include_line_column {
            if let Some(m) = first_match {
                let (l, c) = get_line_column_for_index(
                    text,
                    byte_offset_to_char_index(text, m.start()).unwrap_or(text.chars().count()),
                );
                (Some(l), Some(c))
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        matches.push(RegexFindIterMatch {
            m: first_match
                .map(|m| m.as_str().to_string())
                .unwrap_or_default(),
            span: first_match
                .map(|m| {
                    vec![
                        byte_offset_to_char_index(text, m.start()).unwrap_or(text.chars().count())
                            as i32,
                        byte_offset_to_char_index(text, m.end()).unwrap_or(text.chars().count())
                            as i32,
                    ]
                })
                .unwrap_or_default(),
            line,
            column,
            groups,
            group_dict,
        });
    }

    RegexFindIterResult {
        valid_pattern: true,
        matches,
        truncated,
        match_count,
        error: None,
    }
}

// ── JSON tool helpers ──────────────────────────────────────────────────

fn get_json_type(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(_) => "boolean".to_string(),
        serde_json::Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                "integer".to_string()
            } else {
                "float".to_string()
            }
        }
        serde_json::Value::String(_) => "string".to_string(),
        serde_json::Value::Array(_) => "array".to_string(),
        serde_json::Value::Object(_) => "object".to_string(),
    }
}

fn get_schema_type_name(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(_) => "boolean".to_string(),
        serde_json::Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                "integer".to_string()
            } else {
                "number".to_string()
            }
        }
        serde_json::Value::String(_) => "string".to_string(),
        serde_json::Value::Array(_) => "array".to_string(),
        serde_json::Value::Object(_) => "object".to_string(),
    }
}

fn value_preview(value: &serde_json::Value, max_len: usize) -> Option<String> {
    match value {
        serde_json::Value::Null => Some("null".to_string()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        serde_json::Value::Number(n) => {
            let s = n.to_string();
            if s.len() <= max_len {
                Some(s)
            } else {
                let truncated: String = s.chars().take(max_len.saturating_sub(3)).collect();
                Some(format!("{}...", truncated))
            }
        }
        serde_json::Value::String(s) => {
            if s.chars().count() <= max_len {
                Some(format!("\"{}\"", s))
            } else {
                let truncated: String = s.chars().take(max_len.saturating_sub(3)).collect();
                Some(format!("\"{}...\"", truncated))
            }
        }
        serde_json::Value::Array(a) => Some(format!("[{} items]", a.len())),
        serde_json::Value::Object(o) => Some(format!("{{{} keys}}", o.len())),
    }
}

fn sort_json_keys(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut sorted: Vec<_> = map.iter().collect();
            sorted.sort_by_key(|(k, _)| (*k).clone());
            let new_map: serde_json::Map<String, serde_json::Value> = sorted
                .into_iter()
                .map(|(k, v)| (k.clone(), sort_json_keys(v)))
                .collect();
            serde_json::Value::Object(new_map)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(sort_json_keys).collect())
        }
        other => other.clone(),
    }
}

fn decode_pointer_token(token: &str) -> String {
    token.replace("~1", "/").replace("~0", "~")
}

fn encode_pointer_token(token: &str) -> String {
    token.replace("~", "~0").replace("/", "~1")
}

fn is_serializable(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => true,
        serde_json::Value::Object(map) => map.values().all(is_serializable),
        serde_json::Value::Array(arr) => arr.iter().all(is_serializable),
    }
}

fn canonicalize_for_compare(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut sorted: Vec<_> = map.iter().collect();
            sorted.sort_by_key(|(k, _)| (*k).clone());
            let new_map: serde_json::Map<String, serde_json::Value> = sorted
                .into_iter()
                .map(|(k, v)| (k.clone(), canonicalize_for_compare(v)))
                .collect();
            serde_json::Value::Object(new_map)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(canonicalize_for_compare).collect())
        }
        other => other.clone(),
    }
}

// ── JSON extract ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonExtractResult {
    pub valid_json: bool,
    pub found: bool,
    pub pointer: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub value_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_keys: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub array_length: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub missing_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub available_keys: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<i32>,
    pub summary: String,
}

fn build_extract_found_result(
    value: &serde_json::Value,
    pointer: &str,
    max_output_chars: usize,
) -> JsonExtractResult {
    match value {
        serde_json::Value::Object(map) => {
            let child_keys: Vec<String> = map.keys().cloned().collect();
            let full = serde_json::to_string(value).unwrap_or_default();
            let char_count = full.chars().count();
            let truncated = char_count > max_output_chars;
            let preview = if truncated {
                full.chars().take(max_output_chars).collect()
            } else {
                full.clone()
            };
            JsonExtractResult {
                valid_json: true,
                found: true,
                pointer: pointer.to_string(),
                value_type: Some("object".to_string()),
                value: Some(value.clone()),
                preview: Some(preview),
                child_keys: Some(child_keys.clone()),
                array_length: None,
                truncated: Some(truncated),
                missing_at: None,
                reason: None,
                available_keys: None,
                error: None,
                line: None,
                column: None,
                summary: format!(
                    "Object with {} keys{}",
                    child_keys.len(),
                    if truncated { " (truncated)" } else { "" }
                ),
            }
        }
        serde_json::Value::Array(arr) => {
            let full = serde_json::to_string(value).unwrap_or_default();
            let char_count = full.chars().count();
            let truncated = char_count > max_output_chars;
            let preview = if truncated {
                full.chars().take(max_output_chars).collect()
            } else {
                full.clone()
            };
            JsonExtractResult {
                valid_json: true,
                found: true,
                pointer: pointer.to_string(),
                value_type: Some("array".to_string()),
                value: Some(value.clone()),
                preview: Some(preview),
                child_keys: None,
                array_length: Some(arr.len()),
                truncated: Some(truncated),
                missing_at: None,
                reason: None,
                available_keys: None,
                error: None,
                line: None,
                column: None,
                summary: format!(
                    "Array of {} elements{}",
                    arr.len(),
                    if truncated { " (truncated)" } else { "" }
                ),
            }
        }
        serde_json::Value::String(s) => {
            let truncated = s.chars().count() > max_output_chars;
            let preview = if truncated {
                s.chars().take(max_output_chars).collect()
            } else {
                s.clone()
            };
            let short = if s.chars().count() > 50 {
                let truncated_short: String = s.chars().take(50).collect();
                format!("{}...", truncated_short)
            } else {
                s.clone()
            };
            JsonExtractResult {
                valid_json: true,
                found: true,
                pointer: pointer.to_string(),
                value_type: Some("string".to_string()),
                value: Some(value.clone()),
                preview: Some(preview),
                child_keys: None,
                array_length: None,
                truncated: Some(truncated),
                missing_at: None,
                reason: None,
                available_keys: None,
                error: None,
                line: None,
                column: None,
                summary: format!("String: \"{}\"", short),
            }
        }
        serde_json::Value::Bool(b) => JsonExtractResult {
            valid_json: true,
            found: true,
            pointer: pointer.to_string(),
            value_type: Some("boolean".to_string()),
            value: Some(value.clone()),
            preview: Some(b.to_string()),
            child_keys: None,
            array_length: None,
            truncated: Some(false),
            missing_at: None,
            reason: None,
            available_keys: None,
            error: None,
            line: None,
            column: None,
            summary: format!("Boolean: {}", b),
        },
        serde_json::Value::Null => JsonExtractResult {
            valid_json: true,
            found: true,
            pointer: pointer.to_string(),
            value_type: Some("null".to_string()),
            value: Some(serde_json::Value::Null),
            preview: Some("null".to_string()),
            child_keys: None,
            array_length: None,
            truncated: Some(false),
            missing_at: None,
            reason: None,
            available_keys: None,
            error: None,
            line: None,
            column: None,
            summary: "null".to_string(),
        },
        serde_json::Value::Number(n) => JsonExtractResult {
            valid_json: true,
            found: true,
            pointer: pointer.to_string(),
            value_type: Some("number".to_string()),
            value: Some(value.clone()),
            preview: Some(n.to_string()),
            child_keys: None,
            array_length: None,
            truncated: Some(false),
            missing_at: None,
            reason: None,
            available_keys: None,
            error: None,
            line: None,
            column: None,
            summary: format!("Number: {}", n),
        },
    }
}

pub fn json_extract(
    text: &str,
    pointer: &str,
    max_output_chars: usize,
) -> Result<JsonExtractResult, String> {
    if text.len() > MAX_INPUT_LENGTH {
        return Err(format!(
            "Input length {} exceeds MAX_INPUT_LENGTH {}",
            text.len(),
            MAX_INPUT_LENGTH
        ));
    }

    let parsed: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            let raw = strip_serde_json_position(&e.to_string());
            let mapped = map_json_error_to_python(text, &raw, 0);
            let line = e.line() as i32;
            let col = e.column() as i32;
            return Ok(JsonExtractResult {
                valid_json: false,
                found: false,
                pointer: pointer.to_string(),
                value_type: None,
                value: None,
                preview: None,
                child_keys: None,
                array_length: None,
                truncated: Some(false),
                missing_at: None,
                reason: Some("invalid_json".to_string()),
                available_keys: None,
                error: Some(mapped.clone()),
                line: Some(line),
                column: Some(col),
                summary: format!("Invalid JSON: {} at line {}, column {}", mapped, line, col),
            });
        }
    };

    if pointer.is_empty() {
        return Ok(build_extract_found_result(
            &parsed,
            pointer,
            max_output_chars,
        ));
    }

    let tokens: Vec<&str> = pointer.split('/').collect();
    let tokens: Vec<&str> = if tokens.first() == Some(&"") {
        tokens[1..].to_vec()
    } else {
        tokens
    };

    let mut current = &parsed;
    #[allow(unused_assignments)]
    let mut path_so_far = String::new();

    for (i, token) in tokens.iter().enumerate() {
        let decoded = decode_pointer_token(token);
        path_so_far = format!(
            "/{}",
            tokens[..=i]
                .iter()
                .map(|t| encode_pointer_token(t))
                .collect::<Vec<_>>()
                .join("/")
        );

        match current {
            serde_json::Value::Object(map) => {
                if let Some(val) = map.get(decoded.as_str()) {
                    current = val;
                } else {
                    let available_keys: Vec<String> = map.keys().cloned().collect();
                    return Ok(JsonExtractResult {
                        valid_json: true,
                        found: false,
                        pointer: pointer.to_string(),
                        value_type: Some("object".to_string()),
                        value: None,
                        preview: None,
                        child_keys: None,
                        array_length: None,
                        truncated: Some(false),
                        missing_at: Some(path_so_far.clone()),
                        reason: Some("key_not_found".to_string()),
                        available_keys: Some(available_keys),
                        error: None,
                        line: None,
                        column: None,
                        summary: format!(
                            "Key '{}' not found in object at {}",
                            decoded, path_so_far
                        ),
                    });
                }
            }
            serde_json::Value::Array(arr) => {
                let index: usize = match decoded.parse() {
                    Ok(i) => i,
                    Err(_) => {
                        return Ok(JsonExtractResult {
                            valid_json: true,
                            found: false,
                            pointer: pointer.to_string(),
                            value_type: Some("array".to_string()),
                            value: None,
                            preview: None,
                            child_keys: None,
                            array_length: Some(arr.len()),
                            truncated: Some(false),
                            missing_at: Some(path_so_far.clone()),
                            reason: Some("invalid_pointer_syntax".to_string()),
                            available_keys: None,
                            error: None,
                            line: None,
                            column: None,
                            summary: format!(
                                "Array index expected at {}, got non-integer '{}'",
                                path_so_far, decoded
                            ),
                        });
                    }
                };
                if index >= arr.len() {
                    return Ok(JsonExtractResult {
                        valid_json: true,
                        found: false,
                        pointer: pointer.to_string(),
                        value_type: Some("array".to_string()),
                        value: None,
                        preview: None,
                        child_keys: None,
                        array_length: Some(arr.len()),
                        truncated: Some(false),
                        missing_at: Some(path_so_far.clone()),
                        reason: Some("index_out_of_range".to_string()),
                        available_keys: None,
                        error: None,
                        line: None,
                        column: None,
                        summary: format!(
                            "Index {} out of range for array of length {} at {}",
                            index,
                            arr.len(),
                            path_so_far
                        ),
                    });
                }
                current = &arr[index];
            }
            other => {
                let type_name = match other {
                    serde_json::Value::String(_) => "string",
                    serde_json::Value::Number(_) => "number",
                    serde_json::Value::Bool(_) => "boolean",
                    serde_json::Value::Null => "null",
                    _ => "unknown",
                };
                return Ok(JsonExtractResult {
                    valid_json: true,
                    found: false,
                    pointer: pointer.to_string(),
                    value_type: Some(type_name.to_string()),
                    value: None,
                    preview: None,
                    child_keys: None,
                    array_length: None,
                    truncated: Some(false),
                    missing_at: Some(path_so_far.clone()),
                    reason: Some("invalid_pointer_syntax".to_string()),
                    available_keys: None,
                    error: None,
                    line: None,
                    column: None,
                    summary: format!("Cannot index into {} at {}", type_name, path_so_far),
                });
            }
        }
    }

    Ok(build_extract_found_result(
        current,
        pointer,
        max_output_chars,
    ))
}

// ── JSON compare ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonCompareDiff {
    pub path: String,
    pub kind: String,
    #[serde(rename = "a_type", skip_serializing_if = "Option::is_none")]
    pub a_type: Option<String>,
    #[serde(rename = "b_type", skip_serializing_if = "Option::is_none")]
    pub b_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub a_preview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub b_preview: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonCompareResult {
    pub valid_json_a: bool,
    pub valid_json_b: bool,
    pub equal: bool,
    pub same_type: bool,
    pub diff_count: usize,
    pub diffs: Vec<JsonCompareDiff>,
    pub truncated: bool,
    pub summary: String,
}

#[allow(clippy::too_many_arguments)]
pub fn json_compare(
    a: &str,
    b: &str,
    ignore_object_order: bool,
    ignore_array_order: bool,
    numeric_string_equivalence: bool,
    casefold_keys: bool,
    treat_missing_null_as_equal: bool,
    max_diffs: usize,
) -> Result<JsonCompareResult, String> {
    if a.len() > MAX_INPUT_LENGTH {
        return Err(format!(
            "Input 'a' length {} exceeds maximum {}",
            a.len(),
            MAX_INPUT_LENGTH
        ));
    }
    if b.len() > MAX_INPUT_LENGTH {
        return Err(format!(
            "Input 'b' length {} exceeds maximum {}",
            b.len(),
            MAX_INPUT_LENGTH
        ));
    }

    let mut diffs: Vec<JsonCompareDiff> = Vec::new();
    let mut valid_json_a = true;
    let mut valid_json_b = true;
    let mut parsed_a: Option<serde_json::Value> = None;
    let mut parsed_b: Option<serde_json::Value> = None;

    match serde_json::from_str::<serde_json::Value>(a) {
        Ok(v) => parsed_a = Some(v),
        Err(e) => {
            valid_json_a = false;
            let raw = strip_serde_json_position(&e.to_string());
            let mapped = map_json_error_to_python(a, &raw, 0);
            let line = e.line();
            let col = e.column();
            diffs.push(JsonCompareDiff {
                path: String::new(),
                kind: "parse_error_a".to_string(),
                a_type: None,
                b_type: None,
                a_preview: Some(format!("Line {}, Col {}: {}", line, col, mapped)),
                b_preview: None,
            });
        }
    }

    match serde_json::from_str::<serde_json::Value>(b) {
        Ok(v) => parsed_b = Some(v),
        Err(e) => {
            valid_json_b = false;
            let raw = strip_serde_json_position(&e.to_string());
            let mapped = map_json_error_to_python(b, &raw, 0);
            let line = e.line();
            let col = e.column();
            diffs.push(JsonCompareDiff {
                path: String::new(),
                kind: "parse_error_b".to_string(),
                a_type: None,
                b_type: None,
                a_preview: None,
                b_preview: Some(format!("Line {}, Col {}: {}", line, col, mapped)),
            });
        }
    }

    if !valid_json_a || !valid_json_b {
        let truncated = diffs.len() > max_diffs;
        return Ok(JsonCompareResult {
            valid_json_a,
            valid_json_b,
            equal: false,
            same_type: false,
            diff_count: diffs.len(),
            diffs: diffs.into_iter().take(max_diffs).collect(),
            truncated,
            summary: "One or both inputs are not valid JSON".to_string(),
        });
    }

    let parsed_a = parsed_a.unwrap();
    let parsed_b = parsed_b.unwrap();
    let mut same_type = true;

    fn normalize_key(key: &str, casefold: bool) -> String {
        if casefold {
            let mut result = String::with_capacity(key.len());
            for c in key.chars() {
                if c == '\u{00DF}' {
                    result.push_str("ss");
                } else {
                    result.extend(c.to_lowercase());
                }
            }
            result
        } else {
            key.to_string()
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn compare_values(
        path: &str,
        a_val: &serde_json::Value,
        b_val: &serde_json::Value,
        depth: usize,
        ignore_object_order: bool,
        ignore_array_order: bool,
        numeric_string_equivalence: bool,
        casefold_keys: bool,
        treat_missing_null_as_equal: bool,
        max_diffs: usize,
        diffs: &mut Vec<JsonCompareDiff>,
        same_type: &mut bool,
    ) {
        if depth > 100 || diffs.len() >= max_diffs {
            return;
        }

        if treat_missing_null_as_equal && (a_val.is_null() || b_val.is_null()) {
            return;
        }

        let a_type = get_json_type(a_val);
        let b_type = get_json_type(b_val);

        // numeric_string_equivalence across types
        if numeric_string_equivalence && a_type != b_type {
            let (str_val, num_val, str_type, num_type) =
                if a_type == "string" && (b_type == "integer" || b_type == "float") {
                    (a_val, b_val, &a_type, &b_type)
                } else if b_type == "string" && (a_type == "integer" || a_type == "float") {
                    (b_val, a_val, &b_type, &a_type)
                } else {
                    return;
                };
            if let serde_json::Value::String(s) = str_val {
                if let (Ok(num_a), Ok(num_b)) = (
                    s.parse::<f64>(),
                    match num_val {
                        serde_json::Value::Number(n) => n.as_f64().ok_or(()),
                        _ => Err(()),
                    },
                ) {
                    if (num_a - num_b).abs() < f64::EPSILON {
                        return;
                    }
                    *same_type = false;
                    diffs.push(JsonCompareDiff {
                        path: path.to_string(),
                        kind: "value_changed".to_string(),
                        a_type: Some(str_type.clone()),
                        b_type: Some(num_type.clone()),
                        a_preview: value_preview(a_val, 30),
                        b_preview: value_preview(b_val, 30),
                    });
                    return;
                }
            }
        }

        if a_type != b_type {
            if treat_missing_null_as_equal {
                let a_null = a_val.is_null();
                let b_null = b_val.is_null();
                if !(a_null || b_null) {
                    *same_type = false;
                    diffs.push(JsonCompareDiff {
                        path: path.to_string(),
                        kind: "type_changed".to_string(),
                        a_type: Some(a_type),
                        b_type: Some(b_type),
                        a_preview: value_preview(a_val, 30),
                        b_preview: value_preview(b_val, 30),
                    });
                }
            } else {
                *same_type = false;
                diffs.push(JsonCompareDiff {
                    path: path.to_string(),
                    kind: "type_changed".to_string(),
                    a_type: Some(a_type),
                    b_type: Some(b_type),
                    a_preview: value_preview(a_val, 30),
                    b_preview: value_preview(b_val, 30),
                });
            }
            return;
        }

        // numeric_string_equivalence for same-type strings
        if numeric_string_equivalence && a_type == "string" {
            if let (serde_json::Value::String(s_a), serde_json::Value::String(s_b)) = (a_val, b_val)
            {
                if let (Ok(num_a), Ok(num_b)) = (s_a.parse::<f64>(), s_b.parse::<f64>()) {
                    if (num_a - num_b).abs() < f64::EPSILON {
                        return;
                    }
                }
            }
        }

        match a_val {
            serde_json::Value::Object(a_map) => {
                let b_map = if let serde_json::Value::Object(m) = b_val {
                    m
                } else {
                    unreachable!()
                };

                let a_keys_set: std::collections::HashSet<String> = a_map
                    .keys()
                    .map(|k| normalize_key(k, casefold_keys))
                    .collect();
                let b_keys_set: std::collections::HashSet<String> = b_map
                    .keys()
                    .map(|k| normalize_key(k, casefold_keys))
                    .collect();

                // Map from normalized key to original key
                let keys_a: HashMap<String, String> = a_map
                    .keys()
                    .map(|k| (normalize_key(k, casefold_keys), k.clone()))
                    .collect();
                let keys_b: HashMap<String, String> = b_map
                    .keys()
                    .map(|k| (normalize_key(k, casefold_keys), k.clone()))
                    .collect();

                if !ignore_object_order {
                    let a_key_order: Vec<&String> = a_map.keys().collect();
                    let b_key_order: Vec<&String> = b_map.keys().collect();
                    let len_a = a_key_order.len();
                    let len_b = b_key_order.len();
                    let min_len = len_a.min(len_b);
                    for i in 0..min_len {
                        let a_key = a_key_order[i];
                        let b_key = b_key_order[i];
                        if normalize_key(a_key, casefold_keys)
                            != normalize_key(b_key, casefold_keys)
                        {
                            let new_path = if path.is_empty() {
                                format!("/{}", a_key)
                            } else {
                                format!("{}/{}", path, a_key)
                            };
                            diffs.push(JsonCompareDiff {
                                path: new_path,
                                kind: "key_missing_in_b".to_string(),
                                a_type: Some(get_json_type(a_map.get(a_key.as_str()).unwrap())),
                                b_type: None,
                                a_preview: value_preview(a_map.get(a_key.as_str()).unwrap(), 30),
                                b_preview: None,
                            });
                            break;
                        }
                        // Keys match — recurse into values
                        let new_path = if path.is_empty() {
                            format!("/{}", a_key)
                        } else {
                            format!("{}/{}", path, a_key)
                        };
                        compare_values(
                            &new_path,
                            a_map.get(a_key.as_str()).unwrap(),
                            b_map.get(b_key.as_str()).unwrap(),
                            depth + 1,
                            ignore_object_order,
                            ignore_array_order,
                            numeric_string_equivalence,
                            casefold_keys,
                            treat_missing_null_as_equal,
                            max_diffs,
                            diffs,
                            same_type,
                        );
                    }
                    if len_a != len_b {
                        *same_type = false;
                        diffs.push(JsonCompareDiff {
                            path: path.to_string(),
                            kind: "array_length_changed".to_string(),
                            a_type: Some("object".to_string()),
                            b_type: Some("object".to_string()),
                            a_preview: Some(format!("{} keys", len_a)),
                            b_preview: Some(format!("{} keys", len_b)),
                        });
                    }
                    return;
                }

                // ignore_object_order = true (default)
                for key in a_keys_set.difference(&b_keys_set).collect::<Vec<_>>() {
                    let orig_key = &keys_a[key];
                    if treat_missing_null_as_equal {
                        if let Some(val) = a_map.get(orig_key.as_str()) {
                            if !val.is_null() {
                                let new_path = if path.is_empty() {
                                    format!("/{}", orig_key)
                                } else {
                                    format!("{}/{}", path, orig_key)
                                };
                                diffs.push(JsonCompareDiff {
                                    path: new_path,
                                    kind: "key_missing_in_b".to_string(),
                                    a_type: Some(get_json_type(val)),
                                    b_type: None,
                                    a_preview: value_preview(val, 30),
                                    b_preview: None,
                                });
                            }
                        }
                    } else {
                        let val = a_map.get(orig_key.as_str()).unwrap();
                        let new_path = if path.is_empty() {
                            format!("/{}", orig_key)
                        } else {
                            format!("{}/{}", path, orig_key)
                        };
                        diffs.push(JsonCompareDiff {
                            path: new_path,
                            kind: "key_missing_in_b".to_string(),
                            a_type: Some(get_json_type(val)),
                            b_type: None,
                            a_preview: value_preview(val, 30),
                            b_preview: None,
                        });
                    }
                }

                for key in b_keys_set.difference(&a_keys_set).collect::<Vec<_>>() {
                    let orig_key = &keys_b[key];
                    if treat_missing_null_as_equal {
                        if let Some(val) = b_map.get(orig_key.as_str()) {
                            if !val.is_null() {
                                let new_path = if path.is_empty() {
                                    format!("/{}", orig_key)
                                } else {
                                    format!("{}/{}", path, orig_key)
                                };
                                diffs.push(JsonCompareDiff {
                                    path: new_path,
                                    kind: "key_missing_in_a".to_string(),
                                    a_type: None,
                                    b_type: Some(get_json_type(val)),
                                    a_preview: None,
                                    b_preview: value_preview(val, 30),
                                });
                            }
                        }
                    } else {
                        let val = b_map.get(orig_key.as_str()).unwrap();
                        let new_path = if path.is_empty() {
                            format!("/{}", orig_key)
                        } else {
                            format!("{}/{}", path, orig_key)
                        };
                        diffs.push(JsonCompareDiff {
                            path: new_path,
                            kind: "key_missing_in_a".to_string(),
                            a_type: None,
                            b_type: Some(get_json_type(val)),
                            a_preview: None,
                            b_preview: value_preview(val, 30),
                        });
                    }
                }

                let common_keys: Vec<&String> = a_keys_set.intersection(&b_keys_set).collect();
                for norm_key in &common_keys {
                    let orig_key_a = &keys_a[norm_key.as_str()];
                    let orig_key_b = &keys_b[norm_key.as_str()];
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
                    compare_values(
                        &new_path,
                        a_map.get(orig_key_a.as_str()).unwrap(),
                        b_map.get(orig_key_b.as_str()).unwrap(),
                        depth + 1,
                        ignore_object_order,
                        ignore_array_order,
                        numeric_string_equivalence,
                        casefold_keys,
                        treat_missing_null_as_equal,
                        max_diffs,
                        diffs,
                        same_type,
                    );
                }
            }
            serde_json::Value::Array(a_arr) => {
                let b_arr = if let serde_json::Value::Array(a) = b_val {
                    a
                } else {
                    unreachable!()
                };

                if a_arr.len() != b_arr.len() {
                    *same_type = false;
                    diffs.push(JsonCompareDiff {
                        path: path.to_string(),
                        kind: "array_length_changed".to_string(),
                        a_type: Some(a_type),
                        b_type: Some(b_type),
                        a_preview: Some(format!("{} items", a_arr.len())),
                        b_preview: Some(format!("{} items", b_arr.len())),
                    });
                    return;
                }

                if ignore_array_order && is_serializable(a_val) && is_serializable(b_val) {
                    let mut norm_a: Vec<serde_json::Value> =
                        a_arr.iter().map(canonicalize_for_compare).collect();
                    let mut norm_b: Vec<serde_json::Value> =
                        b_arr.iter().map(canonicalize_for_compare).collect();
                    // serde_json::Value doesn't implement Ord, sort by serialized form
                    norm_a.sort_by(|a, b| {
                        serde_json::to_string(a)
                            .unwrap_or_default()
                            .cmp(&serde_json::to_string(b).unwrap_or_default())
                    });
                    norm_b.sort_by(|a, b| {
                        serde_json::to_string(a)
                            .unwrap_or_default()
                            .cmp(&serde_json::to_string(b).unwrap_or_default())
                    });
                    if norm_a == norm_b {
                        return;
                    }
                    for i in 0..norm_a.len() {
                        compare_values(
                            &format!("{}/[{}]", path, i),
                            &norm_a[i],
                            &norm_b[i],
                            depth + 1,
                            ignore_object_order,
                            ignore_array_order,
                            numeric_string_equivalence,
                            casefold_keys,
                            treat_missing_null_as_equal,
                            max_diffs,
                            diffs,
                            same_type,
                        );
                    }
                } else {
                    for (i, (item_a, item_b)) in a_arr.iter().zip(b_arr.iter()).enumerate() {
                        compare_values(
                            &format!("{}/[{}]", path, i),
                            item_a,
                            item_b,
                            depth + 1,
                            ignore_object_order,
                            ignore_array_order,
                            numeric_string_equivalence,
                            casefold_keys,
                            treat_missing_null_as_equal,
                            max_diffs,
                            diffs,
                            same_type,
                        );
                    }
                }
            }
            _ => {
                if a_val != b_val {
                    *same_type = false;
                    diffs.push(JsonCompareDiff {
                        path: path.to_string(),
                        kind: "value_changed".to_string(),
                        a_type: Some(a_type),
                        b_type: Some(b_type),
                        a_preview: value_preview(a_val, 30),
                        b_preview: value_preview(b_val, 30),
                    });
                }
            }
        }
    }

    compare_values(
        "",
        &parsed_a,
        &parsed_b,
        0,
        ignore_object_order,
        ignore_array_order,
        numeric_string_equivalence,
        casefold_keys,
        treat_missing_null_as_equal,
        max_diffs,
        &mut diffs,
        &mut same_type,
    );

    let truncated = diffs.len() >= max_diffs;
    let diffs: Vec<JsonCompareDiff> = diffs.into_iter().take(max_diffs).collect();
    let equal = diffs.is_empty();

    let summary = if equal {
        "JSON documents are equal".to_string()
    } else {
        format!(
            "JSON documents differ at {} path{}",
            diffs.len(),
            if diffs.len() != 1 { "s" } else { "" }
        )
    };

    Ok(JsonCompareResult {
        valid_json_a: true,
        valid_json_b: true,
        equal,
        same_type,
        diff_count: diffs.len(),
        diffs,
        truncated,
        summary,
    })
}

// ── JSON canonicalize ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonCanonicalizeResult {
    pub valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minified: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    pub duplicate_keys: Vec<String>,
    #[serde(rename = "top_level_type", skip_serializing_if = "Option::is_none")]
    pub top_level_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_level_keys: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<i32>,
}

pub fn json_canonicalize(
    text: &str,
    sort_keys: bool,
    indent: Option<usize>,
    ensure_ascii: bool,
    detect_duplicate_keys: bool,
    trailing_newline: bool,
) -> Result<JsonCanonicalizeResult, String> {
    use sha2::{Digest, Sha256};

    if text.len() > MAX_INPUT_LENGTH {
        return Err(format!(
            "Input length {} exceeds MAX_INPUT_LENGTH {}",
            text.len(),
            MAX_INPUT_LENGTH
        ));
    }

    let mut duplicate_keys: Vec<String> = Vec::new();
    let parsed: serde_json::Value;

    if detect_duplicate_keys {
        match serde_json::from_str::<serde_json::Value>(text) {
            Ok(v) => {
                detect_duplicate_keys_raw(text, &mut duplicate_keys);
                parsed = v;
            }
            Err(e) => {
                let raw = strip_serde_json_position(&e.to_string());
                let mapped = map_json_error_to_python(text, &raw, 0);
                let line = e.line() as i32;
                let col = e.column() as i32;
                return Ok(JsonCanonicalizeResult {
                    valid: false,
                    canonical: None,
                    minified: None,
                    sha256: None,
                    duplicate_keys: vec![],
                    top_level_type: None,
                    top_level_keys: None,
                    error: Some(mapped),
                    line: Some(line),
                    column: Some(col),
                });
            }
        }
    } else {
        match serde_json::from_str::<serde_json::Value>(text) {
            Ok(v) => parsed = v,
            Err(e) => {
                let raw = strip_serde_json_position(&e.to_string());
                let mapped = map_json_error_to_python(text, &raw, 0);
                let line = e.line() as i32;
                let col = e.column() as i32;
                return Ok(JsonCanonicalizeResult {
                    valid: false,
                    canonical: None,
                    minified: None,
                    sha256: None,
                    duplicate_keys: vec![],
                    top_level_type: None,
                    top_level_keys: None,
                    error: Some(mapped),
                    line: Some(line),
                    column: Some(col),
                });
            }
        }
    }

    let (top_level_type, top_level_keys) = match &parsed {
        serde_json::Value::Object(m) => ("object".to_string(), Some(m.keys().cloned().collect())),
        serde_json::Value::Array(_) => ("array".to_string(), None),
        serde_json::Value::String(_) => ("string".to_string(), None),
        serde_json::Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                ("int".to_string(), None)
            } else {
                ("float".to_string(), None)
            }
        }
        serde_json::Value::Bool(_) => ("bool".to_string(), None),
        serde_json::Value::Null => ("NoneType".to_string(), None),
    };

    let canonical_data = if sort_keys {
        sort_json_keys(&parsed)
    } else {
        parsed.clone()
    };

    let canonical = {
        use serde::Serialize;
        if let Some(indent_val) = indent {
            let indent_str = " ".repeat(indent_val);
            let formatter = serde_json::ser::PrettyFormatter::with_indent(indent_str.as_bytes());
            let mut buf = Vec::new();
            {
                let mut serializer = serde_json::Serializer::with_formatter(&mut buf, formatter);
                canonical_data
                    .serialize(&mut serializer)
                    .unwrap_or_default();
            }
            String::from_utf8(buf).unwrap_or_default()
        } else {
            serde_json::to_string(&canonical_data).unwrap_or_default()
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
        canonical.clone()
    } else {
        serde_json::to_string(&canonical_data).unwrap_or_default()
    };
    let minified = if ensure_ascii {
        escape_ascii(&minified_raw)
    } else {
        minified_raw
    };

    let sha256_hash = {
        let mut hasher = Sha256::new();
        hasher.update(canonical.as_bytes());
        let result = hasher.finalize();
        hex_encode(&result)
    };

    Ok(JsonCanonicalizeResult {
        valid: true,
        canonical: Some(canonical),
        minified: Some(minified),
        sha256: Some(sha256_hash),
        duplicate_keys,
        top_level_type: Some(top_level_type),
        top_level_keys,
        error: None,
        line: None,
        column: None,
    })
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn escape_ascii(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii() {
            result.push(c);
        } else {
            for utf16 in c.encode_utf16(&mut [0u16; 2]) {
                result.push_str(&format!("\\u{:04x}", utf16));
            }
        }
    }
    result
}

fn detect_duplicate_keys_raw(text: &str, duplicates: &mut Vec<String>) {
    let trimmed = text.trim();
    if trimmed.is_empty() || (!trimmed.starts_with('{') && !trimmed.starts_with('[')) {
        return;
    }
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape = false;
    let mut key_start: Option<usize> = None;
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
                if let Some(ks) = key_start.take() {
                    let key = String::from_utf8_lossy(&bytes[ks..i]).to_string();
                    if depth > 0 && keys_at_depth.len() >= depth as usize {
                        let idx = (depth - 1) as usize;
                        if !keys_at_depth[idx].insert(key.clone()) {
                            duplicates.push(key);
                        }
                    }
                }
                in_string = false;
            }
            i += 1;
            continue;
        }
        match b {
            b'"' => {
                in_string = true;
                if depth > 0 && i + 1 < bytes.len() {
                    let mut j = i + 1;
                    while j < bytes.len() && bytes[j] != b'"' {
                        if bytes[j] == b'\\' {
                            j += 1;
                        }
                        j += 1;
                    }
                    let next_non_space = if j + 1 < bytes.len() {
                        bytes[j + 1..]
                            .iter()
                            .position(|&c| c != b' ' && c != b'\t' && c != b'\n' && c != b'\r')
                            .map(|p| j + 1 + p)
                    } else {
                        None
                    };
                    if let Some(nns) = next_non_space {
                        if bytes[nns] == b':' {
                            key_start = Some(i + 1);
                        }
                    }
                }
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

// ── Validate schema light ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaViolation {
    pub path: String,
    pub message: String,
    #[serde(rename = "value_type", skip_serializing_if = "Option::is_none")]
    pub value_type: Option<String>,
    #[serde(rename = "expected_type", skip_serializing_if = "Option::is_none")]
    pub expected_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateSchemaLightResult {
    pub valid: bool,
    pub violations: Vec<SchemaViolation>,
    pub truncated: bool,
    pub summary: String,
}

pub fn validate_schema_light(
    data: &serde_json::Value,
    schema: &serde_json::Value,
) -> Result<ValidateSchemaLightResult, String> {
    let schema_map = match schema {
        serde_json::Value::Object(m) => m,
        _ => {
            return Err("Schema must be a JSON object".to_string());
        }
    };

    let mut violations: Vec<SchemaViolation> = Vec::new();
    let mut walk_count: usize = 0;

    fn validate_recursive(
        path: &str,
        value: &serde_json::Value,
        schema_map: &serde_json::Map<String, serde_json::Value>,
        depth: usize,
        walk_count: &mut usize,
        violations: &mut Vec<SchemaViolation>,
    ) {
        *walk_count += 1;
        if *walk_count > MAX_SCHEMA_ELEMENTS {
            return;
        }
        if violations.len() >= MAX_SCHEMA_VIOLATIONS {
            return;
        }
        if depth > MAX_SCHEMA_DEPTH {
            violations.push(SchemaViolation {
                path: path.to_string(),
                message: format!(
                    "schema nesting depth {} exceeds maximum {}",
                    depth, MAX_SCHEMA_DEPTH
                ),
                value_type: Some(get_schema_type_name(value)),
                expected_type: None,
            });
            return;
        }

        let expected_type = schema_map.get("type").and_then(|v| v.as_str());

        if let Some(exp) = expected_type {
            let actual_type = get_schema_type_name(value);
            let type_ok = match exp {
                "object" => actual_type == "object",
                "array" => actual_type == "array",
                "string" => actual_type == "string",
                "number" => actual_type == "number" || actual_type == "integer",
                "integer" => actual_type == "integer",
                "boolean" => actual_type == "boolean",
                "null" => actual_type == "null",
                _ => true,
            };
            if !type_ok {
                violations.push(SchemaViolation {
                    path: path.to_string(),
                    message: format!("expected {}, got {}", exp, actual_type),
                    value_type: Some(actual_type),
                    expected_type: Some(exp.to_string()),
                });
                return;
            }
        }

        // Object validation
        if expected_type == Some("object") {
            if let serde_json::Value::Object(map) = value {
                // required
                if let Some(serde_json::Value::Array(req)) = schema_map.get("required") {
                    for key in req {
                        if let serde_json::Value::String(key_str) = key {
                            if !map.contains_key(key_str.as_str()) {
                                let p = if path.is_empty() {
                                    format!("/{}", key_str)
                                } else {
                                    format!("{}/{}", path, key_str)
                                };
                                violations.push(SchemaViolation {
                                    path: p,
                                    message: format!("missing required key '{}'", key_str),
                                    value_type: None,
                                    expected_type: Some("object".to_string()),
                                });
                            }
                        }
                    }
                }

                // additional_properties
                if let Some(serde_json::Value::Bool(false)) =
                    schema_map.get("additional_properties")
                {
                    let props = schema_map
                        .get("properties")
                        .and_then(|v| v.as_object())
                        .cloned()
                        .unwrap_or_default();
                    for key in map.keys() {
                        if !props.contains_key(key.as_str()) {
                            let p = if path.is_empty() {
                                format!("/{}", key)
                            } else {
                                format!("{}/{}", path, key)
                            };
                            violations.push(SchemaViolation {
                                path: p,
                                message: format!("additional property '{}' not allowed", key),
                                value_type: Some("string".to_string()),
                                expected_type: None,
                            });
                        }
                    }
                }

                // properties
                if let Some(serde_json::Value::Object(props)) = schema_map.get("properties") {
                    for (prop_name, prop_schema) in props {
                        if let Some(val) = map.get(prop_name.as_str()) {
                            let new_path = if path.is_empty() {
                                format!("/{}", prop_name)
                            } else {
                                format!("{}/{}", path, prop_name)
                            };
                            if let serde_json::Value::Object(prop_map) = prop_schema {
                                validate_recursive(
                                    &new_path,
                                    val,
                                    prop_map,
                                    depth + 1,
                                    walk_count,
                                    violations,
                                );
                            }
                        }
                    }
                }
            }
        }

        // Array validation
        if expected_type == Some("array") {
            if let serde_json::Value::Array(arr) = value {
                if let Some(serde_json::Value::Number(min)) = schema_map.get("min_items") {
                    if let Some(min_val) = min.as_u64() {
                        if arr.len() < min_val as usize {
                            violations.push(SchemaViolation {
                                path: path.to_string(),
                                message: format!(
                                    "array has {} items, minimum is {}",
                                    arr.len(),
                                    min_val
                                ),
                                value_type: Some("array".to_string()),
                                expected_type: None,
                            });
                        }
                    }
                }
                if let Some(serde_json::Value::Number(max)) = schema_map.get("max_items") {
                    if let Some(max_val) = max.as_u64() {
                        if arr.len() > max_val as usize {
                            violations.push(SchemaViolation {
                                path: path.to_string(),
                                message: format!(
                                    "array has {} items, maximum is {}",
                                    arr.len(),
                                    max_val
                                ),
                                value_type: Some("array".to_string()),
                                expected_type: None,
                            });
                        }
                    }
                }
                if let Some(serde_json::Value::Object(items_schema)) = schema_map.get("items") {
                    for (i, item) in arr.iter().enumerate() {
                        let item_path = format!("{}/[{}]", path, i);
                        validate_recursive(
                            &item_path,
                            item,
                            items_schema,
                            depth + 1,
                            walk_count,
                            violations,
                        );
                    }
                }
            }
        }

        // String validation
        if expected_type == Some("string") {
            if let serde_json::Value::String(s) = value {
                if let Some(serde_json::Value::Number(min)) = schema_map.get("min_length") {
                    if let Some(min_val) = min.as_u64() {
                        let char_len = s.chars().count();
                        if char_len < min_val as usize {
                            violations.push(SchemaViolation {
                                path: path.to_string(),
                                message: format!(
                                    "string has length {}, minimum is {}",
                                    char_len, min_val
                                ),
                                value_type: Some("string".to_string()),
                                expected_type: None,
                            });
                        }
                    }
                }
                if let Some(serde_json::Value::Number(max)) = schema_map.get("max_length") {
                    if let Some(max_val) = max.as_u64() {
                        let char_len = s.chars().count();
                        if char_len > max_val as usize {
                            violations.push(SchemaViolation {
                                path: path.to_string(),
                                message: format!(
                                    "string has length {}, maximum is {}",
                                    char_len, max_val
                                ),
                                value_type: Some("string".to_string()),
                                expected_type: None,
                            });
                        }
                    }
                }
                if let Some(serde_json::Value::String(pattern)) = schema_map.get("pattern") {
                    if let Ok(re) = Regex::new(pattern) {
                        if !re.is_match(s).unwrap_or(false) {
                            violations.push(SchemaViolation {
                                path: path.to_string(),
                                message: format!(
                                    "string '{}' does not match pattern '{}'",
                                    s, pattern
                                ),
                                value_type: Some("string".to_string()),
                                expected_type: None,
                            });
                        }
                    }
                }
            }
        }

        // Enum validation (any type)
        if let Some(serde_json::Value::Array(enum_vals)) = schema_map.get("enum") {
            if !enum_vals.contains(value) {
                violations.push(SchemaViolation {
                    path: path.to_string(),
                    message: format!("value {:?} is not in enum {:?}", value, enum_vals),
                    value_type: Some(get_schema_type_name(value)),
                    expected_type: None,
                });
            }
        }
    }

    validate_recursive("", data, schema_map, 0, &mut walk_count, &mut violations);

    let truncated = violations.len() >= MAX_SCHEMA_VIOLATIONS || walk_count > MAX_SCHEMA_ELEMENTS;

    let summary = if violations.is_empty() {
        if truncated {
            format!(
                "Validation truncated after {} elements (limit {})",
                walk_count, MAX_SCHEMA_ELEMENTS
            )
        } else {
            "Data is valid".to_string()
        }
    } else if truncated {
        format!(
            "Schema violations detected (truncated, {} shown)",
            violations.len()
        )
    } else {
        format!(
            "Schema violations detected: {} issue{}",
            violations.len(),
            if violations.len() != 1 { "s" } else { "" }
        )
    };

    Ok(ValidateSchemaLightResult {
        valid: violations.is_empty(),
        violations,
        truncated,
        summary,
    })
}

// ── List utilities ─────────────────────────────────────────────────────

fn normalize_for_list(s: &str, normalization: &str, casefold: bool) -> String {
    use unicode_normalization::UnicodeNormalization;
    let mut result = s.to_string();
    if casefold {
        result = caseless::default_case_fold_str(&result);
    }
    if normalization != "raw" {
        result = match normalization {
            "NFC" => result.nfc().collect(),
            "NFD" => result.nfd().collect(),
            "NFKC" => result.nfkc().collect(),
            "NFKD" => result.nfkd().collect(),
            _ => result,
        };
    }
    result
}

pub fn list_dedupe(
    items: &[String],
    normalization: &str,
    casefold: bool,
    _stable: bool,
) -> Result<Vec<String>, String> {
    if items.len() > MAX_LIST_ITEMS {
        return Err(format!(
            "Items count {} exceeds maximum {}",
            items.len(),
            MAX_LIST_ITEMS
        ));
    }
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for item in items {
        let compare_val = normalize_for_list(item, normalization, casefold);
        if seen.insert(compare_val) {
            result.push(item.clone());
        }
    }
    Ok(result)
}

pub fn list_sort(
    items: &[String],
    normalization: &str,
    casefold: bool,
    reverse: bool,
    _stable: bool,
) -> Result<Vec<String>, String> {
    if items.len() > MAX_LIST_ITEMS {
        return Err(format!(
            "Items count {} exceeds maximum {}",
            items.len(),
            MAX_LIST_ITEMS
        ));
    }
    let mut indexed: Vec<(String, &String)> = items
        .iter()
        .map(|item| (normalize_for_list(item, normalization, casefold), item))
        .collect();
    indexed.sort_by(|a, b| a.0.cmp(&b.0));
    let mut result: Vec<String> = indexed.into_iter().map(|(_, item)| item.clone()).collect();
    if reverse {
        result.reverse();
    }
    Ok(result)
}
