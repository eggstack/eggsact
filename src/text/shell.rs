use regex::Regex;
use std::sync::LazyLock;

#[derive(Clone, Copy, PartialEq)]
enum QuoteState {
    None,
    Single,
    Double,
}

#[derive(Default)]
pub struct ShellFeatures {
    pub has_pipe: bool,
    pub has_redirection: bool,
    pub has_command_substitution: bool,
    pub has_variable_expansion: bool,
    pub has_glob_pattern: bool,
    pub has_control_operator: bool,
    pub has_background: bool,
    pub has_unbalanced_quotes: bool,
}

pub struct ShellSplitResult {
    pub parse_ok: bool,
    pub argv: Vec<String>,
    pub argc: usize,
    pub features: ShellFeatures,
    pub findings: Vec<String>,
}

pub struct ShellQuoteJoinResult {
    pub command: String,
    pub roundtrip_ok: bool,
    pub findings: Vec<String>,
}

pub struct ArgvCompareResult {
    pub argv_equal: bool,
    pub left_argv: Vec<String>,
    pub right_argv: Vec<String>,
    pub first_difference: Option<usize>,
    pub findings: Vec<String>,
}

fn is_glob_char(c: char) -> bool {
    c == '*' || c == '?' || c == '['
}

fn detect_features(argv: &[String], raw: &str, unbalanced: bool) -> ShellFeatures {
    let joined = argv.join(" ");

    let has_pipe = joined.contains('|');
    let has_redirection = joined.contains('<') || joined.contains('>');
    let has_command_substitution = COMMAND_SUB_PATTERN.is_match(raw);
    // Strip single-quoted sections before checking for variable expansion
    // Single quotes prevent variable expansion in POSIX shells
    let raw_without_single_quotes: String = raw
        .chars()
        .scan(false, |in_single, c| {
            if c == '\'' {
                *in_single = !*in_single;
            }
            if *in_single {
                Some(' ') // replace single-quoted chars with space
            } else {
                Some(c)
            }
        })
        .collect();
    let has_variable_expansion = VARIABLE_PATTERN.is_match(&raw_without_single_quotes);
    let has_glob_pattern = argv.iter().any(|t| t.chars().any(is_glob_char));

    let mut has_control_operator = false;
    for op in &["&&", "||"] {
        if joined.contains(op) {
            has_control_operator = true;
            break;
        }
    }
    if !has_control_operator && (joined.contains(';') || joined.contains('&')) {
        has_control_operator = true;
    }

    // Background execution: standalone '&' at end of command or between commands
    // (not '&&')
    let has_background = detect_background(raw);

    ShellFeatures {
        has_pipe,
        has_redirection,
        has_command_substitution,
        has_variable_expansion,
        has_glob_pattern,
        has_control_operator,
        has_background,
        has_unbalanced_quotes: unbalanced,
    }
}

static VARIABLE_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$\{[^}]*\}|\$[A-Za-z_][A-Za-z0-9_]*").unwrap());

static COMMAND_SUB_PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\$\(|`").unwrap());

/// Detect background execution (`cmd &` or `cmd1 && cmd2 &`).
///
/// Background execution is a standalone `&` at the end of a pipeline or
/// command, NOT `&&` (logical AND). Matches a non-`&` char followed by
/// `&` not followed by another `&`, then optional whitespace to end.
static BACKGROUND_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[^&]&[^&]\s*$|[^&]&\s*$").unwrap());

fn detect_background(raw: &str) -> bool {
    // Check each line independently (for multi-line commands)
    for line in raw.lines() {
        let trimmed = line.trim_end();
        if BACKGROUND_PATTERN.is_match(trimmed) {
            return true;
        }
        // Also handle single-char `&` at start of line
        if trimmed == "&" {
            return true;
        }
    }
    false
}

pub fn shell_split(command: &str, shell: &str, detect_risky_features: bool) -> ShellSplitResult {
    let mut findings = Vec::new();

    if shell != "posix" {
        return ShellSplitResult {
            parse_ok: false,
            argv: vec![],
            argc: 0,
            features: ShellFeatures::default(),
            findings: vec![format!(
                "Unsupported shell: {:?}. Only 'posix' is supported.",
                shell
            )],
        };
    }

    if command.trim().is_empty() {
        return ShellSplitResult {
            parse_ok: true,
            argv: vec![],
            argc: 0,
            features: ShellFeatures::default(),
            findings: vec!["Empty command".to_string()],
        };
    }

    let (argv, unbalanced, parse_error) = posix_shell_split(command);

    if let Some(err) = &parse_error {
        findings.push(format!("Parse error: {}", err));
    }

    let features = if detect_risky_features {
        detect_features(&argv, command, unbalanced)
    } else {
        ShellFeatures::default()
    };

    if detect_risky_features && !unbalanced {
        if features.has_pipe {
            findings.push("Contains pipe operator (|)".to_string());
        }
        if features.has_redirection {
            findings.push("Contains redirection operator (< or >)".to_string());
        }
        if features.has_command_substitution {
            findings.push("Contains command substitution ($( ) or backticks)".to_string());
        }
        if features.has_variable_expansion {
            findings.push("Contains variable expansion ($VAR or ${VAR})".to_string());
        }
        if features.has_glob_pattern {
            findings.push("Contains glob pattern characters (* ? [)".to_string());
        }
        if features.has_control_operator {
            findings.push("Contains control operator (; & && ||)".to_string());
        }
        if features.has_background {
            findings.push("Contains background execution operator (&)".to_string());
        }
    }

    ShellSplitResult {
        parse_ok: parse_error.is_none(),
        argv: argv.clone(),
        argc: argv.len(),
        features,
        findings,
    }
}

fn posix_shell_split(command: &str) -> (Vec<String>, bool, Option<String>) {
    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_quote: QuoteState = QuoteState::None;
    let mut token_started = false;

    let chars: Vec<char> = command.chars().collect();
    let len = chars.len();
    let mut i = 0;

    let flush_current =
        |tokens: &mut Vec<String>, current: &mut String, token_started: &mut bool| {
            if *token_started {
                tokens.push(std::mem::take(current));
                *token_started = false;
            }
        };

    while i < len {
        let c = chars[i];

        match in_quote {
            QuoteState::None => {
                if c.is_whitespace() {
                    flush_current(&mut tokens, &mut current, &mut token_started);
                    i += 1;
                    continue;
                }

                if c == '#' && !token_started {
                    break;
                }

                if c == '\'' {
                    in_quote = QuoteState::Single;
                    token_started = true;
                } else if c == '"' {
                    in_quote = QuoteState::Double;
                    token_started = true;
                } else if c == '\\' {
                    if i + 1 < len {
                        current.push(chars[i + 1]);
                        token_started = true;
                        i += 2;
                        continue;
                    } else {
                        return (tokens, true, Some("No escaped character".to_string()));
                    }
                } else {
                    current.push(c);
                    token_started = true;
                }
            }
            QuoteState::Single => {
                if c == '\'' {
                    in_quote = QuoteState::None;
                } else {
                    current.push(c);
                }
            }
            QuoteState::Double => {
                if c == '"' {
                    in_quote = QuoteState::None;
                } else if c == '\\' {
                    if i + 1 < len {
                        let next = chars[i + 1];
                        if next == '"' || next == '\\' {
                            current.push(next);
                            i += 2;
                            continue;
                        } else if next == '$' || next == '`' {
                            current.push('\\');
                            current.push(next);
                            token_started = true;
                            i += 2;
                            continue;
                        } else if next == '\n' {
                            token_started = true;
                            i += 2;
                            continue;
                        } else {
                            current.push('\\');
                            current.push(next);
                            token_started = true;
                            i += 2;
                            continue;
                        }
                    }
                    return (tokens, true, Some("No escaped character".to_string()));
                } else {
                    current.push(c);
                    token_started = true;
                }
            }
        }

        i += 1;
    }

    if in_quote != QuoteState::None {
        return (tokens, true, Some("No closing quotation".to_string()));
    }

    if token_started {
        tokens.push(current);
    }

    (tokens, false, None)
}

fn shell_quote(token: &str) -> String {
    if token.is_empty() {
        return "''".to_string();
    }

    let needs_quote = token.chars().any(|c| {
        c.is_whitespace()
            || c == '\''
            || c == '"'
            || c == '\\'
            || c == '$'
            || c == '`'
            || c == '|'
            || c == '<'
            || c == '>'
            || c == ';'
            || c == '&'
    });

    if !needs_quote {
        return token.to_string();
    }

    let mut result = String::new();
    result.push('\'');
    for c in token.chars() {
        if c == '\'' {
            result.push_str("'\"'\"'");
        } else {
            result.push(c);
        }
    }
    result.push('\'');
    result
}

pub fn shell_quote_join(argv: &[String], shell: &str) -> ShellQuoteJoinResult {
    let mut findings = Vec::new();

    if shell != "posix" {
        return ShellQuoteJoinResult {
            command: String::new(),
            roundtrip_ok: false,
            findings: vec![format!(
                "Unsupported shell: {:?}. Only 'posix' is supported.",
                shell
            )],
        };
    }

    let parts: Vec<String> = argv.iter().map(|t| shell_quote(t)).collect();
    let command = parts.join(" ");

    let result = shell_split(&command, "posix", false);
    let roundtrip_ok = if result.parse_ok && result.argv == argv {
        true
    } else {
        if result.parse_ok {
            findings.push(format!(
                "Round-trip mismatch: expected {:?}, got {:?}",
                argv, result.argv
            ));
        } else {
            findings.push("Round-trip parse failed".to_string());
        }
        false
    };

    ShellQuoteJoinResult {
        command,
        roundtrip_ok,
        findings,
    }
}

pub fn argv_compare(
    left_command: Option<&str>,
    right_command: Option<&str>,
    left_argv: Option<&[String]>,
    right_argv: Option<&[String]>,
    shell: &str,
) -> ArgvCompareResult {
    let mut findings = Vec::new();

    let mut resolved_left: Option<Vec<String>> = None;
    let mut resolved_right: Option<Vec<String>> = None;

    if let Some(cmd) = left_command {
        let split = shell_split(cmd, shell, false);
        if !split.parse_ok {
            return ArgvCompareResult {
                argv_equal: false,
                left_argv: vec![],
                right_argv: right_argv.map(|v| v.to_vec()).unwrap_or_default(),
                first_difference: Some(0),
                findings: vec![format!(
                    "Failed to parse left command: {:?}",
                    split.findings
                )],
            };
        }
        resolved_left = Some(split.argv.clone());
        if let Some(lv) = left_argv {
            if split.argv != lv {
                findings.push("Left command parse differs from provided left_argv".to_string());
            }
        }
    } else if let Some(lv) = left_argv {
        resolved_left = Some(lv.to_vec());
    }

    if let Some(cmd) = right_command {
        let split = shell_split(cmd, shell, false);
        if !split.parse_ok {
            return ArgvCompareResult {
                argv_equal: false,
                left_argv: resolved_left.clone().unwrap_or_default(),
                right_argv: vec![],
                first_difference: Some(0),
                findings: vec![format!(
                    "Failed to parse right command: {:?}",
                    split.findings
                )],
            };
        }
        resolved_right = Some(split.argv.clone());
        if let Some(rv) = right_argv {
            if split.argv != rv {
                findings.push("Right command parse differs from provided right_argv".to_string());
            }
        }
    } else if let Some(rv) = right_argv {
        resolved_right = Some(rv.to_vec());
    }

    let left = resolved_left.unwrap_or_default();
    let right = resolved_right.unwrap_or_default();

    let argv_equal = left == right;
    let mut first_diff: Option<usize> = None;

    if !argv_equal {
        for i in 0..std::cmp::min(left.len(), right.len()) {
            if left[i] != right[i] {
                first_diff = Some(i);
                findings.push(format!(
                    "First difference at index {}: '{}' != '{}'",
                    i, left[i], right[i]
                ));
                break;
            }
        }

        if first_diff.is_none() {
            first_diff = Some(std::cmp::min(left.len(), right.len()));
            if left.len() > right.len() {
                findings.push(format!(
                    "Left has {} extra tokens",
                    left.len() - right.len()
                ));
            } else {
                findings.push(format!(
                    "Right has {} extra tokens",
                    right.len() - left.len()
                ));
            }
        }
    }

    ArgvCompareResult {
        argv_equal,
        left_argv: left,
        right_argv: right,
        first_difference: first_diff,
        findings,
    }
}
