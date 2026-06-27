use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

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

#[derive(Deserialize, Debug)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: Option<serde_json::Value>,
    pub id: Option<serde_json::Value>,
}

#[derive(Serialize, Debug)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub result: serde_json::Value,
}

#[derive(Serialize, Debug)]
pub struct JsonRpcError {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub error: JsonRpcErrorDetail,
}

#[derive(Serialize, Debug)]
pub struct JsonRpcErrorDetail {
    pub code: i32,
    pub message: String,
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

    pub fn error(
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
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub protocol_version: String,
    pub capabilities: Capabilities,
    pub server_info: ServerInfo,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub tools: ToolsCapability,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ToolsCapability {
    pub list_changed: bool,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}
