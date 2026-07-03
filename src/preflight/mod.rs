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
/// Uses the canonical verdict vocabulary (allow/review/block) matching
/// the actual `edit_preflight` tool output. `SafeToApply`/`SafeWithWarnings`
/// are kept as domain aliases for backward compatibility.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditVerdict {
    Allow,
    Review,
    Block,
    SafeToApply,
    SafeWithWarnings,
    Other(String),
}

impl EditVerdict {
    pub fn as_str(&self) -> &str {
        match self {
            EditVerdict::Allow => "allow",
            EditVerdict::Review => "review",
            EditVerdict::Block => "block",
            EditVerdict::SafeToApply => "safe_to_apply",
            EditVerdict::SafeWithWarnings => "safe_with_warnings",
            EditVerdict::Other(s) => s,
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "allow" => EditVerdict::Allow,
            "review" => EditVerdict::Review,
            "block" => EditVerdict::Block,
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
// Edit preflight policy enums and result structs
// ---------------------------------------------------------------------------

/// Newline policy for edit preflight.
///
/// Controls whether mixed newline styles (CRLF/LF) should be flagged.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditNewlinePolicy {
    /// Do not check newline styles.
    #[default]
    Skip,
    /// Flag mixed newlines (CRLF/LF in same file).
    Check,
    /// Automatically normalize to LF before comparison.
    NormalizeLf,
    /// Automatically normalize to CRLF before comparison.
    NormalizeCrlf,
}

impl EditNewlinePolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            EditNewlinePolicy::Skip => "skip",
            EditNewlinePolicy::Check => "check",
            EditNewlinePolicy::NormalizeLf => "normalize_lf",
            EditNewlinePolicy::NormalizeCrlf => "normalize_crlf",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "skip" => EditNewlinePolicy::Skip,
            "check" => EditNewlinePolicy::Check,
            "normalize_lf" => EditNewlinePolicy::NormalizeLf,
            "normalize_crlf" => EditNewlinePolicy::NormalizeCrlf,
            _ => EditNewlinePolicy::default(),
        }
    }
}

/// Unicode policy for edit preflight.
///
/// Controls how unicode security checks are applied to the replacement text.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditUnicodePolicy {
    /// No unicode security checks.
    #[default]
    Skip,
    /// Run default security checks (invisible chars, confusables, bidi).
    Default,
    /// Run security checks with source_code policy (stricter).
    SourceCode,
    /// Run security checks with identifier policy.
    Identifier,
}

impl EditUnicodePolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            EditUnicodePolicy::Skip => "skip",
            EditUnicodePolicy::Default => "default",
            EditUnicodePolicy::SourceCode => "source_code",
            EditUnicodePolicy::Identifier => "identifier",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "skip" => EditUnicodePolicy::Skip,
            "default" => EditUnicodePolicy::Default,
            "source_code" => EditUnicodePolicy::SourceCode,
            "identifier" => EditUnicodePolicy::Identifier,
            _ => EditUnicodePolicy::default(),
        }
    }
}

/// Edit metadata providing context for preflight analysis.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EditMetadata {
    /// Description of the edit (for logging/diagnostics).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Author or agent performing the edit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Tool that originated this edit (e.g. "apply_patch", "edit_file").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_tool: Option<String>,
    /// Session identifier for traceability.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Request identifier for traceability.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

/// Path scope check result from composing `path_scope_check`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PathScopeResult {
    /// Whether the target path is inside the workspace root.
    pub inside_root: bool,
    /// Whether the path uses `..` traversal.
    pub escapes_via_dotdot: bool,
    /// Normalized relative path from root.
    pub relative_path: String,
    /// Normalized absolute target path (lexical resolution only, no symlink follow).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized_target: Option<String>,
    /// Human-readable reason for the path scope decision.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Newline check result from detecting newline style inconsistencies.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NewlineCheckResult {
    /// Composite detected newline style: "LF", "CRLF", "CR", "mixed", or "none".
    pub style: String,
    /// Whether mixed newlines were detected.
    pub mixed: bool,
    /// Newline policy that was applied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<String>,
    /// Recommended normalization target ("lf" or "crlf"), if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended_normalization: Option<String>,
    /// Newline style detected in the original text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_style: Option<String>,
    /// Newline style detected in the replacement text, if applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replacement_style: Option<String>,
}

/// Unicode security check result from composing `text_security_inspect`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnicodeCheckResult {
    /// Overall verdict from security inspect: "allow", "review", or "block".
    pub verdict: String,
    /// Machine code from security inspect.
    pub machine_code: String,
    /// Number of findings.
    pub finding_count: usize,
    /// Structured findings from text_security_inspect (when available).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<Value>,
}

/// Fingerprint result from composing `text_fingerprint`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FingerprintResult {
    /// SHA-256 fingerprint of the text.
    pub sha256: String,
    /// Detected newline style in the text.
    pub newline_style: String,
}

// ---------------------------------------------------------------------------
// Shared types
// ---------------------------------------------------------------------------

/// Structured representation of a recommended next tool.
///
/// The tool may return `recommended_next_tool` as either a plain string
/// (legacy shape) or an object with `name`, `reason`, and
/// `arguments_hint`. This struct unifies both shapes.
#[derive(Clone, Debug)]
pub struct RecommendedNextTool {
    /// Tool name to call next.
    pub name: String,
    /// Human-readable reason for the recommendation.
    pub reason: Option<String>,
    /// Optional JSON arguments hint for the next call.
    pub arguments_hint: Option<Value>,
}

/// A structured finding from a tool execution.
///
/// Stores severity and disposition as raw strings for serde compatibility.
/// Use `severity_enum()` and `disposition_enum()` for typed access.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Finding {
    pub code: String,
    pub severity: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disposition: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl Finding {
    /// Parse a finding from a JSON value (permissive).
    pub fn from_value(v: &Value) -> Option<Self> {
        Some(Self {
            code: v.get("code")?.as_str()?.to_string(),
            severity: v
                .get("severity")
                .and_then(|s| s.as_str())
                .unwrap_or("info")
                .to_string(),
            message: v.get("message")?.as_str()?.to_string(),
            disposition: v
                .get("disposition")
                .and_then(|d| d.as_str())
                .map(String::from),
            location: v.get("location").cloned(),
            details: v.get("details").cloned(),
        })
    }

    /// Parse a finding from a JSON value (strict — fail-closed).
    ///
    /// Requires `code`, `severity`, and `message` to be present strings.
    /// Does NOT default `severity` to `"info"` when missing.
    pub fn try_from_value_strict(v: &Value, tool: &'static str) -> Result<Self, PreflightError> {
        let code = v
            .get("code")
            .and_then(|s| s.as_str())
            .ok_or_else(|| PreflightError::ContractViolation {
                tool,
                field: "finding.code",
                message: format!(
                    "finding missing required `code` string, got {:?}",
                    v.get("code")
                ),
            })?
            .to_string();

        let severity = v
            .get("severity")
            .and_then(|s| s.as_str())
            .ok_or_else(|| PreflightError::ContractViolation {
                tool,
                field: "finding.severity",
                message: format!(
                    "finding missing required `severity` string, got {:?}",
                    v.get("severity")
                ),
            })?
            .to_string();

        let message = v
            .get("message")
            .and_then(|s| s.as_str())
            .ok_or_else(|| PreflightError::ContractViolation {
                tool,
                field: "finding.message",
                message: format!(
                    "finding missing required `message` string, got {:?}",
                    v.get("message")
                ),
            })?
            .to_string();

        let disposition = v
            .get("disposition")
            .and_then(|d| d.as_str())
            .map(String::from);
        let location = v.get("location").cloned();
        let details = v.get("details").cloned();

        Ok(Self {
            code,
            severity,
            message,
            disposition,
            location,
            details,
        })
    }

    /// Parse a list of findings from a JSON array (permissive — drops malformed).
    pub fn from_array(arr: &[Value]) -> Vec<Self> {
        arr.iter().filter_map(Finding::from_value).collect()
    }

    /// Parse a list of findings from a JSON array (strict — fail-closed).
    ///
    /// Returns `ContractViolation` if any element is malformed instead of
    /// silently dropping it.
    pub fn from_array_strict(
        arr: &[Value],
        tool: &'static str,
    ) -> Result<Vec<Self>, PreflightError> {
        arr.iter()
            .enumerate()
            .map(|(i, v)| {
                Finding::try_from_value_strict(v, tool).map_err(|e| {
                    PreflightError::ContractViolation {
                        tool,
                        field: "findings",
                        message: format!("malformed finding at index {i}: {e}"),
                    }
                })
            })
            .collect()
    }

    /// Typed severity accessor.
    pub fn severity_enum(&self) -> FindingSeverity {
        FindingSeverity::parse(&self.severity)
    }

    /// Typed disposition accessor.
    pub fn disposition_enum(&self) -> Option<FindingDisposition> {
        self.disposition.as_deref().map(FindingDisposition::parse)
    }
}

/// Parse a `recommended_next_tool` value (string or object) into a
/// `RecommendedNextTool`. Returns `ContractViolation` for malformed
/// objects or values that are neither string nor object.
fn parse_recommended_next_tool(
    v: &Value,
    tool: &'static str,
) -> Result<RecommendedNextTool, PreflightError> {
    match v {
        Value::String(name) => Ok(RecommendedNextTool {
            name: name.clone(),
            reason: None,
            arguments_hint: None,
        }),
        Value::Object(map) => {
            let name = map
                .get("name")
                .and_then(|n| n.as_str())
                .ok_or_else(|| PreflightError::ContractViolation {
                    tool,
                    field: "recommended_next_tool.name",
                    message: format!(
                        "recommended_next_tool object missing required `name` string, got {:?}",
                        map.get("name")
                    ),
                })?
                .to_string();
            let reason = map.get("reason").and_then(|r| r.as_str()).map(String::from);
            let arguments_hint = map.get("arguments_hint").cloned();
            Ok(RecommendedNextTool {
                name,
                reason,
                arguments_hint,
            })
        }
        other => Err(PreflightError::ContractViolation {
            tool,
            field: "recommended_next_tool",
            message: format!("expected string or object, got {}", other),
        }),
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

/// Structured policy configuration for command preflight.
///
/// These fields refine or override the built-in policy enum. Deny beats
/// allow when both are set for the same category.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CommandPolicyConfig {
    /// Explicit allow list of program names.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_commands: Option<Vec<String>>,
    /// Explicit deny list of program names (overrides allow).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deny_commands: Option<Vec<String>>,
    /// Per-program allowed subcommands.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_subcommands: Option<std::collections::HashMap<String, Vec<String>>>,
    /// Per-program denied subcommands (overrides allow).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deny_subcommands: Option<std::collections::HashMap<String, Vec<String>>>,
    /// Allow network access (default false — network findings are emitted).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_network: Option<bool>,
    /// Allow filesystem writes (default false).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_filesystem_write: Option<bool>,
    /// Allow process control (default false).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_process_control: Option<bool>,
    /// Allow environment variable mutation (default false).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_env_mutation: Option<bool>,
    /// Maximum command length in characters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_command_length: Option<u64>,
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
    /// Target file path (enables path_scope_check when workspace_root is set).
    pub file_path: Option<String>,
    /// Workspace root directory (enables path scope validation).
    pub workspace_root: Option<String>,
    /// Newline policy for the replacement text.
    pub newline_policy: EditNewlinePolicy,
    /// Unicode security policy for the replacement text.
    pub unicode_policy: EditUnicodePolicy,
    /// Edit metadata for diagnostics.
    pub edit_metadata: Option<EditMetadata>,
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
            file_path: None,
            workspace_root: None,
            newline_policy: EditNewlinePolicy::default(),
            unicode_policy: EditUnicodePolicy::default(),
            edit_metadata: None,
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
    /// Typed verdict for programmatic routing.
    pub verdict: EditVerdict,
    /// Primary machine-readable status code (highest-priority finding).
    pub machine_code: String,
    /// Additional machine codes when multiple findings exist.
    pub secondary_machine_codes: Vec<String>,
    /// Human-readable summary.
    pub summary: String,
    /// Structured findings.
    pub findings: Vec<Finding>,
    /// Recommended next tool to call (structured).
    pub recommended_next_tool: Option<RecommendedNextTool>,
    /// Path scope check result (when file_path + workspace_root provided).
    pub path_scope: Option<PathScopeResult>,
    /// Newline check result (when newline_policy is not Skip).
    pub newline_check: Option<NewlineCheckResult>,
    /// Unicode security check result (when unicode_policy is not Skip).
    pub unicode_check: Option<UnicodeCheckResult>,
    /// Fingerprint result (when expected_fingerprint provided).
    pub fingerprint: Option<FingerprintResult>,
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
        if let Some(ref fp) = input.file_path {
            args["file_path"] = Value::String(fp.clone());
        }
        if let Some(ref wr) = input.workspace_root {
            args["workspace_root"] = Value::String(wr.clone());
        }
        if input.newline_policy != EditNewlinePolicy::default() {
            args["newline_policy"] = Value::String(input.newline_policy.as_str().to_string());
        }
        if input.unicode_policy != EditUnicodePolicy::default() {
            args["unicode_policy"] = Value::String(input.unicode_policy.as_str().to_string());
        }
        if let Some(ref meta) = input.edit_metadata {
            if let Ok(v) = serde_json::to_value(meta) {
                args["edit_metadata"] = v;
            }
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
        let verdict_str = require_str(result, Self::TOOL, "verdict")?;
        let verdict = EditVerdict::parse(verdict_str);

        let findings = response
            .findings
            .as_ref()
            .map(|f| Finding::from_array_strict(f, Self::TOOL))
            .transpose()?
            .unwrap_or_default();

        let recommended_next_tool = response
            .recommended_next_tool
            .as_ref()
            .map(|v| parse_recommended_next_tool(v, Self::TOOL))
            .transpose()?;

        // Parse optional sub-tool result structs
        let path_scope = result
            .get("path_scope")
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        let newline_check = result
            .get("newline_check")
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        let unicode_check = result
            .get("unicode_check")
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        let fingerprint = result
            .get("fingerprint")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        let raw = response.result.unwrap_or(Value::Null);

        // Parse secondary_machine_codes from result
        let secondary_machine_codes = raw
            .get("secondary_machine_codes")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        Ok(EditPreflightOutput {
            ok_to_apply,
            mode,
            verdict,
            machine_code,
            secondary_machine_codes,
            summary,
            findings,
            recommended_next_tool,
            path_scope,
            newline_check,
            unicode_check,
            fingerprint,
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
    /// Structured policy configuration overrides.
    pub policy_config: Option<CommandPolicyConfig>,
    /// Working directory context (informational).
    pub working_directory: Option<String>,
}

impl Default for CommandPreflightInput {
    fn default() -> Self {
        Self {
            command: String::new(),
            platform: "posix".to_string(),
            policy: CommandPolicy::default(),
            policy_config: None,
            working_directory: None,
        }
    }
}

/// Output from command preflight analysis.
#[derive(Clone, Debug)]
pub struct CommandPreflightOutput {
    /// Typed verdict for programmatic routing.
    pub verdict: CommandVerdict,
    /// Machine-readable status code.
    pub machine_code: String,
    /// Human-readable summary.
    pub summary: String,
    /// Structured findings.
    pub findings: Vec<Finding>,
    /// Extracted program name.
    pub program: Option<String>,
    /// Extracted subcommand.
    pub subcommand: Option<String>,
    /// Detected risky shell features.
    pub features: Vec<String>,
    /// Policy rules that matched.
    pub matched_rules: Vec<String>,
    /// Parsed argv if available.
    pub argv: Option<Vec<String>>,
    /// Recommended next tool to call (structured).
    pub recommended_next_tool: Option<RecommendedNextTool>,
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
        if let Some(ref config) = input.policy_config {
            if let Ok(v) = serde_json::to_value(config) {
                args["policy_config"] = v;
            }
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
        let verdict_str = require_str(result, Self::TOOL, "verdict")?;
        let verdict = CommandVerdict::parse(verdict_str);
        let summary = require_str(result, Self::TOOL, "summary")?.to_string();

        let findings = response
            .findings
            .as_ref()
            .map(|f| Finding::from_array_strict(f, Self::TOOL))
            .transpose()?
            .unwrap_or_default();

        let argv = result.get("argv").and_then(|v| v.as_array()).map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        });

        let program = result
            .get("program")
            .and_then(|v| v.as_str())
            .map(String::from);
        let subcommand = result
            .get("subcommand")
            .and_then(|v| v.as_str())
            .map(String::from);
        let features = result
            .get("features")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let matched_rules = result
            .get("matched_rules")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let recommended_next_tool = response
            .recommended_next_tool
            .as_ref()
            .map(|v| parse_recommended_next_tool(v, Self::TOOL))
            .transpose()?;

        let raw = response.result.unwrap_or(Value::Null);

        Ok(CommandPreflightOutput {
            verdict,
            machine_code,
            summary,
            findings,
            program,
            subcommand,
            features,
            matched_rules,
            argv,
            recommended_next_tool,
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
    /// Typed verdict for programmatic routing.
    pub verdict: ConfigVerdict,
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
        let verdict_str = require_str(result, Self::TOOL, "verdict")?;
        let verdict = ConfigVerdict::parse(verdict_str);
        let summary = require_str(result, Self::TOOL, "summary")?.to_string();

        let detected_format = result
            .get("format")
            .and_then(|v| v.as_str())
            .map(String::from);

        let findings = response
            .findings
            .as_ref()
            .map(|f| Finding::from_array_strict(f, Self::TOOL))
            .transpose()?
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
        assert_eq!(output.verdict, ConfigVerdict::Valid);
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
        assert_eq!(output.verdict, ConfigVerdict::Invalid);
    }

    #[test]
    fn command_preflight_safe() {
        let input = CommandPreflightInput {
            command: "ls -la".to_string(),
            ..Default::default()
        };
        let output = CommandPreflight::run(&input).unwrap();
        assert_eq!(output.verdict, CommandVerdict::Allow);
        assert!(!output.machine_code.is_empty());
    }

    #[test]
    fn finding_from_value_with_disposition() {
        let v = serde_json::json!({
            "code": "TEST_CODE",
            "severity": "info",
            "message": "test message",
            "disposition": "caution"
        });
        let f = Finding::from_value(&v).unwrap();
        assert_eq!(f.code, "TEST_CODE");
        assert_eq!(f.severity, "info");
        assert_eq!(f.severity_enum(), FindingSeverity::Info);
        assert_eq!(f.message, "test message");
        assert_eq!(f.disposition.as_deref(), Some("caution"));
        assert_eq!(f.disposition_enum(), Some(FindingDisposition::Caution));
    }

    #[test]
    fn finding_from_value_without_disposition() {
        let v = serde_json::json!({
            "code": "TEST_CODE",
            "severity": "high",
            "message": "test message"
        });
        let f = Finding::from_value(&v).unwrap();
        assert_eq!(f.severity_enum(), FindingSeverity::High);
        assert_eq!(f.disposition, None);
        assert_eq!(f.disposition_enum(), None);
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
                "verdict": "allow",
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
                "verdict": "allow",
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
                "verdict": "allow",
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
                "verdict": "allow",
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
    fn edit_preflight_missing_verdict_fails_closed() {
        let response = ToolResponse::success(
            serde_json::json!({
                "ok_to_apply": true,
                "mode": "literal",
                "summary": "test",
            }),
            Some("edit_preflight"),
        )
        .with_machine_code("EDIT_OK");
        let err = EditPreflight::parse_response(response).unwrap_err();
        assert!(
            matches!(err, PreflightError::ContractViolation { field, .. } if field == "verdict")
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
                "verdict": "allow",
            }),
            Some("edit_preflight"),
        )
        .with_machine_code("EDIT_OK");
        let output = EditPreflight::parse_response(response).unwrap();
        assert!(!output.machine_code.is_empty());
        assert!(!output.mode.is_empty());
        assert!(!output.summary.is_empty());
        assert_eq!(output.verdict, EditVerdict::Allow);
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
        assert!(!output.summary.is_empty());
        assert_eq!(output.verdict, CommandVerdict::Allow);
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
        assert!(!output.summary.is_empty());
        assert_eq!(output.verdict, ConfigVerdict::Valid);
    }

    // -- Typed enum tests --

    #[test]
    fn edit_verdict_roundtrip() {
        assert_eq!(EditVerdict::Allow.as_str(), "allow");
        assert_eq!(EditVerdict::Review.as_str(), "review");
        assert_eq!(EditVerdict::Block.as_str(), "block");
        assert_eq!(EditVerdict::SafeToApply.as_str(), "safe_to_apply");
        assert_eq!(EditVerdict::SafeWithWarnings.as_str(), "safe_with_warnings");
        let other = EditVerdict::Other("new_value".to_string());
        assert_eq!(other.as_str(), "new_value");
        assert_eq!(EditVerdict::parse("allow"), EditVerdict::Allow);
        assert_eq!(EditVerdict::parse("review"), EditVerdict::Review);
        assert_eq!(EditVerdict::parse("block"), EditVerdict::Block);
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

    // -- Tool-specific preflight route tests --

    #[test]
    fn edit_preflight_literal_no_match_blocks() {
        let input = EditPreflightInput {
            original: "hello world".to_string(),
            mode: ReplacementMode::Literal,
            old: Some("nonexistent".to_string()),
            new: Some("replacement".to_string()),
            ..Default::default()
        };
        let output = EditPreflight::run(&input).unwrap();
        assert!(!output.ok_to_apply);
        assert_eq!(output.verdict, EditVerdict::Block);
        assert!(output.machine_code.contains("AMBIGUOUS"));
    }

    #[test]
    fn edit_preflight_literal_match_allows() {
        let input = EditPreflightInput {
            original: "hello world".to_string(),
            mode: ReplacementMode::Literal,
            old: Some("hello".to_string()),
            new: Some("goodbye".to_string()),
            ..Default::default()
        };
        let output = EditPreflight::run(&input).unwrap();
        assert!(output.ok_to_apply);
        assert_eq!(output.verdict, EditVerdict::Allow);
        assert_eq!(output.machine_code, "EDIT_OK");
    }

    #[test]
    fn command_preflight_pipe_reviews() {
        let input = CommandPreflightInput {
            command: "cat file | grep pattern".to_string(),
            ..Default::default()
        };
        let output = CommandPreflight::run(&input).unwrap();
        assert_eq!(output.verdict, CommandVerdict::Review);
        assert!(!output.findings.is_empty());
    }

    #[test]
    fn config_preflight_toml_valid() {
        let input = ConfigPreflightInput {
            text: "key = \"value\"\n".to_string(),
            format: ConfigFormat::Toml,
            ..Default::default()
        };
        let output = ConfigPreflight::run(&input).unwrap();
        assert!(output.valid);
        assert_eq!(output.verdict, ConfigVerdict::Valid);
    }

    // -- Fixture tests for findings shape --

    #[test]
    fn findings_have_required_fields() {
        let v = serde_json::json!({
            "code": "TEST_CODE",
            "severity": "high",
            "message": "test",
            "disposition": "blocking",
            "location": {"line": 1, "column": 5},
            "details": {"key": "value"}
        });
        let f = Finding::from_value(&v).unwrap();
        assert_eq!(f.code, "TEST_CODE");
        assert_eq!(f.severity, "high");
        assert_eq!(f.severity_enum(), FindingSeverity::High);
        assert_eq!(f.message, "test");
        assert_eq!(f.disposition_enum(), Some(FindingDisposition::Blocking));
        assert!(f.location.is_some());
        assert!(f.details.is_some());
    }

    #[test]
    fn findings_optional_fields_absent() {
        let v = serde_json::json!({
            "code": "MINIMAL",
            "severity": "info",
            "message": "minimal finding"
        });
        let f = Finding::from_value(&v).unwrap();
        assert_eq!(f.disposition, None);
        assert_eq!(f.location, None);
        assert_eq!(f.details, None);
    }

    #[test]
    fn findings_from_array_filters_invalid() {
        let arr = vec![
            serde_json::json!({"code": "A", "severity": "info", "message": "ok"}),
            serde_json::json!({"invalid": true}),
            serde_json::json!({"code": "B", "severity": "high", "message": "also ok"}),
        ];
        let findings = Finding::from_array(&arr);
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].code, "A");
        assert_eq!(findings[1].code, "B");
    }

    #[test]
    fn finding_severity_all_variants() {
        for (s, expected) in [
            ("info", FindingSeverity::Info),
            ("low", FindingSeverity::Low),
            ("medium", FindingSeverity::Medium),
            ("high", FindingSeverity::High),
            ("critical", FindingSeverity::Critical),
        ] {
            assert_eq!(FindingSeverity::parse(s), expected);
        }
    }

    #[test]
    fn finding_disposition_all_variants() {
        for (s, expected) in [
            ("informational", FindingDisposition::Informational),
            ("caution", FindingDisposition::Caution),
            ("blocking", FindingDisposition::Blocking),
        ] {
            assert_eq!(FindingDisposition::parse(s), expected);
        }
    }

    // -- Backward-compatibility tests --

    #[test]
    fn edit_verdict_accepts_legacy_safe_to_apply() {
        let response = ToolResponse::success(
            serde_json::json!({
                "ok_to_apply": true,
                "mode": "literal",
                "summary": "ok",
                "verdict": "safe_to_apply",
            }),
            Some("edit_preflight"),
        )
        .with_machine_code("EDIT_OK");
        let output = EditPreflight::parse_response(response).unwrap();
        assert_eq!(output.verdict, EditVerdict::SafeToApply);
        assert_eq!(output.verdict.as_str(), "safe_to_apply");
    }

    #[test]
    fn edit_verdict_accepts_legacy_safe_with_warnings() {
        let response = ToolResponse::success(
            serde_json::json!({
                "ok_to_apply": true,
                "mode": "literal",
                "summary": "ok",
                "verdict": "safe_with_warnings",
            }),
            Some("edit_preflight"),
        )
        .with_machine_code("EDIT_OK");
        let output = EditPreflight::parse_response(response).unwrap();
        assert_eq!(output.verdict, EditVerdict::SafeWithWarnings);
    }

    #[test]
    fn finding_severity_unknown_maps_to_other() {
        let v = serde_json::json!({
            "code": "X",
            "severity": "nonexistent_level",
            "message": "test"
        });
        let f = Finding::from_value(&v).unwrap();
        assert_eq!(
            f.severity_enum(),
            FindingSeverity::Other("nonexistent_level".to_string())
        );
    }

    #[test]
    fn finding_disposition_none_when_absent() {
        let v = serde_json::json!({
            "code": "X",
            "severity": "info",
            "message": "test"
        });
        let f = Finding::from_value(&v).unwrap();
        assert_eq!(f.disposition, None);
    }

    #[test]
    fn edit_preflight_output_has_raw_for_forward_compat() {
        let input = EditPreflightInput {
            original: "hello".to_string(),
            mode: ReplacementMode::Literal,
            old: Some("hello".to_string()),
            new: Some("world".to_string()),
            ..Default::default()
        };
        let output = EditPreflight::run(&input).unwrap();
        assert!(output.raw.is_object());
        assert!(output.raw.get("ok_to_apply").is_some());
    }

    #[test]
    fn command_preflight_output_has_raw_for_forward_compat() {
        let input = CommandPreflightInput {
            command: "ls".to_string(),
            ..Default::default()
        };
        let output = CommandPreflight::run(&input).unwrap();
        assert!(output.raw.is_object());
        assert!(output.raw.get("verdict").is_some());
    }

    // -- Phase 6: Enhanced edit preflight tests --

    #[test]
    fn edit_preflight_path_scope_inside_root() {
        let input = EditPreflightInput {
            original: "hello world".to_string(),
            mode: ReplacementMode::Literal,
            old: Some("hello".to_string()),
            new: Some("goodbye".to_string()),
            file_path: Some("src/main.rs".to_string()),
            workspace_root: Some(".".to_string()),
            ..Default::default()
        };
        let output = EditPreflight::run(&input).unwrap();
        assert!(output.ok_to_apply);
        assert!(output.path_scope.is_some());
        let ps = output.path_scope.unwrap();
        assert!(ps.inside_root);
        assert!(!ps.escapes_via_dotdot);
    }

    #[test]
    fn edit_preflight_path_scope_escape_blocked() {
        // Use a workspace root that resolves to the project directory
        // and a file path that resolves outside it via traversal
        let workspace = std::env::current_dir()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let input = EditPreflightInput {
            original: "hello world".to_string(),
            mode: ReplacementMode::Literal,
            old: Some("hello".to_string()),
            new: Some("goodbye".to_string()),
            file_path: Some(format!("{}/../outside_project/file.txt", workspace)),
            workspace_root: Some(workspace),
            ..Default::default()
        };
        let output = EditPreflight::run(&input).unwrap();
        assert!(output.path_scope.is_some());
        let ps = output.path_scope.unwrap();
        // The path goes outside root via traversal
        assert!(ps.escapes_via_dotdot);
        // Whether inside_root is true or false depends on the OS resolution,
        // but escapes_via_dotdot should always be true for traversal paths
    }

    #[test]
    fn edit_preflight_newline_check_detects_mixed() {
        let mixed = "line1\nline2\r\nline3\n";
        let input = EditPreflightInput {
            original: mixed.to_string(),
            mode: ReplacementMode::Literal,
            old: Some("line1".to_string()),
            new: Some("LINE1".to_string()),
            newline_policy: EditNewlinePolicy::Check,
            ..Default::default()
        };
        let output = EditPreflight::run(&input).unwrap();
        assert!(output.newline_check.is_some());
        let nc = output.newline_check.unwrap();
        assert!(nc.mixed);
    }

    #[test]
    fn edit_preflight_newline_skip_by_default() {
        let input = EditPreflightInput {
            original: "hello".to_string(),
            mode: ReplacementMode::Literal,
            old: Some("hello".to_string()),
            new: Some("world".to_string()),
            ..Default::default()
        };
        let output = EditPreflight::run(&input).unwrap();
        assert!(output.newline_check.is_none());
    }

    #[test]
    fn edit_preflight_unicode_check_default_policy() {
        let input = EditPreflightInput {
            original: "hello world".to_string(),
            mode: ReplacementMode::Literal,
            old: Some("hello".to_string()),
            new: Some("world".to_string()),
            unicode_policy: EditUnicodePolicy::Default,
            ..Default::default()
        };
        let output = EditPreflight::run(&input).unwrap();
        assert!(output.unicode_check.is_some());
        let uc = output.unicode_check.unwrap();
        assert_eq!(uc.verdict, "allow");
    }

    #[test]
    fn edit_preflight_unicode_skip_by_default() {
        let input = EditPreflightInput {
            original: "hello".to_string(),
            mode: ReplacementMode::Literal,
            old: Some("hello".to_string()),
            new: Some("world".to_string()),
            ..Default::default()
        };
        let output = EditPreflight::run(&input).unwrap();
        assert!(output.unicode_check.is_none());
    }

    #[test]
    fn edit_preflight_enums_parse_roundtrip() {
        // EditNewlinePolicy
        assert_eq!(EditNewlinePolicy::parse("skip"), EditNewlinePolicy::Skip);
        assert_eq!(EditNewlinePolicy::parse("check"), EditNewlinePolicy::Check);
        assert_eq!(
            EditNewlinePolicy::parse("normalize_lf"),
            EditNewlinePolicy::NormalizeLf
        );
        assert_eq!(
            EditNewlinePolicy::parse("normalize_crlf"),
            EditNewlinePolicy::NormalizeCrlf
        );
        assert_eq!(EditNewlinePolicy::Skip.as_str(), "skip");
        assert_eq!(EditNewlinePolicy::Check.as_str(), "check");
        assert_eq!(EditNewlinePolicy::NormalizeLf.as_str(), "normalize_lf");
        assert_eq!(EditNewlinePolicy::NormalizeCrlf.as_str(), "normalize_crlf");

        // EditUnicodePolicy
        assert_eq!(EditUnicodePolicy::parse("skip"), EditUnicodePolicy::Skip);
        assert_eq!(
            EditUnicodePolicy::parse("default"),
            EditUnicodePolicy::Default
        );
        assert_eq!(
            EditUnicodePolicy::parse("source_code"),
            EditUnicodePolicy::SourceCode
        );
        assert_eq!(
            EditUnicodePolicy::parse("identifier"),
            EditUnicodePolicy::Identifier
        );
        assert_eq!(EditUnicodePolicy::Skip.as_str(), "skip");
        assert_eq!(EditUnicodePolicy::Default.as_str(), "default");
        assert_eq!(EditUnicodePolicy::SourceCode.as_str(), "source_code");
        assert_eq!(EditUnicodePolicy::Identifier.as_str(), "identifier");
    }

    #[test]
    fn edit_preflight_parse_response_with_path_scope() {
        let response = ToolResponse::success(
            serde_json::json!({
                "ok_to_apply": true,
                "mode": "literal",
                "summary": "ok",
                "verdict": "allow",
                "path_scope": {
                    "inside_root": true,
                    "escapes_via_dotdot": false,
                    "relative_path": "src/main.rs"
                }
            }),
            Some("edit_preflight"),
        )
        .with_machine_code("EDIT_OK");
        let output = EditPreflight::parse_response(response).unwrap();
        assert!(output.path_scope.is_some());
        let ps = output.path_scope.unwrap();
        assert!(ps.inside_root);
        assert!(!ps.escapes_via_dotdot);
        assert_eq!(ps.relative_path, "src/main.rs");
    }

    #[test]
    fn edit_preflight_parse_response_with_unicode_check() {
        let response = ToolResponse::success(
            serde_json::json!({
                "ok_to_apply": true,
                "mode": "literal",
                "summary": "ok",
                "verdict": "allow",
                "unicode_check": {
                    "verdict": "allow",
                    "machine_code": "TEXT_SECURITY_OK",
                    "finding_count": 0
                }
            }),
            Some("edit_preflight"),
        )
        .with_machine_code("EDIT_OK");
        let output = EditPreflight::parse_response(response).unwrap();
        assert!(output.unicode_check.is_some());
        let uc = output.unicode_check.unwrap();
        assert_eq!(uc.verdict, "allow");
        assert_eq!(uc.machine_code, "TEXT_SECURITY_OK");
        assert_eq!(uc.finding_count, 0);
    }

    #[test]
    fn edit_preflight_parse_response_with_newline_check() {
        let response = ToolResponse::success(
            serde_json::json!({
                "ok_to_apply": true,
                "mode": "literal",
                "summary": "ok",
                "verdict": "review",
                "newline_check": {
                    "style": "mixed",
                    "mixed": true
                }
            }),
            Some("edit_preflight"),
        )
        .with_machine_code("NEWLINE_INCONSISTENCY");
        let output = EditPreflight::parse_response(response).unwrap();
        assert!(output.newline_check.is_some());
        let nc = output.newline_check.unwrap();
        assert_eq!(nc.style, "mixed");
        assert!(nc.mixed);
    }

    #[test]
    fn edit_preflight_parse_response_without_new_fields_is_none() {
        let response = ToolResponse::success(
            serde_json::json!({
                "ok_to_apply": true,
                "mode": "literal",
                "summary": "ok",
                "verdict": "allow"
            }),
            Some("edit_preflight"),
        )
        .with_machine_code("EDIT_OK");
        let output = EditPreflight::parse_response(response).unwrap();
        assert!(output.path_scope.is_none());
        assert!(output.newline_check.is_none());
        assert!(output.unicode_check.is_none());
        assert!(output.fingerprint.is_none());
    }

    // -- Strict finding parsing tests --

    #[test]
    fn structured_recommended_next_tool_parses_correctly() {
        let response = ToolResponse::success(
            serde_json::json!({
                "ok_to_apply": true,
                "mode": "literal",
                "summary": "ok",
                "verdict": "review"
            }),
            Some("edit_preflight"),
        )
        .with_machine_code("EDIT_OK")
        .with_recommended_next_tool(serde_json::json!({
            "name": "command_preflight",
            "reason": "verify shell command",
            "arguments_hint": {"command": "cargo test"}
        }));
        let output = EditPreflight::parse_response(response).unwrap();
        let next = output.recommended_next_tool.unwrap();
        assert_eq!(next.name, "command_preflight");
        assert_eq!(next.reason.as_deref(), Some("verify shell command"));
        assert!(next.arguments_hint.is_some());
    }

    #[test]
    fn legacy_string_recommended_next_tool_parses_to_structured() {
        let response = ToolResponse::success(
            serde_json::json!({
                "ok_to_apply": true,
                "mode": "literal",
                "summary": "ok",
                "verdict": "allow"
            }),
            Some("edit_preflight"),
        )
        .with_machine_code("EDIT_OK")
        .with_recommended_next_tool(serde_json::json!("command_preflight"));
        let output = EditPreflight::parse_response(response).unwrap();
        let next = output.recommended_next_tool.unwrap();
        assert_eq!(next.name, "command_preflight");
        assert!(next.reason.is_none());
        assert!(next.arguments_hint.is_none());
    }

    #[test]
    fn malformed_recommended_next_tool_object_fails_closed() {
        let response = ToolResponse::success(
            serde_json::json!({
                "ok_to_apply": true,
                "mode": "literal",
                "summary": "ok",
                "verdict": "allow"
            }),
            Some("edit_preflight"),
        )
        .with_machine_code("EDIT_OK")
        .with_recommended_next_tool(serde_json::json!({
            "reason": "missing name field"
        }));
        let err = EditPreflight::parse_response(response).unwrap_err();
        assert!(
            matches!(err, PreflightError::ContractViolation { field, .. } if field == "recommended_next_tool.name")
        );
    }

    #[test]
    fn recommended_next_tool_number_fails_closed() {
        let response = ToolResponse::success(
            serde_json::json!({
                "ok_to_apply": true,
                "mode": "literal",
                "summary": "ok",
                "verdict": "allow"
            }),
            Some("edit_preflight"),
        )
        .with_machine_code("EDIT_OK")
        .with_recommended_next_tool(serde_json::json!(42));
        let err = EditPreflight::parse_response(response).unwrap_err();
        assert!(
            matches!(err, PreflightError::ContractViolation { field, .. } if field == "recommended_next_tool")
        );
    }

    #[test]
    fn missing_finding_code_fails_closed() {
        let response = ToolResponse::success(
            serde_json::json!({
                "ok_to_apply": true,
                "mode": "literal",
                "summary": "ok",
                "verdict": "allow"
            }),
            Some("edit_preflight"),
        )
        .with_machine_code("EDIT_OK")
        .with_findings(vec![serde_json::json!({
            "severity": "info",
            "message": "no code field"
        })]);
        let err = EditPreflight::parse_response(response).unwrap_err();
        assert!(
            matches!(err, PreflightError::ContractViolation { field, .. } if field == "findings")
        );
    }

    #[test]
    fn missing_finding_severity_fails_closed() {
        let response = ToolResponse::success(
            serde_json::json!({
                "ok_to_apply": true,
                "mode": "literal",
                "summary": "ok",
                "verdict": "allow"
            }),
            Some("edit_preflight"),
        )
        .with_machine_code("EDIT_OK")
        .with_findings(vec![serde_json::json!({
            "code": "TEST",
            "message": "no severity field"
        })]);
        let err = EditPreflight::parse_response(response).unwrap_err();
        assert!(
            matches!(err, PreflightError::ContractViolation { field, .. } if field == "findings")
        );
    }

    #[test]
    fn missing_finding_message_fails_closed() {
        let response = ToolResponse::success(
            serde_json::json!({
                "ok_to_apply": true,
                "mode": "literal",
                "summary": "ok",
                "verdict": "allow"
            }),
            Some("edit_preflight"),
        )
        .with_machine_code("EDIT_OK")
        .with_findings(vec![serde_json::json!({
            "code": "TEST",
            "severity": "info"
        })]);
        let err = EditPreflight::parse_response(response).unwrap_err();
        assert!(
            matches!(err, PreflightError::ContractViolation { field, .. } if field == "findings")
        );
    }

    #[test]
    fn strict_findings_unknown_severity_maps_to_other() {
        // Strict parsing requires `code`/`severity`/`message` as strings, but
        // unknown severity strings should still parse as `Other` (not error).
        let response = ToolResponse::success(
            serde_json::json!({
                "ok_to_apply": true,
                "mode": "literal",
                "summary": "ok",
                "verdict": "allow"
            }),
            Some("edit_preflight"),
        )
        .with_machine_code("EDIT_OK")
        .with_findings(vec![serde_json::json!({
            "code": "MY_CODE",
            "severity": "nonexistent_level_xyz",
            "message": "test message"
        })]);
        let output = EditPreflight::parse_response(response).unwrap();
        assert_eq!(output.findings.len(), 1);
        assert_eq!(output.findings[0].code, "MY_CODE");
        assert_eq!(
            output.findings[0].severity_enum(),
            FindingSeverity::Other("nonexistent_level_xyz".to_string())
        );
    }

    #[test]
    fn strict_findings_non_string_severity_fails_closed() {
        // Number severity (instead of string) MUST fail closed — strict
        // parsing requires string-shaped severity.
        let v = serde_json::json!({
            "code": "X",
            "severity": 42,
            "message": "test message"
        });
        let err = Finding::try_from_value_strict(&v, "edit_preflight").unwrap_err();
        assert!(
            matches!(err, PreflightError::ContractViolation { field, .. } if field == "finding.severity")
        );
    }

    #[test]
    fn command_preflight_strict_findings_fails_on_malformed() {
        let response = ToolResponse::success(
            serde_json::json!({
                "verdict": "allow",
                "summary": "safe"
            }),
            Some("command_preflight"),
        )
        .with_machine_code("COMMAND_OK")
        .with_findings(vec![serde_json::json!({
            "invalid": true
        })]);
        let err = CommandPreflight::parse_response(response).unwrap_err();
        assert!(
            matches!(err, PreflightError::ContractViolation { field, .. } if field == "findings")
        );
    }

    #[test]
    fn config_preflight_strict_findings_fails_on_malformed() {
        let response = ToolResponse::success(
            serde_json::json!({
                "valid": true,
                "verdict": "valid",
                "summary": "ok"
            }),
            Some("config_preflight"),
        )
        .with_machine_code("CONFIG_OK")
        .with_findings(vec![serde_json::json!({
            "code": "X",
            "severity": "info"
            // missing message
        })]);
        let err = ConfigPreflight::parse_response(response).unwrap_err();
        assert!(
            matches!(err, PreflightError::ContractViolation { field, .. } if field == "findings")
        );
    }

    #[test]
    fn strict_findings_pass_for_well_formed() {
        let response = ToolResponse::success(
            serde_json::json!({
                "ok_to_apply": true,
                "mode": "literal",
                "summary": "ok",
                "verdict": "allow"
            }),
            Some("edit_preflight"),
        )
        .with_machine_code("EDIT_OK")
        .with_findings(vec![
            serde_json::json!({"code": "A", "severity": "info", "message": "ok"}),
            serde_json::json!({"code": "B", "severity": "high", "message": "warn", "disposition": "blocking"}),
        ]);
        let output = EditPreflight::parse_response(response).unwrap();
        assert_eq!(output.findings.len(), 2);
        assert_eq!(output.findings[0].code, "A");
        assert_eq!(output.findings[1].code, "B");
        assert_eq!(output.findings[1].disposition.as_deref(), Some("blocking"));
    }

    #[test]
    fn edit_preflight_parse_response_path_scope_with_normalized_target() {
        let response = ToolResponse::success(
            serde_json::json!({
                "ok_to_apply": true,
                "mode": "literal",
                "summary": "ok",
                "verdict": "allow",
                "path_scope": {
                    "inside_root": true,
                    "escapes_via_dotdot": false,
                    "relative_path": "src/main.rs",
                    "normalized_target": "/workspace/src/main.rs",
                    "reason": null
                }
            }),
            Some("edit_preflight"),
        )
        .with_machine_code("EDIT_OK");
        let output = EditPreflight::parse_response(response).unwrap();
        let ps = output.path_scope.unwrap();
        assert!(ps.inside_root);
        assert_eq!(
            ps.normalized_target.as_deref(),
            Some("/workspace/src/main.rs")
        );
        assert!(ps.reason.is_none());
    }

    #[test]
    fn edit_preflight_parse_response_path_scope_without_new_fields() {
        let response = ToolResponse::success(
            serde_json::json!({
                "ok_to_apply": true,
                "mode": "literal",
                "summary": "ok",
                "verdict": "allow",
                "path_scope": {
                    "inside_root": true,
                    "escapes_via_dotdot": false,
                    "relative_path": "src/main.rs"
                }
            }),
            Some("edit_preflight"),
        )
        .with_machine_code("EDIT_OK");
        let output = EditPreflight::parse_response(response).unwrap();
        let ps = output.path_scope.unwrap();
        assert!(ps.inside_root);
        assert!(ps.normalized_target.is_none());
        assert!(ps.reason.is_none());
    }

    #[test]
    fn edit_preflight_parse_response_newline_check_with_styles() {
        let response = ToolResponse::success(
            serde_json::json!({
                "ok_to_apply": true,
                "mode": "literal",
                "summary": "ok",
                "verdict": "review",
                "newline_check": {
                    "style": "mixed",
                    "mixed": true,
                    "policy": "check",
                    "original_style": "LF",
                    "replacement_style": "CRLF"
                }
            }),
            Some("edit_preflight"),
        )
        .with_machine_code("NEWLINE_INCONSISTENCY");
        let output = EditPreflight::parse_response(response).unwrap();
        let nc = output.newline_check.unwrap();
        assert_eq!(nc.style, "mixed");
        assert!(nc.mixed);
        assert_eq!(nc.original_style.as_deref(), Some("LF"));
        assert_eq!(nc.replacement_style.as_deref(), Some("CRLF"));
    }

    #[test]
    fn edit_preflight_parse_response_unicode_check_with_findings() {
        let response = ToolResponse::success(
            serde_json::json!({
                "ok_to_apply": true,
                "mode": "literal",
                "summary": "ok",
                "verdict": "review",
                "unicode_check": {
                    "verdict": "review",
                    "machine_code": "UNICODE_RISK",
                    "finding_count": 1,
                    "findings": [
                        {"code": "ZERO_WIDTH_SPACE", "severity": "medium", "message": "Zero-width space found"}
                    ]
                }
            }),
            Some("edit_preflight"),
        )
        .with_machine_code("UNICODE_RISK");
        let output = EditPreflight::parse_response(response).unwrap();
        let uc = output.unicode_check.unwrap();
        assert_eq!(uc.verdict, "review");
        assert_eq!(uc.finding_count, 1);
        assert_eq!(uc.findings.len(), 1);
    }

    #[test]
    fn edit_preflight_parse_response_unicode_check_without_findings() {
        let response = ToolResponse::success(
            serde_json::json!({
                "ok_to_apply": true,
                "mode": "literal",
                "summary": "ok",
                "verdict": "allow",
                "unicode_check": {
                    "verdict": "allow",
                    "machine_code": "TEXT_SECURITY_OK",
                    "finding_count": 0
                }
            }),
            Some("edit_preflight"),
        )
        .with_machine_code("EDIT_OK");
        let output = EditPreflight::parse_response(response).unwrap();
        let uc = output.unicode_check.unwrap();
        assert_eq!(uc.findings.len(), 0);
    }
}
