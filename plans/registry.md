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
| Deterministic tool substrate | active | `plans/subsystems/deterministic-tool-substrate-roadmap.md` | 005 ready | 003/004 remain closed historical dependencies; post-004 review found UTS #39 skeleton/Script_Extensions and Rust identifier-inspect correctness gaps owned by 005. |
| MCP presentation surface | active | `plans/subsystems/mcp-presentation-surface-roadmap.md` | 03c active (deterministic prep done; external evidence blocked) | Blocked on provider credentials / eval budget for OpenAI + Anthropic direct/discovery pairs and instructions A/B. |
| Harness integration and docs | active | `plans/subsystems/harness-integration-roadmap.md` | Guard only; no open milestone | Continuous guard (generate-docs check, parity baseline, context isolation). |
| Distribution, update, and release | active | `plans/subsystems/distribution-update-release-roadmap.md` | M001-M004 closed; M005 blocked / planned | mirrored Eggpack adoption plan registered; waits on Eggpack CI M003d + Build M005; updater/product release policy remains local |

## Dependency-ready implementation plans

| Subsystem | Milestone | Status | Implementation plan | Dependencies / handoff note |
|---|---|---|---|---|
| MCP presentation surface | 03c evaluation closure corrective | active | `plans/implementation/mcp-presentation-surface/003c-evaluation-closure-corrective.md` | Deterministic Parts A-C + G1 done. Parts D-F blocked on model credentials/budget. Do not fabricate traces; do not mark complete without OpenAI + Anthropic pairs and instructions A/B. |
| Deterministic tool substrate | 003 Unicode security semantic correctness hardening | closed | `plans/implementation/deterministic-tool-substrate/003-unicode-security-correctness-hardening.md` | Implemented in `3d67807`; closure `plans/closure/deterministic-tool-substrate/003-status.md`. Data epoch held at Unicode 17.0.0. |
| Deterministic tool substrate | 004 Unicode 18 security data qualification | closed | `plans/implementation/deterministic-tool-substrate/004-unicode18-security-data-qualification.md` | Implemented in `2e3860c`; closure `plans/closure/deterministic-tool-substrate/004-status.md`. Shipped claim: "confusables data: Unicode 18.0.0" (6,712 entries). |
| Deterministic tool substrate | 005 Unicode security standards-conformance corrective | ready | `plans/implementation/deterministic-tool-substrate/005-unicode-security-standards-conformance-corrective.md` | Correct UTS #39 Revision 34 public/internal skeleton semantics, Unicode 18 resolved Script_Extensions, Rust identifier_inspect validation, and residual hazard ownership drift. |

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
| Distribution, update, and release | M005 Eggpack producer adoption | `plans/implementation/distribution-update-release/005-eggpack-producer-adoption-and-live-draft-qualification.md`; Eggpack CI M003d + Build M005 must close; mirrored Eggpack Ecosystem M001 plan is registered. |
| MCP presentation surface | 03c Parts D-F (model/client traces, instructions A/B, rollout decision) | No provider credentials, subscriptions, or evaluation budget for ~200 model calls (attempted 2026-09-11: `codex`/`claude` CLIs present, no budget). Generated integrations stay on direct; discovery stays explicitly selectable. |

## Closure work and current control points

- MCP modernization protocol/runtime/discovery implementation is complete
  (`35f7dc2e`, `a5c00b8d`, `77ff57a5`, `121babde`, `40222999`); only the
  `03c` external evidence closure remains active.
- Deterministic tool substrate Milestones 003 and 004 remain closed historical
  control points, but a post-004 conformance review reopened the Unicode line
  under Milestone 005. 005 is ready for handoff and owns the remaining UTS #39
  skeleton/Script_Extensions correctness plus the Rust identifier-inspect and
  typed-hazard drift findings. Harness remains guard-only (03c external
  evidence still blocked). Distribution M001-M004 remain closed historical control points, while M005 is a blocked corrective/adoption plan for Eggpack producer migration; self-update semantics are not reopened.
- The durable rollout gates for `03c` remain: 100% stable-Model coverage,
  retrieval top-1 >=90% / top-3 >=98% / top-5 100%, 0 Model->HarnessOnly
  leaks, discovery/direct byte ratio <=25%, >=40 Model task scenarios
  after containment split, OpenAI pair recorded + scored, Anthropic pair
  recorded + scored, server-instructions A/B recorded.