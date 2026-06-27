use regex::Regex;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::LazyLock;

const _DEFAULT_KEY_PATTERN: &str = r"^[A-Za-z_][A-Za-z0-9_]*$";

static EXPANSION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$\{[^}]*\}|\$[A-Za-z_][A-Za-z0-9_]*").unwrap());

#[derive(Debug, Clone, Serialize)]
pub struct DotenvEntry {
    pub key: String,
    pub value: String,
    pub value_present: bool,
    pub quote_style: String,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct IniKeyValueEntry {
    pub section: Option<String>,
    pub key: String,
    pub value: String,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize)]
pub enum IniLineKind {
    Section { name: String },
    KeyValue(IniKeyValueEntry),
}

#[derive(Debug, Serialize)]
pub struct DuplicateEntry {
    pub key: String,
    pub first_line: usize,
    pub second_line: usize,
    pub section: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct InvalidLine {
    pub line: usize,
    pub text: String,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct DotenvValidateResult {
    pub parse_ok: bool,
    pub entries: Vec<DotenvEntry>,
    pub duplicates: Vec<DuplicateEntry>,
    pub invalid_lines: Vec<InvalidLine>,
    pub requires_quoting: Vec<String>,
    pub contains_expansion_syntax: Vec<String>,
    pub findings: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct IniValidateResult {
    pub parse_ok: bool,
    pub sections: Vec<String>,
    pub keys_by_section: HashMap<String, Vec<String>>,
    pub duplicates: Vec<DuplicateEntry>,
    pub invalid_lines: Vec<InvalidLine>,
    pub findings: Vec<String>,
}

pub fn dotenv_validate(
    text: &str,
    allow_export: bool,
    key_pattern: &str,
    duplicate_policy: &str,
) -> DotenvValidateResult {
    let key_re = match Regex::new(key_pattern) {
        Ok(re) => re,
        Err(_) => {
            return DotenvValidateResult {
                parse_ok: false,
                entries: vec![],
                duplicates: vec![],
                invalid_lines: vec![],
                requires_quoting: vec![],
                contains_expansion_syntax: vec![],
                findings: vec![format!("Invalid key_pattern regex: {}", key_pattern)],
            };
        }
    };
    let mut seen_keys: HashMap<String, usize> = HashMap::new();
    let mut entries: Vec<DotenvEntry> = Vec::new();
    let mut duplicates: Vec<DuplicateEntry> = Vec::new();
    let mut invalid_lines: Vec<InvalidLine> = Vec::new();
    let mut requires_quoting: Vec<String> = Vec::new();
    let mut contains_expansion: Vec<String> = Vec::new();
    let mut findings: Vec<String> = Vec::new();
    let mut parse_ok = true;

    for (line_no, raw_line) in text.split('\n').enumerate() {
        let line_no = line_no + 1;
        let stripped = raw_line.trim();

        if stripped.is_empty() || stripped.starts_with('#') {
            continue;
        }

        let mut line = stripped.to_string();

        if allow_export && line.starts_with("export ") {
            line = line[7..].to_string();
        } else if line.starts_with("export ") {
            invalid_lines.push(InvalidLine {
                line: line_no,
                text: raw_line.to_string(),
                reason: "export keyword not allowed".to_string(),
            });
            parse_ok = false;
            continue;
        }

        let eq_pos = line.find('=');
        if eq_pos.map(|p| p < 1).unwrap_or(true) {
            invalid_lines.push(InvalidLine {
                line: line_no,
                text: raw_line.to_string(),
                reason: "missing '=' separator".to_string(),
            });
            parse_ok = false;
            continue;
        }

        let eq_pos = eq_pos.unwrap();
        let key = line[..eq_pos].trim().to_string();
        let raw_value = line[eq_pos + 1..].to_string();

        if !key_re.is_match(&key) {
            invalid_lines.push(InvalidLine {
                line: line_no,
                text: raw_line.to_string(),
                reason: format!("key '{key}' does not match pattern {key_pattern}"),
            });
            parse_ok = false;
            continue;
        }

        let quote_style;
        let mut value = raw_value.trim().to_string();

        if value.len() >= 2
            && (value.starts_with('\'') || value.starts_with('"'))
            && value.chars().next() == value.chars().last()
        {
            let first_char = value.chars().next().unwrap();
            quote_style = first_char.to_string();
            value = value[1..value.len() - 1].to_string();
        } else {
            quote_style = "none".to_string();
            if let Some(hash_pos) = value.find('#') {
                value = value[..hash_pos].trim_end().to_string();
            }
            if value.contains(' ') && !value.starts_with('{') && !value.starts_with('[') {
                requires_quoting.push(key.clone());
            }
        }

        let value_present = value != "" && value != "''" && value != "\"\"";

        if EXPANSION_RE.is_match(&raw_value) {
            contains_expansion.push(key.clone());
        }

        let entry_key = key.clone();
        let entry = DotenvEntry {
            key: entry_key.clone(),
            value,
            value_present,
            quote_style,
            line: line_no,
        };
        entries.push(entry);

        if let Some(&first_line) = seen_keys.get(&entry_key) {
            let dup = DuplicateEntry {
                key: entry_key.clone(),
                first_line,
                second_line: line_no,
                section: None,
            };
            duplicates.push(dup);
            if duplicate_policy == "error" {
                parse_ok = false;
                findings.push(format!(
                    "Duplicate key '{}' at line {} (first at line {})",
                    entry_key, line_no, first_line
                ));
            } else if duplicate_policy == "warn" {
                findings.push(format!(
                    "Duplicate key '{}' at line {} (first at line {})",
                    entry_key, line_no, first_line
                ));
            }
        } else {
            seen_keys.insert(entry_key, line_no);
        }
    }

    if entries.is_empty() && invalid_lines.is_empty() {
        findings.push("No entries found".to_string());
    }

    DotenvValidateResult {
        parse_ok,
        entries,
        duplicates,
        invalid_lines,
        requires_quoting,
        contains_expansion_syntax: contains_expansion,
        findings,
    }
}

pub fn ini_validate(text: &str, duplicate_policy: &str) -> IniValidateResult {
    let mut seen_keys: HashMap<(Option<String>, String), usize> = HashMap::new();
    let mut seen_sections: HashMap<String, usize> = HashMap::new();
    let mut sections: Vec<String> = Vec::new();
    let mut keys_by_section: HashMap<String, Vec<String>> = HashMap::new();
    let mut duplicates: Vec<DuplicateEntry> = Vec::new();
    let mut invalid_lines: Vec<InvalidLine> = Vec::new();
    let mut findings: Vec<String> = Vec::new();
    let mut parse_ok = true;
    let mut current_section: Option<String> = None;

    let key_value_re = Regex::new(r"^([^=:\s]+)\s*[=:]\s*(.*)").unwrap();

    for (line_no, raw_line) in text.split('\n').enumerate() {
        let line_no = line_no + 1;
        let stripped = raw_line.trim();

        if stripped.is_empty() || stripped.starts_with('#') || stripped.starts_with(';') {
            continue;
        }

        if stripped.starts_with('[') && stripped.ends_with(']') {
            let section_name = stripped[1..stripped.len() - 1].trim().to_string();
            if section_name.is_empty() {
                invalid_lines.push(InvalidLine {
                    line: line_no,
                    text: raw_line.to_string(),
                    reason: "empty section name".to_string(),
                });
                parse_ok = false;
                continue;
            }

            if let Some(&first_line) = seen_sections.get(&section_name) {
                let dup = DuplicateEntry {
                    key: format!("[{section_name}]"),
                    first_line,
                    second_line: line_no,
                    section: Some(section_name.clone()),
                };
                duplicates.push(dup);
                if duplicate_policy == "error" {
                    parse_ok = false;
                    findings.push(format!(
                        "Duplicate section '{section_name}' at line {} (first at line {})",
                        line_no, first_line
                    ));
                } else if duplicate_policy == "warn" {
                    findings.push(format!(
                        "Duplicate section '{section_name}' at line {} (first at line {})",
                        line_no, first_line
                    ));
                }
            } else {
                seen_sections.insert(section_name.clone(), line_no);
            }

            current_section = Some(section_name.clone());
            if !sections.contains(&section_name) {
                sections.push(section_name.clone());
            }
            if !keys_by_section.contains_key(&section_name) {
                keys_by_section.insert(section_name, Vec::new());
            }
            continue;
        }

        if let Some(caps) = key_value_re.captures(stripped) {
            let key = caps.get(1).unwrap().as_str().trim().to_string();

            let section_key = (current_section.clone(), key.clone());
            let section_label = current_section
                .clone()
                .unwrap_or_else(|| "(top-level)".to_string());

            if let Some(&first_line) = seen_keys.get(&section_key) {
                let dup = DuplicateEntry {
                    key: key.clone(),
                    first_line,
                    second_line: line_no,
                    section: Some(section_label.clone()),
                };
                duplicates.push(dup);
                if duplicate_policy == "error" {
                    parse_ok = false;
                    findings.push(format!(
                        "Duplicate key '{key}' in section '{section_label}' at line {line_no} (first at line {first_line})"
                    ));
                } else if duplicate_policy == "warn" {
                    findings.push(format!(
                        "Duplicate key '{key}' in section '{section_label}' at line {line_no} (first at line {first_line})"
                    ));
                }
            } else {
                seen_keys.insert(section_key, line_no);
            }

            if let Some(ref section) = current_section {
                keys_by_section
                    .entry(section.clone())
                    .or_insert_with(Vec::new)
                    .push(key);
            } else {
                keys_by_section
                    .entry("(top-level)".to_string())
                    .or_insert_with(Vec::new)
                    .push(key);
            }
        } else {
            invalid_lines.push(InvalidLine {
                line: line_no,
                text: raw_line.to_string(),
                reason: "not a valid key=value line or section header".to_string(),
            });
            parse_ok = false;
        }
    }

    if sections.is_empty()
        && keys_by_section
            .get("(top-level)")
            .map(|v| v.is_empty())
            .unwrap_or(true)
        && invalid_lines.is_empty()
    {
        findings.push("No sections or keys found".to_string());
    }

    IniValidateResult {
        parse_ok,
        sections,
        keys_by_section,
        duplicates,
        invalid_lines,
        findings,
    }
}
