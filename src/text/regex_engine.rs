use fancy_regex::Regex as FancyRegex;
use regex::Regex as StdRegex;
use serde::{Deserialize, Serialize};

/// Which backend compiled the pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RegexEngineUsed {
    RustRegex,
    FancyRegex,
}

impl RegexEngineUsed {
    pub fn as_str(&self) -> &'static str {
        match self {
            RegexEngineUsed::RustRegex => "rust-regex",
            RegexEngineUsed::FancyRegex => "fancy-regex",
        }
    }
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

/// A backend-independent representation of a regex match.
#[derive(Debug, Clone)]
pub struct CompiledMatch<'t> {
    pub text: &'t str,
    pub start: usize,
    pub end: usize,
}

impl<'t> CompiledMatch<'t> {
    pub fn as_str(&self) -> &'t str {
        self.text
    }
}

/// A backend-independent representation of captured groups.
///
/// Stores one source reference and absolute byte ranges. Every range is
/// absolute relative to the original input passed to the matching method.
/// `get()` slices `source[start..end]` directly — no redundant substring
/// copies are stored per group.
#[derive(Debug, Clone)]
pub struct CompiledCaptures<'t> {
    /// The original input text.
    source: &'t str,
    /// Full match range (group 0). `None` only if the match object had no group 0.
    full_match_range: Option<(usize, usize)>,
    /// Per-group byte ranges (index 1..len). `None` means nonparticipating.
    groups: Vec<Option<(usize, usize)>>,
    /// Named capture name → group index (0 = full match, 1 = first group, ...).
    names: std::collections::BTreeMap<String, usize>,
    /// Total number of capture groups including group 0.
    len: usize,
}

impl<'t> CompiledCaptures<'t> {
    /// Get a capture group by index (0 = full match, 1 = first group, ...).
    pub fn get(&self, i: usize) -> Option<CompiledMatch<'t>> {
        if i == 0 {
            return self.full_match_range.map(|(start, end)| CompiledMatch {
                text: &self.source[start..end],
                start,
                end,
            });
        }
        self.groups.get(i - 1).and_then(|opt| {
            opt.map(|(start, end)| CompiledMatch {
                text: &self.source[start..end],
                start,
                end,
            })
        })
    }

    /// Get a capture group by name.
    pub fn name(&self, name: &str) -> Option<CompiledMatch<'t>> {
        self.names.get(name).and_then(|&i| self.get(i))
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// A compiled regex pattern that encapsulates the actual backend used.
///
/// This ensures `engine_used` is always derived from the compiled variant,
/// not recomputed separately from classification.
pub enum CompiledRegex {
    Rust(StdRegex),
    Fancy(FancyRegex),
}

impl std::fmt::Debug for CompiledRegex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompiledRegex::Rust(_) => write!(f, "CompiledRegex::Rust(...)"),
            CompiledRegex::Fancy(_) => write!(f, "CompiledRegex::Fancy(...)"),
        }
    }
}

impl CompiledRegex {
    /// The backend that actually compiled this pattern.
    pub fn engine_used(&self) -> RegexEngineUsed {
        match self {
            CompiledRegex::Rust(_) => RegexEngineUsed::RustRegex,
            CompiledRegex::Fancy(_) => RegexEngineUsed::FancyRegex,
        }
    }

    /// Find the first match in `text`.
    pub fn find<'t>(&self, text: &'t str) -> Result<Option<CompiledMatch<'t>>, fancy_regex::Error> {
        match self {
            CompiledRegex::Rust(re) => Ok(re.find(text).map(|m| CompiledMatch {
                text: &text[m.start()..m.end()],
                start: m.start(),
                end: m.end(),
            })),
            CompiledRegex::Fancy(re) => Ok(re.find(text)?.map(|m| CompiledMatch {
                text: &text[m.start()..m.end()],
                start: m.start(),
                end: m.end(),
            })),
        }
    }

    /// Capture groups for the first match in `text`.
    pub fn captures<'t>(
        &self,
        text: &'t str,
    ) -> Result<Option<CompiledCaptures<'t>>, fancy_regex::Error> {
        let name_iter: Box<dyn Iterator<Item = Option<String>> + '_> = match self {
            CompiledRegex::Rust(re) => {
                Box::new(re.capture_names().map(|o| o.map(|s| s.to_string())))
            }
            CompiledRegex::Fancy(re) => {
                Box::new(re.capture_names().map(|o| o.map(|s| s.to_string())))
            }
        };
        let mut result = match self {
            CompiledRegex::Rust(re) => Ok(re
                .captures(text)
                .map(|caps| convert_captures_std(&caps, text, 0))),
            CompiledRegex::Fancy(re) => Ok(re
                .captures(text)?
                .map(|caps| convert_captures_fancy(&caps, text))),
        };
        if let Ok(Some(ref mut caps)) = result {
            populate_capture_names(&mut caps.names, name_iter);
        }
        result
    }

    /// Capture groups starting from a byte position in `text`.
    pub fn captures_from_pos<'t>(
        &self,
        text: &'t str,
        pos: usize,
    ) -> Result<Option<CompiledCaptures<'t>>, fancy_regex::Error> {
        let name_iter: Box<dyn Iterator<Item = Option<String>> + '_> = match self {
            CompiledRegex::Rust(re) => {
                Box::new(re.capture_names().map(|o| o.map(|s| s.to_string())))
            }
            CompiledRegex::Fancy(re) => {
                Box::new(re.capture_names().map(|o| o.map(|s| s.to_string())))
            }
        };
        let mut result = match self {
            CompiledRegex::Rust(re) => {
                if pos >= text.len() {
                    return Ok(None);
                }
                Ok(re
                    .captures(&text[pos..])
                    .map(|caps| convert_captures_std(&caps, text, pos)))
            }
            CompiledRegex::Fancy(re) => Ok(re
                .captures_from_pos(text, pos)?
                .map(|caps| convert_captures_fancy(&caps, text))),
        };
        if let Ok(Some(ref mut caps)) = result {
            populate_capture_names(&mut caps.names, name_iter);
        }
        result
    }

    /// Iterator over capture group names.
    pub fn capture_names(&self) -> Box<dyn Iterator<Item = Option<&str>> + '_> {
        match self {
            CompiledRegex::Rust(re) => Box::new(re.capture_names()),
            CompiledRegex::Fancy(re) => Box::new(re.capture_names()),
        }
    }

    /// Test whether the pattern matches the text.
    pub fn is_match(&self, text: &str) -> Result<bool, fancy_regex::Error> {
        match self {
            CompiledRegex::Rust(re) => Ok(re.is_match(text)),
            CompiledRegex::Fancy(re) => re.is_match(text),
        }
    }

    /// Whether this is the rust-regex backend.
    pub fn is_rust(&self) -> bool {
        matches!(self, CompiledRegex::Rust(_))
    }

    /// Whether this is the fancy-regex backend.
    pub fn is_fancy(&self) -> bool {
        matches!(self, CompiledRegex::Fancy(_))
    }
}

/// Convert fancy_regex captures to our backend-independent form.
///
/// fancy_regex returns absolute byte ranges into the original text, so no
/// position adjustment is needed. Names are populated separately by the caller.
fn convert_captures_fancy<'t>(
    caps: &fancy_regex::Captures<'t>,
    text: &'t str,
) -> CompiledCaptures<'t> {
    let full = caps.get(0).unwrap();
    let mut groups = Vec::new();

    for i in 1..caps.len() {
        groups.push(caps.get(i).map(|m| (m.start(), m.end())));
    }

    CompiledCaptures {
        source: text,
        full_match_range: Some((full.start(), full.end())),
        len: caps.len(),
        groups,
        names: std::collections::BTreeMap::new(),
    }
}

/// Convert regex::Captures to our backend-independent form.
///
/// When called from `captures_from_pos`, `pos` is the byte offset added to
/// every range so the result uses absolute positions into the original input.
/// Names are populated separately by the caller.
fn convert_captures_std<'t>(
    caps: &regex::Captures<'t>,
    text: &'t str,
    pos: usize,
) -> CompiledCaptures<'t> {
    let full = caps.get(0).unwrap();
    let mut groups = Vec::new();

    for i in 1..caps.len() {
        groups.push(caps.get(i).map(|m| (m.start() + pos, m.end() + pos)));
    }

    CompiledCaptures {
        source: text,
        full_match_range: Some((full.start() + pos, full.end() + pos)),
        len: caps.len(),
        groups,
        names: std::collections::BTreeMap::new(),
    }
}

/// Populate named capture indices from the backend's capture_names iterator.
fn populate_capture_names(
    names: &mut std::collections::BTreeMap<String, usize>,
    name_iter: impl Iterator<Item = Option<String>>,
) {
    for (idx, name_opt) in name_iter.enumerate() {
        if let Some(name) = name_opt {
            names.insert(name, idx);
        }
    }
}

/// Normalize flag parameters into an inline flag prefix string.
///
/// Recognized flags: IGNORECASE/I, MULTILINE/M, DOTALL/S, VERBOSE/X.
/// Returns the prefix to prepend to the pattern, e.g. `"(?ims)"`.
pub fn normalize_flags(
    flags: Option<&[String]>,
    ignore_case: bool,
    multiline: bool,
    dotall: bool,
) -> String {
    let mut prefix = String::new();
    if ignore_case {
        prefix.push('i');
    }
    if multiline {
        prefix.push('m');
    }
    if dotall {
        prefix.push('s');
    }
    if let Some(flag_list) = flags {
        for flag in flag_list {
            match flag.to_uppercase().as_str() {
                "IGNORECASE" | "I" if !prefix.contains('i') => prefix.push('i'),
                "MULTILINE" | "M" if !prefix.contains('m') => prefix.push('m'),
                "DOTALL" | "S" if !prefix.contains('s') => prefix.push('s'),
                "VERBOSE" | "X" if !prefix.contains('x') => prefix.push('x'),
                _ => {}
            }
        }
    }
    if prefix.is_empty() {
        String::new()
    } else {
        format!("(?{})", prefix)
    }
}

/// Compile a pattern with the appropriate backend.
///
/// Applies flag normalization, rejects unsupported PCRE constructs, and returns
/// the compiled regex with the actual backend used.
pub fn compile_regex(
    pattern: &str,
    flags: Option<&[String]>,
    ignore_case: bool,
    multiline: bool,
    dotall: bool,
) -> Result<CompiledRegex, CompileError> {
    let classification = classify_pattern(pattern);

    if !classification.unsupported_features.is_empty() {
        return Err(CompileError::Unsupported(
            classification.unsupported_features,
        ));
    }

    let flag_prefix = normalize_flags(flags, ignore_case, multiline, dotall);
    let full_pattern = format!("{}{}", flag_prefix, pattern);

    match classification.preferred_engine {
        RegexEngineUsed::RustRegex => match StdRegex::new(&full_pattern) {
            Ok(re) => Ok(CompiledRegex::Rust(re)),
            Err(e) => Err(CompileError::Compile {
                engine: RegexEngineUsed::RustRegex,
                error: e.to_string(),
            }),
        },
        RegexEngineUsed::FancyRegex => match FancyRegex::new(&full_pattern) {
            Ok(re) => Ok(CompiledRegex::Fancy(re)),
            Err(e) => Err(CompileError::Compile {
                engine: RegexEngineUsed::FancyRegex,
                error: e.to_string(),
            }),
        },
    }
}

/// Errors from regex compilation.
#[derive(Debug, Clone)]
pub enum CompileError {
    /// The pattern uses unsupported PCRE-only constructs.
    Unsupported(Vec<String>),
    /// The selected backend failed to compile the pattern.
    Compile {
        engine: RegexEngineUsed,
        error: String,
    },
}

impl CompileError {
    /// The engine that was attempted, if known.
    pub fn engine(&self) -> Option<RegexEngineUsed> {
        match self {
            CompileError::Unsupported(_) => None,
            CompileError::Compile { engine, .. } => Some(*engine),
        }
    }
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

#[cfg(test)]
mod compile_tests {
    use super::*;

    #[test]
    fn compile_simple_pattern_uses_rust_regex() {
        let compiled = compile_regex(r"\d+", None, false, false, false).unwrap();
        assert_eq!(compiled.engine_used(), RegexEngineUsed::RustRegex);
        assert!(compiled.find("abc123").unwrap().is_some());
    }

    #[test]
    fn compile_lookahead_uses_fancy_regex() {
        let compiled = compile_regex(r"\d+(?=px)", None, false, false, false).unwrap();
        assert_eq!(compiled.engine_used(), RegexEngineUsed::FancyRegex);
        assert!(compiled.find("123px").unwrap().is_some());
    }

    #[test]
    fn compile_lookbehind_uses_fancy_regex() {
        let compiled = compile_regex(r"(?<=\$)\d+", None, false, false, false).unwrap();
        assert_eq!(compiled.engine_used(), RegexEngineUsed::FancyRegex);
        assert!(compiled.find("$100").unwrap().is_some());
    }

    #[test]
    fn compile_backreference_uses_fancy_regex() {
        let compiled = compile_regex(r"(\w+)\1", None, false, false, false).unwrap();
        assert_eq!(compiled.engine_used(), RegexEngineUsed::FancyRegex);
        assert!(compiled.find("abcabc").unwrap().is_some());
    }

    #[test]
    fn compile_inline_flags_uses_rust_regex() {
        let compiled = compile_regex(r"(?i)hello", None, false, false, false).unwrap();
        assert_eq!(compiled.engine_used(), RegexEngineUsed::RustRegex);
        assert!(compiled.find("HELLO").unwrap().is_some());
    }

    #[test]
    fn compile_unsupported_returns_error() {
        let err = compile_regex(r"(*SKIP)foo", None, false, false, false);
        assert!(err.is_err());
        match err.unwrap_err() {
            CompileError::Unsupported(features) => {
                assert!(!features.is_empty());
            }
            _ => panic!("Expected Unsupported error"),
        }
    }

    #[test]
    fn compile_invalid_pattern_returns_error() {
        let err = compile_regex(r"[", None, false, false, false);
        assert!(err.is_err());
        match err.unwrap_err() {
            CompileError::Compile { engine, error } => {
                assert_eq!(engine, RegexEngineUsed::RustRegex);
                assert!(!error.is_empty());
            }
            _ => panic!("Expected Compile error"),
        }
    }

    #[test]
    fn compile_captures_work() {
        let compiled = compile_regex(r"(\d+)-(\d+)", None, false, false, false).unwrap();
        let caps = compiled.captures("123-456").unwrap().unwrap();
        assert_eq!(caps.get(1).unwrap().as_str(), "123");
        assert_eq!(caps.get(2).unwrap().as_str(), "456");
    }

    #[test]
    fn compile_captures_from_pos_work() {
        let compiled = compile_regex(r"\d+", None, false, false, false).unwrap();
        let caps = compiled
            .captures_from_pos("abc123def456", 3)
            .unwrap()
            .unwrap();
        assert_eq!(caps.get(0).unwrap().as_str(), "123");
    }

    #[test]
    fn compile_capture_names_work() {
        let compiled = compile_regex(r"(?P<year>\d{4})", None, false, false, false).unwrap();
        let names: Vec<_> = compiled.capture_names().collect();
        assert!(names.contains(&Some("year")));
    }

    #[test]
    fn compile_with_flag_params() {
        let compiled = compile_regex(r"hello", None, true, false, false).unwrap();
        assert!(compiled.find("HELLO").unwrap().is_some());
    }

    #[test]
    fn compile_with_flags_array() {
        let flags = vec!["IGNORECASE".to_string()];
        let compiled = compile_regex(r"hello", Some(&flags), false, false, false).unwrap();
        assert!(compiled.find("HELLO").unwrap().is_some());
    }

    #[test]
    fn compile_engine_used_matches_compiled_variant() {
        // Simple pattern: should be rust-regex
        let compiled = compile_regex(r"\w+", None, false, false, false).unwrap();
        assert_eq!(compiled.engine_used(), RegexEngineUsed::RustRegex);

        // Lookahead pattern: should be fancy-regex
        let compiled = compile_regex(r"(?=x)x", None, false, false, false).unwrap();
        assert_eq!(compiled.engine_used(), RegexEngineUsed::FancyRegex);
    }
}

#[cfg(test)]
mod capture_tests {
    use super::*;

    // ── Rust backend captures ──────────────────────────────────────────

    #[test]
    fn rust_unnamed_captures_at_offset_zero() {
        let re = compile_regex(r"(\d+)-(\d+)", None, false, false, false).unwrap();
        let caps = re.captures("123-456").unwrap().unwrap();
        assert_eq!(caps.get(0).unwrap().as_str(), "123-456");
        assert_eq!(caps.get(1).unwrap().as_str(), "123");
        assert_eq!(caps.get(2).unwrap().as_str(), "456");
    }

    #[test]
    fn rust_unnamed_captures_after_prefix() {
        let re = compile_regex(r"prefix(\d+)", None, false, false, false).unwrap();
        let caps = re.captures("prefix42").unwrap().unwrap();
        assert_eq!(caps.get(0).unwrap().as_str(), "prefix42");
        assert_eq!(caps.get(1).unwrap().as_str(), "42");
    }

    #[test]
    fn rust_named_captures_via_name_lookup() {
        let re = compile_regex(r"(?P<word>[A-Za-z]+)", None, false, false, false).unwrap();
        let caps = re.captures("hello").unwrap().unwrap();
        let m = caps.name("word").unwrap();
        assert_eq!(m.as_str(), "hello");
    }

    #[test]
    fn rust_two_named_and_optional_nonparticipating() {
        let re = compile_regex(
            r"(?P<head>[A-Za-z]+)-(?P<num>\d+)",
            None,
            false,
            false,
            false,
        )
        .unwrap();
        let caps = re.captures("abc-123").unwrap().unwrap();
        assert_eq!(caps.name("head").unwrap().as_str(), "abc");
        assert_eq!(caps.name("num").unwrap().as_str(), "123");
    }

    #[test]
    fn rust_optional_group_nonparticipating() {
        let re = compile_regex(r"(?P<a>a)?(?P<b>b)", None, false, false, false).unwrap();
        let caps = re.captures("b").unwrap().unwrap();
        // "a" group is optional and didn't match
        assert!(caps.name("a").is_none());
        assert_eq!(caps.name("b").unwrap().as_str(), "b");
    }

    #[test]
    fn rust_captures_from_pos_absolute() {
        let re = compile_regex(r"(\d+)", None, false, false, false).unwrap();
        let caps = re.captures_from_pos("abc123def456", 3).unwrap().unwrap();
        // Should be absolute positions, not relative to slice
        let m0 = caps.get(0).unwrap();
        assert_eq!(m0.as_str(), "123");
        assert_eq!(m0.start, 3);
        assert_eq!(m0.end, 6);
        let m1 = caps.get(1).unwrap();
        assert_eq!(m1.start, 3);
        assert_eq!(m1.end, 6);
    }

    #[test]
    fn rust_unicode_before_capture() {
        // \p{L}+ matches Unicode letters; verifies Unicode before the match
        // is handled correctly (offsets into the source are correct).
        let re = compile_regex(r"\p{L}+", None, false, false, false).unwrap();
        let text = "üñîçödé hello";
        let caps = re.captures(text).unwrap().unwrap();
        assert_eq!(caps.get(0).unwrap().as_str(), "üñîçödé");
    }

    #[test]
    fn rust_unicode_inside_capture() {
        let re = compile_regex(r"\w+", None, false, false, false).unwrap();
        let text = "café";
        let caps = re.captures(text).unwrap().unwrap();
        assert_eq!(caps.get(0).unwrap().as_str(), "café");
    }

    // ── Fancy backend captures ─────────────────────────────────────────

    #[test]
    fn fancy_lookbehind_with_unnamed_capture() {
        let re = compile_regex(r"(?<=prefix)(\d+)", None, false, false, false).unwrap();
        assert!(re.is_fancy());
        let caps = re.captures("prefix42").unwrap().unwrap();
        assert_eq!(caps.get(0).unwrap().as_str(), "42");
        assert_eq!(caps.get(1).unwrap().as_str(), "42");
    }

    #[test]
    fn fancy_lookahead_with_named_capture() {
        let re = compile_regex(r"(?P<value>\w+)(?=suffix)", None, false, false, false).unwrap();
        assert!(re.is_fancy());
        let caps = re.captures("hellosuffix").unwrap().unwrap();
        assert_eq!(caps.name("value").unwrap().as_str(), "hello");
    }

    #[test]
    fn fancy_named_capture_not_at_byte_zero() {
        // Lookbehind forces fancy-regex; named capture inside lookbehind
        let re = compile_regex(r"(?<=prefix)(?P<value>\w+)", None, false, false, false).unwrap();
        assert!(re.is_fancy());
        let caps = re.captures("prefixhello").unwrap().unwrap();
        let m = caps.name("value").unwrap();
        assert_eq!(m.as_str(), "hello");
        assert!(m.start > 0); // not at byte zero
    }

    #[test]
    fn fancy_multibyte_unicode_before_capture() {
        let re = compile_regex(r"(?<=üñ)(?P<value>[a-z]+)", None, false, false, false).unwrap();
        assert!(re.is_fancy());
        let caps = re.captures("üñhello").unwrap().unwrap();
        assert_eq!(caps.name("value").unwrap().as_str(), "hello");
    }

    #[test]
    fn fancy_multibyte_unicode_inside_capture() {
        let re = compile_regex(r"(?<=prefix)(?P<value>\w+)", None, false, false, false).unwrap();
        assert!(re.is_fancy());
        let caps = re.captures("prefixcafé").unwrap().unwrap();
        assert_eq!(caps.name("value").unwrap().as_str(), "café");
    }

    #[test]
    fn fancy_captures_from_pos_absolute() {
        let re = compile_regex(r"(?<=prefix)(\d+)", None, false, false, false).unwrap();
        assert!(re.is_fancy());
        let text = "prefix42more";
        let caps = re.captures_from_pos(text, 6).unwrap().unwrap();
        let m0 = caps.get(0).unwrap();
        assert_eq!(m0.as_str(), "42");
        assert_eq!(m0.start, 6);
        assert_eq!(m0.end, 8);
    }

    #[test]
    fn fancy_no_panic_for_valid_utf8() {
        let re = compile_regex(r"(?<=prefix)(?P<value>.+)", None, false, false, false).unwrap();
        assert!(re.is_fancy());
        let text = "prefixüñîçödé";
        let caps = re.captures(text).unwrap().unwrap();
        assert_eq!(caps.name("value").unwrap().as_str(), "üñîçödé");
    }
}

#[cfg(test)]
mod normalize_flags_tests {
    use super::*;

    #[test]
    fn no_flags() {
        assert_eq!(normalize_flags(None, false, false, false), "");
    }

    #[test]
    fn bool_params() {
        assert_eq!(normalize_flags(None, true, false, false), "(?i)");
        assert_eq!(normalize_flags(None, false, true, false), "(?m)");
        assert_eq!(normalize_flags(None, false, false, true), "(?s)");
        assert_eq!(normalize_flags(None, true, true, true), "(?ims)");
    }

    #[test]
    fn flag_array() {
        let flags = vec!["IGNORECASE".to_string()];
        assert_eq!(normalize_flags(Some(&flags), false, false, false), "(?i)");
    }

    #[test]
    fn flag_array_short_form() {
        let flags = vec!["I".to_string(), "M".to_string()];
        assert_eq!(normalize_flags(Some(&flags), false, false, false), "(?im)");
    }

    #[test]
    fn deduplicates_flags() {
        let flags = vec!["I".to_string()];
        assert_eq!(normalize_flags(Some(&flags), true, false, false), "(?i)");
    }

    #[test]
    fn verbose_flag() {
        let flags = vec!["X".to_string()];
        assert_eq!(normalize_flags(Some(&flags), false, false, false), "(?x)");
    }

    #[test]
    fn mixed_bool_and_array() {
        let flags = vec!["M".to_string()];
        assert_eq!(normalize_flags(Some(&flags), true, false, false), "(?im)");
    }
}
