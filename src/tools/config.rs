use crate::mcp::machine_codes;
use crate::mcp::schemas::{disposition, finding, severity, verdict, ToolResponse};
use crate::tools::helpers::*;
use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;

static INLINE_FLAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\(\?([aiLmsux]+)\)").unwrap());

pub fn dotenv_validate(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'text' parameter",
                None,
                Some("dotenv_validate"),
            )
        }
    };
    let allow_export = args
        .get("allow_export")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let key_pattern = args
        .get("key_pattern")
        .and_then(|v| v.as_str())
        .unwrap_or("^[A-Za-z_][A-Za-z0-9_]*$");
    let duplicate_policy = args
        .get("duplicate_policy")
        .and_then(|v| v.as_str())
        .unwrap_or("warn");

    if text.len() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!("Text exceeds {} bytes", MAX_TEXT_LENGTH),
            None,
            Some("dotenv_validate"),
        );
    }

    let valid_policies = ["warn", "error", "allow"];
    if !valid_policies.contains(&duplicate_policy) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported duplicate_policy: {}", duplicate_policy),
            Some(vec![format!("Use one of: {}", valid_policies.join(", "))]),
            Some("dotenv_validate"),
        );
    }

    if key_pattern.len() > 1000 {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            "key_pattern exceeds 1000 chars",
            None,
            Some("dotenv_validate"),
        );
    }

    let safety = crate::text::regex_safety::regex_safety_check(key_pattern);
    if !safety.valid_pattern {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            "key_pattern is not a valid regular expression",
            Some(vec!["Fix the regex syntax in key_pattern".to_string()]),
            Some("dotenv_validate"),
        );
    }
    if safety.risk == "medium" || safety.risk == "high" {
        return ToolResponse::error_with_code(
            "unsafe_pattern",
            machine_codes::INVALID_ARGUMENTS,
            &format!(
                "key_pattern has {} risk of catastrophic backtracking",
                safety.risk
            ),
            Some(vec![
                "Use a simpler key_pattern or break it into smaller parts".to_string(),
                "Use the regex_safety_check tool for detailed analysis and suggestions".to_string(),
            ]),
            Some("dotenv_validate"),
        );
    }

    // Reject inline flags in pattern (e.g., (?s), (?i), (?x))
    if let Some(m) = INLINE_FLAG_RE.find(key_pattern) {
        return ToolResponse::error_with_code(
    "unsafe_pattern",
    machine_codes::INVALID_ARGUMENTS,
    &format!("key_pattern contains inline flags '{}'; use the explicit boolean parameters instead", m.as_str()),
    Some(vec!["Remove inline flags and use ignore_case, multiline, dotall parameters".to_string()]),
    Some("dotenv_validate")
);
    }

    let text_owned = text.to_string();
    let key_pattern_owned = key_pattern.to_string();
    let duplicate_policy_owned = duplicate_policy.to_string();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crate::text::dotenv_validate(
            &text_owned,
            allow_export,
            &key_pattern_owned,
            &duplicate_policy_owned,
        )
    }))
    .unwrap_or_else(|_| crate::text::config::DotenvValidateResult {
        parse_ok: false,
        entries: Vec::new(),
        duplicates: Vec::new(),
        invalid_lines: Vec::new(),
        requires_quoting: Vec::new(),
        contains_expansion_syntax: Vec::new(),
        findings: vec!["Dotenv validation panicked (possible resource limit)".to_string()],
    });

    ToolResponse::success(
        serde_json::json!({
            "parse_ok": result.parse_ok,
            "entries": result.entries,
            "duplicates": result.duplicates,
            "invalid_lines": result.invalid_lines,
            "requires_quoting": result.requires_quoting,
            "contains_expansion_syntax": result.contains_expansion_syntax,
            "findings": result.findings,
        }),
        Some("dotenv_validate"),
    )
    .with_tool("dotenv_validate")
}

pub fn ini_validate(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'text' parameter",
                None,
                Some("ini_validate"),
            )
        }
    };
    let duplicate_policy = args
        .get("duplicate_policy")
        .and_then(|v| v.as_str())
        .unwrap_or("warn");

    if text.len() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!("Text exceeds {} bytes", MAX_TEXT_LENGTH),
            None,
            Some("ini_validate"),
        );
    }

    let valid_policies = ["warn", "error", "allow"];
    if !valid_policies.contains(&duplicate_policy) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported duplicate_policy: {}", duplicate_policy),
            Some(vec![format!("Use one of: {}", valid_policies.join(", "))]),
            Some("ini_validate"),
        );
    }

    let result = crate::text::ini_validate(text, duplicate_policy);

    ToolResponse::success(
        serde_json::json!({
            "parse_ok": result.parse_ok,
            "sections": result.sections,
            "keys_by_section": result.keys_by_section,
            "duplicates": result.duplicates,
            "invalid_lines": result.invalid_lines,
            "findings": result.findings,
        }),
        Some("ini_validate"),
    )
    .with_tool("ini_validate")
}

pub fn config_preflight(args: &Value) -> ToolResponse {
    let budget_ctx = crate::mcp::budget::for_handler(crate::mcp::budget::ToolBudget::HEAVY);

    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'text' parameter",
                None,
                Some("config_preflight"),
            )
        }
    };
    let format = args
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("auto");
    let schema = args.get("schema");
    let strict = args
        .get("strict")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if text.len() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!("Text exceeds {} bytes", MAX_TEXT_LENGTH),
            None,
            Some("config_preflight"),
        );
    }

    let valid_formats = ["auto", "json", "toml", "dotenv", "ini", "cargo_toml"];
    if !valid_formats.contains(&format) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported format: {}", format),
            Some(vec![format!("Use one of: {}", valid_formats.join(", "))]),
            Some("config_preflight"),
        );
    }

    // Auto-detect format
    let detected_format = if format == "auto" {
        let stripped = text.trim();
        if stripped.starts_with('{') || stripped.starts_with('[') {
            // Could be JSON or TOML; try JSON first via the typed core
            // (no validate_json adapter envelope round-trip).
            let is_json = matches!(crate::text::validate_json(text), Ok(r) if r.valid);
            if is_json {
                "json"
            } else {
                "toml"
            }
        } else if stripped.contains('=') && !stripped.starts_with('{') {
            // Heuristic: if contains = and doesn't look like JSON object
            "dotenv"
        } else {
            "json"
        }
    } else {
        format
    };

    let mut subresults = serde_json::Map::new();
    let mut findings: Vec<serde_json::Value> = Vec::new();
    let mut code_list: Vec<String> = Vec::new();
    let mut config_verdict = verdict::VALID;

    if budget_ctx.should_stop() {
        return budget_ctx
            .check_should_stop("config_preflight")
            .unwrap_err();
    }

    match detected_format {
        "json" => {
            // Typed composition: deterministic cores directly, no adapter
            // JSON envelopes.
            match crate::text::validate_json(text) {
                Ok(vj) => {
                    let r = serde_json::json!({
                        "valid": vj.valid,
                        "error": vj.error,
                        "line": vj.line,
                        "column": vj.column,
                        "position": vj.position,
                        "type": vj.json_type,
                        "top_level_keys": vj.top_level_keys,
                    });
                    subresults.insert("validate_json".to_string(), r.clone());
                    let valid = r.get("valid").and_then(|v| v.as_bool()).unwrap_or(false);
                    if !valid {
                        config_verdict = verdict::INVALID;
                        code_list.push(machine_codes::CONFIG_PARSE_FAILED.to_string());
                        findings.push(finding(
                            "JSON_PARSE_ERROR",
                            severity::HIGH,
                            r.get("error")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Invalid JSON"),
                            Some(disposition::BLOCKING),
                            None,
                        ));
                    } else if let Some(sch) = schema {
                        // Typed schema validation: parse once, validate against
                        // the typed core (no validate_schema_light adapter).
                        if let Ok(data) = serde_json::from_str::<Value>(text) {
                            match crate::text::validate_schema_light(&data, sch) {
                                Ok(vs) => {
                                    let vr = serde_json::json!({
                                        "valid": vs.valid,
                                        "violations": vs.violations,
                                        "truncated": vs.truncated,
                                        "summary": vs.summary,
                                    });
                                    subresults
                                        .insert("validate_schema_light".to_string(), vr.clone());
                                    if !vs.valid {
                                        code_list.push(
                                            machine_codes::CONFIG_SCHEMA_MISMATCH.to_string(),
                                        );
                                        config_verdict = verdict::VALID_WITH_WARNINGS;
                                        if !vs.violations.is_empty() {
                                            for violation in &vs.violations {
                                                let msg = violation.message.as_str();
                                                findings.push(finding(
                                                    "SCHEMA_ERROR",
                                                    if strict {
                                                        severity::HIGH
                                                    } else {
                                                        severity::MEDIUM
                                                    },
                                                    msg,
                                                    Some(if strict {
                                                        disposition::BLOCKING
                                                    } else {
                                                        disposition::CAUTION
                                                    }),
                                                    None,
                                                ));
                                            }
                                        } else {
                                            findings.push(finding(
                                                "SCHEMA_ERROR",
                                                if strict {
                                                    severity::HIGH
                                                } else {
                                                    severity::MEDIUM
                                                },
                                                "Schema validation failed",
                                                Some(if strict {
                                                    disposition::BLOCKING
                                                } else {
                                                    disposition::CAUTION
                                                }),
                                                None,
                                            ));
                                        }
                                    }
                                }
                                Err(e) => {
                                    // Mirrors the adapter's invalid-schema path:
                                    // no subresult, no verdict change.
                                    let _ = e;
                                }
                            }
                        }
                    }
                    // Optionally canonicalize via the typed core with the
                    // adapter's defaults (sort_keys=true, no indent).
                    if config_verdict != verdict::INVALID {
                        if let Ok(jc) =
                            crate::text::json_canonicalize(text, true, None, false, true, false)
                        {
                            let changed = jc.canonical.as_deref().is_some_and(|c| c != text);
                            subresults.insert(
                                "json_canonicalize".to_string(),
                                serde_json::json!({
                                    "changed": changed,
                                }),
                            );
                        }
                    }
                }
                Err(e) => {
                    code_list.push(machine_codes::CONFIG_PARSE_FAILED.to_string());
                    findings.push(finding(
                        "CONFIG_ERROR",
                        severity::HIGH,
                        &e,
                        Some(disposition::BLOCKING),
                        None,
                    ));
                }
            }
        }
        "toml" => {
            // Typed composition: validate_toml core directly.
            match crate::text::toml::validate_toml(text) {
                Ok(vt) => {
                    let r = serde_json::json!({
                        "valid": vt.valid,
                        "error": vt.error,
                        "line": vt.line,
                        "column": vt.column,
                        "position": vt.position,
                        "type": vt.toml_type,
                        "top_level_keys": vt.top_level_keys,
                        "tables": vt.tables,
                    });
                    subresults.insert("validate_toml".to_string(), r.clone());
                    if !vt.valid {
                        config_verdict = verdict::INVALID;
                        code_list.push(machine_codes::CONFIG_PARSE_FAILED.to_string());
                        findings.push(finding(
                            "TOML_PARSE_ERROR",
                            severity::HIGH,
                            r.get("error")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Invalid TOML"),
                            Some(disposition::BLOCKING),
                            None,
                        ));
                    } else {
                        // Intentional same-module reuse: toml_shape shares this
                        // module's handler; typed extraction is follow-up.
                        let ts_result = toml_shape_tool(&serde_json::json!({"text": text}));
                        if let Some(ref r) = ts_result.result {
                            subresults.insert("toml_shape".to_string(), r.clone());
                        }
                    }
                }
                Err(e) => {
                    code_list.push(machine_codes::CONFIG_PARSE_FAILED.to_string());
                    findings.push(finding(
                        "CONFIG_ERROR",
                        severity::HIGH,
                        &e,
                        Some(disposition::BLOCKING),
                        None,
                    ));
                }
            }
        }
        "dotenv" => {
            // Typed composition: dotenv core directly with the adapter's
            // defaults (allow_export=true, default key pattern, warn).
            let dv = crate::text::dotenv_validate(text, true, "^[A-Za-z_][A-Za-z0-9_]*$", "warn");
            let r = serde_json::json!({
                "parse_ok": dv.parse_ok,
                "entries": dv.entries,
                "duplicates": dv.duplicates,
                "invalid_lines": dv.invalid_lines,
                "requires_quoting": dv.requires_quoting,
                "contains_expansion_syntax": dv.contains_expansion_syntax,
                "findings": dv.findings,
            });
            subresults.insert("dotenv_validate".to_string(), r.clone());
            if !dv.parse_ok {
                config_verdict = verdict::INVALID;
                code_list.push(machine_codes::CONFIG_PARSE_FAILED.to_string());
                if !dv.findings.is_empty() {
                    for err in &dv.findings {
                        findings.push(finding(
                            "DOTENV_ERROR",
                            severity::HIGH,
                            err.as_str(),
                            Some(disposition::BLOCKING),
                            None,
                        ));
                    }
                } else {
                    findings.push(finding(
                        "DOTENV_ERROR",
                        severity::HIGH,
                        "Invalid dotenv format",
                        Some(disposition::BLOCKING),
                        None,
                    ));
                }
            }
        }
        "ini" => {
            // Typed composition: ini core directly with the adapter default.
            let iv = crate::text::ini_validate(text, "warn");
            let r = serde_json::json!({
                "parse_ok": iv.parse_ok,
                "sections": iv.sections,
                "keys_by_section": iv.keys_by_section,
                "duplicates": iv.duplicates,
                "invalid_lines": iv.invalid_lines,
                "findings": iv.findings,
            });
            subresults.insert("ini_validate".to_string(), r.clone());
            if !iv.parse_ok {
                config_verdict = verdict::INVALID;
                code_list.push(machine_codes::CONFIG_PARSE_FAILED.to_string());
                if !iv.findings.is_empty() {
                    for err in &iv.findings {
                        findings.push(finding(
                            "INI_ERROR",
                            severity::HIGH,
                            err.as_str(),
                            Some(disposition::BLOCKING),
                            None,
                        ));
                    }
                } else {
                    findings.push(finding(
                        "INI_ERROR",
                        severity::HIGH,
                        "Invalid INI format",
                        Some(disposition::BLOCKING),
                        None,
                    ));
                }
            }
        }
        "cargo_toml" => {
            // Typed composition: cargo core directly with adapter defaults.
            let ct = crate::text::cargo_toml_inspect(text, true, true);
            let r = serde_json::json!({
                "parse_ok": ct.parse_ok,
                "package": ct.package,
                "workspace": ct.workspace,
                "dependencies": ct.dependencies,
                "path_dependencies": ct.path_dependencies,
                "suspicious_dependency_names": ct.suspicious_dependency_names,
                "duplicate_or_confusable_dependency_names": ct.duplicate_or_confusable_dependency_names,
                "findings": ct.findings,
            });
            subresults.insert("cargo_toml_inspect".to_string(), r.clone());
            if !ct.parse_ok {
                config_verdict = verdict::INVALID;
                code_list.push(machine_codes::CONFIG_PARSE_FAILED.to_string());
                findings.push(finding(
                    "CARGO_PARSE_ERROR",
                    severity::HIGH,
                    "Cargo.toml parse failed",
                    Some(disposition::BLOCKING),
                    None,
                ));
            } else {
                // Preserve the adapter-result mapping exactly: cargo findings
                // are strings, so severity/code/message lookups miss and each
                // becomes an informational CARGO_NOTE with empty message.
                for _f in &ct.findings {
                    findings.push(finding(
                        "CARGO_NOTE",
                        severity::INFO,
                        "",
                        Some(disposition::INFORMATIONAL),
                        None,
                    ));
                }
            }
        }
        _ => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                &format!("Unsupported format: {}", detected_format),
                Some(vec![format!("Use one of: {}", valid_formats.join(", "))]),
                Some("config_preflight"),
            );
        }
    }

    let parse_ok = config_verdict != verdict::INVALID;

    let machine_code = if !parse_ok {
        code_list
            .first()
            .cloned()
            .unwrap_or_else(|| machine_codes::CONFIG_PARSE_FAILED.to_string())
    } else if !findings.is_empty() {
        code_list
            .first()
            .cloned()
            .unwrap_or_else(|| machine_codes::CONFIG_HAS_WARNINGS.to_string())
    } else {
        machine_codes::CONFIG_OK.to_string()
    };

    let summary = format!(
        "{} config: {} ({} finding(s))",
        detected_format,
        config_verdict,
        findings.len()
    );

    let mut result = serde_json::json!({
        "valid": parse_ok,
        "verdict": config_verdict,
        "format": detected_format,
        "findings": findings,
        "machine_code": machine_code,
        "summary": summary,
    });
    if !subresults.is_empty() {
        result["subresults"] = serde_json::Value::Object(subresults);
    }

    let mut resp =
        ToolResponse::success(result, Some("config_preflight")).with_tool("config_preflight");
    resp = resp
        .with_machine_code(&machine_code)
        .with_verdict(config_verdict);
    if !findings.is_empty() {
        resp = resp.with_findings(findings);
    }
    resp
}

pub fn toml_shape_tool(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'text' parameter",
                None,
                Some("toml_shape"),
            )
        }
    };
    let max_tables = match args.get("max_tables") {
        Some(v) => {
            if v.is_boolean() || !v.is_number() {
                return ToolResponse::error_with_code(
                    "invalid_arguments",
                    machine_codes::INVALID_ARGUMENTS,
                    &format!(
                        "max_tables must be an integer, got {}",
                        match v {
                            Value::Bool(_) => "bool",
                            Value::Null => "null",
                            Value::String(_) =>
                                return ToolResponse::error_with_code(
                                    "invalid_arguments",
                                    machine_codes::INVALID_ARGUMENTS,
                                    "max_tables must be an integer, got string",
                                    None,
                                    Some("toml_shape")
                                ),
                            Value::Array(_) => "array",
                            Value::Object(_) => "object",
                            _ => "unknown",
                        }
                    ),
                    None,
                    Some("toml_shape"),
                );
            }
            if v.as_i64().unwrap_or(0) < 0 {
                return ToolResponse::error_with_code(
                    "invalid_arguments",
                    machine_codes::INVALID_ARGUMENTS,
                    "max_tables must be a non-negative integer",
                    None,
                    Some("toml_shape"),
                );
            }
            v.as_u64().unwrap_or(100) as usize
        }
        None => 100,
    };
    if max_tables == 0 {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            "max_tables must be a positive integer",
            None,
            Some("toml_shape"),
        );
    }
    let detail = args
        .get("detail")
        .and_then(|v| v.as_str())
        .unwrap_or("normal");

    if text.len() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!("Text exceeds {} bytes", MAX_TEXT_LENGTH),
            None,
            Some("toml_shape"),
        );
    }

    let valid_details = ["summary", "normal", "full"];
    if !valid_details.contains(&detail) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported detail level: {}", detail),
            Some(vec![format!("Use one of: {}", valid_details.join(", "))]),
            Some("toml_shape"),
        );
    }

    match crate::text::toml::toml_shape(text, max_tables) {
        Ok(result) => {
            if detail == "summary" {
                ToolResponse::success(
                    serde_json::json!({
                        "valid": result.valid,
                        "truncated": result.truncated,
                        "summary": result.summary,
                    }),
                    Some("toml_shape"),
                )
                .with_tool("toml_shape")
            } else {
                ToolResponse::success(
                    serde_json::json!({
                        "valid": result.valid,
                        "top_level_keys": result.top_level_keys,
                        "tables": result.tables,
                        "truncated": result.truncated,
                        "summary": result.summary,
                    }),
                    Some("toml_shape"),
                )
                .with_tool("toml_shape")
            }
        }
        Err(e) => ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &e,
            None,
            Some("toml_shape"),
        ),
    }
}
