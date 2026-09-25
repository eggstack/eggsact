# Deterministic Tool Substrate Roadmap

Status: active (guard only; no open milestone)

Long-term references:

- `plans/000-long-term-specification.md#2` (calculator, registry, typed hierarchy)
- `plans/000-long-term-specification.md#4` (one crate, typed-first, determinism, bounds)
- `plans/000-long-term-specification.md#5` (compatibility requirements)
- `plans/000-long-term-specification.md#7` (invariants 1-3, 5-6, 8)
- `plans/001-terminology-and-domain-model.md#3` (tool, ToolSpec, category)
- `plans/001-terminology-and-domain-model.md#4` (cores, services, adapters)
- `plans/002-long-term-roadmap.md#phase-0`

Related ADRs:

- `plans/adrs/ADR-0001-planning-conventions-adoption.md`

## 1. Purpose and ownership boundary

Own the deterministic capability substrate: `calc/` + `text/` leaf
cores, `services/` typed composites, the single-source `ToolSpec`
registry, limits/truncation, and the typed-first composition rule.

Consumes: nothing upstream. Consumed by: MCP presentation, harness
integration, and distribution workstreams.

Must not own: MCP transport eras, discovery ranking, preflight facade
selection, installer/update trust, or release publishing.

## 2. Work classification

### Invariants

- One `ToolSpec` per tool; `tool_registration_tables_are_in_sync` green.
- `calc`/root -> typed `text` -> `ToolRegistry`/contexts -> typed
  `preflight` -> MCP server; no sibling-handler calls outside the three
  documented same-module reuses.
- Deterministic exact-input/exact-output; no clock/TZ/env/net in utils.
- Bounded limits with `limits_applied`; automatic truncation.

### Capabilities

- Calculator evaluation with units/constants (`^` XOR, `**` power).
- 86 tools across 23 categories with stable machine codes.

### Infrastructure

- `RepoFacts` / `PatchAnalysis` canonical classifiers with
  differential anti-drift tests.
- `compile_regex()` engine selection with `engine_used` reporting.
- Heuristic-only YAML mode reporting.

### Polish

- Performance hot-path work that preserves all public shapes and
  determinism (non-gating evidence; see distribution roadmap closure
  history and `architecture/performance.md`).

## 3. Non-goals

- No broad utility expansion beyond specification-heavy exact operations.
- No stateful calculator sessions by default.
- No YAML parser dependency; no PCRE2 semantics.
- No host-specific timing thresholds in ordinary tests.

## 4. Current state

Single-crate 86-tool registry with profile/audience/exposure filtering
is shipped. Typed-first composition (`consolidation-01`), shared
analysis (`consolidation-02`), and API/config/build surface
(`consolidation-03`) are closed; detail pruned per the legacy
convention with evidence in git history. The performance campaign
(`85e10bf`) plus closure corrective (`e8067ae`) is closed with the
newline differential matrix as a regression test.

## 5. Target architecture

Unchanged end state: the same substrate with registry-sync, typed
hierarchy, determinism, and bounds guards enforced on every change.
Performance patches remain vertical, differential-tested, and
non-gating.

## 6. Dependency graph

```text
Registry integrity (continuous guard)
    |
    +--> Shared analysis anti-drift (closed)
    |
    `--> Hot-path evidence (closed; future patches are corrective-only)
```

All dependencies are closed. New substrate work requires a corrective
plan against this roadmap.

## 7. Milestones

### Milestone 1 — Typed-first composition guard

Class: invariant

Objective: keep adapters calling cores/services exactly once at the
boundary.

Dependencies: none (continuous).

Deliverable boundary: `architecture/tools.md` reuse list stays at three;
no new adapter-to-adapter calls.

User or operator value: stable deterministic behavior across all
consumers.

Exit conditions: registry sync + differential tests green on every
touch.

Deferred work: none.

### Milestone 2 — Shared analysis anti-drift

Class: infrastructure

Objective: keep `RepoFacts`/`PatchAnalysis` canonical.

Dependencies: Milestone 1 (soft).

Deliverable boundary: differential tests guard cross-tool semantic drift.

User or operator value: consistent repo/patch answers.

Exit conditions: closed (historical consolidation-02).

Deferred work: none.

## 8. Cross-cutting requirements

### Determinism and bounded execution

Explicit inputs only; limits contractual; `limits_applied` checked.

### Protocol and compatibility

Registration-order prefix preserved; `json_query`
deprecated-but-compatible; context isolation unchanged.

### Profile, audience, and surface

No change owned here; substrate MUST NOT bypass filtering.

### Documentation and generated assets

`generate-docs` re-run on registry/profile/exposure change; never
hand-edit generated files.

### Release and qualification

Parity baseline (`accepted_parity_failures.txt`, C1-C6) distinguishes
regressions; bench evidence non-gating.

## 9. Verification strategy

Registry sync, differential semantics, limits/truncation, engine
selection, YAML mode, context isolation, plus the ordered merge gate
with `--test-threads=4` for integration tests.

## 10. Risks and decision points

No open decisions. A new utility category or composition-layer change
requires an ADR before a milestone plan.

## 11. Completion definition

This roadmap closes only if the substrate is intentionally superseded;
as a guard workstream it remains active. Individual milestones close
via `closure/` records; the performance line is already closed.

## 12. Milestone status

| Milestone | Status | Implementation plan | Closure record | Blockers |
|---|---|---|---|---|
| Typed-first composition guard | closed (continuous guard) | — (legacy consolidation-01, pruned; history in git) | — | — |
| Shared analysis anti-drift | closed | — (legacy consolidation-02, pruned; history in git) | — | — |
| Performance campaign + corrective | closed | — (commits `85e10bf` + `e8067ae`) | — (detail in `plans/archive/roadmap.md`) | — |
