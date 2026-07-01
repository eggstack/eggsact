//! Typed preflight wrappers for common codegg workflows.
//!
//! These wrappers provide structured input/output types over the raw JSON
//! tool interface. Each preflight calls the underlying tool handler via
//! `ToolRegistry::call_json` and parses the result into typed fields.
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
    /// The raw tool response for forward compatibility.
    pub raw: Value,
}

/// Typed wrapper for the `edit_preflight` tool.
pub struct EditPreflight;

impl EditPreflight {
    /// Run edit preflight analysis.
    pub fn run(input: &EditPreflightInput) -> Result<EditPreflightOutput, ToolCallError> {
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

        let response = registry.call_json("edit_preflight", args)?;
        Ok(EditPreflightOutput {
            ok_to_apply: response
                .result
                .as_ref()
                .and_then(|r| r.get("ok_to_apply"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            mode: response
                .result
                .as_ref()
                .and_then(|r| r.get("mode"))
                .and_then(|v| v.as_str())
                .unwrap_or("literal")
                .to_string(),
            machine_code: response.machine_code.clone().unwrap_or_default(),
            summary: response
                .result
                .as_ref()
                .and_then(|r| r.get("summary"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            findings: response
                .findings
                .as_ref()
                .map(|f| Finding::from_array(f))
                .unwrap_or_default(),
            recommended_next_tool: response
                .recommended_next_tool
                .as_ref()
                .and_then(|v| v.as_str())
                .map(String::from),
            raw: response.result.unwrap_or(Value::Null),
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
    /// The raw tool response for forward compatibility.
    pub raw: Value,
}

/// Typed wrapper for the `command_preflight` tool.
pub struct CommandPreflight;

impl CommandPreflight {
    /// Run command preflight analysis.
    pub fn run(input: &CommandPreflightInput) -> Result<CommandPreflightOutput, ToolCallError> {
        let registry = ToolRegistry::default();
        let mut args = serde_json::json!({
            "command": input.command,
            "platform": input.platform,
            "policy": input.policy.as_str(),
        });
        if let Some(ref wd) = input.working_directory {
            args["working_directory"] = Value::String(wd.clone());
        }

        let response = registry.call_json("command_preflight", args)?;
        let result = response.result.as_ref();
        Ok(CommandPreflightOutput {
            verdict: result
                .and_then(|r| r.get("verdict"))
                .and_then(|v| v.as_str())
                .unwrap_or("block")
                .to_string(),
            machine_code: response.machine_code.clone().unwrap_or_default(),
            summary: result
                .and_then(|r| r.get("summary"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            findings: response
                .findings
                .as_ref()
                .map(|f| Finding::from_array(f))
                .unwrap_or_default(),
            argv: result
                .and_then(|r| r.get("argv"))
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                }),
            raw: response.result.unwrap_or(Value::Null),
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
    /// The raw tool response for forward compatibility.
    pub raw: Value,
}

/// Typed wrapper for the `config_preflight` tool.
pub struct ConfigPreflight;

impl ConfigPreflight {
    /// Run config preflight analysis.
    pub fn run(input: &ConfigPreflightInput) -> Result<ConfigPreflightOutput, ToolCallError> {
        let registry = ToolRegistry::default();
        let mut args = serde_json::json!({
            "text": input.text,
            "format": input.format.as_str(),
            "strict": input.strict,
        });
        if let Some(ref schema) = input.schema {
            args["schema"] = schema.clone();
        }

        let response = registry.call_json("config_preflight", args)?;
        let result = response.result.as_ref();
        Ok(ConfigPreflightOutput {
            valid: result
                .and_then(|r| r.get("valid"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            verdict: result
                .and_then(|r| r.get("verdict"))
                .and_then(|v| v.as_str())
                .unwrap_or("invalid")
                .to_string(),
            detected_format: result
                .and_then(|r| r.get("format"))
                .and_then(|v| v.as_str())
                .map(String::from),
            machine_code: response.machine_code.clone().unwrap_or_default(),
            summary: result
                .and_then(|r| r.get("summary"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            findings: response
                .findings
                .as_ref()
                .map(|f| Finding::from_array(f))
                .unwrap_or_default(),
            raw: response.result.unwrap_or(Value::Null),
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
}
