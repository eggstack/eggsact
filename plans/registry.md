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
| Distribution, update, and release | active | `plans/subsystems/distribution-update-release-roadmap.md` | M001-M005 closed; M005a planned | Eggpack is producer authority (cutover at `d8014cf`, pinned to Eggpack M003g `e5c81f2`); live draft qualified and published for v1.2.7 (`plans/closure/distribution-update-release/005-status.md`). M005a tracks Windows artifact determinism, the one open rerun-reuse condition |

## Dependency-ready implementation plans

| Subsystem | Milestone | Status | Implementation plan | Dependencies / handoff note |
|---|---|---|---|---|
| Distribution, update, and release | M005 Eggpack producer adoption | closed (conditionally) | `plans/implementation/distribution-update-release/005-eggpack-producer-adoption-and-live-draft-qualification.md` | Closure `plans/closure/distribution-update-release/005-status.md`; cutover `d8014cf`; live draft run 36652731202 (15 assets, receipt `RE_kwDOTGg0Mc4X0gk6`); v1.2.7 published with installer/latest/update smoke verified. Open condition: M005a (Windows byte-reproducibility) |
| Deterministic tool substrate | 008 prompt wire compatibility and layering-guard corrective | closed | `plans/implementation/deterministic-tool-substrate/008-prompt-wire-compatibility-and-layering-guard-corrective.md` | Implemented in `475fc19`; closure `plans/closure/deterministic-tool-substrate/008-status.md`. Pre-007 null/string/array projection restored; generic 86-handler guard green. |
| Deterministic tool substrate | 007 typed-core ownership and adapter deduplication | closed | `plans/implementation/deterministic-tool-substrate/007-typed-core-ownership-and-adapter-deduplication.md` | Implemented in `237afb5` + `7176857`; closure `plans/closure/deterministic-tool-substrate/007-status.md`. Zero handler-to-handler composition; typed prompt core authoritative. |
| MCP presentation surface | 03c evaluation closure corrective | blocked | `plans/implementation/mcp-presentation-surface/003c-evaluation-closure-corrective.md` | Closure `plans/closure/mcp-presentation-surface/003c-status.md` (blocked). Deterministic Parts A-C + G1 re-verified 2026-09-30 (76/76 targets, 88+9 fixtures, top-1 96.59%, 48+4 scenarios, 5.44% bytes). Parts D-F blocked on model credentials/budget. Do not fabricate traces; do not mark complete without OpenAI + Anthropic pairs and instructions A/B. |
| Deterministic tool substrate | 003 Unicode security semantic correctness hardening | closed | `plans/implementation/deterministic-tool-substrate/003-unicode-security-correctness-hardening.md` | Implemented in `3d67807`; closure `plans/closure/deterministic-tool-substrate/003-status.md`. Data epoch held at Unicode 17.0.0. |
| Deterministic tool substrate | 004 Unicode 18 security data qualification | closed | `plans/implementation/deterministic-tool-substrate/004-unicode18-security-data-qualification.md` | Implemented in `2e3860c`; closure `plans/closure/deterministic-tool-substrate/004-status.md`. Shipped claim: "confusables data: Unicode 18.0.0" (6,712 entries). |
| Deterministic tool substrate | 005 Unicode security standards-conformance corrective | closed | `plans/implementation/deterministic-tool-substrate/005-unicode-security-standards-conformance-corrective.md` | Closure `plans/closure/deterministic-tool-substrate/005-status.md`. Historical control point; residual edge cases are owned by 006. |
| Deterministic tool substrate | 006 Unicode conformance edge-case corrective | closed | `plans/implementation/deterministic-tool-substrate/006-unicode-conformance-edge-case-corrective.md` | Closure `plans/closure/deterministic-tool-substrate/006-status.md`. Ordered Bidi_Class @missing defaults, paragraph-local bidi levels, Zzzz-as-constraint resolved sets. |

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
| Distribution, update, and release | M005a deterministic Windows artifacts | blocked | `plans/implementation/distribution-update-release/005a-deterministic-windows-release-artifacts.md`; recorded from M005 closure. Not blocked: implementable now, but it is the last open M005 condition and should be sequenced with the next release, not run mid-release |
| MCP presentation surface | 03c Parts D-F (model/client traces, instructions A/B, rollout decision) | No provider credentials, subscriptions, or evaluation budget for ~200 model calls (attempted 2026-09-11 and re-attempted 2026-09-30: `codex` 0.159.2 / `claude` 2.1.280 CLIs present, no budget/approval). Closure `plans/closure/mcp-presentation-surface/003c-status.md` records the blocked disposition. Generated integrations stay on direct; discovery stays explicitly selectable. |

## Closure work and current control points

- MCP modernization protocol/runtime/discovery implementation is complete
  (`35f7dc2e`, `a5c00b8d`, `77ff57a5`, `121babde`, `40222999`); only the
  `03c` external evidence closure remains blocked
  (`plans/closure/mcp-presentation-surface/003c-status.md`; deterministic
  prep re-verified 2026-09-30, no future plan unblocked — M005a stays
  independent).
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
  `eggsact update` smoke all verified. The single open condition is M005a
  (deterministic Windows artifacts for byte-identical rerun reuse);
  self-update semantics remain eggsact/Eggup-owned and were not migrated.
- The durable rollout gates for `03c` remain: 100% stable-Model coverage,
  retrieval top-1 >=90% / top-3 >=98% / top-5 100%, 0 Model->HarnessOnly
  leaks, discovery/direct byte ratio <=25%, >=40 Model task scenarios
  after containment split, OpenAI pair recorded + scored, Anthropic pair
  recorded + scored, server-instructions A/B recorded.