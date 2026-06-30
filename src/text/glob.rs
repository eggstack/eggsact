use regex::Regex;

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

    if path.len() >= 2 && path.chars().nth(1) == Some(':') {
        segments.push(path[..2].to_string());
        let rest = &path[2..];
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

fn fnmatch_segment(pattern: &str, segment: &str, case_sensitive: bool) -> bool {
    let pattern = if case_sensitive {
        pattern.to_string()
    } else {
        casefold(pattern)
    };
    let segment = if case_sensitive {
        segment.to_string()
    } else {
        casefold(segment)
    };

    let regex_pattern = fnmatch_to_regex(&pattern);
    match Regex::new(&regex_pattern) {
        Ok(re) => re.is_match(&segment),
        Err(_) => false,
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

fn match_double_star(
    pattern_parts: &[&str],
    path_parts: &[String],
    p_idx: usize,
    mut path_idx: usize,
    case_sensitive: bool,
) -> (bool, usize, usize) {
    let next_pattern_idx = p_idx + 1;

    if next_pattern_idx >= pattern_parts.len() {
        return (true, next_pattern_idx, path_parts.len());
    }

    while path_idx <= path_parts.len() {
        let remaining_pattern = &pattern_parts[next_pattern_idx..];
        let remaining_path: Vec<&str> = if path_idx < path_parts.len() {
            path_parts[path_idx..].iter().map(|s| s.as_str()).collect()
        } else {
            vec![]
        };

        let (matched, consumed_p, consumed_path) =
            match_segments(remaining_pattern, &remaining_path, case_sensitive);

        if matched {
            return (
                true,
                next_pattern_idx + consumed_p,
                path_idx + consumed_path,
            );
        }

        if path_idx < path_parts.len() {
            path_idx += 1;
        } else {
            break;
        }
    }

    (false, p_idx, p_idx)
}

fn match_segments(
    pattern_parts: &[&str],
    path_parts: &[&str],
    case_sensitive: bool,
) -> (bool, usize, usize) {
    let mut p_idx = 0;
    let mut path_idx = 0;

    while p_idx < pattern_parts.len() && path_idx < path_parts.len() {
        let pattern_seg = pattern_parts[p_idx];

        if pattern_seg == "**" {
            let path_strs: Vec<String> = path_parts.iter().map(|s| (*s).to_string()).collect();
            let (matched, new_p_idx, new_path_idx) =
                match_double_star(pattern_parts, &path_strs, p_idx, path_idx, case_sensitive);
            if !matched {
                return (false, p_idx, path_idx);
            }
            p_idx = new_p_idx;
            path_idx = new_path_idx;
        } else if pattern_seg.contains("**") {
            return (false, p_idx, path_idx);
        } else {
            if !fnmatch_segment(pattern_seg, path_parts[path_idx], case_sensitive) {
                return (false, p_idx, path_idx);
            }
            p_idx += 1;
            path_idx += 1;
        }
    }

    while p_idx < pattern_parts.len() {
        if pattern_parts[p_idx] == "**" {
            p_idx += 1;
        } else {
            return (false, p_idx, path_idx);
        }
    }

    (p_idx == pattern_parts.len(), p_idx, path_idx)
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

    let mut pattern_parts: Vec<&str> = vec![];
    let mut i = 0;

    let chars: Vec<char> = pattern.chars().collect();
    let char_to_byte: Vec<usize> = pattern
        .char_indices()
        .map(|(byte_offset, _)| byte_offset)
        .chain(std::iter::once(pattern.len()))
        .collect();

    while i < chars.len() {
        if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '*' {
            if i + 2 < chars.len() && chars[i + 2] == '/' {
                pattern_parts.push("**");
                i += 3;
            } else {
                pattern_parts.push("**");
                i += 2;
            }
        } else if chars[i] == '/' {
            i += 1;
        } else {
            let mut j = i;
            while j < chars.len() {
                if chars[j] == '/' {
                    break;
                }
                if j + 1 < chars.len() && chars[j] == '*' && chars[j + 1] == '*' {
                    break;
                }
                j += 1;
            }
            let byte_start = char_to_byte[i];
            let byte_end = char_to_byte[j];
            pattern_parts.push(&pattern[byte_start..byte_end]);
            i = j;
        }
    }

    let path_strs: Vec<&str> = path_parts.iter().map(|s| s.as_str()).collect();
    let (matched, _, _) = match_segments(&pattern_parts, &path_strs, case_sensitive);

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
