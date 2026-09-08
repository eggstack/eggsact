//! Repo-analysis adapters over the canonical [`crate::services::repo`] facts.
//!
//! Each public tool is a projection/policy over shared [`RepoFacts`]:
//! `repo_manifest_inspect` formats manifest/ecosystem facts,
//! `repo_tree_summarize` formats buckets/entrypoints/high-leverage facts,
//! `repo_language_detect` formats language/ecosystem facts, and
//! `test_command_suggest` applies its command-template policy to the shared
//! ecosystems. None re-detects ecosystems, manifests, buckets, or languages.

use crate::mcp::machine_codes;
use crate::mcp::schemas::{disposition, finding, severity, verdict, ToolResponse};
use crate::services::repo as repo_facts;
use crate::tools::helpers::*;
use serde_json::Value;

// ---------------------------------------------------------------------------
// repo_manifest_inspect
// ---------------------------------------------------------------------------

/// Generate tool hints based on detected project types.
fn generate_tool_hints(project_types: &[String]) -> Value {
    let mut inspect_tools = Vec::new();
    let mut commands = Vec::new();

    for ptype in project_types {
        match ptype.as_str() {
            "rust" => {
                if !inspect_tools.contains(&"cargo_toml_inspect") {
                    inspect_tools.push("cargo_toml_inspect");
                }
                if !inspect_tools.contains(&"dependency_edit_preflight") {
                    inspect_tools.push("dependency_edit_preflight");
                }
                if !commands.contains(&"cargo check") {
                    commands.push("cargo check");
                }
                if !commands.contains(&"cargo test") {
                    commands.push("cargo test");
                }
                if !commands.contains(&"cargo fmt --check") {
                    commands.push("cargo fmt --check");
                }
            }
            "python" => {
                if !inspect_tools.contains(&"config_file_inspect") {
                    inspect_tools.push("config_file_inspect");
                }
                if !inspect_tools.contains(&"dependency_edit_preflight") {
                    inspect_tools.push("dependency_edit_preflight");
                }
                if !commands.contains(&"python -m pytest") {
                    commands.push("python -m pytest");
                }
                if !commands.contains(&"ruff check") {
                    commands.push("ruff check");
                }
            }
            "node" => {
                if !inspect_tools.contains(&"config_file_inspect") {
                    inspect_tools.push("config_file_inspect");
                }
                if !inspect_tools.contains(&"dependency_edit_preflight") {
                    inspect_tools.push("dependency_edit_preflight");
                }
                if !commands.contains(&"npm test") {
                    commands.push("npm test");
                }
                if !commands.contains(&"npm run lint") {
                    commands.push("npm run lint");
                }
            }
            "go" => {
                if !commands.contains(&"go build ./...") {
                    commands.push("go build ./...");
                }
                if !commands.contains(&"go test ./...") {
                    commands.push("go test ./...");
                }
            }
            _ => {}
        }
    }

    serde_json::json!({
        "inspect_tools": inspect_tools,
        "commands": commands,
    })
}

pub fn repo_manifest_inspect(args: &Value) -> ToolResponse {
    let paths = match args.get("paths").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'paths' parameter (expected array)",
                None,
                Some("repo_manifest_inspect"),
            )
        }
    };

    let max_paths = args
        .get("max_paths")
        .and_then(|v| v.as_u64())
        .unwrap_or(200) as usize;

    if paths.len() > max_paths {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!(
                "Path list ({} items) exceeds max_paths limit ({})",
                paths.len(),
                max_paths
            ),
            None,
            Some("repo_manifest_inspect"),
        );
    }

    let path_strings: Vec<String> = paths
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();

    // Projection over canonical RepoFacts (shared ecosystem/manifest facts).
    let facts = repo_facts::repo_facts(&path_strings);
    let project_types = facts.project_types.clone();
    let is_unknown = facts.is_unknown;
    let is_mixed = facts.is_mixed;
    let rust_manifests = facts
        .manifests_by_ecosystem
        .get("rust")
        .cloned()
        .unwrap_or_default();
    let python_manifests = facts
        .manifests_by_ecosystem
        .get("python")
        .cloned()
        .unwrap_or_default();
    let node_manifests = facts
        .manifests_by_ecosystem
        .get("node")
        .cloned()
        .unwrap_or_default();
    let go_manifests = facts
        .manifests_by_ecosystem
        .get("go")
        .cloned()
        .unwrap_or_default();
    let other_manifests = facts.other_manifests.clone();
    let config_paths = facts.config_paths.clone();
    let lockfile_paths = facts.lockfile_paths.clone();

    let tool_hints = generate_tool_hints(&project_types);

    let mut findings = Vec::new();

    if is_unknown {
        findings.push(finding(
            machine_codes::REPO_UNKNOWN,
            severity::INFO,
            "Could not determine project type from provided paths",
            Some(disposition::INFORMATIONAL),
            None,
        ));
    }

    if is_mixed {
        findings.push(finding(
            machine_codes::REPO_DETECTED,
            severity::INFO,
            "Multiple project types detected (mixed repository)",
            Some(disposition::INFORMATIONAL),
            None,
        ));
    }

    let manifest_verdict = if is_unknown {
        verdict::REVIEW
    } else {
        verdict::ALLOW
    };

    let machine_code = if is_unknown {
        machine_codes::REPO_UNKNOWN
    } else {
        machine_codes::REPO_DETECTED
    };

    let result = serde_json::json!({
        "project_types": project_types,
        "manifest_paths": {
            "rust": rust_manifests,
            "python": python_manifests,
            "node": node_manifests,
            "go": go_manifests,
            "other": other_manifests,
        },
        "config_paths": config_paths,
        "lockfile_paths": lockfile_paths,
        "tool_hints": tool_hints,
        "verdict": manifest_verdict,
    });

    let mut resp = ToolResponse::success(result, Some("repo_manifest_inspect"))
        .with_machine_code(machine_code)
        .with_verdict(manifest_verdict);

    if !findings.is_empty() {
        resp = resp.with_findings(findings);
    }

    resp
}

// ---------------------------------------------------------------------------
// config_file_inspect
// ---------------------------------------------------------------------------

/// Secret-like key patterns.
const SECRET_PATTERNS: &[&str] = &[
    "secret",
    "token",
    "password",
    "passwd",
    "api_key",
    "apikey",
    "private_key",
    "private-key",
    "access_key",
    "access-key",
    "auth_token",
    "auth-token",
    "credential",
];

/// Debug flag patterns.
const DEBUG_PATTERNS: &[&str] = &[
    "debug",
    "debug_mode",
    "debug_enabled",
    "verbose",
    "trace",
    "log_level",
    "loglevel",
    "logging",
];

/// Command hook patterns.
const HOOK_PATTERNS: &[&str] = &[
    "install",
    "preinstall",
    "postinstall",
    "prepare",
    "uninstall",
    "preuninstall",
    "postuninstall",
    "build",
    "prebuild",
    "postbuild",
    "start",
    "restart",
    "stop",
    "reload",
    "on_start",
    "on_stop",
    "hook",
    "hooks",
];

/// Wildcard host patterns.
const WILDCARD_HOST_PATTERNS: &[&str] = &[
    "cors_origins",
    "allowed_origins",
    "allowed_hosts",
    "hosts",
    "allow_all_origins",
    "wildcard",
];

/// Detect config format from file path.
fn detect_config_format(file_path: &str, text: &str) -> &'static str {
    let lower = file_path.to_lowercase();
    let basename = file_path.rsplit('/').next().unwrap_or(file_path);

    if basename == "Cargo.toml" {
        "cargo_toml"
    } else if basename == "package.json" {
        "package_json"
    } else if basename == "pyproject.toml" {
        "pyproject"
    } else if lower.ends_with(".json") {
        "json"
    } else if lower.ends_with(".toml") {
        "toml"
    } else if lower.ends_with(".yaml") || lower.ends_with(".yml") {
        "yaml"
    } else if lower.ends_with(".env")
        || lower.ends_with(".env.local")
        || lower.ends_with(".env.production")
    {
        "dotenv"
    } else if lower.ends_with(".ini") || lower.ends_with(".cfg") || lower.ends_with(".conf") {
        "ini"
    } else {
        // Heuristic
        let trimmed = text.trim();
        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            "json"
        } else if trimmed.contains("[") && trimmed.contains("]") {
            "ini"
        } else if trimmed.contains("=") && !trimmed.starts_with('{') {
            "dotenv"
        } else {
            "toml"
        }
    }
}

/// Extract TOML key-value pairs (line-based heuristic, used as fallback).
fn toml_key_values(text: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('[') {
            continue;
        }
        if let Some(idx) = trimmed.find('=') {
            let key = trimmed[..idx].trim().to_string();
            let val = trimmed[idx + 1..].trim().to_string();
            if !key.is_empty() {
                pairs.push((key, val));
            }
        }
    }
    pairs
}

/// Extract YAML key-value pairs (line-based heuristic for `key: value`).
fn yaml_key_values(text: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('-') {
            continue;
        }
        // Strip inline comments after value (naive: split on '#')
        let without_comment = trimmed.split('#').next().unwrap_or(trimmed).trim();
        if let Some(idx) = without_comment.find(':') {
            let key = without_comment[..idx].trim().to_string();
            let val = without_comment[idx + 1..]
                .trim()
                .trim_matches(|c| c == '"' || c == '\'')
                .to_string();
            if !key.is_empty() {
                pairs.push((key, val));
            }
        }
    }
    pairs
}

/// Recursively walk a `serde_json::Value` and emit (dotted.path, string_value) pairs.
fn json_walk(value: &serde_json::Value, prefix: &str, out: &mut Vec<(String, String)>) {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                let path = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{}.{}", prefix, k)
                };
                json_walk(v, &path, out);
            }
        }
        serde_json::Value::Array(arr) => {
            for (i, v) in arr.iter().enumerate() {
                let path = format!("{}[{}]", prefix, i);
                json_walk(v, &path, out);
            }
        }
        serde_json::Value::String(s) => {
            out.push((prefix.to_string(), s.clone()));
        }
        serde_json::Value::Bool(b) => {
            out.push((prefix.to_string(), b.to_string()));
        }
        serde_json::Value::Number(n) => {
            out.push((prefix.to_string(), n.to_string()));
        }
        serde_json::Value::Null => {
            out.push((prefix.to_string(), "null".to_string()));
        }
    }
}

/// Parse JSON and extract key-value pairs using recursive object traversal.
/// Returns None if the text is not valid JSON.
fn json_kv_pairs(text: &str) -> Option<Vec<(String, String)>> {
    let value: serde_json::Value = serde_json::from_str(text.trim()).ok()?;
    let mut pairs = Vec::new();
    json_walk(&value, "", &mut pairs);
    Some(pairs)
}

/// Recursively walk a `toml::Value` and emit (dotted.path, string_value) pairs.
fn toml_walk(value: &toml::Value, prefix: &str, out: &mut Vec<(String, String)>) {
    match value {
        toml::Value::Table(map) => {
            for (k, v) in map {
                let path = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{}.{}", prefix, k)
                };
                toml_walk(v, &path, out);
            }
        }
        toml::Value::Array(arr) => {
            for (i, v) in arr.iter().enumerate() {
                let path = format!("{}[{}]", prefix, i);
                toml_walk(v, &path, out);
            }
        }
        toml::Value::String(s) => {
            out.push((prefix.to_string(), s.clone()));
        }
        toml::Value::Boolean(b) => {
            out.push((prefix.to_string(), b.to_string()));
        }
        toml::Value::Integer(n) => {
            out.push((prefix.to_string(), n.to_string()));
        }
        toml::Value::Float(n) => {
            out.push((prefix.to_string(), n.to_string()));
        }
        toml::Value::Datetime(dt) => {
            out.push((prefix.to_string(), dt.to_string()));
        }
    }
}

/// Parse TOML and extract key-value pairs using recursive table traversal.
/// Returns None if the text is not valid TOML.
fn toml_parsed_kv_pairs(text: &str) -> Option<Vec<(String, String)>> {
    let value: toml::Value = text.parse().ok()?;
    let mut pairs = Vec::new();
    toml_walk(&value, "", &mut pairs);
    Some(pairs)
}

/// Extract dotenv key-value pairs.
fn dotenv_key_values(text: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(idx) = trimmed.find('=') {
            let key = trimmed[..idx].trim().to_string();
            let val = trimmed[idx + 1..].trim().to_string();
            if !key.is_empty() {
                if let Some(stripped) = key.strip_prefix("export ") {
                    pairs.push((stripped.to_string(), val));
                } else {
                    pairs.push((key, val));
                }
            }
        }
    }
    pairs
}

/// Extract INI key-value pairs.
fn ini_key_values(text: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    let mut current_section = String::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            current_section = trimmed[1..trimmed.len() - 1].to_string();
            continue;
        }
        if let Some(idx) = trimmed.find('=') {
            let key = trimmed[..idx].trim().to_string();
            let val = trimmed[idx + 1..].trim().to_string();
            let full_key = if current_section.is_empty() {
                key.clone()
            } else {
                format!("{}.{}", current_section, key)
            };
            pairs.push((full_key, val));
        }
    }
    pairs
}

/// Check if a URL value uses http:// where https:// is expected.
fn is_insecure_url(val: &str) -> bool {
    let lower = val.to_lowercase();
    if !lower.starts_with("http://") {
        return false;
    }
    let after_scheme = &lower[7..]; // skip "http://"
                                    // Extract host: up to first /, :, ?, or #
    let host = after_scheme
        .split(['/', ':', '?', '#'])
        .next()
        .unwrap_or("");

    if host == "localhost" {
        return false;
    }
    // IPv4 loopback: 127.x.y.z where x,y,z are valid decimal octets
    if host.starts_with("127.") {
        let parts: Vec<&str> = host.split('.').collect();
        if parts.len() == 4 && parts[1..].iter().all(|s| s.parse::<u8>().is_ok()) {
            return false;
        }
    }
    if host == "0.0.0.0" {
        return false;
    }
    if host == "[::1]" || host == "::1" {
        return false;
    }

    true
}

/// Check if a value looks like a TLS verification disable.
fn is_tls_disabled(key: &str, val: &str) -> bool {
    let lower_key = key.to_lowercase();
    let lower_val = val.to_lowercase();
    (lower_key.contains("verify") || lower_key.contains("tls"))
        && (lower_val == "false" || lower_val == "0" || lower_val == "no")
}

/// Check if a value looks like a wildcard host.
fn is_wildcard_host(key: &str, val: &str) -> bool {
    let lower_key = key.to_lowercase();
    let lower_val = val.to_lowercase();
    WILDCARD_HOST_PATTERNS.iter().any(|p| lower_key.contains(p))
        && (lower_val.contains("*") || lower_val == "true" || lower_val == "all")
}

pub fn config_file_inspect(args: &Value) -> ToolResponse {
    let budget_ctx = crate::mcp::budget::for_handler(crate::mcp::budget::ToolBudget::HEAVY);

    let file_path = match args.get("file_path").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'file_path' parameter",
                None,
                Some("config_file_inspect"),
            )
        }
    };
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'text' parameter",
                None,
                Some("config_file_inspect"),
            )
        }
    };
    let format_arg = args
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("auto");
    let policy = args.get("policy");

    let allow_debug = policy
        .and_then(|p| p.get("allow_debug_flags"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let allow_insecure = policy
        .and_then(|p| p.get("allow_insecure_urls"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let allow_hooks = policy
        .and_then(|p| p.get("allow_command_hooks"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if text.len() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!("Text exceeds {} bytes", MAX_TEXT_LENGTH),
            None,
            Some("config_file_inspect"),
        );
    }

    let format = if format_arg == "auto" {
        detect_config_format(file_path, text)
    } else {
        format_arg
    };

    // Parse key-value pairs based on format — JSON and TOML use parser-backed
    // recursive traversal; dotenv/INI retain line scanners; YAML is heuristic-only
    // (no YAML parser dependency: `yaml_key_values` is a naive `key: value`
    // line scan, so `parse_ok` for YAML only reports that the input was
    // non-empty, not that it is valid YAML — see `analysis_mode` below).
    let kv_pairs: Vec<(String, String)> = match format {
        "json" | "package_json" => json_kv_pairs(text).unwrap_or_else(|| {
            // Fallback to line scanner if parse fails (parse_ok will catch it)
            toml_key_values(text)
        }),
        "pyproject" | "cargo_toml" | "toml" => {
            toml_parsed_kv_pairs(text).unwrap_or_else(|| toml_key_values(text))
        }
        "yaml" => yaml_key_values(text),
        "dotenv" => dotenv_key_values(text),
        "ini" => ini_key_values(text),
        _ => dotenv_key_values(text),
    };

    // Check for parse issues — JSON formats use serde_json parsing; TOML
    // formats use the `toml` parser. YAML has no parser (heuristic-only line
    // scan), so `parse_ok` for YAML is true for any non-empty input and must
    // NOT be read as syntax validation. The additive `analysis_mode` result
    // field (`"parser"` vs `"heuristic"`) lets callers distinguish the two
    // guarantees without breaking the existing boolean shape.
    let parse_ok = match format {
        "json" | "package_json" => serde_json::from_str::<serde_json::Value>(text.trim()).is_ok(),
        "toml" | "cargo_toml" | "pyproject" => text.parse::<toml::Value>().is_ok(),
        "yaml" => !text.trim().is_empty(),
        _ => true,
    };
    // `"parser"` means `parse_ok` reflects a real parser verdict;
    // `"heuristic"` means the scan completed but syntax was not validated.
    let analysis_mode = match format {
        "json" | "package_json" | "toml" | "cargo_toml" | "pyproject" => "parser",
        _ => "heuristic",
    };
    if !parse_ok {
        // Surface parse failures to the caller as a structured finding so
        // callers can route on the parse issue rather than ignoring it.
        // (Deferred until after `findings` is declared below.)
    }

    let mut secret_risks = Vec::new();
    let mut insecure_urls = Vec::new();
    let mut debug_flags = Vec::new();
    let mut command_hooks = Vec::new();
    let mut findings = Vec::new();

    // Surface parse failures now that `findings` is in scope.
    if !parse_ok {
        findings.push(finding(
            machine_codes::CONFIG_PARSE_FAILED,
            severity::HIGH,
            &format!("Config file failed to parse as {}", format),
            Some(disposition::BLOCKING),
            None,
        ));
    }

    if budget_ctx.should_stop() {
        return budget_ctx
            .check_should_stop("config_file_inspect")
            .unwrap_err();
    }

    for (key, val) in &kv_pairs {
        let lower_key = key.to_lowercase();

        // Secret detection
        if SECRET_PATTERNS.iter().any(|p| lower_key.contains(p)) {
            let masked = mask_secret_preview(val);
            secret_risks.push(serde_json::json!({
                "key": key,
                "value_preview": masked,
            }));
            findings.push(finding(
                machine_codes::CONFIG_RISK_SECRET_KEY,
                severity::MEDIUM,
                &format!("Secret-like key detected: {}", key),
                Some(disposition::CAUTION),
                None,
            ));
        }

        // Insecure URL detection
        if is_insecure_url(val) && !allow_insecure {
            insecure_urls.push(serde_json::json!({
                "key": key,
                "url": val,
            }));
            findings.push(finding(
                machine_codes::CONFIG_RISK_INSECURE_URL,
                severity::LOW,
                &format!("Insecure URL (http://) in key '{}': {}", key, val),
                Some(disposition::CAUTION),
                None,
            ));
        }

        // Debug flag detection
        if DEBUG_PATTERNS.iter().any(|p| lower_key.contains(p))
            && (val == "true" || val == "1" || val == "on" || val == "debug")
            && !allow_debug
        {
            debug_flags.push(serde_json::json!({
                "key": key,
                "value": val,
            }));
            findings.push(finding(
                machine_codes::CONFIG_RISK_DEBUG_FLAG,
                severity::LOW,
                &format!("Debug flag enabled: {} = {}", key, val),
                Some(disposition::CAUTION),
                None,
            ));
        }

        // Command hook detection
        if HOOK_PATTERNS.iter().any(|p| lower_key.contains(p))
            && !val.is_empty()
            && val != "true"
            && val != "false"
            && val != "0"
            && val != "1"
            && !allow_hooks
        {
            command_hooks.push(serde_json::json!({
                "key": key,
                "value": val,
            }));
            findings.push(finding(
                machine_codes::CONFIG_RISK_COMMAND_HOOK,
                severity::MEDIUM,
                &format!("Command hook detected: {} = {}", key, val),
                Some(disposition::CAUTION),
                None,
            ));
        }

        // TLS disabled
        if is_tls_disabled(key, val) {
            findings.push(finding(
                machine_codes::CONFIG_RISK_TLS_DISABLED,
                severity::MEDIUM,
                &format!("TLS verification appears disabled: {} = {}", key, val),
                Some(disposition::CAUTION),
                None,
            ));
        }

        // Wildcard host
        if is_wildcard_host(key, val) {
            findings.push(finding(
                machine_codes::CONFIG_RISK_WILDCARD_HOST,
                severity::LOW,
                &format!("Wildcard/permissive host setting: {} = {}", key, val),
                Some(disposition::INFORMATIONAL),
                None,
            ));
        }
    }

    // Build shape summary
    let key_count = kv_pairs.len();
    let section_count = if format == "ini" || format == "toml" || format == "cargo_toml" {
        text.lines()
            .filter(|l| l.trim().starts_with('[') && l.trim().ends_with(']'))
            .count()
    } else {
        0
    };

    let shape_summary = serde_json::json!({
        "key_count": key_count,
        "section_count": section_count,
        "line_count": text.lines().count(),
    });

    // Determine verdict
    let has_high = findings.iter().any(|f| {
        f.get("severity")
            .and_then(|s| s.as_str())
            .map(|s| s == "high" || s == "critical")
            .unwrap_or(false)
    });
    let has_medium = findings.iter().any(|f| {
        f.get("severity")
            .and_then(|s| s.as_str())
            .map(|s| s == "medium")
            .unwrap_or(false)
    });

    let (config_verdict, machine_code) = if !parse_ok {
        (verdict::INVALID, machine_codes::CONFIG_PARSE_FAILED)
    } else if has_high {
        (verdict::BLOCK, machine_codes::CONFIG_HAS_WARNINGS)
    } else if has_medium || !findings.is_empty() {
        (verdict::REVIEW, machine_codes::CONFIG_HAS_WARNINGS)
    } else {
        (verdict::ALLOW, machine_codes::CONFIG_OK)
    };

    let result = serde_json::json!({
        "file_path": file_path,
        "format": format,
        "parse_ok": parse_ok,
        "analysis_mode": analysis_mode,
        "shape_summary": shape_summary,
        "risky_keys": findings.iter().filter(|f| {
            f.get("code").and_then(|c| c.as_str()).map(|c| c.starts_with("CONFIG_RISK_")).unwrap_or(false)
        }).cloned().collect::<Vec<_>>(),
        "secret_risks": secret_risks,
        "insecure_urls": insecure_urls,
        "debug_flags": debug_flags,
        "command_hooks": command_hooks,
        "verdict": config_verdict,
        "machine_code": machine_code,
    });

    let mut resp = ToolResponse::success(result, Some("config_file_inspect"))
        .with_machine_code(machine_code)
        .with_verdict(config_verdict);

    if !findings.is_empty() {
        resp = resp.with_findings(findings);
    }

    resp
}

// ---------------------------------------------------------------------------
// repo_tree_summarize
// ---------------------------------------------------------------------------

pub fn repo_tree_summarize(args: &Value) -> ToolResponse {
    let budget_ctx = crate::mcp::budget::for_handler(crate::mcp::budget::ToolBudget::MODERATE);

    let paths = match require_array_arg(args, "paths", "repo_tree_summarize") {
        Ok(arr) => arr,
        Err(resp) => return *resp,
    };

    let max_paths = args
        .get("max_paths")
        .and_then(|v| v.as_i64())
        .unwrap_or(1000) as usize;

    if paths.len() > max_paths {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!(
                "Too many paths: {} exceeds max_paths {}",
                paths.len(),
                max_paths
            ),
            None,
            Some("repo_tree_summarize"),
        );
    }

    // Convert to String vec
    let path_strs: Vec<String> = paths
        .iter()
        .filter_map(|p| p.as_str().map(|s| s.to_string()))
        .collect();

    if path_strs.len() != paths.len() {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            "All paths must be strings",
            None,
            Some("repo_tree_summarize"),
        );
    }

    for p in &path_strs {
        if p.len() > MAX_TEXT_LENGTH {
            return ToolResponse::error_with_code(
                "input_too_large",
                machine_codes::INPUT_TOO_LARGE,
                &format!("Path '{}' exceeds MAX_TEXT_LENGTH", p),
                None,
                Some("repo_tree_summarize"),
            );
        }
    }

    if budget_ctx.should_stop() {
        return budget_ctx
            .check_should_stop("repo_tree_summarize")
            .unwrap_err();
    }

    // Projection over canonical RepoFacts. Buckets/entrypoints/high-leverage
    // come from the shared classifier; project types come from the shared
    // ecosystem table (canonical). This replaces the former independent
    // manifest-ends_with derivation, which missed lockfiles/source hints
    // (e.g. `Cargo.lock` alone was `unknown` here but `rust` in
    // `repo_manifest_inspect`). Canonical behavior is now `rust` everywhere.
    let facts = repo_facts::repo_facts(&path_strs);
    let buckets = facts.buckets.clone();
    let entrypoint_candidates = facts.entrypoint_candidates.clone();
    let high_leverage_paths = facts.high_leverage_paths.clone();
    let tool_hints = facts.tool_hints.clone();
    let project_types = facts.project_types.clone();
    let manifest_paths = buckets.get("manifests").cloned().unwrap_or_default();

    // Reconstruct the legacy `classify_paths` raw findings from the same
    // canonical buckets so tree-specific LOW findings stay stable.
    let mut raw_findings: Vec<String> = Vec::new();
    {
        let has_manifest = buckets.contains_key("manifests");
        let has_lockfile = buckets.contains_key("lockfiles");
        let has_ci = buckets.contains_key("ci");
        let total_raw = path_strs.len();
        let gen_count = buckets.get("generated").map_or(0, |v| v.len());
        let vend_count = buckets.get("vendor").map_or(0, |v| v.len());
        if has_manifest && !has_lockfile {
            raw_findings.push("Manifest found without lockfile".to_string());
        }
        if has_ci {
            raw_findings
                .push("CI configuration present — may affect build/test workflow".to_string());
        }
        if total_raw > 0 {
            let gen_pct = (gen_count as f64 / total_raw as f64) * 100.0;
            if gen_pct > 50.0 {
                raw_findings.push(format!(
                    "Unusually many generated paths ({}/{} = {:.0}%)",
                    gen_count, total_raw, gen_pct
                ));
            }
            let vend_pct = (vend_count as f64 / total_raw as f64) * 100.0;
            if vend_pct > 50.0 {
                raw_findings.push(format!(
                    "Unusually many vendor/dependency paths ({}/{} = {:.0}%)",
                    vend_count, total_raw, vend_pct
                ));
            }
        }
    }

    let mut directory_count = 0usize;
    let mut file_count = 0usize;
    for p in &path_strs {
        let normalized = p.replace('\\', "/");
        if normalized.ends_with('/') || normalized.ends_with("/.") || normalized.ends_with("/..") {
            directory_count += 1;
        } else {
            file_count += 1;
        }
    }

    if budget_ctx.should_stop() {
        return budget_ctx
            .check_should_stop("repo_tree_summarize")
            .unwrap_err();
    }

    let mut findings = Vec::new();
    let has_lockfile = buckets.contains_key("lockfiles");
    if !manifest_paths.is_empty() && !has_lockfile {
        findings.push(finding(
            machine_codes::REPO_TREE_REVIEW,
            severity::MEDIUM,
            "Manifest found without lockfile",
            Some(disposition::CAUTION),
            None,
        ));
    }

    let total = path_strs.len();
    let generated_count = buckets.get("generated").map_or(0, |v| v.len());
    let vendor_count = buckets.get("vendor").map_or(0, |v| v.len());

    if total > 0 {
        let gen_pct = (generated_count as f64 / total as f64) * 100.0;
        if gen_pct > 50.0 {
            findings.push(finding(
                machine_codes::REPO_TREE_REVIEW,
                severity::MEDIUM,
                &format!(
                    "Unusually many generated paths ({}/{} = {:.0}%)",
                    generated_count, total, gen_pct
                ),
                Some(disposition::CAUTION),
                None,
            ));
        }
        let vend_pct = (vendor_count as f64 / total as f64) * 100.0;
        if vend_pct > 50.0 {
            findings.push(finding(
                machine_codes::REPO_TREE_REVIEW,
                severity::MEDIUM,
                &format!(
                    "Unusually many vendor/dependency paths ({}/{} = {:.0}%)",
                    vendor_count, total, vend_pct
                ),
                Some(disposition::CAUTION),
                None,
            ));
        }
    }

    for msg in &raw_findings {
        findings.push(finding(
            machine_codes::REPO_TREE_REVIEW,
            severity::LOW,
            msg,
            Some(disposition::INFORMATIONAL),
            None,
        ));
    }

    let has_review = findings.iter().any(|f| {
        f.get("severity")
            .and_then(|s| s.as_str())
            .map(|s| s == "high" || s == "medium")
            .unwrap_or(false)
    });

    let (tree_verdict, machine_code) = if has_review {
        (verdict::REVIEW, machine_codes::REPO_TREE_REVIEW)
    } else {
        (verdict::ALLOW, machine_codes::REPO_TREE_OK)
    };

    let result = serde_json::json!({
        "project_types": project_types,
        "path_count": total,
        "directory_count": directory_count,
        "file_count": file_count,
        "buckets": buckets,
        "entrypoint_candidates": entrypoint_candidates,
        "high_leverage_paths": high_leverage_paths,
        "tool_hints": tool_hints,
        "verdict": tree_verdict,
        "machine_code": machine_code,
    });

    let mut resp = ToolResponse::success(result, Some("repo_tree_summarize"))
        .with_machine_code(machine_code)
        .with_verdict(tree_verdict);

    if !findings.is_empty() {
        resp = resp.with_findings(findings);
    }

    resp
}

// ---------------------------------------------------------------------------
// test_command_suggest
// ---------------------------------------------------------------------------

struct CommandTemplate {
    command: &'static str,
    category: &'static str,
    confidence: f64,
    reason: &'static str,
}

fn suggest_commands_for_project(project_types: &[String]) -> Vec<CommandTemplate> {
    let mut commands = Vec::new();

    for ptype in project_types {
        match ptype.as_str() {
            "rust" => {
                commands.push(CommandTemplate {
                    command: "cargo check",
                    category: "build",
                    confidence: 0.95,
                    reason: "Rust project detected via Cargo.toml",
                });
                commands.push(CommandTemplate {
                    command: "cargo test",
                    category: "test",
                    confidence: 0.95,
                    reason: "Rust project detected via Cargo.toml",
                });
                commands.push(CommandTemplate {
                    command: "cargo fmt --check",
                    category: "format",
                    confidence: 0.9,
                    reason: "Rust project detected via Cargo.toml",
                });
                commands.push(CommandTemplate {
                    command: "cargo clippy --all-targets --all-features",
                    category: "lint",
                    confidence: 0.9,
                    reason: "Rust project detected via Cargo.toml",
                });
            }
            "python" => {
                commands.push(CommandTemplate {
                    command: "python -m pytest",
                    category: "test",
                    confidence: 0.85,
                    reason: "Python project detected",
                });
                commands.push(CommandTemplate {
                    command: "ruff check .",
                    category: "lint",
                    confidence: 0.8,
                    reason: "Python project detected",
                });
                commands.push(CommandTemplate {
                    command: "ruff format --check .",
                    category: "format",
                    confidence: 0.75,
                    reason: "Python project detected",
                });
            }
            "node" => {
                commands.push(CommandTemplate {
                    command: "npm test",
                    category: "test",
                    confidence: 0.85,
                    reason: "Node.js project detected via package.json",
                });
                commands.push(CommandTemplate {
                    command: "npm run lint",
                    category: "lint",
                    confidence: 0.8,
                    reason: "Node.js project detected",
                });
                commands.push(CommandTemplate {
                    command: "npx tsc --noEmit",
                    category: "typecheck",
                    confidence: 0.7,
                    reason: "Node.js project detected",
                });
            }
            "go" => {
                commands.push(CommandTemplate {
                    command: "go build ./...",
                    category: "build",
                    confidence: 0.9,
                    reason: "Go project detected via go.mod",
                });
                commands.push(CommandTemplate {
                    command: "go test ./...",
                    category: "test",
                    confidence: 0.9,
                    reason: "Go project detected via go.mod",
                });
                commands.push(CommandTemplate {
                    command: "go vet ./...",
                    category: "lint",
                    confidence: 0.85,
                    reason: "Go project detected via go.mod",
                });
            }
            _ => {}
        }
    }

    commands.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    commands.dedup_by(|a, b| a.command == b.command);
    commands
}

pub fn test_command_suggest(args: &Value) -> ToolResponse {
    let budget_ctx = crate::mcp::budget::for_handler(crate::mcp::budget::ToolBudget::CHEAP);

    let paths = match args.get("paths").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'paths' parameter (expected array)",
                None,
                Some("test_command_suggest"),
            )
        }
    };

    if paths.len() > MAX_LIST_ITEMS {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!(
                "Path list ({} items) exceeds maximum of {}",
                paths.len(),
                MAX_LIST_ITEMS
            ),
            None,
            Some("test_command_suggest"),
        );
    }

    let path_strings: Vec<String> = paths
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();

    // Policy over canonical ecosystems: command templates stay in the adapter.
    let project_types = repo_facts::detect_project_types(&path_strings);

    if budget_ctx.should_stop() {
        return budget_ctx
            .check_should_stop("test_command_suggest")
            .unwrap_err();
    }

    let templates = suggest_commands_for_project(&project_types);

    let suggested_commands: Vec<Value> = templates
        .iter()
        .map(|t| {
            serde_json::json!({
                "command": t.command,
                "category": t.category,
                "confidence": t.confidence,
                "reason": t.reason,
            })
        })
        .collect();

    let is_unknown = project_types.iter().any(|t| t == "unknown");
    let mut findings = Vec::new();

    if is_unknown {
        findings.push(finding(
            machine_codes::TEST_COMMANDS_SUGGESTED,
            severity::INFO,
            "Could not determine project type; suggestions may be inaccurate",
            Some(disposition::INFORMATIONAL),
            None,
        ));
    }

    let (cmd_verdict, machine_code) = if is_unknown {
        (verdict::REVIEW, machine_codes::TEST_COMMANDS_SUGGESTED)
    } else {
        (verdict::ALLOW, machine_codes::TEST_COMMANDS_SUGGESTED)
    };

    let result = serde_json::json!({
        "project_types": project_types,
        "suggested_commands": suggested_commands,
        "verdict": cmd_verdict,
    });

    let mut resp = ToolResponse::success(result, Some("test_command_suggest"))
        .with_tool("test_command_suggest")
        .with_machine_code(machine_code)
        .with_verdict(cmd_verdict);

    if !findings.is_empty() {
        resp = resp.with_findings(findings);
    }

    resp
}

// ---------------------------------------------------------------------------
// repo_language_detect
// ---------------------------------------------------------------------------

pub fn repo_language_detect(args: &Value) -> ToolResponse {
    let budget_ctx = crate::mcp::budget::for_handler(crate::mcp::budget::ToolBudget::CHEAP);

    let paths = match args.get("paths").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'paths' parameter (expected array)",
                None,
                Some("repo_language_detect"),
            )
        }
    };

    let max_paths = args
        .get("max_paths")
        .and_then(|v| v.as_u64())
        .unwrap_or(500) as usize;

    if paths.len() > max_paths {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!(
                "Path list ({} items) exceeds max_paths limit ({})",
                paths.len(),
                max_paths
            ),
            None,
            Some("repo_language_detect"),
        );
    }

    let path_strings: Vec<String> = paths
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();

    // Projection over canonical RepoFacts: shared language evidence and
    // shared ecosystems (which include Rust source hints, so `src/main.rs`
    // alone reports `rust` here just as it does in manifest/tree tools).
    let facts = repo_facts::repo_facts(&path_strings);
    let ecosystems = facts.ecosystems.clone();

    if budget_ctx.should_stop() {
        return budget_ctx
            .check_should_stop("repo_language_detect")
            .unwrap_err();
    }

    let mut languages: Vec<Value> = facts
        .languages
        .iter()
        .map(|l| {
            serde_json::json!({
                "name": l.name,
                "file_count": l.file_count,
                "extensions": l.extensions,
                "confidence": l.confidence,
            })
        })
        .collect();
    languages.sort_by(|a, b| {
        b.get("file_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
            .cmp(&a.get("file_count").and_then(|v| v.as_u64()).unwrap_or(0))
    });

    let primary_language = languages.first().and_then(|l| l.get("name").cloned());

    let is_empty = languages.is_empty();
    let mut findings = Vec::new();

    if is_empty {
        findings.push(finding(
            machine_codes::REPO_LANGUAGE_DETECTED,
            severity::INFO,
            "No programming languages detected from provided paths",
            Some(disposition::INFORMATIONAL),
            None,
        ));
    }

    let (lang_verdict, machine_code) = if is_empty {
        (verdict::REVIEW, machine_codes::REPO_LANGUAGE_DETECTED)
    } else {
        (verdict::ALLOW, machine_codes::REPO_LANGUAGE_DETECTED)
    };

    let result = serde_json::json!({
        "languages": languages,
        "ecosystems": ecosystems,
        "primary_language": primary_language,
        "verdict": lang_verdict,
    });

    let mut resp = ToolResponse::success(result, Some("repo_language_detect"))
        .with_tool("repo_language_detect")
        .with_machine_code(machine_code)
        .with_verdict(lang_verdict);

    if !findings.is_empty() {
        resp = resp.with_findings(findings);
    }

    resp
}
