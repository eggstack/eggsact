# CI, Verification, and Release Simplification

## Status

- **Status:** complete
- **Final consistency cleanup:** ef8d905 — synchronized plan records and completed evidence-backed checklists
- **Repository:** `eggstack/eggsact`
- **Target branch:** `main`
- **Plan baseline:** `4b0bed63930e99b352d17c482b5e59dab366d7c5`
- **Scope:** reduce routine CI and verification complexity, separate merge verification from release verification, and codify direct maintainer publication to crates.io
- **Publication:** implementation of publishing is out of scope; releases remain direct maintainer actions
- **Primary objective:** restore fast iteration without materially weakening correctness controls

## Purpose

Eggsact currently has a verification apparatus that is substantially larger than the product requires. Routine CI is divided into many independent jobs that repeatedly install the toolchain, restore separate caches, compile overlapping target graphs, and run segmented forms of the same test suite. Additional manual and scheduled workflows repeat much of the same work, generate evidence artifacts, and encourage exact-SHA evidence bookkeeping in repository documents.

This plan reduces that apparatus to a small, explicit verification model:

1. ordinary CI answers whether a change is safe to merge;
2. scheduled or manual compatibility checks detect ecosystem drift without blocking normal iteration;
3. a short local release check answers whether a selected version is ready to publish;
4. `cargo publish --locked` and release tagging remain deliberate maintainer actions outside GitHub Actions;
5. workflow logs and immutable package/tag records are the evidence, rather than manually duplicated evidence documents.

The goal is not to remove testing. The goal is to remove duplicated orchestration, repeated compilation, low-value artifacts, and release-process ceremony that does not improve the shipped crate.

---

# Required outcome

After this pass:

1. ordinary pull-request and `main` CI has no more than two logical verification families: one Linux correctness job and one supported-platform compile matrix;
2. formatting, generated-doc checking, Clippy, the complete normal test suite, and package construction run in one Linux job;
3. Windows and macOS perform compilation checks rather than duplicate the entire test suite, unless a specific platform-dependent test is documented and retained separately;
4. MSRV, dependency-policy, latest-compatible, parity, fuzzing, and sanitizer work are classified as scheduled/manual checks rather than ordinary merge gates;
5. the duplicated GitHub release-verification workflow is deleted;
6. no GitHub Actions workflow publishes to crates.io, creates a release tag, or determines release cadence;
7. `docs/verification.md` defines the reduced verification doctrine and ownership of each check;
8. `docs/release.md` defines a short local, manual crates.io release process;
9. release verification is codified through a small local script or equivalent task that cannot publish by itself;
10. release-status/evidence documentation no longer requires run-ID, artifact-digest, exact-SHA, package-file-count, or duplicated test-count maintenance for future releases;
11. branch protection, badges, and documentation reference only checks that still exist;
12. the reduced workflows pass on Linux, Windows, and macOS after implementation.

---

# Non-goals and hard constraints

This pass must not:

- weaken public API behavior, calculator parity semantics, cancellation semantics, regex safety, or deterministic execution contracts;
- delete normal unit, integration, binary, or doctest coverage;
- remove generated documentation consistency checking;
- remove Clippy warning enforcement from ordinary CI;
- remove supported-platform compilation checks;
- automate crates.io publication;
- publish a new crate version;
- create or move a release tag;
- require GitHub Actions approval before a maintainer may publish;
- retain duplicated checks solely to preserve old release-evidence documents;
- introduce a new task runner or large dependency merely to replace a short shell script;
- create a bespoke provenance system;
- require artifact uploads for passing parity, dependency, MSRV, fuzz, sanitizer, package, or release checks;
- make scheduled compatibility workflows required branch-protection checks;
- gate documentation-only changes on expensive scheduled/manual verification families;
- broaden this pass into test-suite redesign or production-code refactoring.

A reduction is successful only when the retained checks remain easy to run locally and failures remain actionable.

---

# Verification doctrine to codify

The implementation must explicitly document four verification tiers.

## Tier 1 — Ordinary merge CI

Required on pull requests and pushes to `main`:

```text
Linux correctness
  - cargo fmt --all -- --check
  - cargo run --locked --bin generate-docs -- --check
  - cargo clippy --locked --all-targets --all-features -- -D warnings
  - cargo test --locked --all-features
  - cargo package --locked

Supported-platform compilation
  - Windows: cargo check --locked --all-targets --all-features
  - macOS:   cargo check --locked --all-targets --all-features
```

These checks are merge-blocking because they directly establish source quality, behavioral correctness, generated-doc consistency, packaging viability, and supported-platform compilation.

## Tier 2 — Scheduled compatibility and policy checks

Run weekly and/or through `workflow_dispatch`, but do not block ordinary merges:

- MSRV compilation and a focused test command;
- `cargo deny check advisories bans licenses sources`;
- latest-compatible dependency resolution;
- Python `eggcalc` parity against the currently published dependency.

These checks detect drift in external dependencies, policy databases, old toolchains, or the Python reference implementation. They are valuable but do not need to execute on every source edit.

## Tier 3 — Targeted hardening

Run manually before material releases or after relevant implementation changes:

- extended fuzz matrices;
- sanitizer runs;
- long repeated concurrency/interleaving loops;
- focused regression campaigns.

These checks should remain available, but ordinary development must not wait on them unless they have found a current reproducible defect.

## Tier 4 — Manual release verification

Run locally by the maintainer from the exact source intended for publication. This tier verifies package readiness and then leaves the actual publish command separate and explicit.

---

# Required execution sequence

Execute in this order:

1. inspect the current workflow and documentation inventory;
2. record the current branch-protection check names if accessible;
3. design the reduced workflow topology before deleting files;
4. consolidate ordinary Linux CI;
5. convert Windows and macOS to compile-only matrix checks;
6. move MSRV and dependency-policy checks out of ordinary CI;
7. simplify parity and latest-compatible workflows;
8. preserve fuzz/sanitizer workflows only as scheduled/manual hardening;
9. delete duplicated release-verification automation;
10. add the local non-publishing release-check command;
11. rewrite verification and release policy documentation;
12. archive or clearly deprecate historical evidence documents without rewriting historical facts;
13. update badges, branch-protection documentation, and workflow references;
14. run local verification;
15. push the implementation and confirm the reduced workflow set passes;
16. record only the resulting implementation commit and concise closure summary in this plan.

Do not create a second evidence-only closure cycle. Closure should be recorded once after the implementation workflows pass.

---

# Workstream 1 — Inventory and freeze the current apparatus

## Files and surfaces to inspect

At minimum inspect:

```text
.github/workflows/ci.yml
.github/workflows/python-parity.yml
.github/workflows/release-verification.yml
.github/workflows/*msrv*
.github/workflows/*latest*
.github/workflows/*fuzz*
.github/workflows/*sanitizer*
.github/workflows/*release*
scripts/
docs/release.md
docs/release-readiness.md
docs/release-*-status.md
README.md
AGENTS.md
Cargo.toml
deny.toml
```

Use repository search rather than assuming every historical workflow still exists.

## Required inventory table

Before editing, create a temporary implementation note containing:

| Check/workflow | Trigger | Current jobs | Duplicates ordinary CI? | Merge-blocking value | New tier | Action |
|---|---|---:|---|---|---|---|
| CI | PR/push/manual | current count | n/a | high | Tier 1 | consolidate |
| Release Verification | manual | current count | yes | low as separate workflow | Tier 4 local | delete |
| Python Parity | weekly/manual | current count | partially | drift only | Tier 2 | simplify |
| MSRV | current trigger | current count | partially | drift/toolchain | Tier 2 | move |
| cargo-deny | current trigger | current count | partially | policy/drift | Tier 2/Tier 4 | move |
| Latest-compatible | current trigger | current count | no | drift only | Tier 2 | retain/simplify |
| Fuzz/sanitizers | current trigger | current count | no | hardening | Tier 3 | manual/scheduled |

This table may live only in the implementation commit message or plan completion section; it does not need to become permanent documentation.

## Baseline commands

```bash
git fetch origin main --prune
git status --short
git log --oneline --decorate -20 origin/main
find .github/workflows -maxdepth 1 -type f -print | sort
find scripts docs -maxdepth 2 -type f -print | sort
```

## Acceptance criteria

- Every active workflow is classified into one of the four tiers.
- No workflow is deleted before its unique responsibility is identified.
- Branch-protection implications are known.
- Historical release records are distinguished from current policy documents.

---

# Workstream 2 — Consolidate ordinary Linux CI

## Required topology

Replace the fragmented Linux jobs in `.github/workflows/ci.yml` with one job named stably, preferably `Linux correctness` or `ci-linux`.

Recommended structure:

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]
  workflow_dispatch:

concurrency:
  group: ci-${{ github.ref }}
  cancel-in-progress: ${{ github.event_name == 'pull_request' }}

permissions:
  contents: read

jobs:
  linux:
    name: Linux correctness
    runs-on: ubuntu-latest
    timeout-minutes: 30
    steps:
      - uses: actions/checkout@<pinned-sha>
      - uses: dtolnay/rust-toolchain@<pinned-sha>
        with:
          toolchain: stable
          components: clippy,rustfmt
      - uses: Swatinem/rust-cache@<pinned-sha>
      - run: cargo fmt --all -- --check
      - run: cargo run --locked --bin generate-docs -- --check
      - run: cargo clippy --locked --all-targets --all-features -- -D warnings
      - run: cargo test --locked --all-features
      - run: cargo package --locked
```

Retain the repository’s existing action-SHA pinning policy. Do not replace pinned action commits with floating tags during this pass.

## Test command rule

Prefer one complete command:

```bash
cargo test --locked --all-features
```

This should replace separate `--lib`, `--bins`, `--tests`, and `--doc` jobs unless the unified command demonstrably omits an existing target. Verify the target coverage from Cargo output.

If parity tests are embedded in an integration target and require Python, retain the minimum exclusion needed for ordinary CI. Prefer an explicit feature or separate test target over a broad name filter, but do not redesign the test architecture in this pass. If the existing command must remain:

```bash
cargo test --locked --all-features -- --skip parity
```

then separately prove doctests still run or add one local `cargo test --locked --doc` step inside the same Linux job. The key requirement is one runner and one compilation cache, not dogmatic command count.

## Package rule

Retain:

```bash
cargo package --locked
```

Remove a separate downstream package job and its `needs` fan-in.

Do not retain shell assertions for package contents unless they protect against a documented recurring defect not already enforced by `Cargo.toml` `include`/`exclude` metadata. If package exclusions are important, encode them declaratively in `Cargo.toml` and add one focused test only if necessary.

## Acceptance criteria

- One Linux runner performs the ordinary Rust correctness gate.
- The Linux job contains no artifact upload.
- The workflow contains no final fan-in package job.
- A single cache is reused across the Linux checks.
- All normal tests that previously ran in ordinary CI still run.
- Failure output identifies the failing command without requiring separate jobs.
- The job passes from a clean checkout.

---

# Workstream 3 — Reduce cross-platform duplication

## Required matrix

Use one matrix job for supported non-Linux platforms:

```yaml
  platform-check:
    name: Check (${{ matrix.os }})
    strategy:
      fail-fast: false
      matrix:
        os: [windows-latest, macos-latest]
    runs-on: ${{ matrix.os }}
    timeout-minutes: 25
    steps:
      - uses: actions/checkout@<pinned-sha>
      - uses: dtolnay/rust-toolchain@<pinned-sha>
        with:
          toolchain: stable
      - uses: Swatinem/rust-cache@<pinned-sha>
      - run: cargo check --locked --all-targets --all-features
```

## Platform-specific exceptions

A test may remain on Windows or macOS only when all of the following are true:

1. it exercises platform-specific path, process, signal, filesystem, encoding, or transport behavior;
2. Linux cannot establish the same contract;
3. the test is focused rather than the entire suite;
4. the reason is documented next to the workflow step.

Example acceptable exception:

```yaml
- name: Windows path regression
  if: runner.os == 'Windows'
  run: cargo test --locked windows_path_round_trip
```

Example unacceptable duplication:

```yaml
cargo test --locked --all-features --lib
cargo test --locked --all-features --bins
cargo test --locked --all-features --tests -- --skip parity
cargo test --locked --doc
```

## Acceptance criteria

- Windows and macOS compile all targets and features.
- Full test-suite duplication is removed.
- Any retained platform test has a documented platform-specific rationale.
- Matrix failures remain independent through `fail-fast: false`.
- The matrix is not serialized behind unrelated scheduled checks.

---

# Workstream 4 — Move MSRV and dependency policy out of ordinary CI

## MSRV

Remove the MSRV job from ordinary `ci.yml`.

Place MSRV in a weekly/manual workflow, either a new small `maintenance.yml` or an existing compatible workflow. Recommended command set:

```bash
cargo check --locked --all-targets --all-features
cargo test --locked --all-features --lib
```

Do not rerun binaries, integration tests, and doctests under MSRV unless a known compatibility issue requires them. The principal MSRV contract is compilation against `package.rust-version`; a focused library test pass is sufficient additional confidence.

The workflow must read the MSRV from `Cargo.toml` or keep a single clearly documented value. Avoid duplicating `1.89.0` across multiple workflow and release files.

## cargo-deny

Remove `cargo-deny` installation and execution from ordinary `ci.yml`.

Run it:

- weekly through the maintenance workflow; and
- locally as part of release verification.

Recommended scheduled command:

```bash
cargo install cargo-deny --version 0.19.0 --locked
cargo deny check advisories bans licenses sources
```

If installation time is a concern, use a maintained action or binary cache only if doing so reduces code and operational complexity. Do not add a custom installation framework.

## Path filters

The maintenance workflow may use a weekly schedule and `workflow_dispatch`. It does not need push/PR triggers. If the implementation retains event-based execution, constrain it to `Cargo.toml`, `Cargo.lock`, `deny.toml`, and the maintenance workflow itself.

## Acceptance criteria

- Ordinary CI contains neither MSRV nor cargo-deny jobs.
- MSRV and cargo-deny remain runnable through GitHub manually and locally.
- Their scheduled failures do not mark unrelated PRs as failed.
- The documented MSRV has one canonical source.
- No release depends on an uploaded MSRV or cargo-deny artifact.

---

# Workstream 5 — Simplify parity and dependency-drift checks

## Python parity

Retain the weekly/manual parity signal, but remove evidence production.

Target workflow:

```yaml
name: Python Parity

on:
  schedule:
    - cron: "0 6 * * 1"
  workflow_dispatch:

permissions:
  contents: read

jobs:
  parity:
    runs-on: ubuntu-latest
    timeout-minutes: 20
    steps:
      - uses: actions/checkout@<pinned-sha>
      - uses: dtolnay/rust-toolchain@<pinned-sha>
        with:
          toolchain: stable
      - uses: actions/setup-python@<pinned-sha>
        with:
          python-version: "3.12"
      - run: pip install eggcalc
      - run: python -m pip show eggcalc
      - run: cargo test --locked --test lib parity -- --nocapture
```

Remove:

- parity JSON generation;
- artifact upload;
- artifact retention policy;
- shell extraction of repository/package provenance;
- documentation requirements to record every passing run.

A failed workflow is the actionable output. The installed package version printed in the log is sufficient context.

## Latest-compatible dependencies

Retain as weekly/manual drift detection. It should perform only the minimum commands needed to prove that the newest semver-compatible dependency set resolves, compiles, and passes a focused test suite.

Do not make it a release prerequisite unless it currently exposes a real unresolved compatibility defect. A locked release is allowed to publish from `Cargo.lock` even when a future-compatible dependency has drifted.

## Acceptance criteria

- Parity still checks against the latest published `eggcalc`.
- Parity uploads no artifacts.
- Latest-compatible remains available but non-blocking.
- Passing scheduled runs require no repository documentation updates.
- A failure contains enough log context to reproduce locally.

---

# Workstream 6 — Reclassify fuzzing, sanitizers, and long-loop verification

## Trigger policy

Fuzz and sanitizer workflows must use only:

- `workflow_dispatch`; and optionally
- a low-frequency schedule justified by runtime and maintenance value.

They must not run on every pull request or push to `main`.

## Matrix reduction review

Inspect each current fuzz/sanitizer matrix entry and classify it:

- unique target with distinct parser/normalizer surface: retain;
- duplicate sanitizer/compiler combination with little incremental signal: remove;
- historical target for removed behavior: delete;
- long-duration target useful only before release: manual only;
- short smoke target useful weekly: scheduled.

The implementation does not need to delete fuzz targets themselves. It should reduce workflow orchestration and cadence.

## Crash handling

Retain crash artifacts only when a run actually fails and produces a reproducer. Do not upload passing-run provenance or empty artifact bundles.

Example:

```yaml
- name: Upload crash reproducer
  if: failure()
  uses: actions/upload-artifact@<pinned-sha>
  with:
    name: fuzz-crash-${{ matrix.target }}
    path: fuzz/artifacts/${{ matrix.target }}/
    if-no-files-found: ignore
```

## Release relationship

Documentation should state:

- hardening runs are recommended before releases that materially change parsing, regex, normalization, concurrency, or execution machinery;
- they are not universally required for every patch release;
- a known reproducible crash remains release-blocking until fixed or explicitly scoped out;
- absence of a fresh fuzz run is not itself a release blocker when the release does not affect those surfaces.

## Acceptance criteria

- Routine PRs do not dispatch extended fuzz/sanitizer matrices.
- Passing runs produce no provenance artifacts.
- Failed runs can still preserve reproducers.
- The retained matrix is justified by unique coverage rather than evidence count.
- Documentation distinguishes targeted hardening from ordinary correctness.

---

# Workstream 7 — Delete GitHub release verification

## Required deletion

Delete `.github/workflows/release-verification.yml` after its unique useful commands have been moved to the local release check.

The following responsibilities must not remain as a separate GitHub workflow:

- rerunning ordinary CI commands;
- package-list evidence generation;
- publish dry run;
- provenance JSON generation;
- release artifact upload;
- exact-SHA release approval.

## Explicit policy

Add this normative statement to `docs/release.md`:

> Eggsact releases are initiated manually by a maintainer. GitHub Actions does not publish crates, create release tags, approve a release candidate, or determine release cadence. Ordinary CI establishes merge correctness. The maintainer runs the local release check against the selected source revision, publishes directly to crates.io, verifies publication, and then creates the annotated version tag.

## Repository search

Search for and remove stale references:

```bash
rg -n "Release Verification|Full Release Gate|release-provenance|provenance artifact|publish.*workflow|workflow.*publish" .
```

Historical documents may mention past workflow runs as historical facts. Current policy and instructions must not direct maintainers to use the deleted workflow.

## Acceptance criteria

- No release-verification workflow remains.
- No GitHub workflow contains `cargo publish` without `--dry-run`.
- Preferably no GitHub workflow contains even the dry-run; the dry-run belongs to the local release check.
- No workflow creates or pushes tags.
- No workflow uploads release-provenance artifacts.
- Current documentation states that release cadence and publication are manual.

---

# Workstream 8 — Add a local, non-publishing release check

## Preferred implementation

Create `scripts/release-check.sh` using portable Bash appropriate for the project’s maintainer environment.

Required properties:

- `set -euo pipefail`;
- runs from repository root or resolves repository root safely;
- refuses a dirty worktree unless an explicit documented override is supplied;
- prints each phase clearly;
- never runs `cargo publish` without `--dry-run`;
- never creates or pushes a tag;
- does not write evidence files into the repository;
- exits nonzero on the first failed check.

Recommended script body:

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT=$(git rev-parse --show-toplevel)
cd "$ROOT"

if [[ -n "$(git status --porcelain)" ]]; then
  echo "release-check: working tree is not clean" >&2
  exit 1
fi

cargo fmt --all -- --check
cargo run --locked --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features
cargo deny check advisories bans licenses sources
cargo package --locked
cargo publish --locked --dry-run

echo "release-check: passed; no publication was performed"
```

Adapt the test command only as needed for parity exclusions already discussed.

## Tool installation

The script may check that `cargo-deny` exists and print a direct installation command if absent. It should not silently install global tools during release verification.

Example:

```bash
if ! command -v cargo-deny >/dev/null 2>&1; then
  echo "cargo-deny is required: cargo install cargo-deny --version 0.19.0 --locked" >&2
  exit 1
fi
```

## Separation from publication

Document publication as separate commands, never embedded in the script:

```bash
cargo publish --locked
# verify crates.io accepted the immutable version
git tag -a vX.Y.Z -m "eggsact vX.Y.Z"
git push origin vX.Y.Z
```

The version tag must be created only after successful crates.io publication, because crates.io versions are immutable and a failed publication attempt may require a version correction before tagging.

## Acceptance criteria

- `scripts/release-check.sh` passes from a clean checkout with required local tools installed.
- It fails on a dirty worktree.
- It performs a publish dry run.
- It cannot publish or tag.
- Its command list is short enough to audit directly.
- `docs/release.md` uses the script as the canonical pre-publication entry point.

---

# Workstream 9 — Rewrite verification and release documentation

## `docs/verification.md`

Create this document if it does not exist. It must contain:

1. the four-tier verification doctrine;
2. exact ordinary CI commands;
3. supported-platform compile policy;
4. scheduled MSRV, dependency-policy, latest-compatible, and parity policy;
5. manual fuzz/sanitizer policy;
6. failure ownership and expected response;
7. a statement that workflow logs are evidence and passing runs do not require documentation commits.

Recommended failure ownership table:

| Failure | Blocks merge? | Blocks release? | Expected response |
|---|---|---|---|
| Linux correctness | yes | yes | fix before merge |
| Windows/macOS compile | yes | yes for supported platforms | fix or explicitly change support policy |
| Weekly MSRV | no immediate PR block | yes if advertised MSRV is broken | repair or raise MSRV deliberately |
| cargo-deny advisory/policy | no immediate PR block | maintainer judgment; security findings normally block | triage dependency/policy |
| Python parity | no immediate PR block | blocks only when changed behavior promises parity | reproduce and classify drift |
| latest-compatible | no | no for locked release unless defect affects users | triage ecosystem drift |
| fuzz/sanitizer crash | not automatically tied to unrelated PR | yes while reproducible and in release scope | minimize and fix/classify |

## `docs/release.md`

Reduce the document to the actual maintainer procedure:

1. choose and bump the version;
2. update changelog and generated docs as applicable;
3. ensure `main` CI is green;
4. run `scripts/release-check.sh` locally;
5. run `cargo publish --locked` manually;
6. verify the crate is live and not yanked;
7. create and push the annotated tag;
8. advance the development version only if the project policy requires it.

Include immutable-version guidance:

- crates.io does not permit replacing an uploaded version;
- after a successful upload, any correction requires a new version;
- do not move a published version tag to different source;
- if publication fails before acceptance, correct the cause and rerun only after confirming whether crates.io accepted the version.

## Historical evidence documents

Do not rewrite historical records to pretend the old apparatus never existed. Instead:

- mark release-readiness/status/evidence files as historical snapshots if they remain useful;
- add a short banner stating that future releases follow `docs/release.md` and do not require equivalent evidence ledgers;
- move them under `docs/history/` only if link maintenance is straightforward;
- delete purely duplicative generated evidence files when no durable historical value exists.

Do not create new per-release evidence templates.

## Acceptance criteria

- A new maintainer can distinguish merge checks, maintenance checks, hardening checks, and publication steps.
- Current release instructions contain no GitHub release gate.
- Passing workflows require no run-ID transcription.
- Historical documents are clearly non-normative.
- The manual publication policy is explicit and unambiguous.

---

# Workstream 10 — Update branch protection, badges, and references

## Branch protection

If repository settings currently require individual old job names such as `Check`, `Generated Docs`, `Clippy`, `Test (lib)`, `Test (bins)`, `Test (integration)`, `Test (doc)`, `MSRV`, `Windows`, `macOS`, `cargo-deny`, or `Package`, update required checks to the reduced names only after the new workflow has produced them at least once.

Recommended required checks:

```text
Linux correctness
Check (windows-latest)
Check (macos-latest)
```

If the GitHub connector or implementation environment cannot modify branch protection, record this as an explicit maintainer follow-up. Do not leave the repository in a state where merging is blocked by deleted check names.

## Badges and references

Search:

```bash
rg -n "actions/workflows|Generated Docs|Test \(lib\)|cargo-deny|MSRV|Release Verification|Full Release Gate" README.md docs plans AGENTS.md .github
```

Update active documentation and badges. Historical plan files do not need retroactive editing unless they are linked as current instructions.

## Acceptance criteria

- No active badge points to a deleted workflow.
- No current document names deleted checks as required.
- Branch protection does not require nonexistent contexts.
- Scheduled/manual checks are not required merge contexts.

---

# Workstream 11 — Local and remote verification

## Local verification

From a clean checkout:

```bash
cargo fmt --all -- --check
cargo run --locked --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features
cargo package --locked
```

Then verify the release script’s safety:

```bash
scripts/release-check.sh
rg -n "cargo publish( |$)" scripts .github/workflows docs
rg -n "git tag|git push.*v" scripts .github/workflows
```

Expected:

- only documentation may show the actual publish/tag commands;
- the script contains only `cargo publish --locked --dry-run`;
- workflows contain no actual publish or tag command.

Validate workflow syntax through an available YAML parser or GitHub dispatch. Avoid adding a new permanent validator dependency solely for this plan.

## Remote verification

After pushing implementation:

1. confirm the CI workflow exposes only the intended Linux and platform matrix checks;
2. confirm Linux correctness passes;
3. confirm Windows compile passes;
4. confirm macOS compile passes;
5. manually dispatch each simplified maintenance/parity workflow once;
6. confirm no release-verification workflow remains in the Actions UI;
7. confirm no passing workflow uploads obsolete provenance/parity artifacts.

Do not run an actual crate publication as verification for this plan.

## Acceptance criteria

- Reduced ordinary CI passes on all supported platforms.
- Scheduled/manual workflows can be dispatched successfully.
- No release has been published.
- No tag has been created.
- The workflow count and job count are materially lower than baseline.
- Normal PR iteration no longer waits for MSRV, cargo-deny, parity, latest-compatible, fuzz, sanitizer, or release-provenance work.

---

# Explicit deletion and retention matrix

The implementation agent must produce a final matrix equivalent to the following, adjusted for actual repository files:

| Surface | Action | Reason |
|---|---|---|
| Fragmented Linux CI jobs | consolidate | duplicated setup, compilation, and test orchestration |
| Full Windows suite | replace with compile check | platform support signal without full-suite duplication |
| Full macOS suite | replace with compile check | platform support signal without full-suite duplication |
| MSRV in ordinary CI | move to weekly/manual | slow-changing compatibility contract |
| cargo-deny in ordinary CI | move to weekly/manual + local release | advisory/policy drift, not every-edit correctness |
| Package fan-in job | fold into Linux job | no need for separate runner or dependency graph |
| Release Verification workflow | delete | duplicates CI and local release checks |
| Release provenance JSON/artifact | delete | duplicates Git/crates.io records |
| Parity report artifact | delete | workflow log already provides version and result |
| Python parity test | retain weekly/manual | useful reference drift detection |
| Latest-compatible test | retain weekly/manual | useful ecosystem drift detection |
| Fuzz targets | retain | valuable hardening assets |
| Extended fuzz/sanitizer cadence | manual/low-frequency | not appropriate for every iteration |
| Generated-doc check | retain in Linux CI | prevents committed generated-doc drift |
| Clippy `-D warnings` | retain in Linux CI | high-signal source-quality gate |
| Full normal Rust tests | retain in Linux CI | primary correctness gate |
| `cargo package --locked` | retain in Linux CI | catches package-construction failures |
| `cargo publish --dry-run` | local release only | exact package registry readiness check |
| Actual `cargo publish` | manual maintainer only | deliberate immutable release action |

---

# Quantitative closure targets

The pass is not complete unless it reaches these measurable outcomes, unless a documented repository fact makes one impossible:

- ordinary CI decreases from 12 jobs to 3 matrix-expanded jobs or fewer;
- ordinary Linux runner setup decreases from multiple independent jobs to one;
- ordinary CI performs no more than one complete normal test-suite execution;
- Windows and macOS each perform one all-target/all-feature compile command, plus only explicitly justified focused tests;
- release-specific GitHub workflows decrease to zero;
- passing parity and release checks upload zero artifacts;
- extended fuzz/sanitizer jobs are absent from PR and push triggers;
- current release documentation no longer asks maintainers to record workflow run IDs or artifact digests;
- actual publication remains exactly one manual `cargo publish --locked` command.

---

# Implementation guidance for trouble areas

## Unified test command unexpectedly runs parity

If `cargo test --locked --all-features` attempts Python parity and fails because `eggcalc` is unavailable, do not split the suite back into four CI jobs. Use one of these narrow solutions:

1. keep one Linux job and run the current segmented commands sequentially in that job;
2. add a test feature that enables parity only in the parity workflow, if the change is small and non-disruptive;
3. move parity cases to a clearly separate integration target.

The accepted result is one runner/cache, even if two test commands are temporarily necessary.

## Doctests omitted by filtered tests

If a global `--skip parity` filter causes uncertainty about doctest execution, add:

```bash
cargo test --locked --doc
```

inside the same Linux job. Do not restore a separate doctest runner.

## Windows-only failure

If Windows compilation exposes a real platform defect, fix the defect or document a support-policy change. Do not restore full Windows CI merely because a compile-only transition uncovered an unrelated issue.

## cargo-deny advisory appears between releases

A scheduled advisory failure should create an issue or maintainer task, not automatically invalidate unrelated merged work. Security-relevant reachable advisories should normally block publication until triaged. Unmaintained or informational advisories may be handled according to `deny.toml` policy.

## Historical evidence links

If moving historical files would break many links, leave them in place with a prominent non-normative banner. Reducing future maintenance is more important than reorganizing history.

## Branch protection cannot be edited

Land the new workflow first so its contexts exist. Then ask a maintainer to replace old required contexts. Do not delete the old workflow before this handoff when doing so would make `main` permanently unmergeable. A two-commit migration is acceptable:

1. add reduced workflow and establish new contexts;
2. update branch protection, then remove obsolete jobs/workflows.

---

# Final acceptance checklist

## Ordinary CI

- [x] One Linux correctness job owns formatting, generated docs, Clippy, normal tests, and packaging.
- [x] One Windows/macOS matrix owns supported-platform compilation.
- [x] Ordinary CI has no MSRV job.
- [x] Ordinary CI has no cargo-deny job.
- [x] Ordinary CI has no release-provenance or artifact upload.
- [x] Ordinary CI passes from a clean checkout.

## Scheduled/manual verification

- [x] MSRV remains available weekly/manually.
- [x] cargo-deny remains available weekly/manually.
- [x] Python parity remains available weekly/manually.
- [x] Latest-compatible remains available weekly/manually.
- [x] Fuzz/sanitizer workflows are manual or low-frequency scheduled only.
- [x] Passing scheduled/manual workflows do not require evidence commits.

## Release apparatus

- [x] `.github/workflows/release-verification.yml` is deleted.
- [x] No workflow publishes to crates.io.
- [x] No workflow creates or pushes tags.
- [x] `scripts/release-check.sh` exists and cannot publish.
- [x] `docs/release.md` declares manual maintainer publication and cadence.
- [x] Immutable crates.io version handling is documented.

## Documentation and repository configuration

- [x] `docs/verification.md` defines the four-tier doctrine.
- [x] Historical evidence documents are marked non-normative or archived.
- [x] Current docs do not require run-ID/artifact-digest bookkeeping.
- [x] Badges reference active workflows.
- [x] Branch protection requires only existing ordinary CI contexts.

## Closure

- [x] The implementation commit is identified.
- [x] Reduced CI passes on Linux, Windows, and macOS.
- [x] Simplified maintenance/parity workflows dispatch successfully.
- [x] No crate version was published during implementation.
- [x] No release tag was created during implementation.
- [x] This plan is updated once with a concise completion record and no evidence-only follow-up cycle.

---

# Completion record template

When implementation is complete, append only this concise record:

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

Do not add package checksums, artifact archive digests, repeated test-count tables, duplicated command transcripts, or a second exact-SHA evidence ledger.
