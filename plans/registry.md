# eggsact Active Planning Registry

This file is the compact control surface for active interim planning.
Detailed requirements and completed history remain in source roadmaps,
implementation plans, `plans/closure/`, and Git history.

Canonical direction remains in:

- `plans/000-long-term-specification.md`
- `plans/001-terminology-and-domain-model.md`
- `plans/002-long-term-roadmap.md`
- `plans/003-planning-process.md`

Legacy single-doc planning (`plans/archive/roadmap.md` plus flat
`plans/*.md` with appended closures) is superseded by ADR-0001 and
retained only for traceability.

## Status vocabulary

- **proposed** -- roadmap or plan exists but is not approved for execution.
- **ready** -- dependencies and interfaces are satisfied; plan may be handed off.
- **active** -- implementation or closure work is in progress.
- **blocked** -- a named dependency or evidence requirement prevents progress.
- **closing** -- implementation landed and closure evidence is being gathered.
- **closed** -- closure record accepted.
- **conditionally closed** -- substantial work landed, but a named correctness or operational evidence condition remains.
- **superseded** -- replaced by another document.
- **archived** -- no longer active and retained for traceability.

## Active subsystem roadmaps

| Subsystem | Status | Roadmap | Current milestone | Dependencies or blockers |
|---|---|---|---|---|
| Deterministic tool substrate | active | `plans/subsystems/deterministic-tool-substrate-roadmap.md` | Guard only; no open milestone | 008 closed the post-007 wire-cardinality/guard-coverage corrective without reopening 007 architecture; workstream is guard-only. |
| MCP presentation surface | active | `plans/subsystems/mcp-presentation-surface-roadmap.md` | 03c blocked (deterministic prep re-verified 2026-09-30; external evidence blocked; closure `plans/closure/mcp-presentation-surface/003c-status.md`) | Blocked on provider credentials / eval budget for OpenAI + Anthropic direct/discovery pairs and instructions A/B. |
| Harness integration and docs | active | `plans/subsystems/harness-integration-roadmap.md` | Guard only; no open milestone | Continuous guard (generate-docs check, parity baseline, context isolation). |
| Distribution, update, and release | active | `plans/subsystems/distribution-update-release-roadmap.md` | Maintenance-only; no open milestone | Eggpack is producer authority (cutover at `d8014cf`, pinned to Eggpack M003g `e5c81f2`); v1.2.7 live draft/release qualification is complete; M005a closed the Windows byte-reproducibility condition with real rerun-reuse evidence. Workstream is guard/maintenance-only. |

## Open implementation work

| Subsystem | Milestone | Status | Plan | Blocker / handoff note |
|---|---|---|---|---|
| MCP presentation surface | 03c evaluation closure corrective | blocked | `plans/implementation/mcp-presentation-surface/003c-evaluation-closure-corrective.md` | Closure `plans/closure/mcp-presentation-surface/003c-status.md` (blocked). Deterministic preparation is complete; model/client evidence and instructions A/B remain blocked on provider credentials and evaluation budget. Do not fabricate traces or mark complete without OpenAI + Anthropic pairs and instructions A/B. |

## Recently closed work (control points)

| Subsystem | Milestone | Status | Controlling evidence |
|---|---|---|---|
| Distribution, update, and release | M005a deterministic Windows artifacts | closed | `plans/closure/distribution-update-release/005a-status.md`; implementation `f135210`; maintenance run `36880110434` (two builds from different target directories, identical SHA-256 `dc1eda1f…`); pipeline rehearsal run `36886042696` attempts 1-2 (same draft 401132612, 15 assets reused, zero refusals; rehearsal draft/tag/branch deleted afterwards). |
| Distribution, update, and release | Eggfetch 0.1.7 updater bump | closed | Archive: `plans/archive/eggfetch-0.1.7-updater-dependency-bump.md` (plan + closure in file). Roadmap history in `plans/archive/roadmap.md`. |
| Distribution, update, and release | Eggfetch 0.2.0 updater adoption | closed | Archive: `plans/archive/eggfetch-0.2.0-updater-adoption.md`; implementation `bfe12d7`; ordinary CI `35734609288`; maintenance `35740940881`. |
| Distribution, update, and release | Eggfetch 0.2.0 adoption closeout corrective | closed | Archive: `plans/archive/eggfetch-0.2.0-adoption-closeout-corrective.md`; implementation `bfe12d7`; maintenance `35740940881` green (MSRV, cargo-deny, native Windows, native macOS). |
| Deterministic tool substrate | Performance campaign + closure corrective | closed | Commits `85e10bf` + `e8067ae`; remote CI `35542158872`; non-gating bench per `architecture/performance.md`. Detail in `plans/archive/roadmap.md`. |
| Distribution, update, and release | Binary distribution (C8 Zig correction `6658702`, v1.2.4 matrix) | closed | Release `v1.2.4`, workflow `33944943782`; installer verification recorded in `plans/archive/roadmap.md`. |

## Blocked work

| Subsystem | Milestone | Blocker |
|---|---|---|
| MCP presentation surface | 03c Parts D-F (model/client traces, instructions A/B, rollout decision) | No provider credentials, subscriptions, or evaluation budget for ~200 model calls (attempted 2026-09-11 and re-attempted 2026-09-30: `codex` 0.159.2 / `claude` 2.1.280 CLIs present, no budget/approval). Closure `plans/closure/mcp-presentation-surface/003c-status.md` records the blocked disposition. Generated integrations stay on direct; discovery stays explicitly selectable. |

## Closure work and current control points

- MCP modernization protocol/runtime/discovery implementation is complete
  (`35f7dc2e`, `a5c00b8d`, `77ff57a5`, `121babde`, `40222999`); only the
  `03c` external evidence closure remains blocked
  (`plans/closure/mcp-presentation-surface/003c-status.md`; deterministic
  prep re-verified 2026-09-30). M005a closed 2026-10-01 without unblocking
  `03c`, which stays blocked on provider credentials.
- Deterministic tool substrate Milestones 003, 004, 005, 006, 007, and 008 remain
  closed historical control points. The Unicode workstream (003 semantic
  hardening + 004 data qualification + 005 standards-conformance corrective
  + 006 edge-case corrective) is complete and is not reopened. Milestone
  007 closed the typed-core/adapter ownership drift identified by the
  September 27 audit, and Milestone 008 closed the two post-007 findings:
  pre-007 prompt recommendation wire cardinality/order is restored and the
  layering guard is generalized to all src/tools handlers. Harness remains
  guard-only (03c external evidence still blocked).
  Distribution M001-M005 are now closed control points: M005 replaced the
  handwritten release workflow with Eggpack-generated CI (producer authority
  pinned to Eggpack M003g `e5c81f2`), qualified a real draft for v1.2.7, and
  that release was published with public installer, latest/download, and
  `eggsact update` smoke all verified. Its one open condition is now closed:
  M005a made the Windows release candidate byte-reproducible (target-scoped
  `/BREPRO` + `/DEBUG:NONE` in `.cargo/config.toml`) and proved same-tag rerun
  reuse of all fifteen staged assets, so a rerun no longer fails closed on the
  Windows asset. The distribution workstream is maintenance-only;
  self-update semantics remain eggsact/Eggup-owned and were not migrated.
- The durable rollout gates for `03c` remain: 100% stable-Model coverage,
  retrieval top-1 >=90% / top-3 >=98% / top-5 100%, 0 Model->HarnessOnly
  leaks, discovery/direct byte ratio <=25%, >=40 Model task scenarios
  after containment split, OpenAI pair recorded + scored, Anthropic pair
  recorded + scored, server-instructions A/B recorded.