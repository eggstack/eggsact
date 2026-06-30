use crate::mcp::schemas::ToolResponse;
use crate::tools::helpers::*;
use serde_json::Value;

pub fn version_compare_tool(args: &Value) -> ToolResponse {
    let a = match args.get("a").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'a' parameter",
                None,
                Some("version_compare"),
            )
        }
    };
    let b = match args.get("b").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'b' parameter",
                None,
                Some("version_compare"),
            )
        }
    };
    let scheme = args
        .get("scheme")
        .and_then(|v| v.as_str())
        .unwrap_or("semver");

    if a.chars().count() > MAX_TEXT_LENGTH || b.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Version string exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("version_compare"),
        );
    }

    let valid_schemes = ["semver", "pep440", "loose"];
    if !valid_schemes.contains(&scheme) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported scheme: {}", scheme),
            Some(vec![format!("Use one of: {}", valid_schemes.join(", "))]),
            Some("version_compare"),
        );
    }

    let result = crate::text::version::version_compare(a, b, scheme);
    ToolResponse::success(
        serde_json::json!({
            "comparison": result.comparison,
            "valid": result.valid,
            "scheme": result.scheme,
            "summary": result.summary,
        }),
        Some("version_compare"),
    )
    .with_tool("version_compare")
}

pub fn version_constraint_check(args: &Value) -> ToolResponse {
    let version = match args.get("version").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'version' parameter",
                None,
                Some("version_constraint_check"),
            )
        }
    };
    let constraint = match args.get("constraint").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'constraint' parameter",
                None,
                Some("version_constraint_check"),
            )
        }
    };
    let scheme = args
        .get("scheme")
        .and_then(|v| v.as_str())
        .unwrap_or("semver");

    let valid_schemes = ["semver", "cargo"];
    if !valid_schemes.contains(&scheme) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported scheme: {}", scheme),
            Some(vec![format!("Use one of: {}", valid_schemes.join(", "))]),
            Some("version_constraint_check"),
        );
    }

    if version.trim().is_empty() {
        return ToolResponse::error(
            "invalid_arguments",
            "Version string is empty",
            Some(vec![
                "Provide a valid version string like '1.2.3'".to_string()
            ]),
            Some("version_constraint_check"),
        );
    }
    if constraint.trim().is_empty() {
        return ToolResponse::error(
            "invalid_arguments",
            "Constraint string is empty",
            Some(vec![
                "Provide a valid constraint like '>=1.0' or '^1.2.3'".to_string()
            ]),
            Some("version_constraint_check"),
        );
    }
    if version.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Input exceeds maximum length of {}", MAX_TEXT_LENGTH),
            None,
            Some("version_constraint_check"),
        );
    }
    if constraint.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Input exceeds maximum length of {}", MAX_TEXT_LENGTH),
            None,
            Some("version_constraint_check"),
        );
    }

    let result = crate::text::check_version_constraint(version, constraint, scheme);

    let satisfies = result.satisfies;
    let has_note = !result.findings.is_empty();

    let envelope_findings: Vec<serde_json::Value> = result
        .findings
        .iter()
        .map(|f| {
            serde_json::json!({
                "code": "CONSTRAINT_NOTE",
                "severity": "info",
                "message": f,
            })
        })
        .collect();

    let mut resp = ToolResponse::success(
        serde_json::json!({
            "satisfies": result.satisfies,
            "parsed_version": result.parsed_version,
            "parsed_constraint": result.parsed_constraint,
            "scheme": result.scheme,
            "explanation": result.explanation,
            "findings": result.findings,
        }),
        Some("version_constraint_check"),
    )
    .with_tool("version_constraint_check");

    if !envelope_findings.is_empty() {
        resp = resp.with_findings(envelope_findings);
    }
    if !satisfies {
        resp = resp.with_machine_code("CONSTRAINT_NOT_SATISFIED");
    } else if has_note {
        resp = resp.with_machine_code("CONSTRAINT_NOTE");
    }
    resp
}
