# Deterministic Tool Substrate Roadmap

Status: active (guard workstream; Milestones 003–006 closed)

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

A September 2026 Unicode/confusables audit reopened this guard workstream for
correctness hardening. Milestone 003 landed substantial semantic fixes
(skeleton grouping, typed bidi hazards, a consolidated script module, Rust XID
validity in identifier analysis, repaired fuzz/property coverage, fail-closed
generation) and Milestone 004 advanced the generated confusables table to
reproducibly pinned Unicode 18.0.0 (6,712 entries) with an inventoried
mixed-provider data graph.

A post-004 standards-conformance review found residual gaps between those
implementations and UTS #39 Revision 34: the public skeleton omitted
Default_Ignorable removal and bidiSkeleton semantics, mixed-script detection was
still an approximation rather than Script_Extensions/resolved-script-set logic,
and Rust validation was missing from identifier_inspect. Milestone 005 landed
that corrective architecture and is retained as immutable historical closure
evidence.

A post-005 edge-case review found three narrower conformance defects: Unicode
18 DerivedBidiClass `@missing` defaults are currently discarded, the
multi-paragraph bidi skeleton assumes paragraph-length output from a
whole-text levels API and can fall back to all-LTR levels, and
Script_Extensions `Unknown`/Zzzz is incorrectly treated as the ALL identity
reserved for Common/Inherited. Milestone 006 corrected only these residual
defects and is closed; the substrate is guard-only.

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
    +--> Hot-path evidence (closed; future patches are corrective-only)
    |
    `--> Unicode security semantic hardening (003, closed)
             |
             `--> Unicode 18 data qualification (004, closed)
                      |
                       `--> UTS #39 standards-conformance corrective (005, closed)
                                |
                                `--> Unicode conformance edge-case corrective (006, closed)
```

Milestones 003, 004, 005, and 006 are closed historical dependencies. The
substrate workstream is guard-only with no open milestone.

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


### Milestone 3 — Unicode security semantic correctness hardening

Class: invariant

Objective: make confusable collision, bidi/invisible classification, script
analysis, identifier validity, normalization diagnostics, and Unicode
verification conform to their documented standards while preserving the 1.x
surface and Unicode 17 data epoch.

Dependencies: registry/typed-first guards (continuous; already satisfied).

Deliverable boundary: whole-string version-pinned confusable skeletons,
typed/non-presentation bidi classification, one authoritative script-property
path, Unicode-correct Rust XID validity, repaired property/fuzz coverage, and a
fail-closed generated-data checker. No Unicode data-version bump.

User or operator value: materially fewer false-positive/false-negative Unicode
security findings and trustworthy regression/fuzz evidence without losing any
existing tool.

Exit conditions: implementation plan
`plans/implementation/deterministic-tool-substrate/003-unicode-security-correctness-hardening.md`
is implemented and closed with focused Unicode evidence plus the ordinary merge
gate.

Deferred work: Unicode 18 source-data refresh (Milestone 004).

### Milestone 4 — Unicode 18 security data qualification

Class: infrastructure

Objective: after Milestone 003 closes, advance the pinned UTS #39
confusables/security data to Unicode 18.0.0 with auditable provenance and
qualified Unicode-data dependency epochs.

Dependencies: Milestone 003 (hard).

Deliverable boundary: Unicode 18 confusables source/checksum, regenerated
static assets, provider-version inventory, changed-mapping regressions, and
accurate documentation of mixed Unicode data epochs if any. No algorithm,
ToolSpec, profile, or protocol redesign.

User or operator value: current Unicode security mappings with reproducible
provenance and no ambiguity about which Unicode version each security-relevant
provider implements.

Exit conditions: implementation plan
`plans/implementation/deterministic-tool-substrate/004-unicode18-security-data-qualification.md`
is implemented after 003 closure, generated assets reproduce exactly, relevant
Unicode differences are reviewed, and the ordinary merge/release qualification
gates are green.

Deferred work: IDNA/UTS #46 and any broader restriction-level product policy
remain out of scope.


### Milestone 5 — Unicode security standards-conformance corrective

Class: invariant

Objective: align the public skeleton and mixed-script semantics with UTS #39
Revision 34, correct the missed Rust identifier-inspection validation path, and
finish typed Unicode-property ownership without changing the public tool
surface or Unicode 18 confusables data.

Dependencies: Milestones 003 and 004 (closed).

Deliverable boundary: UTS #39 internal + bidi skeleton semantics,
Default_Ignorable handling, Unicode 18 Script_Extensions/resolved script sets,
shared Rust XID/keyword validation, consolidated security predicates, and
conformance fixtures. No new MCP capability or restriction-level product
policy.

User or operator value: the Unicode/security APIs mean what their documentation
and standards references say, including RTL/default-ignorable and
Script_Extensions cases that the previous corrective still missed.

Exit conditions: implementation plan
`plans/implementation/deterministic-tool-substrate/005-unicode-security-standards-conformance-corrective.md`
is implemented and 005 closure record
`plans/closure/deterministic-tool-substrate/005-status.md` demonstrates
normative/reference vectors, the ordinary merge gate, and green remote CI.

Deferred work: IDNA/UTS #46 and user-facing restriction-level policy remain out
of scope.


### Milestone 6 — Unicode conformance edge-case corrective

Class: invariant

Objective: correct Unicode 18 Bidi_Class `@missing` defaults,
multi-paragraph bidi-skeleton level scoping, and Unknown/Zzzz resolved-script
semantics while preserving the Milestone 005 architecture and public surface.

Dependencies: Milestones 003, 004, and 005 (closed).

Deliverable boundary: ordered UAX #44 Bidi_Class default overlays,
paragraph-local UAX #9 L1/L2 handling for public skeletons, literal
`Script_Extensions={Unknown}` intersection semantics, and regression fixtures
for all three defects. No dependency, Unicode epoch, ToolSpec, schema, or
product-policy change.

User or operator value: edge-case inputs receive the same Unicode-security
classification promised by the current Unicode 18 / UTS #39 implementation,
including default-only RTL code points, multiple paragraphs, and private-use or
unassigned script values.

Exit conditions: implementation plan
`plans/implementation/deterministic-tool-substrate/006-unicode-conformance-edge-case-corrective.md`
is implemented and a new 006 closure record demonstrates baseline-failing
fixtures, generator freshness, the ordered merge gate, and green remote CI.

Deferred work: IDNA/UTS #46, restriction-level product policy, and unrelated
Unicode provider version refreshes remain out of scope.

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

Milestone 005's architecture remains the target: compact generated Unicode 18
properties plus `unicode-bidi` algorithm-only through the custom data source.
Milestone 006 must correct data interpretation and indexing inside that design,
not replace it.

The main implementation risks are silent fallback behavior: treating a
Bidi_Class miss as L, treating a per-paragraph vector mismatch as LTR, or
treating Unknown as ALL. The corrective should make each condition explicit
and regression-tested.

The current Unicode 18 confusables/property sources are not themselves in
question. No dependency change is expected.

A new public restriction-level, IDNA, or enforcement policy would be a separate
product decision and remains out of scope. A new utility category or
composition-layer change still requires an ADR before a milestone plan.

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
| 003 Unicode security semantic correctness hardening | closed | `plans/implementation/deterministic-tool-substrate/003-unicode-security-correctness-hardening.md` | `plans/closure/deterministic-tool-substrate/003-status.md` (`3d67807`) | — |
| 004 Unicode 18 security data qualification | closed | `plans/implementation/deterministic-tool-substrate/004-unicode18-security-data-qualification.md` | `plans/closure/deterministic-tool-substrate/004-status.md` (`2e3860c`) | — |
| 005 Unicode security standards-conformance corrective | closed | `plans/implementation/deterministic-tool-substrate/005-unicode-security-standards-conformance-corrective.md` | `plans/closure/deterministic-tool-substrate/005-status.md` | — |
| 006 Unicode conformance edge-case corrective | closed | `plans/implementation/deterministic-tool-substrate/006-unicode-conformance-edge-case-corrective.md` | `plans/closure/deterministic-tool-substrate/006-status.md` (`83de61a`) | — |