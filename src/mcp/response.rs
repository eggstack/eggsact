use regex;
use serde::Serialize;
use std::sync::LazyLock;

pub fn escape_ascii_json(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii() {
            result.push(c);
        } else {
            let mut utf16 = [0u16; 2];
            for unit in c.encode_utf16(&mut utf16).iter() {
                result.push_str(&format!("\\u{:04x}", unit));
            }
        }
    }
    result
}

pub fn python_json_dumps<T: Serialize>(value: &T) -> String {
    struct PythonStyleFormatter;

    impl serde_json::ser::Formatter for PythonStyleFormatter {
        fn begin_array_value<W: std::io::Write + ?Sized>(
            &mut self,
            writer: &mut W,
            first: bool,
        ) -> std::io::Result<()> {
            if first {
                Ok(())
            } else {
                writer.write_all(b", ")
            }
        }

        fn begin_object_key<W: std::io::Write + ?Sized>(
            &mut self,
            writer: &mut W,
            first: bool,
        ) -> std::io::Result<()> {
            if first {
                Ok(())
            } else {
                writer.write_all(b", ")
            }
        }

        fn begin_object_value<W: std::io::Write + ?Sized>(
            &mut self,
            writer: &mut W,
        ) -> std::io::Result<()> {
            writer.write_all(b": ")
        }
    }

    let mut buf = Vec::new();
    {
        let mut serializer = serde_json::Serializer::with_formatter(&mut buf, PythonStyleFormatter);
        if value.serialize(&mut serializer).is_err() {
            return String::new();
        }
    }
    let serialized = String::from_utf8(buf).unwrap_or_default();
    escape_ascii_json(&serialized)
}

pub fn wrap_tool_response(tool_response: &ToolResponse) -> serde_json::Value {
    let text = python_json_dumps(tool_response);
    if tool_response.ok {
        serde_json::json!({
            "content": [{"type": "text", "text": text}],
        })
    } else {
        serde_json::json!({
            "content": [{"type": "text", "text": text}],
            "isError": true,
        })
    }
}

static SANITIZE_REGEXES: LazyLock<Vec<(&'static str, regex::Regex, &'static str)>> =
    LazyLock::new(|| {
        vec![
            (
                "file_line",
                regex::Regex::new(r#"File\s+["\'][^"\']*["\'],\s*line\s+\d+"#).unwrap(),
                r#"File "<redacted>", line <redacted>"#,
            ),
            (
                "module_ref",
                regex::Regex::new(r"(?:in\s+)<[^>]+>").unwrap(),
                "in <module>",
            ),
            (
                "var_assign",
                regex::Regex::new(r#"(?m)^\s*[A-Za-z_]\w*\s*=\s*["'][^"']*["']"#).unwrap(),
                "<var>=<redacted>",
            ),
            (
                "system_path",
                regex::Regex::new(
                    r"/(?:etc|proc|dev|sys|run|tmp|var|usr|lib|bin|sbin)(?:/[\w.-]+)+",
                )
                .unwrap(),
                "<path>",
            ),
            (
                "win_path",
                regex::Regex::new(r"[A-Za-z]:\\(?:[\w.-]+\\)+\w+\.\w+").unwrap(),
                "<path>",
            ),
            (
                "no_such_file",
                regex::Regex::new(r#"No such file or directory:\s*['"][^'"]*['"]"#).unwrap(),
                "No such file or directory: '<redacted>'",
            ),
            (
                "mem_addr",
                regex::Regex::new(r"0x[0-9a-fA-F]{8,}").unwrap(),
                "<address>",
            ),
            (
                "json_pos",
                regex::Regex::new(r"(?i)\bline\s+\d+\s+column\s+\d+\b").unwrap(),
                "line <redacted> column <redacted>",
            ),
        ]
    });

static BARE_PATH_REGEX: LazyLock<fancy_regex::Regex> = LazyLock::new(|| {
    fancy_regex::Regex::new(r"(?<![/\w.])(/[\w./-]+\.\w{1,10})(?![/\w])").unwrap()
});

pub fn sanitize_error(msg: &str) -> String {
    let mut result: String = msg.chars().take(8192).collect();
    let mut ascii_result = String::with_capacity(result.len());
    for c in result.chars() {
        if c.is_ascii() {
            ascii_result.push(c);
        } else {
            ascii_result.push('?');
        }
    }
    result = ascii_result;
    for (_name, re, replacement) in SANITIZE_REGEXES.iter() {
        result = re.replace_all(&result, *replacement).to_string();
    }
    result = BARE_PATH_REGEX.replace_all(&result, "<path>").to_string();
    result
}

// Canonical severity, disposition, and verdict constants live in
// `machine_codes`. Re-export here so existing `use crate::mcp::response::{severity, ...}`
// paths continue to work.
pub use crate::mcp::machine_codes::{disposition, severity, verdict};

/// Create a structured finding as a `serde_json::Value`.
///
/// Findings are serialized as JSON objects with standard fields:
/// `code`, `severity`, `message`, and optionally `disposition`, `location`, `details`.
///
/// # Arguments
///
/// * `code` — Machine code constant (e.g. `AMBIGUOUS_REPLACEMENT`).
/// * `severity` — Severity level (`severity::INFO`, etc.).
/// * `message` — Human-readable description of the finding.
/// * `disposition` — Optional disposition (`disposition::BLOCKING`, etc.).
/// * `details` — Optional additional structured data.
pub fn finding(
    code: &str,
    severity: &str,
    message: &str,
    disposition: Option<&str>,
    details: Option<serde_json::Value>,
) -> serde_json::Value {
    let mut f = serde_json::json!({
        "code": code,
        "severity": severity,
        "message": message,
    });
    if let Some(d) = disposition {
        f["disposition"] = serde_json::json!(d);
    }
    if let Some(d) = details {
        f["details"] = d;
    }
    f
}

/// Create a structured finding with a source location.
///
/// # Arguments
///
/// * `code` — Machine code constant.
/// * `severity` — Severity level.
/// * `message` — Human-readable description.
/// * `disposition` — Optional disposition.
/// * `line` — 1-indexed line number.
/// * `column` — 1-indexed column number (optional).
pub fn finding_with_location(
    code: &str,
    severity: &str,
    message: &str,
    disposition: Option<&str>,
    line: usize,
    column: Option<usize>,
) -> serde_json::Value {
    let mut loc = serde_json::json!({
        "line": line,
    });
    if let Some(c) = column {
        loc["column"] = serde_json::json!(c);
    }
    let mut f = serde_json::json!({
        "code": code,
        "severity": severity,
        "message": message,
        "location": loc,
    });
    if let Some(d) = disposition {
        f["disposition"] = serde_json::json!(d);
    }
    f
}

/// Create a finding for a prompt input inspection check.
///
/// These findings have a `span` field instead of `location`.
pub fn prompt_finding(
    code: &str,
    severity: &str,
    message: &str,
    disposition: Option<&str>,
    byte_offset: usize,
    end_byte_offset: usize,
    details: Option<serde_json::Value>,
) -> serde_json::Value {
    let mut f = serde_json::json!({
        "code": code,
        "severity": severity,
        "message": message,
        "span": {
            "byte_offset": byte_offset,
            "end_byte_offset": end_byte_offset,
        },
    });
    if let Some(d) = disposition {
        f["disposition"] = serde_json::json!(d);
    }
    if let Some(d) = details {
        f["details"] = d;
    }
    f
}

#[derive(Serialize, Debug)]
pub struct ToolResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hints: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warnings: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limits_applied: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub findings: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub machine_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recommended_next_tool: Option<serde_json::Value>,
}

impl ToolResponse {
    pub fn success(result: serde_json::Value, tool: Option<&str>) -> Self {
        Self {
            ok: true,
            tool: tool.map(String::from),
            result: Some(result),
            error_type: None,
            error: None,
            hints: None,
            warnings: None,
            limits_applied: None,
            findings: None,
            machine_code: None,
            recommended_next_tool: None,
        }
    }

    #[cfg(test)]
    #[deprecated(
        since = "0.2.0",
        note = "Use `error_with_code` instead. Non-OK tool responses must carry a machine_code."
    )]
    #[doc(hidden)]
    pub(crate) fn error_without_code_for_legacy_tests_only(
        error_type: &str,
        error: &str,
        hints: Option<Vec<String>>,
        tool: Option<&str>,
    ) -> Self {
        Self {
            ok: false,
            tool: tool.map(String::from),
            result: None,
            error_type: Some(error_type.to_string()),
            error: Some(sanitize_error(error)),
            hints: Some(
                hints
                    .unwrap_or_default()
                    .into_iter()
                    .map(|h| sanitize_error(&h))
                    .collect(),
            ),
            warnings: Some(vec![]),
            limits_applied: None,
            findings: None,
            machine_code: None,
            recommended_next_tool: None,
        }
    }

    /// Create an error response with a machine code.
    ///
    /// This is the preferred constructor for non-OK tool responses. It ensures
    /// every error carries a stable machine-readable code.
    ///
    /// # Arguments
    ///
    /// * `error_type` — Coarse error category (legacy, kept for compat).
    /// * `machine_code` — Stable machine code from `machine_codes` module.
    /// * `error` — Human-readable error message.
    /// * `hints` — Optional help text for the caller.
    /// * `tool` — Optional tool name.
    pub fn error_with_code(
        error_type: &str,
        machine_code: &str,
        error: &str,
        hints: Option<Vec<String>>,
        tool: Option<&str>,
    ) -> Self {
        Self {
            ok: false,
            tool: tool.map(String::from),
            result: None,
            error_type: Some(error_type.to_string()),
            error: Some(sanitize_error(error)),
            hints: Some(
                hints
                    .unwrap_or_default()
                    .into_iter()
                    .map(|h| sanitize_error(&h))
                    .collect(),
            ),
            warnings: Some(vec![]),
            limits_applied: None,
            findings: None,
            machine_code: Some(machine_code.to_string()),
            recommended_next_tool: None,
        }
    }

    /// Create a success response with a machine code.
    pub fn success_with_machine_code(
        result: serde_json::Value,
        tool: Option<&str>,
        machine_code: &str,
    ) -> Self {
        Self {
            ok: true,
            tool: tool.map(String::from),
            result: Some(result),
            error_type: None,
            error: None,
            hints: None,
            warnings: None,
            limits_applied: None,
            findings: None,
            machine_code: Some(machine_code.to_string()),
            recommended_next_tool: None,
        }
    }

    pub fn with_tool(mut self, tool: &str) -> Self {
        self.tool = Some(tool.to_string());
        self
    }

    pub fn with_warnings(mut self, warnings: Vec<String>) -> Self {
        self.warnings = Some(warnings);
        self
    }

    pub fn with_limits_applied(mut self, limits: Vec<String>) -> Self {
        self.limits_applied = Some(limits);
        self
    }

    pub fn with_findings(mut self, findings: Vec<serde_json::Value>) -> Self {
        self.findings = Some(findings);
        self
    }

    pub fn with_machine_code(mut self, code: &str) -> Self {
        self.machine_code = Some(code.to_string());
        self
    }

    pub fn with_recommended_next_tool(mut self, tool: serde_json::Value) -> Self {
        self.recommended_next_tool = Some(tool);
        self
    }

    /// Build a structured `recommended_next_tool` value.
    ///
    /// Returns a JSON object with `name`, `reason`, and optionally `arguments_hint`.
    pub fn next_tool(
        name: &str,
        reason: &str,
        arguments_hint: Option<serde_json::Value>,
    ) -> serde_json::Value {
        let mut obj = serde_json::json!({
            "name": name,
            "reason": reason,
        });
        if let Some(hint) = arguments_hint {
            obj["arguments_hint"] = hint;
        }
        obj
    }

    /// Set the `verdict` field inside `result`.
    ///
    /// If `result` is `None`, it is initialized to an empty object first.
    pub fn with_verdict(mut self, verdict: &str) -> Self {
        let result = self.result.get_or_insert_with(|| serde_json::json!({}));
        result["verdict"] = serde_json::json!(verdict);
        self
    }
}

/// Build a preflight "allow" response.
///
/// Sets `ok=true`, verdict to `allow`, machine_code, and optionally findings and next tool.
pub fn preflight_allow(
    tool: &str,
    machine_code: &str,
    result: serde_json::Value,
    findings: Vec<serde_json::Value>,
    next_tool: Option<serde_json::Value>,
) -> ToolResponse {
    let mut resp = ToolResponse::success(result, Some(tool))
        .with_machine_code(machine_code)
        .with_verdict(verdict::ALLOW);
    if !findings.is_empty() {
        resp = resp.with_findings(findings);
    }
    if let Some(nt) = next_tool {
        resp = resp.with_recommended_next_tool(nt);
    }
    resp
}

/// Build a preflight "review" response.
///
/// Sets `ok=true`, verdict to `review`, machine_code, findings, and optionally next tool.
pub fn preflight_review(
    tool: &str,
    machine_code: &str,
    result: serde_json::Value,
    findings: Vec<serde_json::Value>,
    next_tool: Option<serde_json::Value>,
) -> ToolResponse {
    let mut resp = ToolResponse::success(result, Some(tool))
        .with_machine_code(machine_code)
        .with_verdict(verdict::REVIEW);
    if !findings.is_empty() {
        resp = resp.with_findings(findings);
    }
    if let Some(nt) = next_tool {
        resp = resp.with_recommended_next_tool(nt);
    }
    resp
}

/// Build a preflight "block" response.
///
/// Sets `ok=true`, verdict to `block`, machine_code, and findings.
pub fn preflight_block(
    tool: &str,
    machine_code: &str,
    result: serde_json::Value,
    findings: Vec<serde_json::Value>,
) -> ToolResponse {
    let mut resp = ToolResponse::success(result, Some(tool))
        .with_machine_code(machine_code)
        .with_verdict(verdict::BLOCK);
    if !findings.is_empty() {
        resp = resp.with_findings(findings);
    }
    resp
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::machine_codes;

    #[test]
    #[allow(deprecated)]
    fn legacy_error_constructor_only_in_tests() {
        let _ = ToolResponse::error_without_code_for_legacy_tests_only("test", "test", None, None);
    }

    #[test]
    fn finding_with_disposition() {
        let f = finding(
            "TEST_CODE",
            severity::MEDIUM,
            "test message",
            Some(disposition::BLOCKING),
            None,
        );
        assert_eq!(f["code"], "TEST_CODE");
        assert_eq!(f["severity"], "medium");
        assert_eq!(f["disposition"], "blocking");
        assert!(f.get("location").is_none());
    }

    #[test]
    fn finding_without_disposition() {
        let f = finding("TEST_CODE", severity::LOW, "test message", None, None);
        assert_eq!(f["code"], "TEST_CODE");
        assert!(f.get("disposition").is_none());
    }

    #[test]
    fn next_tool_structured() {
        let nt = ToolResponse::next_tool("text_diff_explain", "ambiguous replacement", None);
        assert_eq!(nt["name"], "text_diff_explain");
        assert_eq!(nt["reason"], "ambiguous replacement");
        assert!(nt.get("arguments_hint").is_none());
    }

    #[test]
    fn next_tool_with_arguments_hint() {
        let hint = serde_json::json!({"old_text": "foo"});
        let nt = ToolResponse::next_tool("text_diff_explain", "ambiguous", Some(hint));
        assert_eq!(nt["arguments_hint"]["old_text"], "foo");
    }

    #[test]
    fn with_verdict_sets_result_field() {
        let resp = ToolResponse::success(serde_json::json!({}), None).with_verdict(verdict::ALLOW);
        assert_eq!(resp.result.as_ref().unwrap()["verdict"], "allow");
    }

    #[test]
    fn preflight_allow_builder() {
        let resp = preflight_allow(
            "edit_preflight",
            machine_codes::EDIT_OK,
            serde_json::json!({"ok_to_apply": true}),
            vec![],
            None,
        );
        assert!(resp.ok);
        assert_eq!(resp.machine_code.as_deref(), Some("EDIT_OK"));
        assert_eq!(resp.result.as_ref().unwrap()["verdict"], "allow");
    }

    #[test]
    fn preflight_block_builder() {
        let findings = vec![finding(
            "PATCH_FAILED",
            severity::HIGH,
            "patch does not apply",
            Some(disposition::BLOCKING),
            None,
        )];
        let resp = preflight_block(
            "edit_preflight",
            machine_codes::PATCH_FAILED,
            serde_json::json!({"ok_to_apply": false}),
            findings,
        );
        assert!(resp.ok);
        assert_eq!(resp.result.as_ref().unwrap()["verdict"], "block");
        assert_eq!(resp.findings.as_ref().unwrap().len(), 1);
    }
}
