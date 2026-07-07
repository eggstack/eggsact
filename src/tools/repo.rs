use crate::mcp::machine_codes;
use crate::mcp::schemas::{disposition, finding, severity, verdict, ToolResponse};
use crate::tools::helpers::*;
use serde_json::Value;

// ---------------------------------------------------------------------------
// repo_manifest_inspect
// ---------------------------------------------------------------------------

/// Known manifest patterns by ecosystem.
const RUST_MANIFESTS: &[&str] = &["Cargo.toml", "Cargo.lock", "build.rs"];
const RUST_SOURCE_HINTS: &[&str] = &["src/main.rs", "src/lib.rs", "src/bin/"];
const PYTHON_MANIFESTS: &[&str] = &[
    "pyproject.toml",
    "requirements.txt",
    "setup.cfg",
    "setup.py",
    "Pipfile",
    "poetry.lock",
];
const NODE_MANIFESTS: &[&str] = &[
    "package.json",
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    ".npmrc",
];
const GO_MANIFESTS: &[&str] = &["go.mod", "go.sum"];

/// Detect project type from paths.
fn detect_project_types(paths: &[String]) -> Vec<String> {
    let mut has_rust = false;
    let mut has_python = false;
    let mut has_node = false;
    let mut has_go = false;

    for p in paths {
        let lower = p.to_lowercase();
        let basename = p.rsplit('/').next().unwrap_or(p);

        if RUST_MANIFESTS.contains(&basename) || RUST_SOURCE_HINTS.iter().any(|h| lower.contains(h))
        {
            has_rust = true;
        }
        if PYTHON_MANIFESTS.contains(&basename) {
            has_python = true;
        }
        if NODE_MANIFESTS.contains(&basename) {
            has_node = true;
        }
        if GO_MANIFESTS.contains(&basename) {
            has_go = true;
        }
    }

    let count = [has_rust, has_python, has_node, has_go]
        .iter()
        .filter(|&&b| b)
        .count();

    if count == 0 {
        vec!["unknown".to_string()]
    } else if count > 1 {
        let mut types = Vec::new();
        if has_rust {
            types.push("rust");
        }
        if has_python {
            types.push("python");
        }
        if has_node {
            types.push("node");
        }
        if has_go {
            types.push("go");
        }
        types.push("mixed");
        types.into_iter().map(String::from).collect()
    } else if has_rust {
        vec!["rust".to_string()]
    } else if has_python {
        vec!["python".to_string()]
    } else if has_node {
        vec!["node".to_string()]
    } else {
        vec!["go".to_string()]
    }
}

/// Classified paths by category.
type ClassifiedPaths = (
    Vec<String>,
    Vec<String>,
    Vec<String>,
    Vec<String>,
    Vec<String>,
    Vec<String>,
    Vec<String>,
);

/// Collect manifest, config, and lockfile paths from a path list.
fn classify_manifests(paths: &[String]) -> ClassifiedPaths {
    let mut rust_manifests = Vec::new();
    let mut python_manifests = Vec::new();
    let mut node_manifests = Vec::new();
    let mut go_manifests = Vec::new();
    let mut other_manifests = Vec::new();
    let mut config_paths = Vec::new();
    let mut lockfile_paths = Vec::new();

    let config_patterns = [
        ".env",
        ".gitignore",
        ".editorconfig",
        "tsconfig.json",
        ".eslintrc",
        "rustfmt.toml",
        ".rustfmt.toml",
        "clippy.toml",
        ".clippy.toml",
        "Makefile",
        "Dockerfile",
        ".dockerignore",
    ];
    let lockfile_patterns = [
        "Cargo.lock",
        "package-lock.json",
        "yarn.lock",
        "pnpm-lock.yaml",
        "poetry.lock",
        "Pipfile.lock",
        "go.sum",
    ];

    for p in paths {
        let basename = p.rsplit('/').next().unwrap_or(p);

        if RUST_MANIFESTS.contains(&basename) {
            rust_manifests.push(p.clone());
        } else if PYTHON_MANIFESTS.contains(&basename) {
            python_manifests.push(p.clone());
        } else if NODE_MANIFESTS.contains(&basename) {
            node_manifests.push(p.clone());
        } else if GO_MANIFESTS.contains(&basename) {
            go_manifests.push(p.clone());
        } else if lockfile_patterns.contains(&basename) {
            lockfile_paths.push(p.clone());
        } else if config_patterns.iter().any(|m| basename.contains(m)) {
            config_paths.push(p.clone());
        } else if basename.contains("config")
            || basename.contains("rc")
            || basename.starts_with('.')
        {
            other_manifests.push(p.clone());
        }
    }

    (
        rust_manifests,
        python_manifests,
        node_manifests,
        go_manifests,
        other_manifests,
        config_paths,
        lockfile_paths,
    )
}

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

    let project_types = detect_project_types(&path_strings);
    let is_unknown = project_types.iter().any(|t| t == "unknown");
    let is_mixed = project_types.iter().any(|t| t == "mixed");

    let (
        rust_manifests,
        python_manifests,
        node_manifests,
        go_manifests,
        other_manifests,
        config_paths,
        lockfile_paths,
    ) = classify_manifests(&path_strings);

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
    lower.starts_with("http://")
        && !lower.starts_with("http://localhost")
        && !lower.starts_with("http://127.")
        && !lower.starts_with("http://0.0.0.0")
        && !lower.starts_with("http://[::1]")
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

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
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
    // recursive traversal; dotenv/INI retain line scanners; YAML is heuristic-only.
    let kv_pairs: Vec<(String, String)> = match format {
        "json" | "package_json" => json_kv_pairs(text).unwrap_or_else(|| {
            // Fallback to line scanner if parse fails (parse_ok will catch it)
            toml_key_values(text)
        }),
        "pyproject" | "cargo_toml" | "toml" => {
            toml_parsed_kv_pairs(text).unwrap_or_else(|| toml_key_values(text))
        }
        "yaml" => toml_key_values(text),
        "dotenv" => dotenv_key_values(text),
        "ini" => ini_key_values(text),
        _ => dotenv_key_values(text),
    };

    // Check for parse issues — JSON formats use serde_json parsing; the line
    // scanner below remains a heuristic for key extraction and falls back
    // to non-JSON formats. A real parse failure surfaces as a finding so
    // callers can detect malformed input rather than silently misclassify.
    let parse_ok = match format {
        "json" | "package_json" => serde_json::from_str::<serde_json::Value>(text.trim()).is_ok(),
        "toml" | "cargo_toml" | "pyproject" => text.parse::<toml::Value>().is_ok(),
        "yaml" => !text.trim().is_empty(),
        _ => true,
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
        if p.chars().count() > MAX_TEXT_LENGTH {
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

    let (buckets, entrypoint_candidates, high_leverage_paths, tool_hints, raw_findings) =
        classify_paths(&path_strs);

    let mut directory_count = 0usize;
    let mut file_count = 0usize;
    for p in &path_strs {
        let normalized = p.replace('\\', "/");
        if normalized.ends_with('/') || normalized.ends_with("/.") || normalized.ends_with("/..") {
            directory_count += 1;
        } else {
            let filename = normalized.rsplit('/').next().unwrap_or("");
            if filename.contains('.') {
                file_count += 1;
            } else {
                directory_count += 1;
            }
        }
    }

    if budget_ctx.should_stop() {
        return budget_ctx
            .check_should_stop("repo_tree_summarize")
            .unwrap_err();
    }

    let mut project_types = Vec::new();
    let manifest_paths = buckets.get("manifests").cloned().unwrap_or_default();
    let has_rust = manifest_paths.iter().any(|p| p.ends_with("Cargo.toml"));
    let has_python = manifest_paths.iter().any(|p| {
        p.ends_with("pyproject.toml")
            || p.ends_with("setup.py")
            || p.ends_with("setup.cfg")
            || p.ends_with("requirements.txt")
    });
    let has_node = manifest_paths
        .iter()
        .any(|p| p.ends_with("package.json") || p.ends_with("tsconfig.json"));
    let has_go = manifest_paths.iter().any(|p| p.ends_with("go.mod"));

    if has_rust {
        project_types.push("rust".to_string());
    }
    if has_python {
        project_types.push("python".to_string());
    }
    if has_node {
        project_types.push("node".to_string());
    }
    if has_go {
        project_types.push("go".to_string());
    }
    if project_types.len() > 1 {
        project_types.push("mixed".to_string());
    }
    if project_types.is_empty() {
        project_types.push("unknown".to_string());
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
