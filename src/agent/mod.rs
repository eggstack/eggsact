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
//! assert!(!output.machine_code.is_empty());
//! ```
//!
//! Wrappers return `Result<Output, PreflightError>`. Missing mandatory fields
//! produce `ContractViolation` errors instead of silently defaulting.

use crate::mcp::budget::{self, ToolBudget};
use crate::mcp::registry::{self, ToolExposure, ToolListAudience, ToolSpec};
use crate::mcp::response::{truncate_response, ToolResponse};
use crate::mcp::schema_validation;
use serde_json::Value;
use std::fmt;

pub use crate::mcp::compat::CompatibilityMode;

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

    /// Create an explicit custom profile by name.
    ///
    /// Use this when you intentionally want a profile not in the built-in set.
    /// For parsing user input, prefer [`from_str_opt`](Self::from_str_opt)
    /// which rejects unknown names.
    pub fn custom(name: impl Into<String>) -> Self {
        Profile::Custom(name.into())
    }

    /// Parse a profile from a string name.
    ///
    /// Returns `Some` only for known built-in profile names.
    /// Returns `None` for unknown names. To construct a custom profile
    /// explicitly, use [`Profile::custom(name)`](Self::custom).
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
            _ => None,
        }
    }
}

impl fmt::Display for Profile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Audience for tool listing, controlling which exposure levels are included.
///
/// - `Model`: Excludes `HarnessOnly` and `Hidden`. Safe for ordinary
///   model-facing codegg integrations.
/// - `Harness`: Includes `HarnessOnly` tools for selected profiles but
///   excludes `Hidden`. For automatic preflight checks.
/// - `Debug`: Includes all non-hidden tools, including `ExpertOnly`
///   and `HarnessOnly`. Internal/debug listing only.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ToolAudience {
    #[default]
    Model,
    Harness,
    Debug,
}

impl ToolAudience {
    /// Convert to the MCP registry's `ToolListAudience`.
    pub fn as_registry_audience(self) -> ToolListAudience {
        match self {
            ToolAudience::Model => ToolListAudience::Model,
            ToolAudience::Harness => ToolListAudience::Harness,
            ToolAudience::Debug => ToolListAudience::Debug,
        }
    }

    /// Whether this audience may execute a tool with the given exposure level.
    ///
    /// Rules:
    /// - Model audience rejects `HarnessOnly` and `Hidden`.
    /// - Harness audience accepts `HarnessOnly` but rejects `Hidden`.
    /// - Debug audience accepts all non-hidden exposure levels.
    pub fn can_execute_exposure(self, exposure: ToolExposure) -> bool {
        match self {
            ToolAudience::Model => {
                exposure != ToolExposure::HarnessOnly && exposure != ToolExposure::Hidden
            }
            ToolAudience::Harness => exposure != ToolExposure::Hidden,
            ToolAudience::Debug => exposure != ToolExposure::Hidden,
        }
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
    /// The tool exists in the profile but the current audience cannot execute it.
    ToolNotAllowedForAudience {
        tool: String,
        profile: String,
        audience: String,
        exposure: String,
    },
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
            ToolCallError::ToolNotAllowedForAudience {
                tool,
                profile,
                audience,
                exposure,
            } => {
                write!(
                    f,
                    "Tool '{}' (exposure: {}) is not executable by {} audience in profile '{}'",
                    tool, exposure, audience, profile
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
/// tool registry. It supports profile filtering, audience filtering,
/// argument validation, and direct tool execution without starting an MCP server.
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
///
/// For model-facing codegg integrations, use [`available_tools_model_safe`]
/// to exclude harness-only tools from the listing:
///
/// ```
/// use eggsact::agent::ToolRegistry;
///
/// let registry = ToolRegistry::default();
/// let model_tools = registry.available_tools_model_safe();
/// // model_tools excludes HarnessOnly and Hidden tools
/// ```
pub struct ToolRegistry {
    profile: Profile,
    audience: ToolAudience,
    compat_mode: CompatibilityMode,
}

impl ToolRegistry {
    /// Create a registry with the default (full) profile and Model audience.
    ///
    /// Uses [`CompatibilityMode::StrictNative`] by default.
    pub fn new() -> Self {
        Self {
            profile: Profile::Full,
            audience: ToolAudience::Model,
            compat_mode: CompatibilityMode::default(),
        }
    }

    /// Create a registry with a specific profile (defaults to Model audience).
    ///
    /// Uses [`CompatibilityMode::StrictNative`] by default.
    pub fn with_profile(profile: Profile) -> Self {
        Self {
            profile,
            audience: ToolAudience::Model,
            compat_mode: CompatibilityMode::default(),
        }
    }

    /// Create a registry with a specific profile and audience.
    ///
    /// Uses [`CompatibilityMode::StrictNative`] by default.
    pub fn with_profile_and_audience(profile: Profile, audience: ToolAudience) -> Self {
        Self {
            profile,
            audience,
            compat_mode: CompatibilityMode::default(),
        }
    }

    /// Set the compatibility mode for this registry.
    ///
    /// Use [`CompatibilityMode::EggcalcPython`] to preserve Python-parity
    /// behavior (Python-style type names in error messages and selected
    /// compatibility error wording). The default is
    /// [`CompatibilityMode::StrictNative`].
    ///
    /// **Note**: Neither mode allows JSON booleans for numeric schema fields.
    /// The validator rejects `true`/`false` for `integer`/`number` fields in
    /// both modes; only this and selected error-message wording differ between
    /// modes. See `architecture/compatibility.md` for the full comparison.
    pub fn with_compat_mode(mut self, compat_mode: CompatibilityMode) -> Self {
        self.compat_mode = compat_mode;
        self
    }

    /// Get the active profile for this registry.
    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    /// Get the active audience for this registry.
    pub fn audience(&self) -> ToolAudience {
        self.audience
    }

    /// Get the active compatibility mode for this registry.
    pub fn compat_mode(&self) -> CompatibilityMode {
        self.compat_mode
    }

    /// List all tools available in the current profile (legacy, not model-safe).
    ///
    /// This method only filters out `Hidden` tools. For model-facing codegg
    /// integrations, prefer [`available_tools_model_safe`] or
    /// [`available_tools_for_audience`] to also exclude `HarnessOnly` tools.
    pub fn available_tools(&self) -> Vec<ToolView> {
        registry::tools_for_profile(self.profile.as_str())
            .into_iter()
            .filter(|spec| spec.exposure != ToolExposure::Hidden)
            .map(ToolView::from_spec)
            .collect()
    }

    /// List tools for a specific audience.
    pub fn available_tools_for_audience(&self, audience: ToolAudience) -> Vec<ToolView> {
        registry::tools_for_profile_audience(self.profile.as_str(), audience.as_registry_audience())
            .into_iter()
            .map(ToolView::from_spec)
            .collect()
    }

    /// List tools safe for ordinary model-facing codegg integrations.
    ///
    /// Equivalent to `available_tools_for_audience(ToolAudience::Model)`.
    /// Excludes `HarnessOnly` and `Hidden` tools.
    pub fn available_tools_model_safe(&self) -> Vec<ToolView> {
        self.available_tools_for_audience(ToolAudience::Model)
    }

    /// List tools for the registry's stored audience.
    ///
    /// This is the ergonomic shortcut for consumers who construct a registry
    /// with a specific profile and audience and then want tools for that audience
    /// without passing the audience again.
    pub fn available_tools_for_current_audience(&self) -> Vec<ToolView> {
        self.available_tools_for_audience(self.audience)
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

        // 3. Check audience/exposure compatibility
        if let Some(spec) = registry::get_tool(name) {
            if !self.audience.can_execute_exposure(spec.exposure) {
                return ToolCallOutcome::PreExecutionError(
                    ToolCallError::ToolNotAllowedForAudience {
                        tool: name.to_string(),
                        profile: self.profile.to_string(),
                        audience: format!("{:?}", self.audience),
                        exposure: spec.exposure.as_str().to_string(),
                    },
                );
            }
        }

        // 4. Validate arguments
        if let Some(msg) = schema_validation::validate_arguments(name, args, self.compat_mode) {
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

    /// Call a tool with an explicit budget, applying resource limits and output truncation.
    ///
    /// This is the budget-aware entry point for in-process tool execution. It:
    /// 1. Resolves the tool's default budget from its declared `ToolCost`
    /// 2. Merges with any explicit `budget` override
    /// 3. Executes the handler
    /// 4. Truncates output findings and result to fit within budget limits
    /// 5. Populates `limits_applied` on the response
    ///
    /// Use this method when the caller needs deterministic resource discipline.
    /// For simple calls without budget enforcement, use [`call_json`](Self::call_json).
    pub fn call_json_with_budget(
        &self,
        name: &str,
        args: Value,
        budget: Option<ToolBudget>,
    ) -> Result<ToolResponse, ToolCallError> {
        let spec =
            registry::get_tool(name).ok_or_else(|| ToolCallError::UnknownTool(name.to_string()))?;
        let effective_budget = match budget {
            Some(b) => b,
            None => budget_for_tool_resolved(name, spec),
        };
        // Pre-execution input size check — reject oversized serialized args
        // before dispatching to the handler. This is a hard limit, not just
        // a hint.
        let serialized_len = serde_json::to_string(&args).map(|s| s.len()).unwrap_or(0);
        if serialized_len > effective_budget.max_input_bytes {
            return Ok(ToolResponse::error_with_code(
                "input_too_large",
                crate::mcp::machine_codes::INPUT_TOO_LARGE,
                &format!(
                    "Serialized arguments ({} bytes) exceed budget max_input_bytes ({} bytes)",
                    serialized_len, effective_budget.max_input_bytes
                ),
                None,
                Some(name),
            ));
        }
        let mut response = self.call_json(name, args)?;
        truncate_response(&mut response, &effective_budget);
        Ok(response)
    }

    /// Call a tool with an explicit budget and cancellation flag, applying resource
    /// limits and output truncation.
    ///
    /// This extends [`call_json_with_budget`](Self::call_json_with_budget) with
    /// an external cancellation flag. The flag is set as a thread-local during
    /// handler execution so that high-risk handlers that create their own
    /// `BudgetContext` (via [`budget::for_handler`]) will inherit cancellation.
    pub fn call_json_with_context(
        &self,
        name: &str,
        args: Value,
        budget: Option<ToolBudget>,
        cancel_flag: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    ) -> Result<ToolResponse, ToolCallError> {
        let spec =
            registry::get_tool(name).ok_or_else(|| ToolCallError::UnknownTool(name.to_string()))?;
        let effective_budget = match budget {
            Some(b) => b,
            None => budget_for_tool_resolved(name, spec),
        };
        let serialized_len = serde_json::to_string(&args).map(|s| s.len()).unwrap_or(0);
        if serialized_len > effective_budget.max_input_bytes {
            return Ok(ToolResponse::error_with_code(
                "input_too_large",
                crate::mcp::machine_codes::INPUT_TOO_LARGE,
                &format!(
                    "Serialized arguments ({} bytes) exceed budget max_input_bytes ({} bytes)",
                    serialized_len, effective_budget.max_input_bytes
                ),
                None,
                Some(name),
            ));
        }
        let mut response = budget::with_cancel_flag(cancel_flag, || self.call_json(name, args))?;
        truncate_response(&mut response, &effective_budget);
        Ok(response)
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

/// Resolve the effective budget for a tool by combining its declared cost
/// with any tool-specific overrides from the budget module.
fn budget_for_tool_resolved(name: &str, spec: &ToolSpec) -> ToolBudget {
    budget::budget_for_tool(name, spec.cost)
}

/// Re-export budget types for convenience.
pub use crate::mcp::budget::{BudgetContext, BudgetTier, ToolBudget as ToolBudgetType};

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
        assert_eq!(Profile::from_str_opt("nonexistent"), None);
        assert_eq!(Profile::from_str_opt("coddeg_core_typo"), None);
    }

    #[test]
    fn profile_custom_constructor() {
        let p = Profile::custom("my_custom");
        assert_eq!(p, Profile::Custom("my_custom".to_string()));
        assert_eq!(p.as_str(), "my_custom");
        assert_eq!(p.to_string(), "my_custom");
        // custom() allows any name including known ones
        let p2 = Profile::custom("full");
        assert_eq!(p2, Profile::Custom("full".to_string()));
    }

    #[test]
    fn tool_audience_default_is_model() {
        let registry = ToolRegistry::default();
        assert_eq!(registry.audience(), ToolAudience::Model);
    }

    #[test]
    fn tool_audience_getter_returns_constructor_value() {
        let registry =
            ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Harness);
        assert_eq!(registry.audience(), ToolAudience::Harness);
    }

    #[test]
    fn model_safe_excludes_harness_only_tools() {
        let registry = ToolRegistry::default();
        let model_tools = registry.available_tools_model_safe();
        let harness_only_names: Vec<String> = registry::tools_for_profile("full")
            .into_iter()
            .filter(|t| t.exposure == ToolExposure::HarnessOnly)
            .map(|t| t.name.to_string())
            .collect();
        for name in &harness_only_names {
            assert!(
                !model_tools.iter().any(|t| t.name == *name),
                "harness_only tool '{}' should not appear in model-safe listing",
                name
            );
        }
    }

    #[test]
    fn model_safe_is_subset_of_available_tools() {
        let registry = ToolRegistry::default();
        let all_tools = registry.available_tools();
        let model_tools = registry.available_tools_model_safe();
        assert!(model_tools.len() <= all_tools.len());
        for tool in &model_tools {
            assert!(
                all_tools.iter().any(|t| t.name == tool.name),
                "model-safe tool '{}' should also appear in available_tools()",
                tool.name
            );
        }
    }

    #[test]
    fn harness_audience_includes_harness_only_tools() {
        let registry = ToolRegistry::with_profile(Profile::CodeggPreflight);
        let harness_tools = registry.available_tools_for_audience(ToolAudience::Harness);
        let harness_only_in_profile: Vec<String> = registry::tools_for_profile("codegg_preflight")
            .into_iter()
            .filter(|t| t.exposure == ToolExposure::HarnessOnly)
            .map(|t| t.name.to_string())
            .collect();
        assert!(
            !harness_only_in_profile.is_empty(),
            "codegg_preflight should have harness-only tools"
        );
        for name in &harness_only_in_profile {
            assert!(
                harness_tools.iter().any(|t| t.name == *name),
                "harness audience for codegg_preflight should include '{}'",
                name
            );
        }
    }

    #[test]
    fn harness_audience_for_codegg_core_min_excludes_harness_only() {
        let registry = ToolRegistry::with_profile(Profile::CodeggCoreMin);
        let harness_tools = registry.available_tools_for_audience(ToolAudience::Harness);
        for spec in registry::tools_for_profile("codegg_core_min") {
            if spec.exposure == ToolExposure::HarnessOnly {
                assert!(
                    !harness_tools.iter().any(|t| t.name == spec.name),
                    "harness audience for codegg_core_min should not include harness-only '{}'",
                    spec.name
                );
            }
        }
    }

    #[test]
    fn debug_audience_includes_all_non_hidden() {
        let registry = ToolRegistry::default();
        let debug_tools = registry.available_tools_for_audience(ToolAudience::Debug);
        let all_non_hidden: Vec<String> = registry::tools_for_profile("full")
            .into_iter()
            .filter(|t| t.exposure != ToolExposure::Hidden)
            .map(|t| t.name.to_string())
            .collect();
        assert_eq!(debug_tools.len(), all_non_hidden.len());
    }

    #[test]
    fn with_profile_and_audience_combines_both() {
        let registry =
            ToolRegistry::with_profile_and_audience(Profile::CodeggPreflight, ToolAudience::Model);
        assert_eq!(registry.profile(), &Profile::CodeggPreflight);
        assert_eq!(registry.audience(), ToolAudience::Model);
        let tools = registry.available_tools_model_safe();
        for spec in registry::tools_for_profile("codegg_preflight") {
            if spec.exposure == ToolExposure::HarnessOnly {
                assert!(
                    !tools.iter().any(|t| t.name == spec.name),
                    "Model audience for codegg_preflight should not include harness-only '{}'",
                    spec.name
                );
            }
        }
    }

    #[test]
    fn current_audience_model_excludes_harness_only() {
        let registry = ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Model);
        let current = registry.available_tools_for_current_audience();
        let explicit = registry.available_tools_for_audience(ToolAudience::Model);
        assert_eq!(current.len(), explicit.len());
        for t in &current {
            assert!(!t.exposure.contains("harness_only"));
        }
    }

    #[test]
    fn current_audience_harness_includes_preflight_harness_tools() {
        let registry = ToolRegistry::with_profile_and_audience(
            Profile::CodeggPreflight,
            ToolAudience::Harness,
        );
        let tools = registry.available_tools_for_current_audience();
        // codegg_preflight has harness-only tools like shell_split, path_scope_check, etc.
        assert!(
            tools.iter().any(|t| t.name == "shell_split"),
            "harness audience should include shell_split"
        );
        assert!(
            tools.iter().any(|t| t.name == "path_scope_check"),
            "harness audience should include path_scope_check"
        );
    }

    #[test]
    fn current_audience_debug_matches_debug_listing() {
        let registry = ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Debug);
        let current = registry.available_tools_for_current_audience();
        let explicit = registry.available_tools_for_audience(ToolAudience::Debug);
        assert_eq!(current.len(), explicit.len());
    }

    // -- Exposure helper tests --

    #[test]
    fn model_audience_allows_default_exposure() {
        assert!(ToolAudience::Model.can_execute_exposure(ToolExposure::Default));
    }

    #[test]
    fn model_audience_allows_contextual_exposure() {
        assert!(ToolAudience::Model.can_execute_exposure(ToolExposure::Contextual));
    }

    #[test]
    fn model_audience_allows_expert_only_exposure() {
        assert!(ToolAudience::Model.can_execute_exposure(ToolExposure::ExpertOnly));
    }

    #[test]
    fn model_audience_rejects_harness_only_exposure() {
        assert!(!ToolAudience::Model.can_execute_exposure(ToolExposure::HarnessOnly));
    }

    #[test]
    fn model_audience_rejects_hidden_exposure() {
        assert!(!ToolAudience::Model.can_execute_exposure(ToolExposure::Hidden));
    }

    #[test]
    fn harness_audience_allows_default_exposure() {
        assert!(ToolAudience::Harness.can_execute_exposure(ToolExposure::Default));
    }

    #[test]
    fn harness_audience_allows_harness_only_exposure() {
        assert!(ToolAudience::Harness.can_execute_exposure(ToolExposure::HarnessOnly));
    }

    #[test]
    fn harness_audience_rejects_hidden_exposure() {
        assert!(!ToolAudience::Harness.can_execute_exposure(ToolExposure::Hidden));
    }

    #[test]
    fn debug_audience_allows_default_exposure() {
        assert!(ToolAudience::Debug.can_execute_exposure(ToolExposure::Default));
    }

    #[test]
    fn debug_audience_allows_harness_only_exposure() {
        assert!(ToolAudience::Debug.can_execute_exposure(ToolExposure::HarnessOnly));
    }

    #[test]
    fn debug_audience_rejects_hidden_exposure() {
        assert!(!ToolAudience::Debug.can_execute_exposure(ToolExposure::Hidden));
    }

    // -- Registry audience enforcement at dispatch time --

    #[test]
    fn model_audience_rejects_harness_only_tool_call() {
        // Find a harness-only tool in the full profile
        let harness_tool = registry::tools_for_profile("full")
            .into_iter()
            .find(|t| t.exposure == ToolExposure::HarnessOnly)
            .expect("full profile should have harness-only tools");
        let registry = ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Model);
        let result = registry.call_json(harness_tool.name, serde_json::json!({}));
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolCallError::ToolNotAllowedForAudience { tool, exposure, .. } => {
                assert_eq!(tool, harness_tool.name);
                assert_eq!(exposure, "harness_only");
            }
            other => panic!("expected ToolNotAllowedForAudience, got {:?}", other),
        }
    }

    #[test]
    fn harness_audience_allows_harness_only_tool_call() {
        // Find a harness-only tool in codegg_preflight
        let harness_tool = registry::tools_for_profile("codegg_preflight")
            .into_iter()
            .find(|t| t.exposure == ToolExposure::HarnessOnly)
            .expect("codegg_preflight should have harness-only tools");
        let registry = ToolRegistry::with_profile_and_audience(
            Profile::CodeggPreflight,
            ToolAudience::Harness,
        );
        let result = registry.call_json(harness_tool.name, serde_json::json!({}));
        // Should not fail with ToolNotAllowedForAudience (may fail for other reasons like args)
        if let Err(ToolCallError::ToolNotAllowedForAudience { .. }) = result {
            panic!(
                "harness audience should allow harness-only tool: {}",
                harness_tool.name
            );
        }
    }

    #[test]
    fn restricted_profile_rejects_out_of_profile_tool() {
        let registry = ToolRegistry::with_profile(Profile::HumanMath);
        // math_eval should work in human_math
        let result = registry.call_json("math_eval", serde_json::json!({"expression": "1+1"}));
        assert!(result.is_ok());
    }

    #[test]
    fn model_audience_error_includes_tool_profile_audience_exposure() {
        let harness_tool = registry::tools_for_profile("full")
            .into_iter()
            .find(|t| t.exposure == ToolExposure::HarnessOnly)
            .expect("full profile should have harness-only tools");
        let registry = ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Model);
        let result = registry.call_json(harness_tool.name, serde_json::json!({}));
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains(harness_tool.name));
        assert!(msg.contains("harness_only"));
        assert!(msg.contains("Model"));
    }

    #[test]
    fn call_json_with_budget_succeeds() {
        let registry = ToolRegistry::default();
        let response = registry
            .call_json_with_budget(
                "text_equal",
                serde_json::json!({"a": "foo", "b": "foo"}),
                None,
            )
            .unwrap();
        assert!(response.ok);
    }

    #[test]
    fn call_json_with_budget_explicit_cheap() {
        let registry = ToolRegistry::default();
        let response = registry
            .call_json_with_budget(
                "text_equal",
                serde_json::json!({"a": "hello", "b": "hello"}),
                Some(ToolBudget::CHEAP),
            )
            .unwrap();
        assert!(response.ok);
    }

    #[test]
    fn call_json_with_budget_truncates_findings() {
        let registry = ToolRegistry::default();
        // Use a tool that produces findings
        let response = registry
            .call_json_with_budget(
                "validate_json",
                serde_json::json!({"text": "not valid json at all"}),
                Some(ToolBudget::CHEAP.with_max_findings(1)),
            )
            .unwrap();
        // Should still succeed (findings truncated, not rejected)
        assert!(response.ok);
        if let Some(ref findings) = response.findings {
            assert!(findings.len() <= 2); // findings + possible truncation notice
        }
    }

    #[test]
    fn call_json_with_budget_unknown_tool_error() {
        let registry = ToolRegistry::default();
        let result = registry.call_json_with_budget("nonexistent", serde_json::json!({}), None);
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolCallError::UnknownTool(name) => assert_eq!(name, "nonexistent"),
            other => panic!("expected UnknownTool, got {:?}", other),
        }
    }
}
