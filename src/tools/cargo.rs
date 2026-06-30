use crate::mcp::machine_codes;
use crate::mcp::schemas::ToolResponse;
use crate::tools::helpers::*;
use serde_json::Value;

pub fn cargo_toml_inspect(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'text' parameter",
                None,
                Some("cargo_toml_inspect"),
            )
        }
    };
    let check_workspace = args
        .get("check_workspace")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let check_dependencies = args
        .get("check_dependencies")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("cargo_toml_inspect"),
        );
    }

    let result = crate::text::cargo_toml_inspect(text, check_workspace, check_dependencies);

    let parse_ok = result.parse_ok;
    let has_findings = !result.findings.is_empty();

    let envelope_findings: Vec<serde_json::Value> = result
        .findings
        .iter()
        .map(|msg| {
            let (severity, code) = {
                let lower = msg.to_lowercase();
                if lower.contains("parse error") || lower.contains("not a table") {
                    ("error", "CARGO_PARSE_ERROR")
                } else if lower.contains("missing") {
                    ("warn", "CARGO_MISSING_FIELD")
                } else if lower.contains("confusable") {
                    ("warn", "CARGO_CONFUSABLE_NAMES")
                } else if lower.contains("suspicious") {
                    ("warn", "CARGO_SUSPICIOUS_NAME")
                } else if lower.contains("unrecognized") {
                    ("warn", "CARGO_UNRECOGNIZED_VALUE")
                } else {
                    ("info", "CARGO_NOTE")
                }
            };
            serde_json::json!({
                "code": code,
                "severity": severity,
                "message": msg,
            })
        })
        .collect();

    let mut resp = ToolResponse::success(
    serde_json::json!({
        "parse_ok": result.parse_ok,
        "package": result.package,
        "workspace": result.workspace,
        "dependencies": result.dependencies,
        "path_dependencies": result.path_dependencies,
        "suspicious_dependency_names": result.suspicious_dependency_names,
        "duplicate_or_confusable_dependency_names": result.duplicate_or_confusable_dependency_names,
        "findings": result.findings,
    }),
    Some("cargo_toml_inspect")
    ).with_tool("cargo_toml_inspect");

    if !envelope_findings.is_empty() {
        resp = resp.with_findings(envelope_findings);
    }
    if !parse_ok {
        resp = resp.with_machine_code(machine_codes::CARGO_PARSE_FAILED);
    } else if has_findings {
        resp = resp.with_machine_code(machine_codes::CARGO_HAS_FINDINGS);
    }
    resp
}
