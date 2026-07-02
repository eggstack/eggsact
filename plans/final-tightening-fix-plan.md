# Final Tightening Fix Plan

## Purpose

The post-corrective implementation materially improved eggsact. The dependency direction between registry and server is now correct, schemas were moved into category modules, the public agent API gained audience-aware listing, strict profile parsing was added, and exact codegg profile snapshots now protect critical tool sets.

This plan covers the remaining tightening work before the next feature-expansion phase. The goal is to finish the architectural cleanup so eggsact is stable enough for codegg to consume as a deterministic local substrate.

## Current State

The repository is now in a much better state:

- `ToolDefinition` lives in `src/mcp/registry.rs`, so the registry no longer depends on `server.rs`.
- `server.rs` imports protocol helpers from `mcp::protocol` and response wrappers from `mcp::response`, reducing adapter leakage.
- `src/mcp/schemas/*` splits schema builders by category.
- `ToolRegistry` now stores `profile` and `audience` and exposes `available_tools_for_audience()` plus `available_tools_model_safe()`.
- `Profile::from_str_opt()` is now strict, while custom profiles require explicit `Profile::custom(...)`.
- Exact profile snapshots now protect codegg-facing model/harness tool sets.
- The old machine-code-less error constructor was renamed to `error_without_code_for_legacy_tests_only`, hidden from docs, and deprecated.

Remaining issues are narrower:

1. `registry.rs` still centralizes all `ToolSpec` declarations.
2. Machine-code-less error construction still exists as a public hidden method.
3. The `ToolRegistry` stored audience is not yet used by a current-audience listing method.
4. Preflight tool exposure needs a final policy audit before codegg relies on profile snapshots.
5. `tools/list` filtering logic still lives in `server.rs`.
6. CI or visible verification remains absent.

## Priority 1: Move ToolSpec Declarations into Category-Local Spec Modules

### Problem

`src/mcp/registry.rs` is no longer carrying all schema builders, but it still contains the entire `ALL_TOOLS` declaration. This preserves a central maintenance hotspot. Adding or changing a tool still requires editing a large global file instead of the relevant category.

### Desired End State

The registry root should own shared types and aggregation, not every individual declaration. Category modules should own their own specs near their schemas and implementations.

Target shape:

```text
src/mcp/registry.rs              # types, aggregation, listing helpers
src/mcp/specs/mod.rs             # category aggregation
src/mcp/specs/math.rs            # math ToolSpec declarations
src/mcp/specs/text.rs            # text ToolSpec declarations
src/mcp/specs/json.rs            # json ToolSpec declarations
src/mcp/specs/regex.rs
src/mcp/specs/path.rs
src/mcp/specs/shell.rs
src/mcp/specs/patch.rs
src/mcp/specs/config.rs
src/mcp/specs/unicode.rs
src/mcp/specs/identifier.rs
src/mcp/specs/markdown.rs
src/mcp/specs/list.rs
src/mcp/specs/version.rs
src/mcp/specs/cargo.rs
```

Alternative acceptable shape:

```text
src/tools/math.rs       # implementation + spec declarations
src/tools/text.rs
...
```

The first shape is less disruptive because schemas already live under `src/mcp/schemas/*` and handlers under `src/tools/*`.

### Implementation Steps

1. Add `src/mcp/specs/mod.rs` and category files.
2. Move a small category first, preferably `math` or `version`, from `registry.rs` into `mcp/specs/math.rs` or `mcp/specs/version.rs`.
3. Expose category slices:

   ```rust
   pub const MATH_TOOLS: &[ToolSpec] = &[ ... ];
   ```

4. In `specs/mod.rs`, aggregate by category:

   ```rust
   pub const ALL_TOOL_GROUPS: &[&[ToolSpec]] = &[
       math::MATH_TOOLS,
       text::TEXT_TOOLS,
       json::JSON_TOOLS,
   ];
   ```

5. Because Rust cannot directly concatenate const slices into one static slice without some friction, choose one of these approaches:

   - Keep a `LazyLock<Vec<&'static ToolSpec>>` view for iteration.
   - Use a macro to declare and aggregate tools.
   - Keep `ALL_TOOLS` in registry temporarily but source each category through macros.

6. Preserve the public helpers:

   - `all_tools()`
   - `get_tool()`
   - `tool_names()`
   - `tools_for_profile()`
   - `mcp_tool_definitions()`
   - `input_schema_for()`
   - `output_schema_for()`
   - `tool_handler_for()`
   - `tool_count()`

7. Move remaining categories in batches.
8. Keep exact profile snapshots passing after each batch.

### Acceptance Criteria

- `registry.rs` no longer contains the full `ALL_TOOLS` declaration.
- Category-local spec modules own their tool declarations.
- Root registry APIs remain stable.
- Exact profile snapshot tests pass unchanged.
- Adding a new tool in a category does not require editing a giant central declaration block.

## Priority 2: Finish Machine-Code Enforcement

### Problem

`ToolResponse::error_without_code_for_legacy_tests_only` still permits machine-code-less non-OK responses. It is deprecated and hidden, which is good, but it remains callable from production code.

### Desired End State

Production code should not be able to return a non-OK `ToolResponse` without a machine code. Legacy construction should either be removed or fully quarantined behind `#[cfg(test)]`.

### Implementation Options

Preferred option:

```rust
#[cfg(test)]
pub(crate) fn error_without_code_for_legacy_tests_only(...) -> Self
```

If external tests still require it, keep it public only under `#[cfg(test)]` or move the legacy test to an internal unit test.

Alternative option:

- Keep the function but add a test that scans all non-test source files for `error_without_code_for_legacy_tests_only(` and fails if found outside `response.rs` and tests.

### Implementation Steps

1. Search for all call sites of `error_without_code_for_legacy_tests_only`.
2. Replace production call sites with `error_with_code`.
3. Restrict the legacy constructor to test-only if feasible.
4. Add a source-level guard test:

   - Walk `src/` files.
   - Reject any occurrence of `error_without_code_for_legacy_tests_only(` outside `src/mcp/response.rs`.
   - If test-only constructor remains, allow test modules only.

5. Keep behavior sweep tests that assert non-OK tool responses carry `machine_code`.
6. Update `architecture/machine-codes.md` to say machine-code-less errors are no longer valid outside internal legacy tests.

### Acceptance Criteria

- No production call site can construct a machine-code-less error response.
- Tests fail if the legacy constructor is used in production code.
- All non-OK tool responses in representative tests include `machine_code`.

## Priority 3: Add Current-Audience Listing API

### Problem

`ToolRegistry` stores an audience, but the obvious audience-aware methods require passing an audience explicitly. `available_tools_model_safe()` always uses `ToolAudience::Model`, regardless of the registry's stored audience. This is not wrong, but it is ergonomically incomplete.

### Desired End State

Consumers should be able to construct a registry with profile+audience and then ask for tools for that current audience without repeating the audience.

### Implementation Steps

1. Add:

   ```rust
   pub fn available_tools_for_current_audience(&self) -> Vec<ToolView> {
       self.available_tools_for_audience(self.audience)
   }
   ```

2. Add alias if desired:

   ```rust
   pub fn available_tools_scoped(&self) -> Vec<ToolView>
   ```

3. Update docs/examples:

   ```rust
   let registry = ToolRegistry::with_profile_and_audience(
       Profile::CodeggPreflight,
       ToolAudience::Harness,
   );
   let tools = registry.available_tools_for_current_audience();
   ```

4. Add tests:

   - `current_audience_model_excludes_harness_only`
   - `current_audience_harness_includes_preflight_harness_tools`
   - `current_audience_debug_matches_debug_listing`

### Acceptance Criteria

- Stored audience is directly useful.
- Codegg can construct a registry once and list tools according to that registry's profile/audience.
- Existing APIs remain compatible.

## Priority 4: Audit Preflight Tool Exposure Policy

### Problem

Current exact snapshots place `command_preflight`, `config_preflight`, and `edit_preflight` in `codegg_core_min` model audience. That may be intentional, but it needs an explicit policy decision. Preflight tools can be either:

1. Model-callable advisory tools, or
2. Harness-only enforcement tools.

For codegg, enforcement should usually be harness-driven. Letting the model call advisory preflight tools is useful in some contexts, but it should not replace automatic harness checks.

### Desired End State

Each preflight tool has an explicit exposure decision and documentation:

- `edit_preflight`
- `command_preflight`
- `config_preflight`
- `patch_apply_check`
- `path_scope_check`
- `prompt_input_inspect`
- `unicode_policy_check`
- `text_security_inspect`

Recommended policy:

- `edit_preflight`: `HarnessOnly` by default, optionally `Contextual` only in patch/debug profiles if model advisory use is genuinely useful.
- `command_preflight`: `HarnessOnly` for enforcement; a model-visible advisory command analyzer can exist separately if needed.
- `config_preflight`: may remain model-visible because config validation is frequently explanatory and low-risk.
- `patch_apply_check`: `HarnessOnly`.
- `path_scope_check`: `HarnessOnly`.
- `prompt_input_inspect`: `HarnessOnly`.
- `unicode_policy_check`: `HarnessOnly` or `Contextual` only in security workflows.
- `text_security_inspect`: model-visible contextual tool is acceptable if output is concise and actionable.

### Implementation Steps

1. Add a short `architecture/tool-exposure-policy.md` or update `architecture/mcp-server.md` with the policy.
2. Review each preflight/safety tool's `ToolExposure` and profile membership.
3. Adjust `codegg_core_min` snapshot intentionally if tools move to harness-only.
4. Preserve separate harness snapshots so codegg can still call these automatically.
5. Add tests proving enforcement-critical tools are available in harness audience even if absent from model audience.

### Acceptance Criteria

- Preflight exposure policy is documented.
- Snapshot changes are intentional and reviewed.
- codegg has a clear split between model advisory tools and harness enforcement tools.

## Priority 5: Move `tools/list` Filtering into Registry/List Helpers

### Problem

`server.rs` still owns most `tools/list` filtering: profile, names, tier, tags, compact schema detail, and deprecated-field normalization. This is acceptable now, but it makes the MCP adapter responsible for registry/list semantics.

### Desired End State

`server.rs` should validate MCP parameters and delegate listing to a registry/listing function.

Potential API:

```rust
pub struct ToolListOptions<'a> {
    pub profile: &'a str,
    pub names: Option<Vec<&'a str>>,
    pub tier: Option<u8>,
    pub tags: Option<Vec<&'a str>>,
    pub schema_detail: SchemaDetail,
    pub audience: Option<ToolListAudience>,
}

pub fn list_tool_definitions(options: ToolListOptions<'_>) -> Vec<ToolDefinition>;
```

For MCP compatibility, audience can remain omitted or default to legacy behavior. The in-process API should use explicit audience.

### Implementation Steps

1. Add `SchemaDetail` enum in registry or listing module:

   ```rust
   pub enum SchemaDetail { Compact, Normal, Full }
   ```

2. Add `ToolListOptions`.
3. Move profile/name/tier/tag filtering into `registry::list_tool_definitions(options)`.
4. Move compact-schema handling fully into registry/listing helpers.
5. Keep MCP parameter validation in `server.rs`.
6. Update `server.rs` `tools/list` path to build options and call the registry helper.
7. Add unit tests for list filtering independent of MCP stdio.

### Acceptance Criteria

- `server.rs` no longer manually filters tool definitions.
- Registry/listing tests cover profile/name/tier/tag/schema-detail combinations.
- MCP output remains compatible.

## Priority 6: Add Visible CI or Verification Workflow

### Problem

No GitHub combined status is visible for `main`. Given the size of the refactors, the repo needs an obvious verification signal.

### Desired End State

A GitHub Actions workflow runs at least:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

If project constraints make CI undesirable, the repo should still include a committed verification script and documented handoff command.

### Implementation Steps

1. Add `.github/workflows/ci.yml` unless intentionally avoiding CI.
2. Use stable Rust unless the crate requires nightly.
3. Cache cargo registry/build artifacts if desired, but keep workflow simple initially.
4. Add `cargo test --all-features`.
5. Update `.skills/testing.md` and `AGENTS.md` to reference the CI-equivalent local commands.
6. If CI cannot be enabled, add `scripts/verify.sh` and document it.

### Acceptance Criteria

- PRs/commits have visible fmt/clippy/test status, or an explicit repo-local verification script exists.
- Maintainers know the exact verification command for registry/profile/machine-code changes.

## Priority 7: Revisit Machine-Code Namespace Before codegg Locks In

### Problem

The repo currently uses uppercase snake-case machine codes. That can work, but codegg will soon treat these as stable routing contracts. Before that happens, the names should be audited for specificity and category clarity.

### Desired End State

Machine codes used by codegg are category-prefixed and actionably specific.

Examples:

- `EDIT_SAFE_TO_APPLY`
- `EDIT_OLD_TEXT_NOT_FOUND`
- `EDIT_MULTIPLE_MATCHES`
- `EDIT_STALE_CONTEXT`
- `SHELL_SAFE_COMMAND`
- `SHELL_DESTRUCTIVE_COMMAND`
- `SHELL_NETWORK_ACCESS`
- `CONFIG_VALID`
- `CONFIG_INVALID`
- `UNICODE_BIDI_DETECTED`
- `PATH_SCOPE_ESCAPE`

Generic aliases can remain for compatibility, but codegg-facing docs should prefer the specific forms.

### Implementation Steps

1. Audit `src/mcp/machine_codes.rs` for generic codes.
2. Add category-specific aliases where needed.
3. Migrate critical tools and tests to specific codes first.
4. Keep old constants as deprecated aliases if already documented.
5. Update `architecture/machine-codes.md` and changelog.

### Acceptance Criteria

- codegg-critical machine codes are category-prefixed.
- Existing compatibility is preserved or documented.
- Tests assert expected codes for critical preflight and security cases.

## Suggested Execution Order

1. Add `available_tools_for_current_audience()` and tests. This is small and reduces API confusion.
2. Finish machine-code enforcement or add source guard tests.
3. Audit preflight exposure and update snapshots/docs intentionally.
4. Move `tools/list` filtering into registry/listing helpers.
5. Move `ToolSpec` declarations into category-local spec modules.
6. Add CI or a repo-local verification script.
7. Normalize machine-code namespaces before codegg depends on them heavily.

## Required Verification

Run after each meaningful batch:

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

Additional targeted checks:

```bash
cargo test profile_snapshot
cargo test machine_code
cargo test tool_registry
cargo test preflight
```

## Completion Criteria

This tightening pass is complete when:

- Tool declarations are category-local, not centralized in a huge `registry.rs` block.
- Production code cannot emit machine-code-less non-OK tool responses.
- `ToolRegistry` has a clear current-audience listing method.
- Preflight model/harness exposure policy is documented and reflected in snapshots.
- `tools/list` filtering is tested outside the MCP server adapter.
- CI or a verification script makes fmt/clippy/test status visible.
- codegg-critical machine codes are specific enough to become routing contracts.
