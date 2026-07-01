# Post-Implementation Corrective Plan

## Purpose

The recent implementation moved eggsact substantially in the right direction. The repository now has a consolidated tool registry, split MCP modules, category-level tool modules, a public in-process agent API, typed preflight wrappers, machine-code infrastructure, and profile/exposure enums. That is a significant architectural improvement over the prior monolithic MCP server/tools shape.

This corrective plan focuses on the remaining structural issues that should be addressed before additional feature expansion. The goal is to lock in the new architecture so codegg can safely depend on eggsact as a deterministic local substrate.

## Current State Summary

The repo now contains the right major pieces:

- `src/mcp/registry.rs` defines `ToolSpec`, typed exposure/cost/stability enums, profile helpers, tool lookup, MCP tool definition generation, handler lookup, and audience-aware profile filtering.
- `src/mcp/server.rs` is much smaller and delegates handler lookup/profile/schema validation to `ToolRegistry::prepare_tool_call`.
- `src/mcp/response.rs` owns `ToolResponse`, error sanitization, finding helpers, and machine-code-aware constructors.
- `src/mcp/machine_codes.rs` introduces a machine-code constant set and documentation has been added under `architecture/machine-codes.md`.
- `src/agent/mod.rs` exposes an in-process `ToolRegistry` and typed `Profile` enum.
- `src/preflight/mod.rs` exposes typed edit, command, and config preflight wrappers.
- `src/tools/*` category modules replaced the deleted `src/mcp/tools.rs` implementation monolith.

Remaining issues are now mostly cleanup, enforcement, and boundary-correction issues. They are important because codegg will route behavior on these APIs and contracts.

## Priority 1: Fix Core Dependency Direction

### Problem

`src/mcp/registry.rs` currently imports `ToolDefinition` from `crate::mcp::server`. This reverses the desired dependency direction: the server should depend on registry/protocol types, not the registry depending on the server. The registry is now closer to core infrastructure, while `server.rs` is an MCP stdio adapter.

### Fix

Move `ToolDefinition` out of `server.rs` into a protocol-neutral module. Recommended options:

1. `src/mcp/registry.rs`, if the type is purely a generated view over `ToolSpec`.
2. `src/mcp/protocol.rs`, if the type is considered part of MCP `tools/list` protocol output.
3. `src/mcp/tool_definition.rs`, if keeping it separate improves clarity.

Preferred first pass: move it into `registry.rs` beside the generation function `mcp_tool_definitions()`. Then `server.rs` imports `ToolDefinition` from `registry` only if it still needs the concrete type.

### Implementation Steps

1. Move the `ToolDefinition` struct definition from `server.rs` into `registry.rs` or a new `mcp/tool_definition.rs` module.
2. Update `registry.rs` to stop importing from `crate::mcp::server`.
3. Update `server.rs` imports accordingly.
4. Confirm that `mcp_tool_definitions()` still serializes the same field names: `inputSchema`, `outputSchema`, `tier`, `tags`, `deprecated`, `category`, `llm_exposure`, and `cost`.
5. Remove any unused compatibility `ToolMetadata` type from `server.rs` if it is no longer referenced.

### Acceptance Criteria

- `registry.rs` has no dependency on `server.rs`.
- `server.rs` is an adapter/orchestration module, not the owner of registry view types.
- Existing `tools/list` output remains compatible.
- Tests pass.

## Priority 2: Enforce Machine Codes for Non-OK Responses

### Problem

The new `ToolResponse::error_with_code` constructor is good, but the old `ToolResponse::error` constructor still exists and returns `machine_code: None`. Tests explicitly allow that constructor to produce no machine code. That leaves future regressions possible: a new tool error path can call the old constructor and silently omit machine codes unless a behavior test happens to exercise that exact path.

### Fix

Make it mechanically difficult or impossible for externally returned non-OK tool responses to lack `machine_code`.

There are two acceptable implementation paths.

Path A, strict:

- Remove `ToolResponse::error` or make it private.
- Replace all call sites with `error_with_code`.
- Add a compile-time/no-call-site test pattern by grepping in CI if possible.

Path B, compatibility-preserving:

- Rename the old constructor to `error_without_code_for_legacy_tests_only` or mark it `#[doc(hidden)]` and `#[deprecated]`.
- Keep it only for compatibility tests that intentionally verify legacy behavior.
- Add a test that scans all actual tool handler outputs for representative invalid inputs and asserts non-OK responses have machine codes.
- Add a direct unit test that every call path through common helper validation uses `error_with_code`.

Preferred path: Path A if all tests can be migrated cleanly; otherwise Path B as a temporary bridge with an explicit TODO and deprecation.

### Implementation Steps

1. Search all `ToolResponse::error(` call sites.
2. Replace each with `ToolResponse::error_with_code(...)` and the closest machine code.
3. Add new common machine codes where current cases do not fit.
4. Update `tests/mcp/test_machine_codes.rs` to remove the assertion that old `ToolResponse::error` has no machine code, unless the old constructor remains intentionally hidden/deprecated.
5. Add an invariant test: for a broad table of bad inputs, any `ok == false` tool response must contain a non-null `machine_code` string.
6. Add a lighter direct-unit test for helper-level argument validation errors if those do not travel through MCP.

### Acceptance Criteria

- No ordinary tool implementation should call a machine-code-less error constructor.
- Every non-OK MCP tool response in the test suite has `machine_code`.
- Machine-code-less error construction is impossible or explicitly marked legacy/internal.
- Tests pass.

## Priority 3: Decide and Normalize Machine-Code Naming

### Problem

The roadmap proposed dotted namespaced codes, but the implementation uses uppercase snake-case constants and tests require uppercase strings. Uppercase strings are acceptable, but the current namespace density is inconsistent. Examples like `EDIT_OK`, `COMMAND_OK`, and `TEXT_SECURITY_OK` are usable; generic names such as `INVALID_ARGUMENTS` and `EDIT_FAILED` may become too coarse for codegg routing.

### Fix

Keep uppercase snake case if desired, but make it consistently category-prefixed and semantically specific before codegg bakes these values into routing logic.

Recommended convention:

- Common: `COMMON_INVALID_ARGUMENTS`, `COMMON_INPUT_TOO_LARGE`, `COMMON_TIMEOUT`, `COMMON_OUTPUT_TOO_LARGE`, `COMMON_CANCELLED`.
- Edit: `EDIT_SAFE_TO_APPLY`, `EDIT_SAFE_WITH_WARNINGS`, `EDIT_OLD_TEXT_NOT_FOUND`, `EDIT_MULTIPLE_MATCHES`, `EDIT_STALE_CONTEXT`, `EDIT_PATCH_PARSE_ERROR`.
- Shell: `SHELL_SAFE_COMMAND`, `SHELL_NEEDS_CONFIRMATION`, `SHELL_DESTRUCTIVE_COMMAND`, `SHELL_NETWORK_ACCESS`, `SHELL_PACKAGE_INSTALL`, `SHELL_PRIVILEGE_ESCALATION`.
- Config: `CONFIG_VALID`, `CONFIG_INVALID`, `CONFIG_FORMAT_UNKNOWN`, `CONFIG_SCHEMA_FAILED`.
- Unicode: `UNICODE_INVISIBLE_DETECTED`, `UNICODE_BIDI_DETECTED`, `UNICODE_CONFUSABLE_DETECTED`, `UNICODE_POLICY_VIOLATION`.
- JSON: `JSON_VALID`, `JSON_INVALID`, `JSON_EQUAL`, `JSON_NOT_EQUAL`, `JSON_DUPLICATE_KEY`.
- Path: `PATH_WITHIN_SCOPE`, `PATH_SCOPE_ESCAPE`, `PATH_TRAVERSAL_DETECTED`, `PATH_PLATFORM_AMBIGUOUS`.

If renaming everything is too disruptive, add aliases first and migrate call sites incrementally. Avoid changing already-documented external codes without an intentional compatibility note in `CHANGELOG.md`.

### Implementation Steps

1. Audit `src/mcp/machine_codes.rs` for generic or ambiguous codes.
2. Add category-prefixed constants for critical codegg workflows.
3. Migrate edit, shell, config, path, Unicode, and JSON tools first.
4. Keep backward-compatible aliases for old names if tests or docs depend on them.
5. Update `architecture/machine-codes.md`.
6. Add tests enforcing category prefixes for non-common workflow codes.

### Acceptance Criteria

- Codegg-critical machine codes are category-prefixed and specific.
- Docs match constants.
- Compatibility aliases are documented if retained.
- Tests pass.

## Priority 4: Add Audience-Aware ToolRegistry APIs

### Problem

Audience filtering exists in `registry::tools_for_profile_audience`, but `ToolRegistry::available_tools()` only filters hidden tools. That means model-facing codegg integrations can accidentally list harness-only tools if they call the most obvious public API.

### Fix

Make audience explicit in the public in-process API and guide codegg toward the safe path.

Recommended additions:

```rust
pub enum ToolAudience {
    Model,
    Harness,
    Debug,
}

impl ToolRegistry {
    pub fn with_profile_and_audience(profile: Profile, audience: ToolAudience) -> Self;
    pub fn audience(&self) -> ToolAudience;
    pub fn available_tools_for_audience(&self, audience: ToolAudience) -> Vec<ToolView>;
    pub fn available_tools_model_safe(&self) -> Vec<ToolView>;
}
```

Either store audience inside `ToolRegistry`, or make the audience-specific method the recommended listing API. The important part is that codegg has a clear, ergonomic way to request model-safe tools.

### Implementation Steps

1. Re-export or wrap `registry::ToolListAudience` in `agent` as a public `ToolAudience` type.
2. Add `ToolRegistry::available_tools_for_audience` and `ToolRegistry::available_tools_model_safe`.
3. Update docs and examples to use model-safe listing for model-facing usage.
4. Decide whether `ToolRegistry::available_tools()` should remain legacy/full-profile behavior or default to `Model` audience. If changing behavior risks surprises, keep it but document that it is not model-safe.
5. Add tests verifying harness-only tools do not appear in `available_tools_model_safe()`.

### Acceptance Criteria

- Public API has an obvious model-safe listing path.
- Harness-only tools are excluded from model-safe listing tests.
- Harness audience includes expected preflight tools.
- Existing behavior is preserved or changes are documented.

## Priority 5: Strict Profile Parsing and Profile Validation

### Problem

`Profile::from_str_opt` is documented as returning `None` for unknown names, but currently returns `Some(Profile::Custom(...))` for any unknown string. This makes typos look valid and can lead to empty or surprising tool sets. For codegg, strict profile parsing is safer.

### Fix

Separate strict parsing from custom construction.

Recommended API:

```rust
impl Profile {
    pub fn from_known_str(name: &str) -> Option<Self>;
    pub fn custom(name: impl Into<String>) -> Self;
}
```

Then either:

- Change `from_str_opt` to be strict and return `None` for unknown names, or
- Rename current behavior to `from_str_or_custom` and update docs.

Preferred behavior: make `from_str_opt` strict, add `custom()` for explicit custom profiles, and update tests accordingly.

### Implementation Steps

1. Add `Profile::from_known_str` or change `from_str_opt` directly.
2. Add `Profile::custom` constructor.
3. Update docs to state the exact behavior.
4. Update tests that currently assert unknown profiles are accepted.
5. Ensure `ToolRegistry::with_profile(Profile::Custom(...))` validates profile names before listing/calling, returning clear errors for unknown profiles if no tools exist.

### Acceptance Criteria

- Unknown profile strings do not silently parse as valid known profiles.
- Custom profiles are explicit.
- Tests cover strict parsing and custom construction separately.
- Docs match implementation.

## Priority 6: Decompose `registry.rs` Before It Becomes the New Monolith

### Problem

The previous MCP/tools monolith was removed, but `src/mcp/registry.rs` is now very large and contains many schema builder functions plus registry declarations. This is better than four independent tables, but it risks becoming the next central maintenance hotspot.

### Fix

Move tool specs and schemas into category modules while keeping one root registry aggregation point.

Recommended shape:

```text
src/mcp/registry/
  mod.rs
  types.rs
  profiles.rs
  listing.rs
  specs/
    mod.rs
    math.rs
    text.rs
    json.rs
    regex.rs
    path.rs
    shell.rs
    patch.rs
    config.rs
    unicode.rs
    identifier.rs
    markdown.rs
    list.rs
    version.rs
    cargo.rs
```

Alternative: place specs beside implementation modules under `src/tools/*` and have each module export `pub const SPECS: &[ToolSpec]` or `pub fn specs() -> Vec<ToolSpec>`. This keeps implementation, schema, and spec together by category.

Preferred approach for maintainability: category modules own their specs, input schemas, output schemas, and handlers; root registry only aggregates.

### Implementation Steps

1. Create registry submodules for types/profile/listing first.
2. Move `ToolSpec`, enums, and `ToolListAudience` into `registry/types.rs` or equivalent.
3. Move profile constants and profile validation into `registry/profiles.rs`.
4. Move listing/filtering helpers into `registry/listing.rs`.
5. Move one low-risk category first, for example math or version, into category specs.
6. Confirm no behavior change with tests.
7. Move remaining categories in small batches.
8. Keep root `registry::all_tools()` API stable.

### Acceptance Criteria

- `registry.rs` is no longer a multi-thousand-line file.
- Root registry aggregation remains the single source of truth.
- Tool declarations are category-local.
- Adding a new tool requires editing the relevant category module, not a central monolith.
- Tests pass after each batch.

## Priority 7: Replace Weak Profile Snapshot Tests with Exact Snapshots

### Problem

Current profile tests mostly assert non-empty lists and absence of harness-only tools in model-facing lists. That is useful but does not catch accidental profile churn. codegg will depend on specific profiles being stable.

### Fix

Add exact sorted snapshots for critical profiles. These can be inline `assert_eq!` arrays initially, or external fixture files if preferred.

Critical snapshots:

- `codegg_core_min` model audience.
- `codegg_core` model audience.
- `codegg_preflight` harness audience.
- `codegg_patch` model and harness audiences.
- `codegg_config` model and harness audiences.
- `codegg_shell` model and harness audiences.
- `codegg_unicode_security` model and harness audiences.
- `codegg_repo_audit` model audience.

Each snapshot should include tool names and optionally exposure values.

### Implementation Steps

1. Add a helper to return sorted `(tool_name, exposure)` pairs for profile+audience.
2. Write exact expected arrays for the critical profiles.
3. Keep tests clear that updates are intentional and require review.
4. Add a short doc note explaining when profile snapshot changes are acceptable.

### Acceptance Criteria

- Accidental profile additions/removals fail tests.
- Harness-only leakage into model profiles is caught.
- codegg-facing profile contract is explicit.

## Priority 8: Move Remaining Server Generic Helpers Out of `server.rs`

### Problem

`server.rs` is smaller, but still owns generic helpers that do not need to live in the server adapter:

- compact input schema conversion
- compact output schema conversion
- Python-style JSON serialization for MCP text payloads
- tool response wrapping
- close-match lookup
- JSON-RPC error constructors
- possibly legacy `ToolMetadata`

### Fix

Move these into appropriate modules:

- schema compaction -> `mcp/schema_compaction.rs` or `registry/listing.rs`
- MCP response wrapping / Python-style JSON text payload -> `mcp/response.rs` or `mcp/protocol.rs`
- close-match lookup -> `mcp/registry.rs` or a small `mcp/suggestions.rs`
- JSON-RPC error constructors -> `mcp/protocol.rs`

### Implementation Steps

1. Move JSON-RPC error constructors first; low risk.
2. Move MCP wrapper serialization into `response.rs` and keep output exact.
3. Move schema compaction into a dedicated function used by `tools/list`.
4. Move close-match helper to registry/suggestions.
5. Remove unused legacy structs.

### Acceptance Criteria

- `server.rs` mostly contains request validation, method dispatch, and stdio loop.
- No generic schema or response transformation logic is stranded in `server.rs`.
- MCP output remains byte-compatible where tests assert it.

## Priority 9: Clarify MCP Serial Execution Semantics

### Problem

The MCP stdio loop still spawns request handling and immediately awaits it, so the server remains effectively serial at the read-loop level. This may be acceptable, especially now that codegg can use the in-process API for high-throughput calls, but the runtime worker/semaphore code can make the behavior look more concurrent than it is.

### Fix

Document the current semantics explicitly and add a test or comment preventing future confusion. Defer true concurrent MCP unless codegg actually needs it.

### Implementation Steps

1. Update `architecture/mcp-server.md` to state that stdio request handling is serialized by the read loop.
2. Clarify that `MAX_TOOL_WORKERS` limits blocking work within the current architecture but does not imply fully concurrent MCP request reads.
3. Add a TODO or future phase note for out-of-order JSON-RPC response support if true concurrency is needed.
4. Ensure codegg docs recommend in-process API for frequent harness calls.

### Acceptance Criteria

- Runtime semantics are documented accurately.
- No one expects MCP stdio to be high-throughput concurrent until intentionally changed.
- codegg integration guidance points to the in-process API for repeated preflight calls.

## Priority 10: Add Local/CI Verification Signals

### Problem

No remote commit status was visible during review. The repository may already pass locally, but there is no visible CI signal from the connector. Given the size of the refactor, automated verification is important.

### Fix

Ensure the repo has a GitHub Actions workflow or documented local verification command for the new architecture.

Minimum recommended workflow:

```yaml
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

If CI is intentionally not enabled, add a `docs/testing.md` or `.skills/testing.md` section that lists the required local verification commands.

### Acceptance Criteria

- A maintainer can quickly determine whether the refactor passes formatting, clippy, and tests.
- Future commits touching registry/tool contracts are verified automatically or by documented handoff command.

## Suggested Execution Order

1. Dependency direction fix: move `ToolDefinition` out of `server.rs`.
2. Machine-code enforcement: replace or quarantine `ToolResponse::error`.
3. Audience-aware public API: add model-safe `ToolRegistry` listing methods.
4. Strict profile parsing: separate strict parse from explicit custom profiles.
5. Exact profile snapshots: lock codegg-facing profile contracts.
6. Server cleanup: move schema compaction, response wrapping, suggestions, and JSON-RPC errors out of `server.rs`.
7. Registry decomposition: move specs/schemas by category.
8. Machine-code taxonomy normalization: category-prefix critical codes.
9. MCP serial semantics documentation.
10. CI/local verification signal.

This order fixes correctness and API-contract risks before larger mechanical decomposition.

## Testing Requirements

For this corrective pass, require at minimum:

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

Add or update targeted tests for:

- registry no longer imports from server
- all non-OK tool responses include machine code
- model-safe public listing excludes harness-only tools
- harness audience includes expected preflight tools
- strict profile parsing rejects unknown names
- exact codegg profile snapshots
- MCP `tools/list` output still serializes expected fields
- preflight wrappers still parse expected outputs

## Completion Criteria

This corrective plan is complete when:

- The core registry no longer depends on the MCP server adapter.
- Machine-code-less non-OK tool responses are impossible or quarantined as deprecated legacy behavior.
- Public codegg-facing APIs expose model-safe and harness audiences explicitly.
- Profile parsing is strict unless custom profiles are explicitly constructed.
- codegg-critical profile contents are protected by exact snapshots.
- `registry.rs` is decomposed or at least clearly staged for category-local specs.
- `server.rs` contains only server/protocol orchestration concerns.
- MCP serial behavior is documented honestly.
- Formatting, clippy, and full tests pass.
