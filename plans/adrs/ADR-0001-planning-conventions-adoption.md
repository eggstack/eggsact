# ADR-0001: Adopt CodeGG Planning Conventions

Status: accepted

Date: 2026-09-25

Decision owners: project maintainers

Related specification sections:

- `plans/000-long-term-specification.md#4` (architectural principles)
- `plans/000-long-term-specification.md#5` (compatibility requirements)
- `plans/002-long-term-roadmap.md` (roadmap governance)
- `plans/003-planning-process.md` (document classes, registry requirements)

Affected subsystem roadmaps:

- `plans/subsystems/deterministic-tool-substrate-roadmap.md`
- `plans/subsystems/mcp-presentation-surface-roadmap.md`
- `plans/subsystems/harness-integration-roadmap.md`
- `plans/subsystems/distribution-update-release-roadmap.md`

## Context

eggsact used a single living `plans/roadmap.md` (pruned on ship, git
history as evidence) plus flat `plans/*.md` files with closure records
appended to the same file. CodeGG uses separated canonical docs
(`000/001/002/003`), subsystem roadmaps, bounded implementation plans,
separate closure records, archive retention, and a compact
`registry.md` control surface.

The flat convention mixes long-term direction with transient execution
detail, appends closures to plans instead of gating them separately, and
provides no compact status vocabulary or dependency-ready handoff
contract for agents.

## Decision drivers

- Keep durable direction stable while execution detail churns.
- Give implementation agents one bounded, baseline-tied handoff artifact.
- Gate closure on recorded evidence, not commit messages or scaffolding.
- Preserve completed history without pruning the active control surface.
- Stay proportionate to a single-crate deterministic tool (no daemon,
  team, or distributed concepts imported wholesale).

## Considered options

### Option A — Adopt CodeGG structure adapted to eggsact scale

Add `000/001/002/003`, `README`, `registry.md`, `adrs/`/`subsystems/`/
`implementation/`/`closure/`/`archive/` with eggsact verification
mapping (merge gate, generate-docs check, parity baseline,
release contract, non-gating bench). Migrate the one active plan
(`03c`) to `implementation/`; archive the legacy single doc and closed
flat plans unchanged.

Benefits: separated horizons, bounded handoffs, evidence-gated closure,
traceable archive, compact registry. Costs: one-time migration churn;
historical links to `plans/roadmap.md` resolve via archive/git history.

### Option B — Keep flat convention with minor hardening

Add status headers and a checklist to the existing flat files.

Benefits: no churn. Costs: retains horizon mixing, appended closures,
and unbounded living-doc growth; does not fix handoff or closure
gating. Rejected.

## Decision

Adopt Option A. The canonical set is `000/001/002/003`; the active
control surface is `plans/registry.md`; subsystem roadmaps own
workstreams; `implementation/` owns bounded handoffs;
`closure/` owns evidence gates; `archive/` retains the legacy era
unchanged.

## Consequences

### Positive

- Long-term docs stop churning with execution detail.
- Agents receive baseline-tied, invariant-guarded, verification-explicit
  handoffs.
- Closure requires requirement-to-evidence matrices and exact commands run.
- Registry stays compact; detail lives in source documents.

### Negative

- One-time migration touches only `plans/`; historical cross-links to
  `plans/roadmap.md` now resolve via `plans/archive/roadmap.md` and git
  history until callers update.

### Neutral or deferred

- No Rust, schema, profile, protocol, or release behavior changes in
  this adoption. No new subsystem workstreams beyond the initial four.

## Compatibility and migration

Docs and plans referencing `plans/roadmap.md` are historical evidence
and are not rewritten, except that `architecture/performance.md` may
point to the archive path on a future touch. New planning references
MUST use `plans/registry.md` and subsystem paths. `AGENTS.md` gains a
planning pointer; no merge-gate or code changes are included.

## Determinism and bounded-execution implications

None. Planning-only change.

## Verification

- `cargo fmt --all -- --check` green (plans-only change: no Rust diff).
- `cargo run --locked --features dev-tools --bin generate-docs -- --check`
  green (no registry/profile/exposure change).
- `plans/registry.md` links resolve to existing files.
- No closed evidence rewritten: archived files byte-identical to
  pre-migration originals except the moved `03c` header mapping.

## Supersession

None.
