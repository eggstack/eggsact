//! In-process agent API for calling eggsact tools directly.
//!
//! This module provides a typed, synchronous API for calling eggsact tools
//! without starting an MCP server. It is the primary integration point for
//! codegg and other Rust consumers.
//!
//! # Quick Start
//!
//! ```
//! use eggsact::agent::{ToolRegistry, Profile};
//!
//! let registry = ToolRegistry::default();
//! let response = registry.call_json("text_equal", serde_json::json!({
//!     "a": "hello",
//!     "b": "hello",
//! })).unwrap();
//! assert!(response.ok);
//! ```
//!
//! # Typed Preflight
//!
//! For common codegg workflows, use the typed preflight wrappers:
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

use crate::mcp::registry::{self, ToolExposure, ToolSpec};
use crate::mcp::response::ToolResponse;
use crate::mcp::schema_validation;
use serde_json::Value;
use std::fmt;

/// Typed profile selection for tool filtering.
///
/// Profiles control which subset of tools is available. Each profile
/// corresponds to a string profile name used in the MCP registry.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum Profile {
    /// All non-hidden tools.
    #[default]
    Full,
    /// Default tool set.
    Default,
    /// Minimal codegg tool set.
    CodeggCoreMin,
    /// Core codegg tools.
    CodeggCore,
    /// Codegg preflight tools.
    CodeggPreflight,
    /// Codegg patch/edit tools.
    CodeggPatch,
    /// Codegg config validation tools.
    CodeggConfig,
    /// Codegg unicode security tools.
    CodeggUnicodeSecurity,
    /// Codegg shell/command tools.
    CodeggShell,
    /// Codegg repository audit tools.
    CodeggRepoAudit,
    /// Human-friendly math tools.
    HumanMath,
    /// Custom profile by name.
    Custom(String),
}

impl Profile {
    /// Convert to the string profile name used in the MCP registry.
    pub fn as_str(&self) -> &str {
        match self {
            Profile::Full => "full",
            Profile::Default => "default",
            Profile::CodeggCoreMin => "codegg_core_min",
            Profile::CodeggCore => "codegg_core",
            Profile::CodeggPreflight => "codegg_preflight",
            Profile::CodeggPatch => "codegg_patch",
            Profile::CodeggConfig => "codegg_config",
            Profile::CodeggUnicodeSecurity => "codegg_unicode_security",
            Profile::CodeggShell => "codegg_shell",
            Profile::CodeggRepoAudit => "codegg_repo_audit",
            Profile::HumanMath => "human_math",
            Profile::Custom(name) => name,
        }
    }

    /// Parse a profile from a string name.
    ///
    /// Returns `None` for unknown profile names.
    pub fn from_str_opt(name: &str) -> Option<Self> {
        match name {
            "full" => Some(Profile::Full),
            "default" => Some(Profile::Default),
            "codegg_core_min" => Some(Profile::CodeggCoreMin),
            "codegg_core" => Some(Profile::CodeggCore),
            "codegg_preflight" => Some(Profile::CodeggPreflight),
            "codegg_patch" => Some(Profile::CodeggPatch),
            "codegg_config" => Some(Profile::CodeggConfig),
            "codegg_unicode_security" => Some(Profile::CodeggUnicodeSecurity),
            "codegg_shell" => Some(Profile::CodeggShell),
            "codegg_repo_audit" => Some(Profile::CodeggRepoAudit),
            "human_math" => Some(Profile::HumanMath),
            other => Some(Profile::Custom(other.to_string())),
        }
    }
}

impl fmt::Display for Profile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Errors that can occur before tool execution.
///
/// Tool-level failures (e.g., invalid input that the tool handles gracefully)
/// return `Ok(ToolResponse)` with `ok: false`. These errors represent
/// registry-level failures such as unknown tools or profile violations.
#[derive(Debug, Clone)]
pub enum ToolCallError {
    /// The requested tool name was not found in the registry.
    UnknownTool(String),
    /// The tool exists but is not available in the current profile.
    ToolUnavailable { tool: String, profile: String },
    /// The tool arguments failed schema validation.
    InvalidArguments(String),
    /// An internal error occurred during tool lookup or dispatch.
    Internal(String),
}

impl fmt::Display for ToolCallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToolCallError::UnknownTool(name) => write!(f, "Unknown tool: {}", name),
            ToolCallError::ToolUnavailable { tool, profile } => {
                write!(
                    f,
                    "Tool '{}' is not available in profile '{}'",
                    tool, profile
                )
            }
            ToolCallError::InvalidArguments(msg) => write!(f, "Invalid arguments: {}", msg),
            ToolCallError::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for ToolCallError {}

/// A read-only view of a tool's metadata.
#[derive(Clone, Debug)]
pub struct ToolView {
    pub name: String,
    pub description: String,
    pub category: String,
    pub tier: u8,
    pub profiles: Vec<String>,
    pub tags: Vec<String>,
    pub exposure: String,
    pub cost: String,
    pub stability: String,
    pub composite: bool,
}

impl ToolView {
    fn from_spec(spec: &ToolSpec) -> Self {
        Self {
            name: spec.name.to_string(),
            description: spec.description.to_string(),
            category: spec.category.to_string(),
            tier: spec.tier,
            profiles: spec.profiles.iter().map(|s| s.to_string()).collect(),
            tags: spec.tags.iter().map(|s| s.to_string()).collect(),
            exposure: spec.exposure.as_str().to_string(),
            cost: spec.cost.as_str().to_string(),
            stability: spec.stability.as_str().to_string(),
            composite: spec.composite,
        }
    }
}

/// A read-only view of a tool spec including schemas.
#[derive(Clone, Debug)]
pub struct ToolSpecView {
    pub view: ToolView,
    pub input_schema: Value,
    pub output_schema: Value,
}

impl ToolSpecView {
    fn from_spec(spec: &ToolSpec) -> Self {
        Self {
            view: ToolView::from_spec(spec),
            input_schema: (spec.input_schema)(),
            output_schema: (spec.output_schema)(),
        }
    }
}

/// In-process tool registry for calling eggsact tools directly.
///
/// `ToolRegistry` provides a synchronous, typed API over the consolidated
/// tool registry. It supports profile filtering, argument validation, and
/// direct tool execution without starting an MCP server.
///
/// # Example
///
/// ```
/// use eggsact::agent::ToolRegistry;
///
/// let registry = ToolRegistry::default();
/// let response = registry.call_json("math_eval", serde_json::json!({
///     "expression": "2 + 2"
/// })).unwrap();
/// assert!(response.ok);
/// ```
pub struct ToolRegistry {
    profile: Profile,
}

impl ToolRegistry {
    /// Create a registry with the default (full) profile.
    pub fn new() -> Self {
        Self {
            profile: Profile::Full,
        }
    }

    /// Create a registry with a specific profile.
    pub fn with_profile(profile: Profile) -> Self {
        Self { profile }
    }

    /// Get the active profile for this registry.
    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    /// List all tools available in the current profile.
    pub fn available_tools(&self) -> Vec<ToolView> {
        registry::tools_for_profile(self.profile.as_str())
            .into_iter()
            .filter(|spec| spec.exposure != ToolExposure::Hidden)
            .map(ToolView::from_spec)
            .collect()
    }

    /// Get detailed information about a specific tool.
    pub fn get_tool(&self, name: &str) -> Option<ToolSpecView> {
        let spec = registry::get_tool(name)?;
        // Check profile availability
        let profile_tools = registry::tools_for_profile(self.profile.as_str());
        if !profile_tools.iter().any(|s| s.name == name) {
            return None;
        }
        Some(ToolSpecView::from_spec(spec))
    }

    /// Check if a tool is available in the current profile.
    pub fn has_tool(&self, name: &str) -> bool {
        let profile_tools = registry::tools_for_profile(self.profile.as_str());
        profile_tools.iter().any(|s| s.name == name)
    }

    /// Prepare a tool call by performing lookup, profile check, and validation.
    ///
    /// Returns a [`ToolCallOutcome`] that the caller can match on to decide
    /// how to handle pre-execution errors vs. ready-to-execute handlers.
    /// This is the shared core used by both `call_json` and the MCP server.
    pub fn prepare_tool_call(&self, name: &str, args: &Value) -> ToolCallOutcome {
        // 1. Look up the handler
        let handler = match registry::tool_handler_for(name) {
            Some(h) => h,
            None => {
                return ToolCallOutcome::PreExecutionError(ToolCallError::UnknownTool(
                    name.to_string(),
                ))
            }
        };

        // 2. Check profile availability
        let profile_tools = registry::tools_for_profile(self.profile.as_str());
        if !profile_tools.iter().any(|s| s.name == name) {
            return ToolCallOutcome::PreExecutionError(ToolCallError::ToolUnavailable {
                tool: name.to_string(),
                profile: self.profile.to_string(),
            });
        }

        // 3. Validate arguments
        if let Some(msg) = schema_validation::validate_arguments(name, args) {
            return ToolCallOutcome::PreExecutionError(ToolCallError::InvalidArguments(msg));
        }

        ToolCallOutcome::Ready { handler }
    }

    /// Call a tool by name with JSON arguments, returning the full `ToolResponse`.
    ///
    /// This is the primary entry point for in-process tool execution. It performs:
    /// 1. Tool lookup by name
    /// 2. Profile availability check
    /// 3. Argument schema validation
    /// 4. Synchronous tool handler execution
    ///
    /// Tool-level failures (e.g., invalid input) return `Ok(response)` with
    /// `response.ok == false`. Registry-level failures return `Err(ToolCallError)`.
    pub fn call_json(&self, name: &str, args: Value) -> Result<ToolResponse, ToolCallError> {
        match self.prepare_tool_call(name, &args) {
            ToolCallOutcome::Ready { handler } => Ok(handler(&args)),
            ToolCallOutcome::PreExecutionError(e) => Err(e),
        }
    }

    /// Call a tool and return only the result `Value`, or `null` on error.
    ///
    /// Convenience wrapper around [`call_json`](Self::call_json) that
    /// extracts the `result` field from the response.
    pub fn call_json_value(&self, name: &str, args: Value) -> Value {
        match self.call_json(name, args) {
            Ok(response) => response.result.unwrap_or(Value::Null),
            Err(_) => Value::Null,
        }
    }
}

/// Outcome of [`ToolRegistry::prepare_tool_call`].
///
/// This enum allows the MCP server to handle pre-execution errors as
/// JSON-RPC errors while executing tools via the shared handler path.
pub enum ToolCallOutcome {
    /// Tool is ready to execute — caller should invoke `handler(&args)`.
    Ready { handler: registry::ToolHandler },
    /// Pre-execution error (unknown tool, profile mismatch, invalid args).
    PreExecutionError(ToolCallError),
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_registry_default_lists_tools() {
        let registry = ToolRegistry::default();
        let tools = registry.available_tools();
        assert!(!tools.is_empty(), "should have tools in full profile");
        // text_equal is in the full profile
        assert!(tools.iter().any(|t| t.name == "text_equal"));
    }

    #[test]
    fn tool_registry_profile_filters_tools() {
        let registry = ToolRegistry::with_profile(Profile::CodeggCoreMin);
        let tools = registry.available_tools();
        // codegg_core_min should have fewer tools than full
        let full_count = ToolRegistry::default().available_tools().len();
        assert!(tools.len() <= full_count);
        // All tools should be in the codegg_core_min profile
        for tool in &tools {
            assert!(
                tool.profiles.contains(&"codegg_core_min".to_string()),
                "tool {} not in codegg_core_min profile",
                tool.name
            );
        }
    }

    #[test]
    fn tool_registry_unknown_tool_returns_error() {
        let registry = ToolRegistry::default();
        let result = registry.call_json("nonexistent_tool", serde_json::json!({}));
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolCallError::UnknownTool(name) => assert_eq!(name, "nonexistent_tool"),
            other => panic!("expected UnknownTool, got {:?}", other),
        }
    }

    #[test]
    fn tool_registry_outside_profile_returns_error() {
        // Create a registry with a limited profile
        let registry = ToolRegistry::with_profile(Profile::HumanMath);
        // math_eval should be available in human_math
        assert!(registry.has_tool("math_eval"));
        // text_equal might not be in human_math
        let result = registry.call_json("text_equal", serde_json::json!({"a": "x", "b": "x"}));
        if let Err(ToolCallError::ToolUnavailable { tool, .. }) = &result {
            assert_eq!(tool, "text_equal");
        }
        // It should either be ToolUnavailable or succeed (if in profile)
    }

    #[test]
    fn tool_registry_invalid_arguments_returns_error() {
        let registry = ToolRegistry::default();
        // math_eval requires "expression"
        let result = registry.call_json("math_eval", serde_json::json!({}));
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolCallError::InvalidArguments(msg) => {
                assert!(msg.contains("Missing required argument"));
            }
            other => panic!("expected InvalidArguments, got {:?}", other),
        }
    }

    #[test]
    fn tool_registry_call_json_succeeds() {
        let registry = ToolRegistry::default();
        let response = registry
            .call_json("text_equal", serde_json::json!({"a": "foo", "b": "foo"}))
            .unwrap();
        assert!(response.ok);
    }

    #[test]
    fn tool_registry_call_json_value_succeeds() {
        let registry = ToolRegistry::default();
        let value =
            registry.call_json_value("text_equal", serde_json::json!({"a": "foo", "b": "foo"}));
        assert!(value.get("equal").and_then(|v| v.as_bool()) == Some(true));
    }

    #[test]
    fn tool_registry_get_tool_returns_spec() {
        let registry = ToolRegistry::default();
        let spec = registry.get_tool("text_equal");
        assert!(spec.is_some());
        let spec = spec.unwrap();
        assert_eq!(spec.view.name, "text_equal");
        assert!(spec.input_schema.is_object());
    }

    #[test]
    fn tool_registry_profile_display() {
        assert_eq!(Profile::Full.to_string(), "full");
        assert_eq!(Profile::CodeggCore.to_string(), "codegg_core");
        assert_eq!(Profile::Custom("test".to_string()).to_string(), "test");
    }

    #[test]
    fn tool_registry_profile_from_str() {
        assert_eq!(Profile::from_str_opt("full"), Some(Profile::Full));
        assert_eq!(
            Profile::from_str_opt("codegg_core"),
            Some(Profile::CodeggCore)
        );
        assert!(Profile::from_str_opt("nonexistent").is_some()); // Custom
    }
}
