use crate::mcp::schemas::ToolResponse;
use crate::tools::helpers::*;
use serde_json::Value;
use std::time::Duration;

pub fn dotenv_validate(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
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
        return ToolResponse::error(
            "input_too_large",
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("dotenv_validate"),
        );
    }

    let valid_policies = ["warn", "error", "allow"];
    if !valid_policies.contains(&duplicate_policy) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported duplicate_policy: {}", duplicate_policy),
            Some(vec![format!("Use one of: {}", valid_policies.join(", "))]),
            Some("dotenv_validate"),
        );
    }

    if key_pattern.len() > 1000 {
        return ToolResponse::error(
            "input_too_large",
            "key_pattern exceeds 1000 chars",
            None,
            Some("dotenv_validate"),
        );
    }

    let safety = crate::text::regex_safety::regex_safety_check(key_pattern);
    if !safety.valid_pattern {
        return ToolResponse::error(
            "invalid_arguments",
            "key_pattern is not a valid regular expression",
            Some(vec!["Fix the regex syntax in key_pattern".to_string()]),
            Some("dotenv_validate"),
        );
    }
    if safety.risk == "medium" || safety.risk == "high" {
        return ToolResponse::error(
            "unsafe_pattern",
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
        return ToolResponse::error(
    "unsafe_pattern",
    &format!("key_pattern contains inline flags '{}'; use the explicit boolean parameters instead", m.as_str()),
    Some(vec!["Remove inline flags and use ignore_case, multiline, dotall parameters".to_string()]),
    Some("dotenv_validate")
);
    }

    // Run validation on a dedicated thread with timeout to prevent ReDoS from
    // hanging the server (matching Python's multiprocessing.Process isolation).
    let text_owned = text.to_string();
    let key_pattern_owned = key_pattern.to_string();
    let duplicate_policy_owned = duplicate_policy.to_string();
    let result = match run_with_timeout(Duration::from_secs(REGEX_TIMEOUT_SECONDS), move || {
        crate::text::dotenv_validate(
            &text_owned,
            allow_export,
            &key_pattern_owned,
            &duplicate_policy_owned,
        )
    }) {
        Ok(r) => r,
        Err(_timeout) => {
            return ToolResponse::error(
                "timeout",
                "Regex execution exceeded time limit (possible ReDoS)",
                Some(vec!["Try a simpler key_pattern or shorter text".to_string()]),
                Some("dotenv_validate"),
            )
        }
    };

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
            return ToolResponse::error(
                "invalid_arguments",
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
        return ToolResponse::error(
            "input_too_large",
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("ini_validate"),
        );
    }

    let valid_policies = ["warn", "error", "allow"];
    if !valid_policies.contains(&duplicate_policy) {
        return ToolResponse::error(
            "invalid_arguments",
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
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
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
        return ToolResponse::error(
            "input_too_large",
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("config_preflight"),
        );
    }

    let valid_formats = ["auto", "json", "toml", "dotenv", "ini", "cargo_toml"];
    if !valid_formats.contains(&format) {
        return ToolResponse::error(
            "invalid_arguments",
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
    let mut machine_codes: Vec<String> = Vec::new();
    let mut verdict = "valid";

    match detected_format {
        "json" => {
            let vj_result =
                crate::tools::validation::validate_json(&serde_json::json!({"text": text}));
            if let Some(ref r) = vj_result.result {
                subresults.insert("validate_json".to_string(), r.clone());
                let valid = r.get("valid").and_then(|v| v.as_bool()).unwrap_or(false);
                if !valid {
                    verdict = "invalid";
                    machine_codes.push("CONFIG_PARSE_FAILED".to_string());
                    findings.push(serde_json::json!({
                        "code": "JSON_PARSE_ERROR",
                        "severity": "error",
                        "message": r.get("error").and_then(|v| v.as_str()).unwrap_or("Invalid JSON"),
                    }));
                } else if let Some(sch) = schema {
                    let vs_args = serde_json::json!({"text": text, "schema": sch});
                    let vs_result = crate::tools::validation::validate_schema_light_tool(&vs_args);
                    if let Some(ref vr) = vs_result.result {
                        subresults.insert("validate_schema_light".to_string(), vr.clone());
                        let vs_valid = vr.get("valid").and_then(|v| v.as_bool()).unwrap_or(true);
                        if !vs_valid {
                            machine_codes.push("CONFIG_SCHEMA_MISMATCH".to_string());
                            verdict = "valid_with_warnings";
                            if let Some(violations) =
                                vr.get("violations").and_then(|v| v.as_array())
                            {
                                for violation in violations {
                                    let msg = violation
                                        .get("message")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("Schema violation");
                                    findings.push(serde_json::json!({
                                        "code": "SCHEMA_ERROR",
                                        "severity": if strict { "error" } else { "warn" },
                                        "message": msg,
                                    }));
                                }
                            } else {
                                findings.push(serde_json::json!({
                                    "code": "SCHEMA_ERROR",
                                    "severity": if strict { "error" } else { "warn" },
                                    "message": "Schema validation failed",
                                }));
                            }
                        }
                    }
                }
                // Optionally canonicalize
                if verdict != "invalid" {
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
                machine_codes.push("CONFIG_PARSE_FAILED".to_string());
                findings.push(serde_json::json!({
                    "code": "CONFIG_ERROR",
                    "severity": "error",
                    "message": e,
                }));
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
                    verdict = "invalid";
                    machine_codes.push("CONFIG_PARSE_FAILED".to_string());
                    findings.push(serde_json::json!({
                        "code": "TOML_PARSE_ERROR",
                        "severity": "error",
                        "message": r.get("error").and_then(|v| v.as_str()).unwrap_or("Invalid TOML"),
                    }));
                } else {
                    let ts_result =
                        crate::mcp::tools::toml_shape_tool(&serde_json::json!({"text": text}));
                    if let Some(ref r) = ts_result.result {
                        subresults.insert("toml_shape".to_string(), r.clone());
                    }
                }
            } else if let Some(ref e) = vt_result.error {
                machine_codes.push("CONFIG_PARSE_FAILED".to_string());
                findings.push(serde_json::json!({
                    "code": "CONFIG_ERROR",
                    "severity": "error",
                    "message": e,
                }));
            }
        }
        "dotenv" => {
            let dv_result = dotenv_validate(&serde_json::json!({"text": text}));
            if let Some(ref r) = dv_result.result {
                subresults.insert("dotenv_validate".to_string(), r.clone());
                let parse_ok = r.get("parse_ok").and_then(|v| v.as_bool()).unwrap_or(false);
                if !parse_ok {
                    verdict = "invalid";
                    machine_codes.push("CONFIG_PARSE_FAILED".to_string());
                    if let Some(dv_findings) = r.get("findings").and_then(|v| v.as_array()) {
                        for err in dv_findings {
                            findings.push(serde_json::json!({
                                "code": "DOTENV_ERROR",
                                "severity": "error",
                                "message": err.as_str().unwrap_or("Invalid dotenv format"),
                            }));
                        }
                    } else {
                        findings.push(serde_json::json!({
                            "code": "DOTENV_ERROR",
                            "severity": "error",
                            "message": "Invalid dotenv format",
                        }));
                    }
                }
            } else if let Some(ref e) = dv_result.error {
                machine_codes.push("CONFIG_PARSE_FAILED".to_string());
                findings.push(serde_json::json!({
                    "code": "CONFIG_ERROR",
                    "severity": "error",
                    "message": e,
                }));
            }
        }
        "ini" => {
            let iv_result = ini_validate(&serde_json::json!({"text": text}));
            if let Some(ref r) = iv_result.result {
                subresults.insert("ini_validate".to_string(), r.clone());
                let parse_ok = r.get("parse_ok").and_then(|v| v.as_bool()).unwrap_or(false);
                if !parse_ok {
                    verdict = "invalid";
                    machine_codes.push("CONFIG_PARSE_FAILED".to_string());
                    if let Some(iv_findings) = r.get("findings").and_then(|v| v.as_array()) {
                        for err in iv_findings {
                            findings.push(serde_json::json!({
                                "code": "INI_ERROR",
                                "severity": "error",
                                "message": err.as_str().unwrap_or("Invalid INI format"),
                            }));
                        }
                    } else {
                        findings.push(serde_json::json!({
                            "code": "INI_ERROR",
                            "severity": "error",
                            "message": "Invalid INI format",
                        }));
                    }
                }
            } else if let Some(ref e) = iv_result.error {
                machine_codes.push("CONFIG_PARSE_FAILED".to_string());
                findings.push(serde_json::json!({
                    "code": "CONFIG_ERROR",
                    "severity": "error",
                    "message": e,
                }));
            }
        }
        "cargo_toml" => {
            let ct_result =
                crate::tools::cargo::cargo_toml_inspect(&serde_json::json!({"text": text}));
            if let Some(ref r) = ct_result.result {
                subresults.insert("cargo_toml_inspect".to_string(), r.clone());
                let parse_ok = r.get("parse_ok").and_then(|v| v.as_bool()).unwrap_or(false);
                if !parse_ok {
                    verdict = "invalid";
                    machine_codes.push("CONFIG_PARSE_FAILED".to_string());
                    findings.push(serde_json::json!({
                        "code": "CARGO_PARSE_ERROR",
                        "severity": "error",
                        "message": "Cargo.toml parse failed",
                    }));
                } else {
                    if let Some(ct_findings) = r.get("findings").and_then(|v| v.as_array()) {
                        for f in ct_findings {
                            findings.push(serde_json::json!({
                                "code": f.get("code").and_then(|v| v.as_str()).unwrap_or("CARGO_NOTE"),
                                "severity": f.get("severity").and_then(|v| v.as_str()).unwrap_or("info"),
                                "message": f.get("message").and_then(|v| v.as_str()).unwrap_or(""),
                            }));
                        }
                    }
                }
            } else if let Some(ref e) = ct_result.error {
                machine_codes.push("CONFIG_PARSE_FAILED".to_string());
                findings.push(serde_json::json!({
                    "code": "CONFIG_ERROR",
                    "severity": "error",
                    "message": e,
                }));
            }
        }
        _ => unreachable!(),
    }

    let parse_ok = verdict != "invalid";

    let machine_code = if !parse_ok {
        machine_codes
            .first()
            .cloned()
            .unwrap_or_else(|| "CONFIG_PARSE_FAILED".to_string())
    } else if !findings.is_empty() {
        machine_codes
            .first()
            .cloned()
            .unwrap_or_else(|| "CONFIG_HAS_WARNINGS".to_string())
    } else {
        "CONFIG_OK".to_string()
    };

    let summary = format!(
        "{} config: {} ({} finding(s))",
        detected_format,
        verdict,
        findings.len()
    );

    let mut result = serde_json::json!({
        "valid": parse_ok,
        "verdict": verdict,
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
    resp = resp.with_machine_code(&machine_code);
    if !findings.is_empty() {
        resp = resp.with_findings(findings);
    }
    resp
}
