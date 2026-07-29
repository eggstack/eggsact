# CI Simplification Final Record Correction

## Status

- **Status:** ready for implementation
- **Repository:** `eggstack/eggsact`
- **Target branch:** `main`
- **Plan baseline:** `4f6780fd0b4768307d10de18ee9d6c6c47a86d89`
- **Parent implementation plan:** `plans/2026-07-28-ci-verification-release-simplification.md`
- **Parent closure plan:** `plans/2026-07-28-ci-simplification-closure-pass.md`
- **Prior consistency plan:** `plans/2026-07-29-ci-simplification-final-consistency-cleanup.md`
- **Scope:** documentation-only correction of the final CI-simplification records
- **Publication:** strictly out of scope
- **Primary objective:** correct the remaining commit-identity and completion-state inconsistencies without modifying CI, release behavior, source code, tests, dependencies, or repository policy

## Purpose

The CI simplification is operationally complete. Commit `4f6780fd0b4768307d10de18ee9d6c6c47a86d89` correctly reconciled the two original plans by:

- replacing stale ordinary-CI run `30406093095` with authoritative run `30418424791`;
- checking the evidence-backed acceptance items in both original plans;
- preserving operational closure commit `a376e7c0b78481b53225afa79dac84be6f01ed8f`;
- changing only documentation.

Three narrow record defects remain:

1. both original plans identify `ef8d905` as the final consistency cleanup, but `ef8d905ace2e5e9a1e98508327b94d0c61102b74` created the cleanup **plan**; the actual cleanup **implementation** is `4f6780fd0b4768307d10de18ee9d6c6c47a86d89`;
2. `plans/2026-07-29-ci-simplification-final-consistency-cleanup.md` remains marked `ready for implementation` even though its work was executed;
3. that prior consistency plan's explicit final checklist remains unchecked, including repository-settings items that must not be checked unless branch protection and rulesets are directly inspected.

This pass exists only to make those records truthful and internally consistent. It must not reopen CI design or create another evidence process.

---

# Authoritative identifiers

Use these meanings consistently:

```text
CI simplification implementation:
5774529119b03e3bfff4406810c7ca6c66f84c9c

Operational closure implementation:
a376e7c0b78481b53225afa79dac84be6f01ed8f

Final consistency plan creation:
ef8d905ace2e5e9a1e98508327b94d0c61102b74

Final consistency implementation:
4f6780fd0b4768307d10de18ee9d6c6c47a86d89

Authoritative ordinary CI run:
30418424791

Maintenance verification run:
30418110835

Parity verification run:
30418110943
```

Do not conflate plan-creation commits with implementation commits.

Do not replace the operational closure SHA `a376e7c` with a later documentation commit.

Do not attempt to insert the SHA of the not-yet-created execution commit into the same commit. This pass must avoid self-referential SHA bookkeeping.

---

# Required outcome

After this pass:

1. both original completed plans identify `4f6780f` as the final consistency **implementation**;
2. neither original plan describes `ef8d905` as the cleanup implementation;
3. the prior consistency plan is marked `complete`;
4. the prior consistency plan records that:
   - its plan-creation commit was `ef8d905`;
   - its implementation commit was `4f6780f`;
5. all evidence-backed checklist items in the prior consistency plan are checked;
6. repository-settings checklist items are checked only after direct inspection of classic branch protection and repository rulesets;
7. if repository-settings inspection is blocked, the plan remains incomplete and the blocked items remain unchecked with a concise explanation;
8. no workflow, source, test, dependency, release script, or release document outside the three named plan files changes;
9. no publication, tag creation, GitHub Release creation, or repository-policy expansion occurs;
10. no further CI-simplification closure plan is needed.

---

# Files allowed to change

Only these files may change:

```text
plans/2026-07-28-ci-verification-release-simplification.md
plans/2026-07-28-ci-simplification-closure-pass.md
plans/2026-07-29-ci-simplification-final-consistency-cleanup.md
```

A repository-settings correction is allowed only if direct inspection discovers an actual stale required-check context. Such a settings change is not a repository file change and must be described precisely in the final commit message or plan status.

Any other file change is out of scope and must be reverted before commit.

---

# Hard constraints

This pass must not:

- edit `.github/workflows/ci.yml`;
- edit any maintenance, parity, fuzz, sanitizer, or compatibility workflow;
- rename jobs or checks;
- add or remove workflow triggers;
- rerun workflows solely to create more evidence;
- edit `scripts/release-check.sh`;
- edit source code or tests;
- edit dependencies or lockfiles;
- add artifacts, checksums, provenance, manifests, or evidence bundles;
- publish to crates.io;
- create or push a tag;
- create a GitHub Release;
- enable branch protection merely because it is being inspected;
- create a second completion-record table;
- rewrite historical release evidence;
- create another follow-up plan for minor wording;
- record this plan-creation commit as the execution result;
- require the execution commit to contain its own SHA.

If any instruction appears to require a self-referential commit SHA, prefer semantic labels and immutable prior commit IDs rather than a second status-only commit.

---

# Execution sequence

Perform these steps in order.

## Step 1 — Freeze current `main`

Run:

```bash
git fetch origin main --prune
git rev-parse origin/main
git log --oneline --decorate -8 origin/main
git status --short
```

Expected baseline head when this plan was written:

```text
4f6780fd0b4768307d10de18ee9d6c6c47a86d89
```

If `main` has advanced, inspect every intervening commit before editing. Continue only if the remaining defects still exist.

### Acceptance criteria

- Current `main` is known.
- The working tree is clean.
- All commits after `4f6780f`, if any, are inspected.
- No later commit has already completed this correction.

## Step 2 — Inspect the exact current defects

Run:

```bash
grep -n "Final consistency cleanup" \
  plans/2026-07-28-ci-verification-release-simplification.md \
  plans/2026-07-28-ci-simplification-closure-pass.md

grep -nE "Status:|Status:\*\*|ef8d905|4f6780f|30418424791" \
  plans/2026-07-29-ci-simplification-final-consistency-cleanup.md

grep -n -- "- \[ \]" \
  plans/2026-07-29-ci-simplification-final-consistency-cleanup.md
```

Expected findings:

```text
The two original plans name ef8d905 as the final consistency cleanup.
The prior consistency plan is ready for implementation.
The prior consistency plan has unchecked final acceptance items.
```

### Acceptance criteria

- Each stated defect is reproduced from current files.
- No edit is made based only on this handoff's description.
- The exact line locations to change are known.

## Step 3 — Reconfirm existing workflow evidence

Do not create new runs. Inspect the existing runs:

```bash
gh run view 30418424791 --repo eggstack/eggsact \
  --json conclusion,headSha,jobs,url

gh run view 30418110835 --repo eggstack/eggsact \
  --json conclusion,headSha,jobs,url

gh run view 30418110943 --repo eggstack/eggsact \
  --json conclusion,headSha,jobs,url
```

Confirm ordinary CI contains exactly:

```text
Linux correctness — success
Check (windows-latest) — success
Check (macos-latest) — success
```

Confirm maintenance contains:

```text
MSRV — success
cargo-deny — success
```

Confirm parity contains:

```text
Parity (latest eggcalc) — success
```

Do not copy full logs into plan files.

### Acceptance criteria

- Run `30418424791` is directly inspected and successful.
- It contains exactly the intended three rendered jobs.
- Run `30418110835` is directly inspected and successful.
- Run `30418110943` is directly inspected and successful.
- No new workflow run is created for this docs-only pass.

## Step 4 — Directly inspect repository settings

This step is mandatory before checking repository-settings items.

Run:

```bash
gh api repos/eggstack/eggsact/branches/main/protection
```

Expected unprotected result:

```text
HTTP 404: Branch not protected
```

Then run:

```bash
gh api repos/eggstack/eggsact/rulesets --paginate
```

Expected no-ruleset result:

```json
[]
```

Interpretation rules:

- `404 Branch not protected` means classic protection is not enabled.
- `[]` means no repository rulesets exist.
- A non-empty ruleset response must be inspected for target branches and required checks.
- Do not summarize a non-empty but irrelevant ruleset list as "no rulesets." State the actual disposition.
- If a ruleset targets `main`, verify it does not require deleted legacy contexts.
- If stale required contexts exist, remove only those stale contexts.
- Do not enable new branch protection.

Legacy contexts that must not remain required include:

```text
Check
Generated Docs
Clippy
Test (lib)
Test (bins)
Test (integration)
Test (doc)
MSRV (1.89.0)
cargo-deny
Windows
macOS
Package
Full Release Gate
```

Current ordinary CI contexts are:

```text
Linux correctness
Check (windows-latest)
Check (macos-latest)
```

### Blocked-permission behavior

If either settings endpoint cannot be inspected because of permissions:

1. do not check the corresponding checklist items;
2. add a concise note to the prior consistency plan:

```text
Repository-settings verification blocked by insufficient permission; prior disposition retained but not independently reconfirmed.
```

3. leave the prior consistency plan status as `blocked`, not `complete`;
4. do not guess.

### Acceptance criteria

- Classic branch protection is directly inspected.
- Repository rulesets are directly inspected.
- The recorded disposition exactly matches the observed configuration.
- No stale required-check context remains.
- No new repository policy is introduced.

## Step 5 — Correct the two original plans

Files:

```text
plans/2026-07-28-ci-verification-release-simplification.md
plans/2026-07-28-ci-simplification-closure-pass.md
```

Replace this inaccurate metadata:

```text
Final consistency cleanup: ef8d905 — synchronized plan records and completed evidence-backed checklists
```

With wording that distinguishes planning from implementation. Use this exact form unless surrounding style requires punctuation adjustment:

```text
Final consistency plan: ef8d905
Final consistency implementation: 4f6780f — synchronized plan records and completed evidence-backed checklists
```

Do not modify:

- implementation commit `5774529`;
- operational closure commit `a376e7c`;
- ordinary CI run `30418424791`;
- maintenance run `30418110835`;
- parity run `30418110943`.

Do not add a new completion table.

### Acceptance criteria

- Both plans identify `ef8d905` only as the plan-creation commit.
- Both plans identify `4f6780f` as the consistency implementation commit.
- Both plans retain `a376e7c` as the operational closure commit where applicable.
- Both plans retain `30418424791` as the authoritative ordinary CI run.
- No duplicate completion record is introduced.

## Step 6 — Complete the prior consistency plan

File:

```text
plans/2026-07-29-ci-simplification-final-consistency-cleanup.md
```

Update the status block.

Preferred completed form:

```text
- **Status:** complete
- **Plan commit:** ef8d905ace2e5e9a1e98508327b94d0c61102b74
- **Implementation commit:** 4f6780fd0b4768307d10de18ee9d6c6c47a86d89
- **Ordinary CI run:** 30418424791 — success
- **Maintenance run:** 30418110835 — success
- **Parity run:** 30418110943 — success
- **Repository settings:** branch protection not enabled; no rulesets; no stale required contexts
```

Adjust only the repository-settings line if direct inspection yields a different precise result.

Do not add the SHA of the current execution commit. The file should record the plan commit and the implementation commit that it was designed to verify. This avoids a self-reference loop.

Update the explicit final acceptance checklist:

- check every item supported by direct evidence;
- check repository-settings items only after Step 4 succeeds;
- leave no supported item unchecked;
- leave no unsupported item checked.

The following checklist groups should be fully checked when Steps 3 and 4 succeed:

```text
Evidence consistency
Repository settings
Closure plan
Parent plan
Scope control
Final closure
```

### Acceptance criteria

- Status is `complete` only if all mandatory verification succeeded.
- `ef8d905` is labeled as the plan commit.
- `4f6780f` is labeled as the implementation commit.
- Every evidence-backed checklist item is checked.
- No item is checked based on copied assertions alone.
- No self-referential execution SHA is required.

## Step 7 — Run narrow consistency searches

Run:

```bash
grep -RIn "Final consistency cleanup: ef8d905" \
  plans/2026-07-28-ci-verification-release-simplification.md \
  plans/2026-07-28-ci-simplification-closure-pass.md \
  plans/2026-07-29-ci-simplification-final-consistency-cleanup.md || true

grep -RIn "30406093095" \
  plans/2026-07-28-ci-verification-release-simplification.md \
  plans/2026-07-28-ci-simplification-closure-pass.md \
  plans/2026-07-29-ci-simplification-final-consistency-cleanup.md || true

grep -RInE "ef8d905|4f6780f|a376e7c|30418424791" \
  plans/2026-07-28-ci-verification-release-simplification.md \
  plans/2026-07-28-ci-simplification-closure-pass.md \
  plans/2026-07-29-ci-simplification-final-consistency-cleanup.md

grep -n -- "- \[ \]" \
  plans/2026-07-29-ci-simplification-final-consistency-cleanup.md || true
```

Expected result when fully complete:

- no inaccurate `Final consistency cleanup: ef8d905` line;
- no stale `30406093095` in the authoritative completion records;
- clear semantic labels for `ef8d905`, `4f6780f`, and `a376e7c`;
- no unchecked checklist item in the prior consistency plan.

Historical explanatory text may mention a stale identifier as a defect description. Do not delete valid historical discussion solely because a grep finds it. Evaluate context.

### Acceptance criteria

- No current metadata mislabels `ef8d905` as implementation.
- No authoritative completion record uses `30406093095`.
- Commit meanings are consistent across all three files.
- No supported checklist item remains unchecked.

## Step 8 — Inspect the final diff

Run:

```bash
git diff --check
git diff --stat
git diff --name-only
git diff -- \
  plans/2026-07-28-ci-verification-release-simplification.md \
  plans/2026-07-28-ci-simplification-closure-pass.md \
  plans/2026-07-29-ci-simplification-final-consistency-cleanup.md
```

Expected changed files:

```text
plans/2026-07-28-ci-verification-release-simplification.md
plans/2026-07-28-ci-simplification-closure-pass.md
plans/2026-07-29-ci-simplification-final-consistency-cleanup.md
```

No workflow or code file should appear.

### Acceptance criteria

- `git diff --check` passes.
- Exactly the intended three plan files changed.
- The diff contains only record corrections and checklist updates.
- No unrelated reformatting or prose expansion is present.

## Step 9 — Commit and push once

Suggested commit message:

```text
docs(plans): close final CI consistency records
```

Run:

```bash
git add \
  plans/2026-07-28-ci-verification-release-simplification.md \
  plans/2026-07-28-ci-simplification-closure-pass.md \
  plans/2026-07-29-ci-simplification-final-consistency-cleanup.md

git commit -m "docs(plans): close final CI consistency records"
git push origin main
```

Then verify:

```bash
git show --stat --oneline HEAD
git show --check HEAD
git status --short
```

Do not add a second status-only commit to record the SHA of this commit.

### Acceptance criteria

- One narrow documentation commit lands on `main`.
- Only the three approved plan files are included.
- The working tree is clean.
- The commit is pushed to `origin/main`.
- No follow-up evidence commit is created.

---

# Trouble-area examples

## Correct commit identity

Incorrect:

```text
Final consistency cleanup: ef8d905
```

This implies the plan-creation commit performed the cleanup.

Correct:

```text
Final consistency plan: ef8d905
Final consistency implementation: 4f6780f
```

## Correct closure identity

Incorrect:

```text
Closure SHA: <new docs-only correction commit>
```

Correct:

```text
Operational closure SHA: a376e7c
Final consistency implementation: 4f6780f
```

The later record-correction commit does not replace either semantic identity.

## Correct repository-settings verification

Incorrect:

```text
No branch protection exists because the plan already says so.
```

Correct:

```text
gh api repos/eggstack/eggsact/branches/main/protection
# 404 Branch not protected

gh api repos/eggstack/eggsact/rulesets --paginate
# []
```

Then check the related checklist items.

## Correct blocked behavior

Incorrect:

```text
The API was unavailable, but the old claim is probably correct, so mark complete.
```

Correct:

```text
Repository-settings verification blocked by permission.
Leave those items unchecked and set status to blocked.
```

## Correct scope control

Incorrect:

```text
While here, simplify workflow comments and rename maintenance jobs.
```

Correct:

```text
Change only the three plan files. Stop after the records are consistent.
```

---

# Explicit acceptance checklist

## Baseline and evidence

- [ ] Current `main` was fetched and inspected.
- [ ] No later commit already completed this correction.
- [ ] Ordinary CI run `30418424791` was directly inspected.
- [ ] Its three rendered jobs were directly inspected and passed.
- [ ] Maintenance run `30418110835` was directly inspected and passed.
- [ ] Parity run `30418110943` was directly inspected and passed.
- [ ] No new workflow run was created solely for documentation evidence.

## Repository settings

- [ ] Classic branch protection for `main` was directly inspected.
- [ ] Repository rulesets were directly inspected.
- [ ] The written disposition exactly matches the observed settings.
- [ ] No stale legacy required-check context remains.
- [ ] No new branch-protection or ruleset policy was introduced.

## Original plans

- [ ] Both original plans label `ef8d905` as the consistency plan commit.
- [ ] Both original plans label `4f6780f` as the consistency implementation commit.
- [ ] Neither original plan mislabels `ef8d905` as implementation.
- [ ] Operational closure SHA `a376e7c` remains unchanged.
- [ ] Authoritative ordinary CI run `30418424791` remains unchanged.
- [ ] No duplicate completion record was added.

## Prior consistency plan

- [ ] Status is `complete`, or `blocked` only if direct settings inspection was impossible.
- [ ] Plan commit `ef8d905` is recorded accurately.
- [ ] Implementation commit `4f6780f` is recorded accurately.
- [ ] All evidence-backed checklist items are checked.
- [ ] No unsupported checklist item is checked.
- [ ] No self-referential execution SHA is required.

## Scope control

- [ ] Only the three approved plan files changed.
- [ ] No workflow changed.
- [ ] No source code or test changed.
- [ ] No dependency or lockfile changed.
- [ ] No release script changed.
- [ ] No publication or tag action occurred.
- [ ] No artifact, checksum ledger, or evidence bundle was introduced.

## Commit and closure

- [ ] `git diff --check` passed.
- [ ] One narrow commit was created.
- [ ] The commit was pushed to `origin/main`.
- [ ] The working tree is clean.
- [ ] No second status-only commit was created.
- [ ] No further CI-simplification closure pass is required.

---

# Final completion standard

This pass is complete only when:

```text
ef8d905 is identified as the final consistency plan commit.
4f6780f is identified as the final consistency implementation commit.
a376e7c remains the operational closure commit.
30418424791 remains the authoritative ordinary CI run.
The prior consistency plan is accurately complete.
Repository-settings claims are directly verified.
Only three plan files changed.
No CI or release behavior changed.
No publication occurred.
No self-referential SHA or follow-up evidence commit was introduced.
```

Once these conditions are satisfied, stop. The CI simplification line of work is closed.