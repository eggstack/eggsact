use crate::mcp::machine_codes;
use crate::mcp::schemas::{disposition, finding, severity, verdict, ToolResponse};
use crate::tools::helpers::*;
use regex::Regex;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

// ---------------------------------------------------------------------------
// Parser-backed TOML helpers
// ---------------------------------------------------------------------------

/// Extract dependency info from a TOML table of dependencies.
/// Returns Vec<(name, version, source_type)>.
fn toml_dep_table(deps: &toml::Value, _section: &str) -> Vec<(String, String, String)> {
    let mut result = Vec::new();
    let map = match deps.as_table() {
        Some(m) => m,
        None => return result,
    };
    for (name, val) in map {
        match val {
            // Simple string version: `name = "1.0"`
            toml::Value::String(s) => {
                result.push((name.clone(), s.clone(), "registry".to_string()));
            }
            // Inline table: `name = { version = "1", git = "...", ... }`
            toml::Value::Table(t) => {
                let version = t
                    .get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let source = if t.contains_key("git") {
                    "git".to_string()
                } else if t.contains_key("path") {
                    "path".to_string()
                } else if t.contains_key("registry") {
                    "registry".to_string()
                } else if t.get("workspace").and_then(|v| v.as_bool()) == Some(true) {
                    "workspace".to_string()
                } else {
                    "registry".to_string()
                };
                result.push((name.clone(), version, source));
            }
            _ => {}
        }
    }
    result
}

/// Parse Cargo.toml dependencies using the `toml` parser.
/// Returns `None` if the document is malformed (caller should fall back to heuristic).
fn parse_cargo_deps_toml(text: &str, section: &str) -> Option<Vec<(String, String, String)>> {
    let doc: toml::Value = toml::from_str(text).ok()?;

    let mut result = Vec::new();

    // Standard sections: [dependencies], [dev-dependencies], [build-dependencies]
    if let Some(deps) = doc.get(section) {
        result.extend(toml_dep_table(deps, section));
    }

    // Also check [section.name] sub-tables for inline table dependencies
    // that were declared as `name = { ... }` at the top level
    if let Some(table) = doc.as_table() {
        let prefix = format!("{}.", section);
        for (key, val) in table {
            if key.starts_with(&prefix) {
                // This is a sub-table like [dependencies.name]
                let dep_name = &key[prefix.len()..];
                // Check if this dep was already captured from the top-level table
                if !result.iter().any(|(n, _, _)| n == dep_name) {
                    // Extract version/source from sub-table
                    if let Some(sub) = val.as_table() {
                        let version = sub
                            .get("version")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let source = if sub.contains_key("git") {
                            "git".to_string()
                        } else if sub.contains_key("path") {
                            "path".to_string()
                        } else if sub.contains_key("registry") {
                            "registry".to_string()
                        } else if sub.get("workspace").and_then(|v| v.as_bool()) == Some(true) {
                            "workspace".to_string()
                        } else {
                            "registry".to_string()
                        };
                        result.push((dep_name.to_string(), version, source));
                    }
                }
            }
        }
    }

    // Target-specific dependencies: [target.'cfg(...)'.dependencies]
    if let Some(targets) = doc.get("target").and_then(|t| t.as_table()) {
        for (_cfg, target_cfg) in targets {
            if let Some(deps) = target_cfg.get(section) {
                result.extend(toml_dep_table(deps, section));
            }
        }
    }

    // Workspace dependencies: [workspace.dependencies]
    if section == "dependencies" {
        if let Some(ws_deps) = doc.get("workspace").and_then(|w| w.get("dependencies")) {
            for (name, val) in ws_deps.as_table()? {
                // Only add if not already present from [dependencies]
                if !result.iter().any(|(n, _, _)| n == name) {
                    match val {
                        toml::Value::String(s) => {
                            result.push((name.clone(), s.clone(), "registry".to_string()));
                        }
                        toml::Value::Table(t) => {
                            let version = t
                                .get("version")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let source = if t.contains_key("git") {
                                "git".to_string()
                            } else if t.contains_key("path") {
                                "path".to_string()
                            } else if t.get("workspace").and_then(|v| v.as_bool()) == Some(true) {
                                "workspace".to_string()
                            } else {
                                "registry".to_string()
                            };
                            result.push((name.clone(), version, source));
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    Some(result)
}

/// Check if a Cargo.toml has a build script using TOML parser.
fn has_cargo_build_script_toml(text: &str) -> Option<bool> {
    let doc: toml::Value = toml::from_str(text).ok()?;
    let package = doc.get("package")?;
    // Check [package].build = "..." or [package].build = true
    Some(package.get("build").is_some())
}

/// Check for [patch] or [replace] sections using TOML parser.
fn has_cargo_patch_section_toml(text: &str) -> Option<bool> {
    let doc: toml::Value = toml::from_str(text).ok()?;
    let table = doc.as_table()?;
    // Check for any key starting with "patch" or "replace"
    Some(table.keys().any(|k| {
        k == "patch" || k.starts_with("patch.") || k == "replace" || k.starts_with("replace.")
    }))
}

/// Detect pyproject.toml build backend changes.
fn detect_pyproject_build_backend_changes(old_text: &str, new_text: &str) -> Vec<Value> {
    let mut changes = Vec::new();
    let old_doc: Option<toml::Value> = toml::from_str(old_text).ok();
    let new_doc: Option<toml::Value> = toml::from_str(new_text).ok();

    if let (Some(old), Some(new)) = (old_doc, new_doc) {
        let old_backend = old
            .get("build-system")
            .and_then(|bs| bs.get("build-backend"))
            .and_then(|bb| bb.as_str());
        let new_backend = new
            .get("build-system")
            .and_then(|bs| bs.get("build-backend"))
            .and_then(|bb| bb.as_str());

        if old_backend != new_backend && new_backend.is_some() {
            changes.push(serde_json::json!({
                "type": "build_backend",
                "old": old_backend,
                "new": new_backend,
            }));
        }
    }
    changes
}

/// Parse pyproject.toml dependencies using the TOML parser.
fn parse_pyproject_deps_toml(text: &str) -> Option<Vec<String>> {
    let doc: toml::Value = toml::from_str(text).ok()?;
    let mut deps = Vec::new();

    // [project] dependencies = [...]
    if let Some(project) = doc.get("project") {
        if let Some(dep_list) = project.get("dependencies").and_then(|d| d.as_array()) {
            for item in dep_list {
                if let Some(s) = item.as_str() {
                    // Extract name before version specifier
                    if let Some(name) = s
                        .split(['>', '<', '=', '!', ';', '['])
                        .next()
                        .map(|n| n.trim().to_string())
                    {
                        if !name.is_empty() {
                            deps.push(name);
                        }
                    }
                }
            }
        }

        // [project.optional-dependencies] groups
        if let Some(opt_deps) = project
            .get("optional-dependencies")
            .and_then(|d| d.as_table())
        {
            for (_group, items) in opt_deps {
                if let Some(arr) = items.as_array() {
                    for item in arr {
                        if let Some(s) = item.as_str() {
                            if let Some(name) = s
                                .split(['>', '<', '=', '!', ';', '['])
                                .next()
                                .map(|n| n.trim().to_string())
                            {
                                if !name.is_empty() {
                                    deps.push(name);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // [build-system] requires = [...]
    if let Some(build_system) = doc.get("build-system") {
        if let Some(requires) = build_system.get("requires").and_then(|r| r.as_array()) {
            for item in requires {
                if let Some(s) = item.as_str() {
                    if let Some(name) = s
                        .split(['>', '<', '=', '!', ';', '['])
                        .next()
                        .map(|n| n.trim().to_string())
                    {
                        if !name.is_empty() {
                            deps.push(name);
                        }
                    }
                }
            }
        }
    }

    Some(deps)
}

/// Parse package.json dependencies using serde_json.
fn parse_package_json_deps_json(text: &str) -> Option<Vec<(String, String, String)>> {
    let doc: serde_json::Value = serde_json::from_str(text).ok()?;
    let mut result = Vec::new();

    let sections = [
        "dependencies",
        "devDependencies",
        "peerDependencies",
        "optionalDependencies",
    ];

    for section in &sections {
        if let Some(obj) = doc.get(*section).and_then(|d| d.as_object()) {
            for (name, val) in obj {
                let version = val.as_str().unwrap_or("").to_string();
                result.push((name.clone(), version, section.to_string()));
            }
        }
    }

    Some(result)
}

/// Parse package.json scripts using serde_json.
fn parse_package_json_scripts_json(text: &str) -> Option<Vec<(String, String)>> {
    let doc: serde_json::Value = serde_json::from_str(text).ok()?;
    let mut result = Vec::new();

    if let Some(scripts) = doc.get("scripts").and_then(|s| s.as_object()) {
        for (key, val) in scripts {
            if let Some(v) = val.as_str() {
                result.push((key.clone(), v.to_string()));
            }
        }
    }

    Some(result)
}

/// Detect ecosystem from file path and content.
fn detect_ecosystem(file_path: &str, text: &str) -> Option<&'static str> {
    let lower = file_path.to_lowercase();
    if lower.ends_with("cargo.toml") {
        Some("rust")
    } else if lower.ends_with("pyproject.toml")
        || lower.ends_with("requirements.txt")
        || lower.ends_with("setup.cfg")
        || lower.ends_with("setup.py")
    {
        Some("python")
    } else if lower.ends_with("package.json") || lower.ends_with("package-lock.json") {
        Some("node")
    } else {
        // Heuristic from content
        let trimmed = text.trim();
        if trimmed.contains("[package]") || trimmed.contains("[dependencies]") {
            Some("rust")
        } else if trimmed.contains("[project]") || trimmed.contains("[build-system]") {
            Some("python")
        } else if trimmed.starts_with('{') && trimmed.contains("\"dependencies\"") {
            Some("node")
        } else {
            None
        }
    }
}

/// Parse Cargo.toml dependency names from a specific section.
/// Returns (name, version_spec, source_type) tuples.
fn parse_cargo_deps(text: &str, section: &str) -> Vec<(String, String, String)> {
    let mut deps = Vec::new();
    let mut in_section = false;
    let section_header = format!("[{}]", section);
    let mut current_name = String::new();
    let mut current_version = String::new();
    let mut current_source = String::new();

    for line in text.lines() {
        let trimmed = line.trim();

        // Check if we're entering a new section
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            // Save any pending dep
            if in_section && !current_name.is_empty() {
                deps.push((
                    current_name.clone(),
                    current_version.clone(),
                    current_source.clone(),
                ));
                current_name.clear();
                current_version.clear();
                current_source.clear();
            }
            in_section =
                trimmed == section_header || trimmed.starts_with(&format!("[{}.", section));
            continue;
        }

        if !in_section {
            continue;
        }

        // Inline dependency: name = "version"
        if let Some(idx) = trimmed.find('=') {
            let key = trimmed[..idx].trim();
            let val = trimmed[idx + 1..].trim();

            if !key.starts_with('"') && !key.starts_with('#') {
                // This is a key = value at section level
                if !current_name.is_empty() {
                    deps.push((
                        current_name.clone(),
                        current_version.clone(),
                        current_source.clone(),
                    ));
                    current_name.clear();
                    current_version.clear();
                    current_source.clear();
                }

                let name = key.to_string();
                if val.starts_with('"') && val.ends_with('"') {
                    deps.push((
                        name,
                        val.trim_matches('"').to_string(),
                        "registry".to_string(),
                    ));
                } else if val.starts_with('{') {
                    // Table-style dependency on same line — skip for now, will be parsed below
                    current_name = name;
                }
            } else {
                // Sub-key of a table-style dependency
                let sub_key = key.trim_matches('"');
                let sub_val = val.trim_matches('"');
                match sub_key {
                    "version" => current_version = sub_val.to_string(),
                    "git" => current_source = "git".to_string(),
                    "path" => current_source = "path".to_string(),
                    _ => {}
                }
            }
        }

        // Table header within section: [dependencies.name]
        if trimmed.starts_with(&format!("[{}.", section)) && trimmed.ends_with(']') {
            // Save any pending dep
            if !current_name.is_empty() {
                deps.push((
                    current_name.clone(),
                    current_version.clone(),
                    current_source.clone(),
                ));
                current_name.clear();
                current_version.clear();
                current_source.clear();
            }
            let inner = &trimmed[section.len() + 3..trimmed.len() - 1];
            current_name = inner.to_string();
        }
    }

    // Save final pending dep
    if in_section && !current_name.is_empty() {
        deps.push((
            current_name.clone(),
            current_version.clone(),
            current_source.clone(),
        ));
    }

    deps
}

/// Check if a Cargo.toml has a build script.
fn has_cargo_build_script(text: &str) -> bool {
    // Check for [package] section and "build" key
    let mut in_package = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "[package]" {
            in_package = true;
            continue;
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_package = false;
            continue;
        }
        if in_package && trimmed.starts_with("build") && trimmed.contains('=') {
            return true;
        }
    }
    false
}

/// Check for [patch] or [replace] sections in Cargo.toml.
fn has_cargo_patch_section(text: &str) -> bool {
    for line in text.lines() {
        let trimmed = line.trim();
        if (trimmed.starts_with("[patch.")
            || trimmed == "[patch]"
            || trimmed.starts_with("[replace."))
            && trimmed.ends_with(']')
        {
            return true;
        }
    }
    false
}

/// Parse pyproject.toml dependency names from [project.dependencies] or [project.optional-dependencies].
fn parse_pyproject_deps(text: &str) -> Vec<String> {
    let mut deps = Vec::new();
    let mut in_deps = false;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "[project]" || trimmed.starts_with("[project.") {
            in_deps = trimmed == "[project.dependencies]"
                || trimmed.starts_with("[project.optional-dependencies]");
            continue;
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_deps = false;
            continue;
        }
        if in_deps && trimmed.starts_with('"') && trimmed.contains('=') {
            // Dependency entry like: "requests>=2.0",
            // Just extract the name before any version specifier
            if let Some(name) = trimmed.split(['>', '<', '=', '!']).next() {
                let name = name.trim_matches(|c: char| c == '"' || c == '\'' || c == ' ');
                if !name.is_empty() {
                    deps.push(name.to_string());
                }
            }
        }
    }
    deps
}

/// Parse requirements.txt dependency names.
/// Returns (name, spec) tuples. Improved to detect:
/// - direct URLs (http/https/file)
/// - editable installs (-e)
/// - local paths (relative/absolute)
/// - unconstrained specs (no version pin)
fn parse_requirements_deps(text: &str) -> Vec<(String, String)> {
    let mut deps = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Editable installs: -e package @ url or -e .
        if trimmed.starts_with("-e ") || trimmed.starts_with("--editable ") {
            let rest = trimmed
                .trim_start_matches("-e")
                .trim_start_matches("--editable")
                .trim()
                .trim_start_matches('=');
            deps.push(("editable".to_string(), format!("-e {}", rest)));
            continue;
        }

        // Constraints/includes flags: -c constraints.txt, -r requirements.txt
        if trimmed.starts_with("-c ") || trimmed.starts_with("-r ") || trimmed.starts_with("-f ") {
            deps.push(("reference".to_string(), trimmed.to_string()));
            continue;
        }

        // URL deps: name @ https://... or just https://...
        if trimmed.contains('@') && (trimmed.contains("://") || trimmed.contains("git+")) {
            deps.push(("url".to_string(), trimmed.to_string()));
            continue;
        }

        // Local path deps: ./path or /absolute/path or name @ ./path
        if trimmed.starts_with("./") || trimmed.starts_with("../") || trimmed.starts_with('/') {
            deps.push(("path".to_string(), trimmed.to_string()));
            continue;
        }

        // Standard dependency: name[extras]>=version or name==version
        let name = trimmed
            .split(['>', '<', '=', '!', '[', ';'])
            .next()
            .unwrap_or(trimmed)
            .trim();

        // Check if unconstrained (no version specifier at all)
        let has_version_spec = trimmed.contains('>')
            || trimmed.contains('<')
            || trimmed.contains('=')
            || trimmed.contains('!');
        if !name.is_empty() && !has_version_spec {
            deps.push((name.to_string(), format!("{} (unconstrained)", trimmed)));
        } else if !name.is_empty() {
            deps.push((name.to_string(), trimmed.to_string()));
        }
    }
    deps
}

/// Parse package.json dependencies.
fn parse_package_json_deps(text: &str) -> Vec<(String, String, String)> {
    let mut deps = Vec::new();
    // Simple JSON key extraction for "dependencies", "devDependencies", "peerDependencies"
    let sections = ["dependencies", "devDependencies", "peerDependencies"];

    for section in &sections {
        let mut in_section = false;
        let section_pattern = format!("\"{}\"", section);

        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.contains(&section_pattern) && trimmed.contains('{') {
                in_section = true;
                continue;
            }
            if in_section && trimmed == "}" {
                in_section = false;
                continue;
            }
            if in_section {
                // Pattern: "name": "version"
                if let Some(stripped) = trimmed.strip_suffix(',') {
                    let trimmed = stripped;
                    let parts: Vec<&str> = trimmed.splitn(2, ':').collect();
                    if parts.len() == 2 {
                        let name = parts[0].trim().trim_matches('"');
                        let version = parts[1].trim().trim_matches('"');
                        if !name.is_empty() && !version.is_empty() {
                            deps.push((name.to_string(), version.to_string(), section.to_string()));
                        }
                    }
                }
            }
        }
    }
    deps
}

/// Parse package.json scripts section.
fn parse_package_json_scripts(text: &str) -> Vec<(String, String)> {
    let mut scripts = Vec::new();
    let mut in_scripts = false;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.contains("\"scripts\"") && trimmed.contains('{') {
            in_scripts = true;
            continue;
        }
        if in_scripts && trimmed == "}" {
            break;
        }
        if in_scripts {
            if let Some(stripped) = trimmed.strip_suffix(',') {
                let trimmed = stripped;
                let parts: Vec<&str> = trimmed.splitn(2, ':').collect();
                if parts.len() == 2 {
                    let key = parts[0].trim().trim_matches('"');
                    let val = parts[1].trim().trim_matches('"');
                    if !key.is_empty() && !val.is_empty() {
                        scripts.push((key.to_string(), val.to_string()));
                    }
                }
            }
        }
    }
    scripts
}

/// Detect risky npm scripts (install hooks).
const NPM_RISKY_SCRIPTS: &[&str] = &[
    "install",
    "postinstall",
    "preinstall",
    "prepare",
    "uninstall",
];

/// Classify version constraint change.
fn classify_version_change(old_ver: &str, new_ver: &str) -> Option<&'static str> {
    if old_ver == new_ver {
        return None;
    }

    let old_is_exact = !old_ver.contains('^')
        && !old_ver.contains('~')
        && !old_ver.contains('*')
        && !old_ver.contains('>')
        && !old_ver.contains('<');
    let new_is_exact = !new_ver.contains('^')
        && !new_ver.contains('~')
        && !new_ver.contains('*')
        && !new_ver.contains('>')
        && !new_ver.contains('<');

    if (old_is_exact && !new_is_exact) || (old_ver.contains('^') && !new_ver.contains('^')) {
        Some("widened")
    } else {
        Some("changed")
    }
}

pub fn dependency_edit_preflight(args: &Value) -> ToolResponse {
    let budget_ctx = crate::mcp::budget::for_handler(crate::mcp::budget::ToolBudget::MODERATE);

    let file_path = match args.get("file_path").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'file_path' parameter",
                None,
                Some("dependency_edit_preflight"),
            )
        }
    };
    let old_text = match args.get("old_text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'old_text' parameter",
                None,
                Some("dependency_edit_preflight"),
            )
        }
    };
    let new_text = match args.get("new_text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'new_text' parameter",
                None,
                Some("dependency_edit_preflight"),
            )
        }
    };
    let ecosystem_arg = args
        .get("ecosystem")
        .and_then(|v| v.as_str())
        .unwrap_or("auto");
    let policy = args.get("policy");

    let allow_path = policy
        .and_then(|p| p.get("allow_path_deps"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let allow_git = policy
        .and_then(|p| p.get("allow_git_deps"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let allow_patch = policy
        .and_then(|p| p.get("allow_patch_sections"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if old_text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!("old_text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("dependency_edit_preflight"),
        );
    }
    if new_text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!("new_text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("dependency_edit_preflight"),
        );
    }

    let ecosystem = if ecosystem_arg == "auto" {
        match detect_ecosystem(file_path, new_text) {
            Some(e) => e,
            None => {
                // Unknown ecosystem — fail closed with a clear machine code
                // rather than silently defaulting to Rust and producing
                // misleading findings.
                return ToolResponse::error_with_code(
                    "unknown_ecosystem",
                    machine_codes::DEPENDENCY_UNKNOWN_ECOSYSTEM,
                    &format!(
                        "Could not detect ecosystem for '{}'. Pass an explicit ecosystem (rust, python, or node).",
                        file_path
                    ),
                    None,
                    Some("dependency_edit_preflight"),
                );
            }
        }
    } else {
        ecosystem_arg
    };

    let mut findings: Vec<Value> = Vec::new();
    let mut added: Vec<String> = Vec::new();
    let mut removed: Vec<String> = Vec::new();
    let mut version_changed: Vec<Value> = Vec::new();
    let mut source_changed: Vec<Value> = Vec::new();
    let mut hook_changes: Vec<Value> = Vec::new();

    if budget_ctx.should_stop() {
        return budget_ctx
            .check_should_stop("dependency_edit_preflight")
            .unwrap_err();
    }

    match ecosystem {
        "rust" => {
            let sections = ["dependencies", "dev-dependencies", "build-dependencies"];
            for section in &sections {
                let old_deps: HashMap<String, (String, String)> =
                    parse_cargo_deps_toml(old_text, section)
                        .unwrap_or_else(|| parse_cargo_deps(old_text, section))
                        .into_iter()
                        .map(|(n, v, s)| (n, (v, s)))
                        .collect();
                let new_deps: HashMap<String, (String, String)> =
                    parse_cargo_deps_toml(new_text, section)
                        .unwrap_or_else(|| parse_cargo_deps(new_text, section))
                        .into_iter()
                        .map(|(n, v, s)| (n, (v, s)))
                        .collect();

                // Detect additions
                for name in new_deps.keys() {
                    if !old_deps.contains_key(name) {
                        added.push(name.clone());
                        findings.push(finding(
                            machine_codes::DEPENDENCY_ADDED,
                            severity::MEDIUM,
                            &format!("New dependency '{}' added to [{}]", name, section),
                            Some(disposition::CAUTION),
                            None,
                        ));
                        // Emit source-specific findings for newly added deps
                        if let Some((_ver, src)) = new_deps.get(name) {
                            if src == "git" {
                                findings.push(finding(
                                    machine_codes::DEPENDENCY_GIT_SOURCE,
                                    severity::MEDIUM,
                                    &format!(
                                        "New git dependency '{}' added to [{}]",
                                        name, section
                                    ),
                                    Some(disposition::CAUTION),
                                    None,
                                ));
                            } else if src == "path" {
                                findings.push(finding(
                                    machine_codes::DEPENDENCY_PATH_SOURCE,
                                    severity::MEDIUM,
                                    &format!(
                                        "New path dependency '{}' added to [{}]",
                                        name, section
                                    ),
                                    Some(disposition::CAUTION),
                                    None,
                                ));
                            }
                        }
                    }
                }

                // Detect removals
                for name in old_deps.keys() {
                    if !new_deps.contains_key(name) {
                        removed.push(name.clone());
                        findings.push(finding(
                            machine_codes::DEPENDENCY_REMOVED,
                            severity::MEDIUM,
                            &format!("Dependency '{}' removed from [{}]", name, section),
                            Some(disposition::CAUTION),
                            None,
                        ));
                    }
                }

                // Detect version and source changes
                for name in new_deps.keys() {
                    if let Some((old_ver, old_src)) = old_deps.get(name) {
                        if let Some((new_ver, new_src)) = new_deps.get(name) {
                            // Version change
                            if old_ver != new_ver {
                                let change_type = classify_version_change(old_ver, new_ver);
                                version_changed.push(serde_json::json!({
                                    "name": name,
                                    "section": section,
                                    "old": old_ver,
                                    "new": new_ver,
                                    "change_type": change_type.unwrap_or("changed"),
                                }));
                                findings.push(finding(
                                    machine_codes::DEPENDENCY_VERSION_WIDENED,
                                    severity::LOW,
                                    &format!(
                                        "Version constraint changed for '{}': {} → {}",
                                        name, old_ver, new_ver
                                    ),
                                    Some(disposition::INFORMATIONAL),
                                    None,
                                ));
                            }

                            // Source change
                            if old_src != new_src
                                && (old_src != "registry" || new_src != "registry")
                            {
                                let machine = match new_src.as_str() {
                                    "git" => machine_codes::DEPENDENCY_GIT_SOURCE,
                                    "path" => machine_codes::DEPENDENCY_PATH_SOURCE,
                                    _ => machine_codes::DEPENDENCY_VERSION_WIDENED,
                                };
                                let sev = if new_src == "git" || new_src == "path" {
                                    severity::MEDIUM
                                } else {
                                    severity::LOW
                                };
                                let disp = if new_src == "git" || new_src == "path" {
                                    disposition::CAUTION
                                } else {
                                    disposition::INFORMATIONAL
                                };
                                source_changed.push(serde_json::json!({
                                    "name": name,
                                    "section": section,
                                    "old_source": old_src,
                                    "new_source": new_src,
                                }));
                                findings.push(finding(
                                    machine,
                                    sev,
                                    &format!(
                                        "Source changed for '{}': {} → {}",
                                        name, old_src, new_src
                                    ),
                                    Some(disp),
                                    None,
                                ));
                            }

                            // Check for git/path with policy
                            if new_src == "git" && !allow_git {
                                findings.push(finding(
                                    machine_codes::DEPENDENCY_GIT_SOURCE,
                                    severity::HIGH,
                                    &format!(
                                        "Git dependency '{}' requires review (policy: allow_git_deps=false)",
                                        name
                                    ),
                                    Some(disposition::CAUTION),
                                    None,
                                ));
                            }
                            if new_src == "path" && !allow_path {
                                findings.push(finding(
                                    machine_codes::DEPENDENCY_PATH_SOURCE,
                                    severity::MEDIUM,
                                    &format!(
                                        "Path dependency '{}' flagged by policy (allow_path_deps=false)",
                                        name
                                    ),
                                    Some(disposition::CAUTION),
                                    None,
                                ));
                            }
                        }
                    }
                }
            }

            // Check for new build script
            let old_has_build = has_cargo_build_script_toml(old_text)
                .unwrap_or_else(|| has_cargo_build_script(old_text));
            let new_has_build = has_cargo_build_script_toml(new_text)
                .unwrap_or_else(|| has_cargo_build_script(new_text));
            if !old_has_build && new_has_build {
                findings.push(finding(
                    machine_codes::DEPENDENCY_BUILD_SCRIPT,
                    severity::MEDIUM,
                    "Build script field added to [package]",
                    Some(disposition::CAUTION),
                    None,
                ));
            }

            // Check for patch/replace sections
            let old_has_patch = has_cargo_patch_section_toml(old_text)
                .unwrap_or_else(|| has_cargo_patch_section(old_text));
            let new_has_patch = has_cargo_patch_section_toml(new_text)
                .unwrap_or_else(|| has_cargo_patch_section(new_text));
            if !old_has_patch && new_has_patch {
                let sev = if allow_patch {
                    severity::LOW
                } else {
                    severity::HIGH
                };
                findings.push(finding(
                    machine_codes::DEPENDENCY_PATCH_OVERRIDE,
                    sev,
                    "Patch/replace section added",
                    Some(if allow_patch {
                        disposition::INFORMATIONAL
                    } else {
                        disposition::CAUTION
                    }),
                    None,
                ));
            }
        }
        "python" => {
            let old_deps: HashSet<String> = if file_path.ends_with("requirements.txt") {
                parse_requirements_deps(old_text)
                    .into_iter()
                    .map(|(n, _)| n)
                    .collect()
            } else {
                parse_pyproject_deps_toml(old_text)
                    .unwrap_or_else(|| parse_pyproject_deps(old_text))
                    .into_iter()
                    .collect()
            };
            let new_deps: HashSet<String> = if file_path.ends_with("requirements.txt") {
                parse_requirements_deps(new_text)
                    .into_iter()
                    .map(|(n, _)| n)
                    .collect()
            } else {
                parse_pyproject_deps_toml(new_text)
                    .unwrap_or_else(|| parse_pyproject_deps(new_text))
                    .into_iter()
                    .collect()
            };

            for name in &new_deps {
                if !old_deps.contains(name) {
                    added.push(name.clone());
                    findings.push(finding(
                        machine_codes::DEPENDENCY_ADDED,
                        severity::MEDIUM,
                        &format!("New dependency '{}' added", name),
                        Some(disposition::CAUTION),
                        None,
                    ));
                }
            }
            for name in &old_deps {
                if !new_deps.contains(name) {
                    removed.push(name.clone());
                    findings.push(finding(
                        machine_codes::DEPENDENCY_REMOVED,
                        severity::MEDIUM,
                        &format!("Dependency '{}' removed", name),
                        Some(disposition::CAUTION),
                        None,
                    ));
                }
            }

            // Check for direct URL dependencies in requirements.txt
            if file_path.ends_with("requirements.txt") {
                let old_urls = extract_url_deps(old_text);
                let new_urls = extract_url_deps(new_text);
                for url in &new_urls {
                    if !old_urls.contains(url) {
                        findings.push(finding(
                            machine_codes::DEPENDENCY_GIT_SOURCE,
                            severity::MEDIUM,
                            &format!("Direct URL dependency detected: {}", url),
                            Some(disposition::CAUTION),
                            None,
                        ));
                    }
                }

                // Detect editable installs in new text
                for line in new_text.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("-e ") || trimmed.starts_with("--editable ") {
                        let was_present = old_text.lines().any(|l| l.trim() == trimmed);
                        if !was_present {
                            findings.push(finding(
                                machine_codes::DEPENDENCY_PATH_SOURCE,
                                severity::MEDIUM,
                                &format!("Editable install added: {}", trimmed),
                                Some(disposition::CAUTION),
                                None,
                            ));
                        }
                    }
                }

                // Detect local path dependencies in new text
                for line in new_text.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("./")
                        || trimmed.starts_with("../")
                        || trimmed.starts_with('/')
                    {
                        let was_present = old_text.lines().any(|l| l.trim() == trimmed);
                        if !was_present {
                            findings.push(finding(
                                machine_codes::DEPENDENCY_PATH_SOURCE,
                                severity::MEDIUM,
                                &format!("Local path dependency detected: {}", trimmed),
                                Some(disposition::CAUTION),
                                None,
                            ));
                        }
                    }
                }

                // Detect unconstrained specs in new text
                for (name, spec) in parse_requirements_deps(new_text) {
                    if spec.ends_with(" (unconstrained)")
                        && name != "editable"
                        && name != "reference"
                    {
                        let was_constrained = parse_requirements_deps(old_text)
                            .iter()
                            .any(|(n, s)| n == &name && !s.ends_with(" (unconstrained)"));
                        if was_constrained
                            || !parse_requirements_deps(old_text)
                                .iter()
                                .any(|(n, _)| n == &name)
                        {
                            findings.push(finding(
                                machine_codes::DEPENDENCY_VERSION_WIDENED,
                                severity::LOW,
                                &format!("Unconstrained version for '{}': {}", name, spec),
                                Some(disposition::INFORMATIONAL),
                                None,
                            ));
                        }
                    }
                }

                // Detect constraints/includes flags
                for line in new_text.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("-c ")
                        || trimmed.starts_with("-r ")
                        || trimmed.starts_with("-f ")
                    {
                        let was_present = old_text.lines().any(|l| l.trim() == trimmed);
                        if !was_present {
                            findings.push(finding(
                                machine_codes::DEPENDENCY_ADDED,
                                severity::LOW,
                                &format!("Reference file flag added: {}", trimmed),
                                Some(disposition::INFORMATIONAL),
                                None,
                            ));
                        }
                    }
                }
            }

            // Detect build backend changes in pyproject.toml
            if !file_path.ends_with("requirements.txt") {
                let backend_changes = detect_pyproject_build_backend_changes(old_text, new_text);
                for change in backend_changes {
                    hook_changes.push(change.clone());
                    findings.push(finding(
                        machine_codes::DEPENDENCY_BUILD_SCRIPT,
                        severity::MEDIUM,
                        &format!(
                            "Build backend changed: {} → {}",
                            change.get("old").and_then(|v| v.as_str()).unwrap_or("none"),
                            change.get("new").and_then(|v| v.as_str()).unwrap_or("none")
                        ),
                        Some(disposition::CAUTION),
                        None,
                    ));
                }
            }
        }
        "node" => {
            let old_deps: HashMap<String, (String, String)> =
                parse_package_json_deps_json(old_text)
                    .unwrap_or_else(|| parse_package_json_deps(old_text))
                    .into_iter()
                    .map(|(n, v, s)| (n, (v, s)))
                    .collect();
            let new_deps: HashMap<String, (String, String)> =
                parse_package_json_deps_json(new_text)
                    .unwrap_or_else(|| parse_package_json_deps(new_text))
                    .into_iter()
                    .map(|(n, v, s)| (n, (v, s)))
                    .collect();

            for name in new_deps.keys() {
                if !old_deps.contains_key(name) {
                    added.push(name.clone());
                    findings.push(finding(
                        machine_codes::DEPENDENCY_ADDED,
                        severity::MEDIUM,
                        &format!("New dependency '{}' added", name),
                        Some(disposition::CAUTION),
                        None,
                    ));
                    // Emit git/tarball specifier finding for newly added deps
                    if let Some((ver, _sec)) = new_deps.get(name) {
                        if ver.starts_with("http")
                            || ver.starts_with("git+")
                            || ver.starts_with("git://")
                            || ver.starts_with("github:")
                            || ver.starts_with("gitlab:")
                            || ver.contains("tarball")
                            || ver.contains(".tgz")
                            || ver.contains("bitbucket:")
                        {
                            findings.push(finding(
                                machine_codes::DEPENDENCY_GIT_SOURCE,
                                severity::MEDIUM,
                                &format!(
                                    "New dependency '{}' uses URL/tarball specifier: {}",
                                    name, ver
                                ),
                                Some(disposition::CAUTION),
                                None,
                            ));
                        }
                    }
                }
            }
            for name in old_deps.keys() {
                if !new_deps.contains_key(name) {
                    removed.push(name.clone());
                    findings.push(finding(
                        machine_codes::DEPENDENCY_REMOVED,
                        severity::MEDIUM,
                        &format!("Dependency '{}' removed", name),
                        Some(disposition::CAUTION),
                        None,
                    ));
                }
            }

            // Version changes
            for name in new_deps.keys() {
                if let Some((old_ver, _old_sec)) = old_deps.get(name) {
                    if let Some((new_ver, _new_sec)) = new_deps.get(name) {
                        if old_ver != new_ver {
                            let change_type = classify_version_change(old_ver, new_ver);
                            version_changed.push(serde_json::json!({
                                "name": name,
                                "old": old_ver,
                                "new": new_ver,
                                "change_type": change_type.unwrap_or("changed"),
                            }));
                            findings.push(finding(
                                machine_codes::DEPENDENCY_VERSION_WIDENED,
                                severity::LOW,
                                &format!(
                                    "Version changed for '{}': {} → {}",
                                    name, old_ver, new_ver
                                ),
                                Some(disposition::INFORMATIONAL),
                                None,
                            ));
                        }
                    }
                }
            }

            // Script changes
            let old_scripts = parse_package_json_scripts_json(old_text)
                .unwrap_or_else(|| parse_package_json_scripts(old_text));
            let new_scripts: HashMap<String, String> = parse_package_json_scripts_json(new_text)
                .unwrap_or_else(|| parse_package_json_scripts(new_text))
                .into_iter()
                .collect();

            for (key, val) in &old_scripts {
                if let Some(new_val) = new_scripts.get(key) {
                    if val != new_val {
                        let is_risky = NPM_RISKY_SCRIPTS.contains(&key.as_str());
                        hook_changes.push(serde_json::json!({
                            "hook": key,
                            "old": val,
                            "new": new_val,
                            "risky": is_risky,
                        }));
                        findings.push(finding(
                            machine_codes::DEPENDENCY_BUILD_SCRIPT,
                            if is_risky {
                                severity::HIGH
                            } else {
                                severity::LOW
                            },
                            &format!("Script '{}' changed: {} → {}", key, val, new_val),
                            Some(if is_risky {
                                disposition::CAUTION
                            } else {
                                disposition::INFORMATIONAL
                            }),
                            None,
                        ));
                    }
                }
            }

            // Detect new risky scripts
            for key in new_scripts.keys() {
                if !old_scripts.iter().any(|(k, _)| k == key)
                    && NPM_RISKY_SCRIPTS.contains(&key.as_str())
                {
                    hook_changes.push(serde_json::json!({
                        "hook": key,
                        "new": new_scripts[key],
                        "risky": true,
                        "change_type": "added",
                    }));
                    findings.push(finding(
                        machine_codes::DEPENDENCY_BUILD_SCRIPT,
                        severity::HIGH,
                        &format!("Risky script '{}' added: {}", key, new_scripts[key]),
                        Some(disposition::CAUTION),
                        None,
                    ));
                }
            }

            // Detect URL/tarball/git specifiers in dependencies
            for (name, (ver, _sec)) in &new_deps {
                if ver.starts_with("http")
                    || ver.starts_with("git+")
                    || ver.starts_with("git://")
                    || ver.starts_with("github:")
                    || ver.starts_with("gitlab:")
                    || ver.starts_with("bitbucket:")
                    || ver.contains("tarball")
                    || ver.contains(".tgz")
                {
                    let was_url = old_deps
                        .get(name)
                        .map(|(v, _)| {
                            v.starts_with("http")
                                || v.starts_with("git+")
                                || v.starts_with("git://")
                                || v.starts_with("github:")
                                || v.starts_with("gitlab:")
                                || v.starts_with("bitbucket:")
                        })
                        .unwrap_or(false);
                    if !was_url {
                        findings.push(finding(
                            machine_codes::DEPENDENCY_GIT_SOURCE,
                            severity::MEDIUM,
                            &format!("Dependency '{}' uses URL/tarball specifier: {}", name, ver),
                            Some(disposition::CAUTION),
                            None,
                        ));
                    }
                }
            }

            // Detect packageManager changes
            let old_pm = serde_json::from_str::<serde_json::Value>(old_text)
                .ok()
                .and_then(|d| {
                    d.get("packageManager")
                        .and_then(|v| v.as_str())
                        .map(String::from)
                });
            let new_pm = serde_json::from_str::<serde_json::Value>(new_text)
                .ok()
                .and_then(|d| {
                    d.get("packageManager")
                        .and_then(|v| v.as_str())
                        .map(String::from)
                });
            if old_pm != new_pm && new_pm.is_some() {
                findings.push(finding(
                    machine_codes::DEPENDENCY_BUILD_SCRIPT,
                    severity::LOW,
                    &format!("packageManager changed: {:?} → {:?}", old_pm, new_pm),
                    Some(disposition::INFORMATIONAL),
                    None,
                ));
            }
        }
        _ => {}
    }

    // Determine verdict
    let has_blocking = findings.iter().any(|f| {
        f.get("disposition")
            .and_then(|d| d.as_str())
            .map(|d| d == "blocking")
            .unwrap_or(false)
    });
    let has_caution = findings.iter().any(|f| {
        f.get("severity")
            .and_then(|s| s.as_str())
            .map(|s| s == "high" || s == "critical")
            .unwrap_or(false)
    });

    let (dep_verdict, machine_code) = if has_blocking {
        (verdict::BLOCK, machine_codes::DEPENDENCY_PATCH_OVERRIDE)
    } else if has_caution || !added.is_empty() || !removed.is_empty() {
        (verdict::REVIEW, machine_codes::DEPENDENCY_ADDED)
    } else if !version_changed.is_empty() || !source_changed.is_empty() {
        (verdict::REVIEW, machine_codes::DEPENDENCY_VERSION_WIDENED)
    } else {
        (verdict::ALLOW, machine_codes::DEPENDENCY_OK)
    };

    let result = serde_json::json!({
        "file_path": file_path,
        "ecosystem": ecosystem,
        "verdict": dep_verdict,
        "dependency_changes": {
            "added": added,
            "removed": removed,
            "version_changed": version_changed,
            "source_changed": source_changed,
        },
        "hook_changes": hook_changes,
    });

    let mut resp = ToolResponse::success(result, Some("dependency_edit_preflight"))
        .with_machine_code(machine_code)
        .with_verdict(dep_verdict);

    if !findings.is_empty() {
        resp = resp.with_findings(findings);
    }

    resp
}

/// Extract URL-style dependencies from requirements.txt lines.
fn extract_url_deps(text: &str) -> HashSet<String> {
    let mut urls = HashSet::new();
    let url_re = Regex::new(r"(https?://\S+)").ok();
    if let Some(re) = url_re {
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('-') {
                continue;
            }
            for m in re.find_iter(trimmed) {
                urls.insert(m.as_str().to_string());
            }
        }
    }
    urls
}
