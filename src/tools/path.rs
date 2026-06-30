use crate::mcp::machine_codes;
use crate::mcp::schemas::ToolResponse;
use crate::tools::helpers::*;
use serde_json::Value;

pub fn path_normalize_tool(args: &Value) -> ToolResponse {
    let path = match args.get("path").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
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
        return ToolResponse::error(
            "invalid_arguments",
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
            return ToolResponse::error(
                "invalid_arguments",
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
        return ToolResponse::error(
            "input_too_large",
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
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported style: {}", style),
            Some(vec![format!("Use one of: {}", valid_styles.join(", "))]),
            Some("path_analyze"),
        );
    }

    let valid_details = ["summary", "normal", "full"];
    if !valid_details.contains(&detail) {
        return ToolResponse::error(
            "invalid_arguments",
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
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'left' parameter",
                None,
                Some("path_compare"),
            )
        }
    };
    let right = match args.get("right").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
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
        return ToolResponse::error(
            "input_too_large",
            "Left path exceeds MAX_TEXT_LENGTH",
            None,
            Some("path_compare"),
        );
    }
    if right.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            "Right path exceeds MAX_TEXT_LENGTH",
            None,
            Some("path_compare"),
        );
    }

    let valid_platforms = ["posix", "windows"];
    if !valid_platforms.contains(&platform) {
        return ToolResponse::error(
            "invalid_arguments",
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
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'root' parameter",
                None,
                Some("path_scope_check"),
            )
        }
    };
    let target = match args.get("target").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
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
        return ToolResponse::error(
            "input_too_large",
            "Root path exceeds MAX_TEXT_LENGTH",
            None,
            Some("path_scope_check"),
        );
    }
    if target.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            "Target path exceeds MAX_TEXT_LENGTH",
            None,
            Some("path_scope_check"),
        );
    }

    let valid_platforms = ["posix", "windows"];
    if !valid_platforms.contains(&platform) {
        return ToolResponse::error(
            "invalid_arguments",
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
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'pattern' parameter",
                None,
                Some("glob_match"),
            )
        }
    };
    let path = match args.get("path").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
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
        return ToolResponse::error(
            "input_too_large",
            "Pattern or path exceeds maximum length",
            None,
            Some("glob_match"),
        );
    }

    let valid_platforms = ["posix", "windows"];
    if !valid_platforms.contains(&platform) {
        return ToolResponse::error(
            "invalid_arguments",
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
