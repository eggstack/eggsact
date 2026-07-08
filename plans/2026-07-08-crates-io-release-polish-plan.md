# Crates.io Release Polish Follow-Up Plan

Date: 2026-07-08

Repository: `eggstack/eggsact`

Context:

- GitHub CI is currently reported as 6/7 passing with the remaining job still running.
- Publishing to crates.io should be handled directly by the maintainer, not by GitHub CI.
- CI should remain a verification gate, not a publish mechanism.
- The crate is close to release-ready after the schema-boundary, analysis-tool, diagnostics, release-checklist, and parity-doc corrective passes.

Related plans:

- `plans/2026-07-08-final-ci-parity-release-followup.md`
- `plans/2026-07-08-corrective-verification-schema-release-pass.md`
- `plans/2026-07-07-milestone-9-ci-release-closure.md`

## Objective

Perform the final polish needed for a well-documented crates.io release while explicitly keeping publishing as a direct maintainer action. The outcome should be a repo that clearly separates CI verification from manual crates.io publishing, has one canonical release checklist, has consistent version/tool/parity/package documentation, and contains enough release-readiness evidence for a maintainer to publish confidently from a local authenticated environment.

## Release policy decision

Publishing policy for this repo:

- GitHub CI verifies release readiness.
- GitHub CI must not publish to crates.io.
- The maintainer publishes directly using local `cargo publish` after the documented release gate passes.
- Crates.io credentials must not be placed in GitHub Actions secrets for this release line unless a future explicit decision changes the policy.
- Release tags should be pushed only after the publish dry run succeeds and the final publish decision is made.

This policy should be stated clearly in the release docs so future agents do not add a crates.io publish workflow by default.

## Workstream 1: CI completion and final gate interpretation

### 1.1 Wait for current CI completion

Once the current GitHub Actions run finishes, record the result:

- commit SHA;
- workflow name;
- jobs passed/failed;
- failed job name, if any;
- whether failure is repo-code, environment, cache, or external-service related.

Current expected state is 6/7 passing with one job still running. Do not mark CI green until all required jobs complete.

### 1.2 If CI fails

If the remaining job fails, fix the failure rather than documenting it away. Likely failure classes:

- generated docs drift;
- package exclude/include issue;
- doctest mismatch;
- integration test filter issue;
- clippy warning;
- tool count/profile count documentation drift.

After fixing, push the patch and verify CI again.

### 1.3 If CI passes

If all jobs pass, update release-readiness documentation with:

- latest green commit SHA;
- CI result: 7/7 pass;
- note that CI validates but does not publish.

Do not add a publishing job.

Acceptance:

- Latest release candidate has a completed CI result.
- Release docs clearly describe CI as a verification gate only.

## Workstream 2: Make release docs canonical and non-duplicative

The repo now has multiple release-related docs or references. Clean this up so there is one canonical release checklist and all other locations point to it.

### 2.1 Choose canonical path

Preferred canonical path:

- `docs/release.md`

Reasoning:

- It is user-facing and easier to discover than architecture-specific paths.
- crates.io/public-release instructions are operational docs, not core architecture.

### 2.2 Convert duplicate release docs to pointers

If `docs/architecture/release.md` exists and duplicates `docs/release.md`, either:

1. replace it with a short pointer to `../release.md`, or
2. keep only architecture-specific release-gate rationale and link to `../release.md` for commands.

Avoid two independent checklists.

### 2.3 Update references

Update references in:

- `README.md` if release docs are mentioned;
- `AGENTS.md`;
- `.skills/release.md`;
- `.skills/testing.md`;
- `docs/contributing.md`;
- architecture docs.

Acceptance:

- There is one canonical release checklist.
- All release docs agree on command order and publish policy.
- No doc suggests GitHub Actions publishes to crates.io.

## Workstream 3: Direct crates.io publish procedure

Add or refine the manual crates.io publishing section in `docs/release.md`.

### 3.1 Prerequisites

Document:

- maintainer must be logged in locally with `cargo login` or have a valid local crates.io token;
- do not commit tokens;
- do not store crates.io token in GitHub Actions for this release process;
- release should be run from a clean working tree on `main` at the verified commit;
- ensure local Rust toolchain is stable and current enough for the crate.

### 3.2 Pre-publish commands

Canonical local gate:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --lib
cargo test --all-features --bins
cargo test --all-features --tests -- --skip parity
cargo test --doc
cargo run --bin generate-docs -- --check
cargo package --verbose
cargo publish --dry-run
```

Optional helper commands:

```bash
cargo run --bin verify-eggsact
./release.sh
cargo package --list
```

Parity, if Python reference is available:

```bash
cargo test --test lib parity
```

### 3.3 Publish command

Manual publish command:

```bash
cargo publish
```

State explicitly:

- Run only after `cargo publish --dry-run` succeeds.
- Run only from the verified commit.
- Run only with a clean working tree.
- Do not run from CI for this release line.

### 3.4 Tagging order

Recommended order:

1. Ensure version in `Cargo.toml` is final.
2. Run full local release gate.
3. Run `cargo publish --dry-run`.
4. Publish with `cargo publish`.
5. Create tag after publish succeeds:

```bash
git tag vX.Y.Z
git push origin vX.Y.Z
```

Rationale: crates.io releases are immutable; tag after successful publish avoids a tag pointing at a failed release attempt.

Alternative acceptable policy:

- tag before publish only if maintainer explicitly prefers that convention and is prepared to fix failures with a patch version bump.

Document the chosen policy.

Acceptance:

- Manual publish procedure is explicit and safe.
- No tokens/secrets are introduced to repo or CI.
- Tagging policy is unambiguous.

## Workstream 4: Crates.io metadata and package presentation polish

Review `Cargo.toml` and package metadata from the perspective of crates.io users.

### 4.1 Metadata fields

Verify:

- `name = "eggsact"`.
- `version` is correct for release.
- `edition` is correct.
- `description` accurately describes deterministic MCP/in-process utility tools.
- `license` is correct.
- `repository`, `homepage`, and `documentation` fields are correct.
- `readme = "README.md"` is correct.
- `keywords` fit crates.io limits and include useful terms.
- `categories` are accurate and accepted by crates.io.
- `exclude` does not remove required build/runtime files.

### 4.2 README crates.io rendering

Review README as it will appear on crates.io:

- first paragraph should explain the crate without needing project context;
- quick install/use examples should work;
- MCP usage should be concise;
- in-process API example should compile or be marked illustrative;
- feature list should match current 80-tool state;
- release/parity caveats should not dominate the top-level README;
- badges should not imply CI status until current CI is green.

### 4.3 docs.rs behavior

Check whether docs.rs will build with current package metadata. If special rustdoc configuration is needed, add it deliberately. Otherwise avoid custom docs.rs metadata.

Optional local check:

```bash
cargo doc --all-features --no-deps
```

Add this to release docs as optional if it is useful but not part of CI.

Acceptance:

- Crates.io metadata is accurate.
- README is appropriate for public users.
- Package contents support crates.io/docs.rs build.

## Workstream 5: Package contents and excludes

Run:

```bash
cargo package --list
cargo package --verbose
```

Verify included:

- `Cargo.toml`;
- `README.md`;
- `LICENSE`;
- source files under `src/`;
- generated source files required for build/runtime, especially Unicode/confusables generated data;
- generated docs only if intentionally included and useful;
- examples if present and intended.

Verify excluded:

- `plans/`;
- `.github/`;
- internal task skills if excluded by policy;
- development-only scripts/data if excluded;
- temporary release notes;
- local artifacts.

If `.skills/` is included, decide whether that is intended. For a public crate, these may not be necessary. If excluded, ensure no docs link to files absent from the package in a way that confuses crates.io readers.

Acceptance:

- Package contents are intentional.
- No required runtime/build file is excluded.
- No obvious internal planning files ship to crates.io.

## Workstream 6: Version, changelog, and release notes

### 6.1 Version decision

Confirm whether current release should be:

- patch release: bugfix/docs/hardening only;
- minor release: new tools and expanded public API/tool surface;
- major release: breaking behavior/API changes.

Given the addition of multiple public tools and profile/diagnostic surface expansion, a minor version bump is likely more appropriate than patch unless the crate is still pre-1.0 and the existing versioning policy says otherwise.

### 6.2 Changelog

If `CHANGELOG.md` exists, update it. If not, either:

- create `CHANGELOG.md`, or
- document that GitHub Releases are the changelog source.

Recommended changelog sections:

- Added: deterministic coding-agent tools, diagnostics tools, schema-boundary invariants.
- Changed: docs repositioning, CI split, release process.
- Fixed: stale parity/tool counts, command preflight hardening.
- Known differences: accepted Python parity failures.

### 6.3 GitHub release notes draft

Prepare a release note draft, even if not published immediately. Include:

- summary;
- install command;
- major tool additions;
- verification status;
- parity note;
- direct crates.io publish policy if relevant.

Acceptance:

- Versioning rationale is documented.
- Changelog or release notes exist.
- Release notes do not overclaim parity with Python.

## Workstream 7: Final release-readiness note

Create or update `docs/release-readiness.md`.

Include:

- release candidate commit SHA;
- local release-gate result;
- GitHub CI result;
- crates.io publish mode: direct maintainer publish, not GitHub CI;
- package status;
- parity status;
- known deferred items;
- publish checklist status.

Suggested format:

```markdown
# Release Readiness

Date: YYYY-MM-DD
Commit: <sha>
Version: <version>

## Verification

- GitHub CI: 7/7 passing on <sha>
- Local release gate: pass/fail
- cargo package: pass/fail
- cargo publish --dry-run: pass/fail/not run

## Publishing

Publishing is manual from a maintainer machine using `cargo publish`. GitHub CI verifies only and does not publish.

## Known Deferred Items

- Python parity: 33 accepted failures out of 418 tests; Rust ships 80 tools vs Python 67.
```

Acceptance:

- A maintainer can decide whether to publish by reading one note.
- The note accurately distinguishes CI verification from publishing.

## Workstream 8: Final sanity checks

Before publishing, verify:

```bash
git status --short
git branch --show-current
git log -1 --oneline
cargo metadata --no-deps
cargo package --list
cargo publish --dry-run
```

Check:

- on `main`;
- clean working tree;
- latest commit is the intended release candidate;
- crate name/version are correct;
- package list is intentional;
- dry run passes.

Acceptance:

- No accidental local changes.
- No accidental wrong branch publish.
- Dry run passes.

## Suggested commit structure

Use small commits:

1. `docs(release): clarify manual crates.io publishing policy`
2. `docs(release): make release checklist canonical`
3. `docs(package): document package contents and excludes`
4. `docs: add release readiness note`
5. `chore: update changelog for crates.io release`
6. `chore: bump version for release` if needed

Avoid mixing version bump with unrelated docs churn if possible.

## Final acceptance criteria

This release-polish pass is complete when:

- GitHub CI has completed successfully or any failure has been fixed and rerun.
- Release docs state that crates.io publishing is manual/direct, not through GitHub CI.
- There is one canonical release checklist.
- `cargo publish --dry-run` is documented and run before publish.
- Package contents are verified.
- README and Cargo metadata are crates.io-ready.
- Version/changelog/release notes are prepared.
- Release-readiness note exists and reflects the final candidate.
- No crates.io token or publishing workflow is added to GitHub Actions.

## Handoff note

The repo is past core hardening. This pass is about making the public release boring: clear docs, clean package, verified CI, local dry run, manual publish, and no ambiguous release procedure.
