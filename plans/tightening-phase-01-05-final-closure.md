# Final Closure Plan for Tightening Phases 01–05

## Purpose

This plan closes the remaining small gaps after the latest phase 01–05 tightening implementation pass. The repo is now in strong shape: profile/audience dispatch exists, compatibility docs are aligned with bool rejection, typed preflight wrappers parse structured next-tool hints and strict findings, route-critical tool classification exists, generated docs/tool cards exist, and CI includes a generated-docs check.

This plan should be treated as a final closure pass before declaring phases 01–05 complete and moving fully into phase 06 work.

## Current State

The following work appears materially implemented:

- Dispatch-time profile and audience checks in `ToolRegistry::prepare_tool_call`.
- MCP `tools/call` using the active profile and model audience rather than an unconditional full-profile registry.
- `CompatibilityMode` documentation corrected so `EggcalcPython` keeps Python-style type names but still rejects JSON booleans for numeric fields.
- Strict typed preflight wrappers with `PreflightError::{ToolCall, ToolRejected, ContractViolation}`.
- Structured `RecommendedNextTool` parsing for legacy string and object-shaped `recommended_next_tool` values.
- Strict preflight finding parsing for edit, command, and config wrappers.
- Route-critical tool classification through `ROUTE_CRITICAL_TOOLS` and `is_route_critical`.
- Route-contract tests for route-critical tools and simple utility tools.
- `generate-docs` binary, generated tool-card file, generated README/profile blocks, and CI check mode.
- Early phase 06 edit preflight composition work, including path scope, newline, Unicode, and fingerprint outputs.

The remaining closure items are narrow and should be handled before new feature expansion.

## Closure Item 1: Add MCP Active-Profile `tools/call` Regression Test

### Problem

In-process profile enforcement is now tested through `ToolRegistry::with_profile_and_audience(Profile::CodeggCoreMin, ToolAudience::Model)`. That proves the registry boundary, but it does not prove the MCP server’s active-profile `tools/call` path. The original bug was MCP-specific: `tools/call` could construct a default full-profile registry while `tools/list` respected a restricted active profile.

### Required Change

Add an integration test that spawns the `eggsact --mcp` binary with a restricted active profile environment variable.

Recommended test name:

```rust
fn test_mcp_tools_call_honors_active_profile_env()
```

Recommended helper:

```rust
fn mcp_request_with_env(request: &str, envs: &[(&str, &str)]) -> String
```

The helper should mirror the existing `mcp_request` helper but set environment variables on `Command` before spawning.

### Test Flow

1. Spawn MCP with the supported active profile env var set to `codegg_core_min`.
2. Send `tools/list` and verify the response contains `codegg_core_min` tools.
3. Choose a known valid full-profile tool that is not in `codegg_core_min`.
4. Call `tools/call` for that tool with valid minimal arguments.
5. Assert a JSON-RPC error, preferably `-32602`, with text indicating profile unavailability.

Use a deterministic known out-of-profile tool rather than the first arbitrary excluded tool. Avoid `{}` arguments unless the selected tool genuinely accepts `{}`. The failure must prove profile rejection, not schema rejection.

Candidate tool selection strategy:

- Use registry helpers in the test to find a tool present in full/model but absent in `codegg_core_min` and then provide valid arguments from a small hardcoded match table.
- Or select a known stable tool with simple valid args that is outside `codegg_core_min`, such as a math/list/regex utility if confirmed absent.

### Acceptance Criteria

- The test fails if MCP `tools/call` regresses to `ToolRegistry::default()`.
- The test failure mode cannot be confused with invalid arguments.
- The test documents that MCP `tools/call` uses active profile only unless per-call profile override is explicitly implemented later.

## Closure Item 2: Filter Hidden Tools Out of Generated README and Tool Cards

### Problem

The generated README table currently builds from `all_tools_vec()` without clearly filtering hidden tools. Existing generator tests assert all non-hidden tools are present, but they do not assert hidden tools are absent. If hidden tools exist now or are added later, generated public docs could accidentally expose them.

### Required Change

Update `generate_readme_tools()` to filter out `ToolExposure::Hidden` before grouping tools by category.

Update `generate_tool_cards()` to ensure profile/audience listing cannot include hidden tools. It currently uses `tools_for_profile_audience(profile, ToolListAudience::Model)`, which should already exclude hidden for normal profiles, but add an explicit defensive filter anyway.

Recommended pattern:

```rust
let visible_tools: Vec<&ToolSpec> = all_tools_vec()
    .into_iter()
    .filter(|spec| spec.exposure != ToolExposure::Hidden)
    .collect();
```

### Required Tests

Add tests to `src/bin/generate_docs.rs` test module:

```rust
fn generated_readme_excludes_hidden_tools()
fn generated_tool_cards_exclude_hidden_tools()
```

The test should iterate registered hidden tools and assert their backtick names do not appear in the README tool table or generated cards.

If there are no hidden tools currently, keep the test but make it robust:

- If hidden tools exist, assert absence.
- If no hidden tools exist, assert the test setup still passes without panic and add a comment explaining it guards future additions.

### Acceptance Criteria

- Public generated docs never include hidden tools.
- Tests explicitly check absence of hidden tools.
- Generated docs are regenerated and `cargo run --bin generate-docs -- --check` passes.

## Closure Item 3: Enrich Generated Profile Reference With Harness Counts

### Problem

The generated profile reference currently reports only model-audience tools and tool names. For codegg, harness visibility is a central concept. The profile reference should make the distinction explicit so handoff agents can quickly understand what ordinary model sessions see versus what the harness can execute.

### Required Change

Update `generate_profile_reference()` to include at least these columns:

```markdown
| Profile | Model Tools | Harness Tools | Model Tool Names | Harness-Only Tools |
```

Recommended content:

- `Model Tools`: count from `tools_for_profile_audience(profile, ToolListAudience::Model)`.
- `Harness Tools`: count from `tools_for_profile_audience(profile, ToolListAudience::Harness)`.
- `Model Tool Names`: names from model-audience listing.
- `Harness-Only Tools`: names present in harness-audience listing but not model-audience listing.

If the table becomes too wide for README-style display, keep it in `architecture/mcp-server.md` only and make README point to it.

### Required Tests

Update `profile_counts_match_registry()` to verify both model and harness counts.

Add a test that, for profiles containing harness-only tools, the generated profile reference includes at least one harness-only tool name in the `Harness-Only Tools` column.

### Acceptance Criteria

- Generated profile reference distinguishes model and harness visibility.
- Counts match registry helper output.
- Harness-only tools are visible in architecture docs, not model-facing README/tool cards unless intended.

## Closure Item 4: Clarify Generator Scope in Docs

### Problem

Generated docs now exist, but the repo should clearly say which files are generated, what command updates them, and what CI expects. This prevents future manual edits from fighting the generator.

### Required Change

Update contributor-facing docs in one or more of:

- `README.md`
- `AGENTS.md`
- `.skills/release.md`
- `.skills/testing.md`

Add a concise section:

```markdown
## Generated Documentation

Registry-derived sections are generated by:

cargo run --bin generate-docs

CI checks staleness with:

cargo run --bin generate-docs -- --check

Do not manually edit content between generated markers.
```

Also mention the generated files:

- `README.md` generated tool block.
- `architecture/mcp-server.md` generated profile block.
- `generated/tool-cards.md`.

### Acceptance Criteria

- A contributor can discover the generation command without reading CI YAML.
- Generated marker policy is documented.
- Release/testing docs mention the check mode.

## Closure Item 5: Verify Route-Critical Classification Is Exported and Documented

### Problem

`ROUTE_CRITICAL_TOOLS` and `is_route_critical` exist in registry listing. They should be reachable from the public paths used by tests and future codegg integration, and architecture docs should define what route-critical means.

### Required Change

Confirm the exports are accessible through `eggsact::mcp::registry::{is_route_critical, ROUTE_CRITICAL_TOOLS}`. If the current re-export chain is fragile, explicitly re-export from `src/mcp/registry/mod.rs`.

Update architecture docs with a compact response-classification section:

- Simple utility tools: deterministic data, no verdict required on success.
- Inspection tools: may emit findings; machine code required when findings are actionable.
- Route-critical tools: must emit success machine code, verdict, summary, and strict findings when present.

### Required Tests

Keep or add tests that:

- Every route-critical tool name exists in the registry.
- Route-critical successful responses include envelope `machine_code`.
- Route-critical result objects include `verdict` and `summary` where applicable.
- Simple tools are not forced into verdicts.

### Acceptance Criteria

- Route-critical classification is documented and exported.
- Tests enforce the boundary.
- Simple utility tools are not over-constrained.

## Closure Item 6: CI Status and Local Verification Notes

### Problem

GitHub combined status may not show workflow state for direct pushes in some connector contexts. The repository should rely on CI YAML plus explicit local commands for handoff verification.

### Required Change

Add a short verification block to the closure commit notes or docs listing:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo run --bin generate-docs -- --check
cargo package --verbose
```

If the implementation agent cannot run all commands locally, they must note which commands were not run and why.

### Acceptance Criteria

- CI and local verification expectations match.
- Handoff agents know the exact closure gate.

## Recommended Implementation Order

1. Add the MCP active-profile regression test.
2. Filter hidden tools from generated README/tool cards and add hidden-exclusion tests.
3. Enrich generated profile reference with harness counts and harness-only names.
4. Regenerate docs and cards.
5. Update generator/contributor docs.
6. Confirm route-critical exports and architecture wording.
7. Run full verification commands.

This order avoids regenerating docs twice and ensures the most important regression test lands first.

## Final Acceptance Criteria

Phases 01–05 can be considered closed when all of the following are true:

- MCP active-profile `tools/call` has a regression test that would catch the original default-full-registry bug.
- Generated README and tool cards exclude hidden tools, with tests proving exclusion.
- Generated profile reference includes model and harness visibility counts.
- Generated-doc workflow and marker policy are documented for contributors and agents.
- Route-critical classification is exported, documented, and tested.
- `cargo fmt`, `cargo clippy`, `cargo test`, `cargo run --bin generate-docs -- --check`, and `cargo package` pass.

## Non-Goals

Do not expand phase 06 edit preflight behavior in this closure pass.

Do not add new tool categories.

Do not redesign profile semantics.

Do not require simple utility tools to emit verdicts.

Do not implement external MCP authorization or a harness-only MCP capability system.
