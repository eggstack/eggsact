# API, Config Semantics, and Build-Surface Cleanup

Status: planned
Priority: P2
Scope: public API discipline, config correctness, typed workflow coverage, measured feature gating, documentation drift

## Objective

Tighten the library/API surface after the internal consolidation work, resolve ambiguous config/YAML semantics, deepen typed workflow integration where it has clear downstream value, and evaluate coarse feature gating without turning eggsact into a highly configurable workspace.

This plan follows the typed-composition and shared-analysis plans. Do not begin API narrowing before internal call sites have been migrated away from raw tool handlers.

## Part A — Public Rust API discipline

### A1. Inventory documented and accidental public surface

Audit all public modules/re-exports from `lib.rs`, `tools/mod.rs`, `mcp/mod.rs`, `agent/`, `preflight/`, `text/mod.rs`, and `calc/`.

Classify each public item as:

- intentionally stable library API;
- transport/adapter API useful for advanced integrations;
- implementation detail accidentally exposed;
- compatibility-only/deprecated.

Use docs/examples/tests and crates.io compatibility constraints to inform the classification. Do not assume an undocumented `pub` item is unused.

### A2. Establish the supported API hierarchy

Preferred long-term hierarchy:

1. `calc` and root calculator re-exports for calculator consumers;
2. `text` typed deterministic primitives for direct utility use;
3. `agent::ToolRegistry`/execution-context API for generic in-process tool dispatch;
4. typed workflow/preflight APIs for coding-agent harnesses;
5. MCP server entry surface for stdio integration.

Raw `tools::*` handlers are adapter internals and should not be the recommended Rust integration surface.

Document this hierarchy in `docs/library-api.md` and architecture docs before deprecating anything.

### A3. Deprecate accidental raw-handler API safely

If semver policy permits deprecation in 1.x, mark raw handler imports/documentation as deprecated or explicitly unstable while preserving them for compatibility. Remove them only at a planned breaking release.

Avoid mass churn if Rust deprecation attributes would generate unusable internal warnings. A practical intermediate state is acceptable: keep `pub` for compatibility but remove it from the recommended API and ensure internal code no longer depends on public visibility.

Do not change MCP tool stability because a Rust adapter handler is being deprecated.

### A4. Review MCP internals exposed as library API

Apply the same discipline to `mcp` internals. Preserve what downstream consumers reasonably need (server launch/integration types, registry views if intentionally supported) while avoiding promises around execution implementation details that should be crate-private.

Any actual visibility reduction must follow compatibility policy and be staged, not slipped into an unrelated patch release.

## Part B — Typed workflow coverage

### B1. Prefer workflow-level types over one wrapper per tool

Do not create typed wrappers for all 86 tools. Add typed APIs only for stable agent workflows where route-critical fields matter and downstream callers benefit from compile-time contracts.

### B2. Add typed dependency-edit preflight

`dependency_edit_preflight` is already a high-value composite across Rust, Python, and Node ecosystems and is a strong candidate for a typed facade.

Provide typed input/policy/result structures sufficient for callers to access:

- detected ecosystem/source type;
- additions/removals/version/source changes;
- scripts/hooks/patch overrides where applicable;
- findings and verdict;
- machine code and recommended next action/tool if present.

Use the canonical typed internal service from the preceding plans rather than adding another JSON parse layer where possible.

### B3. Evaluate typed patch/repository review facades

After `PatchAnalysis` and `RepoFacts` exist, determine whether downstream codegg/harness use warrants one typed patch-review facade and/or one typed repo-audit facade.

Only add them if there is a concrete consumer workflow. Avoid duplicating every projection tool as a separate typed wrapper.

### B4. Preserve strict contract behavior

Typed workflow APIs must continue the existing `PreflightError` principle: route-critical missing or malformed fields are contract violations, not silently defaulted values. Forward-compatible enums may retain `Other(String)` where appropriate.

## Part C — Config/YAML semantic correction

### C1. Remove false implication of YAML parsing

`config_file_inspect` recognizes `.yaml`/`.yml` and currently uses a line-oriented heuristic. It must not report parser-backed syntax validity when no YAML parser exists.

Choose and document a contract such as:

- `analysis_mode: "heuristic"` / equivalent field; or
- `parse_ok: null`/unavailable for heuristic YAML if compatible with existing output schema; or
- a finding/machine code explicitly stating YAML syntax was not validated.

Prefer the least disruptive compatible change. Do not introduce a YAML parser merely to preserve a misleading boolean.

### C2. Align auto-detection semantics

Audit `config_preflight`, `config_file_inspect`, and lower-level config/TOML helpers so format names and guarantees are explicit and consistent.

`config_preflight` currently intentionally excludes YAML. Keep that policy unless a concrete consumer requires real YAML validation.

### C3. Define a threshold for first-class YAML support

Real YAML support should require a concrete workflow and an explicit dependency/semantic review. Candidate triggers include deterministic validation of GitHub Actions, Kubernetes, Compose, or similar files in a downstream harness.

If that trigger occurs in the future, treat YAML as a separate feature line with parser selection, semantic limits, binary-size impact, schema/docs, and fuzzing. It is not part of this consolidation pass.

## Part D — Coarse feature-gating evaluation

### D1. Measure first

Current `default = []` does not make runtime dependencies optional. Before changing Cargo features, measure:

- clean compile time for a calculator-only example;
- dependency tree (`cargo tree`);
- release binary size;
- incremental build implications;
- whether downstream library users actually need calculator/text-only builds.

Record measurements in the implementation PR/commit notes or a short temporary evidence document; do not add permanent complexity without evidence.

### D2. If justified, use coarse features only

A reasonable upper bound is a few domain-level features, for example:

- core/calculator/text essentials;
- structured-config support;
- agent/MCP integration.

Exact naming should follow dependency boundaries discovered during implementation.

Do not create per-tool or per-category Cargo features. Avoid feature combinations that require a large CI matrix.

### D3. Preserve the binary experience

The shipped `eggsact` binary should continue to build with the full supported feature set and retain all 86 tools by default. Feature gating is primarily for library consumers, not a reason to fragment release binaries.

If a clean coarse split is not possible without significant conditional-compilation complexity, document the measurement and explicitly decline feature gating. That is an acceptable successful outcome.

## Part E — Compatibility cleanup and known overlap

### E1. `json_query` retirement path

Keep `json_query` compatibility during 1.x as required by policy. Verify it remains clearly deprecated and is not promoted by default documentation/tool recommendations. If profile/exposure policy allows hiding it further without breaking compatibility guarantees, do so cautiously.

Plan actual removal only for the next breaking release. Do not spend implementation effort enhancing it.

### E2. Stateful calculator context remains opt-in

Do not change `call_json_with_execution_context()` cloning semantics. Context isolation is the safe default for agent concurrency.

If a real consumer needs persistent calculator PRNG/memory/variables through `ToolRegistry`, design a separate explicit mutable/session API in a future plan. `evaluate_with_context()`/`run_with_context()` remain the current stateful calculator path.

## Part F — Documentation drift and maintenance checks

### F1. Correct known architecture drift

Audit architecture docs for paths/modules that no longer exist or moved. Known starting points:

- stale `sync_pool.rs` references versus current execution-pool organization;
- stale `mcp::tools` wording in `text/synthesis.rs` comments;
- any module-count/path references changed by the consolidation plans.

### F2. Add a lightweight path-reference check if worthwhile

Consider a small script/test that validates explicitly marked source-path references in architecture docs. Keep it simple; do not attempt to parse all Markdown prose or build a documentation framework.

Only add this if it catches real drift cheaply and can run in ordinary CI without external dependencies.

### F3. Regenerate/check generated docs

Any ToolSpec metadata changes must flow through the existing generator. Never hand-edit generated profile/tool-card assets.

## Implementation order

1. Complete consolidation plan 01.
2. Complete consolidation plan 02.
3. Inventory public API and document supported hierarchy.
4. Fix YAML/config guarantee wording/behavior.
5. Add typed dependency preflight; evaluate patch/repo typed facades.
6. Measure feature-gating value and implement only if justified.
7. Stage any API deprecations compatible with current semver policy.
8. Correct documentation drift and add only lightweight regression checks.
9. Run full verification and update roadmap closure evidence.

## Verification

Run the full verification sequence in `AGENTS.md`, including generated-doc check and packaging dry-run before any release-facing API/Cargo changes are considered complete.

For feature changes, test both the full binary build and each supported library feature configuration. Keep the feature matrix minimal.

For API changes, compile all examples/doctests and add compile-time regression coverage for documented stable imports where practical.

## Completion criteria

This plan is complete when the recommended Rust API no longer encourages raw adapter handlers, typed workflow coverage addresses the highest-value composites without proliferating wrappers, YAML inspection no longer implies parser-backed validation, feature gating has either been implemented on measured evidence or explicitly rejected as not worth the complexity, deprecated overlap has a clear retirement path, and architecture/docs accurately describe the resulting tree.