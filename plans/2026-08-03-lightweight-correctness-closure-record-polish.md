# Lightweight Correctness Closure-Record Polish Pass

## Status

- **Status:** ready for implementation
- **Repository:** `eggstack/eggsact`
- **Target branch:** `main`
- **Planning baseline:** `67ecdb8ac07149d5e469db89d1a0c079dfb1ba93`
- **Parent roadmap:** `plans/2026-07-31-lightweight-correctness-simplification-roadmap.md`
- **Primary closure plan:** `plans/2026-08-01-lightweight-correctness-final-closure-pass.md`
- **Scope:** documentation-only reconciliation of completion records, measurements, acceptance criteria, commit identities, and CI evidence
- **Expected implementation shape:** one bounded documentation commit touching only the existing July 31/August 1 planning records

## Purpose

The lightweight correctness and simplification implementation is materially complete. The remaining defects are in the planning record rather than production behavior.

The current documents contain several inconsistencies:

1. the final closure plan still says `ready for implementation` while its completion record says `complete`;
2. the final closure plan leaves every acceptance checkbox unchecked despite recording successful implementation and verification;
3. several records use ambiguous or abbreviated commit references where exact identities are required;
4. Phase 1 and Phase 3 describe the corrective closure work without recording its exact SHA;
5. the final closure record labels a documentation commit as the final implementation SHA;
6. Phase 4 checks off measurement criteria that its own completion record says were not experimentally measured;
7. Phase 4 says remote CI is both complete and `pending push`;
8. the roadmap records final-only CLI timing where its measurement discipline calls for baseline and final values;
9. the documents do not consistently distinguish implementation, packaging, documentation, completion-record, and repository-head commits.

This pass must make the planning record internally consistent without reopening the completed production work or creating another verification framework.

---

# Required outcome

After this pass:

1. every affected plan has one unambiguous status;
2. acceptance criteria accurately reflect evidence already present or gathered during this polish pass;
3. commit references use full 40-character SHAs where the record is intended to be exact;
4. implementation commits are not mislabeled as documentation commits or vice versa;
5. the corrective implementation SHA is recorded in Phase 1, Phase 3, the final closure plan, and the roadmap where relevant;
6. Phase 4 candidate dispositions distinguish measured changes, feasibility-only review, explicit deferral, and not-evaluated candidates;
7. no checklist says a candidate was measured when no measurement exists;
8. remote CI is recorded once with the successful ordinary run and its three job conclusions;
9. CLI timing is either presented as a genuine same-host baseline/final comparison or explicitly labeled final-only with no performance conclusion;
10. binary-size measurements identify the exact compared SHAs and profile;
11. the default-install inventory remains recorded as only the `eggsact` binary;
12. the roadmap closes with exact implementation identities and no unsupported claim;
13. no production code, workflow, dependency, schema, generated artifact, or release process changes;
14. no second evidence-only cleanup plan is required.

---

# Hard constraints and non-goals

This pass must not:

- modify Rust or Python source;
- modify `Cargo.toml`, `Cargo.lock`, build scripts, generated assets, or schemas;
- add, remove, or change tools;
- change regex, calculator, TOML, MCP, dispatch, timeout, cancellation, or output-budget behavior;
- alter test coverage;
- add or modify GitHub Actions workflows;
- add benchmark scripts, result directories, artifacts, or permanent evidence infrastructure;
- rerun the full test suite solely because Markdown planning files changed;
- automate crates.io publication;
- change release cadence;
- rewrite Git history;
- squash or amend already-pushed implementation commits;
- create a new roadmap or phase;
- invent baseline measurements;
- infer a successful CI result from an unchecked status field;
- mark a criterion complete solely because a completion paragraph claims success;
- require a plan file to contain the SHA of the same commit that creates it;
- create a follow-up commit solely to self-record the polish commit SHA.

A one-time local measurement command or temporary worktree is allowed. It must not be committed.

---

# Canonical commit identity map

Use these exact identities unless repository inspection proves they are incorrect:

| Role | Commit |
|---|---|
| Phase 1 implementation | `98d3aae00efc29436af808c430da6766ea76ebf6` |
| Phase 2 implementation | `0a3ace9e21853e4ded7f0a8c2a9bcb9ab4f1cc94` |
| Phase 2 documentation | `e009d86b9b0efcce89d5f43c2ec86efcc8fe4614` |
| Phase 2 gap fix | `25c4893455719027cdc889a853039a918611ec65` |
| Phase 3 dispatch consolidation | `63bac39b87596e2f7721c4042f369afe92a41bcd` |
| Phase 3 calculator/test-hook completion | `021795bc72eee444510ff9f4472e16a611418b6d` |
| Original Phase 4 implementation | `a8dc5e69e8ce3d38c17f7cf944d8967408b9701a` |
| Phase 4 documentation | `3d876bcfd447d2d6a642f461e7f6960c6987cd2f` |
| Final closure plan | `11aaa592a35e87e253f25eb86373ead954bf51a9` |
| Corrective implementation and dev-tool gating | `1cb0ce581849b540e41fd8cc5ae130c63c449727` |
| Closure documentation reconciliation | `2f55d805edc4c7987dee367b7612819fe521f60a` |
| Closure completion-record update | `67ecdb8ac07149d5e469db89d1a0c079dfb1ba93` |

Use role-specific labels. Do not use an ambiguous field called only `Final SHA` when it could mean any of these:

- final production implementation;
- final Phase 4 implementation;
- closure documentation;
- completion-record update;
- current repository head.

Preferred labels are:

```text
Corrective implementation SHA
Phase 4 implementation SHA
Closure documentation SHA
Completion-record SHA
Pre-polish main SHA
```

The resulting polish commit is visible in Git history. Do not introduce a second commit merely to write its own SHA into these plans.

---

# Target files

The implementation agent must inspect all five existing records and edit only those that require correction:

```text
plans/2026-07-31-lightweight-correctness-simplification-roadmap.md
plans/2026-07-31-phase-1-regex-and-mcp-contract-repairs.md
plans/2026-07-31-phase-2-deterministic-output-and-toml-corrections.md
plans/2026-07-31-phase-3-dispatch-and-runtime-simplification.md
plans/2026-07-31-phase-4-measured-footprint-reduction-and-closure.md
plans/2026-08-01-lightweight-correctness-final-closure-pass.md
```

This list contains six files because the parent roadmap and five phase/closure records form one consistency set.

Do not edit implementation documentation such as `README.md`, `architecture/`, `docs/`, `AGENTS.md`, or generated output unless inspection finds a direct contradiction created by the planning text. No such implementation-document defect is currently confirmed.

---

# Workstream 0 — establish a clean documentation baseline

Before editing:

```bash
git fetch origin main --prune
git switch main
git reset --hard origin/main
git status --short
git rev-parse HEAD
git log --oneline --decorate -20
```

Expected starting head:

```text
67ecdb8ac07149d5e469db89d1a0c079dfb1ba93
```

If `main` has advanced, inspect every intervening commit before applying this plan. If a later commit already reconciles these records, reduce or cancel the pass rather than duplicating it.

Create a small scratch matrix outside the repository:

| Document | Status field | Checklist state | Commit labels | Measurement state | CI state |
|---|---|---|---|---|---|
| Roadmap | inspect | inspect | inspect | inspect | inspect |
| Phase 1 | inspect | inspect | inspect | n/a | inspect |
| Phase 2 | inspect | inspect | inspect | n/a | inspect |
| Phase 3 | inspect | inspect | inspect | n/a | inspect |
| Phase 4 | inspect | inspect | inspect | inspect | inspect |
| Final closure | inspect | inspect | inspect | inspect | inspect |

Do not commit this scratch matrix.

## Acceptance criteria

- The worktree begins clean.
- The actual starting SHA is recorded in the eventual commit message.
- Intervening commits, if any, are reviewed before editing.
- No file outside the six planning records is selected without a confirmed contradiction.

---

# Workstream 1 — reconcile measurement evidence

## Existing evidence

The current records contain:

```text
Environment: Linux x86_64
rustc/cargo: 1.97.1 / 1.97.1
Build command: cargo build --release --locked --bin eggsact
Release binary before: 12,856,752 bytes
Release binary after: 12,856,656 bytes
Final-only CLI medians:
  --help: 15.2 ms
  --version: 15.3 ms
  2+2: 501.0 ms
  thirty plus five: 499.7 ms
Default install inventory: eggsact only
```

The binary-size comparison has before/after values. The CLI timing record does not currently contain paired baseline/final values.

## Preferred bounded correction

Use temporary detached worktrees to collect a same-host paired comparison without modifying repository files:

```bash
BASE=63bac39b87596e2f7721c4042f369afe92a41bcd
FINAL=1cb0ce581849b540e41fd8cc5ae130c63c449727
TMP=$(mktemp -d)

git worktree add --detach "$TMP/base" "$BASE"
git worktree add --detach "$TMP/final" "$FINAL"

(
  cd "$TMP/base"
  cargo build --release --locked --bin eggsact
)

(
  cd "$TMP/final"
  cargo build --release --locked --bin eggsact
)
```

Use the same machine, shell environment, Rust toolchain, release profile, and measurement script for both binaries.

A temporary standard-library Python script is acceptable for repeatable timing. It must not be added to the repository. Measure at least ten cold process invocations per command and report the median:

```text
--help
--version
2+2
thirty plus five
```

Record:

```text
baseline SHA
final implementation SHA
OS/architecture
rustc --version
cargo --version
build command
sample count
median baseline milliseconds
median final milliseconds
absolute delta
percentage delta only when baseline is nonzero
```

Clean up temporary worktrees:

```bash
git worktree remove --force "$TMP/base"
git worktree remove --force "$TMP/final"
rm -rf "$TMP"
```

## Allowed fallback

If the historical baseline cannot be built reproducibly on the same host and toolchain:

1. do not substitute a measurement from another host;
2. do not describe the final-only timings as a before/after comparison;
3. retain the final-only values with an explicit label;
4. state why the baseline is unavailable;
5. remove or rewrite any acceptance criterion claiming paired startup improvement;
6. preserve the architectural fact that non-MCP paths no longer construct Tokio, but do not convert that fact into a timing claim.

Documentation truthfulness is more important than forcing a measurement.

## Binary-size terminology

Identify exactly what the two size values compare. If they compare pre-gating and post-gating builds, label them that way. Do not imply they isolate all Phase 4 changes unless the compared SHAs prove that.

If a new same-host build produces different byte counts, use the new values only when both sides are rebuilt under the same toolchain/profile. Do not mix historical and current numbers.

## Install inventory

Reconfirm the final implementation default install using a temporary root:

```bash
cargo install --path "$TMP/final" --locked --root "$TMP/install"
find "$TMP/install/bin" -maxdepth 1 -type f -print
```

Expected inventory:

```text
eggsact
```

Do not enable `dev-tools` for the default-install audit.

## Acceptance criteria

- Measurement labels identify exact SHAs and profiles.
- CLI timing is either genuinely paired or explicitly final-only.
- No performance percentage is calculated from incomparable data.
- Binary sizes are not mixed across hosts or toolchains.
- The default install inventory is reconfirmed or accurately marked as prior evidence.
- No measurement script or result artifact is committed.

---

# Workstream 2 — repair the final closure plan

Target:

```text
plans/2026-08-01-lightweight-correctness-final-closure-pass.md
```

## Status

Change the top-level status from `ready for implementation` to `complete`.

Retain the original planning baseline and purpose as historical context. Add a concise completion note near the status block if needed so a reader does not mistake the original baseline for the current repository head.

## Acceptance checklist

Review each unchecked item against code, tests, measurement evidence, and CI.

For criteria proven by the corrective implementation or existing tests, change `[ ]` to `[x]`.

For criteria whose wording overstates available evidence, rewrite the criterion before checking it. Examples:

```text
Bad:
- [ ] Exact baseline and final process-start measurements are recorded.

Acceptable with paired data:
- [x] Same-host baseline and final process-start medians are recorded for the four required CLI paths.

Acceptable without paired data:
- [x] Final-only process-start medians are labeled as final-only, and no startup-improvement claim is made.
```

Do not leave the plan `complete` with an entirely unchecked acceptance section.

## Commit identities

Replace abbreviated or ambiguous fields with the canonical identity map.

At minimum record:

```text
Starting main SHA: 11aaa592a35e87e253f25eb86373ead954bf51a9
Corrective implementation SHA: 1cb0ce581849b540e41fd8cc5ae130c63c449727
Closure documentation SHA: 2f55d805edc4c7987dee367b7612819fe521f60a
Completion-record SHA: 67ecdb8ac07149d5e469db89d1a0c079dfb1ba93
Pre-polish main SHA: 67ecdb8ac07149d5e469db89d1a0c079dfb1ba93
```

Do not call `2f55d805...` the final implementation SHA. It is a documentation reconciliation commit.

## CI record

The existing ordinary run `30688082724` has three successful jobs:

```text
Linux correctness — success
Check (windows-latest) — success
Check (macos-latest) — success
```

Before attributing the run to a specific commit, confirm the run metadata or commit association. Record the run once. Do not add a run ledger.

## Closure statement

Retain the substantive closure statement, but remove any claim that exceeds the reconciled evidence.

The final statement should distinguish:

- production correctness is closed;
- ordinary CI passed;
- default installation exposes only the intended binary;
- release remains manual;
- no production follow-up is authorized by this polish pass.

## Acceptance criteria

- Top-level status and completion record agree.
- Every checked item is supported.
- No unsupported measurement item remains checked.
- Exact commit identities are role-labeled.
- CI is recorded without duplication.
- No self-referential polish SHA is required.

---

# Workstream 3 — repair phase completion records

## Phase 1

Target:

```text
plans/2026-07-31-phase-1-regex-and-mcp-contract-repairs.md
```

Replace the prose-only corrective reference with:

```text
Corrective closure pass commit: 1cb0ce581849b540e41fd8cc5ae130c63c449727
```

Keep the original Phase 1 implementation SHA:

```text
98d3aae00efc29436af808c430da6766ea76ebf6
```

Do not imply that all capture fixes were present in the original Phase 1 implementation commit.

## Phase 2

Target:

```text
plans/2026-07-31-phase-2-deterministic-output-and-toml-corrections.md
```

Normalize the abbreviated implementation SHA to:

```text
0a3ace9e21853e4ded7f0a8c2a9bcb9ab4f1cc94
```

Retain:

```text
Documentation commit: e009d86b9b0efcce89d5f43c2ec86efcc8fe4614
Gap fix commit: 25c4893455719027cdc889a853039a918611ec65
```

Add the documentation commit only if it is absent from the completion record. Do not rewrite the detailed map/TOML evidence.

## Phase 3

Target:

```text
plans/2026-07-31-phase-3-dispatch-and-runtime-simplification.md
```

Replace:

```text
Direct-dispatch corrective commit: corrective closure pass commit
```

with:

```text
Direct-dispatch corrective commit: 1cb0ce581849b540e41fd8cc5ae130c63c449727
```

Retain the two original Phase 3 implementation identities:

```text
63bac39b87596e2f7721c4042f369afe92a41bcd
021795bc72eee444510ff9f4472e16a611418b6d
```

## Acceptance criteria

- Phase 1 distinguishes original implementation from corrective capture/policy work.
- Phase 2 uses full SHAs for implementation, documentation, and gap fix.
- Phase 3 names the exact direct-dispatch corrective commit.
- No phase claims that a later corrective change existed in its original implementation commit.
- Existing technical evidence remains intact.

---

# Workstream 4 — repair Phase 4 candidate and verification records

Target:

```text
plans/2026-07-31-phase-4-measured-footprint-reduction-and-closure.md
```

## Candidate table

Retain the distinction already present in the result table:

```text
measured/accepted
accepted install-surface correction
not experimentally evaluated
feasibility only
deferred
```

Do not normalize all candidates to `measured`.

## Acceptance checklist wording

The current checklist conflicts with the completion record. Rewrite the affected criteria to match the intended success condition.

Required replacements should express these facts:

- the release-profile candidate received a truthful `not experimentally evaluated` disposition;
- the confusables representation received a feasibility-only/deferred disposition;
- TOML consolidation received a feasibility-only rejection/defer disposition;
- schema caching and trivial regex cleanup remain explicitly deferred for lack of value/evidence;
- accepted Tokio/runtime/install changes have evidence;
- no unmeasured optimization is described as a measured win.

Do not leave these contradictory combinations:

```text
[x] Release profile is measured rather than assumed.
Completion: release profile not experimentally evaluated.

[x] Confusables static representation is evaluated and measured.
Completion: feasibility disposition only.
```

## SHA labels

Replace ambiguous `Final SHA` with role-specific fields. At minimum:

```text
Phase 4 implementation SHA: a8dc5e69e8ce3d38c17f7cf944d8967408b9701a
Phase 4 documentation SHA: 3d876bcfd447d2d6a642f461e7f6960c6987cd2f
Corrective packaging SHA: 1cb0ce581849b540e41fd8cc5ae130c63c449727
Closure documentation SHA: 2f55d805edc4c7987dee367b7612819fe521f60a
Completion-record SHA: 67ecdb8ac07149d5e469db89d1a0c079dfb1ba93
```

The baseline SHA remains:

```text
63bac39b87596e2f7721c4042f369afe92a41bcd
```

## Measurements

Replace the current timing line with either:

- a baseline/final table produced by Workstream 1; or
- an explicit final-only table with no claim of measured startup improvement.

Preserve the architectural observation that non-MCP paths no longer create a Tokio runtime.

Do not attribute the approximately 96-byte change to Tokio feature narrowing unless the compared SHAs isolate that change. Use narrower wording such as `recorded pre-gating/post-gating release binary difference` when isolation is uncertain.

## Remote CI

Replace `Remote CI: pending push` with the confirmed ordinary CI result after verifying run association.

Do not add maintenance/parity workflow claims.

## Acceptance criteria

- Phase 4 status remains complete only after contradictions are removed.
- Candidate table and checklist use the same evidence vocabulary.
- Deferred candidates are not presented as failures.
- Unmeasured candidates are not presented as measured.
- CI is no longer both complete and pending.
- Binary and timing labels are precise.

---

# Workstream 5 — repair the parent roadmap

Target:

```text
plans/2026-07-31-lightweight-correctness-simplification-roadmap.md
```

## Commit list

Replace abbreviated implementation references with full SHAs or a concise role-based table.

Include the corrective implementation separately rather than pretending it was part of the original Phase 1 or Phase 3 commits:

```text
Corrective closure implementation: 1cb0ce581849b540e41fd8cc5ae130c63c449727
```

## Measurement discipline

The roadmap requires:

```text
cold CLI timing before/after
```

Therefore use one of two truthful outcomes:

1. add the paired values from Workstream 1; or
2. revise the closure summary to state final-only timing and no before/after startup conclusion.

Do not leave a final-only timing line under a before/after requirement without qualification.

## Phase 4 statement

Keep the narrow architectural outcome:

- Tokio features were narrowed;
- runtime construction moved to the MCP path;
- default installation exposes only `eggsact` after dev-tool gating.

Qualify the binary-size statement according to the exact compared builds.

## Closure statement

The roadmap may remain complete. This polish pass is not a fifth implementation phase.

The final statement should say that the production line is closed and that this pass only reconciled records. Do not authorize new optimization work.

## Acceptance criteria

- Roadmap commit identities match phase records.
- Roadmap measurements match Phase 4 and final closure records.
- Corrective work is separately identified.
- No final-only metric is presented as paired evidence.
- The roadmap remains closed without adding a phase.

---

# Workstream 6 — cross-document consistency sweep

After edits, run targeted searches:

```bash
rg -n "ready for implementation|pending push|pending commit|Final SHA|corrective closure pass commit" \
  plans/2026-07-31-lightweight-correctness-simplification-roadmap.md \
  plans/2026-07-31-phase-1-regex-and-mcp-contract-repairs.md \
  plans/2026-07-31-phase-2-deterministic-output-and-toml-corrections.md \
  plans/2026-07-31-phase-3-dispatch-and-runtime-simplification.md \
  plans/2026-07-31-phase-4-measured-footprint-reduction-and-closure.md \
  plans/2026-08-01-lightweight-correctness-final-closure-pass.md
```

Every remaining match must be intentional historical prose, not an active status or completion field.

Check unchecked criteria:

```bash
rg -n -- "- \[ \]" \
  plans/2026-07-31-lightweight-correctness-simplification-roadmap.md \
  plans/2026-07-31-phase-1-regex-and-mcp-contract-repairs.md \
  plans/2026-07-31-phase-2-deterministic-output-and-toml-corrections.md \
  plans/2026-07-31-phase-3-dispatch-and-runtime-simplification.md \
  plans/2026-07-31-phase-4-measured-footprint-reduction-and-closure.md \
  plans/2026-08-01-lightweight-correctness-final-closure-pass.md
```

An unchecked item may remain only when:

- the plan is intentionally historical and the item is explicitly superseded; or
- the wording states that the item was not required or was unavailable.

Prefer rewriting stale criteria over leaving unexplained unchecked boxes in a `complete` plan.

Check exact corrective SHA coverage:

```bash
rg -n "1cb0ce581849b540e41fd8cc5ae130c63c449727" plans/
```

It should appear in the relevant Phase 1, Phase 3, Phase 4, roadmap, and final closure records.

Review the final diff:

```bash
git diff --check
git diff --stat
git diff -- plans/
```

## Acceptance criteria

- No active `pending` placeholder remains.
- No ambiguous `Final SHA` label remains in the affected records.
- No unexplained unchecked checklist remains under a complete status.
- The same measurement values and labels appear across all records.
- The same commit role maps to the same SHA everywhere.
- The diff contains planning Markdown only.

---

# Verification policy

This is a documentation-only pass. Verification must remain proportionate.

Required:

```bash
git diff --check
rg -n "ready for implementation|pending push|pending commit|Final SHA|corrective closure pass commit" <affected files>
rg -n -- "- \[ \]" <affected files>
```

Also confirm:

```bash
git diff --name-only
```

Only the six target planning files should be modified by the implementation commit.

Do not rerun thousands of unit tests for Markdown-only changes. Existing ordinary CI run `30688082724` already records successful Linux correctness, Windows checking, and macOS checking for the completed implementation line. If ordinary CI automatically runs for the polish commit, allow it to complete normally, but do not create another documentation commit merely to record that run.

Run generated-doc checking only if a non-plan documentation file is unexpectedly changed. The expected pass does not touch generated documentation.

---

# Commit and push sequence

Use one implementation commit:

```text
docs: reconcile lightweight correctness closure records
```

Before committing:

```bash
git status --short
git diff --check
git diff --stat
git diff --name-only
```

After committing:

```bash
git show --stat --oneline HEAD
git diff --exit-code HEAD^ -- . ':!plans/**'
git push origin main
```

The exclusion check must show no changes outside `plans/`.

Do not amend implementation history. Do not create a second commit solely to record the SHA of the first polish commit.

---

# Execution sequence for a smaller implementation agent

Execute in this order:

1. synchronize to current `origin/main`;
2. confirm the current head and clean worktree;
3. inspect intervening commits after `67ecdb8` if any;
4. build the six-document consistency matrix;
5. confirm the canonical commit identity map from Git history;
6. verify ordinary CI run `30688082724` and its commit association;
7. attempt the bounded paired measurement using temporary worktrees;
8. if paired measurement is unavailable, select the explicit final-only fallback;
9. reconfirm default install inventory without `dev-tools`;
10. correct the final closure plan status and acceptance checklist;
11. correct final closure commit labels and measurements;
12. add exact corrective SHA to Phase 1;
13. normalize Phase 2 implementation/documentation/gap-fix SHAs;
14. add exact direct-dispatch corrective SHA to Phase 3;
15. reconcile Phase 4 candidate/checklist terminology;
16. replace Phase 4 pending CI text;
17. replace ambiguous Phase 4 SHA labels;
18. reconcile roadmap commit and measurement summaries;
19. run the cross-document searches;
20. inspect the entire Markdown diff;
21. confirm only the six planning records changed;
22. create one documentation commit;
23. push to `origin/main`;
24. stop.

Do not use this pass to investigate new performance candidates or production cleanup.

---

# Required acceptance checklist

## Scope

- [ ] Only the six targeted planning records are modified.
- [ ] No production code or configuration changes.
- [ ] No workflow or release changes.
- [ ] No permanent measurement infrastructure.

## Status and checklists

- [ ] Final closure top-level status is `complete`.
- [ ] Final closure completion record also says `complete`.
- [ ] Proven final-closure criteria are checked.
- [ ] Unavailable or unmeasured evidence is labeled truthfully.
- [ ] Phase 4 checklist agrees with its candidate dispositions.
- [ ] No complete plan has unexplained active unchecked criteria.

## Commit identities

- [ ] Phase 1 records the exact corrective SHA.
- [ ] Phase 2 records full implementation, documentation, and gap-fix SHAs.
- [ ] Phase 3 records the exact direct-dispatch corrective SHA.
- [ ] Phase 4 distinguishes implementation, packaging, documentation, and completion-record SHAs.
- [ ] Final closure distinguishes implementation, documentation, and completion-record SHAs.
- [ ] Roadmap includes the corrective implementation separately.
- [ ] No ambiguous active `Final SHA` field remains.

## Measurements

- [ ] Binary-size comparison identifies exact compared builds.
- [ ] CLI timing is paired or explicitly final-only.
- [ ] Same-host/toolchain/profile constraints are recorded.
- [ ] No unsupported performance conclusion remains.
- [ ] Default install inventory records only `eggsact`.

## CI and closure

- [ ] Ordinary CI run and three successful jobs are recorded once.
- [ ] `Remote CI: pending push` is removed.
- [ ] Release remains manual.
- [ ] The roadmap remains closed.
- [ ] No new implementation phase is created.
- [ ] No second evidence-only commit is required.

## Verification

- [ ] `git diff --check` passes.
- [ ] Placeholder search has no active unresolved result.
- [ ] Unchecked-checkbox search has no unexplained active result.
- [ ] Corrective SHA appears in every relevant record.
- [ ] Final diff contains planning Markdown only.
- [ ] The polish commit is pushed to `origin/main`.

---

# Stop conditions

Stop and reassess only if:

1. `main` has advanced with conflicting changes to the same planning records;
2. the canonical commit identity map is contradicted by Git history;
3. CI run `30688082724` cannot be associated with the completed implementation line;
4. the historical baseline cannot build and a truthful final-only fallback would materially contradict a release claim;
5. inspection discovers an actual production defect rather than a documentation inconsistency.

For conditions 1-4, preserve the documentation-only scope and use the narrowest truthful wording.

For condition 5, do not modify production code under this plan. Record the reproducible defect separately and stop the polish pass only if it invalidates closure.

---

# Completion record template

Fill this section only when executing the polish pass.

## Implementation

- **Status:** pending
- **Starting main SHA:** pending
- **Polish commit:** visible in Git history; do not add a second self-recording commit
- **Files changed:** pending

## Measurement disposition

- **CLI timing:** pending paired measurement or explicit final-only fallback
- **Binary size:** pending exact compared-build labels
- **Install inventory:** pending reconfirmation

## Reconciliation summary

- **Final closure status/checklist:** pending
- **Phase 1 exact corrective SHA:** pending
- **Phase 2 exact SHA normalization:** pending
- **Phase 3 exact corrective SHA:** pending
- **Phase 4 candidate/measurement/CI reconciliation:** pending
- **Roadmap reconciliation:** pending

## Verification

- **Placeholder search:** pending
- **Unchecked-checkbox search:** pending
- **Diff check:** pending
- **Remote push:** pending

## Final statement

Pending. When complete, state only that the planning record now accurately reflects the already-completed production implementation, measurements, install surface, and CI evidence. Do not reopen the roadmap.