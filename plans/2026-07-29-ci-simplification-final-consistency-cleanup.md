# CI Simplification Final Consistency Cleanup

## Status

- **Status:** ready for implementation
- **Repository:** `eggstack/eggsact`
- **Target branch:** `main`
- **Plan baseline:** `3077d210c3ef4bcf2aecd181aa96599c33dbe90a`
- **Parent implementation plan:** `plans/2026-07-28-ci-verification-release-simplification.md`
- **Parent closure plan:** `plans/2026-07-28-ci-simplification-closure-pass.md`
- **Scope:** documentation-only reconciliation of already-completed CI simplification work
- **Publication:** strictly out of scope
- **Primary objective:** make the two completed plans internally consistent with the verified workflow results without adding new verification machinery, evidence artifacts, or implementation changes

## Purpose

The CI simplification is operationally complete. The reduced ordinary CI run `30418424791` passed exactly the intended three rendered checks:

```text
Linux correctness
Check (windows-latest)
Check (macos-latest)
```

The maintenance dispatch `30418110835` passed MSRV and `cargo-deny`. The parity dispatch `30418110943` passed. No routine artifacts were produced. The local release check was recorded as passing without publication, and GitHub Actions contains no crate-publication path.

Only consistency defects remain in the plan documents:

1. the parent plan still records ordinary CI run `30406093095`, while the corrected authoritative run is `30418424791`;
2. the explicit acceptance checklist in the closure plan remains unchecked despite the plan being marked complete;
3. the final closure checklist in the parent plan remains unchecked despite the plan being marked complete;
4. the branch-protection and ruleset disposition is asserted as `not enabled; no rulesets`, but this final pass should independently reconfirm that claim before marking the corresponding checklist items complete;
5. closure-SHA terminology must remain clear and must not be repeatedly rewritten to point at documentation-only follow-up commits.

This is a final consistency cleanup, not another implementation or evidence pass.

---

# Required outcome

After this pass:

1. both completed plans reference ordinary CI run `30418424791` as the authoritative reduced-CI run;
2. both plans contain internally consistent implementation, closure, and verification identifiers;
3. every acceptance item that is supported by existing evidence is checked;
4. branch protection and repository rulesets are independently inspected and accurately recorded;
5. no checklist item is checked based only on assumption or copied prose;
6. the documents clearly distinguish the implementation commit, the operational closure commit, and this final documentation cleanup commit;
7. no source code, workflow, release script, test, dependency, or repository setting is changed unless the repository-settings inspection reveals an actual stale required-check configuration;
8. no new evidence ledger, exact-SHA table, artifact manifest, checksum list, or workflow is introduced;
9. the work lands in one narrow commit whenever repository settings require no change;
10. the CI simplification line of work is left fully consistent and requires no further closure plan.

---

# Non-goals and hard constraints

This pass must not:

- redesign `.github/workflows/ci.yml`;
- alter maintenance, parity, fuzz, sanitizer, or latest-compatible workflow triggers;
- add, remove, or rename CI jobs merely for documentation consistency;
- rerun all historical workflows solely to create additional evidence;
- add artifact uploads or provenance generation;
- add crates.io credentials, trusted publishing, OIDC publication, tag automation, or GitHub Release automation;
- publish a crate;
- create a release tag;
- modify product code or tests;
- create a new release-readiness document;
- copy full workflow logs into plan files;
- change the recorded operational closure SHA merely because this plan produces a later documentation commit;
- mark an unverified repository-settings claim complete.

If an actual stale required status check is discovered, correcting that repository setting is in scope. No YAML change is required merely to satisfy a stale setting.

---

# Authoritative facts for this pass

Treat the following as the starting evidence set:

```text
Implementation commit:
5774529119b03e3bfff4406810c7ca6c66f84c9c

Operational closure commit:
a376e7c0b78481b53225afa79dac84be6f01ed8f

Final ordinary CI run:
30418424791

Ordinary CI checks:
Linux correctness — pass
Check (windows-latest) — pass
Check (macos-latest) — pass

Maintenance dispatch:
30418110835
MSRV — pass
cargo-deny — pass

Parity dispatch:
30418110943
Parity (latest eggcalc) — pass

Routine artifacts from those runs:
none

GitHub publication workflow:
absent

Recorded branch-protection disposition:
not enabled; no rulesets
```

Do not replace the final ordinary CI run with an earlier successful run. Do not treat the plan-creation commit or this final cleanup commit as the implementation commit.

---

# Required execution sequence

Execute in this order:

1. fetch and freeze current `main`;
2. inspect the three post-plan commits after `06cfc6c3924c842188ba5a99b8995bf90c501abb`;
3. inspect the current versions of both plan files;
4. reconfirm ordinary CI run `30418424791` and its three job conclusions;
5. reconfirm maintenance run `30418110835` and parity run `30418110943`;
6. independently inspect classic branch protection and repository rulesets for `main`;
7. reconcile the parent plan completion record;
8. reconcile both acceptance checklists;
9. run a narrow consistency search across the two plans;
10. commit the documentation-only cleanup once;
11. inspect the resulting diff and stop.

Do not create a second follow-up commit merely to record that the first cleanup commit passed CI. The existing operational evidence is sufficient.

---

# Workstream 1 — Freeze current state and inspect post-plan history

Run:

```bash
git fetch origin main --prune
git rev-parse origin/main
git log --oneline --decorate 06cfc6c3924c842188ba5a99b8995bf90c501abb..origin/main
git diff --stat 06cfc6c3924c842188ba5a99b8995bf90c501abb..origin/main
git diff --name-status 06cfc6c3924c842188ba5a99b8995bf90c501abb..origin/main
```

Expected post-plan commits at the current baseline:

```text
a376e7c  ci: close CI simplification — fix stale reference, mark plans complete
68c4a36  docs: close CI simplification — verify maintenance/parity dispatch, correct closure record
3077d21  docs: update closure CI run to 30418424791 (all 3 checks pass)
```

Expected changed files since the closure-plan commit:

```text
.opencode/skills/testing/SKILL.md
plans/2026-07-28-ci-simplification-closure-pass.md
plans/2026-07-28-ci-verification-release-simplification.md
```

## Acceptance criteria

- Current `main` is a descendant of `06cfc6c3924c842188ba5a99b8995bf90c501abb`.
- Every post-plan commit is inspected.
- No workflow or product-code change after the plan commit is overlooked.
- The cleanup is based on current `main`, not on stale local files.

---

# Workstream 2 — Reconfirm operational evidence without expanding it

Use GitHub Actions UI, `gh`, or API access to inspect the existing runs.

Suggested commands:

```bash
gh run view 30418424791 --repo eggstack/eggsact
gh run view 30418424791 --repo eggstack/eggsact --json conclusion,headSha,jobs,url

gh run view 30418110835 --repo eggstack/eggsact --json conclusion,headSha,jobs,url
gh run view 30418110943 --repo eggstack/eggsact --json conclusion,headSha,jobs,url
```

For ordinary CI, confirm exactly these rendered jobs and no others:

```text
Linux correctness
Check (windows-latest)
Check (macos-latest)
```

For maintenance, confirm:

```text
MSRV
cargo-deny
```

For parity, confirm:

```text
Parity (latest eggcalc)
```

Confirm that ordinary CI did not execute MSRV, cargo-deny, parity, fuzzing, sanitizers, publication, tag creation, provenance generation, or artifact upload.

## Acceptance criteria

- Run `30418424791` concluded successfully.
- It contains exactly three rendered jobs with the expected names.
- All three jobs concluded successfully.
- Linux executed fmt, generated-doc check, Clippy, normal tests, doctests, and package construction.
- Windows and macOS executed compile checks only.
- Run `30418110835` passed both maintenance jobs.
- Run `30418110943` passed parity.
- No additional workflow execution is required for this cleanup.

---

# Workstream 3 — Independently verify repository settings

The existing plans state:

```text
Branch protection: not enabled; no rulesets
```

Do not merely copy that statement. Verify it.

Preferred commands where authenticated permissions allow:

```bash
gh api repos/eggstack/eggsact/branches/main/protection

gh api repos/eggstack/eggsact/rulesets --paginate
```

Interpret results carefully:

- A `404 Branch not protected` response confirms classic branch protection is not enabled.
- An empty ruleset list confirms no repository rulesets exist.
- A ruleset that exists but does not target `main` must be described accurately rather than summarized as “no rulesets.”
- A ruleset targeting `main` must be inspected for required workflows or status checks.
- If stale contexts such as `Check`, `Clippy`, `Generated Docs`, `Test (lib)`, `Test (bins)`, `Test (integration)`, `Test (doc)`, `MSRV (1.89.0)`, `cargo-deny`, `Windows`, `macOS`, or `Package` are required, remove those stale requirements.
- If required checks are enabled, the only ordinary CI contexts that may remain are the actual emitted names:

```text
Linux correctness
Check (windows-latest)
Check (macos-latest)
```

Do not enable branch protection merely because this plan inspects it. This pass verifies and repairs stale configuration; it does not introduce a new governance policy.

## Trouble-area example

Incorrect conclusion:

```text
The repository metadata did not mention branch protection, so no protection exists.
```

Correct conclusion:

```text
gh api repos/eggstack/eggsact/branches/main/protection returned 404 Branch not protected.
gh api repos/eggstack/eggsact/rulesets returned an empty list.
Disposition: branch protection not enabled; no rulesets; no stale required contexts.
```

If permissions prevent inspection, do not check the related acceptance items. Record:

```text
Repository-settings verification blocked by insufficient permission; prior disposition retained as unverified.
```

In that blocked case, the plan must remain incomplete rather than asserting closure.

## Acceptance criteria

- Classic branch protection for `main` was directly inspected.
- Repository rulesets were directly inspected.
- The resulting disposition is stated precisely.
- No deleted legacy context is required.
- No PR can be blocked by a nonexistent legacy status context.
- No new branch-protection policy was introduced unintentionally.

---

# Workstream 4 — Reconcile the closure plan

File:

```text
plans/2026-07-28-ci-simplification-closure-pass.md
```

Required edits:

1. preserve `Status: complete`;
2. preserve `Closure SHA: a376e7c` as the operational closure commit unless the document explicitly defines a different semantic;
3. preserve ordinary CI run `30418424791` as the authoritative run;
4. preserve maintenance run `30418110835` and parity run `30418110943`;
5. update the explicit acceptance checklist from unchecked boxes to checked boxes only where evidence exists;
6. leave no satisfied item unchecked;
7. leave no unsupported item checked;
8. add one concise final-consistency line if needed, for example:

```text
- **Final consistency cleanup:** <NEW_SHA> — synchronized plan records and completed evidence-backed checklists
```

Do not replace the operational closure SHA with the final cleanup SHA. These identify different events.

## Checklist mapping

The closure-plan checklist items should be checked when the following evidence is present:

- ordinary CI structure: current `.github/workflows/ci.yml`;
- successful real CI run and rendered names: run `30418424791`;
- maintenance disposition: current maintenance workflow and run `30418110835`;
- parity disposition: current parity workflow and run `30418110943`;
- release ownership: current workflows plus `scripts/release-check.sh`;
- repository settings: direct API/UI inspection from Workstream 3;
- documentation state: current normative docs and stale-reference search;
- closure state: both plans complete and final run/disposition recorded.

## Acceptance criteria

- Every closure-plan checklist item accurately reflects current evidence.
- No unchecked item remains solely because the earlier closure agent forgot to update boxes.
- No checked item depends on an unverified branch-protection assumption.
- The final run ID is `30418424791` everywhere in the closure plan where the authoritative ordinary CI run is intended.
- Closure SHA and final-cleanup SHA have distinct, clearly stated meanings.

---

# Workstream 5 — Reconcile the parent plan

File:

```text
plans/2026-07-28-ci-verification-release-simplification.md
```

Required edits:

1. replace the stale ordinary CI run `30406093095` with `30418424791` in the completion record;
2. preserve implementation commit `5774529119b03e3bfff4406810c7ca6c66f84c9c`;
3. preserve closure commit `a376e7c` as the operational closure commit;
4. preserve maintenance run `30418110835` and parity run `30418110943`;
5. update the final closure checklist from unchecked to checked where evidence exists;
6. ensure the branch-protection checklist items match the independently verified disposition;
7. do not add a second completion-record table.

Canonical completion record:

```text
Implementation commit: 5774529119b03e3bfff4406810c7ca6c66f84c9c
Closure commit: a376e7c
Ordinary CI run: 30418424791 (https://github.com/eggstack/eggsact/actions/runs/30418424791)
Linux correctness: pass
Windows compile: pass
macOS compile: pass
Maintenance workflow dispatch: pass (30418110835 — MSRV + cargo-deny)
Python parity dispatch: pass (30418110943)
Release workflow present: no
Branch protection: not enabled; no rulesets
Publication performed: no
Tag created: no
Result: complete
```

Adjust only the branch-protection line if direct inspection produces a different precise result.

## Acceptance criteria

- The parent plan contains no reference to `30406093095` as the final authoritative CI run.
- Its completion record matches the closure plan.
- Its final closure checklist is fully reconciled with evidence.
- The implementation and closure commits are not conflated with the final documentation cleanup commit.
- The plan remains marked `complete` only if all required settings checks were completed.

---

# Workstream 6 — Narrow consistency audit

Run searches limited to the current normative records and relevant guidance:

```bash
grep -RIn '30406093095' \
  plans/2026-07-28-ci-verification-release-simplification.md \
  plans/2026-07-28-ci-simplification-closure-pass.md \
  docs README.md AGENTS.md .opencode/skills || true

grep -RIn '30418424791' \
  plans/2026-07-28-ci-verification-release-simplification.md \
  plans/2026-07-28-ci-simplification-closure-pass.md

grep -RInE '\- \[ \].*(CI|MSRV|cargo-deny|parity|branch protection|ruleset|publication|tag|closure)' \
  plans/2026-07-28-ci-verification-release-simplification.md \
  plans/2026-07-28-ci-simplification-closure-pass.md || true

grep -RInE 'Test \(lib\)|Test \(bins\)|Test \(integration\)|Generated Docs|MSRV \(1\.89\.0\)|Full Release Gate' \
  docs README.md AGENTS.md .opencode/skills || true
```

Interpret historical references correctly. Historical plans may describe old job names when clearly marked non-normative. Do not rewrite archival history merely to eliminate search matches.

## Acceptance criteria

- `30406093095` is absent from the two active completion records.
- `30418424791` appears consistently as the final ordinary CI run.
- No evidence-backed acceptance item remains unchecked in either plan.
- No normative documentation describes the removed twelve-job model as current.
- No historical evidence document is expanded.

---

# Workstream 7 — Commit discipline and final inspection

Preferred result: one documentation-only commit.

Suggested commit message:

```text
docs(plans): reconcile final CI simplification closure records
```

Before committing:

```bash
git diff --check
git diff -- plans/2026-07-28-ci-verification-release-simplification.md \
             plans/2026-07-28-ci-simplification-closure-pass.md
```

After committing:

```bash
git show --stat --oneline HEAD
git show --check HEAD
git status --short
```

The expected changed files are only:

```text
plans/2026-07-28-ci-verification-release-simplification.md
plans/2026-07-28-ci-simplification-closure-pass.md
```

A third file is acceptable only when needed to correct a directly related current-facing documentation inconsistency discovered by the required search. Explain it in the commit message.

Do not produce another status-only follow-up commit after this one.

## Acceptance criteria

- The final diff is documentation-only unless a stale repository setting required correction.
- No workflow, source, test, or release-script file changed.
- The commit contains no evidence artifact or copied log output.
- The working tree is clean after the commit.
- The commit is pushed to `origin/main`.

---

# Explicit final acceptance checklist

## Evidence consistency

- [ ] Ordinary CI run `30418424791` was directly reconfirmed.
- [ ] Its three rendered jobs and conclusions were directly reconfirmed.
- [ ] Maintenance run `30418110835` was directly reconfirmed.
- [ ] Parity run `30418110943` was directly reconfirmed.
- [ ] No new workflow run was created solely for documentation evidence.

## Repository settings

- [ ] Classic branch protection for `main` was directly inspected.
- [ ] Repository rulesets were directly inspected.
- [ ] No stale legacy required-check context remains.
- [ ] The written repository-settings disposition exactly matches the observed configuration.

## Closure plan

- [ ] The authoritative ordinary CI run is `30418424791`.
- [ ] All evidence-backed checklist items are checked.
- [ ] No unsupported checklist item is checked.
- [ ] Operational closure SHA `a376e7c` remains semantically distinct from the final cleanup commit.

## Parent plan

- [ ] Stale run `30406093095` was replaced with `30418424791`.
- [ ] The completion record matches the closure plan.
- [ ] All evidence-backed final checklist items are checked.
- [ ] No duplicate completion record was added.

## Scope control

- [ ] No CI workflow was redesigned.
- [ ] No product code or test changed.
- [ ] No publication or tag action occurred.
- [ ] No artifact, checksum ledger, or evidence bundle was introduced.
- [ ] The cleanup landed in one narrow commit unless an actual repository-setting correction required otherwise.

## Final closure

- [ ] Both plans are internally consistent and remain accurately marked complete.
- [ ] The final diff passes `git diff --check` / `git show --check`.
- [ ] The final commit is pushed to `origin/main`.
- [ ] No further CI-simplification closure pass is required.

---

# Final completion standard

This pass is complete only when all of the following are true:

```text
Both plans identify 30418424791 as the final ordinary CI run.
Both plans accurately distinguish implementation, operational closure, and final documentation cleanup commits.
All evidence-backed checklist items are checked.
Repository branch-protection and ruleset claims were directly verified.
No stale required status context exists.
No workflow or release mechanism changed.
No publication occurred.
No new evidence bureaucracy was created.
One narrow final commit closes the documentation inconsistency.
```

Once those conditions are met, stop. The CI simplification line of work is closed.