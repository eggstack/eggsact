use crate::mcp::machine_codes;
use crate::mcp::schemas::ToolResponse;
use crate::tools::helpers::*;
use serde_json::Value;

pub fn shell_split(args: &Value) -> ToolResponse {
    let command = match args.get("command").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'command' parameter",
                None,
                Some("shell_split"),
            )
        }
    };
    let shell = args
        .get("shell")
        .and_then(|v| v.as_str())
        .unwrap_or("posix");
    let detect_risky_features = args
        .get("detect_risky_features")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    if command.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Command exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("shell_split"),
        );
    }

    let valid_shells = ["posix"];
    if !valid_shells.contains(&shell) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported shell: {}", shell),
            Some(vec!["Use one of: posix".to_string()]),
            Some("shell_split"),
        );
    }

    let result = crate::text::shell_split(command, shell, detect_risky_features);

    ToolResponse::success(
        serde_json::json!({
            "parse_ok": result.parse_ok,
            "argv": result.argv,
            "argc": result.argc,
            "features": {
                "has_pipe": result.features.has_pipe,
                "has_redirection": result.features.has_redirection,
                "has_command_substitution": result.features.has_command_substitution,
                "has_variable_expansion": result.features.has_variable_expansion,
                "has_glob_pattern": result.features.has_glob_pattern,
                "has_control_operator": result.features.has_control_operator,
                "has_unbalanced_quotes": result.features.has_unbalanced_quotes,
            },
            "findings": result.findings,
        }),
        Some("shell_split"),
    )
    .with_tool("shell_split")
}

pub fn shell_quote_join(args: &Value) -> ToolResponse {
    let argv_raw = match args.get("argv").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'argv' parameter",
                None,
                Some("shell_quote_join"),
            )
        }
    };
    let shell = args
        .get("shell")
        .and_then(|v| v.as_str())
        .unwrap_or("posix");

    if argv_raw.len() > MAX_LIST_ITEMS {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INVALID_ARGUMENTS,
            &format!("argv length {} exceeds MAX_LIST_ITEMS", argv_raw.len()),
            None,
            Some("shell_quote_join"),
        );
    }

    let non_str_indices: Vec<usize> = argv_raw
        .iter()
        .enumerate()
        .filter(|(_, v)| !v.is_string())
        .map(|(i, _)| i)
        .collect();
    if !non_str_indices.is_empty() {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            "All argv elements must be strings",
            Some(vec![format!(
                "Non-string items at indices: {:?}",
                &non_str_indices[..5.min(non_str_indices.len())]
            )]),
            Some("shell_quote_join"),
        );
    }
    let oversized_indices: Vec<usize> = argv_raw
        .iter()
        .enumerate()
        .filter(|(_, v)| {
            v.as_str()
                .is_some_and(|s| s.chars().count() > MAX_TEXT_LENGTH)
        })
        .map(|(i, _)| i)
        .collect();
    if !oversized_indices.is_empty() {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INVALID_ARGUMENTS,
            &format!("argv items exceed max length {}", MAX_TEXT_LENGTH),
            Some(vec![format!(
                "Oversized items at indices: {:?}",
                &oversized_indices[..5.min(oversized_indices.len())]
            )]),
            Some("shell_quote_join"),
        );
    }
    let mut argv: Vec<String> = Vec::with_capacity(argv_raw.len());
    for v in argv_raw.iter() {
        if let Some(s) = v.as_str() {
            argv.push(s.to_string());
        }
    }

    let valid_shells = ["posix"];
    if !valid_shells.contains(&shell) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported shell: {}", shell),
            Some(vec![format!("Use one of: {}", valid_shells.join(", "))]),
            Some("shell_quote_join"),
        );
    }

    let result = crate::text::shell_quote_join(&argv, shell);

    ToolResponse::success(
        serde_json::json!({
            "command": result.command,
            "roundtrip_ok": result.roundtrip_ok,
            "findings": result.findings,
        }),
        Some("shell_quote_join"),
    )
    .with_tool("shell_quote_join")
}

pub fn argv_compare(args: &Value) -> ToolResponse {
    let left_command = args.get("left_command").and_then(|v| v.as_str());
    let right_command = args.get("right_command").and_then(|v| v.as_str());
    let left_argv = match args.get("left_argv").and_then(|v| v.as_array()) {
        Some(arr) => {
            if arr.len() > MAX_LIST_ITEMS {
                return ToolResponse::error_with_code(
                    "input_too_large",
                    machine_codes::INVALID_ARGUMENTS,
                    &format!("left_argv length {} exceeds {}", arr.len(), MAX_LIST_ITEMS),
                    None,
                    Some("argv_compare"),
                );
            }
            let non_str: Vec<usize> = arr
                .iter()
                .enumerate()
                .filter(|(_, v)| !v.is_string())
                .map(|(i, _)| i)
                .collect();
            if !non_str.is_empty() {
                return ToolResponse::error_with_code(
                    "invalid_arguments",
                    machine_codes::INVALID_ARGUMENTS,
                    "All left_argv elements must be strings",
                    Some(vec![format!(
                        "Non-string items at indices: {:?}",
                        &non_str[..5.min(non_str.len())]
                    )]),
                    Some("argv_compare"),
                );
            }
            let oversized: Vec<usize> = arr
                .iter()
                .enumerate()
                .filter(|(_, v)| {
                    v.as_str()
                        .is_some_and(|s| s.chars().count() > MAX_TEXT_LENGTH)
                })
                .map(|(i, _)| i)
                .collect();
            if !oversized.is_empty() {
                return ToolResponse::error_with_code(
                    "input_too_large",
                    machine_codes::INVALID_ARGUMENTS,
                    &format!("left_argv items exceed max length {}", MAX_TEXT_LENGTH),
                    Some(vec![format!(
                        "Oversized items at indices: {:?}",
                        &oversized[..5.min(oversized.len())]
                    )]),
                    Some("argv_compare"),
                );
            }
            Some(
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect::<Vec<_>>(),
            )
        }
        None => None,
    };
    let right_argv = match args.get("right_argv").and_then(|v| v.as_array()) {
        Some(arr) => {
            if arr.len() > MAX_LIST_ITEMS {
                return ToolResponse::error_with_code(
                    "input_too_large",
                    machine_codes::INVALID_ARGUMENTS,
                    &format!("right_argv length {} exceeds {}", arr.len(), MAX_LIST_ITEMS),
                    None,
                    Some("argv_compare"),
                );
            }
            let non_str: Vec<usize> = arr
                .iter()
                .enumerate()
                .filter(|(_, v)| !v.is_string())
                .map(|(i, _)| i)
                .collect();
            if !non_str.is_empty() {
                return ToolResponse::error_with_code(
                    "invalid_arguments",
                    machine_codes::INVALID_ARGUMENTS,
                    "All right_argv elements must be strings",
                    Some(vec![format!(
                        "Non-string items at indices: {:?}",
                        &non_str[..5.min(non_str.len())]
                    )]),
                    Some("argv_compare"),
                );
            }
            let oversized: Vec<usize> = arr
                .iter()
                .enumerate()
                .filter(|(_, v)| {
                    v.as_str()
                        .is_some_and(|s| s.chars().count() > MAX_TEXT_LENGTH)
                })
                .map(|(i, _)| i)
                .collect();
            if !oversized.is_empty() {
                return ToolResponse::error_with_code(
                    "input_too_large",
                    machine_codes::INVALID_ARGUMENTS,
                    &format!("right_argv items exceed max length {}", MAX_TEXT_LENGTH),
                    Some(vec![format!(
                        "Oversized items at indices: {:?}",
                        &oversized[..5.min(oversized.len())]
                    )]),
                    Some("argv_compare"),
                );
            }
            Some(
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect::<Vec<_>>(),
            )
        }
        None => None,
    };
    let shell = args
        .get("shell")
        .and_then(|v| v.as_str())
        .unwrap_or("posix");

    let valid_shells = ["posix"];
    if !valid_shells.contains(&shell) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported shell: {}", shell),
            Some(vec![format!("Use one of: {}", valid_shells.join(", "))]),
            Some("argv_compare"),
        );
    }

    // XOR validation: each side must be either a *_command OR an *_argv, not both (and not neither).
    let left_both = left_command.is_some() == left_argv.is_some();
    let right_both = right_command.is_some() == right_argv.is_some();
    if left_both {
        let msg = if left_command.is_some() && left_argv.is_some() {
            "Provide exactly one of left_command or left_argv, not both"
        } else {
            "Provide exactly one of left_command or left_argv"
        };
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            msg,
            None,
            Some("argv_compare"),
        );
    }
    if right_both {
        let msg = if right_command.is_some() && right_argv.is_some() {
            "Provide exactly one of right_command or right_argv, not both"
        } else {
            "Provide exactly one of right_command or right_argv"
        };
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            msg,
            None,
            Some("argv_compare"),
        );
    }

    if let Some(cmd) = left_command {
        if cmd.chars().count() > MAX_TEXT_LENGTH {
            return ToolResponse::error_with_code(
                "input_too_large",
                machine_codes::INVALID_ARGUMENTS,
                "Left command exceeds MAX_TEXT_LENGTH",
                None,
                Some("argv_compare"),
            );
        }
    }
    if let Some(cmd) = right_command {
        if cmd.chars().count() > MAX_TEXT_LENGTH {
            return ToolResponse::error_with_code(
                "input_too_large",
                machine_codes::INVALID_ARGUMENTS,
                "Right command exceeds MAX_TEXT_LENGTH",
                None,
                Some("argv_compare"),
            );
        }
    }

    let left_ref = left_command;
    let right_ref = right_command;
    let left_argv_ref = left_argv.as_deref();
    let right_argv_ref = right_argv.as_deref();

    let result =
        crate::text::argv_compare(left_ref, right_ref, left_argv_ref, right_argv_ref, shell);

    ToolResponse::success(
        serde_json::json!({
            "argv_equal": result.argv_equal,
            "left_argv": result.left_argv,
            "right_argv": result.right_argv,
            "first_difference": result.first_difference,
            "findings": result.findings,
        }),
        Some("argv_compare"),
    )
    .with_tool("argv_compare")
}

pub fn command_preflight(args: &Value) -> ToolResponse {
    let command = match args.get("command").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'command' parameter",
                None,
                Some("command_preflight"),
            )
        }
    };
    let platform = args
        .get("platform")
        .and_then(|v| v.as_str())
        .unwrap_or("posix");
    let policy = args
        .get("policy")
        .and_then(|v| v.as_str())
        .unwrap_or("default");
    let _working_directory = args.get("working_directory").and_then(|v| v.as_str());

    if command.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Command exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("command_preflight"),
        );
    }

    let valid_platforms = ["posix", "windows", "auto"];
    if !valid_platforms.contains(&platform) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported platform: {}", platform),
            Some(vec![format!("Use one of: {}", valid_platforms.join(", "))]),
            Some("command_preflight"),
        );
    }

    let valid_policies = ["default", "strict", "permissive"];
    if !valid_policies.contains(&policy) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported policy: {}", policy),
            Some(vec![format!("Use one of: {}", valid_policies.join(", "))]),
            Some("command_preflight"),
        );
    }

    let mut subresults = serde_json::Map::new();
    let mut findings: Vec<serde_json::Value> = Vec::new();
    let mut code_list: Vec<String> = Vec::new();

    // 1. Always call shell_split
    if platform == "windows" {
        return ToolResponse::error_with_code(
            "unsupported_platform",
            machine_codes::UNSUPPORTED_FEATURE,
            "Windows shell splitting is not supported; only 'posix' is available",
            Some(vec!["Use platform='posix' or platform='auto'".to_string()]),
            Some("command_preflight"),
        );
    }
    let shell = "posix";
    let ss_args = serde_json::json!({"command": command, "shell": shell});
    let ss_result = shell_split(&ss_args);
    if let Some(ref r) = ss_result.result {
        subresults.insert(
            "shell_split".to_string(),
            serde_json::json!({
                "argv": r.get("argv").cloned().unwrap_or(serde_json::json!([])),
                "features": r.get("features").cloned().unwrap_or(serde_json::json!({})),
            }),
        );
        if let Some(features) = r.get("features") {
            if let Some(obj) = features.as_object() {
                let risky: Vec<&String> = obj
                    .iter()
                    .filter(|(_, v)| v.as_bool() == Some(true))
                    .map(|(k, _)| k)
                    .collect();
                for rf in &risky {
                    let sev = if policy == "strict" { "error" } else { "warn" };
                    findings.push(serde_json::json!({
                        "code": "RISKY_SHELL_FEATURE",
                        "severity": sev,
                        "message": rf,
                    }));
                }
                if !risky.is_empty() && !code_list.contains(&machine_codes::SHELL_RISK.to_string())
                {
                    code_list.push(machine_codes::SHELL_RISK.to_string());
                }
            }
        }
    } else if let Some(ref e) = ss_result.error {
        code_list.push(machine_codes::SHELL_PARSE_ERROR.to_string());
        findings.push(serde_json::json!({
            "code": "SHELL_PARSE_ERROR",
            "severity": "error",
            "message": e,
        }));
    }

    // 2. Check for regex-like args in the command
    let looks_like_regex = command.contains("grep")
        || command.contains("sed")
        || command.contains("awk")
        || command.to_lowercase().contains("regex");
    let argv: Vec<String> = subresults
        .get("shell_split")
        .and_then(|r| r.get("argv"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    if looks_like_regex && !argv.is_empty() {
        let regex_metachars: std::collections::HashSet<char> = ".*+?[]|^$\\(){}".chars().collect();
        let regex_args: Vec<&String> = argv
            .iter()
            .filter(|arg| {
                !arg.starts_with('-')
                    && !arg.is_empty()
                    && arg.chars().any(|c| regex_metachars.contains(&c))
            })
            .collect();
        for pattern in &regex_args {
            let rs_args = serde_json::json!({"pattern": pattern.as_str()});
            let rs_result = crate::tools::regex_safety_check_tool(&rs_args);
            if let Some(ref r) = rs_result.result {
                let risk = r.get("risk").and_then(|v| v.as_str()).unwrap_or("none");
                let mut has_rs_findings = false;
                if let Some(findings_arr) = r.get("findings").and_then(|v| v.as_array()) {
                    has_rs_findings = !findings_arr.is_empty();
                    for f in findings_arr {
                        let sev = if risk != "none" { "warn" } else { "info" };
                        let kind = f
                            .get("kind")
                            .and_then(|v| v.as_str())
                            .unwrap_or("REGEX_RISK");
                        findings.push(serde_json::json!({
                            "code": kind.to_uppercase(),
                            "severity": sev,
                            "message": f.get("message").and_then(|v| v.as_str()).unwrap_or(""),
                        }));
                    }
                }
                if has_rs_findings
                    && risk != "none"
                    && !code_list.contains(&machine_codes::REGEX_RISK.to_string())
                {
                    code_list.push(machine_codes::REGEX_RISK.to_string());
                }
                let regex_summary = serde_json::json!({
                    "pattern": pattern.as_str(),
                    "findings_count": r.get("findings").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0),
                    "risk": risk,
                });
                let regex_subresults = subresults
                    .entry("regex_safety_check".to_string())
                    .or_insert_with(|| serde_json::json!([]));
                if let Some(items) = regex_subresults.as_array_mut() {
                    items.push(regex_summary);
                } else {
                    *regex_subresults = serde_json::json!([regex_summary]);
                }
            }
        }
    }

    // 3. Determine verdict
    let has_error = findings
        .iter()
        .any(|f| f.get("severity").and_then(|v| v.as_str()) == Some("error"));
    let has_warn = findings
        .iter()
        .any(|f| f.get("severity").and_then(|v| v.as_str()) == Some("warn"));
    let verdict = if has_error {
        "block"
    } else if has_warn {
        "review"
    } else {
        "allow"
    };

    // Build primary machine_code
    let unique_codes: Vec<String> = code_list.into_iter().fold(Vec::new(), |mut acc, c| {
        if !acc.contains(&c) {
            acc.push(c);
        }
        acc
    });
    let primary_code = unique_codes
        .first()
        .cloned()
        .unwrap_or_else(|| machine_codes::COMMAND_OK.to_string());

    let summary = format!("Command {} ({} finding(s))", verdict, findings.len());

    let mut result = serde_json::json!({
        "verdict": verdict,
        "command": command,
        "platform": platform,
        "policy": policy,
        "findings": findings,
        "machine_code": primary_code,
        "summary": summary,
    });
    if let Some(wd) = _working_directory {
        result["working_directory"] = serde_json::json!(wd);
    }
    if !subresults.is_empty() {
        result["subresults"] = serde_json::Value::Object(subresults);
    }

    let mut resp =
        ToolResponse::success(result, Some("command_preflight")).with_tool("command_preflight");
    resp = resp.with_machine_code(&primary_code);
    if !findings.is_empty() {
        resp = resp.with_findings(findings);
    }
    resp
}
