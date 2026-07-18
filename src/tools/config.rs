use crate::mcp::machine_codes;
use crate::mcp::schemas::{disposition, finding, severity, verdict, ToolResponse};
use crate::tools::helpers::*;
use serde_json::Value;

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

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
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
    let inline_flag_re = regex::Regex::new(r"\(\?([aiLmsux]+)\)").unwrap();
    if let Some(m) = inline_flag_re.find(key_pattern) {
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

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
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

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
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
            // Could be JSON or TOML; try JSON first
            let vj_result =
                crate::tools::validation::validate_json(&serde_json::json!({"text": text}));
            let is_json = vj_result
                .result
                .as_ref()
                .and_then(|r| r.get("valid"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
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
            let vj_result =
                crate::tools::validation::validate_json(&serde_json::json!({"text": text}));
            if let Some(ref r) = vj_result.result {
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
                    let vs_args = serde_json::json!({"text": text, "schema": sch});
                    let vs_result = crate::tools::validation::validate_schema_light_tool(&vs_args);
                    if let Some(ref vr) = vs_result.result {
                        subresults.insert("validate_schema_light".to_string(), vr.clone());
                        let vs_valid = vr.get("valid").and_then(|v| v.as_bool()).unwrap_or(true);
                        if !vs_valid {
                            code_list.push(machine_codes::CONFIG_SCHEMA_MISMATCH.to_string());
                            config_verdict = verdict::VALID_WITH_WARNINGS;
                            if let Some(violations) =
                                vr.get("violations").and_then(|v| v.as_array())
                            {
                                for violation in violations {
                                    let msg = violation
                                        .get("message")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("Schema violation");
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
                }
                // Optionally canonicalize
                if config_verdict != verdict::INVALID {
                    let jc_result =
                        crate::tools::json::json_canonicalize(&serde_json::json!({"text": text}));
                    if let Some(ref r) = jc_result.result {
                        let canonical = r.get("canonical").and_then(|v| v.as_str());
                        let changed = canonical.is_some_and(|c| c != text);
                        subresults.insert(
                            "json_canonicalize".to_string(),
                            serde_json::json!({
                                "changed": changed,
                            }),
                        );
                    }
                }
            } else if let Some(ref e) = vj_result.error {
                code_list.push(machine_codes::CONFIG_PARSE_FAILED.to_string());
                findings.push(finding(
                    "CONFIG_ERROR",
                    severity::HIGH,
                    e,
                    Some(disposition::BLOCKING),
                    None,
                ));
            }
        }
        "toml" => {
            let vt_result =
                crate::tools::validation::validate_toml_tool(&serde_json::json!({"text": text}));
            if let Some(ref r) = vt_result.result {
                subresults.insert("validate_toml".to_string(), r.clone());
                let valid = r
                    .get("valid")
                    .or_else(|| r.get("parse_ok"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if !valid {
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
                    let ts_result = toml_shape_tool(&serde_json::json!({"text": text}));
                    if let Some(ref r) = ts_result.result {
                        subresults.insert("toml_shape".to_string(), r.clone());
                    }
                }
            } else if let Some(ref e) = vt_result.error {
                code_list.push(machine_codes::CONFIG_PARSE_FAILED.to_string());
                findings.push(finding(
                    "CONFIG_ERROR",
                    severity::HIGH,
                    e,
                    Some(disposition::BLOCKING),
                    None,
                ));
            }
        }
        "dotenv" => {
            let dv_result = dotenv_validate(&serde_json::json!({"text": text}));
            if let Some(ref r) = dv_result.result {
                subresults.insert("dotenv_validate".to_string(), r.clone());
                let parse_ok = r.get("parse_ok").and_then(|v| v.as_bool()).unwrap_or(false);
                if !parse_ok {
                    config_verdict = verdict::INVALID;
                    code_list.push(machine_codes::CONFIG_PARSE_FAILED.to_string());
                    if let Some(dv_findings) = r.get("findings").and_then(|v| v.as_array()) {
                        for err in dv_findings {
                            findings.push(finding(
                                "DOTENV_ERROR",
                                severity::HIGH,
                                err.as_str().unwrap_or("Invalid dotenv format"),
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
            } else if let Some(ref e) = dv_result.error {
                code_list.push(machine_codes::CONFIG_PARSE_FAILED.to_string());
                findings.push(finding(
                    "CONFIG_ERROR",
                    severity::HIGH,
                    e,
                    Some(disposition::BLOCKING),
                    None,
                ));
            }
        }
        "ini" => {
            let iv_result = ini_validate(&serde_json::json!({"text": text}));
            if let Some(ref r) = iv_result.result {
                subresults.insert("ini_validate".to_string(), r.clone());
                let parse_ok = r.get("parse_ok").and_then(|v| v.as_bool()).unwrap_or(false);
                if !parse_ok {
                    config_verdict = verdict::INVALID;
                    code_list.push(machine_codes::CONFIG_PARSE_FAILED.to_string());
                    if let Some(iv_findings) = r.get("findings").and_then(|v| v.as_array()) {
                        for err in iv_findings {
                            findings.push(finding(
                                "INI_ERROR",
                                severity::HIGH,
                                err.as_str().unwrap_or("Invalid INI format"),
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
            } else if let Some(ref e) = iv_result.error {
                code_list.push(machine_codes::CONFIG_PARSE_FAILED.to_string());
                findings.push(finding(
                    "CONFIG_ERROR",
                    severity::HIGH,
                    e,
                    Some(disposition::BLOCKING),
                    None,
                ));
            }
        }
        "cargo_toml" => {
            let ct_result =
                crate::tools::cargo::cargo_toml_inspect(&serde_json::json!({"text": text}));
            if let Some(ref r) = ct_result.result {
                subresults.insert("cargo_toml_inspect".to_string(), r.clone());
                let parse_ok = r.get("parse_ok").and_then(|v| v.as_bool()).unwrap_or(false);
                if !parse_ok {
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
                    if let Some(ct_findings) = r.get("findings").and_then(|v| v.as_array()) {
                        for f in ct_findings {
                            let sev = match f
                                .get("severity")
                                .and_then(|v| v.as_str())
                                .unwrap_or("info")
                            {
                                "error" => severity::HIGH,
                                "warn" => severity::MEDIUM,
                                _ => severity::INFO,
                            };
                            let disp = match sev {
                                severity::HIGH => Some(disposition::BLOCKING),
                                severity::MEDIUM => Some(disposition::CAUTION),
                                _ => Some(disposition::INFORMATIONAL),
                            };
                            findings.push(finding(
                                f.get("code")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("CARGO_NOTE"),
                                sev,
                                f.get("message").and_then(|v| v.as_str()).unwrap_or(""),
                                disp,
                                None,
                            ));
                        }
                    }
                }
            } else if let Some(ref e) = ct_result.error {
                code_list.push(machine_codes::CONFIG_PARSE_FAILED.to_string());
                findings.push(finding(
                    "CONFIG_ERROR",
                    severity::HIGH,
                    e,
                    Some(disposition::BLOCKING),
                    None,
                ));
            }
        }
        _ => unreachable!(),
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

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
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
