//! Typed preflight wrappers for common codegg workflows.
//!
//! These wrappers provide structured input/output types over the raw JSON
//! tool interface. Each preflight calls the underlying tool handler via
//! `ToolRegistry::call_json` and parses the result into typed fields.
//!
//! # Error Taxonomy
//!
//! Preflight wrappers return `Result<Output, PreflightError>` where
//! `PreflightError` distinguishes three failure modes:
//!
//! - **`ToolCall`** — the registry rejected the call before execution
//!   (unknown tool, invalid arguments, audience mismatch).
//! - **`ToolRejected`** — the tool executed but returned `ok: false`
//!   with an error message and optional machine code.
//! - **`ContractViolation`** — the tool returned `ok: true` but the
//!   response shape violated the typed contract (missing mandatory
//!   field, unexpected type). This is a hard failure — the wrapper
//!   will not silently default missing route-critical fields.
//!
//! # Example
//!
//! ```
//! use eggsact::preflight::{ConfigPreflight, ConfigPreflightInput, ConfigFormat};
//!
//! let input = ConfigPreflightInput {
//!     text: r#"{"key": "value"}"#.to_string(),
//!     format: ConfigFormat::Json,
//!     schema: None,
//!     strict: false,
//! };
//! let output = ConfigPreflight::run(&input).unwrap();
//! assert!(output.valid);
//! ```

use crate::agent::{ToolCallError, ToolRegistry};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

// ---------------------------------------------------------------------------
// PreflightError
// ---------------------------------------------------------------------------

/// Error type for typed preflight wrappers.
///
/// Distinguishes registry-level call errors, tool-level rejections,
/// and typed-contract violations (missing mandatory fields).
#[derive(Debug, Clone)]
pub enum PreflightError {
    /// The registry rejected the call before execution.
    ToolCall(ToolCallError),
    /// The tool executed but returned a non-OK result.
    ToolRejected {
        machine_code: Option<String>,
        error_type: Option<String>,
        message: String,
    },
    /// The tool returned `ok: true` but the response violated the typed contract.
    ContractViolation {
        tool: &'static str,
        field: &'static str,
        message: String,
    },
}

impl fmt::Display for PreflightError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PreflightError::ToolCall(e) => write!(f, "tool call error: {e}"),
            PreflightError::ToolRejected {
                machine_code,
                error_type,
                message,
            } => {
                write!(f, "tool rejected")?;
                if let Some(mc) = machine_code {
                    write!(f, " [{mc}]")?;
                }
                if let Some(et) = error_type {
                    write!(f, " ({et})")?;
                }
                write!(f, ": {message}")
            }
            PreflightError::ContractViolation {
                tool,
                field,
                message,
            } => {
                write!(f, "contract violation in {tool}.{field}: {message}")
            }
        }
    }
}

impl std::error::Error for PreflightError {}

impl From<ToolCallError> for PreflightError {
    fn from(e: ToolCallError) -> Self {
        PreflightError::ToolCall(e)
    }
}

// ---------------------------------------------------------------------------
// Typed enums for stable route fields
// ---------------------------------------------------------------------------

/// Edit preflight verdict.
///
/// Uses `KnownOrOther` for forward compatibility — codegg can handle
/// new values deliberately rather than crashing on unknown variants.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditVerdict {
    SafeToApply,
    SafeWithWarnings,
    Other(String),
}

impl EditVerdict {
    pub fn as_str(&self) -> &str {
        match self {
            EditVerdict::SafeToApply => "safe_to_apply",
            EditVerdict::SafeWithWarnings => "safe_with_warnings",
            EditVerdict::Other(s) => s,
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "safe_to_apply" => EditVerdict::SafeToApply,
            "safe_with_warnings" => EditVerdict::SafeWithWarnings,
            other => EditVerdict::Other(other.to_string()),
        }
    }
}

impl fmt::Display for EditVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Command preflight verdict.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandVerdict {
    Allow,
    Review,
    Block,
    Other(String),
}

impl CommandVerdict {
    pub fn as_str(&self) -> &str {
        match self {
            CommandVerdict::Allow => "allow",
            CommandVerdict::Review => "review",
            CommandVerdict::Block => "block",
            CommandVerdict::Other(s) => s,
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "allow" => CommandVerdict::Allow,
            "review" => CommandVerdict::Review,
            "block" => CommandVerdict::Block,
            other => CommandVerdict::Other(other.to_string()),
        }
    }
}

impl fmt::Display for CommandVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Config preflight verdict.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigVerdict {
    Valid,
    ValidWithWarnings,
    Invalid,
    Other(String),
}

impl ConfigVerdict {
    pub fn as_str(&self) -> &str {
        match self {
            ConfigVerdict::Valid => "valid",
            ConfigVerdict::ValidWithWarnings => "valid_with_warnings",
            ConfigVerdict::Invalid => "invalid",
            ConfigVerdict::Other(s) => s,
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "valid" => ConfigVerdict::Valid,
            "valid_with_warnings" => ConfigVerdict::ValidWithWarnings,
            "invalid" => ConfigVerdict::Invalid,
            other => ConfigVerdict::Other(other.to_string()),
        }
    }
}

impl fmt::Display for ConfigVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Finding severity level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FindingSeverity {
    Info,
    Low,
    Medium,
    High,
    Critical,
    Other(String),
}

impl FindingSeverity {
    pub fn as_str(&self) -> &str {
        match self {
            FindingSeverity::Info => "info",
            FindingSeverity::Low => "low",
            FindingSeverity::Medium => "medium",
            FindingSeverity::High => "high",
            FindingSeverity::Critical => "critical",
            FindingSeverity::Other(s) => s,
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "info" => FindingSeverity::Info,
            "low" => FindingSeverity::Low,
            "medium" => FindingSeverity::Medium,
            "high" => FindingSeverity::High,
            "critical" => FindingSeverity::Critical,
            other => FindingSeverity::Other(other.to_string()),
        }
    }
}

/// Finding disposition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FindingDisposition {
    Informational,
    Caution,
    Blocking,
    Other(String),
}

impl FindingDisposition {
    pub fn as_str(&self) -> &str {
        match self {
            FindingDisposition::Informational => "informational",
            FindingDisposition::Caution => "caution",
            FindingDisposition::Blocking => "blocking",
            FindingDisposition::Other(s) => s,
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "informational" => FindingDisposition::Informational,
            "caution" => FindingDisposition::Caution,
            "blocking" => FindingDisposition::Blocking,
            other => FindingDisposition::Other(other.to_string()),
        }
    }
}

// ---------------------------------------------------------------------------
// Shared types
// ---------------------------------------------------------------------------

/// A structured finding from a tool execution.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Finding {
    pub code: String,
    pub severity: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl Finding {
    /// Parse a finding from a JSON value.
    pub fn from_value(v: &Value) -> Option<Self> {
        Some(Self {
            code: v.get("code")?.as_str()?.to_string(),
            severity: v.get("severity")?.as_str()?.to_string(),
            message: v.get("message")?.as_str()?.to_string(),
            location: v.get("location").cloned(),
            details: v.get("details").cloned(),
        })
    }

    /// Parse a list of findings from a JSON array.
    pub fn from_array(arr: &[Value]) -> Vec<Self> {
        arr.iter().filter_map(Finding::from_value).collect()
    }
}

/// Line ending policy for edit operations.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub enum LineEndingPolicy {
    #[default]
    Preserve,
    NormalizeLf,
    NormalizeCrlf,
}

impl LineEndingPolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            LineEndingPolicy::Preserve => "preserve",
            LineEndingPolicy::NormalizeLf => "normalize_lf",
            LineEndingPolicy::NormalizeCrlf => "normalize_crlf",
        }
    }
}

/// Unicode policy for edit operations.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub enum UnicodePolicy {
    #[default]
    Raw,
    Nfc,
    Nfkc,
    Casefold,
    WhitespaceCollapse,
}

impl UnicodePolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            UnicodePolicy::Raw => "exact",
            UnicodePolicy::Nfc => "nfc",
            UnicodePolicy::Nfkc => "nfkc",
            UnicodePolicy::Casefold => "casefold",
            UnicodePolicy::WhitespaceCollapse => "whitespace_collapse",
        }
    }
}

/// Edit replacement mode.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub enum ReplacementMode {
    #[default]
    Literal,
    Patch,
    LineRange,
}

impl ReplacementMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ReplacementMode::Literal => "literal",
            ReplacementMode::Patch => "patch",
            ReplacementMode::LineRange => "line_range",
        }
    }
}

/// Command analysis policy.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub enum CommandPolicy {
    #[default]
    Default,
    Strict,
    Permissive,
}

impl CommandPolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            CommandPolicy::Default => "default",
            CommandPolicy::Strict => "strict",
            CommandPolicy::Permissive => "permissive",
        }
    }
}

/// Config format for config preflight.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub enum ConfigFormat {
    #[default]
    Auto,
    Json,
    Toml,
    Dotenv,
    Ini,
    CargoToml,
}

impl ConfigFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            ConfigFormat::Auto => "auto",
            ConfigFormat::Json => "json",
            ConfigFormat::Toml => "toml",
            ConfigFormat::Dotenv => "dotenv",
            ConfigFormat::Ini => "ini",
            ConfigFormat::CargoToml => "cargo_toml",
        }
    }
}

// ---------------------------------------------------------------------------
// Strict contract parsing helpers
// ---------------------------------------------------------------------------

/// Extract a mandatory string field from a JSON object.
/// Returns `ContractViolation` if missing or not a string.
fn require_str<'a>(
    result: &'a Value,
    tool: &'static str,
    field: &'static str,
) -> Result<&'a str, PreflightError> {
    result
        .get(field)
        .and_then(|v| v.as_str())
        .ok_or_else(|| PreflightError::ContractViolation {
            tool,
            field,
            message: format!("expected string, got {:?}", result.get(field)),
        })
}

/// Extract a mandatory boolean field from a JSON object.
/// Returns `ContractViolation` if missing or not a boolean.
fn require_bool(
    result: &Value,
    tool: &'static str,
    field: &'static str,
) -> Result<bool, PreflightError> {
    result
        .get(field)
        .and_then(|v| v.as_bool())
        .ok_or_else(|| PreflightError::ContractViolation {
            tool,
            field,
            message: format!("expected boolean, got {:?}", result.get(field)),
        })
}

/// Extract the `result` object from a successful ToolResponse.
/// Returns `ContractViolation` if `result` is None or not an object.
fn require_result_object<'a>(
    response: &'a crate::mcp::response::ToolResponse,
    tool: &'static str,
) -> Result<&'a Value, PreflightError> {
    response
        .result
        .as_ref()
        .filter(|r| r.is_object())
        .ok_or_else(|| PreflightError::ContractViolation {
            tool,
            field: "result",
            message: format!(
                "expected object, got {:?}",
                response.result.as_ref().map(|v| v.to_string())
            ),
        })
}

/// Extract machine_code from a ToolResponse. Returns ContractViolation if missing.
fn require_machine_code(
    response: &crate::mcp::response::ToolResponse,
    tool: &'static str,
) -> Result<String, PreflightError> {
    response
        .machine_code
        .clone()
        .ok_or_else(|| PreflightError::ContractViolation {
            tool,
            field: "machine_code",
            message: "missing mandatory machine_code".to_string(),
        })
}

// ---------------------------------------------------------------------------
// Edit Preflight
// ---------------------------------------------------------------------------

/// Input for edit preflight analysis.
#[derive(Clone, Debug)]
pub struct EditPreflightInput {
    /// Original source text.
    pub original: String,
    /// Edit mode: literal, patch, or line_range.
    pub mode: ReplacementMode,
    /// Text to find (literal mode).
    pub old: Option<String>,
    /// Replacement text (literal mode).
    pub new: Option<String>,
    /// Unified diff patch (patch mode).
    pub patch: Option<String>,
    /// First line (line_range mode).
    pub start_line: Option<u64>,
    /// Last line inclusive (line_range mode).
    pub end_line: Option<u64>,
    /// Expected SHA-256 fingerprint for verification.
    pub expected_fingerprint: Option<String>,
    /// Strict mode for patch matching.
    pub strict: bool,
}

impl Default for EditPreflightInput {
    fn default() -> Self {
        Self {
            original: String::new(),
            mode: ReplacementMode::default(),
            old: None,
            new: None,
            patch: None,
            start_line: None,
            end_line: None,
            expected_fingerprint: None,
            strict: true,
        }
    }
}

/// Output from edit preflight analysis.
#[derive(Clone, Debug)]
pub struct EditPreflightOutput {
    /// Whether the edit is safe to apply.
    pub ok_to_apply: bool,
    /// Edit mode used.
    pub mode: String,
    /// Machine-readable status code.
    pub machine_code: String,
    /// Human-readable summary.
    pub summary: String,
    /// Structured findings.
    pub findings: Vec<Finding>,
    /// Recommended next tool to call.
    pub recommended_next_tool: Option<String>,
    /// The raw tool response for diagnostics and forward compatibility.
    pub raw: Value,
}

/// Typed wrapper for the `edit_preflight` tool.
pub struct EditPreflight;

impl EditPreflight {
    const TOOL: &'static str = "edit_preflight";

    /// Run edit preflight analysis.
    pub fn run(input: &EditPreflightInput) -> Result<EditPreflightOutput, PreflightError> {
        let registry = ToolRegistry::default();
        let mut args = serde_json::json!({
            "original": input.original,
            "replacement_mode": input.mode.as_str(),
            "strict": input.strict,
        });
        if let Some(ref old) = input.old {
            args["old"] = Value::String(old.clone());
        }
        if let Some(ref new) = input.new {
            args["new"] = Value::String(new.clone());
        }
        if let Some(ref patch) = input.patch {
            args["patch"] = Value::String(patch.clone());
        }
        if let Some(start) = input.start_line {
            args["start_line"] = Value::Number(start.into());
        }
        if let Some(end) = input.end_line {
            args["end_line"] = Value::Number(end.into());
        }
        if let Some(ref fp) = input.expected_fingerprint {
            args["expected_fingerprint"] = Value::String(fp.clone());
        }

        let response = registry.call_json(Self::TOOL, args)?;
        Self::parse_response(response)
    }

    /// Parse a ToolResponse into a typed EditPreflightOutput.
    ///
    /// This is public so tests can exercise contract parsing without
    /// calling the full registry.
    pub fn parse_response(
        response: crate::mcp::response::ToolResponse,
    ) -> Result<EditPreflightOutput, PreflightError> {
        if !response.ok {
            return Err(PreflightError::ToolRejected {
                machine_code: response.machine_code,
                error_type: response.error_type,
                message: response.error.unwrap_or_default(),
            });
        }

        let result = require_result_object(&response, Self::TOOL)?;
        let machine_code = require_machine_code(&response, Self::TOOL)?;
        let mode = require_str(result, Self::TOOL, "mode")?.to_string();
        let summary = require_str(result, Self::TOOL, "summary")?.to_string();
        let ok_to_apply = require_bool(result, Self::TOOL, "ok_to_apply")?;

        let findings = response
            .findings
            .as_ref()
            .map(|f| Finding::from_array(f))
            .unwrap_or_default();

        let recommended_next_tool = response
            .recommended_next_tool
            .as_ref()
            .and_then(|v| v.as_str())
            .map(String::from);

        let raw = response.result.unwrap_or(Value::Null);

        Ok(EditPreflightOutput {
            ok_to_apply,
            mode,
            machine_code,
            summary,
            findings,
            recommended_next_tool,
            raw,
        })
    }
}

// ---------------------------------------------------------------------------
// Command Preflight
// ---------------------------------------------------------------------------

/// Input for command preflight analysis.
#[derive(Clone, Debug)]
pub struct CommandPreflightInput {
    /// Command string to analyze.
    pub command: String,
    /// Target platform.
    pub platform: String,
    /// Analysis policy.
    pub policy: CommandPolicy,
    /// Working directory context (informational).
    pub working_directory: Option<String>,
}

impl Default for CommandPreflightInput {
    fn default() -> Self {
        Self {
            command: String::new(),
            platform: "posix".to_string(),
            policy: CommandPolicy::default(),
            working_directory: None,
        }
    }
}

/// Output from command preflight analysis.
#[derive(Clone, Debug)]
pub struct CommandPreflightOutput {
    /// Verdict: "allow", "review", or "block".
    pub verdict: String,
    /// Machine-readable status code.
    pub machine_code: String,
    /// Human-readable summary.
    pub summary: String,
    /// Structured findings.
    pub findings: Vec<Finding>,
    /// Parsed argv if available.
    pub argv: Option<Vec<String>>,
    /// The raw tool response for diagnostics and forward compatibility.
    pub raw: Value,
}

/// Typed wrapper for the `command_preflight` tool.
pub struct CommandPreflight;

impl CommandPreflight {
    const TOOL: &'static str = "command_preflight";

    /// Run command preflight analysis.
    pub fn run(input: &CommandPreflightInput) -> Result<CommandPreflightOutput, PreflightError> {
        let registry = ToolRegistry::default();
        let mut args = serde_json::json!({
            "command": input.command,
            "platform": input.platform,
            "policy": input.policy.as_str(),
        });
        if let Some(ref wd) = input.working_directory {
            args["working_directory"] = Value::String(wd.clone());
        }

        let response = registry.call_json(Self::TOOL, args)?;
        Self::parse_response(response)
    }

    /// Parse a ToolResponse into a typed CommandPreflightOutput.
    pub fn parse_response(
        response: crate::mcp::response::ToolResponse,
    ) -> Result<CommandPreflightOutput, PreflightError> {
        if !response.ok {
            return Err(PreflightError::ToolRejected {
                machine_code: response.machine_code,
                error_type: response.error_type,
                message: response.error.unwrap_or_default(),
            });
        }

        let result = require_result_object(&response, Self::TOOL)?;
        let machine_code = require_machine_code(&response, Self::TOOL)?;
        let verdict = require_str(result, Self::TOOL, "verdict")?.to_string();
        let summary = require_str(result, Self::TOOL, "summary")?.to_string();

        let findings = response
            .findings
            .as_ref()
            .map(|f| Finding::from_array(f))
            .unwrap_or_default();

        let argv = result.get("argv").and_then(|v| v.as_array()).map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        });

        let raw = response.result.unwrap_or(Value::Null);

        Ok(CommandPreflightOutput {
            verdict,
            machine_code,
            summary,
            findings,
            argv,
            raw,
        })
    }
}

// ---------------------------------------------------------------------------
// Config Preflight
// ---------------------------------------------------------------------------

/// Input for config preflight analysis.
#[derive(Clone, Debug, Default)]
pub struct ConfigPreflightInput {
    /// Config text to validate.
    pub text: String,
    /// Config format (auto-detect if not specified).
    pub format: ConfigFormat,
    /// Optional JSON schema for validation.
    pub schema: Option<Value>,
    /// Strict validation mode.
    pub strict: bool,
}

/// Output from config preflight analysis.
#[derive(Clone, Debug)]
pub struct ConfigPreflightOutput {
    /// Whether the config is structurally valid.
    pub valid: bool,
    /// Verdict: "valid", "valid_with_warnings", or "invalid".
    pub verdict: String,
    /// Detected config format.
    pub detected_format: Option<String>,
    /// Machine-readable status code.
    pub machine_code: String,
    /// Human-readable summary.
    pub summary: String,
    /// Structured findings.
    pub findings: Vec<Finding>,
    /// The raw tool response for diagnostics and forward compatibility.
    pub raw: Value,
}

/// Typed wrapper for the `config_preflight` tool.
pub struct ConfigPreflight;

impl ConfigPreflight {
    const TOOL: &'static str = "config_preflight";

    /// Run config preflight analysis.
    pub fn run(input: &ConfigPreflightInput) -> Result<ConfigPreflightOutput, PreflightError> {
        let registry = ToolRegistry::default();
        let mut args = serde_json::json!({
            "text": input.text,
            "format": input.format.as_str(),
            "strict": input.strict,
        });
        if let Some(ref schema) = input.schema {
            args["schema"] = schema.clone();
        }

        let response = registry.call_json(Self::TOOL, args)?;
        Self::parse_response(response)
    }

    /// Parse a ToolResponse into a typed ConfigPreflightOutput.
    pub fn parse_response(
        response: crate::mcp::response::ToolResponse,
    ) -> Result<ConfigPreflightOutput, PreflightError> {
        if !response.ok {
            return Err(PreflightError::ToolRejected {
                machine_code: response.machine_code,
                error_type: response.error_type,
                message: response.error.unwrap_or_default(),
            });
        }

        let result = require_result_object(&response, Self::TOOL)?;
        let machine_code = require_machine_code(&response, Self::TOOL)?;
        let valid = require_bool(result, Self::TOOL, "valid")?;
        let verdict = require_str(result, Self::TOOL, "verdict")?.to_string();
        let summary = require_str(result, Self::TOOL, "summary")?.to_string();

        let detected_format = result
            .get("format")
            .and_then(|v| v.as_str())
            .map(String::from);

        let findings = response
            .findings
            .as_ref()
            .map(|f| Finding::from_array(f))
            .unwrap_or_default();

        let raw = response.result.unwrap_or(Value::Null);

        Ok(ConfigPreflightOutput {
            valid,
            verdict,
            detected_format,
            machine_code,
            summary,
            findings,
            raw,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Profile;
    use crate::mcp::response::ToolResponse;

    // -- Success path tests --

    #[test]
    fn config_preflight_json_valid() {
        let input = ConfigPreflightInput {
            text: r#"{"key": "value"}"#.to_string(),
            format: ConfigFormat::Json,
            ..Default::default()
        };
        let output = ConfigPreflight::run(&input).unwrap();
        assert!(output.valid);
        assert_eq!(output.verdict, "valid");
        assert!(!output.machine_code.is_empty());
        assert!(!output.summary.is_empty());
    }

    #[test]
    fn config_preflight_json_invalid() {
        let input = ConfigPreflightInput {
            text: "{invalid json}".to_string(),
            format: ConfigFormat::Json,
            ..Default::default()
        };
        let output = ConfigPreflight::run(&input).unwrap();
        assert!(!output.valid);
        assert_eq!(output.verdict, "invalid");
    }

    #[test]
    fn command_preflight_safe() {
        let input = CommandPreflightInput {
            command: "ls -la".to_string(),
            ..Default::default()
        };
        let output = CommandPreflight::run(&input).unwrap();
        assert_eq!(output.verdict, "allow");
        assert!(!output.machine_code.is_empty());
    }

    #[test]
    fn finding_from_value() {
        let v = serde_json::json!({
            "code": "TEST_CODE",
            "severity": "info",
            "message": "test message"
        });
        let f = Finding::from_value(&v).unwrap();
        assert_eq!(f.code, "TEST_CODE");
        assert_eq!(f.severity, "info");
        assert_eq!(f.message, "test message");
    }

    #[test]
    fn profile_as_str_roundtrip() {
        let profiles = vec![
            Profile::Full,
            Profile::Default,
            Profile::CodeggCoreMin,
            Profile::CodeggCore,
            Profile::CodeggPreflight,
            Profile::CodeggPatch,
            Profile::CodeggConfig,
            Profile::CodeggUnicodeSecurity,
            Profile::CodeggShell,
            Profile::CodeggRepoAudit,
            Profile::HumanMath,
        ];
        for profile in profiles {
            let s = profile.as_str();
            let parsed = Profile::from_str_opt(s).unwrap();
            assert_eq!(profile, parsed);
        }
    }

    // -- Contract violation tests (malformed responses fail closed) --

    #[test]
    fn edit_preflight_missing_machine_code_fails_closed() {
        let response = ToolResponse::success(
            serde_json::json!({
                "ok_to_apply": true,
                "mode": "literal",
                "summary": "test",
            }),
            Some("edit_preflight"),
        );
        let err = EditPreflight::parse_response(response).unwrap_err();
        assert!(
            matches!(err, PreflightError::ContractViolation { field, .. } if field == "machine_code")
        );
    }

    #[test]
    fn edit_preflight_missing_result_object_fails_closed() {
        let mut response = ToolResponse::success(serde_json::json!(null), Some("edit_preflight"));
        response.machine_code = Some("EDIT_OK".to_string());
        let err = EditPreflight::parse_response(response).unwrap_err();
        assert!(
            matches!(err, PreflightError::ContractViolation { field, .. } if field == "result")
        );
    }

    #[test]
    fn edit_preflight_missing_ok_to_apply_fails_closed() {
        let response = ToolResponse::success(
            serde_json::json!({
                "mode": "literal",
                "summary": "test",
            }),
            Some("edit_preflight"),
        )
        .with_machine_code("EDIT_OK");
        let err = EditPreflight::parse_response(response).unwrap_err();
        assert!(
            matches!(err, PreflightError::ContractViolation { field, .. } if field == "ok_to_apply")
        );
    }

    #[test]
    fn edit_preflight_missing_mode_fails_closed() {
        let response = ToolResponse::success(
            serde_json::json!({
                "ok_to_apply": true,
                "summary": "test",
            }),
            Some("edit_preflight"),
        )
        .with_machine_code("EDIT_OK");
        let err = EditPreflight::parse_response(response).unwrap_err();
        assert!(matches!(err, PreflightError::ContractViolation { field, .. } if field == "mode"));
    }

    #[test]
    fn edit_preflight_missing_summary_fails_closed() {
        let response = ToolResponse::success(
            serde_json::json!({
                "ok_to_apply": true,
                "mode": "literal",
            }),
            Some("edit_preflight"),
        )
        .with_machine_code("EDIT_OK");
        let err = EditPreflight::parse_response(response).unwrap_err();
        assert!(
            matches!(err, PreflightError::ContractViolation { field, .. } if field == "summary")
        );
    }

    #[test]
    fn command_preflight_missing_verdict_fails_closed() {
        let response = ToolResponse::success(
            serde_json::json!({
                "summary": "test",
            }),
            Some("command_preflight"),
        )
        .with_machine_code("COMMAND_OK");
        let err = CommandPreflight::parse_response(response).unwrap_err();
        assert!(
            matches!(err, PreflightError::ContractViolation { field, .. } if field == "verdict")
        );
    }

    #[test]
    fn command_preflight_missing_machine_code_fails_closed() {
        let response = ToolResponse::success(
            serde_json::json!({
                "verdict": "allow",
                "summary": "test",
            }),
            Some("command_preflight"),
        );
        let err = CommandPreflight::parse_response(response).unwrap_err();
        assert!(
            matches!(err, PreflightError::ContractViolation { field, .. } if field == "machine_code")
        );
    }

    #[test]
    fn config_preflight_missing_valid_fails_closed() {
        let response = ToolResponse::success(
            serde_json::json!({
                "verdict": "valid",
                "summary": "test",
            }),
            Some("config_preflight"),
        )
        .with_machine_code("CONFIG_OK");
        let err = ConfigPreflight::parse_response(response).unwrap_err();
        assert!(matches!(err, PreflightError::ContractViolation { field, .. } if field == "valid"));
    }

    #[test]
    fn config_preflight_missing_verdict_fails_closed() {
        let response = ToolResponse::success(
            serde_json::json!({
                "valid": true,
                "summary": "test",
            }),
            Some("config_preflight"),
        )
        .with_machine_code("CONFIG_OK");
        let err = ConfigPreflight::parse_response(response).unwrap_err();
        assert!(
            matches!(err, PreflightError::ContractViolation { field, .. } if field == "verdict")
        );
    }

    #[test]
    fn config_preflight_missing_summary_fails_closed() {
        let response = ToolResponse::success(
            serde_json::json!({
                "valid": true,
                "verdict": "valid",
            }),
            Some("config_preflight"),
        )
        .with_machine_code("CONFIG_OK");
        let err = ConfigPreflight::parse_response(response).unwrap_err();
        assert!(
            matches!(err, PreflightError::ContractViolation { field, .. } if field == "summary")
        );
    }

    #[test]
    fn config_preflight_missing_machine_code_fails_closed() {
        let response = ToolResponse::success(
            serde_json::json!({
                "valid": true,
                "verdict": "valid",
                "summary": "test",
            }),
            Some("config_preflight"),
        );
        let err = ConfigPreflight::parse_response(response).unwrap_err();
        assert!(
            matches!(err, PreflightError::ContractViolation { field, .. } if field == "machine_code")
        );
    }

    #[test]
    fn command_preflight_missing_summary_fails_closed() {
        let response = ToolResponse::success(
            serde_json::json!({
                "verdict": "allow",
            }),
            Some("command_preflight"),
        )
        .with_machine_code("COMMAND_OK");
        let err = CommandPreflight::parse_response(response).unwrap_err();
        assert!(
            matches!(err, PreflightError::ContractViolation { field, .. } if field == "summary")
        );
    }

    // -- Tool rejection test --

    #[test]
    fn tool_rejection_maps_to_tool_rejected_error() {
        let response = ToolResponse::error_with_code(
            "validation",
            "INVALID_ARGUMENTS",
            "bad input",
            None,
            Some("config_preflight"),
        );
        let err = ConfigPreflight::parse_response(response).unwrap_err();
        match &err {
            PreflightError::ToolRejected {
                machine_code,
                error_type,
                message,
            } => {
                assert_eq!(machine_code.as_deref(), Some("INVALID_ARGUMENTS"));
                assert_eq!(error_type.as_deref(), Some("validation"));
                assert_eq!(message, "bad input");
            }
            _ => panic!("expected ToolRejected, got: {err}"),
        }
    }

    // -- Regression: no empty-string defaults for mandatory fields --

    #[test]
    fn edit_preflight_no_empty_string_defaults() {
        let response = ToolResponse::success(
            serde_json::json!({
                "ok_to_apply": true,
                "mode": "literal",
                "summary": "looks good",
            }),
            Some("edit_preflight"),
        )
        .with_machine_code("EDIT_OK");
        let output = EditPreflight::parse_response(response).unwrap();
        assert!(!output.machine_code.is_empty());
        assert!(!output.mode.is_empty());
        assert!(!output.summary.is_empty());
    }

    #[test]
    fn command_preflight_no_empty_string_defaults() {
        let response = ToolResponse::success(
            serde_json::json!({
                "verdict": "allow",
                "summary": "safe command",
            }),
            Some("command_preflight"),
        )
        .with_machine_code("COMMAND_OK");
        let output = CommandPreflight::parse_response(response).unwrap();
        assert!(!output.machine_code.is_empty());
        assert!(!output.verdict.is_empty());
        assert!(!output.summary.is_empty());
    }

    #[test]
    fn config_preflight_no_empty_string_defaults() {
        let response = ToolResponse::success(
            serde_json::json!({
                "valid": true,
                "verdict": "valid",
                "summary": "config is good",
            }),
            Some("config_preflight"),
        )
        .with_machine_code("CONFIG_OK");
        let output = ConfigPreflight::parse_response(response).unwrap();
        assert!(!output.machine_code.is_empty());
        assert!(!output.verdict.is_empty());
        assert!(!output.summary.is_empty());
    }

    // -- Typed enum tests --

    #[test]
    fn edit_verdict_roundtrip() {
        assert_eq!(EditVerdict::SafeToApply.as_str(), "safe_to_apply");
        assert_eq!(EditVerdict::SafeWithWarnings.as_str(), "safe_with_warnings");
        let other = EditVerdict::Other("new_value".to_string());
        assert_eq!(other.as_str(), "new_value");
        assert_eq!(
            EditVerdict::parse("safe_to_apply"),
            EditVerdict::SafeToApply
        );
        assert_eq!(
            EditVerdict::parse("unknown"),
            EditVerdict::Other("unknown".to_string())
        );
    }

    #[test]
    fn command_verdict_roundtrip() {
        assert_eq!(CommandVerdict::Allow.as_str(), "allow");
        assert_eq!(CommandVerdict::Review.as_str(), "review");
        assert_eq!(CommandVerdict::Block.as_str(), "block");
        assert_eq!(CommandVerdict::parse("allow"), CommandVerdict::Allow);
        assert_eq!(
            CommandVerdict::parse("unknown"),
            CommandVerdict::Other("unknown".to_string())
        );
    }

    #[test]
    fn config_verdict_roundtrip() {
        assert_eq!(ConfigVerdict::Valid.as_str(), "valid");
        assert_eq!(
            ConfigVerdict::ValidWithWarnings.as_str(),
            "valid_with_warnings"
        );
        assert_eq!(ConfigVerdict::Invalid.as_str(), "invalid");
        assert_eq!(ConfigVerdict::parse("valid"), ConfigVerdict::Valid);
    }

    #[test]
    fn finding_severity_roundtrip() {
        assert_eq!(FindingSeverity::Info.as_str(), "info");
        assert_eq!(FindingSeverity::Critical.as_str(), "critical");
        assert_eq!(FindingSeverity::parse("info"), FindingSeverity::Info);
        assert_eq!(
            FindingSeverity::parse("unknown"),
            FindingSeverity::Other("unknown".to_string())
        );
    }

    #[test]
    fn finding_disposition_roundtrip() {
        assert_eq!(FindingDisposition::Informational.as_str(), "informational");
        assert_eq!(FindingDisposition::Blocking.as_str(), "blocking");
        assert_eq!(
            FindingDisposition::parse("caution"),
            FindingDisposition::Caution
        );
    }

    // -- PreflightError display test --

    #[test]
    fn preflight_error_display() {
        let tc = PreflightError::ToolCall(ToolCallError::UnknownTool("foo".into()));
        assert!(tc.to_string().contains("foo"));

        let tr = PreflightError::ToolRejected {
            machine_code: Some("BAD".into()),
            error_type: Some("validation".into()),
            message: "nope".into(),
        };
        assert!(tr.to_string().contains("BAD"));
        assert!(tr.to_string().contains("nope"));

        let cv = PreflightError::ContractViolation {
            tool: "my_tool",
            field: "x",
            message: "missing".into(),
        };
        assert!(cv.to_string().contains("my_tool.x"));
    }

    // -- Missing result entirely (ok=true but no result object) --

    #[test]
    fn edit_preflight_null_result_fails_closed() {
        let mut response = ToolResponse::success(Value::Null, Some("edit_preflight"));
        response.machine_code = Some("EDIT_OK".to_string());
        let err = EditPreflight::parse_response(response).unwrap_err();
        assert!(
            matches!(err, PreflightError::ContractViolation { field, .. } if field == "result")
        );
    }

    #[test]
    fn command_preflight_null_result_fails_closed() {
        let mut response = ToolResponse::success(Value::Null, Some("command_preflight"));
        response.machine_code = Some("COMMAND_OK".to_string());
        let err = CommandPreflight::parse_response(response).unwrap_err();
        assert!(
            matches!(err, PreflightError::ContractViolation { field, .. } if field == "result")
        );
    }

    #[test]
    fn config_preflight_null_result_fails_closed() {
        let mut response = ToolResponse::success(Value::Null, Some("config_preflight"));
        response.machine_code = Some("CONFIG_OK".to_string());
        let err = ConfigPreflight::parse_response(response).unwrap_err();
        assert!(
            matches!(err, PreflightError::ContractViolation { field, .. } if field == "result")
        );
    }
}
