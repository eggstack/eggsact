# Typed Composition and Module-Boundary Consolidation

Status: complete
Priority: P1
Scope: internal architecture and maintainability; preserve public behavior

## Objective

Restore the intended layering so deterministic typed/core operations are composed directly in Rust and JSON-shaped tool adapters remain boundary code. Remove internal dependencies on `serde_json::Value`/`ToolResponse` field names where composite tools currently call other tool handlers and then re-parse their response envelopes.

This is a consolidation pass, not a feature expansion. Preserve the 86-tool registry, MCP wire contracts, machine codes, profiles, audiences, public CLI behavior, compatibility mode behavior, and existing deterministic semantics unless a separately documented correctness bug is found.

## Problem statement

The architecture intends `text/` and other pure helpers to be the reusable substrate and `tools/` to adapt that substrate into MCP/agent contracts. Several composites currently cross this boundary in the opposite direction. For example, `edit_preflight` constructs JSON arguments, calls `crate::tools::text::text_fingerprint_tool()` and `text_security_inspect()`, then extracts fields such as `newline_style`, `verdict`, `machine_code`, and `findings` from `ToolResponse.result`.

This creates runtime coupling between internal Rust composition and transport-facing field names, duplicates validation/conversion work, and contributes to large handler modules. The existing typed `preflight` API improves downstream ergonomics but itself wraps `ToolRegistry::call_json`, so typed structures are not yet the canonical internal representation.

## Non-goals

- Do not merge unrelated MCP tools merely to reduce tool count.
- Do not introduce a workspace or split eggsact into multiple crates.
- Do not add macros/code-generation frameworks for schemas.
- Do not change MCP response envelopes, tool names, profile membership, machine codes, or compatibility behavior as part of this pass.
- Do not remove public raw handlers in a patch/minor release; API narrowing belongs to the follow-up API plan.
- Do not rewrite mature calculator internals unless needed to remove a concrete boundary violation.

## Design direction

Canonical flow should be:

```text
Typed deterministic core/service
        |
        +--> typed composite/service
        |
        +--> tools/* JSON adapter -> ToolResponse
                              |
                    MCP / ToolRegistry
```

Composite implementation code should call typed core/service functions, not sibling tool adapters. JSON serialization/deserialization should happen at the outer boundary.

Use ordinary Rust structs/enums/functions. Prefer small explicit types over generic abstractions. Where the public typed preflight structs already match the real contract, reuse or relocate them rather than creating parallel types.

## Implementation phases

### Phase 1 — Inventory adapter-to-adapter composition

Search `src/tools/`, `src/preflight/`, and `src/agent/` for calls from one tool handler into another `crate::tools::*` handler. Produce a short implementation note in the commit/PR identifying each call site and classifying it as:

1. should call an existing typed leaf function;
2. needs a new typed service result;
3. intentionally must use full dispatch policy and should remain a registry call.

Known starting points include `edit_preflight` newline/fingerprint composition and text-security composition. Also inspect `structured_data_compare`, `command_preflight`, `config_preflight`, and dependency composites.

Acceptance: every remaining handler-to-handler call is intentional and documented by code comments/tests, or removed.

### Phase 2 — Introduce typed service results where missing

Create minimal typed results for composite-relevant operations. Candidate seams include:

- text fingerprint/newline facts;
- Unicode/text-security inspection result;
- config validation result;
- shell/command lexical analysis result;
- edit-preflight findings/verdict derivation.

Do not duplicate MCP schemas as Rust types mechanically. Only introduce types used by multiple internal callers or public typed APIs.

Typed services must not depend on `ToolResponse`, MCP registry metadata, profile/audience policy, or JSON schema validation.

Acceptance: service tests can call the typed functions without constructing `serde_json::Value`.

### Phase 3 — Migrate composites to typed composition

Refactor composites incrementally. For each composite:

1. retain its current outer handler signature (`fn(&Value) -> ToolResponse`);
2. parse/validate its own adapter input;
3. call typed internal operations;
4. derive findings/verdict/machine codes from typed results;
5. construct the existing response shape once at the boundary.

Preserve current cancellation checkpoints. If a typed operation can be long-running, pass a lightweight budget/cancellation view or add explicit stage checks rather than depending on MCP response construction.

For `edit_preflight`, remove JSON construction/re-parsing for fingerprint/newline and text-security checks. Verify all replacement modes (`literal`, `patch`, `line_range`) and policies remain behaviorally identical.

Acceptance: no composite uses another tool handler merely to obtain an internal result that is available as a typed/core operation.

### Phase 4 — Split responsibility-dense modules only along real seams

After typed seams exist, split oversized files where the new architecture provides natural ownership boundaries. Prioritize:

- `src/preflight/mod.rs` -> shared types plus workflow modules (`edit`, `command`, `config`, `patch`, `text_security` or equivalent);
- `src/agent/mod.rs` -> profile/audience/types, registry facade, execution/context internals while preserving re-exports;
- `src/text/validate.rs` -> semantic domains where doing so reduces cross-domain coupling;
- `src/tools/text.rs` and `src/tools/patch.rs` only if typed-core extraction leaves coherent adapter submodules.

Do not split generated tables or mature parser code solely for line-count aesthetics.

Acceptance: public import paths documented as stable continue to compile, and module splitting reduces mixed responsibilities rather than just file size.

### Phase 5 — Remove duplicated handler-side contract logic where safe

Once raw handlers are clearly adapters and typed inputs exist, consolidate repeated defaults/enums/constants shared between schemas and handlers. Use shared constants or typed parsers for values such as detail levels, platform names, policy enums, and limits.

Do not assume schema validation always ran while raw handlers remain publicly callable. Maintain defensive validation until the API-surface plan establishes an explicit deprecation path.

## Testing requirements

For each migrated composite, add characterization tests before or during the refactor covering success, invalid arguments, policy variants, machine code selection, findings, verdict, and `recommended_next_tool` where applicable.

Add tests that compare the pre-refactor public response fixtures/contracts to the new adapter output. Existing route-critical tests and generated schema/doc checks remain authoritative.

Run the repository verification order from `AGENTS.md`, at minimum:

```bash
cargo fmt --all -- --check
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
```

If public Rust module organization changes, also run downstream-style compile tests/examples for documented imports.

## Completion criteria

This plan is complete when internal composite behavior is typed-first, adapter-to-adapter JSON composition has been eliminated except where full dispatch policy is genuinely required, the largest mixed-responsibility modules have been split only where natural seams exist, and all existing tool/MCP/public behavior remains compatible.

Record any deliberate remaining JSON composition in `architecture/tools.md` or the relevant deep-dive. Update `architecture/overview.md`, `architecture/preflight.md`, and `architecture/agent-api.md` to match the final layering.

## Implementation note (shipped)

New `src/services/` layer (`fingerprint`, `newline`, `security`) over `text/` cores; `tools/*` adapters now call typed cores/services, never sibling handlers:

1. `edit_preflight` literal `text_replace_check_tool` → `text::replace::text_replace_check` (existing typed leaf);
2. `edit_preflight` patch `patch_apply_check` (same-file) → `text::patch_apply_check` core with adapter-equivalent length guards;
3. `edit_preflight` line-range `line_range_extract_tool` → `text::line_range_extract` core;
4. `edit_preflight` fingerprint/newline `text_fingerprint_tool` (5 sites) → `services::fingerprint_facts` / `services::newline_facts`;
5. `edit_preflight` `path_scope_check` → `text::path_scope_check` core;
6. `edit_preflight` `text_security_inspect` → `services::inspect_text_security` (new typed pipeline; cancellation via `should_stop` view);
7. `text_security_inspect` adapter → delegates to `services::inspect_text_security`; internal `unicode_policy_check` / `identifier_inspect` / `prompt_input_inspect` adapter calls replaced by `text::unicode_policy_check` / `text::identifier_inspect` / `text::inspect_prompt::prompt_input_inspect` cores (text_inspect essentials computed directly from `unicode_tools`/`confusables` with identical limits/warnings);
8. `config_preflight` `validate_json` / `validate_schema_light_tool` / `json_canonicalize` / `validate_toml_tool` / `cargo_toml_inspect` → `text::validate_json` / `text::validate_schema_light` / `text::json_canonicalize` / `text::toml::validate_toml` / `text::cargo_toml_inspect` cores; same-file `dotenv_validate` / `ini_validate` adapters → `text::dotenv_validate` / `text::ini_validate` cores;
9. `structured_data_compare` `validate_json` (2 sites) → `text::validate_json` core;
10. `command_preflight` `regex_safety_check_tool` → `text::regex_safety_check` core.

No call site required full dispatch policy; all were category 1/2 (existing or new typed leaf/service). Remaining same-module JSON reuses are intentional and commented: `json_compare` / `json_shape_tool` in `structured_data_compare` (plus dead-code `TYPE_MISMATCH` parity BUG-006), `toml_shape_tool` in `config_preflight`.

Module splits: new `services/` directory is the seam (fingerprint/newline/security). `preflight/mod.rs`, `agent/mod.rs`, `text/validate.rs`, `tools/text.rs`/`patch.rs` deliberately not split further (coherent single responsibilities; size-only splits deferred). Raw handlers keep defensive validation. All 86 tools, wire contracts, machine codes, profiles, and CLI behavior preserved (3642 tests pass, parity excluded).