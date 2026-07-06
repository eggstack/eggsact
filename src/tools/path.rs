use crate::mcp::machine_codes;
use crate::mcp::response::{disposition, finding, severity, verdict};
use crate::mcp::schemas::ToolResponse;
use crate::tools::helpers::*;
use serde_json::Value;
use std::collections::HashMap;

pub fn path_normalize_tool(args: &Value) -> ToolResponse {
    let path = match args.get("path").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'path' parameter",
                None,
                Some("path_normalize"),
            )
        }
    };
    let platform = args
        .get("platform")
        .and_then(|v| v.as_str())
        .unwrap_or("posix");
    let collapse_dot_segments = args
        .get("collapse_dot_segments")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let preserve_trailing_separator = args
        .get("preserve_trailing_separator")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let valid_platforms = ["posix", "windows"];
    if !valid_platforms.contains(&platform) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported platform: {}", platform),
            Some(vec![format!("Use one of: {}", valid_platforms.join(", "))]),
            Some("path_normalize"),
        );
    }

    let result = crate::text::path::path_normalize(
        path,
        platform,
        collapse_dot_segments,
        preserve_trailing_separator,
    );
    ToolResponse::success(
        serde_json::json!({
            "normalized": result.normalized,
            "is_absolute": result.is_absolute,
            "components": result.components,
            "warnings": result.warnings,
        }),
        Some("path_normalize"),
    )
    .with_tool("path_normalize")
}

pub fn path_analyze(args: &Value) -> ToolResponse {
    let path = match args.get("path").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'path' parameter",
                None,
                Some("path_analyze"),
            )
        }
    };
    let style = args.get("style").and_then(|v| v.as_str()).unwrap_or("auto");
    let detail = args
        .get("detail")
        .and_then(|v| v.as_str())
        .unwrap_or("normal");

    if path.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INVALID_ARGUMENTS,
            &format!(
                "Path length {} exceeds MAX_TEXT_LENGTH {}",
                path.chars().count(),
                MAX_TEXT_LENGTH
            ),
            None,
            Some("path_analyze"),
        );
    }

    let valid_styles = ["auto", "posix", "windows"];
    if !valid_styles.contains(&style) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported style: {}", style),
            Some(vec![format!("Use one of: {}", valid_styles.join(", "))]),
            Some("path_analyze"),
        );
    }

    let valid_details = ["summary", "normal", "full"];
    if !valid_details.contains(&detail) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported detail level: {}", detail),
            Some(vec![format!("Use one of: {}", valid_details.join(", "))]),
            Some("path_analyze"),
        );
    }

    let result = crate::text::path_analyze(path, style);

    let has_traversal = result.has_traversal;
    let is_hidden = result.hidden;
    let mut envelope_findings: Vec<serde_json::Value> = Vec::new();
    if has_traversal {
        envelope_findings.push(serde_json::json!({
            "code": "PATH_TRAVERSAL",
            "severity": "warn",
            "message": "Path contains parent directory traversal (..)",
            "details": {"normalized_lexical": result.normalized_lexical},
        }));
    }
    if is_hidden {
        envelope_findings.push(serde_json::json!({
            "code": "PATH_HIDDEN",
            "severity": "info",
            "message": "Path starts with a dot (hidden file/directory)",
        }));
    }
    let machine_code = if has_traversal {
        Some(machine_codes::PATH_HAS_TRAVERSAL)
    } else if is_hidden {
        Some(machine_codes::PATH_IS_HIDDEN)
    } else {
        None
    };

    if detail == "summary" {
        let mut resp = ToolResponse::success(
            serde_json::json!({
                "summary": result.summary,
                "style": result.style,
                "absolute": result.absolute,
                "hidden": result.hidden,
                "has_traversal": result.has_traversal,
                "warnings": result.warnings,
            }),
            Some("path_analyze"),
        )
        .with_tool("path_analyze");
        if !envelope_findings.is_empty() {
            resp = resp.with_findings(envelope_findings.clone());
        }
        if let Some(code) = machine_code {
            resp = resp.with_machine_code(code);
        }
        resp
    } else {
        let mut resp = ToolResponse::success(
            serde_json::json!({
                "input": result.input,
                "style": result.style,
                "absolute": result.absolute,
                "has_traversal": result.has_traversal,
                "components": result.components,
                "parent": result.parent,
                "name": result.name,
                "stem": result.stem,
                "suffix": result.suffix,
                "suffixes": result.suffixes,
                "hidden": result.hidden,
                "normalized_lexical": result.normalized_lexical,
                "warnings": result.warnings,
                "summary": result.summary,
            }),
            Some("path_analyze"),
        )
        .with_tool("path_analyze");
        if !envelope_findings.is_empty() {
            resp = resp.with_findings(envelope_findings);
        }
        if let Some(code) = machine_code {
            resp = resp.with_machine_code(code);
        }
        resp
    }
}

pub fn path_compare(args: &Value) -> ToolResponse {
    let left = match args.get("left").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'left' parameter",
                None,
                Some("path_compare"),
            )
        }
    };
    let right = match args.get("right").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'right' parameter",
                None,
                Some("path_compare"),
            )
        }
    };
    let platform = args
        .get("platform")
        .and_then(|v| v.as_str())
        .unwrap_or("posix");
    let case_sensitive = args
        .get("case_sensitive")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let normalize_separators = args
        .get("normalize_separators")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let collapse_dot_segments = args
        .get("collapse_dot_segments")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    if left.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INVALID_ARGUMENTS,
            "Left path exceeds MAX_TEXT_LENGTH",
            None,
            Some("path_compare"),
        );
    }
    if right.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INVALID_ARGUMENTS,
            "Right path exceeds MAX_TEXT_LENGTH",
            None,
            Some("path_compare"),
        );
    }

    let valid_platforms = ["posix", "windows"];
    if !valid_platforms.contains(&platform) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported platform: {}", platform),
            Some(vec![format!("Use one of: {}", valid_platforms.join(", "))]),
            Some("path_compare"),
        );
    }

    let result = crate::text::path_compare(
        left,
        right,
        platform,
        case_sensitive,
        normalize_separators,
        collapse_dot_segments,
    );

    ToolResponse::success(
        serde_json::json!({
            "equal": result.equal,
            "left_normalized": result.left_normalized,
            "right_normalized": result.right_normalized,
            "differences": result.differences,
            "findings": result.findings,
        }),
        Some("path_compare"),
    )
    .with_tool("path_compare")
}

pub fn path_scope_check(args: &Value) -> ToolResponse {
    let root = match args.get("root").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'root' parameter",
                None,
                Some("path_scope_check"),
            )
        }
    };
    let target = match args.get("target").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'target' parameter",
                None,
                Some("path_scope_check"),
            )
        }
    };
    let platform = args
        .get("platform")
        .and_then(|v| v.as_str())
        .unwrap_or("posix");
    let case_sensitive = args
        .get("case_sensitive")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    if root.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INVALID_ARGUMENTS,
            "Root path exceeds MAX_TEXT_LENGTH",
            None,
            Some("path_scope_check"),
        );
    }
    if target.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INVALID_ARGUMENTS,
            "Target path exceeds MAX_TEXT_LENGTH",
            None,
            Some("path_scope_check"),
        );
    }

    let valid_platforms = ["posix", "windows"];
    if !valid_platforms.contains(&platform) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported platform: {}", platform),
            Some(vec![format!("Use one of: {}", valid_platforms.join(", "))]),
            Some("path_scope_check"),
        );
    }

    let result = crate::text::path_scope_check(root, target, platform, case_sensitive);

    ToolResponse::success(
        serde_json::json!({
            "inside_root": result.inside_root,
            "root_normalized": result.root_normalized,
            "target_normalized": result.target_normalized,
            "relative_path": result.relative_path,
            "escapes_via_dotdot": result.escapes_via_dotdot,
            "absolute_target": result.absolute_target,
            "findings": result.findings,
        }),
        Some("path_scope_check"),
    )
    .with_tool("path_scope_check")
}

pub fn glob_match_tool(args: &Value) -> ToolResponse {
    let pattern = match args.get("pattern").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'pattern' parameter",
                None,
                Some("glob_match"),
            )
        }
    };
    let path = match args.get("path").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'path' parameter",
                None,
                Some("glob_match"),
            )
        }
    };
    let platform = args
        .get("platform")
        .and_then(|v| v.as_str())
        .unwrap_or("posix");
    let case_sensitive = args
        .get("case_sensitive")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    if pattern.chars().count() > MAX_TEXT_LENGTH || path.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INVALID_ARGUMENTS,
            "Pattern or path exceeds maximum length",
            None,
            Some("glob_match"),
        );
    }

    let valid_platforms = ["posix", "windows"];
    if !valid_platforms.contains(&platform) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported platform: {}", platform),
            Some(vec![format!("Use one of: {}", valid_platforms.join(", "))]),
            Some("glob_match"),
        );
    }

    let result = crate::text::glob::glob_match(pattern, path, platform, case_sensitive);

    ToolResponse::success(
        serde_json::json!({
            "matches": result.matches,
            "normalized_pattern": result.normalized_pattern,
            "normalized_path": result.normalized_path,
            "matched_segment": result.matched_segment,
            "unmatched_segment": result.unmatched_segment,
            "summary": result.summary,
        }),
        Some("glob_match"),
    )
    .with_tool("glob_match")
}

pub fn path_batch_scope_check(args: &Value) -> ToolResponse {
    let root = match args.get("root").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'root' parameter",
                None,
                Some("path_batch_scope_check"),
            )
        }
    };

    let targets = match require_array_arg(args, "targets", "path_batch_scope_check") {
        Ok(arr) => arr,
        Err(resp) => return *resp,
    };

    let max_targets = args
        .get("max_targets")
        .and_then(|v| v.as_i64())
        .unwrap_or(1000) as usize;
    let allow_absolute = args
        .get("allow_absolute")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let case_sensitive = args
        .get("case_sensitive")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    if root.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            "Root path exceeds MAX_TEXT_LENGTH",
            None,
            Some("path_batch_scope_check"),
        );
    }

    if targets.len() > max_targets {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!(
                "Too many targets: {} exceeds max_targets {}",
                targets.len(),
                max_targets
            ),
            None,
            Some("path_batch_scope_check"),
        );
    }

    let valid_platforms = ["posix", "windows"];
    let platform = args
        .get("platform")
        .and_then(|v| v.as_str())
        .unwrap_or("posix");
    if !valid_platforms.contains(&platform) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported platform: {}", platform),
            Some(vec![format!("Use one of: {}", valid_platforms.join(", "))]),
            Some("path_batch_scope_check"),
        );
    }

    let mut escaping_targets = Vec::new();
    let mut absolute_targets = Vec::new();
    let mut dotdot_targets = Vec::new();
    let mut normalized_targets = Vec::new();
    let mut findings = Vec::new();
    let mut seen_normalized: HashMap<String, Vec<String>> = HashMap::new();
    let mut all_inside = true;

    for target_val in targets {
        let target = match target_val.as_str() {
            Some(s) => s,
            None => {
                findings.push(finding(
                    machine_codes::INVALID_ARGUMENTS,
                    severity::MEDIUM,
                    "Non-string target path in targets array",
                    Some(disposition::BLOCKING),
                    None,
                ));
                all_inside = false;
                continue;
            }
        };

        if target.chars().count() > MAX_TEXT_LENGTH {
            findings.push(finding(
                machine_codes::INPUT_TOO_LARGE,
                severity::HIGH,
                &format!("Target path '{}' exceeds MAX_TEXT_LENGTH", target),
                Some(disposition::BLOCKING),
                None,
            ));
            all_inside = false;
            continue;
        }

        let result = crate::text::path_scope_check(root, target, platform, case_sensitive);

        // Track normalized for duplicate detection
        seen_normalized
            .entry(result.target_normalized.clone())
            .or_default()
            .push(target.to_string());

        normalized_targets.push(serde_json::json!({
            "original": target,
            "normalized": result.target_normalized,
            "inside_root": result.inside_root,
        }));

        if !result.inside_root {
            all_inside = false;
            escaping_targets.push(target.to_string());
            findings.push(finding(
                machine_codes::PATH_SCOPE_ESCAPE,
                severity::HIGH,
                &format!("Target '{}' escapes root", target),
                Some(disposition::BLOCKING),
                None,
            ));
        }

        if !target.is_empty() && target.starts_with('/') {
            absolute_targets.push(target.to_string());
            if !allow_absolute {
                findings.push(finding(
                    machine_codes::PATH_SCOPE_ESCAPE,
                    severity::MEDIUM,
                    &format!("Absolute target '{}' not allowed", target),
                    Some(disposition::CAUTION),
                    None,
                ));
            }
        }

        if target.contains("..") {
            dotdot_targets.push(target.to_string());
            if result.escapes_via_dotdot && result.inside_root {
                // dotdot that normalizes inside is informational
                findings.push(finding(
                    machine_codes::PATH_HAS_TRAVERSAL,
                    severity::LOW,
                    &format!(
                        "Target '{}' contains '..' but normalizes inside root",
                        target
                    ),
                    Some(disposition::INFORMATIONAL),
                    None,
                ));
            }
        }
    }

    // Detect duplicates
    let mut duplicate_normalized_targets = Vec::new();
    for (norm, originals) in &seen_normalized {
        if originals.len() > 1 {
            duplicate_normalized_targets.push(serde_json::json!({
                "normalized": norm,
                "originals": originals,
            }));
            findings.push(finding(
                machine_codes::PATH_BATCH_REVIEW,
                severity::LOW,
                &format!(
                    "Duplicate normalized path '{}' reached by {} inputs",
                    norm,
                    originals.len()
                ),
                Some(disposition::CAUTION),
                None,
            ));
        }
    }

    let machine_code = if all_inside && escaping_targets.is_empty() {
        machine_codes::PATH_BATCH_OK
    } else {
        machine_codes::PATH_BATCH_REVIEW
    };

    let v = if all_inside && escaping_targets.is_empty() {
        verdict::ALLOW
    } else {
        verdict::REVIEW
    };

    ToolResponse::success(
        serde_json::json!({
            "all_inside_root": all_inside,
            "targets_checked": targets.len(),
            "escaping_targets": escaping_targets,
            "absolute_targets": absolute_targets,
            "dotdot_targets": dotdot_targets,
            "normalized_targets": normalized_targets,
            "duplicate_normalized_targets": duplicate_normalized_targets,
            "findings": findings,
            "verdict": v,
            "machine_code": machine_code,
        }),
        Some("path_batch_scope_check"),
    )
    .with_tool("path_batch_scope_check")
    .with_machine_code(machine_code)
    .with_verdict(v)
}
