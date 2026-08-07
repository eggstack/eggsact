# Phase 4 — Release and CI Simplification

## Status

- **Status:** complete
- **Repository:** `eggstack/eggsact`
- **Target branch:** `main`
- **Roadmap:** `plans/2026-08-04-bounded-correctness-simplification-roadmap.md`
- **Roadmap commit:** `2211ebb3adae4df6551023676047d018e113a4f7`
- **Depends on:** Phases 1 through 3
- **Priority:** medium-high; required before footprint measurements and any later release
- **Scope:** restore one working canonical local release gate, remove duplicated verification entry points, correct feature-gated commands, and reduce routine GitHub CI to work that is proportionate to a local/manual-release utility
- **Expected change size:** small-to-medium, primarily scripts, Cargo target declarations, workflows, diagnostics, and documentation

## Objective

Make verification easy to run and hard to misunderstand.

After this phase:

1. `scripts/release-check.sh` is the one canonical full release-readiness command;
2. every invocation of a feature-gated development binary includes `--features dev-tools`;
3. duplicate `release.sh` and `verify-eggsact` orchestration is removed rather than maintained in parallel;
4. installed-binary diagnostics report runtime/package facts rather than source-tree-relative file existence;
5. ordinary push/PR CI runs only merge-relevant correctness checks;
6. packaging, publish dry-run, dependency-policy, compatibility drift, fuzzing, and publication remain appropriately local, scheduled, or manual;
7. GitHub Actions still never publishes, tags, or determines release cadence.

---

# Hard constraints

This phase must not:

- publish to crates.io;
- create or push a release tag;
- add crates.io tokens or secrets;
- add a GitHub release workflow;
- add provenance, attestations, SBOMs, artifact digests, or upload steps;
- add another task runner or verification binary;
- add a plan registry, evidence registry, or release-state database;
- add a new workflow family;
- expand ordinary CI to fuzzing, sanitizers, parity, MSRV matrices, latest dependencies, or packaging matrices;
- remove tests needed to prove corrected product contracts;
- silently drop an advertised platform without updating support documentation;
- regenerate Unicode confusables from the network as an implicit ordinary release-check step;
- create exact-SHA evidence loops or repeated closure commits.

Prefer deleting duplicate commands and using existing workflows at lower cadence.

---

# Files to inspect first

At minimum inspect:

```text
Cargo.toml
src/main.rs
src/bin/generate_docs.rs
src/bin/verify_eggsact.rs
scripts/release-check.sh
release.sh
build.sh
.github/workflows/ci.yml
.github/workflows/maintenance.yml
.github/workflows/latest-compatible.yml
.github/workflows/parity.yml
.github/workflows/fuzz-scheduled.yml
docs/verification.md
docs/release.md
docs/contributing.md
architecture/cli-binaries.md
architecture/testing.md
README.md
AGENTS.md
.opencode/skills/release/SKILL.md
.opencode/skills/testing/SKILL.md
CHANGELOG.md
```

Search for:

```text
cargo run --bin generate-docs
cargo run --bin verify-eggsact
generate-docs -- --check
verify-eggsact
release.sh
release-check.sh
cargo package
cargo publish --dry-run
package-list.txt
confusables_generated.rs exists
tool-cards.md exists
../eggcalc parity ref exists
schedule:
workflow_dispatch
upload-artifact
publish
```

Use exact repository search. Do not rely on the current documentation's command list because it is part of the defect.

---

# Workstream 1 — Establish one canonical local release gate

## Canonical command

Retain:

```bash
scripts/release-check.sh
```

It must:

1. require a clean worktree;
2. run from the repository root;
3. use `--locked` consistently;
4. run formatting;
5. run generated-doc freshness with `--features dev-tools`;
6. run Clippy with warnings denied;
7. run the final ordinary non-parity test command established in Phase 3;
8. run doc tests;
9. run `cargo-deny` when installed, with a clear prerequisite error when absent;
10. run `cargo package --locked`;
11. run `cargo publish --locked --dry-run`;
12. never publish or tag;
13. leave the worktree clean.

Recommended command skeleton:

```bash
cargo fmt --all -- --check
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity
cargo test --locked --doc
cargo deny check advisories bans licenses sources
cargo package --locked
cargo publish --locked --dry-run
```

If Phase 3 adopts a bounded test-thread count, use that exact command here.

Do not regenerate docs or Unicode data inside the check. A release check verifies the selected source; it must not silently rewrite it.

## Cleanliness test

Run the script from a clean tree and verify `git status --short` remains empty afterward. Do not write package lists or reports into the repository.

## Acceptance criteria

- the canonical script succeeds from a clean, correctly prepared tree;
- it fails clearly when generated docs are stale;
- it fails clearly when `cargo-deny` is unavailable;
- it does not create `package-list.txt` or other evidence files;
- it never publishes or tags.

---

# Workstream 2 — Remove duplicate verification entry points

## `release.sh`

Preferred outcome: delete `release.sh`.

Reasons:

- it duplicates the canonical script;
- it currently invokes feature-gated binaries incorrectly;
- it regenerates files before checking them;
- it writes package output into the worktree;
- keeping two full gates guarantees future drift.

A trivial delegating wrapper is acceptable only if an established external workflow demonstrably invokes `./release.sh`. The wrapper must contain no independent verification steps:

```bash
#!/usr/bin/env bash
exec "$(dirname "$0")/scripts/release-check.sh" "$@"
```

Do not retain a wrapper merely from caution. Search references first.

## `verify-eggsact` binary

Preferred outcome: delete `src/bin/verify_eggsact.rs` and its `Cargo.toml` target.

Reasons:

- it duplicates shell/Cargo orchestration;
- it contains a second command list that has already drifted;
- it emits a report that is not needed for local release decisions;
- it increases maintenance and documentation surface;
- it is already feature-gated and not part of the user-facing product.

If a real consumer is found, reduce it to invoking the canonical script rather than preserving a second orchestration implementation. Do not add a process-wrapper crate.

## `build.sh`

Evaluate whether the four-line `build.sh` adds value beyond `cargo build`. Preferred outcome is deletion unless referenced by an external packaging/deployment process. This is a low-priority cleanup and must not block the phase.

## Acceptance criteria

- one full verification implementation remains;
- repository search finds no stale independent command sequence;
- removed targets are removed from Cargo/docs/skills;
- no user-facing eggsact functionality is removed.

---

# Workstream 3 — Correct all feature-gated development commands

`generate-docs` remains a useful development target and is gated by:

```toml
required-features = ["dev-tools"]
```

Every invocation must use:

```bash
cargo run --locked --features dev-tools --bin generate-docs -- --check
```

or, for regeneration:

```bash
cargo run --locked --features dev-tools --bin generate-docs
```

Required search-and-fix locations include workflows, scripts, README, architecture, docs, AGENTS, and skills.

Do not remove the feature gate to make stale commands work. The gate correctly prevents development binaries from being installed by default.

## Acceptance criteria

- repository search finds no `cargo run ... --bin generate-docs` invocation lacking `--features dev-tools`;
- no `verify-eggsact` command remains if the target is deleted;
- ordinary `cargo install eggsact` exposes only the intended `eggsact` binary.

---

# Workstream 4 — Simplify installed diagnostics

## Current problem

`--diagnostics` advertises development commands and checks source-relative paths such as:

```text
src/text/confusables_generated.rs
generated/tool-cards.md
../eggcalc
```

These checks are meaningful only in a particular source checkout. After `cargo install`, false values do not indicate a broken installation.

## Required diagnostics boundary

Installed diagnostics should report stable runtime/package facts only, such as:

- crate version;
- tool count;
- available profiles and counts;
- active profile/audience/schema detail;
- effective request/worker/output limits;
- supported MCP protocol versions;
- compatibility mode names.

Remove:

- generated-doc command strings;
- verification command strings;
- source-file existence probes;
- `../eggcalc` existence probes;
- any claim that a source checkout is required for healthy runtime operation.

Development commands belong in repository documentation, not installed runtime diagnostics.

Preserve text and JSON output modes. Update snapshots/tests as needed.

## Acceptance criteria

- diagnostics are truthful from both a source checkout and an installed binary;
- diagnostics do not inspect repository-relative development files;
- no new diagnostics subsystem or filesystem scan is added.

---

# Workstream 5 — Reduce ordinary CI to merge correctness

## Target ordinary CI

For push/PR to `main`, use one Linux correctness job with one cache:

```bash
cargo fmt --all -- --check
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity
cargo test --locked --doc
```

Use the final Phase 3 test-thread setting if one was adopted.

Remove `cargo package` from ordinary CI. Packaging and publish dry-run belong to the local release gate.

Do not split these commands into multiple jobs. One job minimizes duplicated compilation and orchestration.

## Cross-platform compile checks

Eggsact currently advertises Windows x86_64 and macOS ARM64 support. Preserve that claim through lower-cadence checks rather than every push/PR.

Preferred implementation:

- remove the Windows/macOS matrix from ordinary `ci.yml`;
- add the same compile matrix to the existing `maintenance.yml` workflow;
- run it scheduled and manually;
- use `cargo check --locked --all-targets --all-features` only.

Do not add a new workflow file.

A monthly schedule is sufficient for platform/MSRV/dependency policy drift in this project. If the repository convention strongly prefers weekly maintenance, retaining weekly is acceptable; do not make it merge-blocking.

## Maintenance workflow cadence

Consolidate slow-changing policy checks in existing workflows:

- MSRV: monthly/manual or existing low-frequency maintenance;
- cargo-deny: monthly/manual and before release locally;
- Windows/macOS compile: same maintenance workflow;
- latest-compatible dependencies: monthly/manual, not merge-blocking;
- Python parity: monthly/manual and before releases affecting compatibility;
- fuzz/sanitizer: manual, targeted, unchanged.

Do not create path-filter automation to guess whether compatibility changes matter. Manual dispatch is sufficient.

## Action and cache discipline

- retain pinned action SHAs;
- retain least-privilege `contents: read`;
- use at most one cache per job;
- do not upload artifacts on success;
- do not add job matrices beyond the two supported-platform compile entries;
- do not record run IDs in repository files.

## Acceptance criteria

- ordinary CI has one Linux correctness job;
- package/publish dry-run is absent from ordinary CI;
- supported platforms remain checked at maintenance cadence;
- no check publishes, tags, uploads evidence, or blocks ordinary iteration unnecessarily;
- workflow documentation exactly matches effective triggers and commands.

---

# Workstream 6 — Reconcile release and verification documentation

Update:

```text
README.md
docs/verification.md
docs/release.md
docs/contributing.md
architecture/cli-binaries.md
architecture/testing.md
AGENTS.md
.opencode/skills/release/SKILL.md
.opencode/skills/testing/SKILL.md
CHANGELOG.md
```

Required doctrine:

### Ordinary merge CI

- Linux formatting, generated-doc freshness, Clippy, tests, and doctests.

### Maintenance

- supported-platform compile checks;
- MSRV and dependency policy;
- latest-compatible and Python parity at low cadence/manual.

### Targeted hardening

- manual fuzz/sanitizer runs only when affected surfaces change.

### Manual release

- maintainer prepares generated sources/docs deliberately;
- maintainer runs `scripts/release-check.sh` from a clean tree;
- maintainer publishes directly with `cargo publish --locked`;
- maintainer tags only after successful publication.

Do not add release evidence or status documents.

---

# Rejection searches

Before completion, search for and disposition:

```text
cargo run --bin generate-docs
cargo run --locked --bin generate-docs
verify-eggsact
./release.sh
package-list.txt
confusables_generated.rs exists
tool-cards.md exists
parity ref exists
cargo package --locked             # allowed only in release docs/script
cargo publish --locked --dry-run   # allowed only in release docs/script
upload-artifact
release-verification
```

A command containing `--features dev-tools` is valid. Do not mechanically reject it.

---

# Execution order for a smaller implementation agent

1. Sync to latest `origin/main`; confirm Phase 3 tests are stable.
2. Search all references to release scripts and development binaries.
3. Correct `scripts/release-check.sh` first and run it from a clean tree.
4. Delete or trivially delegate `release.sh` based on actual references.
5. Delete `verify-eggsact` and its Cargo target unless a real consumer is found.
6. Simplify `--diagnostics` and update focused tests.
7. Reduce ordinary `ci.yml` to one Linux correctness job.
8. Move platform checks into existing maintenance workflow and reduce cadence as planned.
9. Reconcile all command/document references.
10. Run ordinary verification and the canonical full local release check.
11. Fill this completion record once.

Do not begin binary-size candidates until the canonical release artifact can be produced reliably.

---

# Verification

Focused checks:

```bash
cargo test --locked --all-features parse_args
cargo test --locked --all-features diagnostics
cargo run --locked --features dev-tools --bin generate-docs -- --check
```

Ordinary verification:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity
cargo test --locked --doc
cargo run --locked --features dev-tools --bin generate-docs -- --check
```

Canonical release verification from a clean tree:

```bash
scripts/release-check.sh
git status --short
```

The final `git status --short` must be empty.

Workflow syntax should be reviewed through GitHub's normal Actions parsing after push. Do not add a YAML linter dependency or workflow solely for workflow validation.

---

# Acceptance checklist

- [ ] `scripts/release-check.sh` is the sole full release gate.
- [ ] The canonical script uses `--features dev-tools` correctly.
- [ ] The canonical script leaves a clean worktree.
- [ ] `release.sh` is deleted or is only a trivial delegating wrapper with a proven consumer.
- [ ] `verify-eggsact` is deleted or reduced to delegation only with a proven consumer.
- [ ] `build.sh` is deleted or explicitly retained for a real use.
- [ ] Diagnostics contain no source-tree-relative health checks.
- [ ] All generated-doc commands include the feature gate.
- [ ] Ordinary CI is one Linux correctness job.
- [ ] Packaging and publish dry-run are local release checks, not ordinary CI.
- [ ] Platform, MSRV, dependency, and parity checks use low-cadence/manual existing workflows.
- [ ] Fuzz/sanitizer checks remain manual and targeted.
- [ ] No workflow publishes, tags, uploads evidence, or creates release artifacts.
- [ ] Documentation and skills match effective commands/triggers.
- [ ] Ordinary verification and the canonical release check pass.

---

# Completion record

- **Implementation commit(s):** `63c7a94` (release/CI simplification)
- **Canonical release command:** `scripts/release-check.sh` — sole canonical full local release gate
- **`release.sh` disposition:** deleted
- **`verify-eggsact` disposition:** deleted
- **`build.sh` disposition:** deleted
- **Diagnostics changes:** removed source-tree-relative file existence checks; installed binaries report stable runtime/package facts only
- **Ordinary CI final shape:** single Linux correctness job (fmt, clippy, tests, doc-tests, generate-docs check)
- **Maintenance cadence:** Windows/macOS compile-checks and MSRV/dependency/parity/fuzz checks in scheduled/manual `maintenance.yml`
- **Canonical release-check result:** passes from clean worktree
- **Worktree cleanliness result:** script requires clean worktree, exits non-zero otherwise
- **Documentation updated:** AGENTS.md, CI workflows, scripts/release-check.sh
- **Final phase disposition:** complete
