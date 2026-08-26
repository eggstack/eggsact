use crate::text::confusables::find_confusables;

/// Windows-specific prefix classification for path analysis.
///
/// The lexical path tools have no per-drive current-working-directory state,
/// so a drive-relative target (`C:foo`) cannot be resolved lexically. The
/// classifier lets every path helper treat drive-relative paths
/// conservatively instead of silently concatenating them under an unrelated
/// root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowsPrefix<'a> {
    /// No special prefix (plain relative path).
    None,
    /// Drive-relative: `C:foo` — relative to the current directory on drive C.
    DriveRelative { drive: &'a str },
    /// Drive-rooted (absolute): `C:\foo` or `C:/foo`.
    DriveRooted { drive: &'a str },
    /// UNC path: `\\host\share` or `//host/share`.
    Unc {
        host: &'a str,
        share: Option<&'a str>,
    },
}

impl<'a> WindowsPrefix<'a> {
    /// Returns `true` if this prefix represents an absolute path.
    fn is_absolute(&self) -> bool {
        matches!(self, Self::DriveRooted { .. } | Self::Unc { .. })
    }
}

/// Classify the leading prefix of a Windows-style path.
fn _classify_windows_prefix(path: &str) -> WindowsPrefix<'_> {
    if path.is_empty() {
        return WindowsPrefix::None;
    }

    // UNC: \\host\share or //host/share
    if path.starts_with("\\\\") || path.starts_with("//") {
        let parts: Vec<&str> = path.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
        if parts.len() >= 2 {
            return WindowsPrefix::Unc {
                host: parts[0],
                share: Some(parts[1]),
            };
        } else if parts.len() == 1 {
            return WindowsPrefix::Unc {
                host: parts[0],
                share: None,
            };
        } else {
            return WindowsPrefix::Unc {
                host: "",
                share: None,
            };
        }
    }

    // Drive letter: X: or X:\ or X:/
    if let Some(first) = path.chars().next() {
        if first.is_ascii_alphabetic() {
            let second_byte = first.len_utf8();
            if second_byte < path.len() && path.as_bytes()[second_byte] == b':' {
                let next = second_byte + 1;
                if next < path.len() {
                    let sep = path.as_bytes()[next];
                    if sep == b'/' || sep == b'\\' {
                        return WindowsPrefix::DriveRooted {
                            drive: &path[..second_byte],
                        };
                    }
                }
                return WindowsPrefix::DriveRelative {
                    drive: &path[..second_byte],
                };
            }
        }
    }

    WindowsPrefix::None
}

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
    if let Some(first) = path.chars().next() {
        if first.is_ascii_alphabetic() {
            let second_byte = first.len_utf8();
            if second_byte < path.len() && path.as_bytes()[second_byte] == b':' {
                return true;
            }
        }
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

    if let Some(first) = path.chars().next() {
        if first.is_ascii_alphabetic() {
            let second_byte = first.len_utf8();
            if second_byte < path.len() && path.as_bytes()[second_byte] == b':' {
                let root_end = second_byte + 1; // include the ':'
                let root = path[..root_end].to_string();
                let rest = &path[root_end..];
                if rest.is_empty() {
                    return (vec![], Some(root));
                }
                let parts: Vec<&str> = rest.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
                return (parts, Some(root));
            }
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
    let absolute = if actual_style == "windows" {
        _classify_windows_prefix(path).is_absolute()
    } else {
        root.is_some()
    };

    let confusables = find_confusables(path);
    if !confusables.is_empty() {
        warnings.push(format!(
            "Path contains {} confusable character(s)",
            confusables.len()
        ));
    }

    // Classify Windows prefix to add drive-relative warnings.
    if actual_style == "windows" {
        let prefix = _classify_windows_prefix(path);
        if let WindowsPrefix::DriveRelative { drive } = prefix {
            warnings.push(format!(
                "Drive-relative path on drive {}; \
                 cannot be resolved lexically without the current directory on drive {}",
                drive, drive
            ));
        }
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

    // Detect UNC prefix structurally: \\host\share or //host/share
    // The prefix (host + share) is protected from dot-segment collapse.
    let (is_unc, unc_protected_count): (bool, usize) = if actual_platform == "windows" {
        if path.starts_with("\\\\") || path.starts_with("//") {
            let parts: Vec<&str> = path.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
            if parts.len() >= 2 {
                (true, 2)
            } else if parts.len() == 1 {
                (true, 1)
            } else {
                (false, 0)
            }
        } else {
            (false, 0)
        }
    } else {
        (false, 0)
    };

    let split_seps: &[char] = if actual_platform == "windows" {
        &['/', '\\']
    } else {
        &['/']
    };

    let mut components: Vec<&str> = vec![];
    let mut prefix_components: Vec<&str> = vec![];

    for part in path.split(split_seps) {
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
                // Pop the last collapsible component if available.
                // For UNC paths, prefix components (host/share) are already
                // separated into prefix_components and never appear in
                // components, so no special sentinel check is needed.
                // At the UNC share root (empty components), .. is clamped:
                // the share boundary cannot be escaped via normalization.
                // Scope checking (path_scope_check) detects the escape.
                if !components.is_empty() && components.last() != Some(&"..") {
                    components.pop();
                }
                // When components is empty at UNC root, .. is silently
                // clamped — not pushed to components.
            } else {
                components.push(part);
            }
            continue;
        }

        // For UNC paths, the first two non-empty parts after the
        // leading separators are host and share — they are protected.
        if is_unc && prefix_components.len() < unc_protected_count {
            prefix_components.push(part);
        } else {
            components.push(part);
        }
    }

    if preserve_trailing_separator && had_trailing_separator && !components.is_empty() {
        components.push("");
    }

    // Build the normalized result: prefix + collapsible components
    let mut normalized = if components.is_empty() {
        String::new()
    } else {
        components.join(sep)
    };

    if actual_platform == "posix" && path.starts_with('/') && !normalized.starts_with('/') {
        normalized = format!("/{}", normalized);
    } else if actual_platform == "windows" {
        if is_unc {
            // Rebuild UNC root from structural prefix, not from component text
            let host = prefix_components.first().unwrap_or(&"");
            let share = prefix_components.get(1).unwrap_or(&"");
            if prefix_components.len() >= 2 {
                if normalized.is_empty() {
                    normalized = format!("\\\\{}\\{}", host, share);
                } else {
                    normalized = format!("\\\\{}\\{}\\{}", host, share, normalized);
                }
            } else if !prefix_components.is_empty() {
                // Incomplete UNC (host only)
                let prefix_str = prefix_components.join(sep);
                normalized = format!("\\\\{}", prefix_str);
            } else {
                normalized = format!("\\\\{}", normalized);
            }
        } else if let Some(first) = path.chars().next() {
            if first.is_ascii_alphabetic() {
                let second_byte = first.len_utf8();
                if second_byte < path.len() && path.as_bytes()[second_byte] == b':' {
                    let drive = &path[..=second_byte];
                    let tail = normalized
                        .strip_prefix(drive)
                        .unwrap_or(normalized.as_str());
                    normalized = format!("{}{}", drive, tail);
                }
            }
        }
    }

    if normalized.is_empty() {
        if actual_platform == "posix" && path.starts_with('/') {
            normalized = "/".to_string();
        } else if actual_platform == "windows" && is_unc {
            normalized = "\\\\".to_string();
        }
    }

    let is_absolute = if actual_platform == "posix" {
        path.starts_with('/')
    } else {
        // Windows absolute: drive letter + separator (C:\foo), or UNC (\\server\share)
        let has_drive_root = if let Some(first) = path.chars().next() {
            if first.is_ascii_alphabetic() {
                let second_byte = first.len_utf8();
                second_byte < path.len()
                    && path.as_bytes()[second_byte] == b':'
                    && second_byte + 1 < path.len()
                    && matches!(path.as_bytes()[second_byte + 1], b'/' | b'\\')
            } else {
                false
            }
        } else {
            false
        };
        has_drive_root || is_unc
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

    // Classify the target's Windows prefix.  Drive-relative targets
    // (`C:foo`) cannot be resolved lexically — the result depends on
    // the caller's current directory on drive C, which this lexical API
    // does not model.
    let target_prefix = if actual_platform == "windows" {
        _classify_windows_prefix(&target_pre)
    } else {
        WindowsPrefix::None
    };

    if let WindowsPrefix::DriveRelative { drive } = target_prefix {
        findings.push(format!(
            "Drive-relative target '{}' cannot be resolved lexically; \
             the result depends on the current directory on drive {}",
            target_pre, drive
        ));
    }

    let root_norm = path_normalize(&root_pre, actual_platform, true, false);
    let target_norm = path_normalize(&target_pre, actual_platform, true, false);

    let root_normalized = root_norm.normalized;
    let target_normalized = target_norm.normalized;

    let root_is_abs = root_norm.is_absolute;
    let target_is_abs = target_norm.is_absolute;

    if target_is_abs && !root_is_abs {
        findings.push("Target is absolute but root is relative".to_string());
    }

    // Drive-relative targets are conservatively not inside the root.
    if matches!(target_prefix, WindowsPrefix::DriveRelative { .. }) {
        return PathScopeCheckResult {
            inside_root: false,
            root_normalized,
            target_normalized: target_normalized.clone(),
            relative_path: String::new(),
            escapes_via_dotdot: false,
            absolute_target: target_normalized,
            findings,
        };
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

    // Resolve .. segments in absolute_target to compute the effective path.
    // This correctly determines whether a path with .. escapes the root.
    let sep_char = if actual_platform == "windows" {
        '\\'
    } else {
        '/'
    };
    let resolved = resolve_dot_segments(&absolute_target, actual_platform, sep_char);

    let mut root_cmp = root_normalized.clone();
    let mut resolved_cmp = resolved.clone();
    if !case_sensitive {
        root_cmp = root_cmp.to_lowercase();
        resolved_cmp = resolved_cmp.to_lowercase();
    }

    let root_prefix = if actual_platform == "posix" {
        format!("{}/", root_cmp.trim_end_matches('/'))
    } else {
        format!("{}\\", root_cmp.trim_end_matches('\\'))
    };

    let escapes_via_dotdot = target.split(['/', '\\']).any(|seg| seg == "..");

    // For UNC paths, detect whether .. escapes above the share boundary.
    // path_normalize silently clamps .. at the share root, so we must
    // detect the escape from the raw target before normalization.
    let unc_escape_above_share = if actual_platform == "windows" {
        let target_for_check = if !target_is_abs && !absolute_target.is_empty() {
            absolute_target.as_str()
        } else {
            target_pre.as_str()
        };
        let is_unc_target =
            target_for_check.starts_with("\\\\") || target_for_check.starts_with("//");
        if is_unc_target && escapes_via_dotdot {
            let split: Vec<&str> = target_for_check
                .split(['/', '\\'])
                .filter(|s| !s.is_empty())
                .collect();
            // Track depth relative to the share root.
            // host and share are prefix components; depth starts at 0
            // for the share. Each component after share adds 1, each
            // .. subtracts 1. If depth < 0, we escaped above share.
            if split.len() >= 2 {
                let mut depth: i32 = 0;
                let mut escaped = false;
                for seg in &split[2..] {
                    if *seg == ".." {
                        depth -= 1;
                        if depth < 0 {
                            escaped = true;
                            break;
                        }
                    } else if *seg != "." {
                        depth += 1;
                    }
                }
                escaped
            } else {
                false
            }
        } else {
            false
        }
    } else {
        false
    };

    // For POSIX paths, detect whether .. escapes above the root.
    // path_normalize collapses .. silently, so we detect the escape by
    // analyzing the raw target's component structure before normalization.
    // For relative targets, we combine root + target and check if ..
    // depth goes below zero.
    let posix_escape_above_root = if actual_platform == "posix" && escapes_via_dotdot {
        // Resolve the raw target against root components.
        // For relative paths, root components are the base — popping
        // below them means escape.
        let raw_parts: Vec<&str> = target_pre.split('/').filter(|s| !s.is_empty()).collect();
        let root_components: Vec<&str> = if target_is_abs {
            Vec::new()
        } else {
            root_pre.split('/').filter(|s| !s.is_empty()).collect()
        };
        let root_len = root_components.len();
        let mut resolved = root_components;
        let mut escaped = false;
        for seg in &raw_parts {
            if *seg == ".." {
                if resolved.len() > root_len {
                    resolved.pop();
                } else if !resolved.is_empty() {
                    // Popping a root component = escape
                    resolved.pop();
                    escaped = true;
                    break;
                } else {
                    // Stack empty, can't pop = escape
                    escaped = true;
                    break;
                }
            } else if *seg != "." {
                resolved.push(seg);
            }
        }
        escaped
    } else {
        false
    };

    let mut inside_root = resolved_cmp.starts_with(&root_prefix) || resolved_cmp == root_cmp;

    // Reject escapes: POSIX .. above root, UNC share boundary escape,
    // or resolved path landing exactly on root via .. (clamped escape).
    if (escapes_via_dotdot && resolved_cmp == root_cmp)
        || unc_escape_above_share
        || posix_escape_above_root
    {
        inside_root = false;
    }

    let mut relative_path = String::new();
    if inside_root {
        relative_path = resolved_cmp
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

/// Resolve `.` and `..` segments in a path, producing the effective path
/// without traversal segments. Does not touch UNC prefix components.
fn resolve_dot_segments(path: &str, platform: &str, sep: char) -> String {
    let parts: Vec<&str> = path.split(sep).collect();
    let mut resolved: Vec<&str> = Vec::new();

    for part in &parts {
        if *part == "." || (*part).is_empty() {
            continue;
        } else if *part == ".." {
            // For UNC paths starting with \\, the first two non-empty
            // parts are host and share — never pop them.
            let is_unc =
                platform == "windows" && (path.starts_with("\\\\") || path.starts_with("//"));
            let protected = if is_unc { 2 } else { 0 };

            if resolved.len() > protected {
                resolved.pop();
            }
            // At the protected boundary, .. is silently absorbed
        } else {
            resolved.push(part);
        }
    }

    let mut result = resolved.join(&sep.to_string());
    if platform == "posix" && path.starts_with('/') {
        result = format!("/{}", result);
    } else if platform == "windows" && (path.starts_with("\\\\") || path.starts_with("//")) {
        // Rebuild UNC prefix
        if resolved.len() >= 2 {
            result = format!("\\\\{}\\{}", resolved[0], resolved[1]);
            if resolved.len() > 2 {
                result.push('\\');
                result.push_str(&resolved[2..].join(&sep.to_string()));
            }
        } else if resolved.len() == 1 {
            result = format!("\\\\{}", resolved[0]);
        } else {
            result = "\\\\".to_string();
        }
    }

    result
}
