use std::sync::LazyLock;

use super::types::ToolSpec;
use crate::mcp::specs::*;

// ---------------------------------------------------------------------------
// Static registry (aggregated from category-local modules)
//
// Each category owns its tool declarations in src/mcp/specs/<category>.rs.
// This file aggregates them into ALL_TOOLS, preserving the exact order needed
// for parity with the Python eggcalc reference implementation.
//
// Adding a new tool: append to the category in specs/<category>.rs.
// The category slice and ALL_TOOLS aggregation below will pick it up
// automatically.
// ---------------------------------------------------------------------------

static ALL_TOOLS_VEC: LazyLock<Vec<ToolSpec>> = LazyLock::new(|| {
    let mut tools = Vec::new();
    tools.extend_from_slice(MATH_TOOLS);
    tools.extend_from_slice(TEXT_TOOLS);
    tools.extend_from_slice(VALIDATION_TOOLS);
    tools.extend_from_slice(REGEX_TOOLS);
    tools.extend_from_slice(LIST_TOOLS);
    tools.extend_from_slice(JSON_TOOLS);
    tools.extend_from_slice(PATH_TOOLS);
    tools.extend_from_slice(SHELL_TOOLS);
    tools.extend_from_slice(MARKDOWN_TOOLS);
    tools.extend_from_slice(CONFIG_TOOLS);
    tools.extend_from_slice(UNICODE_TOOLS);
    tools.extend_from_slice(IDENTIFIER_TOOLS);
    tools.extend_from_slice(VERSION_TOOLS);
    tools.extend_from_slice(TOML_TOOLS);
    tools.extend_from_slice(PATCH_TOOLS);
    tools.extend_from_slice(CARGO_TOOLS);
    tools.extend_from_slice(DEPENDENCY_TOOLS);
    tools.extend_from_slice(REPO_TOOLS);
    tools.extend_from_slice(ANALYSIS_TOOLS);
    tools.extend_from_slice(DIAGNOSTICS_TOOLS);
    tools
});

pub fn all_tools() -> &'static [ToolSpec] {
    &ALL_TOOLS_VEC
}

// ---------------------------------------------------------------------------
// Profile constants
// ---------------------------------------------------------------------------

pub const PROFILE_NAMES: &[&str] = &[
    "full",
    "default",
    "codegg_core_min",
    "codegg_core",
    "codegg_preflight",
    "codegg_patch",
    "codegg_config",
    "codegg_unicode_security",
    "codegg_shell",
    "codegg_repo_audit",
    "human_math",
];
