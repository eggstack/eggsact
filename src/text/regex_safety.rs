use fancy_regex::Regex;
use serde::{Deserialize, Serialize};

const MAX_PATTERN_LENGTH: usize = 1000;
const MAX_PATTERN_NESTING: usize = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegexSafetyFinding {
    pub kind: String,
    pub span: Vec<i32>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegexSafetyResult {
    pub valid_pattern: bool,
    pub risk: String,
    pub findings: Vec<RegexSafetyFinding>,
}

fn check_pattern_complexity(pattern: &str) -> Result<(), String> {
    let char_count = pattern.chars().count();
    if char_count > MAX_PATTERN_LENGTH {
        return Err(format!(
            "Pattern length {} exceeds maximum {}",
            char_count, MAX_PATTERN_LENGTH
        ));
    }

    let pat_chars: Vec<char> = pattern.chars().collect();
    let pat_len = pat_chars.len();
    let mut nesting_depth: usize = 0;
    let mut max_nesting: usize = 0;
    let mut in_char_class = false;
    let mut group_stack: Vec<bool> = Vec::new();
    let mut prev_group_had_quantifier = false;
    let mut i = 0;

    while i < pat_len {
        let c = pat_chars[i];

        if c == '\\' && i + 1 < pat_len {
            prev_group_had_quantifier = false;
            i += 2;
            continue;
        }

        if c == '[' {
            nesting_depth += 1;
            max_nesting = max_nesting.max(nesting_depth);
            in_char_class = true;
        } else if c == ']' {
            if nesting_depth > 0 {
                nesting_depth -= 1;
            }
            in_char_class = false;
        } else if c == '(' && !in_char_class {
            nesting_depth += 1;
            max_nesting = max_nesting.max(nesting_depth);
            group_stack.push(false);
            prev_group_had_quantifier = false;
        } else if c == ')' && !in_char_class {
            if nesting_depth == 0 {
                return Err(format!("Unmatched closing ')' at position {}", i));
            }
            nesting_depth -= 1;
            if let Some(inner_had_quantifier) = group_stack.pop() {
                if let Some(parent_flag) = group_stack.last_mut() {
                    *parent_flag = *parent_flag || inner_had_quantifier;
                }
                prev_group_had_quantifier = inner_had_quantifier;
            } else {
                prev_group_had_quantifier = false;
            }
        } else if (c == '+' || c == '*' || c == '?') && !in_char_class {
            if c == '?' && i > 0 && pat_chars[i - 1] == '(' {
                prev_group_had_quantifier = false;
            } else {
                if i > 0
                    && (pat_chars[i - 1] == '+'
                        || pat_chars[i - 1] == '*'
                        || pat_chars[i - 1] == '?')
                {
                    return Err(format!("Adjacent quantifiers detected at position {}", i));
                }
                if prev_group_had_quantifier {
                    return Err(format!(
                        "Nested quantifiers detected at position {}: \
                         quantifier after group with internal quantifier",
                        i
                    ));
                }
                if let Some(top) = group_stack.last_mut() {
                    *top = true;
                }
                prev_group_had_quantifier = false;
            }
        } else if c == '{' && !in_char_class {
            let j = i + 1;
            if j < pat_len && pat_chars[j].is_ascii_digit() {
                let mut k = j;
                while k < pat_len && pat_chars[k].is_ascii_digit() {
                    k += 1;
                }
                if k < pat_len && pat_chars[k] == ',' {
                    k += 1;
                    while k < pat_len && pat_chars[k].is_ascii_digit() {
                        k += 1;
                    }
                    if k < pat_len && pat_chars[k] == '}' {
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
                        i = k;
                    }
                } else if k < pat_len && pat_chars[k] == '}' {
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
                    i = k;
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

    if max_nesting > MAX_PATTERN_NESTING {
        return Err(format!(
            "Pattern nesting depth {} exceeds maximum {}",
            max_nesting, MAX_PATTERN_NESTING
        ));
    }

    Ok(())
}

pub fn regex_safety_check(pattern: &str) -> RegexSafetyResult {
    let mut findings: Vec<RegexSafetyFinding> = Vec::new();

    match Regex::new(pattern) {
        Ok(_) => {}
        Err(_) => {
            return RegexSafetyResult {
                valid_pattern: false,
                risk: "low".to_string(),
                findings: vec![],
            };
        }
    }

    if let Err(msg) = check_pattern_complexity(pattern) {
        findings.push(RegexSafetyFinding {
            kind: "complexity".to_string(),
            span: vec![0, pattern.chars().count() as i32],
            message: msg,
        });
        return RegexSafetyResult {
            valid_pattern: true,
            risk: "high".to_string(),
            findings,
        };
    }

    let pat_chars: Vec<char> = pattern.chars().collect();
    let pat_len = pat_chars.len();
    let mut i = 0;
    let mut paren_depth: usize = 0;
    let mut has_inner_quantifier = false;
    let mut last_paren_end: i32 = -1;

    while i < pat_len {
        let c = pat_chars[i];

        if c == '\\' && i + 1 < pat_len {
            i += 2;
            continue;
        }

        if c == '[' {
            i += 1;
            while i < pat_len {
                let cc = pat_chars[i];
                if cc == '\\' && i + 1 < pat_len {
                    i += 2;
                    continue;
                }
                if cc == ']' {
                    break;
                }
                i += 1;
            }
            i += 1;
            continue;
        }

        if c == '(' {
            paren_depth += 1;
            has_inner_quantifier = false;
            i += 1;
            continue;
        }

        if c == ')' {
            last_paren_end = i as i32;
            if paren_depth > 0 {
                paren_depth -= 1;
            }
            i += 1;
            continue;
        }

        let mut j;

        if c == '+' || c == '*' {
            j = i + 1;
            while j < pat_len && pattern.chars().nth(j) == Some(c) {
                j += 1;
            }
            if j < pat_len && pattern.chars().nth(j) == Some('?') {
                j += 1;
            }

            if paren_depth > 0 {
                if has_inner_quantifier {
                    findings.push(RegexSafetyFinding {
                        kind: "nested_quantifier".to_string(),
                        span: vec![i as i32, j as i32],
                        message: "Nested quantifiers may cause catastrophic backtracking"
                            .to_string(),
                    });
                }
                has_inner_quantifier = true;
            } else if paren_depth == 0 && last_paren_end > 0 {
                if has_inner_quantifier {
                    findings.push(RegexSafetyFinding {
                        kind: "nested_quantifier".to_string(),
                        span: vec![i as i32, j as i32],
                        message: "Quantifier after group with quantifier may cause catastrophic backtracking".to_string(),
                    });
                }
            }

            i = j;
            continue;
        }

        if c == '{' {
            j = i + 1;
            while j < pat_len && pattern.chars().nth(j) != Some('}') {
                j += 1;
            }
            if j < pat_len {
                j += 1;
            }

            if paren_depth > 0 {
                if has_inner_quantifier {
                    findings.push(RegexSafetyFinding {
                        kind: "nested_quantifier".to_string(),
                        span: vec![i as i32, j as i32],
                        message: "Nested quantifiers may cause catastrophic backtracking"
                            .to_string(),
                    });
                }
                has_inner_quantifier = true;
            }

            i = j;
            continue;
        }

        i += 1;
    }

    let backref_re = Regex::new(r"\\([1-9])|\\g<").unwrap();
    if let Ok(Some(_)) = backref_re.find(pattern) {
        findings.push(RegexSafetyFinding {
            kind: "backreference".to_string(),
            span: vec![0, pattern.chars().count() as i32],
            message: "Backreferences can cause exponential matching in some cases".to_string(),
        });
    }

    let dot_star_re = Regex::new(r"\.\*").unwrap();
    for cap in dot_star_re.find_iter(pattern) {
        let cap = cap.expect("find_iter should always return a match");
        let byte_range = cap.range();
        // Convert byte offsets to codepoint indices
        let cp_start = pattern[..byte_range.start].chars().count() as i32;
        let cp_end = pattern[..byte_range.end].chars().count() as i32;
        findings.push(RegexSafetyFinding {
            kind: "ambiguous_dot_star".to_string(),
            span: vec![cp_start, cp_end],
            message: "Ambiguous dot-star pattern".to_string(),
        });
    }

    let risk = if findings.iter().any(|f| f.kind == "nested_quantifier") {
        "high".to_string()
    } else if !findings.is_empty() {
        "medium".to_string()
    } else {
        "low".to_string()
    };

    RegexSafetyResult {
        valid_pattern: true,
        risk,
        findings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regex_safety_check_valid_pattern() {
        let result = regex_safety_check(r"\d+");
        assert!(result.valid_pattern);
        assert_eq!(result.risk, "low");
        assert!(result.findings.is_empty());
    }

    #[test]
    fn test_regex_safety_check_invalid_pattern() {
        let result = regex_safety_check(r"[");
        assert!(!result.valid_pattern);
        assert_eq!(result.risk, "low");
    }

    #[test]
    fn test_regex_safety_check_nested_quantifiers() {
        let result = regex_safety_check(r"(a+)+");
        assert!(result.valid_pattern);
        assert_eq!(result.risk, "high");
        assert!(!result.findings.is_empty());
        assert_eq!(result.findings[0].kind, "complexity");
    }

    #[test]
    fn test_regex_safety_check_backreference() {
        let result = regex_safety_check(r"(\w+)\1");
        assert!(result.valid_pattern);
        assert!(!result.findings.is_empty());
        assert_eq!(result.findings[0].kind, "backreference");
    }

    #[test]
    fn test_regex_safety_check_ambiguous_dot_star() {
        let result = regex_safety_check(r".*");
        assert!(result.valid_pattern);
        assert!(!result.findings.is_empty());
        assert_eq!(result.findings[0].kind, "ambiguous_dot_star");
    }

    #[test]
    fn test_regex_safety_check_complexity() {
        let pattern = r"(a".repeat(10);
        let result = regex_safety_check(&pattern);
        assert!(!result.valid_pattern);
    }
}
