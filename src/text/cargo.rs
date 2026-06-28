use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::LazyLock;
use toml::Value;
use unicode_normalization::UnicodeNormalization;

const MAX_INPUT_LENGTH: usize = 200_000;

const CARGO_PACKAGE_FIELDS: [&str; 6] = [
    "name",
    "version",
    "edition",
    "license",
    "repository",
    "readme",
];

const EDITION_VALUES: [&str; 4] = ["2015", "2018", "2021", "2024"];

static SUSPICIOUS_NAME_PATTERNS: LazyLock<Vec<fancy_regex::Regex>> = LazyLock::new(|| {
    vec![
        fancy_regex::Regex::new(r"^\d").unwrap(),
        fancy_regex::Regex::new(r"[^a-zA-Z0-9_\-]").unwrap(),
        fancy_regex::Regex::new(r"_{2,}").unwrap(),
        fancy_regex::Regex::new(r"--").unwrap(),
        fancy_regex::Regex::new(r"\.").unwrap(),
        fancy_regex::Regex::new(r"[A-Z]").unwrap(),
    ]
});

fn detect_suspicious_name(name: &str) -> bool {
    for pat in SUSPICIOUS_NAME_PATTERNS.iter() {
        if pat.is_match(name).unwrap_or(false) {
            return true;
        }
    }
    false
}

fn normalize_ident(name: &str) -> String {
    let normalized: String = name.nfkc().collect();
    let casefolded = normalized.to_lowercase();
    regex::Regex::new(r"[\-_.]+")
        .unwrap()
        .replace_all(&casefolded, "_")
        .to_string()
}

fn detect_duplicates(names: &[String]) -> Vec<String> {
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    for name in names {
        let key = normalize_ident(name);
        groups.entry(key).or_default().push(name.clone());
    }
    let mut dupes: Vec<String> = Vec::new();
    for (_, group) in groups.iter() {
        if group.len() > 1 {
            let mut sorted: Vec<String> = group.to_vec();
            sorted.sort();
            dupes.extend(sorted);
        }
    }
    dupes.sort();
    dupes.dedup();
    dupes
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PackageInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edition: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readme: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceInfo {
    pub present: bool,
    pub members: Vec<String>,
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DependencyForm {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git: Option<String>,
    pub workspace: bool,
    pub inline_table: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub features: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optional: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_features: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DependencySection {
    pub dependencies: HashMap<String, DependencyForm>,
    pub dev_dependencies: HashMap<String, DependencyForm>,
    pub build_dependencies: HashMap<String, DependencyForm>,
    pub target_specific: HashMap<String, HashMap<String, DependencyForm>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CargoInspectResult {
    pub parse_ok: bool,
    pub package: PackageInfo,
    pub workspace: WorkspaceInfo,
    pub dependencies: DependencySection,
    pub path_dependencies: Vec<String>,
    pub suspicious_dependency_names: Vec<String>,
    pub duplicate_or_confusable_dependency_names: Vec<String>,
    pub findings: Vec<String>,
}

fn parse_dep_value(raw: &toml::Value) -> DependencyForm {
    if let Value::Table(table) = raw {
        let mut form = DependencyForm {
            inline_table: true,
            ..Default::default()
        };

        if let Some(v) = table.get("version") {
            form.version = Some(v.as_str().unwrap_or("").to_string());
        }
        if let Some(v) = table.get("path") {
            form.path = Some(v.as_str().unwrap_or("").to_string());
        }
        if let Some(v) = table.get("git") {
            form.git = Some(v.as_str().unwrap_or("").to_string());
        }
        if let Some(v) = table.get("branch") {
            form.branch = Some(v.as_str().unwrap_or("").to_string());
        }
        if let Some(v) = table.get("tag") {
            form.tag = Some(v.as_str().unwrap_or("").to_string());
        }
        if let Some(v) = table.get("registry") {
            form.registry = Some(v.as_str().unwrap_or("").to_string());
        }
        if let Some(v) = table.get("workspace") {
            form.workspace = v.as_bool().unwrap_or(false);
        }
        if let Some(features) = table.get("features").and_then(|f| f.as_array()) {
            form.features = Some(
                features
                    .iter()
                    .map(|f| f.as_str().unwrap_or("").to_string())
                    .collect(),
            );
        }
        if let Some(v) = table.get("optional") {
            form.optional = v.as_bool();
        }
        if let Some(v) = table.get("default-features") {
            form.default_features = v.as_bool();
        }
        form
    } else {
        DependencyForm {
            version: Some(raw.as_str().unwrap_or("").to_string()),
            inline_table: false,
            workspace: false,
            ..Default::default()
        }
    }
}

pub fn cargo_toml_inspect(
    text: &str,
    check_workspace: bool,
    check_dependencies: bool,
) -> CargoInspectResult {
    if text.len() > MAX_INPUT_LENGTH {
        return CargoInspectResult {
            parse_ok: false,
            package: PackageInfo::default(),
            workspace: WorkspaceInfo::default(),
            dependencies: DependencySection::default(),
            path_dependencies: Vec::new(),
            suspicious_dependency_names: Vec::new(),
            duplicate_or_confusable_dependency_names: Vec::new(),
            findings: vec![format!(
                "Input length {} exceeds maximum {}",
                text.len(),
                MAX_INPUT_LENGTH
            )],
        };
    }

    let parsed: Value = match text.parse() {
        Ok(v) => v,
        Err(e) => {
            return CargoInspectResult {
                parse_ok: false,
                package: PackageInfo::default(),
                workspace: WorkspaceInfo::default(),
                dependencies: DependencySection::default(),
                path_dependencies: Vec::new(),
                suspicious_dependency_names: Vec::new(),
                duplicate_or_confusable_dependency_names: Vec::new(),
                findings: vec![format!("TOML parse error: {}", e)],
            };
        }
    };

    let mut findings: Vec<String> = Vec::new();

    let pkg_raw = parsed.get("package");
    let mut pkg_raw_table: Option<&toml::map::Map<String, Value>> = None;
    if let Some(Value::Table(table)) = pkg_raw {
        pkg_raw_table = Some(table);
    } else if pkg_raw.is_some() {
        findings.push("'[package]' section is not a table".to_string());
    }

    let mut package = PackageInfo::default();
    if let Some(table) = pkg_raw_table {
        for field in CARGO_PACKAGE_FIELDS.iter() {
            if let Some(val) = table.get(*field) {
                let str_val = val.as_str().unwrap_or("").to_string();
                match *field {
                    "name" => {
                        package.name = Some(str_val);
                    }
                    "version" => {
                        package.version = Some(str_val);
                    }
                    "edition" => {
                        package.edition = Some(str_val);
                    }
                    "license" => {
                        package.license = Some(str_val);
                    }
                    "repository" => {
                        package.repository = Some(str_val);
                    }
                    "readme" => {
                        package.readme = Some(str_val);
                    }
                    _ => {}
                }
            }
        }
    }

    if package.name.is_none() || package.name.as_ref().is_none_or(|s| s.is_empty()) {
        findings.push("Missing or empty 'name' in [package]".to_string());
    }
    if package.version.is_none() || package.version.as_ref().is_none_or(|s| s.is_empty()) {
        findings.push("Missing or empty 'version' in [package]".to_string());
    }
    if package.edition.is_none() {
        findings.push(
            "Missing 'edition' in [package] (inherits workspace edition or defaults to 2015)"
                .to_string(),
        );
    } else if let Some(ref ed) = package.edition {
        if !EDITION_VALUES.contains(&ed.as_str()) {
            let mut sorted_editions = EDITION_VALUES.to_vec();
            sorted_editions.sort();
            findings.push(format!(
                "Unrecognized edition '{}'; expected one of: {}",
                ed,
                sorted_editions.join(", ")
            ));
        }
    }

    let mut workspace = WorkspaceInfo::default();
    if check_workspace {
        if let Some(ws_raw) = parsed.get("workspace") {
            workspace.present = true;
            if let Value::Table(table) = ws_raw {
                if let Some(members) = table.get("members").and_then(|m| m.as_array()) {
                    workspace.members = members
                        .iter()
                        .map(|v| v.as_str().unwrap_or("").to_string())
                        .collect();
                }
                if let Some(exclude) = table.get("exclude").and_then(|e| e.as_array()) {
                    workspace.exclude = exclude
                        .iter()
                        .map(|v| v.as_str().unwrap_or("").to_string())
                        .collect();
                }
            } else {
                findings.push("'[workspace]' is not a table".to_string());
            }
        }
    }

    let mut dep_section = DependencySection::default();
    let mut all_dep_names: Vec<String> = Vec::new();
    let mut path_deps: Vec<String> = Vec::new();

    if check_dependencies {
        let dep_tables = [
            ("dependencies", "dependencies"),
            ("dev-dependencies", "dev_dependencies"),
            ("build-dependencies", "build_dependencies"),
        ];

        for (table_key, section_key) in dep_tables.iter() {
            if let Some(raw_deps) = parsed.get(*table_key) {
                if let Value::Table(table) = raw_deps {
                    let mut parsed_deps: HashMap<String, DependencyForm> = HashMap::new();
                    for (dep_name, dep_value) in table.iter() {
                        let form = parse_dep_value(dep_value);
                        parsed_deps.insert(dep_name.clone(), form.clone());
                        all_dep_names.push(dep_name.clone());

                        if let Some(path) = &form.path {
                            path_deps.push(path.clone());
                        }

                        if form.git.is_some() && detect_suspicious_name(dep_name) {
                            findings.push(format!(
                                "Git dependency '{}' has suspicious name pattern",
                                dep_name
                            ));
                        }
                    }

                    match *section_key {
                        "dependencies" => dep_section.dependencies = parsed_deps,
                        "dev_dependencies" => dep_section.dev_dependencies = parsed_deps,
                        "build_dependencies" => dep_section.build_dependencies = parsed_deps,
                        _ => {}
                    }
                } else {
                    findings.push(format!("'[{}]' is not a table", table_key));
                }
            }
        }

        if let Some(target_section) = parsed.get("target").and_then(|t| t.as_table()) {
            for (target_key, target_val) in target_section.iter() {
                if let Value::Table(target_table) = target_val {
                    let mut target_deps: HashMap<String, DependencyForm> = HashMap::new();

                    for dep_table_key in
                        ["dependencies", "dev-dependencies", "build-dependencies"].iter()
                    {
                        if let Some(raw_deps) =
                            target_table.get(*dep_table_key).and_then(|d| d.as_table())
                        {
                            for (dep_name, dep_value) in raw_deps.iter() {
                                let form = parse_dep_value(dep_value);
                                target_deps.insert(dep_name.clone(), form.clone());
                                all_dep_names.push(dep_name.clone());

                                if let Some(path) = &form.path {
                                    path_deps.push(path.clone());
                                }
                            }
                        }
                    }

                    if !target_deps.is_empty() {
                        dep_section
                            .target_specific
                            .insert(target_key.clone(), target_deps);
                    }
                }
            }
        }
    }

    let mut suspicious: Vec<String> = all_dep_names
        .iter()
        .filter(|name| detect_suspicious_name(name))
        .cloned()
        .collect();
    suspicious.sort();
    suspicious.dedup();

    let dupes = detect_duplicates(&all_dep_names);
    if !dupes.is_empty() {
        findings.push(format!(
            "Confusable dependency names detected: {}",
            dupes.join(", ")
        ));
    }

    CargoInspectResult {
        parse_ok: true,
        package,
        workspace,
        dependencies: dep_section,
        path_dependencies: path_deps,
        suspicious_dependency_names: suspicious,
        duplicate_or_confusable_dependency_names: dupes,
        findings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_package() {
        let text = r#"
[package]
name = "test"
version = "1.0.0"
edition = "2021"
"#;
        let result = cargo_toml_inspect(text, true, true);
        assert!(result.parse_ok);
        assert_eq!(result.package.name, Some("test".to_string()));
        assert_eq!(result.package.version, Some("1.0.0".to_string()));
        assert_eq!(result.package.edition, Some("2021".to_string()));
    }

    #[test]
    fn test_missing_package_fields() {
        let text = r#"
[package]
name = "test"
"#;
        let result = cargo_toml_inspect(text, true, true);
        assert!(result.parse_ok);
        assert!(result
            .findings
            .contains(&"Missing or empty 'version' in [package]".to_string()));
        assert!(result.findings.contains(
            &"Missing 'edition' in [package] (inherits workspace edition or defaults to 2015)"
                .to_string()
        ));
    }

    #[test]
    fn test_invalid_edition() {
        let text = r#"
[package]
name = "test"
version = "1.0.0"
edition = "2030"
"#;
        let result = cargo_toml_inspect(text, true, true);
        assert!(result.parse_ok);
        assert!(result
            .findings
            .iter()
            .any(|f| f.contains("Unrecognized edition")));
    }

    #[test]
    fn test_workspace_members() {
        let text = r#"
[workspace]
members = ["crate1", "crate2"]
exclude = ["old_crate"]
"#;
        let result = cargo_toml_inspect(text, true, true);
        assert!(result.parse_ok);
        assert!(result.workspace.present);
        assert_eq!(result.workspace.members, vec!["crate1", "crate2"]);
        assert_eq!(result.workspace.exclude, vec!["old_crate"]);
    }

    #[test]
    fn test_dependencies_parsing() {
        let text = r#"
[package]
name = "test"
version = "1.0.0"
edition = "2021"

[dependencies]
serde = "1.0"
once_cell = { version = "1.0", features = ["full"] }
"#;
        let result = cargo_toml_inspect(text, true, true);
        assert!(result.parse_ok);
        assert_eq!(result.dependencies.dependencies.len(), 2);
        assert!(result.dependencies.dependencies.contains_key("serde"));
        assert!(result.dependencies.dependencies.contains_key("once_cell"));
    }

    #[test]
    fn test_path_dependencies() {
        let text = r#"
[package]
name = "test"
version = "1.0.0"
edition = "2021"

[dependencies]
local_crate = { path = "../local_crate" }
"#;
        let result = cargo_toml_inspect(text, true, true);
        assert!(result.parse_ok);
        assert_eq!(result.path_dependencies, vec!["../local_crate"]);
    }

    #[test]
    fn test_suspicious_dependency_name() {
        let text = r#"
[package]
name = "test"
version = "1.0.0"
edition = "2021"

[dependencies]
InvalidName = "1.0"
"#;
        let result = cargo_toml_inspect(text, true, true);
        assert!(result.parse_ok);
        assert!(result
            .suspicious_dependency_names
            .contains(&"InvalidName".to_string()));
    }

    #[test]
    fn test_confusable_dependencies() {
        let text = r#"
[package]
name = "test"
version = "1.0.0"
edition = "2021"

[dependencies]
foo-bar = "1.0"
foo_bar = "1.0"
"#;
        let result = cargo_toml_inspect(text, true, true);
        assert!(result.parse_ok);
        assert!(result
            .findings
            .iter()
            .any(|f| f.contains("Confusable dependency names")));
    }

    #[test]
    fn test_input_too_long() {
        let text = "a".repeat(250_000);
        let result = cargo_toml_inspect(&text, true, true);
        assert!(!result.parse_ok);
        assert!(result
            .findings
            .iter()
            .any(|f| f.contains("exceeds maximum")));
    }

    #[test]
    fn test_invalid_toml() {
        let text = "[invalid";
        let result = cargo_toml_inspect(text, true, true);
        assert!(!result.parse_ok);
        assert!(result
            .findings
            .iter()
            .any(|f| f.contains("TOML parse error")));
    }

    #[test]
    fn test_dev_and_build_dependencies() {
        let text = r#"
[package]
name = "test"
version = "1.0.0"
edition = "2021"

[dev-dependencies]
test_util = "1.0"

[build-dependencies]
build_script = "1.0"
"#;
        let result = cargo_toml_inspect(text, true, true);
        assert!(result.parse_ok);
        assert!(result
            .dependencies
            .dev_dependencies
            .contains_key("test_util"));
        assert!(result
            .dependencies
            .build_dependencies
            .contains_key("build_script"));
    }

    #[test]
    fn test_target_specific_dependencies() {
        let text = r#"
[package]
name = "test"
version = "1.0.0"
edition = "2021"

[target.'cfg(windows)'.dependencies]
winapi = "0.3"
"#;
        let result = cargo_toml_inspect(text, true, true);
        assert!(result.parse_ok);
        assert!(result
            .dependencies
            .target_specific
            .contains_key("cfg(windows)"));
    }
}
