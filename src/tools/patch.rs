use crate::mcp::machine_codes;
use crate::mcp::schemas::ToolResponse;
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
            return ToolResponse::error(
                "invalid_arguments",
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
            return ToolResponse::error(
                "invalid_arguments",
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
        return ToolResponse::error(
            "input_too_large",
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
        return ToolResponse::error(
            "input_too_large",
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

    ToolResponse::success(
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
            "findings": result.findings,
        }),
        Some("patch_apply_check"),
    )
    .with_tool("patch_apply_check")
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
            return ToolResponse::error(
                "invalid_arguments",
                &format!("patch_text must be a string, got {}", type_name),
                None,
                Some("patch_summary"),
            );
        }
    };

    const MAX_PATCH_LENGTH: usize = 100_000;
    if patch_text.chars().count() > MAX_PATCH_LENGTH {
        return ToolResponse::error(
            "input_too_large",
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

    ToolResponse::success(
        serde_json::json!({
            "files_changed": result.files_changed,
            "hunks_total": result.hunks_total,
            "additions": result.additions,
            "deletions": result.deletions,
            "renames_detected": result.renames_detected,
            "binary_patch_detected": result.binary_patch_detected,
            "line_ranges_by_file": result.line_ranges_by_file,
            "findings": result.findings,
        }),
        Some("patch_summary"),
    )
    .with_tool("patch_summary")
}

pub fn edit_preflight(args: &Value) -> ToolResponse {
    let original = match args.get("original").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
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

    if original.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Original text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("edit_preflight"),
        );
    }

    let valid_modes = ["literal", "patch", "line_range"];
    if !valid_modes.contains(&replacement_mode) {
        return ToolResponse::error(
            "invalid_arguments",
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
                    return ToolResponse::error(
                        "invalid_arguments",
                        "literal mode requires both 'old' and 'new'",
                        None,
                        Some("edit_preflight"),
                    )
                }
            };
            let new = match args.get("new").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => {
                    return ToolResponse::error(
                        "invalid_arguments",
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
                    findings.push(serde_json::json!({
                        "code": "NO_MATCH",
                        "severity": "error",
                        "message": "old text not found in original",
                    }));
                } else if match_count > 1 {
                    findings.push(serde_json::json!({
                        "code": "MULTIPLE_MATCHES",
                        "severity": "warn",
                        "message": format!("Found {} matches; use allow_multiple=true", match_count),
                    }));
                }
            }
        }
        "patch" => {
            let patch_text = match args.get("patch").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => {
                    return ToolResponse::error(
                        "invalid_arguments",
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
                    findings.push(serde_json::json!({
                        "code": "PATCH_ERROR",
                        "severity": "error",
                        "message": e,
                    }));
                }
                ToolResponse {
                    result: Some(ref r),
                    ..
                } => {
                    subresults.insert("patch_apply_check".to_string(), r.clone());
                    if let Some(applies) = r.get("applies").and_then(|v| v.as_bool()) {
                        if !applies {
                            findings.push(serde_json::json!({
                                "code": "PATCH_FAILED",
                                "severity": "error",
                                "message": "Patch does not apply cleanly",
                            }));
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
                    return ToolResponse::error(
                        "invalid_arguments",
                        "line_range mode requires 'start_line' and 'end_line'",
                        None,
                        Some("edit_preflight"),
                    )
                }
            };
            let end_line = match args.get("end_line").and_then(|v| v.as_u64()) {
                Some(n) => n as usize,
                None => {
                    return ToolResponse::error(
                        "invalid_arguments",
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
                        findings.push(serde_json::json!({
                            "code": "INVALID_RANGE",
                            "severity": "error",
                            "message": "Invalid line range",
                        }));
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
    if let Some(fp) = expected_fingerprint {
        let (actual_fp, fp_source) = if replacement_mode == "patch" {
            // Use result_fingerprint from patch_apply_check subresult
            let fp_val = subresults
                .get("patch_apply_check")
                .and_then(|r| r.get("result_fingerprint"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            (fp_val.to_string(), "patch_apply_check")
        } else if replacement_mode == "line_range" {
            // Use fingerprint from line_range_extract subresult
            let fp_val = subresults
                .get("line_range_extract")
                .and_then(|r| r.get("fingerprint"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            (fp_val.to_string(), "line_range_extract")
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
            subresults.insert(
                "text_fingerprint".to_string(),
                fp_result.result.unwrap_or(serde_json::Value::Null),
            );
            (fp_val, "text_fingerprint")
        };
        if actual_fp != fp {
            findings.push(serde_json::json!({
                "code": "FINGERPRINT_MISMATCH",
                "severity": "warn",
                "message": format!("Expected {}, got {} (from {})", fp, actual_fp, fp_source),
            }));
        }
    }

    let has_error = findings
        .iter()
        .any(|f| f.get("severity").and_then(|v| v.as_str()) == Some("error"));
    let ok_to_apply = !has_error;

    // Determine machine_code and recommended_next_tool (matching Python's first-inserted-wins)
    let mut code_list: Vec<String> = Vec::new();
    let mut recommended_next: Option<String> = None;

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
        recommended_next = Some("text_diff_explain".to_string());
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
            recommended_next = Some("text_diff_explain".to_string());
        }
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
        "mode": replacement_mode,
        "findings": findings,
        "machine_code": machine_code_str,
        "recommended_next_tool": recommended_next,
        "summary": summary,
    });
    if !subresults.is_empty() {
        result["subresults"] = serde_json::Value::Object(subresults);
    }

    let mut resp =
        ToolResponse::success(result, Some("edit_preflight")).with_tool("edit_preflight");
    resp = resp.with_machine_code(&machine_code_str);
    if !findings.is_empty() {
        resp = resp.with_findings(findings.clone());
    }
    if let Some(ref next) = recommended_next {
        let next_val: serde_json::Value = serde_json::Value::String(next.clone());
        resp = resp.with_recommended_next_tool(next_val);
    }
    resp
}
