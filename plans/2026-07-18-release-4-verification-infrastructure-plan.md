# Release 4 — Verification Infrastructure Plan

## Purpose

Release 4 establishes reproducible, cross-platform, dependency-aware, package-aware, and parity-aware verification for eggsact. It does not materially expand runtime behavior or add tools. Its purpose is to make release claims evidence-backed and repeatable.

This plan depends on closure of `plans/2026-07-18-releases-1-3-final-correctness-plan.md`. Release 4 may be implemented in parallel, but its final acceptance gate must run against a commit where Releases 1–3 are closed.

## Release objective

A candidate release commit must be validated across:

- the declared minimum supported Rust version;
- stable Rust;
- Linux, Windows, and macOS for the supported surface;
- dependency advisories, licenses, bans, and source policy;
- locked and latest-compatible dependency graphs;
- Python eggcalc parity where the reference package is available;
- generated documentation consistency;
- package contents and publish dry-run behavior;
- manual crates.io release policy with no CI publishing credentials.

## Non-goals

Release 4 must not:

- publish automatically to crates.io;
- add broad fuzzing or benchmarks;
- introduce Cargo feature decomposition;
- split the crate;
- change MCP protocol behavior except where a cross-platform defect requires a narrow fix;
- add new tools;
- claim support for a platform that is not tested;
- require Python parity on every ordinary pull request if that would make core CI unreliable.

---

# Workstream 1 — Declare and enforce MSRV

## MSRV selection

Determine the actual minimum Rust version by testing the full default package and all supported binaries, not by guessing from dependency metadata.

Required process:

1. Inspect direct dependencies and their declared `rust-version` requirements.
2. Test candidate Rust versions in ascending order.
3. Select the lowest version that can:
   - resolve the locked dependency graph;
   - build the library;
   - build all binaries;
   - run the supported test subset;
   - build documentation;
   - package successfully.
4. Record any dependency that prevents a lower MSRV.

## Manifest and documentation

Add to `Cargo.toml`:

```toml
rust-version = "<selected-version>"
```

Update:

- `README.md`
- `docs/compatibility-policy.md`
- `docs/contributing.md`
- `docs/release.md`
- `AGENTS.md`

State:

- the selected MSRV;
- which package features and binaries it covers;
- how and when MSRV may be raised;
- that an MSRV increase requires at least a minor release and changelog entry unless project policy says otherwise.

## CI job

Add a dedicated MSRV job using the exact declared toolchain.

The job should run at minimum:

```bash
cargo check --all-targets --all-features
cargo test --all-features --lib
cargo test --all-features --bins
cargo test --doc
```

If some integration tests depend on newer tooling rather than language compatibility, document and minimize that exception.

## Acceptance criteria

- `Cargo.toml` declares `rust-version`.
- CI tests that exact toolchain.
- The locked dependency graph resolves on MSRV.
- Documentation defines the MSRV policy.
- A test failure on MSRV blocks the package/release gate.

---

# Workstream 2 — Track `Cargo.lock` and define dependency graph policy

## Lockfile

Track `Cargo.lock` in the repository because eggsact ships binaries and requires reproducible CI and packaging evidence.

Requirements:

- Remove any ignore rule that excludes `Cargo.lock`.
- Generate the lockfile with the current supported stable toolchain.
- Commit it.
- CI ordinary jobs use `--locked` where appropriate.
- Release verification fails if the lockfile is stale.

## Locked graph job

The primary stable jobs should use:

```bash
cargo check --locked --all-targets --all-features
cargo test --locked --all-features ...
cargo package --locked
```

## Latest-compatible graph job

Add a separate job that intentionally refreshes within semver-compatible constraints:

```bash
cargo update
cargo check --all-targets --all-features
cargo test --all-features --lib
cargo test --all-features --bins
cargo test --all-features --tests -- --skip parity
```

This job should detect upcoming breakage without changing the committed lockfile.

Decide whether it is:

- required on every pull request; or
- scheduled and non-blocking until triaged.

Preferred policy:

- locked graph is blocking on every push/PR;
- latest-compatible graph runs weekly and via `workflow_dispatch`;
- known upstream failures require an issue or status note, not silent exclusion.

## Acceptance criteria

- `Cargo.lock` is committed and current.
- Blocking jobs use `--locked`.
- A latest-compatible dependency job exists.
- Lockfile and latest-compatible semantics are documented.

---

# Workstream 3 — Dependency, license, and source policy with cargo-deny

## Configuration

Review and update `deny.toml` to define:

- advisory policy;
- allowed licenses;
- denied or clarified licenses;
- duplicate-version policy;
- banned crates if applicable;
- allowed crate sources and registries;
- git dependency policy;
- yanked dependency handling.

Avoid overly broad `allow` entries. Every exception should include a reason and, where relevant, an expiry or follow-up issue.

## CI

Add a blocking job:

```bash
cargo deny check advisories bans licenses sources
```

Pin the cargo-deny installation strategy to a reviewed version or use a maintained action with explicit versioning.

## Release verification

Add cargo-deny to the canonical release checklist and local verification script.

## Acceptance criteria

- `cargo deny check` passes on the locked graph.
- CI blocks on advisories, disallowed licenses, banned dependencies, and unapproved sources according to policy.
- Exceptions are documented and minimal.
- Release docs include the same command.

---

# Workstream 4 — Cross-platform CI

## Support policy

Define the supported platform tier before adding matrices.

Suggested policy:

- Tier 1: Ubuntu latest, full test and package gate.
- Tier 2: Windows latest and macOS latest, targeted build/test gate for platform-independent and platform-specific behavior.
- Unsupported targets are not implied by successful cross-compilation alone.

## Windows job

Run at minimum:

```powershell
cargo fmt --all -- --check
cargo check --locked --all-targets --all-features
cargo test --locked --all-features --lib
cargo test --locked --all-features --bins
cargo test --locked --all-features --tests -- --skip parity
cargo test --doc
```

Review tests that assume:

- POSIX paths;
- shell quoting;
- executable file permissions;
- `/tmp`;
- Unix line endings;
- process signals;
- environment-variable case behavior.

Use explicit platform parameters where tools already support them. Do not skip broad suites merely to make Windows green; isolate truly platform-specific tests with documented guards.

## macOS job

Run the same core gate, with attention to:

- Apple Silicon compatibility where hosted runner architecture permits;
- filesystem case sensitivity assumptions;
- path normalization;
- subprocess and stdout behavior;
- line endings and Unicode filesystem behavior.

## Matrix design

Do not multiply every job across every OS unnecessarily. A practical layout is:

- Linux: full check, generated docs, Clippy, test partitions, package, cargo-deny.
- Windows: check plus full non-parity tests.
- macOS: check plus full non-parity tests.
- MSRV: Linux targeted gate.

## Acceptance criteria

- Windows and macOS jobs run on every pull request or push to `main`.
- Platform-specific skips are narrow and documented.
- Path, shell, and subprocess tests pass on their intended platforms.
- The release support matrix is documented.

---

# Workstream 5 — Scheduled Python parity verification

## Purpose

Python parity is important but depends on an external reference package and may not be suitable as a hard dependency for every ordinary CI run. Make it scheduled, reproducible, and visible.

## Workflow

Add a scheduled workflow, for example weekly, plus `workflow_dispatch`.

The job should:

1. Set up the supported Python version.
2. Install the pinned or explicitly selected eggcalc reference version.
3. Record the installed reference version in logs and artifacts.
4. Build eggsact.
5. Run the parity suite.
6. Compare failures with `tests/fixtures/accepted_parity_failures.txt`.
7. Fail if:
   - a previously passing parity case regresses;
   - the accepted-failure list contains a test that now passes without being removed;
   - an unknown failure appears;
   - the fixture format is invalid.

## Version policy

Decide whether parity tracks:

- a pinned reference version for reproducibility;
- latest published eggcalc for drift detection; or
- both.

Preferred design:

- pinned parity job is release-blocking evidence;
- latest-reference parity job is scheduled drift detection.

## Artifacts

Upload a compact parity report containing:

- eggsact commit SHA;
- eggsact package version;
- eggcalc version;
- Python version;
- passed count;
- accepted failure count;
- new failure count;
- stale accepted failure count.

Do not upload secrets or large build directories.

## Acceptance criteria

- Scheduled parity workflow exists and is manually runnable.
- Reference version is explicit.
- Unknown and stale accepted failures are detected.
- Release docs describe how to obtain current parity evidence.

---

# Workstream 6 — Package content and provenance checks

## Package inspection

Add a deterministic package-content check using:

```bash
cargo package --locked --list
cargo package --locked
```

Maintain an allowlist or structural assertions for expected package contents.

Verify that published packages exclude:

- `plans/`
- agent skill directories
- CI-only files
- local scripts not required by consumers
- internal data not needed at runtime
- secrets or environment files
- large generated artifacts

Verify that packages include:

- source required to build library and binaries;
- README and license;
- required generated source or schemas;
- public documentation assets needed by docs.rs;
- tests only when intentionally shipped.

## Provenance record

Generate a machine-readable release evidence file or CI artifact containing:

- commit SHA;
- package version;
- Rust stable version;
- MSRV;
- target OS;
- lockfile checksum;
- package checksum when available;
- cargo-deny result;
- test job conclusions;
- parity reference version and result;
- `cargo package --list` output.

This artifact is evidence only. It must not publish the crate.

## Publish dry-run

The package job should run:

```bash
cargo publish --dry-run --locked
```

If crates.io network access makes this unreliable in ordinary CI, retain it in a manually triggered release-verification workflow and run `cargo package --locked` on every PR.

## Acceptance criteria

- Package contents are inspected and constrained.
- Release evidence identifies the exact source commit and lockfile.
- Publish dry-run is part of the documented release gate.
- No GitHub workflow has crates.io publish credentials or executes `cargo publish` without `--dry-run`.

---

# Workstream 7 — CI workflow structure and reliability

## Workflow organization

Keep jobs independently diagnosable. Suggested jobs:

- `fmt`
- `generated-docs`
- `clippy`
- `test-lib`
- `test-bins`
- `test-integration`
- `test-doc`
- `msrv`
- `windows`
- `macos`
- `cargo-deny`
- `package`

Scheduled/manual workflows:

- `latest-compatible-dependencies`
- `python-parity`
- `release-verification`

## Concurrency and cancellation

Use workflow concurrency groups to cancel superseded PR runs while preserving `main` and scheduled evidence.

## Caching

Use Rust caching carefully:

- include OS, toolchain, and lockfile in cache keys;
- do not share incompatible target directories across MSRV/stable or OSes;
- cache failure must not fail the build;
- avoid caches that conceal stale generated outputs.

## Timeouts

Set explicit job timeouts high enough for legitimate builds but low enough to detect hangs. Record unusually slow tests rather than broadly increasing all timeouts.

## Permissions

Use least-privilege workflow permissions. Ordinary verification should require read-only repository contents.

## Acceptance criteria

- CI jobs are partitioned by failure domain.
- Superseded PR runs are cancelled.
- Workflow permissions are minimal.
- Caches are toolchain/OS/lockfile aware.
- Package depends on all blocking evidence jobs.

---

# Workstream 8 — Release process reconciliation

## Manual publishing policy

Preserve the explicit policy that crates.io publishing is performed directly by the maintainer from an authenticated local environment, not by GitHub Actions.

The canonical release sequence should be:

1. Verify clean working tree and intended commit.
2. Confirm version and changelog.
3. Run the full local gate with `--locked`.
4. Confirm current blocking CI is green.
5. Confirm current scheduled/manual parity evidence.
6. Run `cargo package --locked`.
7. Inspect package contents.
8. Run `cargo publish --dry-run --locked`.
9. Run `cargo publish --locked` locally.
10. Confirm crates.io publication.
11. Create and push the matching tag after publication succeeds.
12. Record release evidence and published checksum/link in the release-readiness note.

Do not tag before a successful crates.io publish if that conflicts with the repository’s established tag-after-publish policy.

## Release readiness document

Update the canonical readiness document with fields for:

- candidate commit;
- version;
- MSRV;
- stable toolchain;
- Linux/Windows/macOS results;
- cargo-deny result;
- locked graph result;
- latest-compatible result;
- parity result;
- package list review;
- publish dry-run;
- known deferred items;
- final maintainer sign-off.

## Acceptance criteria

- One canonical release procedure exists.
- CI verifies but does not publish.
- Tagging order is explicit.
- Readiness evidence can be reproduced by another maintainer.

---

# Suggested implementation sequence

1. Close the final Releases 1–3 correctness plan.
2. Select and document MSRV.
3. Commit `Cargo.lock` and convert blocking jobs to `--locked`.
4. Add cargo-deny policy and CI.
5. Add Windows and macOS jobs.
6. Add latest-compatible scheduled workflow.
7. Add pinned and/or latest Python parity workflow.
8. Add package-content assertions and evidence artifact.
9. Add release-verification workflow without publish credentials.
10. Reconcile release docs and readiness template.
11. Run the complete Release 4 gate.

Recommended commit structure:

1. `build: declare MSRV and track Cargo.lock`
2. `ci: enforce locked and dependency policy checks`
3. `ci: add Windows and macOS verification`
4. `ci: add latest-compatible and parity schedules`
5. `release: verify package contents and provenance`
6. `docs: finalize reproducible manual release process`

---

# Required verification matrix

## Linux stable

```bash
cargo fmt --all -- --check
cargo run --locked --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features --lib
cargo test --locked --all-features --bins
cargo test --locked --all-features --tests -- --skip parity
cargo test --locked --doc
cargo deny check advisories bans licenses sources
cargo package --locked --verbose
```

## MSRV

```bash
cargo check --locked --all-targets --all-features
cargo test --locked --all-features --lib
cargo test --locked --all-features --bins
cargo test --locked --doc
```

## Windows and macOS

```bash
cargo check --locked --all-targets --all-features
cargo test --locked --all-features --lib
cargo test --locked --all-features --bins
cargo test --locked --all-features --tests -- --skip parity
cargo test --locked --doc
```

## Latest-compatible

```bash
cargo update
cargo check --all-targets --all-features
cargo test --all-features --lib
cargo test --all-features --bins
cargo test --all-features --tests -- --skip parity
```

## Package/release

```bash
cargo package --locked --list
cargo package --locked --verbose
cargo publish --dry-run --locked
```

## Parity

Run the repository’s canonical parity command against the recorded eggcalc reference version and validate the accepted-failure fixture.

---

# Release 4 closure criteria

Release 4 is complete only when:

- Releases 1–3 final correctness closure is complete.
- `rust-version` is declared and tested.
- `Cargo.lock` is tracked and blocking jobs use `--locked`.
- Linux stable, MSRV, Windows, and macOS gates pass.
- cargo-deny policy is blocking and green.
- latest-compatible dependency verification exists and has current evidence.
- scheduled/manual Python parity verification exists and has current evidence.
- package contents are inspected and constrained.
- package provenance identifies commit, version, toolchain, lockfile, and verification results.
- `cargo publish --dry-run --locked` passes in the documented release environment.
- GitHub Actions contains no automatic crates.io publishing path or credentials.
- canonical release docs and readiness evidence are current.

The implementing agent should leave a concise status note containing exact workflow names, current run links or IDs, selected MSRV rationale, dependency-policy exceptions, parity reference version, package-list result, and any deliberately non-blocking scheduled failure.
