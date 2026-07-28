# Final Single-SHA Evidence-Only Closure Pass

## Status

- **Status:** complete (evidence-only pass); corrective implementation pass `75ea503` closed the calculator normalization backtrack finding
- **Repository:** `eggstack/eggsact`
- **Target branch:** `main`
- **Plan baseline:** `c6fc38594fa72f410673a229edca4be5b91fb016`
- **Frozen code-and-test baseline:** `3e5b41c6ac5a8daaba11d5dfacb822f6da033464`
- **Corrective pass SHA:** `75ea50369510d98617741d4025fc626a0983b2e0`
- **Scope:** workflow execution, artifact inspection, and release-document correction only
- **Runtime/source changes:** prohibited
- **Test/fuzz-target changes:** prohibited
- **Workflow changes:** prohibited unless a workflow cannot be dispatched against an exact ref; if a workflow edit is required, stop and reopen implementation with a new code SHA
- **Publication:** out of scope; crates.io publication and tag creation remain direct maintainer actions

## Purpose

The implementation and deterministic-test work is complete. The remaining closure defect is an evidence-chain inconsistency:

- the declared final baseline is `3e5b41c6ac5a8daaba11d5dfacb822f6da033464`;
- extended fuzz and sanitizer evidence exists on that SHA;
- ordinary CI, release verification, latest-compatible dependencies, Python parity, and provenance evidence are still recorded against `50f9132f23c72e9a0df9475774430bdea9ac32d7`;
- Release 5 still contains stale references to older `06f7a0b` runs;
- the provenance artifact identity is incomplete in tracked documentation;
- package-file counts are inconsistent between local package output and provenance records;
- the final fuzz-target inventory contains a duplicate target name;
- some evidence text conflates code-under-test CI with CI on a later documentation-only commit.

This pass must close those documentation and workflow-proof gaps without changing the code being released.

The central rule is simple:

> Every workflow used to satisfy final Release 4 or Release 5 closure must run successfully with `head_sha == 3e5b41c6ac5a8daaba11d5dfacb822f6da033464`.

No “equivalent code,” “non-production delta,” or inherited-run exception is permitted in final evidence.

---

# Non-goals and hard constraints

This pass must not modify:

- `src/**`;
- `tests/**`;
- `fuzz/fuzz_targets/**`;
- `fuzz/corpus/**`;
- `Cargo.toml` or `Cargo.lock`;
- `.github/workflows/**`;
- generated tool documentation;
- MCP schemas, machine codes, budgets, profiles, audiences, or runtime behavior.

This pass must not:

- reinterpret `50f9132` as the final code SHA;
- reuse a workflow run from another SHA as exact evidence;
- mark a skipped, cancelled, neutral, or missing matrix entry as passing;
- publish to crates.io;
- create a release tag;
- add an automatic publishing path;
- write a self-referential “current main head” or “final evidence head” field into tracked documentation;
- create additional commits merely to record the CI run of the preceding documentation commit;
- alter source or tests in response to a failing workflow.

If any required workflow fails because of a real code, test, fuzz-target, dependency, or workflow defect:

1. stop this evidence-only pass;
2. leave the relevant closure criterion open;
3. record the exact failing run and job;
4. create a separate corrective implementation plan;
5. produce a new code SHA only after that corrective work lands;
6. rerun all final evidence workflows against the new SHA.

An evidence-only pass may not silently become another implementation pass.

---

# Current verified implementation state to preserve

The following implementation facts are already considered complete and must not be reopened here:

- MCP lifecycle transitions are mutex-owned;
- `begin_running` occurs inside the blocking closure;
- closure exit is signalled on every blocking-closure return path;
- timeout and completion races are tested with exact gates;
- test handler slots use exclusive RAII leases;
- the sync pool has fixed workers and a bounded queue;
- queue insertion tests use post-`try_send` signals;
- queued deadline expiry and cancellation skip handler invocation;
- timeout and channel disconnection are classified separately;
- mutable execution context commits only after successful, uncancelled completion;
- `two_jobs_run_concurrently` proves simultaneous handler execution;
- `timed_out_running_retains_worker` proves retained worker occupancy;
- Release 5 fuzz-target coverage includes all 12 intended surfaces;
- the final fuzz-target assertion correction is present at `3e5b41c`.

Do not spend time redesigning, re-reviewing, or refactoring those areas during this pass.

---

# Required execution sequence

Execute in this order:

1. prove that `3e5b41c` is the last non-documentation commit relevant to the release;
2. create an immutable temporary verification branch at exactly `3e5b41c`;
3. inspect current workflow names, dispatch inputs, and branch filters;
4. run ordinary CI against the exact verification branch;
5. run release verification against the exact verification branch;
6. run extended fuzz and sanitizer matrices against the exact verification branch;
7. run latest-compatible dependencies against the exact verification branch;
8. run Python parity against the exact verification branch;
9. inspect every run and matrix job through GitHub Actions APIs or `gh`;
10. download release and parity artifacts;
11. record GitHub artifact identities and calculate extracted-file checksums;
12. reproduce and explain package-file counts from a clean checkout of `3e5b41c`;
13. update all closure documents in one documentation-only commit;
14. push that commit and require ordinary CI to pass;
15. do not edit tracked evidence again solely to record that documentation commit's CI run;
16. delete the temporary verification branch after all immutable run identities are safely recorded.

If a workflow was already run on exact `3e5b41c`, it may be reused only after independently confirming its full head SHA, ref, event, conclusion, and complete job matrix.

---

# Workstream 1 — Freeze and prove the exact baseline

## Required baseline

Use:

```text
CODE_SHA=3e5b41c6ac5a8daaba11d5dfacb822f6da033464
```

## Required repository-history proof

Confirm that every commit after `CODE_SHA` is documentation-only.

Suggested commands:

```bash
CODE_SHA=3e5b41c6ac5a8daaba11d5dfacb822f6da033464

git fetch origin main --prune
git diff --name-status "$CODE_SHA"..origin/main
```

Expected result: only documentation, plan, or repository-guidance files are changed after `CODE_SHA`.

At minimum, no path after `CODE_SHA` may exist under:

```text
src/
tests/
fuzz/fuzz_targets/
fuzz/corpus/
.github/workflows/
Cargo.toml
Cargo.lock
```

If a code, test, fuzz, workflow, manifest, or lockfile change exists after `CODE_SHA`, stop and select the last such commit as the new baseline. All evidence workflows must then be run against that new SHA.

## Clean-checkout identity proof

Create a clean worktree:

```bash
git worktree add /tmp/eggsact-evidence-only "$CODE_SHA"
cd /tmp/eggsact-evidence-only

test "$(git rev-parse HEAD)" = "$CODE_SHA"
test -z "$(git status --porcelain)"
```

Record:

- full SHA;
- branch-detached state;
- clean status before evidence inspection;
- clean status after package inspection.

## Acceptance criteria

- The full 40-character `CODE_SHA` is recorded.
- Every post-`CODE_SHA` commit is documentation-only.
- A clean checkout at `CODE_SHA` is demonstrated.
- No implementation file is modified during this pass.
- The evidence pass stops if the baseline identity cannot be proven.

---

# Workstream 2 — Create an immutable verification ref

## Branch creation

Create a temporary branch whose ref points exactly to `CODE_SHA`:

```bash
git branch -f verification/final-evidence "$CODE_SHA"
git push --force-with-lease origin verification/final-evidence
```

If branch-protection policy disallows force-updating the branch, use a unique branch name such as:

```text
verification/final-evidence-3e5b41c
```

## Ref verification

Confirm locally and remotely:

```bash
test "$(git rev-parse verification/final-evidence)" = "$CODE_SHA"
git ls-remote origin refs/heads/verification/final-evidence
```

The remote ref must equal the exact full SHA.

## Immutability rule

After workflow dispatch begins:

- do not add commits to the verification branch;
- do not rebase it;
- do not move the ref;
- do not merge documentation commits into it;
- do not reuse the branch for later code.

If the branch moves, all attached runs must be treated as suspect until their individual `head_sha` values are verified.

## Acceptance criteria

- One remote branch points exactly to `CODE_SHA`.
- The branch remains unchanged during all workflow runs.
- Every final workflow records the same full `head_sha`.
- The branch is deleted only after evidence is committed and reviewed.

---

# Workstream 3 — Discover workflow contracts before dispatch

## Required workflow inventory

Inspect `.github/workflows/` at `CODE_SHA`, not only at current documentation head.

Identify the actual filenames and display names for:

1. ordinary CI;
2. release verification;
3. extended fuzz and sanitizer matrix;
4. latest-compatible dependencies;
5. Python parity.

Suggested commands:

```bash
git show "$CODE_SHA":.github/workflows/ci.yml
git show "$CODE_SHA":.github/workflows/release-verification.yml
git show "$CODE_SHA":.github/workflows/fuzz-scheduled.yml

gh workflow list
```

Also locate the actual workflow files for latest-compatible and Python parity.

## Dispatchability checks

For each workflow, determine:

- whether it supports `workflow_dispatch`;
- whether it accepts inputs;
- whether `--ref verification/final-evidence` is supported;
- whether branch filters suppress execution;
- whether a push to the verification branch triggers it automatically;
- whether the workflow itself checks out `${{ github.sha }}` or a different ref;
- whether any reusable workflow receives the exact SHA.

## No workflow edits in this pass

If a required workflow cannot run against an exact ref without modification, stop. Do not edit workflow YAML as part of an evidence-only pass.

Document the missing dispatch capability and create a corrective plan. A workflow edit would create a new code-and-workflow baseline and require all final evidence to restart.

## Acceptance criteria

- Actual workflow filenames and names are recorded.
- Dispatch method is known for every workflow.
- Checkout behavior is verified.
- No workflow silently tests default-branch HEAD instead of the requested ref.
- No workflow file is changed.

---

# Workstream 4 — Run every final workflow on the same SHA

## Required workflow set

Every item below must complete with:

```text
head_sha == 3e5b41c6ac5a8daaba11d5dfacb822f6da033464
conclusion == success
```

### A. Ordinary CI

Required jobs include, according to the repository's current CI contract:

- format/check;
- clippy with warnings denied;
- generated documentation check;
- library tests;
- binary tests;
- integration tests;
- documentation tests;
- MSRV check/tests;
- Windows check/tests;
- macOS check/tests;
- cargo-deny;
- package verification.

Do not accept a run if a required matrix job is missing, cancelled, or skipped without a documented platform-policy reason.

### B. Release verification

Required coverage includes:

- format;
- generated docs;
- clippy;
- library tests;
- binary tests;
- integration tests;
- documentation tests;
- cargo-deny;
- package contents;
- package build;
- `cargo publish --dry-run`;
- provenance generation;
- provenance artifact upload.

### C. Extended fuzz and sanitizer

Required matrix:

- all 12 extended fuzz targets;
- all 7 AddressSanitizer targets.

The 12 unique fuzz targets must be recorded exactly once each:

1. `calculator_expression`
2. `calculator_normalization`
3. `unified_diff`
4. `shell_tokenization`
5. `shell_quoting`
6. `regex_classification`
7. `regex_execution`
8. `json_pointer`
9. `toml_config`
10. `unicode_inspection`
11. `markdown_fences`
12. `glob_matching`

The final evidence must not duplicate `calculator_normalization` or omit another target.

The sanitizer target list must match the actual seven jobs emitted by the workflow.

### D. Latest-compatible dependencies

The run must use the exact verification ref and finish successfully. Record whether it updates dependencies in a temporary working tree, uses `cargo update`, or runs another compatibility strategy.

### E. Python parity

The run must:

- use the exact verification ref;
- complete successfully;
- report zero unaccepted failures;
- record accepted ignored differences separately;
- record eggsact version, eggcalc version, and Python version;
- upload the parity report if the workflow normally does so.

## Dispatch examples

Use actual filenames discovered in Workstream 3. Example only:

```bash
gh workflow run ci.yml --ref verification/final-evidence
gh workflow run release-verification.yml --ref verification/final-evidence
gh workflow run fuzz-scheduled.yml --ref verification/final-evidence
gh workflow run latest-compatible.yml --ref verification/final-evidence
gh workflow run python-parity.yml --ref verification/final-evidence
```

Do not copy these names without checking the repository.

## Run inspection

For each run, collect:

```bash
gh run view "$RUN_ID" --json \
  databaseId,url,workflowName,event,status,conclusion,headSha,headBranch,jobs
```

Also inspect all jobs:

```bash
gh run view "$RUN_ID" --json jobs
```

Record:

- workflow name;
- run ID;
- immutable URL;
- event type;
- head branch;
- full head SHA;
- conclusion;
- job name;
- job conclusion;
- matrix target where applicable;
- attempt number if a rerun occurred.

## Rerun policy

A failed job may be rerun only after determining whether the failure is environmental.

Acceptable environmental rerun examples:

- runner provisioning failure;
- GitHub service outage;
- artifact service transient error;
- network fetch failure with no repository-related error.

Unacceptable evidence behavior:

- rerunning until a flaky repository test happens to pass without investigation;
- omitting failed attempts;
- calling a run successful when one matrix entry remains failed;
- treating a cancelled older run as irrelevant without recording the successful replacement attempt.

If a rerun is used, record the failed attempt and the successful attempt, with the reason the failed attempt was classified as infrastructure-only.

## Acceptance criteria

- All five required workflows run on exactly `CODE_SHA`.
- All required jobs and matrix entries pass.
- The extended fuzz list contains 12 unique targets.
- The sanitizer list contains all seven actual targets.
- No evidence is inherited from `50f9132`, `06f7a0b`, or `fa6a6e9` for final closure.
- Historical runs may remain in a clearly labelled historical section only.

---

# Workstream 5 — Normalize provenance artifact evidence

## Required release artifact identity

For the exact-SHA release-verification run, record both GitHub's artifact identity and the extracted file identity.

Required GitHub fields:

- workflow run ID;
- artifact ID;
- artifact name;
- artifact size;
- artifact creation time;
- artifact expiration time;
- workflow head SHA;
- GitHub-provided artifact digest, when available.

Example format:

```text
Artifact ID: 1234567890
Artifact name: release-provenance
Workflow head SHA: <CODE_SHA>
GitHub archive digest: sha256:<digest>
```

## Download and extraction

Download the exact artifact:

```bash
gh run download "$RELEASE_RUN_ID" \
  --name release-provenance \
  --dir /tmp/eggsact-release-provenance

find /tmp/eggsact-release-provenance -type f -print
```

Do not assume the extracted filename. Record the actual name.

Calculate a checksum for every extracted file:

```bash
find /tmp/eggsact-release-provenance -type f -print0 \
  | sort -z \
  | xargs -0 shasum -a 256
```

## Digest terminology

Keep these distinct:

- **GitHub artifact archive digest:** digest of GitHub's uploaded artifact archive, when reported by the API;
- **Downloaded ZIP digest:** checksum of the downloaded archive file, if independently computed;
- **Extracted provenance file digest:** checksum of the actual provenance JSON/text file after extraction.

Do not label one as another.

## Provenance-content validation

Open the provenance file and verify it records:

- package name `eggsact`;
- package version `1.2.0`;
- full `CODE_SHA`;
- stable Rust version used by release verification;
- MSRV `1.89.0`;
- lockfile SHA-256;
- package file count and the count's defined semantics;
- successful publish dry run;
- generation timestamp, if present.

If the artifact records `50f9132`, it cannot satisfy this pass. Run release verification again correctly against `CODE_SHA`.

## Parity artifact identity

If Python parity emits a report artifact, record:

- artifact ID;
- artifact name;
- GitHub digest;
- extracted filename;
- extracted-file SHA-256;
- exact workflow head SHA.

## Acceptance criteria

- Artifact ID and name are present in tracked evidence.
- GitHub archive digest and extracted-file digest are separately labelled.
- Provenance content records the exact final SHA.
- Artifact metadata and provenance content agree.
- No checksum is copied from an earlier-SHA artifact.

---

# Workstream 6 — Resolve package-file-count discrepancies

## Current inconsistency

Tracked documentation currently reports both 235 and 236 package files without defining the counting method.

This pass must establish canonical counts rather than selecting one arbitrarily.

## Reproduce package output

From the clean checkout at `CODE_SHA`:

```bash
cargo package --locked --list > /tmp/eggsact-package-list.txt
wc -l /tmp/eggsact-package-list.txt

cargo package --locked
CRATE=$(ls target/package/eggsact-1.2.0.crate)
tar -tzf "$CRATE" | sort > /tmp/eggsact-crate-entries.txt
wc -l /tmp/eggsact-crate-entries.txt
```

Also inspect unique paths:

```bash
sort -u /tmp/eggsact-package-list.txt > /tmp/eggsact-package-list.unique
sort -u /tmp/eggsact-crate-entries.txt > /tmp/eggsact-crate-entries.unique

wc -l /tmp/eggsact-package-list.unique
wc -l /tmp/eggsact-crate-entries.unique
```

## Explain the difference

Compare lists after normalizing the archive's package-root prefix. Determine whether the difference comes from generated cargo metadata such as:

- `.cargo_vcs_info.json`;
- normalized `Cargo.toml`;
- retained `Cargo.toml.orig`;
- package-root directory entries;
- another Cargo-generated file.

Do not assume the cause. Show the exact differing path or entry.

## Canonical documentation format

Use separate labels, for example:

```text
cargo package --list source paths: 235
crate archive file entries: 236
Difference: <exact Cargo-generated entry>
```

If the actual counts differ from that example, record the observed values.

The provenance file must specify which count it records. If it cannot be changed because that would require a workflow edit, explain its semantics in documentation based on the artifact's contents and local reproduction.

## Acceptance criteria

- Both counts are reproduced from `CODE_SHA`.
- Counting commands are documented.
- The exact differing entry is identified.
- Release 4, release readiness, and closure evidence use consistent terminology.
- No bare “package files” count remains ambiguous.

---

# Workstream 7 — Correct tracked closure documents

## Files to update

At minimum:

- `docs/releases/2026-07-final-closure-evidence.md`;
- `docs/release-4-status.md`;
- `docs/release-5-status.md`;
- `docs/release-readiness.md`;
- `plans/2026-07-26-final-polish-and-exact-evidence-closure-pass.md`;
- this plan, if in-place status tracking is repository convention.

Do not edit unrelated documentation.

## Required corrections

### A. Use one final SHA

Every final workflow row must name:

```text
3e5b41c6ac5a8daaba11d5dfacb822f6da033464
```

Do not retain final-closure rows naming:

- `50f9132`;
- `06f7a0b`;
- `fa6a6e9`.

Those may appear only under a clearly marked historical-evidence heading.

### B. Correct Release 5 stale rows

Replace stale Release 5 claims that full CI or release verification ran on exact `06f7a0b`.

Release 5's closure table must reference the new exact-SHA run IDs produced by this pass.

### C. Correct fuzz inventory

List the 12 unique fuzz targets exactly once. Remove the duplicate `calculator_normalization` entry.

### D. Normalize artifact records

Add:

- artifact ID;
- artifact name;
- GitHub archive digest;
- extracted provenance filename;
- extracted provenance SHA-256;
- parity artifact fields, if present;
- package-count semantics.

### E. Distinguish code CI from docs CI

Use terminology such as:

```text
Code-under-test CI
```

for the run whose head SHA is `CODE_SHA`.

Use:

```text
Evidence-commit CI
```

for ordinary CI on the later documentation-only commit.

Do not say the evidence-commit run executed “on CODE_SHA.” It executed on the evidence commit, whose tree contains documentation changes layered over the verified code baseline.

### F. Remove self-referential fields

Do not add or retain:

- `Final main head`;
- `Current main SHA`;
- `Final evidence head` as a value intended to equal the commit containing the field;
- a tracked field that must be updated after every evidence-doc edit.

Use durable identities instead:

```text
Code-under-test SHA: <CODE_SHA>
Evidence document path: <path>
Workflow run IDs: <immutable IDs>
Artifact IDs and digests: <immutable values>
```

Git history identifies the documentation commit.

### G. Correct plan status

The preceding polish plan currently claims completion despite mixed-SHA evidence. Change its status to one of:

```text
superseded by final single-SHA evidence-only closure pass
```

or:

```text
implementation complete; evidence closure completed by <this plan path>
```

Only mark this plan complete after all exact-SHA workflow and artifact criteria are met.

## Evidence-table template

Use a table equivalent to:

| Gate | Run ID | Head SHA | Result | Required jobs |
|---|---:|---|---|---|
| Ordinary CI | `<id>` | `<CODE_SHA>` | success | 12/12 |
| Release verification | `<id>` | `<CODE_SHA>` | success | all release jobs |
| Extended fuzz/sanitizer | `<id>` | `<CODE_SHA>` | success | 12 fuzz + 7 ASan |
| Latest-compatible | `<id>` | `<CODE_SHA>` | success | all required jobs |
| Python parity | `<id>` | `<CODE_SHA>` | success | zero unaccepted failures |

## Acceptance criteria

- All final run rows use one full SHA.
- Release 5 contains no stale “exact SHA” references.
- Artifact identities are complete.
- Package counts are unambiguous.
- Fuzz target names are unique and complete.
- Code CI and evidence-commit CI are not conflated.
- No self-referential SHA loop remains.

---

# Workstream 8 — Create one evidence commit and stop editing

## Commit composition

After every workflow is complete and every artifact is inspected, make one documentation-only commit containing all evidence corrections.

Suggested message:

```text
docs(release): finalize single-SHA closure evidence
```

The commit must modify only:

- release-status documents;
- release-readiness documents;
- closure-evidence documents;
- plan status metadata;
- optionally `AGENTS.md` only if an existing instruction must be corrected, though this should normally be unnecessary.

## Post-commit CI

Push the evidence commit to `main` and require ordinary CI to complete successfully.

Inspect that run directly in GitHub Actions.

Do not edit tracked files again solely to write that run ID into the same evidence document. That would create another evidence commit requiring another run.

The final handoff report may state:

```text
Evidence commit: <full SHA>
Ordinary CI attached to evidence commit: <run ID>, success
```

That operational report does not need to be committed into the repository.

## What to do if documentation CI fails

If the failure is due to:

- formatting;
- broken links checked by CI;
- generated-doc drift caused by the documentation edits;
- malformed Markdown or metadata;

correct it in one follow-up documentation commit and run CI again.

Do not update the evidence document to include that follow-up run's ID.

If the failure reveals source/test behavior, stop and reopen implementation.

## Acceptance criteria

- One evidence commit contains all final tracked corrections.
- No implementation path changes in the evidence commit.
- Ordinary CI passes on the evidence commit.
- No subsequent “record the last CI run” commit is created.
- Git history, not a self-referential field, identifies the evidence commit.

---

# Workstream 9 — Remove temporary verification resources

After all evidence is committed and reviewed:

```bash
git push origin --delete verification/final-evidence
```

Remove the local branch and worktree:

```bash
git branch -D verification/final-evidence
git worktree remove /tmp/eggsact-evidence-only
```

Do not delete GitHub Actions runs or artifacts.

Record branch deletion in the handoff summary, not by creating another repository commit.

## Acceptance criteria

- Temporary verification branch is deleted.
- Immutable workflow runs remain accessible.
- Required artifacts remain unexpired at closure time.
- Local temporary worktree is removed.

---

# Required final verification checklist

## Baseline identity

- [x] `CODE_SHA` is `3e5b41c6ac5a8daaba11d5dfacb822f6da033464`.
- [x] No implementation-relevant file changed after `CODE_SHA`.
- [x] Clean checkout at `CODE_SHA` verified.
- [x] Temporary verification branch points exactly to `CODE_SHA`.

## Exact-SHA workflows

- [x] Ordinary CI passes on `CODE_SHA`.
- [x] Release verification passes on `CODE_SHA`.
- [x] Extended fuzz passes all 12 targets on `CODE_SHA`.
- [x] AddressSanitizer passes all seven targets on `CODE_SHA`.
- [x] Latest-compatible passes on `CODE_SHA`.
- [x] Python parity passes on `CODE_SHA`.
- [x] Every run's full `head_sha` is recorded.
- [x] Every required job conclusion is recorded.
- [x] Any infrastructure-only rerun is explained.

## Artifact proof

- [x] Release provenance artifact ID recorded.
- [x] Release provenance artifact name recorded.
- [x] GitHub artifact archive digest recorded.
- [x] Extracted provenance filename recorded.
- [x] Extracted provenance file SHA-256 recorded.
- [x] Provenance content records `CODE_SHA`.
- [x] Parity artifact identity and checksum recorded, if emitted.

## Package-count proof

- [x] `cargo package --list` count reproduced.
- [x] crate archive entry count reproduced.
- [x] Difference is explained by exact entry name.
- [x] Documentation uses distinct count labels.
- [x] Provenance count semantics are documented.

## Documentation correction

- [x] Release 4 uses exact-SHA final workflows.
- [x] Release 5 uses exact-SHA final workflows.
- [x] Release readiness uses exact-SHA final workflows.
- [x] Closure evidence uses exact-SHA final workflows.
- [x] Old runs are historical only.
- [x] Duplicate fuzz-target name removed.
- [x] No self-referential head field remains.
- [x] Code-under-test CI and evidence-commit CI are clearly distinguished.
- [x] Previous polish plan no longer falsely claims mixed-SHA closure.

## Finalization

- [x] One documentation-only evidence commit created.
- [x] Ordinary CI passes on that evidence commit.
- [x] No follow-up commit is created merely to record that CI run.
- [x] Temporary verification branch deleted.
- [x] crates.io publication not performed.
- [x] release tag not created.

---

# Stop conditions

Stop and report rather than marking this plan complete if:

- any required workflow cannot run against an exact ref;
- any required run has a head SHA other than `CODE_SHA`;
- any required matrix entry fails, is missing, or is unjustifiably skipped;
- a workflow failure requires modifying source, tests, fuzz targets, workflows, manifest, or lockfile;
- provenance records a different commit;
- an artifact cannot be downloaded or inspected;
- package counts cannot be reconciled;
- Python parity has an unaccepted failure;
- the evidence commit contains implementation changes;
- closure would require automatic crates.io publication.

In those cases, leave closure status open and preserve the precise failing evidence.

---

# Definition of done

This line of work is formally closed only when:

1. `3e5b41c6ac5a8daaba11d5dfacb822f6da033464` is proven to be the frozen release code-and-test baseline;
2. ordinary CI, release verification, extended fuzzing, sanitizers, latest-compatible dependencies, and Python parity all pass with that exact `head_sha`;
3. every required matrix job is accounted for;
4. release and parity artifact identities are complete and checksummed;
5. provenance content names the exact baseline;
6. package-file counts are reproduced and semantically reconciled;
7. Release 4, Release 5, release readiness, and closure evidence all use the same final run set;
8. no stale exact-SHA claim remains;
9. no self-referential evidence-head field remains;
10. one documentation-only evidence commit lands and its ordinary CI passes;
11. no additional commit is created merely to record that CI run;
12. temporary verification resources are removed;
13. crates.io publication and tag creation remain direct maintainer actions.

Until all thirteen conditions hold, describe the repository as implementation-complete and release-ready in substance, but evidence closure still open.
