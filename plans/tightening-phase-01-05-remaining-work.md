# Remaining Tightening Work Through Phase 05

## Purpose

This handoff plan closes the remaining work from the first five tightening phases after the recent implementation pass. The repo is in substantially better shape: profile/audience checks are now in dispatch, the compatibility-mode seam exists, typed preflight wrappers fail closed on key fields, and response helpers now support verdicts, structured findings, machine codes, and structured next-tool hints.

The remaining work is not a restart. It is a corrective closeout pass to align tests, documentation, wrappers, and generated-doc scaffolding with the implementation that has already landed.

## Current State Summary

The recent commits addressed the main architecture gaps:

- `ToolRegistry::prepare_tool_call` now checks profile membership and audience/exposure compatibility before argument validation and handler execution.
- MCP `tools/call` now constructs a profile-aware registry from the active profile instead of using a default full-profile registry.
- `CompatibilityMode::{EggcalcPython, StrictNative}` exists, and the in-process registry defaults to strict native behavior while MCP opts into compatibility mode.
- `PreflightError` now distinguishes registry call errors, tool-level rejections, and typed contract violations.
- Edit, command, and config preflight wrappers now require route-critical fields rather than silently defaulting them.
- `ToolResponse` has helpers for structured findings, verdicts, machine codes, and structured `recommended_next_tool` objects.
- `edit_preflight`, `patch_apply_check`, and `config_preflight` now emit verdicts and machine codes.

The remaining issues are narrower:

- Some profile enforcement tests do not prove the intended behavior because they pass a `profile` parameter to `tools/call`, while the server currently uses the active runtime profile instead of a per-call profile parameter.
- `CompatibilityMode::EggcalcPython` documentation says bool-as-int coercion is preserved, but the validator and hardening tests reject bool numeric inputs.
- Typed preflight parsing still treats findings permissively and does not fail on malformed route-critical findings.
- `recommended_next_tool` is now structured in tool responses, but the typed `EditPreflight` wrapper still parses only string-shaped next-tool values.
- Some route-adjacent tools still lack clear route-contract classification, so machine-code/verdict coverage expectations are ambiguous.
- Phase 05 generated docs/tool-card work has not materially landed; docs were updated manually, but there is no generator or CI staleness check yet.

## Workstream A: Correct Phase 01 Test Coverage for Profile and Audience Dispatch

### Goal

Make the tests prove the actual dispatch behavior: active-profile MCP calls and in-process registry calls must enforce the same boundaries as listing.

### Required Changes

1. Fix the MCP restricted-profile regression test.

   The current test shape that passes `"profile": "codegg_core_min"` inside `tools/call` params is misleading if the server does not support per-call profile override. Replace it with one of these designs:

   Preferred design:

   - Spawn the MCP binary with `EGGCALC_MCP_PROFILE=codegg_core_min` or the current supported profile env var.
   - Call `tools/list` and record the restricted set.
   - Pick a valid full-profile tool that is not in `codegg_core_min`.
   - Call `tools/call` for that tool with valid minimal arguments.
   - Assert JSON-RPC error `-32602` or the expected profile-unavailable error.

   Alternative design:

   - Keep MCP testing focused on active profile.
   - Add a separate in-process `ToolRegistry::with_profile(Profile::CodeggCoreMin)` test for direct restricted dispatch.

2. Add a harness-only exposure test that uses a tool known to be present in a harness profile and marked `HarnessOnly`.

   Test matrix:

   - Model audience rejects it with `ToolNotAllowedForAudience`.
   - Harness audience accepts it if arguments are valid.
   - Listing and dispatch agree for model audience.

3. Add an explicit assertion that `ToolRegistry::available_tools_for_current_audience()` and `prepare_tool_call()` agree for model and harness audiences.

4. If per-call `profile` support is intended for MCP `tools/call`, implement it explicitly and validate it like `tools/list`. If it is not intended, update docs and tests to state that `tools/call` uses the active profile only.

### Acceptance Criteria

- Tests fail if MCP `tools/call` regresses to `ToolRegistry::default()`.
- Tests use valid tool arguments so failures prove profile/audience rejection, not schema rejection.
- The behavior around per-call profile override is either implemented and documented or explicitly absent and documented.

## Workstream B: Resolve CompatibilityMode Documentation and Validator Semantics

### Goal

Make the compatibility contract true. Either `EggcalcPython` permits bool-as-int where intended, or docs must say bool-as-number is intentionally rejected despite Python-style type names.

### Required Changes

1. Decide the intended behavior.

   Recommended decision: keep bool numeric rejection in both MCP and strict native paths. This is safer for agent/harness use and matches current hardening tests.

2. If keeping bool rejection, update:

   - `src/mcp/compat.rs` docs.
   - `architecture/compatibility.md`.
   - Any README/API doc references that claim bool-as-int compatibility.
   - Phase or implementation comments that describe bool-as-int coercion.

   New wording should be precise:

   - `EggcalcPython` preserves Python-style type names in error messages and selected legacy formatting.
   - It does not allow JSON booleans for numeric schema fields because MCP/JSON booleans are commonly model mistakes and should remain rejected.

3. If allowing bool-as-int instead, update validator logic and tests consistently. This path is not recommended.

4. Add compatibility tests:

   - `json_type_name` differs by mode.
   - Bool numeric input is rejected in strict native mode.
   - Bool numeric input behavior in MCP compatibility mode matches the chosen documented behavior.
   - Error wording uses the selected mode's type names.

### Acceptance Criteria

- `CompatibilityMode` docs match `schema_validation.rs` behavior.
- Hardening tests and compatibility tests no longer imply contradictory behavior.
- In-process strict mode remains the codegg default.

## Workstream C: Finish Fail-Closed Typed Wrapper Parsing

### Goal

Complete Phase 03 by making typed wrappers fail closed not only on missing top-level route fields, but also on malformed route-critical nested fields.

### Required Changes

1. Add a typed `RecommendedNextTool` struct:

   ```rust
   pub struct RecommendedNextTool {
       pub name: String,
       pub reason: Option<String>,
       pub arguments_hint: Option<Value>,
   }
   ```

2. Update `EditPreflightOutput` to use:

   ```rust
   pub recommended_next_tool: Option<RecommendedNextTool>
   ```

   If public API compatibility requires keeping the existing string field, add a new field and deprecate the string field, but prefer moving to the structured field before codegg depends on it.

3. Parse both supported shapes during a transition period:

   - String: `"text_diff_explain"` becomes `{ name: "text_diff_explain", reason: None, arguments_hint: None }`.
   - Object: requires a string `name`, optional string `reason`, optional `arguments_hint` value.

   Malformed objects should return `PreflightError::ContractViolation`.

4. Make finding parsing strict for typed preflight wrappers.

   Keep permissive `Finding::from_value` if useful for generic display, but add `Finding::try_from_value_strict` or equivalent. Strict parsing should require:

   - `code` string.
   - `severity` string.
   - `message` string.
   - optional `disposition` string if present.
   - optional `location`, `span`, and `details` values.

5. Update edit, command, and config wrappers to use strict finding parsing for route-critical wrappers.

6. Add tests:

   - Structured `recommended_next_tool` parses correctly.
   - Legacy string next-tool parses during transition.
   - Malformed next-tool object fails closed.
   - Finding missing `code` fails closed.
   - Finding missing `severity` fails closed.
   - Finding missing `message` fails closed.
   - Finding with unknown severity parses as `Other` only if the field is present and string-shaped.

### Acceptance Criteria

- Typed preflight wrappers can consume the structured `ToolResponse::next_tool(...)` shape currently emitted by `edit_preflight`.
- Route-critical findings do not silently lose malformed entries through `filter_map`.
- Contract-violation tests cover malformed next-tool and finding shapes.

## Workstream D: Complete Route-Contract Classification Through Phase 04

### Goal

Clarify which tools must emit verdicts/machine codes and which tools may remain simple deterministic utilities. This avoids ambiguous machine-code coverage expectations.

### Required Changes

1. Define tool response classes in docs and tests:

   - Simple utility tools: deterministic data return; machine code optional on success, mandatory on non-OK.
   - Inspection tools: may return findings; should include structured findings and machine code when findings are actionable.
   - Route-critical preflight/composite tools: must include top-level verdict, machine code, summary, and structured findings when present.

2. Create a static list or registry metadata field for route-critical tools.

   Initial route-critical set:

   - `edit_preflight`
   - `command_preflight`
   - `config_preflight`
   - `patch_apply_check`
   - `text_security_inspect` if used for ingress routing
   - `cargo_toml_inspect` if used by config/repo audit routing

3. Add tests that apply only to route-critical tools:

   - successful response includes envelope `machine_code`.
   - result includes `verdict` or documented domain verdict.
   - result includes `summary` where applicable.
   - findings, if present, use strict shape.
   - `recommended_next_tool`, if present, is structured or legacy-string-compatible.

4. Do not force every simple tool to emit verdicts. Instead, explicitly test that all non-OK tool responses use `error_with_code` or equivalent machine-code-bearing constructor.

5. Review current tools touched in the implementation:

   - `patch_summary` currently uses `PATCH_FAILED` for binary/rename review conditions. Consider adding more precise machine codes later, but do not block this closeout on taxonomy expansion.
   - `dotenv_validate`, `ini_validate`, and `toml_shape` can remain simple utility tools unless promoted to route-critical workflows.
   - `cargo_toml_inspect` should be checked for consistent top-level finding and machine-code behavior because `config_preflight` wraps it.

### Acceptance Criteria

- Tests distinguish route-critical tools from simple utility tools.
- Route-critical tools have consistent route fields.
- Simple tools are not forced into artificial verdicts.
- Non-OK responses carry machine codes.

## Workstream E: Land Phase 05 Generated Docs and Tool Cards Scaffolding

### Goal

Move from manually updated docs to registry-derived generated surfaces. This does not need to generate every doc page in the first pass. It must establish the generator, generated-block pattern, and CI check mode.

### Required Changes

1. Add a generator command.

   Recommended minimal shape:

   ```text
   src/bin/generate_docs.rs
   ```

   Supported commands:

   ```bash
   cargo run --bin generate_docs
   cargo run --bin generate_docs -- --check
   ```

2. Add generated block replacement support.

   Use markers such as:

   ```markdown
   <!-- BEGIN GENERATED: eggsact tools -->
   <!-- END GENERATED: eggsact tools -->
   ```

   The generator should replace only marked blocks. If a target file lacks markers, check mode should fail with a clear message.

3. Generate the initial README tool table block.

   Include:

   - category
   - tool name
   - tier
   - exposure
   - stability
   - short description

4. Generate a profile reference block in `architecture/mcp-server.md` or a new `architecture/profiles.md`.

   Include:

   - profile name
   - model-audience count
   - harness-audience count
   - debug/non-hidden count if useful
   - intended consumer summary from a small static metadata table

5. Generate compact codegg tool cards.

   Recommended initial file:

   ```text
   .skills/codegg-tool-cards.md
   ```

   Or, if `.skills/mcp-tools.md` is already the intended injection surface, add generated sections there.

   Each card should include:

   - tool name
   - category/tier/exposure/cost
   - when to use
   - when not to use
   - required arguments from input schema
   - route fields for route-critical tools
   - common machine codes if available

   If `ToolSpec` lacks `when_to_use` and `when_not_to_use`, keep those fields generic in the first pass and add a follow-up plan to enrich registry metadata.

6. Add a generated docs CI check.

   Update `.github/workflows/ci.yml` with a check step after build/test:

   ```bash
   cargo run --bin generate_docs -- --check
   ```

   If adding the CI step is too invasive for this pass, add a test named `generated_docs_are_current` that performs the same check.

7. Add generator tests.

   - Tool table contains all non-hidden registered tools.
   - Profile counts match registry helper outputs.
   - Tool cards reference only known tools.
   - Check mode reports stale generated docs.

### Acceptance Criteria

- At least one README generated block is maintained by the generator.
- At least one profile reference block is maintained by the generator.
- A compact codegg tool-card surface exists and is generated from registry metadata.
- CI or tests fail when generated content is stale.
- Manual tool tables are either removed, marked generated, or clearly documented as prose-only.

## Recommended Implementation Order

1. Fix tests for active-profile MCP dispatch and audience enforcement.
2. Align compatibility docs with bool numeric rejection or implement the documented behavior.
3. Add structured `RecommendedNextTool` parsing and strict finding parsing.
4. Add route-critical response-contract tests and classify tools by response class.
5. Implement the doc/tool-card generator scaffold and CI check mode.

This order keeps correctness ahead of documentation generation. The generator should reflect stable semantics, not lock in unresolved contract inconsistencies.

## Verification Checklist

Before closing this plan, run:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo run --bin generate_docs -- --check
```

If `generate_docs` is implemented as an xtask or test instead of a binary, replace the final command with the equivalent check command and document it in README or contributor docs.

## Non-Goals

Do not implement phases 6 and beyond in this pass.

Do not redesign the entire registry.

Do not force every simple utility tool to emit verdicts.

Do not add new deterministic tool categories unless required to support generated docs.

Do not change codegg integration directly; this pass prepares eggsact for safer codegg consumption.

## Final Acceptance Criteria

The phase 1 through 5 tightening line can be considered closed when:

- MCP active-profile dispatch tests prove restricted profiles are enforced.
- In-process registry tests prove audience/exposure enforcement.
- Compatibility docs and validator behavior match.
- Typed preflight wrappers parse structured next-tool hints and fail closed on malformed findings.
- Route-critical tools have explicit route-contract tests.
- Generated docs/tool-card scaffolding exists and is checked by CI or tests.
- All standard CI checks pass.
