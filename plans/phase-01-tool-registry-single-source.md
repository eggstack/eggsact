# Phase 1: Tool Registry Single Source of Truth

## Goal

Replace the current multi-table MCP tool registration model with a single-source `ToolSpec` registry. The immediate objective is to preserve all current behavior while making future tool additions safer, easier to review, and harder to drift.

Today, the MCP layer effectively maintains separate surfaces for handler dispatch, tool metadata, raw `ToolDefinition` entries, output schemas, profile membership, and docs. Existing tests catch some drift, but the design still requires humans to update multiple places for a single tool. Phase 1 should collapse those surfaces into one declarative registry and generate all derived views from it.

## Scope

This phase should focus on registry structure and compatibility preservation. It should not attempt a broad module split, behavior changes to tools, new feature work, or major protocol changes. The existing MCP responses should remain compatible with current tests and consumers.

In scope:

- Define a central `ToolSpec` abstraction.
- Move tool metadata, schemas, exposure, profile data, and handler references into the registry.
- Generate handler lookup, `tools/list`, output schema lookup, profile lookup, tag/tier filtering, and tool count from the registry.
- Preserve existing profile names and current default behavior.
- Strengthen registration invariants and tests.

Out of scope:

- Rewriting individual tool implementations.
- Changing MCP transport semantics.
- Introducing typed codegg APIs.
- Changing response payload shape beyond what is required for generated registry support.

## Proposed Design

Add a registry module, initially under `src/mcp/registry.rs` or `src/mcp/tool_registry.rs`. The name is less important than the direction: all tool declarations should be concentrated into typed declarations rather than parallel maps.

A first-pass structure can be simple:

```rust
pub type ToolHandler = fn(&serde_json::Value) -> crate::mcp::schemas::ToolResponse;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ToolExposure {
    Default,
    Contextual,
    ExpertOnly,
    HarnessOnly,
    Hidden,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ToolCost {
    Cheap,
    Moderate,
    Heavy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ToolStability {
    Stable,
    Deprecated,
    Experimental,
}

pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub handler: ToolHandler,
    pub input_schema: fn() -> serde_json::Value,
    pub output_schema: fn() -> serde_json::Value,
    pub category: &'static str,
    pub tier: u8,
    pub profiles: &'static [&'static str],
    pub tags: &'static [&'static str],
    pub exposure: ToolExposure,
    pub harness_use: &'static [&'static str],
    pub aliases: &'static [&'static str],
    pub cost: ToolCost,
    pub stability: ToolStability,
    pub composite: bool,
}
```

Using functions for schemas avoids some static initialization pain and keeps the first pass straightforward. If repeated schema construction becomes expensive, add a registry-level `LazyLock` cache later.

The registry should expose read-only derived helpers:

```rust
pub fn all_tools() -> &'static [ToolSpec];
pub fn get_tool(name: &str) -> Option<&'static ToolSpec>;
pub fn tool_names() -> Vec<&'static str>;
pub fn tools_for_profile(profile: &str) -> Vec<&'static ToolSpec>;
pub fn available_profiles() -> &'static [&'static str];
pub fn mcp_tool_definitions() -> Vec<ToolDefinition>;
pub fn input_schema_for(name: &str) -> Option<Value>;
pub fn output_schema_for(name: &str) -> Option<Value>;
```

The initial implementation can keep all `ToolSpec` declarations in one file if needed, but the declarations should be written in a way that Phase 2 can move them into category modules without changing consumers.

## Implementation Sequence

### Step 1: Add Registry Types

Create the registry module and define `ToolSpec`, exposure/cost/stability enums, conversion helpers, profile constants, and a static registry slice.

Use enum-to-string methods so existing MCP output can preserve current strings. For compatibility, map:

- `ToolExposure::Default` -> `default`
- `ToolExposure::Contextual` -> `contextual`
- `ToolExposure::ExpertOnly` -> `expert_only`
- `ToolExposure::HarnessOnly` -> `harness_only`
- `ToolExposure::Hidden` -> `hidden`

Do the same for cost and stability.

### Step 2: Port Tool Declarations

Move the existing handler table, metadata, input schema definitions, and output schemas into `ToolSpec` declarations. This is the core mechanical part of the phase.

Preserve names exactly. Do not rename tools. Preserve profile membership, tags, tier, category, cost, stability, and composite flags exactly unless an existing value is clearly inconsistent and a test can prove the intended behavior.

### Step 3: Generate MCP Tool Definitions

Refactor `list_tools_raw()` or its replacement so it iterates over registry specs and creates `ToolDefinition` values. The enrichment step should become unnecessary or minimal because all metadata should already be present on the spec.

The generated `ToolDefinition` should preserve the current serialized fields:

- `name`
- `description`
- `inputSchema`
- `outputSchema`
- `tier`
- `tags`
- `deprecated`
- `category`
- `llm_exposure`
- `cost`

Compact, normal, and full schema behavior should remain compatible.

### Step 4: Generate Handler Lookup

Replace direct search through `TOOL_HANDLERS` with registry lookup. The call path should locate the `ToolSpec` and invoke `spec.handler`.

Unknown-tool suggestions should use registry tool names instead of the old handler table.

### Step 5: Generate Profile Membership

Replace `TOOL_METADATA`-derived profile maps with registry-derived profile membership. Existing profile names must remain stable:

- `full`
- `default`
- `codegg_core_min`
- `codegg_core`
- `codegg_preflight`
- `codegg_patch`
- `codegg_config`
- `codegg_unicode_security`
- `codegg_shell`
- `codegg_repo_audit`
- `human_math`

The `full` profile should continue to include all non-hidden tools.

### Step 6: Replace Output Schema Lookup

Remove the separate output schema map after all output schemas are represented in `ToolSpec`. If retaining an internal cache for performance, it must be derived from the registry and not hand-maintained.

### Step 7: Strengthen Invariant Tests

Replace the old sync test with registry invariant tests:

- Tool names are unique.
- Every tool has a nonempty description.
- Every tool has a valid input schema object.
- Every tool has a valid output schema object.
- Every tool references only known profiles.
- Every profile has at least one tool unless intentionally empty.
- Every non-hidden tool appears in `full` output.
- Deprecated tools serialize with `deprecated: true` in non-compact output.
- Compact schema generation still strips/truncates as expected.
- `mcp_tool_count()` equals registry tool count after exposure filtering rules.

## Compatibility Requirements

This phase should keep the current public behavior stable. Existing tests for protocol, lifecycle, real tool use, response structure, and tool coverage should continue to pass. If the serialized ordering of `tools/list` changes, either preserve the old order intentionally or add tests that assert deterministic sorted order.

Avoid opportunistic cleanup that changes output text, error strings, or schema field names. The purpose is structural consolidation, not semantic change.

## Testing Plan

Run:

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

Add targeted tests for:

- `tools/list` returns the same count as before.
- `tools/list` supports `profile`, `tier`, `tags`, `names`, and `schema_detail` filters after the registry migration.
- `tools/call` can invoke representative tools from each category.
- Unknown tool suggestion still works.
- Invalid profile rejection still works.
- Deprecated `json_query` remains marked deprecated.

If exact output snapshots already exist, update only where generated order differs and document the reason.

## Risks

The largest risk is accidental schema drift. Porting many hand-written schemas into `ToolSpec` declarations is mechanical but error-prone. Mitigate by adding temporary comparison tests if practical: build old and new lists in parallel during the transition and assert equivalent names, schemas, metadata, and output schemas before deleting old tables.

The second risk is static initialization complexity. Prefer simple schema functions returning `serde_json::json!` values for the first pass. Optimize later if needed.

The third risk is ordering instability. Make ordering deterministic by preserving declaration order or sorting explicitly.

## Acceptance Criteria

Phase 1 is complete when:

- A single registry owns handler, schemas, metadata, profile membership, exposure, cost, stability, and composite flags.
- Old manually synchronized handler/metadata/definition/output schema tables are removed or reduced to generated registry views.
- Existing MCP clients see compatible tool names, schemas, metadata, and call behavior.
- Registry invariant tests protect against future drift.
- `cargo fmt`, `cargo clippy`, and `cargo test` pass.

## Handoff Notes

Prefer minimal behavior changes. If a schema inconsistency is discovered during migration, preserve existing behavior first and leave a TODO or follow-up issue unless it is clearly a bug covered by a new test. Phase 1 should make future changes easier; it should not bundle broad tool semantics changes.
