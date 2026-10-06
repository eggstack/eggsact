use regex::Regex;
use std::collections::HashMap;

/// Maximum number of `**` segments accepted in a pattern.
///
/// [`match_from`] recurses into the segment after every `**`, so N segments
/// nest N frames deep with nothing bounding the depth: a ~9 KB pattern
/// (`"**/"` repeated) overflowed the default thread stack and aborted the whole
/// process. Adjacent `**` runs are collapsed first, and rejecting above this
/// bound keeps the recursion depth constant. The limit sits roughly 45x below
/// the depth that overflowed, so no realistic pattern is refused.
pub const MAX_DOUBLE_STAR_SEGMENTS: usize = 64;

#[derive(Debug, Clone)]
pub struct GlobMatchResult {
    pub matches: bool,
    pub normalized_pattern: String,
    pub normalized_path: String,
    pub matched_segment: Option<String>,
    pub unmatched_segment: Option<String>,
    pub summary: String,
}

fn split_path_posix(path: &str) -> Vec<&str> {
    if path.is_empty() {
        return vec![];
    }
    path.split('/').filter(|p| !p.is_empty()).collect()
}

fn split_path_windows(path: &str) -> Vec<String> {
    let mut segments: Vec<String> = vec![];

    let chars: Vec<char> = path.chars().collect();
    if chars.len() >= 2 && chars[1] == ':' {
        segments.push(chars[..2].iter().collect());
        let rest: String = chars[2..].iter().collect();
        if !rest.is_empty() {
            let parts: Vec<&str> = rest.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
            segments.extend(parts.iter().map(|p| p.to_string()));
        }
        return segments;
    }

    if path.starts_with("\\\\") {
        let parts: Vec<&str> = path.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
        if parts.len() >= 2 {
            segments.push(format!("\\\\{}\\{}", parts[0], parts[1]));
            segments.extend(parts[2..].iter().map(|p| p.to_string()));
        } else if !parts.is_empty() {
            segments.push(format!("\\\\{}", parts[0]));
        }
        return segments;
    }

    let parts: Vec<&str> = path.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
    parts.into_iter().map(|p| p.to_string()).collect()
}

fn casefold(s: &str) -> String {
    s.to_lowercase()
}

/// Compiled segment patterns, keyed by `(pattern, case_sensitive)`.
///
/// The matcher visits one state per `**` split point, so the same handful of
/// segment patterns is compiled tens of thousands of times per call. Compiling
/// once per distinct pattern turns that into a hash lookup.
type SegmentRegexCache = HashMap<(String, bool), Option<Regex>>;

/// Upper bound on `**` split-point trials per [`glob_match`] call.
///
/// Memoizing `(pattern_idx, path_idx)` makes ordinary patterns linear, but a
/// pattern with several `**` segments still costs
/// O(path_segments × double_star_levels) split-point trials, and that product
/// is unbounded under the 100 KB input cap: `**/*` repeated six times against a
/// 49,000-segment path was measured at 58 s in release mode. This budget caps
/// the work so the call stays inside the tool time budget. Exhausting it is
/// reported explicitly — it must never be answered as a plain "no match".
const MAX_MATCH_STEPS: usize = 1_000_000;

struct MatchContext {
    memo: HashMap<(usize, usize), MatchState>,
    segments: SegmentRegexCache,
    steps_left: usize,
    exhausted: bool,
}

impl MatchContext {
    fn new() -> Self {
        Self {
            memo: HashMap::new(),
            segments: HashMap::new(),
            steps_left: MAX_MATCH_STEPS,
            exhausted: false,
        }
    }
}

fn fnmatch_segment(
    pattern: &str,
    segment: &str,
    case_sensitive: bool,
    cache: &mut SegmentRegexCache,
) -> bool {
    let key = (pattern.to_string(), case_sensitive);
    let compiled = cache.entry(key).or_insert_with(|| {
        let effective = if case_sensitive {
            pattern.to_string()
        } else {
            casefold(pattern)
        };
        Regex::new(&fnmatch_to_regex(&effective)).ok()
    });
    match compiled {
        Some(regex) if case_sensitive => regex.is_match(segment),
        Some(regex) => regex.is_match(&casefold(segment)),
        None => false,
    }
}

fn fnmatch_to_regex(pattern: &str) -> String {
    let mut regex_parts = String::from("^");
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let char = chars[i];
        match char {
            '*' => regex_parts.push_str("[^/]*"),
            '?' => regex_parts.push_str("[^/]"),
            '[' => {
                let mut j = i + 1;
                if j < chars.len() && chars[j] == '!' {
                    j += 1;
                }
                if j < chars.len() && chars[j] == ']' {
                    j += 1;
                }
                while j < chars.len() && chars[j] != ']' {
                    j += 1;
                }
                if j >= chars.len() {
                    regex_parts.push_str("\\[");
                    i += 1;
                } else {
                    let mut char_class = String::new();
                    for c in chars.iter().take(j + 1).skip(i) {
                        char_class.push(*c);
                    }
                    if char_class.starts_with("[!") {
                        char_class = format!("[^{}", &char_class[2..]);
                    }
                    regex_parts.push_str(&char_class);
                    i = j;
                }
            }
            '/' => regex_parts.push('/'),
            '\\' | '.' | '^' | '$' | '|' | '(' | ')' | '{' | '}' | '+' => {
                regex_parts.push('\\');
                regex_parts.push(char);
            }
            _ => {
                regex_parts.push(char);
            }
        }
        i += 1;
    }

    regex_parts.push('$');
    regex_parts
}

/// Match `pattern_parts[pattern_idx..]` against `path_parts[path_idx..]`.
///
/// Returns the absolute `(pattern_idx, path_idx)` reached on success. Indexes
/// are always into the full slices, never into a tail — that keeps the memo key
/// meaningful across the whole match.
type MatchState = Option<(usize, usize)>;

fn match_from(
    pattern_parts: &[String],
    path_parts: &[&str],
    pattern_idx: usize,
    path_idx: usize,
    case_sensitive: bool,
    ctx: &mut MatchContext,
) -> MatchState {
    if let Some(cached) = ctx.memo.get(&(pattern_idx, path_idx)) {
        return *cached;
    }
    let outcome = match_from_uncached(
        pattern_parts,
        path_parts,
        pattern_idx,
        path_idx,
        case_sensitive,
        ctx,
    );
    ctx.memo.insert((pattern_idx, path_idx), outcome);
    outcome
}

fn match_from_uncached(
    pattern_parts: &[String],
    path_parts: &[&str],
    pattern_idx: usize,
    path_idx: usize,
    case_sensitive: bool,
    ctx: &mut MatchContext,
) -> MatchState {
    let mut p_idx = pattern_idx;
    let mut path_idx = path_idx;

    while p_idx < pattern_parts.len() && path_idx < path_parts.len() {
        let pattern_seg: &str = pattern_parts[p_idx].as_str();

        if pattern_seg == "**" {
            let next_pattern_idx = p_idx + 1;
            // A trailing `**` swallows whatever path remains.
            if next_pattern_idx >= pattern_parts.len() {
                return Some((next_pattern_idx, path_parts.len()));
            }
            // Try every split point for this `**`. Each candidate recurses into
            // `match_from`, which memoizes on `(pattern_idx, path_idx)`, so a
            // state is explored once instead of once per distinct route to it.
            for candidate in path_idx..=path_parts.len() {
                if ctx.steps_left == 0 {
                    ctx.exhausted = true;
                    return None;
                }
                ctx.steps_left -= 1;
                if let Some(reached) = match_from(
                    pattern_parts,
                    path_parts,
                    next_pattern_idx,
                    candidate,
                    case_sensitive,
                    ctx,
                ) {
                    return Some(reached);
                }
            }
            return None;
        }

        // A segment that merely *contains* `**` (e.g. `a**b`) is not a
        // double-star; it can never match, matching prior behaviour.
        if pattern_seg.contains("**") {
            return None;
        }

        if !fnmatch_segment(
            pattern_seg,
            path_parts[path_idx],
            case_sensitive,
            &mut ctx.segments,
        ) {
            return None;
        }
        p_idx += 1;
        path_idx += 1;
    }

    // The path is exhausted: only trailing `**` segments may remain.
    while p_idx < pattern_parts.len() {
        if pattern_parts[p_idx].as_str() != "**" {
            return None;
        }
        p_idx += 1;
    }

    Some((p_idx, path_idx))
}

/// Split a glob pattern into segments, keeping `**` as its own segment.
///
/// Separators are `/` everywhere; on Windows `\` separates too, so a pattern
/// splits the same way [`split_path_windows`] splits the path. Without this the
/// pattern stayed a single `src\*.rs` segment whose backslash
/// [`fnmatch_to_regex`] escaped to a literal, and every native Windows pattern
/// reported "does not match" — including one compared against itself.
///
/// A Windows drive prefix and a `\\server\share` UNC prefix each form one
/// leading segment, mirroring [`split_path_windows`] so both sides line up.
fn split_pattern(pattern: &str, windows: bool) -> Vec<String> {
    let chars: Vec<char> = pattern.chars().collect();
    let byte_at: Vec<usize> = pattern
        .char_indices()
        .map(|(byte_offset, _)| byte_offset)
        .chain(std::iter::once(pattern.len()))
        .collect();
    let is_separator = |i: usize| chars[i] == '/' || (windows && chars[i] == '\\');

    let mut parts: Vec<String> = vec![];
    let mut i = 0;

    if windows {
        if chars.len() >= 2 && chars[1] == ':' {
            parts.push(pattern[..byte_at[2]].to_string());
            i = 2;
        } else if pattern.starts_with("\\\\") {
            let mut cursor = 2;
            let mut host = String::new();
            while cursor < chars.len() && !is_separator(cursor) {
                host.push(chars[cursor]);
                cursor += 1;
            }
            if cursor < chars.len() {
                cursor += 1;
            }
            let mut share = String::new();
            while cursor < chars.len() && !is_separator(cursor) {
                share.push(chars[cursor]);
                cursor += 1;
            }
            if !host.is_empty() {
                if share.is_empty() {
                    parts.push(format!("\\\\{}", host));
                } else {
                    parts.push(format!("\\\\{}\\{}", host, share));
                }
            }
            i = cursor;
        }
    }

    while i < chars.len() {
        if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '*' {
            parts.push("**".to_string());
            i += 2;
            if i < chars.len() && is_separator(i) {
                i += 1;
            }
        } else if is_separator(i) {
            i += 1;
        } else {
            let mut j = i;
            while j < chars.len() && !is_separator(j) {
                if j + 1 < chars.len() && chars[j] == '*' && chars[j + 1] == '*' {
                    break;
                }
                j += 1;
            }
            parts.push(pattern[byte_at[i]..byte_at[j]].to_string());
            i = j;
        }
    }

    // `**/**` matches exactly what `**` matches. Collapsing redundant runs keeps
    // the segment count — and therefore the matcher recursion depth — honest.
    parts.dedup_by(|a, b| a == "**" && b == "**");
    parts
}

fn count_double_star_segments(pattern_parts: &[String]) -> usize {
    pattern_parts
        .iter()
        .filter(|part| part.as_str() == "**")
        .count()
}

/// Number of effective `**` segments in `pattern` for `platform`.
///
/// The tool adapter uses this to reject an over-deep pattern with
/// `INVALID_ARGUMENTS` rather than let [`glob_match`] answer with a
/// non-match verdict it could not actually evaluate.
pub fn double_star_segment_count(pattern: &str, platform: &str) -> usize {
    count_double_star_segments(&split_pattern(pattern, platform == "windows"))
}

pub fn glob_match(
    pattern: &str,
    path: &str,
    platform: &str,
    case_sensitive: bool,
) -> GlobMatchResult {
    let normalized_pattern = pattern.to_string();
    let normalized_path = path.to_string();

    let path_parts: Vec<String> = if platform == "windows" {
        split_path_windows(path)
    } else {
        split_path_posix(path)
            .into_iter()
            .map(|s| s.to_string())
            .collect()
    };

    let pattern_parts = split_pattern(pattern, platform == "windows");

    // Bound the recursion before it starts rather than trusting the input cap:
    // MAX_TEXT_LENGTH is ~11x above the depth that overflowed the stack.
    let double_star_segments = count_double_star_segments(&pattern_parts);
    if double_star_segments > MAX_DOUBLE_STAR_SEGMENTS {
        return GlobMatchResult {
            matches: false,
            normalized_pattern,
            normalized_path,
            matched_segment: None,
            unmatched_segment: None,
            summary: format!(
                "Pattern has {} `**` segments; at most {} are supported",
                double_star_segments, MAX_DOUBLE_STAR_SEGMENTS
            ),
        };
    }

    let path_strs: Vec<&str> = path_parts.iter().map(|s| s.as_str()).collect();
    let mut ctx = MatchContext::new();
    let matched = match_from(&pattern_parts, &path_strs, 0, 0, case_sensitive, &mut ctx).is_some();
    if ctx.exhausted {
        return GlobMatchResult {
            matches: false,
            normalized_pattern,
            normalized_path,
            matched_segment: None,
            unmatched_segment: None,
            summary: format!(
                "Pattern and path exceed the maximum matching effort of {} steps",
                MAX_MATCH_STEPS
            ),
        };
    }

    if matched {
        GlobMatchResult {
            matches: true,
            normalized_pattern,
            normalized_path,
            matched_segment: None,
            unmatched_segment: None,
            summary: "Pattern matches path".to_string(),
        }
    } else {
        GlobMatchResult {
            matches: false,
            normalized_pattern,
            normalized_path,
            matched_segment: None,
            unmatched_segment: None,
            summary: "Pattern does not match path".to_string(),
        }
    }
}
