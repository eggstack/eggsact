# Phase 5: Profile and Exposure Hardening

## Goal

Make tool exposure explicit, typed, documented, and test-gated so codegg can safely use eggsact without overwhelming models or leaking harness-only utilities into ordinary model-facing tool lists.

The current profile system is directionally strong. It already includes codegg-specific profiles and `tools/list` filtering by profile, name, tier, tags, and schema detail. The hardening work is to make exposure semantics non-stringly typed, remove documentation drift, and enforce model-visible versus harness-only boundaries.

## Scope

In scope:

- Convert exposure, cost, and stability string fields to enums internally.
- Generate serialized strings from enums for MCP compatibility.
- Define model-visible, contextual, expert, harness-only, and hidden exposure semantics.
- Audit every existing tool's profile and exposure assignment.
- Add tests preventing harness-only tools from appearing in default model-facing lists.
- Update docs to match implementation.
- Add profile snapshots for codegg-critical profiles.

Out of scope:

- Adding new tool categories.
- Changing individual tool behavior.
- Implementing codegg-side integration.
- Removing MCP filters that already exist.

## Exposure Model

Use a typed enum internally:

```rust
pub enum ToolExposure {
    ModelDefault,
    ModelContextual,
    ModelExpert,
    HarnessOnly,
    Hidden,
}
```

Serialized MCP strings can remain concise:

- `model_default` or preserve `default` for compatibility.
- `model_contextual` or preserve `contextual`.
- `model_expert` or preserve `expert_only`.
- `harness_only`.
- `hidden`.

To minimize compatibility risk, preserve current output strings unless there is a strong reason to rename. Internally, use clearer enum variants.

Semantics:

### `ModelDefault`

Safe and useful for ordinary model-visible use. These tools can appear in `default` or `codegg_core_min` model-facing lists. They should be cheap, easy to explain, and unlikely to create tool overload.

Examples: basic text equality, text measurement, JSON validation, path normalization, simple config validation.

### `ModelContextual`

Useful when the current workflow calls for the category. These should not be in the smallest default list, but codegg can expose them when editing, config work, shell planning, Unicode investigation, or repo audit is active.

Examples: JSON extraction, text windowing, patch summary, markdown structure, path analysis.

### `ModelExpert`

Specialized tools that should only appear for manager/reviewer/research agents or explicit expert workflows.

Examples: identifier table inspection, JSON shape analysis, version constraints, repo audit helpers.

### `HarnessOnly`

Tools that codegg should call automatically but the model should not generally see. These include safety checks and preflight tools that are better enforced by the harness than delegated to the model.

Examples: edit preflight, patch apply check, Unicode policy check, prompt input inspection, command preflight in strict modes.

### `Hidden`

Internal or compatibility tools that should not be listed except in debug/developer contexts.

## Profile Model

Preserve current profile names:

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

Clarify each profile's intended user:

### `default`

General-purpose MCP profile for ordinary clients. Should not become too large. It may include model-default tools and a small number of contextual tools if they are broadly useful.

### `codegg_core_min`

Smallest codegg model-visible profile. This should be the default for ordinary coder-agent sessions. It should include only tools that reduce hallucination without causing choice overload.

### `codegg_core`

Broader but still model-safe codegg profile. Suitable for manager/reviewer agents or sessions where deterministic utility use is desired.

### `codegg_preflight`

Harness-oriented profile for automatic checks. This may include harness-only tools but should not be exposed directly to the model by default.

### `codegg_patch`

Patch/edit-focused tools. Split model-visible inspection helpers from harness-only mutation preflight checks by exposure level.

### `codegg_config`

JSON/TOML/config validation and inspection tools for generated config work.

### `codegg_unicode_security`

Unicode, hidden-character, confusable, prompt-ingress, and identifier security checks. Most of these should be harness-only or contextual.

### `codegg_shell`

Shell argv and command preflight helpers. Direct model visibility should be limited; harness use should be automatic before command execution.

### `codegg_repo_audit`

Specialized repo inspection tools for manager/reviewer/research workflows. Not default coder-agent exposure.

### `human_math`

Calculator, unit, and constant tools for direct human utility and general MCP use.

## Implementation Sequence

### Step 1: Introduce Typed Enums

Convert internal string fields for exposure, cost, and stability into enums in the registry. Provide `as_str()` or `Display` implementations that preserve existing serialized strings.

Also consider typed categories later, but do not block this phase on category enum work.

### Step 2: Update Registry Declarations

Audit every tool declaration and replace string exposure/cost/stability values with enum variants. Preserve current behavior first. If a tool's current exposure is clearly wrong, adjust only with a test and a note.

High-priority audit targets:

- `patch_apply_check`
- `edit_preflight`
- `command_preflight`
- `unicode_policy_check`
- `prompt_input_inspect`
- `text_security_inspect`
- `identifier_table_inspect`
- `cargo_toml_inspect`

### Step 3: Define Listing Modes

Add clear listing modes so callers can ask for model-safe versus harness-capable views.

Suggested API concept:

```rust
pub enum ToolListAudience {
    Model,
    Harness,
    Debug,
}
```

MCP `tools/list` can preserve current behavior, but codegg's in-process API should be explicit. If MCP needs this too, add an optional `audience` parameter later, but avoid breaking existing clients.

Rules:

- `Model` excludes `HarnessOnly` and `Hidden`.
- `Harness` includes harness-only tools for selected profiles but excludes hidden.
- `Debug` can include hidden tools if explicitly enabled.

### Step 4: Enforce Profile/Exposure Boundaries

Add tests that default model-facing profile listings do not include harness-only tools.

Important cases:

- `codegg_core_min` model audience excludes harness-only tools.
- `codegg_core` model audience excludes harness-only tools.
- `default` model audience excludes harness-only tools.
- `codegg_preflight` harness audience includes expected preflight tools.
- `full` debug or legacy mode behavior is documented and tested.

If current MCP `full` includes harness-only tools, decide whether to preserve that for compatibility and add a safer codegg audience filter separately. Do not accidentally break existing broad MCP users without intention.

### Step 5: Add Profile Snapshots

Create profile snapshot tests for:

- `codegg_core_min`
- `codegg_core`
- `codegg_preflight`
- `codegg_patch`
- `codegg_config`
- `codegg_unicode_security`
- `codegg_shell`
- `codegg_repo_audit`

Snapshots should include tool names and exposure values. They should be easy to update intentionally when profiles change.

### Step 6: Update Docs

Fix the architecture documentation so exposure semantics match implementation. Generate docs from the registry if Phase 1 makes that easy. If generation is not implemented yet, update docs manually but leave a TODO for generated docs in Phase 11.

Docs should include:

- profile names
- intended consumers
- exposure levels
- audience filtering rules
- how codegg should choose profiles
- harness-only warning

### Step 7: Add Codegg Guidance

Add a short architecture note or README section describing recommended codegg use:

- ordinary coder-agent model list: `codegg_core_min` with model audience
- edit harness: `codegg_preflight` or `codegg_patch` with harness audience
- shell harness: `codegg_shell` with harness audience
- config edits: `codegg_config`
- suspicious input ingress: `codegg_unicode_security`
- manager/reviewer repo work: `codegg_repo_audit`

## Testing Plan

Run:

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

Add tests for:

- enum serialization preserves current strings.
- every tool has a valid exposure enum.
- hidden tools are excluded from ordinary listing.
- harness-only tools are excluded from model audience lists.
- harness audience includes expected preflight tools.
- profile snapshots match intended names.
- invalid profile handling remains compatible.

## Compatibility Requirements

Existing MCP `tools/list` behavior should remain compatible unless a deliberate safe-mode parameter is added. Since codegg can use the in-process API after Phase 4, safer audience filtering can be enforced there without breaking broad MCP clients.

If MCP output field `llm_exposure` currently contains values such as `default`, `contextual`, `expert_only`, and `harness_only`, preserve those strings initially.

## Risks

The main risk is accidentally hiding tools from existing MCP clients. Mitigate by preserving legacy list behavior unless the request explicitly asks for model-safe audience filtering.

The second risk is profiles becoming political or subjective. Keep criteria operational: default model visibility should be small, cheap, safe, and broadly useful; harness-only should be automatic validation or safety checks.

The third risk is stale docs. Prefer generated docs from the registry where possible.

## Acceptance Criteria

Phase 5 is complete when:

- exposure, cost, and stability are typed internally.
- docs match implementation.
- model versus harness audience semantics are explicit.
- codegg-critical profiles have snapshot tests.
- harness-only tools are excluded from model-safe listings.
- existing MCP compatibility is preserved unless intentionally extended.
- formatting, clippy, and tests pass.

## Handoff Notes

Do not use this phase to debate every individual tool's long-term placement. Preserve current assignments where uncertain and add TODOs for later profile tuning. The important deliverable is the enforcement mechanism and tests that make future profile changes intentional.
