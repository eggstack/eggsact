# CI Simplification Closure Pass

## Status

- **Status:** complete
- **Closure SHA:** a376e7c
- **Ordinary CI run:** [30406093095](https://github.com/eggstack/eggsact/actions/runs/30406093095) — success
- **Maintenance workflow dispatch:** [30418110835](https://github.com/eggstack/eggsact/actions/runs/30418110835) — success (MSRV + cargo-deny)
- **Parity workflow dispatch:** [30418110943](https://github.com/eggstack/eggsact/actions/runs/30418110943) — success
- **Checks:** Linux correctness; Check (windows-latest); Check (macos-latest)
- **Required-check configuration:** branch protection not enabled; no rulesets; no stale legacy names
- **Local release check:** passed (fmt, generated-docs, clippy, tests, doctests, cargo-deny, package, publish dry-run)
- **GitHub publication automation:** absent
- **Repository:** `eggstack/eggsact`
- **Target branch:** `main`
- **Plan baseline:** `5774529119b03e3bfff4406810c7ca6c66f84c9c`
- **Parent plan:** `plans/2026-07-28-ci-verification-release-simplification.md`
- **Scope:** validate the reduced CI model end to end, remove stale required-check expectations, correct residual workflow/documentation inconsistencies, and close the simplification effort without rebuilding an evidence-heavy release apparatus
- **Publication:** out of scope; crates.io publication and release tagging remain direct maintainer actions
- **Primary objective:** prove that the simplified verification model is operational and leave the repository with a stable, low-friction merge gate

## Purpose

Commit `5774529119b03e3bfff4406810c7ca6c66f84c9c` implemented the substantive simplification:

- ordinary CI was reduced from twelve jobs to one Linux correctness job and one Windows/macOS compile-check matrix;
- MSRV and dependency-policy checks were moved to maintenance workflows;
- duplicated release verification was deleted;
- PR fuzz smoke was removed;
- extended fuzzing became manual-only;
- parity reporting and provenance artifacts were removed;
- a local, non-publishing release check was added;
- verification and release doctrine were rewritten around manual publication.

The implementation is structurally aligned with the parent plan. The remaining work is closure, not redesign. The repository still needs evidence that the new workflow configuration parses and passes, confirmation that repository settings no longer require deleted job names, and a narrow audit for stale references or accidental duplication.

This pass must resist the repository's former tendency to turn closure into another large evidence project. One successful reduced CI run, a repository-settings check, and concise documentation correction are sufficient.

---

# Required outcome

After this pass:

1. the reduced ordinary CI workflow has completed successfully on a commit containing the simplified configuration;
2. the observed required check names are documented exactly as GitHub reports them;
3. branch protection or repository rulesets do not require any deleted legacy job names;
4. ordinary CI exposes only the intended Linux correctness check and the two platform matrix checks;
5. all workflow YAML files parse and their triggers match the four-tier verification doctrine;
6. no GitHub Actions workflow publishes to crates.io, creates tags, creates GitHub releases, or determines release cadence;
7. the local release-check script runs through its non-publishing path successfully;
8. stale badges, documentation, skills, comments, and plan text do not present the removed twelve-job/evidence-heavy model as current policy;
9. no new provenance, parity-report, package-manifest, or exact-SHA evidence artifact is introduced for closure;
10. the parent simplification plan and this closure plan are marked complete only after the operational checks pass.

---

# Non-goals and hard constraints

This pass must not:

- reintroduce separate fmt, generated-docs, Clippy, lib-test, bin-test, integration-test, doctest, MSRV, cargo-deny, or package jobs into ordinary CI;
- restore full Windows or macOS test suites without a reproduced platform-specific defect that cannot be detected by compile checks;
- restore PR fuzzing, sanitizer matrices, release-provenance generation, parity artifacts, or package-content evidence scripts;
- add crates.io tokens, trusted publishing, OIDC publication, release automation, tag automation, or GitHub Release creation;
- make scheduled/manual maintenance workflows required merge gates;
- require all maintenance workflows to pass on every ordinary change;
- create new release-readiness ledgers, exact-SHA evidence tables, artifact checksums, or immutable evidence bundles;
- rewrite product implementation or tests unrelated to a failure exposed by the reduced CI configuration;
- broaden scope into a new release process;
- publish a crate or create a release tag;
- mark closure complete from static YAML inspection alone.

If ordinary CI fails because of a real source, test, generated-doc, or packaging defect, fix that narrow defect. Do not weaken or remove the corresponding high-signal check merely to make the workflow green.

---

# Current intended verification model

The closure agent must preserve this model.

## Tier 1 — ordinary merge CI

Triggered on pull requests and pushes to `main`.

Expected checks:

```text
Linux correctness
Check (windows-latest)
Check (macos-latest)
```

The Linux job owns:

```bash
cargo fmt --all -- --check
cargo run --locked --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity
cargo test --locked --doc
cargo package --locked
```

The platform matrix owns only:

```bash
cargo check --locked --all-targets --all-features
```

## Tier 2 — scheduled ecosystem maintenance

Examples:

- MSRV validation;
- `cargo deny` policy checks;
- latest-compatible dependency drift;
- Python parity where retained on a schedule.

These detect drift and maintenance work. They are not ordinary merge gates.

## Tier 3 — manual deep verification

Examples:

- extended fuzzing;
- sanitizers;
- targeted compatibility investigations.

These run when risk, code changes, or maintainer judgment justify them.

## Tier 4 — local manual release verification

A maintainer executes the local release check, reviews the package, and then separately chooses whether to publish and tag.

GitHub Actions does not publish.

---

# Required execution sequence

Execute in this order:

1. freeze and inspect current `main`;
2. compare current `main` to implementation commit `5774529`;
3. statically audit workflow files and references;
4. trigger or obtain one ordinary CI run on the current simplified configuration;
5. inspect every job and failed step if the run is not green;
6. fix only defects required for the reduced CI model to operate;
7. rerun ordinary CI until all intended checks pass;
8. inspect repository branch protection/ruleset required checks;
9. remove stale required legacy check names and retain only intended merge checks;
10. execute the local release-check script without publishing;
11. perform a concise stale-reference audit;
12. update plan statuses and closure notes once;
13. stop.

Do not create a sequence of documentation-only evidence commits after operational success. Prefer one narrow implementation/settings correction commit and one final status update at most.

---

# Workstream 1 — Freeze the actual baseline

## Required inspection

Run:

```bash
git fetch origin main --prune
git rev-parse origin/main
git log --oneline --decorate -20 origin/main
git diff --stat 5774529119b03e3bfff4406810c7ca6c66f84c9c..origin/main
git diff --name-status 5774529119b03e3bfff4406810c7ca6c66f84c9c..origin/main
```

Classify every post-implementation change as one of:

- relevant workflow correction;
- relevant documentation correction;
- unrelated product work;
- superseding simplification work;
- evidence-only churn.

If later commits changed workflows, use current `main` as the operational baseline and document the delta. Do not blindly validate only the historical implementation SHA.

## Acceptance criteria

- The current `main` SHA is recorded in the closure commit message or plan status.
- Every workflow-affecting change after `5774529` has been inspected.
- No stale assumption about file names, job names, or triggers is used.

---

# Workstream 2 — Static workflow audit

Inspect every file under `.github/workflows/`.

## Ordinary CI checks

Confirm `.github/workflows/ci.yml`:

- triggers only on intended PR and `main` push events;
- contains one Linux correctness job;
- contains one two-entry platform matrix;
- uses `--locked` consistently;
- skips only the parity tests that require the external Python package;
- runs doctests explicitly;
- builds the publishable package;
- has no dependency on scheduled/manual workflows;
- has no artifact upload;
- has no release or publication permission;
- has no secrets requirement.

Check job and matrix naming carefully. GitHub required checks generally use rendered job names, not YAML job IDs. The expected visible names should be stable and human-readable.

## Maintenance workflow checks

Confirm maintenance workflows:

- do not run on every pull request;
- have explicit schedule and/or `workflow_dispatch` triggers;
- do not publish;
- do not upload routine evidence artifacts;
- fail normally when they find drift rather than attempting automated release changes.

## Manual deep-verification checks

Confirm fuzz/sanitizer workflows:

- have `workflow_dispatch` only unless a deliberately retained low-frequency schedule is documented;
- are not required merge checks;
- upload crash artifacts only when needed for debugging, not routine provenance;
- do not generate evidence ledgers.

## Deleted workflow checks

Confirm these concepts are absent:

- duplicated full release gate;
- GitHub Actions crates.io publication;
- tag creation;
- GitHub Release creation;
- release provenance JSON;
- parity report artifacts;
- package manifest assertion artifacts;
- PR fuzz smoke.

Useful searches:

```bash
grep -RInE 'cargo publish|crates\.io|CARGO_REGISTRY_TOKEN|id-token|gh release|git tag|release-provenance|parity-report|upload-artifact' .github scripts docs README.md AGENTS.md plans || true
grep -RInE 'Test \(lib\)|Test \(bins\)|Test \(integration\)|Generated Docs|cargo-deny|MSRV \(' .github docs README.md AGENTS.md plans || true
```

Interpret matches by context. Historical plan documents may name old checks, but they must be clearly marked historical or superseded.

## Acceptance criteria

- All YAML files are syntactically valid.
- Trigger classification matches the four-tier doctrine.
- Ordinary CI contains no accidental high-cost maintenance work.
- No publication path exists in GitHub Actions.
- No current normative document describes the removed workflow as active.

---

# Workstream 3 — Obtain a real reduced-CI run

Static review is insufficient. Obtain one real run against a commit that contains the simplified workflow.

Preferred methods, in order:

1. use the natural run created by the closure-plan or implementation follow-up commit;
2. push a no-op documentation correction only if a real correction is needed;
3. use `workflow_dispatch` only if the ordinary CI workflow supports it and the resulting job names/triggers are representative;
4. open a temporary branch/PR only if branch settings require PR context to expose required checks.

Do not create meaningless churn solely to manufacture evidence if an existing post-implementation run is available.

## Required run inspection

Record only:

- commit SHA;
- workflow run URL or ID;
- conclusion;
- visible job names;
- job conclusions;
- total duration if readily available.

Do not record toolchain hashes, cache keys, artifact checksums, package file manifests, or redundant step-by-step evidence.

Expected successful checks:

```text
Linux correctness
Check (windows-latest)
Check (macos-latest)
```

## Failure triage

If Linux correctness fails:

- identify the first failing command;
- reproduce it locally with the same command;
- classify it as source defect, generated-doc drift, lint failure, test failure, doctest failure, package failure, or CI-only configuration issue;
- apply the smallest correct fix;
- retain the check.

If Windows/macOS compile checks fail:

- inspect the exact target/compiler error;
- determine whether it is a real portability issue or shell/YAML mismatch;
- fix portability or runner syntax narrowly;
- do not replace `cargo check` with a weaker command.

If the workflow does not start:

- inspect YAML parse errors, trigger branches, permissions, workflow disablement, and repository Actions settings;
- correct the configuration;
- rerun.

## Acceptance criteria

- One workflow run on the simplified configuration completes successfully.
- Exactly three rendered checks are observed: one Linux and two platform checks.
- No deleted legacy job runs.
- No release, provenance, parity artifact, fuzz, MSRV, or cargo-deny work runs as part of ordinary CI.

---

# Workstream 4 — Repair required-check configuration

The YAML simplification does not automatically update branch protection or repository rulesets. Deleted check names can leave merges permanently blocked.

## Required inspection

Using GitHub repository settings, `gh api`, or the relevant administration interface, inspect:

- branch protection for `main`;
- repository rulesets targeting `main`;
- required status check names;
- whether strict up-to-date branch requirements are enabled;
- whether workflows are required by file/name rather than status context.

Example commands where permissions allow:

```bash
gh api repos/eggstack/eggsact/branches/main/protection

gh api repos/eggstack/eggsact/rulesets
```

Expected obsolete contexts may include:

```text
Check
Generated Docs
Clippy
Test (lib)
Test (bins)
Test (integration)
Test (doc)
MSRV (1.89.0)
Windows
macOS
cargo-deny
Package
Fuzz Smoke
Full Release Gate
```

Remove obsolete contexts.

Retain only the intended ordinary merge checks, using the exact names observed from the successful run:

```text
Linux correctness
Check (windows-latest)
Check (macos-latest)
```

If this small repository intentionally has no branch protection, document that fact briefly; do not add a complex ruleset merely because the plan mentions checking it.

## Important edge cases

- Matrix names may be reported with spacing or parentheses different from expectation. Use observed GitHub check names exactly.
- Required checks can remain stale even when no current workflow emits them.
- A ruleset and classic branch protection can both apply. Inspect both.
- Do not make maintenance workflows required.
- Do not require manual workflows.

## Acceptance criteria

- No deleted job name is required for merge.
- Required checks, if enabled, match checks emitted by current ordinary CI exactly.
- A representative PR is not blocked waiting for a nonexistent context.
- No new complicated ruleset is introduced.

---

# Workstream 5 — Validate the local release check

The release script is a convenience wrapper, not a second CI system.

## Required execution

From a clean checkout:

```bash
git status --short
./scripts/release-check.sh
```

If the script supports flags, run its documented default non-publishing mode.

Confirm it:

- exits nonzero on a failed command;
- does not publish;
- does not create or push a tag;
- does not create a GitHub Release;
- does not require GitHub credentials;
- does not upload artifacts;
- uses locked dependency resolution where applicable;
- leaves the worktree unchanged except for ordinary build outputs ignored by Git.

Expected conceptual command set:

```bash
cargo fmt --all -- --check
cargo run --locked --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity
cargo test --locked --doc
cargo deny check advisories bans licenses sources
cargo package --locked
cargo publish --locked --dry-run
```

The exact set may differ if `cargo deny` or dry-run publication is deliberately optional and documented. The script must remain short and readable; do not build a release state machine.

## Failure handling

If `cargo publish --dry-run` fails because the current development version is already published or otherwise intentionally non-publishable, do not add version mutation logic. Instead:

- verify `cargo package --locked` succeeds;
- document when dry-run should be executed during an actual versioned release;
- keep publication itself outside the script.

## Acceptance criteria

- The local release-check path has been executed successfully or a precise version-state limitation is documented.
- The script contains no publishing side effect.
- The script remains understandable without separate tooling or generated evidence.

---

# Workstream 6 — Stale-reference and naming cleanup

Audit normative surfaces:

```text
README.md
AGENTS.md
docs/verification.md
docs/release.md
.github/workflows/*.yml
scripts/release-check.sh
skills/**
current plans
```

## Required corrections

Correct only current-facing inconsistencies, including:

- “3-job CI” versus “two logical jobs / three rendered checks” ambiguity;
- badges pointing to deleted workflow files;
- instructions telling contributors to wait for old job names;
- release instructions requiring a GitHub release-verification workflow;
- references to routine provenance/parity artifacts;
- wording that implies GitHub Actions chooses release cadence;
- instructions that require exact-SHA evidence tables for ordinary releases.

Recommended terminology:

```text
one Linux correctness job plus a two-platform compile-check matrix
```

or:

```text
three rendered ordinary CI checks
```

Avoid calling it “three jobs” where that could confuse YAML job definitions with matrix-expanded checks.

## Historical documentation

Do not rewrite every old release plan. Historical documents may retain old details if prominently marked:

```text
Historical record — non-normative. Current verification and release policy is defined in docs/verification.md and docs/release.md.
```

One marker per historical document is sufficient. Do not maintain old evidence records further.

## Acceptance criteria

- Current-facing documentation uses consistent terminology.
- All workflow badges resolve to existing workflows.
- No normative instruction requires deleted checks or workflows.
- Historical evidence remains historical and is not expanded.

---

# Workstream 7 — Close plans once, without evidence churn

After all operational criteria pass:

1. update the parent plan status to `complete`;
2. update this closure plan status to `complete`;
3. record the final implementation/closure SHA;
4. record one successful ordinary CI run ID or URL;
5. state whether branch protection/rulesets were corrected or were not enabled;
6. state that the local release check was exercised;
7. stop updating historical release-evidence documents.

Recommended closure record:

```text
- Simplified CI operational on: <SHA>
- Ordinary CI run: <URL or ID>
- Checks: Linux correctness; Check (windows-latest); Check (macos-latest)
- Required-check configuration: corrected / not enabled
- Local release check: passed / package passed with documented dry-run version limitation
- GitHub publication automation: absent
```

This is enough. Do not add checksums, artifact identities, archive digests, complete logs, or repeated exact-SHA tables.

## Acceptance criteria

- Both plans accurately reflect completion.
- The closure record is concise and operationally useful.
- No follow-up documentation-only evidence loop is created.

---

# Explicit acceptance checklist

## Ordinary CI

- [ ] Current `main` and all post-`5774529` workflow changes were inspected.
- [ ] `.github/workflows/ci.yml` contains one Linux job and one two-entry platform matrix.
- [ ] Linux runs fmt, generated-doc check, Clippy, normal tests, doctests, and package construction.
- [ ] Windows and macOS run compile checks only.
- [ ] One real reduced-CI run completed successfully.
- [ ] The run emitted exactly the intended three rendered checks.
- [ ] No legacy ordinary CI job ran.

## Maintenance and deep verification

- [ ] MSRV and cargo-deny are not ordinary merge gates.
- [ ] Latest-compatible and parity are scheduled/manual drift checks.
- [ ] Extended fuzz/sanitizer work is manual or deliberately low-frequency and non-blocking.
- [ ] PR fuzz smoke remains deleted.
- [ ] Routine provenance and parity artifacts remain removed.

## Release ownership

- [ ] No workflow publishes to crates.io.
- [ ] No workflow creates tags or GitHub Releases.
- [ ] No workflow determines release cadence.
- [ ] `scripts/release-check.sh` was exercised in non-publishing mode.
- [ ] Manual release documentation is current and concise.

## Repository settings

- [ ] Classic branch protection was inspected.
- [ ] Repository rulesets were inspected.
- [ ] Deleted job names are not required.
- [ ] Current required checks, if enabled, exactly match emitted check names.
- [ ] A PR is not blocked by nonexistent status contexts.

## Documentation

- [ ] Current documentation distinguishes logical jobs from rendered matrix checks.
- [ ] Badges reference existing workflows.
- [ ] Normative docs do not reference the removed release-verification model.
- [ ] Historical release evidence is clearly non-normative.
- [ ] No new evidence ledger or artifact manifest was created.

## Closure

- [ ] Parent plan marked complete.
- [ ] Closure plan marked complete.
- [ ] One successful ordinary CI run recorded.
- [ ] Required-check disposition recorded.
- [ ] Local release-check disposition recorded.
- [ ] No additional closure pass is needed.

---

# Final completion standard

This line of work is complete when the repository demonstrates all of the following in practice:

```text
A normal change triggers three fast, understandable checks.
Linux performs the full correctness gate once.
Windows and macOS prove supported-platform compilation.
Maintenance and deep verification do not block iteration.
GitHub Actions never publishes the crate.
A maintainer can run one short local release check and then separately choose to publish.
No deleted status context blocks merging.
No evidence bureaucracy is required to prove any of the above.
```

Anything beyond that standard requires a separately justified defect or risk. Do not expand this closure pass merely to recreate the complexity that the parent plan removed.
