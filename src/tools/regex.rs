use crate::mcp::machine_codes;
use crate::mcp::schemas::ToolResponse;
use crate::tools::helpers::*;
use serde_json::Value;
use std::time::Duration;

pub fn validate_regex(args: &Value) -> ToolResponse {
    let pattern = match args.get("pattern") {
        Some(v) => match v.as_str() {
            Some(s) => s,
            None => {
                return ToolResponse::error_with_code(
                    "invalid_arguments",
                    machine_codes::INVALID_ARGUMENTS,
                    &format!("pattern must be a string, got {}", json_type_name(v)),
                    None,
                    Some("validate_regex"),
                )
            }
        },
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "pattern must be a string, got NoneType",
                None,
                Some("validate_regex"),
            )
        }
    };
    let samples = match args.get("samples") {
        Some(Value::Array(arr)) => arr,
        Some(v) => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                &format!("samples must be a list, got {}", json_type_name(v)),
                None,
                Some("validate_regex"),
            )
        }
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "samples must be a list, got NoneType",
                None,
                Some("validate_regex"),
            )
        }
    };
    let flags = match args.get("flags") {
        Some(Value::Array(arr)) => {
            // Validate all flags are strings
            let non_str_flags: Vec<usize> = arr
                .iter()
                .enumerate()
                .filter(|(_, v)| !v.is_string())
                .map(|(i, _)| i)
                .collect();
            if !non_str_flags.is_empty() {
                return ToolResponse::error_with_code(
                    "invalid_arguments",
                    machine_codes::INVALID_ARGUMENTS,
                    "All flags must be strings",
                    Some(vec![format!(
                        "Non-string items at indices: {:?}",
                        &non_str_flags[..5.min(non_str_flags.len())]
                    )]),
                    Some("validate_regex"),
                );
            }
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        }
        Some(v) => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                &format!("flags must be a list, got {}", json_type_name(v)),
                None,
                Some("validate_regex"),
            )
        }
        None => Vec::new(),
    };
    let ignore_case = args
        .get("ignore_case")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let multiline = args
        .get("multiline")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let dotall = args
        .get("dotall")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let ascii = args.get("ascii").and_then(|v| v.as_bool()).unwrap_or(false);

    if samples.len() > MAX_REGEX_SAMPLES {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!(
                "Number of samples {} exceeds MAX_REGEX_SAMPLES {}",
                samples.len(),
                MAX_REGEX_SAMPLES
            ),
            Some(vec![format!(
                "Maximum {} samples allowed",
                MAX_REGEX_SAMPLES
            )]),
            Some("validate_regex"),
        );
    }

    if pattern.chars().count() > MAX_PATTERN_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!(
                "Pattern length {} exceeds MAX_PATTERN_LENGTH {}",
                pattern.chars().count(),
                MAX_PATTERN_LENGTH
            ),
            Some(vec![format!(
                "Maximum pattern length is {} characters",
                MAX_PATTERN_LENGTH
            )]),
            Some("validate_regex"),
        );
    }

    let mut total_chars: usize = 0;
    let non_str_indices: Vec<usize> = samples
        .iter()
        .enumerate()
        .filter(|(_, v)| !v.is_string())
        .map(|(i, _)| i)
        .collect();
    if !non_str_indices.is_empty() {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            "All samples must be strings",
            Some(vec![format!(
                "Non-string items at indices: {:?}",
                &non_str_indices[..5.min(non_str_indices.len())]
            )]),
            Some("validate_regex"),
        );
    }
    let long_samples: Vec<usize> = samples
        .iter()
        .enumerate()
        .filter(|(_, v)| {
            v.as_str()
                .is_some_and(|s| s.chars().count() > MAX_REGEX_SAMPLE_LENGTH)
        })
        .map(|(i, _)| i)
        .collect();
    if !long_samples.is_empty() {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!(
                "Sample(s) at indices {:?} exceed MAX_REGEX_SAMPLE_LENGTH {}",
                long_samples, MAX_REGEX_SAMPLE_LENGTH
            ),
            Some(vec![format!(
                "Maximum {} characters per sample",
                MAX_REGEX_SAMPLE_LENGTH
            )]),
            Some("validate_regex"),
        );
    }
    let sample_strs: Vec<String> = samples
        .iter()
        .map(|v| {
            let s = v.as_str().unwrap_or("");
            total_chars += s.chars().count();
            s.to_string()
        })
        .collect();

    if total_chars > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!(
                "Total sample size {} characters exceeds MAX_TEXT_LENGTH {}",
                total_chars, MAX_TEXT_LENGTH
            ),
            Some(vec![format!(
                "Maximum total {} characters across all samples",
                MAX_TEXT_LENGTH
            )]),
            Some("validate_regex"),
        );
    }

    let safety = crate::text::regex_safety_check(pattern);
    if safety.risk == "medium" || safety.risk == "high" {
        return ToolResponse::error_with_code(
            "unsafe_pattern",
            machine_codes::REGEX_UNSAFE,
            &format!(
                "Pattern has {} risk of catastrophic backtracking",
                safety.risk
            ),
            Some(vec![
                "Try a simpler pattern or break it into smaller parts".to_string(),
                "Use the regex_safety_check tool for detailed analysis and suggestions".to_string(),
            ]),
            Some("validate_regex"),
        );
    }

    let flags_clone: Option<Vec<String>> = if flags.is_empty() { None } else { Some(flags) };
    let pattern_owned = pattern.to_string();
    let samples_owned: Vec<String> = sample_strs;
    let result = match run_with_timeout(Duration::from_secs(REGEX_TIMEOUT_SECONDS), move || {
        let refs: Vec<&str> = samples_owned.iter().map(|s| s.as_str()).collect();
        crate::text::regex_test(
            &pattern_owned,
            &refs,
            flags_clone.as_ref(),
            ignore_case,
            multiline,
            dotall,
            ascii,
        )
    }) {
        Ok(r) => r,
        Err(_timeout) => {
            return ToolResponse::error_with_code(
                "timeout",
                machine_codes::REGEX_UNSAFE,
                "Regex execution exceeded time limit (possible ReDoS)",
                Some(vec!["Try a simpler pattern or fewer samples".to_string()]),
                Some("validate_regex"),
            )
        }
    };

    let flags_used = serde_json::json!({
        "ignore_case": ignore_case,
        "multiline": multiline,
        "dotall": dotall,
        "ascii": ascii,
    });

    let mut result_value = serde_json::json!({
        "valid_pattern": result.valid_pattern,
        "results": result.results,
        "flags_used": flags_used,
    });
    if let Some(ref err) = result.error {
        result_value["error"] = serde_json::json!(err);
    }
    if let Some(ref engine) = result.engine_used {
        result_value["engine_used"] = serde_json::json!(engine);
    }
    if let Some(ref dialect) = result.dialect {
        result_value["dialect"] = serde_json::json!(dialect);
    }
    if let Some(ref unsupported) = result.unsupported_features {
        result_value["unsupported_features"] = serde_json::json!(unsupported);
    }

    // Emit REGEX_UNSUPPORTED_FEATURE for unsupported dialect constructs
    let mut resp =
        ToolResponse::success(result_value, Some("validate_regex")).with_tool("validate_regex");
    if let Some(ref unsupported) = result.unsupported_features {
        if !unsupported.is_empty() {
            resp = resp.with_machine_code(machine_codes::REGEX_UNSUPPORTED_FEATURE);
        }
    }
    resp
}

pub fn regex_safety_check_tool(args: &Value) -> ToolResponse {
    let pattern = match args.get("pattern").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'pattern' parameter",
                None,
                Some("regex_safety_check"),
            )
        }
    };

    if pattern.chars().count() > MAX_PATTERN_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!("Pattern exceeds {} chars", MAX_PATTERN_LENGTH),
            Some(vec![format!(
                "Maximum pattern length is {} characters",
                MAX_PATTERN_LENGTH
            )]),
            Some("regex_safety_check"),
        );
    }

    let result = crate::text::regex_safety::regex_safety_check(pattern);
    let risk = result.risk.clone();
    let findings_list = result.findings.clone();
    let pattern_length = pattern.chars().count();

    let envelope_findings: Vec<serde_json::Value> = findings_list
        .iter()
        .map(|f| {
            serde_json::json!({
                "code": f.kind.to_uppercase(),
                "severity": "warn",
                "message": f.message.clone(),
                "details": {"pattern_length": pattern_length}
            })
        })
        .collect();

    let machine_code = if risk == "medium" || risk == "high" {
        Some(machine_codes::REGEX_UNSAFE.to_string())
    } else {
        None
    };

    let mut resp = ToolResponse::success(
        serde_json::json!({
            "valid_pattern": result.valid_pattern,
            "risk": result.risk,
            "findings": result.findings,
        }),
        Some("regex_safety_check"),
    )
    .with_tool("regex_safety_check");

    if !envelope_findings.is_empty() {
        resp = resp.with_findings(envelope_findings);
    }
    if let Some(ref code) = machine_code {
        resp = resp.with_machine_code(code);
    }
    resp
}

pub fn regex_finditer_tool(args: &Value) -> ToolResponse {
    let budget_ctx = crate::mcp::budget::for_handler(crate::mcp::budget::ToolBudget::MODERATE);

    let pattern = match _require_str(args, "pattern", "regex_finditer") {
        Ok(s) => s,
        Err(e) => return *e,
    };
    let text = match _require_str(args, "text", "regex_finditer") {
        Ok(s) => s,
        Err(e) => return *e,
    };
    let flags = match args.get("flags") {
        Some(Value::Array(arr)) => {
            let non_str_flags: Vec<usize> = arr
                .iter()
                .enumerate()
                .filter(|(_, v)| !v.is_string())
                .map(|(i, _)| i)
                .collect();
            if !non_str_flags.is_empty() {
                return ToolResponse::error_with_code(
                    "invalid_arguments",
                    machine_codes::INVALID_ARGUMENTS,
                    "All flags must be strings",
                    Some(vec![format!(
                        "Non-string items at indices: {:?}",
                        &non_str_flags[..5.min(non_str_flags.len())]
                    )]),
                    Some("regex_finditer"),
                );
            }
            Some(
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect::<Vec<_>>(),
            )
        }
        Some(v) => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                &format!("flags must be a list, got {}", json_type_name(v)),
                None,
                Some("regex_finditer"),
            )
        }
        None => None,
    };
    let max_matches = args
        .get("max_matches")
        .and_then(|v| v.as_u64())
        .unwrap_or(MAX_MATCHES_REGEX as u64) as usize;
    if max_matches < 1 {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("max_matches must be at least 1, got {}", max_matches),
            Some(vec!["Set max_matches to 1 or higher".to_string()]),
            Some("regex_finditer"),
        );
    }
    if max_matches > MAX_MATCHES_HARD_CAP {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!(
                "max_matches {} exceeds maximum of {}",
                max_matches, MAX_MATCHES_HARD_CAP
            ),
            Some(vec![format!(
                "Set max_matches to {} or lower",
                MAX_MATCHES_HARD_CAP
            )]),
            Some("regex_finditer"),
        );
    }
    let max_matches = max_matches.min(MAX_MATCHES_HARD_CAP);
    let include_line_column = args
        .get("include_line_column")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let include_groups = args
        .get("include_groups")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let char_count = text.chars().count();
    if char_count > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            Some(vec![format!(
                "Maximum input length is {} characters",
                MAX_TEXT_LENGTH
            )]),
            Some("regex_finditer"),
        );
    }

    if pattern.chars().count() > MAX_PATTERN_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!(
                "Pattern length {} exceeds MAX_PATTERN_LENGTH {}",
                pattern.chars().count(),
                MAX_PATTERN_LENGTH
            ),
            Some(vec![format!(
                "Maximum pattern length is {} characters",
                MAX_PATTERN_LENGTH
            )]),
            Some("regex_finditer"),
        );
    }

    if budget_ctx.should_stop() {
        return budget_ctx.check_should_stop("regex_finditer").unwrap_err();
    }

    let safety = crate::text::regex_safety_check(pattern);
    if safety.risk == "medium" || safety.risk == "high" {
        return ToolResponse::error_with_code(
            "unsafe_pattern",
            machine_codes::REGEX_UNSAFE,
            &format!(
                "Pattern has {} risk of catastrophic backtracking",
                safety.risk
            ),
            Some(vec![
                "Try a simpler pattern or break it into smaller parts".to_string(),
                "Use the regex_safety_check tool for detailed analysis and suggestions".to_string(),
            ]),
            Some("regex_finditer"),
        );
    }

    let pattern_owned = pattern.to_string();
    let text_owned = text.to_string();
    let flags_owned: Option<Vec<String>> = flags;
    let result = match run_with_timeout(Duration::from_secs(REGEX_TIMEOUT_SECONDS), move || {
        crate::text::validate::regex_finditer(
            &pattern_owned,
            &text_owned,
            flags_owned.as_ref(),
            max_matches,
            include_line_column,
            include_groups,
        )
    }) {
        Ok(r) => r,
        Err(_timeout) => {
            return ToolResponse::error_with_code(
                "timeout",
                machine_codes::REGEX_UNSAFE,
                "Regex execution exceeded time limit (possible ReDoS)",
                Some(vec![
                    "Try a simpler pattern or reduce max_matches".to_string()
                ]),
                Some("regex_finditer"),
            )
        }
    };

    if budget_ctx.should_stop() {
        return budget_ctx.check_should_stop("regex_finditer").unwrap_err();
    }

    let matches: Vec<serde_json::Value> = result
        .matches
        .iter()
        .map(|m| {
            let mut obj = serde_json::json!({
                "match": m.m,
                "span": m.span,
                "groups": m.groups,
                "groupdict": m.group_dict,
            });
            if let (Some(line), Some(column)) = (m.line, m.column) {
                obj["line"] = serde_json::json!(line);
                obj["column"] = serde_json::json!(column);
            }
            obj
        })
        .collect();

    let mut resp = ToolResponse::success(
        serde_json::json!({
            "valid_pattern": result.valid_pattern,
            "matches": matches,
            "truncated": result.truncated,
            "match_count": result.match_count,
            "error": result.error,
            "engine_used": result.engine_used,
            "dialect": result.dialect,
            "unsupported_features": result.unsupported_features,
        }),
        Some("regex_finditer"),
    )
    .with_tool("regex_finditer");

    // Emit REGEX_UNSUPPORTED_FEATURE for unsupported dialect constructs
    if let Some(ref unsupported) = result.unsupported_features {
        if !unsupported.is_empty() {
            resp = resp.with_machine_code(machine_codes::REGEX_UNSUPPORTED_FEATURE);
        }
    }
    resp
}
