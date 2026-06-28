use crate::text::confusables::find_confusables;

#[derive(Debug, Clone)]
pub struct PathNormalizeResult {
    pub normalized: String,
    pub is_absolute: bool,
    pub components: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PathAnalyzeResult {
    pub input: String,
    pub style: String,
    pub absolute: bool,
    pub has_traversal: bool,
    pub components: Vec<String>,
    pub parent: Option<String>,
    pub name: Option<String>,
    pub stem: Option<String>,
    pub suffix: Option<String>,
    pub suffixes: Vec<String>,
    pub hidden: bool,
    pub normalized_lexical: String,
    pub warnings: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone)]
pub struct PathCompareResult {
    pub equal: bool,
    pub left_normalized: String,
    pub right_normalized: String,
    pub differences: Vec<String>,
    pub findings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PathScopeCheckResult {
    pub inside_root: bool,
    pub root_normalized: String,
    pub target_normalized: String,
    pub relative_path: String,
    pub escapes_via_dotdot: bool,
    pub absolute_target: String,
    pub findings: Vec<String>,
}

fn _detect_windows_path(path: &str) -> bool {
    if path.len() < 2 {
        return false;
    }
    if path.chars().nth(1) == Some(':') {
        return true;
    }
    if path.starts_with("\\\\") {
        return true;
    }
    if path.contains('\\') {
        return true;
    }
    false
}

fn _split_posix_components(path: &str) -> (Vec<&str>, Option<&str>) {
    if path.is_empty() {
        return (vec![], None);
    }

    if let Some(rest) = path.strip_prefix('/') {
        let root = "/";
        if rest.is_empty() {
            return (vec![], Some(root));
        }
        let parts: Vec<&str> = rest.split('/').filter(|p| !p.is_empty()).collect();
        return (parts, Some(root));
    }

    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    (parts, None)
}

fn _split_windows_components(path: &str) -> (Vec<&str>, Option<String>) {
    if path.is_empty() {
        return (vec![], None);
    }

    if path.len() >= 2 {
        let chars: Vec<char> = path.chars().collect();
        if chars[1] == ':' {
            let root = path[..2].to_string();
            let rest = &path[2..];
            if rest.is_empty() {
                return (vec![], Some(root));
            }
            let parts: Vec<&str> = rest.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
            return (parts, Some(root));
        }
    }

    if path.starts_with("\\\\") {
        let parts: Vec<&str> = path.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
        if parts.len() >= 3 {
            let root = format!("\\\\{}\\{}", parts[0], parts[1]);
            let components: Vec<&str> = parts[2..].to_vec();
            return (components, Some(root));
        } else {
            return (vec![], Some(path.to_string()));
        }
    }

    if path.contains('\\') {
        let parts: Vec<&str> = path.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
        return (parts, None);
    }

    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    (parts, None)
}

fn _get_suffixes(name: &str) -> Vec<String> {
    if name.is_empty() || name == "." {
        return vec![];
    }

    let parts: Vec<&str> = name.split('.').collect();
    if parts.len() <= 1 {
        return vec![];
    }

    let mut suffixes = vec![];
    for i in 1..parts.len() {
        let suffix = format!(".{}", parts[i..].join("."));
        suffixes.push(suffix);
    }

    suffixes
}

pub fn path_analyze(path: &str, style: &str) -> PathAnalyzeResult {
    let mut warnings: Vec<String> = vec![];
    let input_path = path.to_string();

    let actual_style = if style == "auto" {
        if _detect_windows_path(path) {
            "windows".to_string()
        } else {
            "posix".to_string()
        }
    } else {
        style.to_string()
    };

    let (raw_components, root): (Vec<&str>, Option<String>) = if actual_style == "windows" {
        _split_windows_components(path)
    } else {
        let (comps, root_str) = _split_posix_components(path);
        (comps, root_str.map(|s| s.to_string()))
    };

    let sep = if actual_style == "windows" { "\\" } else { "/" };

    let mut components: Vec<&str> = vec![];
    let mut normalized_parts: Vec<&str> = vec![];

    for (i, comp) in raw_components.iter().enumerate() {
        if *comp == "." {
            warnings.push(format!(
                "Redundant current directory segment at position {}",
                i
            ));
            components.push(comp);
            normalized_parts.push(comp);
        } else if *comp == ".." {
            warnings.push(format!("Parent traversal segment at position {}", i));
            components.push(comp);
            normalized_parts.push(comp);
        } else {
            components.push(comp);
            normalized_parts.push(comp);
        }
    }

    let has_traversal = raw_components.contains(&"..");
    let absolute = root.is_some();

    let confusables = find_confusables(path);
    if !confusables.is_empty() {
        warnings.push(format!(
            "Path contains {} confusable character(s)",
            confusables.len()
        ));
    }

    let name = components.last().map(|s| s.to_string());

    let (suffixes, suffix, stem) = if let Some(ref name_str) = name {
        let suffs = _get_suffixes(name_str);
        let suff = suffs.last().cloned();
        let full_suff = suffs.first().cloned();
        let stm = if let Some(ref fs) = full_suff {
            if !fs.is_empty() {
                let name_len = name_str.len();
                let fs_len = fs.len();
                name_str[..name_len - fs_len].to_string()
            } else {
                name_str.to_string()
            }
        } else {
            name_str.to_string()
        };
        (suffs, suff, Some(stm))
    } else {
        (vec![], None, None)
    };

    let parent = if !components.is_empty() {
        let parent_parts = &components[..components.len() - 1];
        if !parent_parts.is_empty() {
            let joined = parent_parts.join(sep);
            if let Some(ref root_str) = root {
                if actual_style == "posix" {
                    Some(format!("{}{}", sep, joined))
                } else {
                    Some(format!("{}{}{}", root_str, sep, joined))
                }
            } else {
                Some(joined)
            }
        } else {
            None
        }
    } else {
        None
    };

    let mut hidden = false;
    if let Some(ref name_str) = name {
        if name_str != "." && name_str != ".." {
            hidden = name_str.starts_with('.');
        }
    }

    let mut normalized = normalized_parts.join(sep);
    if root.is_some() && actual_style == "posix" {
        normalized = format!("{}{}", sep, normalized);
    }

    let mut summary_parts = vec![];
    if actual_style != "auto" {
        summary_parts.push(actual_style.to_uppercase());
    }
    if absolute {
        summary_parts.push("absolute".to_string());
    } else {
        summary_parts.push("relative".to_string());
    }
    if hidden {
        summary_parts.push("hidden".to_string());
    }
    if has_traversal {
        summary_parts.push("with traversal".to_string());
    }
    if components.len() == 1 {
        summary_parts.push(format!("single component '{}'", components[0]));
    } else if !components.is_empty() {
        summary_parts.push(format!("{} components", components.len()));
    }
    if let Some(ref suff) = suffix {
        if suffixes.len() > 1 {
            summary_parts.push(format!("suffixes {:?}", suffixes));
        } else {
            summary_parts.push(format!("suffix '{}'", suff));
        }
    }

    let summary = if !summary_parts.is_empty() {
        summary_parts.join(", ")
    } else {
        "empty path".to_string()
    };

    PathAnalyzeResult {
        input: input_path,
        style: actual_style,
        absolute,
        has_traversal,
        components: components.into_iter().map(|s| s.to_string()).collect(),
        parent,
        name,
        stem,
        suffix,
        suffixes,
        hidden,
        normalized_lexical: normalized,
        warnings,
        summary,
    }
}

pub fn path_normalize(
    path: &str,
    platform: &str,
    collapse_dot_segments: bool,
    preserve_trailing_separator: bool,
) -> PathNormalizeResult {
    let mut warnings: Vec<String> = vec![];
    let mut has_dot_dot = false;
    let mut has_dot = false;
    let had_trailing_separator = path.ends_with('/') || path.ends_with('\\');

    let actual_platform = if platform != "posix" && platform != "windows" {
        "posix"
    } else {
        platform
    };

    let sep = if actual_platform == "posix" {
        "/"
    } else {
        "\\"
    };

    let mut components: Vec<&str> = vec![];
    let mut is_unc_track =
        actual_platform == "windows" && (path.starts_with("\\\\") || path.starts_with("//"));

    for part in path.split(sep) {
        if part.is_empty() {
            continue;
        }
        if part == "." {
            has_dot = true;
            if collapse_dot_segments {
                warnings.push("Collapsing dot segment".to_string());
                continue;
            } else {
                components.push(part);
                continue;
            }
        } else if part == ".." {
            has_dot_dot = true;
            if collapse_dot_segments {
                warnings.push("Collapsing dot-dot segment".to_string());
                if is_unc_track {
                    if !components.is_empty()
                        && components.last() != Some(&"")
                        && components.last() != Some(&"..")
                    {
                        if components.last() != Some(&"server") || components.len() == 1 {
                            components.pop();
                        } else {
                            components.push("..");
                        }
                    } else {
                        components.push("..");
                    }
                } else if !components.is_empty() && components.last() != Some(&"..") {
                    components.pop();
                } else {
                    components.push("..");
                }
            } else {
                components.push(part);
            }
            continue;
        } else if is_unc_track && (part == "server" || part == "share") {
            if components.len() >= 2 {
                is_unc_track = false;
            }
            components.push(part);
        } else if !part.is_empty() && part != "." && part != ".." {
            components.push(part);
        }
    }

    if preserve_trailing_separator && had_trailing_separator && !components.is_empty() {
        components.push("");
    }

    let mut normalized = if components.is_empty() {
        String::new()
    } else {
        components.join(sep)
    };

    if actual_platform == "posix" && path.starts_with('/') && !normalized.starts_with('/') {
        normalized = format!("/{}", normalized);
    } else if actual_platform == "windows" {
        if is_unc_track {
            normalized = format!("\\\\{}", normalized);
        } else if path.len() >= 2 && path.chars().nth(1) == Some(':') {
            normalized = format!("{}{}", &path[..2], normalized);
        }
    }

    if normalized.is_empty() {
        if actual_platform == "posix" && path.starts_with('/') {
            normalized = "/".to_string();
        } else if actual_platform == "windows" && is_unc_track {
            normalized = "\\\\".to_string();
        }
    }

    let is_absolute = if actual_platform == "posix" {
        path.starts_with('/')
    } else {
        (path.len() >= 2 && path.chars().nth(1) == Some(':')) || is_unc_track
    };

    if has_dot && !collapse_dot_segments {
        warnings.push("Path contains dot segments".to_string());
    }
    if has_dot_dot && !collapse_dot_segments {
        warnings.push("Path contains parent traversal segments".to_string());
    }

    PathNormalizeResult {
        normalized,
        is_absolute,
        components: components.into_iter().map(|s| s.to_string()).collect(),
        warnings,
    }
}

fn _normalize_path_for_compare(
    path: &str,
    platform: &str,
    collapse_dot_segments: bool,
    normalize_separators: bool,
) -> String {
    let mut result = path.to_string();
    if normalize_separators {
        if platform == "posix" {
            result = result.replace('\\', "/");
        } else {
            result = result.replace('/', "\\");
        }
    }
    let norm_result = path_normalize(&result, platform, collapse_dot_segments, false);
    norm_result.normalized
}

pub fn path_compare(
    left: &str,
    right: &str,
    platform: &str,
    case_sensitive: bool,
    normalize_separators: bool,
    collapse_dot_segments: bool,
) -> PathCompareResult {
    let mut findings: Vec<String> = vec![];

    let actual_platform = if platform != "posix" && platform != "windows" {
        "posix"
    } else {
        platform
    };

    let left_normalized = _normalize_path_for_compare(
        left,
        actual_platform,
        collapse_dot_segments,
        normalize_separators,
    );
    let right_normalized = _normalize_path_for_compare(
        right,
        actual_platform,
        collapse_dot_segments,
        normalize_separators,
    );

    let mut left_cmp = left_normalized.clone();
    let mut right_cmp = right_normalized.clone();

    if !case_sensitive {
        left_cmp = left_cmp.to_lowercase();
        right_cmp = right_cmp.to_lowercase();
    }

    let equal = left_cmp == right_cmp;

    let mut differences: Vec<String> = vec![];
    if !equal {
        differences.push(format!(
            "Normalized forms differ: '{}' vs '{}'",
            left_normalized, right_normalized
        ));
    }

    if !case_sensitive {
        findings.push("Case-insensitive comparison used".to_string());
    }
    if normalize_separators {
        findings.push("Separators normalized to platform default".to_string());
    }
    if collapse_dot_segments {
        findings.push("Dot segments collapsed".to_string());
    }

    PathCompareResult {
        equal,
        left_normalized,
        right_normalized,
        differences,
        findings,
    }
}

pub fn path_scope_check(
    root: &str,
    target: &str,
    platform: &str,
    case_sensitive: bool,
) -> PathScopeCheckResult {
    let mut findings: Vec<String> = vec![];

    let actual_platform = if platform != "posix" && platform != "windows" {
        "posix"
    } else {
        platform
    };

    fn pre_normalize(p: &str, platform: &str) -> String {
        if platform == "windows" {
            p.replace('/', "\\")
        } else {
            p.replace('\\', "/")
        }
    }

    let root_pre = pre_normalize(root, actual_platform);
    let target_pre = pre_normalize(target, actual_platform);

    let root_norm = path_normalize(&root_pre, actual_platform, true, false);
    let target_norm = path_normalize(&target_pre, actual_platform, true, false);

    let root_normalized = root_norm.normalized;
    let target_normalized = target_norm.normalized;

    let root_is_abs = root_norm.is_absolute;
    let target_is_abs = target_norm.is_absolute;

    if target_is_abs && !root_is_abs {
        findings.push("Target is absolute but root is relative".to_string());
    }

    let mut absolute_target = target_normalized.clone();
    if !target_is_abs {
        if actual_platform == "posix" {
            let stripped_root = root_normalized.trim_end_matches('/');
            absolute_target = format!("{}/{}", stripped_root, target_normalized);
        } else {
            let stripped_root = root_normalized.trim_end_matches('\\');
            absolute_target = format!("{}\\{}", stripped_root, target_normalized);
        }
        let abs_norm = path_normalize(&absolute_target, actual_platform, true, false);
        absolute_target = abs_norm.normalized;
    }

    let mut root_cmp = root_normalized.clone();
    let mut target_cmp = absolute_target.clone();
    if !case_sensitive {
        root_cmp = root_cmp.to_lowercase();
        target_cmp = target_cmp.to_lowercase();
    }

    let root_prefix = if actual_platform == "posix" {
        format!("{}/", root_cmp.trim_end_matches('/'))
    } else {
        format!("{}\\", root_cmp.trim_end_matches('\\'))
    };

    let inside_root = target_cmp.starts_with(&root_prefix) || target_cmp == root_cmp;

    let escapes_via_dotdot = target.split(['/', '\\']).any(|seg| seg == "..");

    let mut relative_path = String::new();
    if inside_root {
        relative_path = target_cmp
            .get(root_prefix.len()..)
            .unwrap_or("")
            .to_string();
        if relative_path.is_empty() {
            relative_path = ".".to_string();
        }
    }

    if !case_sensitive {
        findings.push("Case-insensitive comparison used".to_string());
    }
    if escapes_via_dotdot {
        findings.push("Target path contains parent traversal segments".to_string());
    }
    if !target_is_abs {
        findings.push("Target is relative, resolved against root".to_string());
    }

    PathScopeCheckResult {
        inside_root,
        root_normalized,
        target_normalized,
        relative_path,
        escapes_via_dotdot,
        absolute_target,
        findings,
    }
}
