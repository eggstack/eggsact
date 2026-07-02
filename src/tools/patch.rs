use crate::mcp::machine_codes;
use crate::mcp::schemas::{disposition, finding, severity, verdict, ToolResponse};
use crate::tools::helpers::*;
use serde_json::Value;

pub fn patch_apply_check(args: &Value) -> ToolResponse {
    let original_text_val = args.get("original_text");
    let original_text = match original_text_val.and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            let type_name = match original_text_val {
                Some(v) => json_type_name(v),
                None => "NoneType",
            };
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                &format!("original_text must be a string, got {}", type_name),
                None,
                Some("patch_apply_check"),
            );
        }
    };
    let patch_text_val = args.get("patch_text");
    let patch_text = match patch_text_val.and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            let type_name = match patch_text_val {
                Some(v) => json_type_name(v),
                None => "NoneType",
            };
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                &format!("patch_text must be a string, got {}", type_name),
                None,
                Some("patch_apply_check"),
            );
        }
    };
    let strict = args.get("strict").and_then(|v| v.as_bool()).unwrap_or(true);
    let return_result_fingerprint = args
        .get("return_result_fingerprint")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let return_result_text = args
        .get("return_result_text")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    const MAX_ORIGINAL_LENGTH: usize = 200_000;
    const MAX_PATCH_LENGTH: usize = 100_000;

    if original_text.chars().count() > MAX_ORIGINAL_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!(
                "Original text length {} exceeds maximum of {}",
                original_text.chars().count(),
                MAX_ORIGINAL_LENGTH
            ),
            Some(vec![format!(
                "Maximum original text length is {}",
                MAX_ORIGINAL_LENGTH
            )]),
            Some("patch_apply_check"),
        );
    }
    if patch_text.chars().count() > MAX_PATCH_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!(
                "Patch text length {} exceeds maximum of {}",
                patch_text.chars().count(),
                MAX_PATCH_LENGTH
            ),
            Some(vec![format!(
                "Maximum patch text length is {}",
                MAX_PATCH_LENGTH
            )]),
            Some("patch_apply_check"),
        );
    }

    let result = crate::text::patch_apply_check(
        original_text,
        patch_text,
        strict,
        return_result_fingerprint,
        return_result_text,
    );

    let mut findings: Vec<serde_json::Value> = Vec::new();
    for msg in &result.findings {
        findings.push(finding(
            "PATCH_FINDING",
            severity::MEDIUM,
            msg,
            Some(disposition::CAUTION),
            None,
        ));
    }
    if !result.patch_parse_ok {
        findings.push(finding(
            "PATCH_PARSE_FAILED",
            severity::HIGH,
            "Patch failed to parse",
            Some(disposition::BLOCKING),
            None,
        ));
    } else if !result.applies {
        findings.push(finding(
            "PATCH_FAILED",
            severity::HIGH,
            "Patch does not apply cleanly",
            Some(disposition::BLOCKING),
            None,
        ));
    }

    let (response_verdict, machine_code) = if !result.patch_parse_ok || !result.applies {
        (verdict::BLOCK, machine_codes::PATCH_FAILED)
    } else if result.hunks_failed > 0 {
        (verdict::REVIEW, machine_codes::PATCH_FAILED)
    } else {
        (verdict::ALLOW, machine_codes::EDIT_OK)
    };

    let mut resp = ToolResponse::success(
        serde_json::json!({
            "patch_parse_ok": result.patch_parse_ok,
            "applies": result.applies,
            "hunks_total": result.hunks_total,
            "hunks_applied": result.hunks_applied,
            "hunks_failed": result.hunks_failed,
            "failed_hunks": result.failed_hunks,
            "affected_line_ranges": result.affected_line_ranges,
            "newline_style_before": result.newline_style_before,
            "newline_style_after": result.newline_style_after,
            "result_fingerprint": result.result_fingerprint,
            "result_text": result.result_text,
            "verdict": response_verdict,
            "findings": findings,
        }),
        Some("patch_apply_check"),
    )
    .with_tool("patch_apply_check")
    .with_machine_code(machine_code)
    .with_verdict(response_verdict);
    if !findings.is_empty() {
        resp = resp.with_findings(findings);
    }
    resp
}

pub fn patch_summary(args: &Value) -> ToolResponse {
    let patch_text_val = args.get("patch_text");
    let patch_text = match patch_text_val.and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            let type_name = match patch_text_val {
                Some(v) => json_type_name(v),
                None => "NoneType",
            };
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                &format!("patch_text must be a string, got {}", type_name),
                None,
                Some("patch_summary"),
            );
        }
    };

    const MAX_PATCH_LENGTH: usize = 100_000;
    if patch_text.chars().count() > MAX_PATCH_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!(
                "Patch text length {} exceeds maximum of {}",
                patch_text.chars().count(),
                MAX_PATCH_LENGTH
            ),
            None,
            Some("patch_summary"),
        );
    }

    let result = crate::text::patch_summary(patch_text);

    let mut findings: Vec<serde_json::Value> = Vec::new();
    for msg in &result.findings {
        findings.push(finding(
            "PATCH_SUMMARY_FINDING",
            severity::INFO,
            msg,
            Some(disposition::INFORMATIONAL),
            None,
        ));
    }

    let has_warnings = result.binary_patch_detected || !result.renames_detected.is_empty();
    let response_verdict = if has_warnings {
        verdict::REVIEW
    } else {
        verdict::ALLOW
    };
    let machine_code = if has_warnings {
        machine_codes::PATCH_FAILED
    } else {
        machine_codes::EDIT_OK
    };

    let mut resp = ToolResponse::success(
        serde_json::json!({
            "files_changed": result.files_changed,
            "hunks_total": result.hunks_total,
            "additions": result.additions,
            "deletions": result.deletions,
            "renames_detected": result.renames_detected,
            "binary_patch_detected": result.binary_patch_detected,
            "line_ranges_by_file": result.line_ranges_by_file,
            "findings": findings,
            "verdict": response_verdict,
        }),
        Some("patch_summary"),
    )
    .with_tool("patch_summary")
    .with_machine_code(machine_code)
    .with_verdict(response_verdict);
    if !findings.is_empty() {
        resp = resp.with_findings(findings);
    }
    resp
}

pub fn edit_preflight(args: &Value) -> ToolResponse {
    let original = match args.get("original").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'original' parameter",
                None,
                Some("edit_preflight"),
            )
        }
    };
    let replacement_mode = args
        .get("replacement_mode")
        .and_then(|v| v.as_str())
        .unwrap_or("literal");
    let strict = args.get("strict").and_then(|v| v.as_bool()).unwrap_or(true);
    let expected_fingerprint = args.get("expected_fingerprint").and_then(|v| v.as_str());
    let file_path = args.get("file_path").and_then(|v| v.as_str());
    let workspace_root = args.get("workspace_root").and_then(|v| v.as_str());
    let newline_policy = args
        .get("newline_policy")
        .and_then(|v| v.as_str())
        .unwrap_or("skip");
    let unicode_policy = args
        .get("unicode_policy")
        .and_then(|v| v.as_str())
        .unwrap_or("skip");

    if original.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!("Original text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("edit_preflight"),
        );
    }

    let valid_modes = ["literal", "patch", "line_range"];
    if !valid_modes.contains(&replacement_mode) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!(
                "replacement_mode must be one of: {}",
                valid_modes.join(", ")
            ),
            None,
            Some("edit_preflight"),
        );
    }

    let mut subresults = serde_json::Map::new();
    let mut findings: Vec<serde_json::Value> = Vec::new();

    match replacement_mode {
        "literal" => {
            let old = match args.get("old").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => {
                    return ToolResponse::error_with_code(
                        "invalid_arguments",
                        machine_codes::INVALID_ARGUMENTS,
                        "literal mode requires both 'old' and 'new'",
                        None,
                        Some("edit_preflight"),
                    )
                }
            };
            let new = match args.get("new").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => {
                    return ToolResponse::error_with_code(
                        "invalid_arguments",
                        machine_codes::INVALID_ARGUMENTS,
                        "literal mode requires both 'old' and 'new'",
                        None,
                        Some("edit_preflight"),
                    )
                }
            };
            let tr_args = serde_json::json!({
                "text": original,
                "old": old,
                "new": new,
                "mode": "exact",
            });
            let tr_result = crate::tools::text_replace_check_tool(&tr_args);
            if let Some(ref r) = tr_result.result {
                subresults.insert("text_replace_check".to_string(), r.clone());
                let match_count = r.get("match_count").and_then(|v| v.as_u64()).unwrap_or(0);
                if match_count == 0 {
                    findings.push(finding(
                        "NO_MATCH",
                        severity::HIGH,
                        "old text not found in original",
                        Some(disposition::BLOCKING),
                        None,
                    ));
                } else if match_count > 1 {
                    findings.push(finding(
                        "MULTIPLE_MATCHES",
                        severity::MEDIUM,
                        &format!("Found {} matches; use allow_multiple=true", match_count),
                        Some(disposition::CAUTION),
                        None,
                    ));
                }
            }
        }
        "patch" => {
            let patch_text = match args.get("patch").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => {
                    return ToolResponse::error_with_code(
                        "invalid_arguments",
                        machine_codes::INVALID_ARGUMENTS,
                        "patch mode requires 'patch'",
                        None,
                        Some("edit_preflight"),
                    )
                }
            };
            let pa_args = serde_json::json!({
                "original_text": original,
                "patch_text": patch_text,
                "strict": strict,
                "return_result_fingerprint": true,
                "return_result_text": false,
            });
            let pa_result = patch_apply_check(&pa_args);
            match pa_result {
                ToolResponse {
                    error: Some(ref e), ..
                } => {
                    findings.push(finding(
                        "PATCH_ERROR",
                        severity::HIGH,
                        e,
                        Some(disposition::BLOCKING),
                        None,
                    ));
                }
                ToolResponse {
                    result: Some(ref r),
                    ..
                } => {
                    subresults.insert("patch_apply_check".to_string(), r.clone());
                    if let Some(applies) = r.get("applies").and_then(|v| v.as_bool()) {
                        if !applies {
                            findings.push(finding(
                                "PATCH_FAILED",
                                severity::HIGH,
                                "Patch does not apply cleanly",
                                Some(disposition::BLOCKING),
                                None,
                            ));
                        }
                    }
                }
                _ => {}
            }
        }
        "line_range" => {
            let start_line = match args.get("start_line").and_then(|v| v.as_u64()) {
                Some(n) => n as usize,
                None => {
                    return ToolResponse::error_with_code(
                        "invalid_arguments",
                        machine_codes::INVALID_ARGUMENTS,
                        "line_range mode requires 'start_line' and 'end_line'",
                        None,
                        Some("edit_preflight"),
                    )
                }
            };
            let end_line = match args.get("end_line").and_then(|v| v.as_u64()) {
                Some(n) => n as usize,
                None => {
                    return ToolResponse::error_with_code(
                        "invalid_arguments",
                        machine_codes::INVALID_ARGUMENTS,
                        "line_range mode requires 'start_line' and 'end_line'",
                        None,
                        Some("edit_preflight"),
                    )
                }
            };
            let lr_args = serde_json::json!({
                "text": original,
                "start_line": start_line,
                "end_line": end_line,
            });
            let lr_result = crate::tools::text::line_range_extract_tool(&lr_args);
            if let Some(ref r) = lr_result.result {
                subresults.insert("line_range_extract".to_string(), r.clone());
                if let Some(valid_range) = r.get("valid_range").and_then(|v| v.as_bool()) {
                    if !valid_range {
                        findings.push(finding(
                            "INVALID_RANGE",
                            severity::HIGH,
                            "Invalid line range",
                            Some(disposition::BLOCKING),
                            None,
                        ));
                    }
                }
            }
        }
        _ => unreachable!(),
    }

    // Check expected_fingerprint if provided (matching Python per-mode behavior)
    // Python:
    //   literal mode: fingerprints original text
    //   patch mode:   fingerprints result_fingerprint from patch_apply_check
    //   line_range mode: fingerprints fingerprint from line_range_extract
    let mut fingerprint_result: Option<Value> = None;
    if let Some(fp) = expected_fingerprint {
        let (actual_fp, fp_source, newline_style) = if replacement_mode == "patch" {
            // Use result_fingerprint from patch_apply_check subresult
            let fp_val = subresults
                .get("patch_apply_check")
                .and_then(|r| r.get("result_fingerprint"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let nl = subresults
                .get("patch_apply_check")
                .and_then(|r| r.get("newline_style_after"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            (fp_val.to_string(), "patch_apply_check", nl)
        } else if replacement_mode == "line_range" {
            // Use fingerprint from line_range_extract subresult
            let fp_val = subresults
                .get("line_range_extract")
                .and_then(|r| r.get("fingerprint"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let nl = subresults
                .get("line_range_extract")
                .and_then(|r| r.get("newline_style"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            (fp_val.to_string(), "line_range_extract", nl)
        } else {
            // literal mode: fingerprint original text
            let fp_args = serde_json::json!({"text": original});
            let fp_result = crate::tools::text::text_fingerprint_tool(&fp_args);
            let fp_val = fp_result
                .result
                .as_ref()
                .and_then(|r| r.get("sha256"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let nl = fp_result
                .result
                .as_ref()
                .and_then(|r| r.get("newline_style"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            subresults.insert(
                "text_fingerprint".to_string(),
                fp_result.result.unwrap_or(serde_json::Value::Null),
            );
            (fp_val, "text_fingerprint", nl)
        };
        fingerprint_result = Some(serde_json::json!({
            "sha256": actual_fp,
            "newline_style": newline_style,
        }));
        if actual_fp != fp {
            findings.push(finding(
                "FINGERPRINT_MISMATCH",
                severity::MEDIUM,
                &format!("Expected {}, got {} (from {})", fp, actual_fp, fp_source),
                Some(disposition::CAUTION),
                None,
            ));
        }
    }

    // --- Path scope check (when file_path + workspace_root are provided) ---
    let mut path_scope_result: Option<Value> = None;
    if let (Some(fp), Some(wr)) = (file_path, workspace_root) {
        let ps_args = serde_json::json!({
            "root": wr,
            "target": fp,
        });
        let ps_resp = crate::tools::path::path_scope_check(&ps_args);
        if let Some(ref r) = ps_resp.result {
            path_scope_result = Some(r.clone());
            subresults.insert("path_scope_check".to_string(), r.clone());
            let inside_root = r
                .get("inside_root")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            if !inside_root {
                findings.push(finding(
                    "PATH_SCOPE_ESCAPE",
                    severity::HIGH,
                    "Target path is outside workspace root",
                    Some(disposition::BLOCKING),
                    None,
                ));
            }
        }
    }

    // --- Newline style detection (when policy is not "skip") ---
    let mut newline_check_result: Option<Value> = None;
    if newline_policy != "skip" {
        // Detect newline style on original text using text_fingerprint
        let fp_args = serde_json::json!({"text": original, "unicode": "raw", "newline": "raw"});
        let fp_resp = crate::tools::text::text_fingerprint_tool(&fp_args);
        if let Some(ref r) = fp_resp.result {
            let style = r
                .get("newline_style")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let mixed = style == "mixed";
            let nc = serde_json::json!({
                "style": style,
                "mixed": mixed,
            });
            newline_check_result = Some(nc.clone());
            subresults.insert("newline_check".to_string(), nc);
            if mixed {
                findings.push(finding(
                    "NEWLINE_INCONSISTENCY",
                    severity::MEDIUM,
                    "File has mixed newline styles (CRLF and LF)",
                    Some(disposition::CAUTION),
                    None,
                ));
            }
        }
    }

    // --- Unicode security check (when policy is not "skip") ---
    let mut unicode_check_result: Option<Value> = None;
    if unicode_policy != "skip" {
        // Determine text to inspect: prefer `new` (literal mode), otherwise original
        let inspect_text = args.get("new").and_then(|v| v.as_str()).unwrap_or(original);
        let us_args = serde_json::json!({
            "text": inspect_text,
            "policy": unicode_policy,
            "detail": "summary",
        });
        let us_resp = crate::tools::text::text_security_inspect(&us_args);
        if let Some(ref r) = us_resp.result {
            let us_verdict = r
                .get("verdict")
                .and_then(|v| v.as_str())
                .unwrap_or("allow")
                .to_string();
            let us_machine_code = r
                .get("machine_code")
                .and_then(|v| v.as_str())
                .unwrap_or("TEXT_SECURITY_OK")
                .to_string();
            let us_findings_count = r
                .get("findings")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            let uc = serde_json::json!({
                "verdict": us_verdict,
                "machine_code": us_machine_code,
                "finding_count": us_findings_count,
            });
            unicode_check_result = Some(uc.clone());
            subresults.insert("text_security_inspect".to_string(), r.clone());
            if us_verdict == "block" {
                findings.push(finding(
                    "UNICODE_RISK",
                    severity::HIGH,
                    "Unicode security check blocked replacement text",
                    Some(disposition::BLOCKING),
                    None,
                ));
            } else if us_verdict == "review" {
                findings.push(finding(
                    "UNICODE_RISK",
                    severity::MEDIUM,
                    "Unicode security check flagged replacement text for review",
                    Some(disposition::CAUTION),
                    None,
                ));
            }
        }
    }

    let has_error = findings
        .iter()
        .any(|f| f.get("severity").and_then(|v| v.as_str()) == Some(severity::HIGH));
    let has_warning = findings
        .iter()
        .any(|f| f.get("severity").and_then(|v| v.as_str()) == Some(severity::MEDIUM));
    let ok_to_apply = !has_error;

    // Determine verdict
    let response_verdict = if has_error {
        verdict::BLOCK
    } else if has_warning {
        verdict::REVIEW
    } else {
        verdict::ALLOW
    };

    // Determine machine_code and recommended_next_tool (matching Python's first-inserted-wins)
    let mut code_list: Vec<String> = Vec::new();
    let mut recommended_next: Option<serde_json::Value> = None;

    let has_no_match = findings
        .iter()
        .any(|f| f.get("code").and_then(|v| v.as_str()) == Some("NO_MATCH"));
    let has_multiple = findings
        .iter()
        .any(|f| f.get("code").and_then(|v| v.as_str()) == Some("MULTIPLE_MATCHES"));
    let has_patch_fail = findings
        .iter()
        .any(|f| f.get("code").and_then(|v| v.as_str()) == Some("PATCH_FAILED"));
    let has_patch_error = findings
        .iter()
        .any(|f| f.get("code").and_then(|v| v.as_str()) == Some("PATCH_ERROR"));
    let has_fingerprint = findings
        .iter()
        .any(|f| f.get("code").and_then(|v| v.as_str()) == Some("FINGERPRINT_MISMATCH"));
    let has_invalid_range = findings
        .iter()
        .any(|f| f.get("code").and_then(|v| v.as_str()) == Some("INVALID_RANGE"));

    if has_no_match || has_multiple {
        code_list.push(machine_codes::AMBIGUOUS_REPLACEMENT.to_string());
        recommended_next = Some(ToolResponse::next_tool(
            "text_diff_explain",
            "literal replacement was ambiguous",
            None,
        ));
    }
    if (has_patch_fail || has_patch_error)
        && !code_list.contains(&machine_codes::PATCH_FAILED.to_string())
    {
        code_list.push(machine_codes::PATCH_FAILED.to_string());
    }
    if has_invalid_range && !code_list.contains(&machine_codes::LINE_RANGE_INVALID.to_string()) {
        code_list.push(machine_codes::LINE_RANGE_INVALID.to_string());
    }
    if has_fingerprint {
        if !code_list.contains(&machine_codes::FINGERPRINT_MISMATCH.to_string()) {
            code_list.push(machine_codes::FINGERPRINT_MISMATCH.to_string());
        }
        if recommended_next.is_none() {
            recommended_next = Some(ToolResponse::next_tool(
                "text_diff_explain",
                "content fingerprint mismatch",
                None,
            ));
        }
    }
    // Path scope escape is always BLOCK-level
    let has_path_escape = findings
        .iter()
        .any(|f| f.get("code").and_then(|v| v.as_str()) == Some("PATH_SCOPE_ESCAPE"));
    if has_path_escape && !code_list.contains(&machine_codes::PATH_SCOPE_ESCAPE.to_string()) {
        code_list.push(machine_codes::PATH_SCOPE_ESCAPE.to_string());
    }
    // Unicode risk
    let has_unicode_risk = findings
        .iter()
        .any(|f| f.get("code").and_then(|v| v.as_str()) == Some("UNICODE_RISK"));
    if has_unicode_risk && !code_list.contains(&machine_codes::UNICODE_RISK.to_string()) {
        code_list.push(machine_codes::UNICODE_RISK.to_string());
    }
    // Newline inconsistency
    let has_newline_inc = findings
        .iter()
        .any(|f| f.get("code").and_then(|v| v.as_str()) == Some("NEWLINE_INCONSISTENCY"));
    if has_newline_inc && !code_list.contains(&machine_codes::NEWLINE_INCONSISTENCY.to_string()) {
        code_list.push(machine_codes::NEWLINE_INCONSISTENCY.to_string());
    }
    if has_error && code_list.is_empty() {
        code_list.push(machine_codes::EDIT_FAILED.to_string());
    }
    if code_list.is_empty() {
        code_list.push(machine_codes::EDIT_OK.to_string());
    }
    let machine_code_str = code_list[0].clone();

    // Build summary
    let summary = if ok_to_apply {
        format!("Edit OK ({} mode)", replacement_mode)
    } else {
        format!("Edit blocked ({} mode)", replacement_mode)
    };
    let summary = if findings.is_empty() {
        summary
    } else {
        format!("{}; {} finding(s)", summary, findings.len())
    };

    let mut result = serde_json::json!({
        "ok_to_apply": ok_to_apply,
        "verdict": response_verdict,
        "mode": replacement_mode,
        "findings": findings,
        "machine_code": machine_code_str,
        "recommended_next_tool": recommended_next,
        "summary": summary,
    });
    if !subresults.is_empty() {
        result["subresults"] = serde_json::Value::Object(subresults);
    }
    if let Some(ps) = path_scope_result {
        result["path_scope"] = ps;
    }
    if let Some(nc) = newline_check_result {
        result["newline_check"] = nc;
    }
    if let Some(uc) = unicode_check_result {
        result["unicode_check"] = uc;
    }
    if let Some(fp) = fingerprint_result {
        result["fingerprint"] = fp;
    }

    let mut resp =
        ToolResponse::success(result, Some("edit_preflight")).with_tool("edit_preflight");
    resp = resp
        .with_machine_code(&machine_code_str)
        .with_verdict(response_verdict);
    if !findings.is_empty() {
        resp = resp.with_findings(findings.clone());
    }
    resp
}
