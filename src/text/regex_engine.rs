use serde::{Deserialize, Serialize};

/// Which backend compiled the pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RegexEngineUsed {
    RustRegex,
    FancyRegex,
}

/// A regex feature detected during classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegexFeature {
    LookAhead,
    LookBehind,
    Backreference,
    NamedCapture,
    InlineFlags,
    UnsupportedPcreConstruct(String),
}

/// Result of classifying a regex pattern for backend routing.
#[derive(Debug, Clone)]
pub struct RegexClassification {
    pub preferred_engine: RegexEngineUsed,
    pub features: Vec<RegexFeature>,
    pub unsupported_features: Vec<String>,
}

/// Classify a regex pattern to determine which backend should compile it.
///
/// This is a conservative scanner: it identifies constructs that require
/// `fancy-regex` or are known unsupported. It handles escapes and character
/// classes correctly enough to avoid false positives on lookaround-like text
/// inside literals.
pub fn classify_pattern(pattern: &str) -> RegexClassification {
    let chars: Vec<char> = pattern.chars().collect();
    let len = chars.len();
    let mut features = Vec::new();
    let mut unsupported = Vec::new();
    let mut needs_fancy = false;
    let mut in_char_class = false;

    let mut i = 0;
    while i < len {
        let c = chars[i];

        // Skip escaped characters — the next char is literal
        if c == '\\' && i + 1 < len {
            let next = chars[i + 1];
            // Check for backreferences: \1-\9
            if next.is_ascii_digit() && next != '0' {
                features.push(RegexFeature::Backreference);
                needs_fancy = true;
            }
            // Detect \K (reset match start) — PCRE-only
            else if next == 'K' {
                let desc = "backslash_K".to_string();
                unsupported.push(desc);
                features.push(RegexFeature::UnsupportedPcreConstruct(
                    "backslash_K".to_string(),
                ));
            }
            i += 2;
            continue;
        }

        // Track character classes — contents are literal
        if c == '[' && !in_char_class {
            in_char_class = true;
            i += 1;
            continue;
        }
        if c == ']' && in_char_class {
            in_char_class = false;
            i += 1;
            continue;
        }
        if in_char_class {
            i += 1;
            continue;
        }

        // Detect group-opening constructs: (?...)
        if c == '(' && i + 1 < len && chars[i + 1] == '?' {
            i += 2; // skip (?
            if i >= len {
                break;
            }
            let group_start = i - 2;
            match chars[i] {
                // Lookahead: (?=...) or (?!...)
                '=' | '!' => {
                    features.push(RegexFeature::LookAhead);
                    needs_fancy = true;
                }
                // Lookbehind: (?<=...) or (?<!...)  — but NOT named group (?<name>...)
                '<' => {
                    i += 1;
                    if i < len && (chars[i] == '=' || chars[i] == '!') {
                        features.push(RegexFeature::LookBehind);
                        needs_fancy = true;
                    }
                    // If neither = nor !, it's a named group — no special feature needed
                }
                // Inline flags: (?i), (?m), (?s), (?x), (?imsx:...)
                'i' | 'm' | 's' | 'x' => {
                    features.push(RegexFeature::InlineFlags);
                    // Inline flags alone don't force fancy-regex
                }
                // Branch reset: (?|...) — unsupported PCRE construct
                '|' => {
                    let name = "branch_reset_?|".to_string();
                    unsupported.push(name);
                    features.push(RegexFeature::UnsupportedPcreConstruct(
                        "branch_reset".to_string(),
                    ));
                }
                // Non-capturing group (?:...) — allowed, no special routing needed
                ':' => {}
                // Atomic group (?>...) — unsupported PCRE construct
                '>' => {
                    let name = "atomic_group_?>".to_string();
                    unsupported.push(name);
                    features.push(RegexFeature::UnsupportedPcreConstruct(
                        "atomic_group".to_string(),
                    ));
                }
                _ => {
                    // (?P=name) backreference
                    if chars[i] == 'P' && i + 2 < len && chars[i + 1] == '=' {
                        features.push(RegexFeature::Backreference);
                        needs_fancy = true;
                    }
                    // (?P<name>...) — named capture group (supported by both engines)
                    else if chars[i] == 'P' && i + 2 < len && chars[i + 1] == '<' {
                        features.push(RegexFeature::NamedCapture);
                    }
                    // (?R), (?1), (?&name) — recursion/subroutine constructs (unsupported)
                    else if chars[i] == 'R' || chars[i].is_ascii_digit() || chars[i] == '&' {
                        let desc = format!("recursion_or_subroutine_at_{}", group_start);
                        unsupported.push(desc);
                        features.push(RegexFeature::UnsupportedPcreConstruct(
                            "recursion_or_subroutine".to_string(),
                        ));
                    }
                }
            }
            i += 1;
            continue;
        }

        // Detect PCRE control verbs at group level: (*SKIP), (*PRUNE), etc.
        // These appear as (*WORD) which we detect as ( followed by *
        if c == '(' && i + 1 < len && chars[i + 1] == '*' {
            let _verb_start = i;
            i += 2;
            // Collect the verb name
            let mut verb = String::new();
            while i < len && chars[i] != ')' {
                verb.push(chars[i]);
                i += 1;
            }
            // Known PCRE control verbs
            let verb_upper = verb.to_uppercase();
            if matches!(
                verb_upper.as_str(),
                "SKIP"
                    | "PRUNE"
                    | "ACCEPT"
                    | "FAIL"
                    | "F"
                    | "THEN"
                    | "COMMIT"
                    | "COMMITTHEN"
                    | "RESET"
                    | "ATOMIC"
            ) {
                let desc = format!("control_verb_{}", verb_upper);
                unsupported.push(desc);
                features.push(RegexFeature::UnsupportedPcreConstruct(format!(
                    "control_verb_{}",
                    verb_upper
                )));
            }
            i += 1;
            continue;
        }

        i += 1;
    }

    let preferred_engine = if needs_fancy {
        RegexEngineUsed::FancyRegex
    } else {
        RegexEngineUsed::RustRegex
    };

    RegexClassification {
        preferred_engine,
        features,
        unsupported_features: unsupported,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_simple_pattern_uses_rust_regex() {
        let c = classify_pattern(r"\d+");
        assert_eq!(c.preferred_engine, RegexEngineUsed::RustRegex);
        assert!(c.unsupported_features.is_empty());
    }

    #[test]
    fn classify_positive_lookahead_uses_fancy_regex() {
        let c = classify_pattern(r"\d+(?=px)");
        assert_eq!(c.preferred_engine, RegexEngineUsed::FancyRegex);
        assert!(c
            .features
            .iter()
            .any(|f| matches!(f, RegexFeature::LookAhead)));
    }

    #[test]
    fn classify_negative_lookahead_uses_fancy_regex() {
        let c = classify_pattern(r"\d+(?!px)");
        assert_eq!(c.preferred_engine, RegexEngineUsed::FancyRegex);
        assert!(c
            .features
            .iter()
            .any(|f| matches!(f, RegexFeature::LookAhead)));
    }

    #[test]
    fn classify_positive_lookbehind_uses_fancy_regex() {
        let c = classify_pattern(r"(?<=\$)\d+");
        assert_eq!(c.preferred_engine, RegexEngineUsed::FancyRegex);
        assert!(c
            .features
            .iter()
            .any(|f| matches!(f, RegexFeature::LookBehind)));
    }

    #[test]
    fn classify_negative_lookbehind_uses_fancy_regex() {
        let c = classify_pattern(r"(?<!\$)\d+");
        assert_eq!(c.preferred_engine, RegexEngineUsed::FancyRegex);
        assert!(c
            .features
            .iter()
            .any(|f| matches!(f, RegexFeature::LookBehind)));
    }

    #[test]
    fn classify_escaped_lookahead_is_rust_regex() {
        // \(\?= is a literal ( followed by literal =
        let c = classify_pattern(r"\(\?=literal");
        assert_eq!(c.preferred_engine, RegexEngineUsed::RustRegex);
    }

    #[test]
    fn classify_lookahead_inside_char_class_is_rust_regex() {
        let c = classify_pattern(r"[?=]+");
        assert_eq!(c.preferred_engine, RegexEngineUsed::RustRegex);
    }

    #[test]
    fn classify_backreference_uses_fancy_regex() {
        let c = classify_pattern(r"(\w+)\1");
        assert_eq!(c.preferred_engine, RegexEngineUsed::FancyRegex);
        assert!(c
            .features
            .iter()
            .any(|f| matches!(f, RegexFeature::Backreference)));
    }

    #[test]
    fn classify_named_capture_is_rust_regex() {
        let c = classify_pattern(r"(?P<year>\d{4})");
        assert_eq!(c.preferred_engine, RegexEngineUsed::RustRegex);
        assert!(c
            .features
            .iter()
            .any(|f| matches!(f, RegexFeature::NamedCapture)));
    }

    #[test]
    fn classify_inline_flags_is_rust_regex() {
        let c = classify_pattern(r"(?i)hello");
        assert_eq!(c.preferred_engine, RegexEngineUsed::RustRegex);
        assert!(c
            .features
            .iter()
            .any(|f| matches!(f, RegexFeature::InlineFlags)));
    }

    #[test]
    fn classify_branch_reset_is_unsupported() {
        let c = classify_pattern(r"(?|a|b)");
        assert!(!c.unsupported_features.is_empty());
    }

    #[test]
    fn classify_backslash_k_is_unsupported() {
        let c = classify_pattern(r"\K\d+");
        assert!(!c.unsupported_features.is_empty());
    }

    #[test]
    fn classify_control_verb_skip_is_unsupported() {
        let c = classify_pattern(r"(*SKIP)foo");
        assert!(!c.unsupported_features.is_empty());
    }

    #[test]
    fn classify_control_verb_prune_is_unsupported() {
        let c = classify_pattern(r"(*PRUNE)foo");
        assert!(!c.unsupported_features.is_empty());
    }

    #[test]
    fn classify_control_verb_accept_is_unsupported() {
        let c = classify_pattern(r"(*ACCEPT)");
        assert!(!c.unsupported_features.is_empty());
    }

    #[test]
    fn classify_atomic_group_is_unsupported() {
        let c = classify_pattern(r"(?>abc)");
        assert!(!c.unsupported_features.is_empty());
    }

    #[test]
    fn classify_simple_word_boundary_is_rust_regex() {
        let c = classify_pattern(r"\b[a-z_][a-z0-9_]*\b");
        assert_eq!(c.preferred_engine, RegexEngineUsed::RustRegex);
    }

    #[test]
    fn classify_captures_are_rust_regex() {
        let c = classify_pattern(r"(foo)-(bar)");
        assert_eq!(c.preferred_engine, RegexEngineUsed::RustRegex);
    }
}
