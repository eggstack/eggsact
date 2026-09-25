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
| Deterministic tool substrate | active | `plans/subsystems/deterministic-tool-substrate-roadmap.md` | 003 ready; 004 blocked on 003 | Unicode security semantic hardening is dependency-ready; Unicode 18 data qualification follows only after 003 closure. |
| MCP presentation surface | active | `plans/subsystems/mcp-presentation-surface-roadmap.md` | 03c active (deterministic prep done; external evidence blocked) | Blocked on provider credentials / eval budget for OpenAI + Anthropic direct/discovery pairs and instructions A/B. |
| Harness integration and docs | active | `plans/subsystems/harness-integration-roadmap.md` | Guard only; no open milestone | Continuous guard (generate-docs check, parity baseline, context isolation). |
| Distribution, update, and release | closed | `plans/subsystems/distribution-update-release-roadmap.md` | All milestones closed; maintenance only | New work requires a corrective plan. |

## Dependency-ready implementation plans

| Subsystem | Milestone | Status | Implementation plan | Dependencies / handoff note |
|---|---|---|---|---|
| MCP presentation surface | 03c evaluation closure corrective | active | `plans/implementation/mcp-presentation-surface/003c-evaluation-closure-corrective.md` | Deterministic Parts A-C + G1 done. Parts D-F blocked on model credentials/budget. Do not fabricate traces; do not mark complete without OpenAI + Anthropic pairs and instructions A/B. |
| Deterministic tool substrate | 003 Unicode security semantic correctness hardening | ready | `plans/implementation/deterministic-tool-substrate/003-unicode-security-correctness-hardening.md` | Preserve Unicode 17 data epoch while fixing skeleton/collision, bidi/script, Rust XID, fuzz/property, normalization/mapping, and generator correctness. |

## Recently closed work (control points)

| Subsystem | Milestone | Status | Controlling evidence |
|---|---|---|---|
| Distribution, update, and release | Eggfetch 0.1.7 updater bump | closed | Archive: `plans/archive/eggfetch-0.1.7-updater-dependency-bump.md` (plan + closure in file). Roadmap history in `plans/archive/roadmap.md`. |
| Distribution, update, and release | Eggfetch 0.2.0 updater adoption | closed | Archive: `plans/archive/eggfetch-0.2.0-updater-adoption.md`; implementation `bfe12d7`; ordinary CI `35734609288`; maintenance `35740940881`. |
| Distribution, update, and release | Eggfetch 0.2.0 adoption closeout corrective | closed | Archive: `plans/archive/eggfetch-0.2.0-adoption-closeout-corrective.md`; implementation `bfe12d7`; maintenance `35740940881` green (MSRV, cargo-deny, native Windows, native macOS). |
| Deterministic tool substrate | Performance campaign + closure corrective | closed | Commits `85e10bf` + `e8067ae`; remote CI `35542158872`; non-gating bench per `architecture/performance.md`. Detail in `plans/archive/roadmap.md`. |
| Distribution, update, and release | Binary distribution (C8 Zig correction `6658702`, v1.2.4 matrix) | closed | Release `v1.2.4`, workflow `33944943782`; installer verification recorded in `plans/archive/roadmap.md`. |

## Blocked work

| Subsystem | Milestone | Blocker |
|---|---|---|
| MCP presentation surface | 03c Parts D-F (model/client traces, instructions A/B, rollout decision) | No provider credentials, subscriptions, or evaluation budget for ~200 model calls (attempted 2026-09-11: `codex`/`claude` CLIs present, no budget). Generated integrations stay on direct; discovery stays explicitly selectable. |
| Deterministic tool substrate | 004 Unicode 18 security data qualification | Hard dependency on Milestone 003 closure; do not mix semantic corrections with Unicode data-epoch changes. |

## Closure work and current control points

- MCP modernization protocol/runtime/discovery implementation is complete
  (`35f7dc2e`, `a5c00b8d`, `77ff57a5`, `121babde`, `40222999`); only the
  `03c` external evidence closure remains active.
- Deterministic tool substrate is reopened for Unicode-security correctness:
  Milestone 003 is ready for handoff and Milestone 004 is blocked on 003
  closure. Harness remains guard-only; distribution remains closed/maintenance.
- The durable rollout gates for `03c` remain: 100% stable-Model coverage,
  retrieval top-1 >=90% / top-3 >=98% / top-5 100%, 0 Model->HarnessOnly
  leaks, discovery/direct byte ratio <=25%, >=40 Model task scenarios
  after containment split, OpenAI pair recorded + scored, Anthropic pair
  recorded + scored, server-instructions A/B recorded.