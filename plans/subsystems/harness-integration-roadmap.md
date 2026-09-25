# Harness Integration Roadmap

Status: active (guard only; no open milestone)

Long-term references:

- `plans/000-long-term-specification.md#2` (in-process library, preflight)
- `plans/000-long-term-specification.md#5` (context isolation, strict profile parsing)
- `plans/000-long-term-specification.md#7` (invariants 3, 8, 10)
- `plans/001-terminology-and-domain-model.md#4` (registry, preflight, contexts)
- `plans/002-long-term-roadmap.md#phase-2`

Related ADRs:

- `plans/adrs/ADR-0001-planning-conventions-adoption.md`

## 1. Purpose and ownership boundary

Own the harness surface: `agent::ToolRegistry`, `ExecutionContext`
semantics, typed `preflight` wrappers, `docs/library-api.md` hierarchy,
generated-docs pipeline, and the parity baseline.

Consumes: deterministic substrate. Must not own: capability semantics,
MCP transport eras, or release publishing.

## 2. Work classification

### Invariants

- Recommended hierarchy (`calc`/root -> typed `text` ->
  `ToolRegistry`/contexts -> typed `preflight` -> MCP server); raw
  `tools::*`/`services::*`/`mcp` internals are 1.x-compat, not the
  import surface.
- `call_json_with_execution_context()` clones `eval_ctx`;
  `evaluate_with_context()`/`run_with_context()` for calculator state;
  `..._context_mut` deprecated; re-entrant mutable access panics.
- `Profile::from_str_opt` strict; `Profile::custom(name)` for custom;
  model-facing code uses `available_tools_model_safe()`.

### Capabilities

- In-process tool calls with budgets and execution contexts.
- Typed preflight wrappers (e.g. `DependencyPreflight`).

### Infrastructure

- `generate-docs` pipeline with `--check`; confusables pinning
  (Unicode 17.0.0 + SHA).

### Polish

- Doc/architecture drift correction (module-path vocabulary, wrapper
  counts); declined coarse feature gating and patch-review/repo-audit
  facades recorded as evaluated-and-declined.

## 3. Non-goals

- No visibility reduction in 1.x for `pub` adapter internals.
- No `json_query` promotion in default docs.
- No persistent calculator sessions by default.
- No lightweight path-reference checker as a substitute for real drift
  review.

## 4. Current state

Shipped: `ToolRegistry`/`ExecutionContext` APIs, typed preflight
wrappers, `DependencyPreflight`, additive `analysis_mode` on
`config_file_inspect`, generated docs, property tests, fuzz targets.
Parity baseline: 37 accepted C1-C6 failures; only non-listed failures
are regressions.

## 5. Target architecture

Unchanged: the same typed import surface with drift-proof generation
and an honest parity baseline.

## 6. Dependency graph

```text
Registry guard (continuous)
    |
    +--> Preflight typing (closed)
    |
    `--> Docs generation guard (continuous)
```

No open hard dependencies.

## 7. Milestones

### Milestone 1 — Typed preflight composition

Class: infrastructure + capability

Objective: harness code calls typed services/cores through documented
wrappers.

Dependencies: substrate guard (soft).

Deliverable boundary: `DependencyPreflight` added; declined facades
recorded with rationale.

User or operator value: safer harness composition without handler
chaining.

Exit conditions: closed (legacy consolidation-03 slice).

Deferred work: first-class YAML and stateful sessions deferred until a
concrete workflow justifies them.

## 8. Cross-cutting requirements

### Determinism and bounded execution

Context isolation preserved; budgets explicit; concurrent MCP responses
correlated by JSON-RPC `id`.

### Protocol and compatibility

No per-call profile; server-wide env applies; discovery facades are not
`ToolSpec`s (touching them updates `test_discovery.rs`).

### Profile, audience, and surface

Harness audience retains full visibility where Model is filtered.

### Documentation and generated assets

After any registry/profile/exposure change: re-run `generate-docs`;
CI checks with `-- --check`. Never hand-edit generated files.

### Release and qualification

Parity requires `eggcalc` at `../eggcalc`; excluded from CI by design.

## 9. Verification strategy

Registry/profile/audience suites, context isolation tests, doc `--check`,
parity with accepted-failure list, merge gate in order.

## 10. Risks and decision points

No open decisions. A new public harness API or stateful session
requires an ADR first.

## 11. Completion definition

Guard workstream: remains active. Milestones close via `closure/`
records when new harness work lands.

## 12. Milestone status

| Milestone | Status | Implementation plan | Closure record | Blockers |
|---|---|---|---|---|
| Typed preflight composition | closed | — (legacy consolidation-03 slice; history in git) | — | — |
