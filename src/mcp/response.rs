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
        since = "1.0.0",
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

/// Truncate a `ToolResponse` to fit within the given budget limits.
///
/// This applies deterministic truncation rules:
/// - `findings` array is capped at `budget.max_findings` (highest-severity first).
/// - `result` string representation is capped at `budget.max_output_bytes`.
/// - `limits_applied` is populated with any truncation that occurred.
///
/// The tool's `machine_code` is set to `OUTPUT_TOO_LARGE` only when truncation
/// changes the routing verdict (e.g., a route-critical tool loses all findings).
pub fn truncate_response(response: &mut ToolResponse, budget: &crate::mcp::budget::ToolBudget) {
    let mut limits: Vec<String> = Vec::new();

    // Truncate findings array — keep total length <= max_findings.
    // We reserve one slot for the synthetic truncation notice so the
    // observed cap is `max_findings - 1` real findings + 1 notice,
    // never exceeding `max_findings` total.
    if let Some(ref mut findings) = response.findings {
        if findings.len() > budget.max_findings {
            // Sort by severity (highest first) before truncating
            let severity_order = |s: &str| match s {
                "critical" => 0,
                "high" => 1,
                "medium" => 2,
                "low" => 3,
                "info" => 4,
                _ => 5,
            };
            findings.sort_by(|a, b| {
                let sev_a = a.get("severity").and_then(|v| v.as_str()).unwrap_or("info");
                let sev_b = b.get("severity").and_then(|v| v.as_str()).unwrap_or("info");
                severity_order(sev_a).cmp(&severity_order(sev_b))
            });
            let omitted = findings.len().saturating_sub(budget.max_findings);
            // Reserve last slot for the truncation notice.
            let real_cap = budget.max_findings.saturating_sub(1);
            findings.truncate(real_cap);
            findings.push(serde_json::json!({
                "code": "OUTPUT_TOO_LARGE",
                "severity": "info",
                "message": format!("{} findings omitted due to output budget", omitted),
                "disposition": "informational",
            }));
            limits.push(format!("findings_truncated:{}", omitted));
        }
    }

    // Truncate result string if it exists and is too large. We replace
    // oversized result with a summary object that preserves route-critical
    // top-level keys (machine_code, verdict, ok, summary) so callers can
    // still route on the response. The existing user-supplied `summary`
    // field is kept; only if absent do we emit a default placeholder.
    if let Some(ref mut result) = response.result {
        let result_str = serde_json::to_string(result).unwrap_or_default();
        if result_str.len() > budget.max_output_bytes {
            let original_len = result_str.len();
            // Build a small summary and use it as the new result.
            let summary = serde_json::json!({
                "truncated": true,
                "original_size_bytes": original_len,
                "max_output_bytes": budget.max_output_bytes,
            });
            // Preserve route-critical keys at the top level if present.
            let mut merged = serde_json::Map::new();
            if let Some(obj) = result.as_object() {
                for key in &["machine_code", "verdict", "summary", "ok"] {
                    if let Some(v) = obj.get(*key) {
                        merged.insert((*key).to_string(), v.clone());
                    }
                }
            }
            for (k, v) in summary.as_object().unwrap() {
                merged.insert(k.clone(), v.clone());
            }
            // Inject default truncation message only when caller didn't
            // supply a `summary` field — caller-supplied summaries are
            // more informative for harnesses than our boilerplate.
            if !merged.contains_key("summary") {
                merged.insert(
                    "summary".to_string(),
                    serde_json::Value::String(
                        "Result exceeded output budget; full payload suppressed.".to_string(),
                    ),
                );
            }
            *result = serde_json::Value::Object(merged);
            limits.push(format!("result_truncated:{}", original_len));
        }
    }

    if !limits.is_empty() {
        let existing = response.limits_applied.take().unwrap_or_default();
        let mut all_limits = existing;
        all_limits.extend(limits);
        response.limits_applied = Some(all_limits);
    }
}

#[derive(Serialize, Debug, Default, Clone)]
pub struct CallMetrics {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_bytes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_bytes_before_truncation: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_bytes_after_truncation: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_tier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limits_applied_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_tool_count: Option<usize>,
}

impl ToolResponse {
    pub fn with_metrics(mut self, metrics: CallMetrics) -> Self {
        if let Some(ref mut result) = self.result {
            if let Ok(metrics_val) = serde_json::to_value(&metrics) {
                result["_metrics"] = metrics_val;
            }
        }
        self
    }
}

pub struct CallMetricsBuilder {
    metrics: CallMetrics,
}

impl Default for CallMetricsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl CallMetricsBuilder {
    pub fn new() -> Self {
        Self {
            metrics: CallMetrics::default(),
        }
    }

    pub fn elapsed_ms(mut self, ms: u64) -> Self {
        self.metrics.elapsed_ms = Some(ms);
        self
    }

    pub fn input_bytes(mut self, bytes: usize) -> Self {
        self.metrics.input_bytes = Some(bytes);
        self
    }

    pub fn output_bytes_before(mut self, bytes: usize) -> Self {
        self.metrics.output_bytes_before_truncation = Some(bytes);
        self
    }

    pub fn output_bytes_after(mut self, bytes: usize) -> Self {
        self.metrics.output_bytes_after_truncation = Some(bytes);
        self
    }

    pub fn budget_tier(mut self, tier: &str) -> Self {
        self.metrics.budget_tier = Some(tier.to_string());
        self
    }

    pub fn limits_applied_count(mut self, count: usize) -> Self {
        self.metrics.limits_applied_count = Some(count);
        self
    }

    pub fn sub_tool_count(mut self, count: usize) -> Self {
        self.metrics.sub_tool_count = Some(count);
        self
    }

    pub fn build(self) -> CallMetrics {
        self.metrics
    }
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

    #[test]
    fn truncate_findings_within_budget() {
        let budget = crate::mcp::budget::ToolBudget::CHEAP.with_max_findings(3);
        let mut resp = ToolResponse::success(serde_json::json!({}), Some("test_tool"))
            .with_findings(vec![
                finding("A", severity::LOW, "low sev", None, None),
                finding("B", severity::HIGH, "high sev", None, None),
                finding("C", severity::MEDIUM, "medium sev", None, None),
                finding("D", severity::CRITICAL, "critical sev", None, None),
            ]);
        truncate_response(&mut resp, &budget);
        let findings = resp.findings.as_ref().unwrap();
        // Total findings length must not exceed max_findings: keep
        // highest-severity (critical, high) + truncation notice.
        assert_eq!(findings.len(), 3);
        assert_eq!(findings[0]["severity"], "critical");
        assert_eq!(findings[1]["severity"], "high");
        assert_eq!(findings[2]["code"], "OUTPUT_TOO_LARGE");
    }

    #[test]
    fn truncate_findings_populates_limits_applied() {
        let budget = crate::mcp::budget::ToolBudget::CHEAP.with_max_findings(2);
        let mut resp = ToolResponse::success(serde_json::json!({}), Some("test_tool"))
            .with_findings(vec![
                finding("A", severity::LOW, "a", None, None),
                finding("B", severity::LOW, "b", None, None),
                finding("C", severity::LOW, "c", None, None),
                finding("D", severity::LOW, "d", None, None),
            ]);
        truncate_response(&mut resp, &budget);
        assert!(resp.limits_applied.is_some());
        let limits = resp.limits_applied.as_ref().unwrap();
        assert!(limits.iter().any(|l| l.starts_with("findings_truncated:")));
    }

    #[test]
    fn truncate_noop_when_within_budget() {
        let budget = crate::mcp::budget::ToolBudget::CHEAP;
        let mut resp = ToolResponse::success(serde_json::json!({}), Some("test_tool"))
            .with_findings(vec![finding("A", severity::LOW, "a", None, None)]);
        truncate_response(&mut resp, &budget);
        assert_eq!(resp.findings.as_ref().unwrap().len(), 1);
        assert!(resp.limits_applied.is_none());
    }

    #[test]
    fn truncate_findings_total_never_exceeds_cap() {
        // Exact-length contract: total findings length must be <= max_findings
        // regardless of how many findings the source produced.
        for max in 1usize..=10 {
            let budget = crate::mcp::budget::ToolBudget::CHEAP.with_max_findings(max);
            // Produce 5x the cap to force truncation.
            let many: Vec<_> = (0..max * 5)
                .map(|i| {
                    let s = format!("F{}", i);
                    finding(&s, severity::LOW, "msg", None, None)
                })
                .collect();
            let mut resp =
                ToolResponse::success(serde_json::json!({}), Some("t")).with_findings(many);
            truncate_response(&mut resp, &budget);
            let findings = resp.findings.as_ref().unwrap();
            assert!(
                findings.len() <= max,
                "max_findings={} but total findings len={}",
                max,
                findings.len()
            );
            // Last entry should be the truncation notice.
            let last = findings.last().unwrap();
            assert_eq!(last["code"], "OUTPUT_TOO_LARGE");
        }
    }

    #[test]
    fn truncate_result_replaces_oversized_payload() {
        // Result truncation is REAL: replaces oversized result with a
        // summary object that preserves route-critical keys.
        let budget = crate::mcp::budget::ToolBudget::CHEAP.with_max_output_bytes(50);
        // Build a large result that's guaranteed to exceed 50 bytes.
        let large_text = "x".repeat(500);
        let mut resp = ToolResponse::success(
            serde_json::json!({
                "machine_code": "EDIT_OK",
                "verdict": "allow",
                "summary": "all good",
                "data": large_text,
            }),
            Some("test_tool"),
        )
        .with_machine_code(machine_codes::EDIT_OK)
        .with_verdict(verdict::ALLOW);
        truncate_response(&mut resp, &budget);
        // Serialized result must now be small enough to fit.
        let result = resp.result.as_ref().unwrap();
        let result_str = serde_json::to_string(result).unwrap();
        assert!(
            result_str.len() <= 200,
            "result should have been replaced; len={}",
            result_str.len()
        );
        // Route-critical keys preserved.
        assert_eq!(result["machine_code"], "EDIT_OK");
        assert_eq!(result["verdict"], "allow");
        // Truncation marker set.
        assert_eq!(result["truncated"], true);
        assert!(result["original_size_bytes"].as_u64().unwrap() > 50);
        assert_eq!(result["max_output_bytes"], 50);
        // limits_applied records the truncation.
        let limits = resp.limits_applied.as_ref().unwrap();
        assert!(limits.iter().any(|l| l.starts_with("result_truncated:")));
    }

    #[test]
    fn truncate_result_preserves_summary_when_present() {
        // Existing top-level `summary` string should be kept alongside the
        // truncation metadata so callers can show a human message.
        let budget = crate::mcp::budget::ToolBudget::CHEAP.with_max_output_bytes(40);
        let mut resp = ToolResponse::success(
            serde_json::json!({
                "machine_code": "EDIT_OK",
                "summary": "edited 3 lines",
                "ok": true,
                "big_blob": "y".repeat(500),
            }),
            Some("test_tool"),
        )
        .with_machine_code(machine_codes::EDIT_OK);
        truncate_response(&mut resp, &budget);
        let result = resp.result.as_ref().unwrap();
        assert_eq!(result["summary"], "edited 3 lines");
        assert_eq!(result["ok"], true);
        assert_eq!(result["truncated"], true);
    }

    #[test]
    fn truncate_under_max_does_not_mutate_result() {
        // When result fits within budget, it must be left intact.
        let budget = crate::mcp::budget::ToolBudget::CHEAP; // 1 MB cap
        let original = serde_json::json!({"k": "v", "n": 42});
        let mut resp = ToolResponse::success(original.clone(), Some("test_tool"));
        truncate_response(&mut resp, &budget);
        assert_eq!(resp.result.as_ref().unwrap(), &original);
        assert!(resp.limits_applied.is_none());
    }

    #[test]
    fn call_metrics_default_all_none() {
        let m = CallMetrics::default();
        let val = serde_json::to_value(&m).unwrap();
        assert_eq!(val, serde_json::json!({}));
    }

    #[test]
    fn call_metrics_builder_sets_fields() {
        let m = CallMetricsBuilder::new()
            .elapsed_ms(42)
            .input_bytes(128)
            .output_bytes_before(256)
            .output_bytes_after(200)
            .budget_tier("standard")
            .limits_applied_count(2)
            .sub_tool_count(3)
            .build();
        assert_eq!(m.elapsed_ms, Some(42));
        assert_eq!(m.input_bytes, Some(128));
        assert_eq!(m.output_bytes_before_truncation, Some(256));
        assert_eq!(m.output_bytes_after_truncation, Some(200));
        assert_eq!(m.budget_tier.as_deref(), Some("standard"));
        assert_eq!(m.limits_applied_count, Some(2));
        assert_eq!(m.sub_tool_count, Some(3));
    }

    #[test]
    fn with_metrics_attaches_to_result() {
        let metrics = CallMetricsBuilder::new().elapsed_ms(10).build();
        let resp =
            ToolResponse::success(serde_json::json!({"key": "val"}), None).with_metrics(metrics);
        let result = resp.result.unwrap();
        assert_eq!(result["_metrics"]["elapsed_ms"], 10);
        assert_eq!(result["key"], "val");
    }

    #[test]
    fn with_metrics_no_result_noop() {
        let metrics = CallMetricsBuilder::new().elapsed_ms(10).build();
        let resp = ToolResponse {
            ok: false,
            tool: None,
            result: None,
            error_type: Some("test".into()),
            error: Some("test".into()),
            hints: None,
            warnings: None,
            limits_applied: None,
            findings: None,
            machine_code: None,
            recommended_next_tool: None,
        }
        .with_metrics(metrics);
        assert!(resp.result.is_none());
    }

    #[test]
    fn call_metrics_serialization_skips_none() {
        let m = CallMetrics {
            elapsed_ms: Some(5),
            input_bytes: None,
            output_bytes_before_truncation: None,
            output_bytes_after_truncation: None,
            budget_tier: None,
            limits_applied_count: None,
            sub_tool_count: None,
        };
        let val = serde_json::to_value(&m).unwrap();
        let obj = val.as_object().unwrap();
        assert_eq!(obj.len(), 1);
        assert_eq!(obj["elapsed_ms"], 5);
    }
}
