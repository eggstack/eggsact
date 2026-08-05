# Release Checklist

This is the canonical release document for the eggsact crate. Crates.io publishing is a manual maintainer action -- GitHub CI verifies merge correctness but does not publish, create release tags, or determine release cadence.

## Release policy

- GitHub CI establishes merge correctness (Tier 1).
- The maintainer runs the local release check against the selected source revision.
- The maintainer publishes directly to crates.io with `cargo publish --locked`.
- The maintainer creates the annotated version tag after successful publication.
- No GitHub Actions workflow publishes to crates.io, creates a release tag, or approves a release candidate.

## Pre-release

1. Working tree clean: `git status` shows no uncommitted changes.
2. On `main` branch.
3. Version in `Cargo.toml` matches intended release.
4. `CHANGELOG.md` entry for the release exists.
5. Confusables data regenerated:
   ```bash
   python3 scripts/generate_confusables.py
   ```
6. Generated docs regenerated:
   ```bash
   cargo run --features dev-tools --bin generate-docs
   ```

## Release verification

Run the local release check from a clean worktree:

```bash
scripts/release-check.sh
```

This runs formatting, generated-docs, Clippy, tests, cargo-deny, package, and publish dry-run. It refuses a dirty worktree and never publishes or tags.

Optional parity gate (requires Python `eggcalc` at `../eggcalc`):

```bash
cargo build
cargo test --test lib parity
```

## Manual crates.io publishing

Publishing is a direct maintainer action. Do not run from CI.

### Prerequisites

- Maintainer logged in locally with `cargo login` or has a valid local crates.io token.
- Do not commit tokens.
- Clean working tree on `main` at the verified commit.
- Local Rust toolchain stable and current.

### Publish

```bash
cargo publish --locked
```

### Tagging order

1. Ensure version in `Cargo.toml` is final.
2. Run the local release check: `scripts/release-check.sh`.
3. Publish with `cargo publish --locked`.
4. On success, create and push the annotated tag:
   ```bash
   git tag -a vX.Y.Z -m "eggsact vX.Y.Z"
   git push origin vX.Y.Z
   ```

crates.io releases are immutable. Tagging after publish avoids a tag pointing at a failed attempt.

### Immutable version guidance

- crates.io does not permit replacing an uploaded version.
- After a successful upload, any correction requires a new version.
- Do not move a published version tag to different source.
- If publication fails before acceptance, correct the cause and rerun only after confirming whether crates.io accepted the version.

## Post-release

1. Verify the crate appears on [crates.io](https://crates.io/crates/eggsact).
2. Bump version to next development version if needed.

## Package contents

`cargo package --locked` excludes: `plans/`, `data/`, `scripts/`, `.github/`, `.opencode/`, `.agents/`, `deny.toml`, `AGENTS.md`.

Verify with:

```bash
cargo package --locked --list
```

## CI

GitHub Actions CI runs on push/PR to `main` (plus manual `workflow_dispatch`):

**Linux correctness** (single job, one cache):
- `cargo fmt --all -- --check`
- `cargo run --locked --features dev-tools --bin generate-docs -- --check`
- `cargo clippy --locked --all-targets --all-features -- -D warnings`
- `cargo test --locked --all-features -- --skip parity --test-threads=4`
- `cargo test --locked --doc`

**Supported-platform compilation** (matrix, scheduled/manual only):
- Windows: `cargo check --locked --all-targets --all-features`
- macOS: `cargo check --locked --all-targets --all-features`

MSRV, cargo-deny, parity, latest-compatible, and fuzz/sanitizer checks are scheduled/manual (not merge-blocking). See `docs/verification.md`.

Parity tests are excluded from CI because Python `eggcalc` is not available in the CI environment. Run parity locally with `cargo test --test lib parity`.

GitHub CI verifies merge correctness but does **not** publish to crates.io. The maintainer publishes manually per this document.

See `docs/verification.md` for the full verification doctrine and failure ownership.
