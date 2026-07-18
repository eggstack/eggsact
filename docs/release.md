# Release Checklist

This is the canonical release document for the eggsact crate. Crates.io publishing is a manual maintainer action — GitHub CI verifies release readiness but does not publish.

## Release policy

- GitHub CI verifies release readiness.
- GitHub CI does not publish to crates.io.
- The maintainer publishes directly with `cargo publish` from a local authenticated environment.
- Crates.io tokens must not be placed in GitHub Actions secrets for this release line.
- Tags are created only after `cargo publish --dry-run` succeeds and the publish decision is made.

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
   cargo run --bin generate-docs
   ```

## Canonical release gate

Run the following commands in order. All must pass before proceeding.

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features --lib
cargo test --locked --all-features --bins
cargo test --locked --all-features --tests -- --skip parity
cargo test --locked --doc
cargo run --locked --bin generate-docs -- --check
cargo deny check advisories bans licenses sources
cargo package --locked --verbose
```

`./release.sh` runs the same pipeline (including confusables and docs regeneration) in one step.

## Optional parity gate

Local only. Requires the Python `eggcalc` package at `../eggcalc`.

```bash
cargo build
cargo test --test lib parity
```

As of 2026-07-08, the Rust `full` profile ships 80 tools while Python defines 67. There are 33 accepted parity failures out of 418 tests. These are tracked for follow-up and are not regressions. See `docs/parity.md` for the full breakdown.

## Manual crates.io publishing

Publishing is a direct maintainer action. Do not run from CI for this release line.

### Prerequisites

- Maintainer logged in locally with `cargo login` or has a valid local crates.io token.
- Do not commit tokens.
- Do not store the crates.io token in GitHub Actions for this release.
- Clean working tree on `main` at the verified commit.
- Local Rust toolchain stable and current.

### Pre-publish

```bash
cargo publish --dry-run --locked
```

Must succeed before proceeding.

### Publish

```bash
cargo publish --locked
```

Run from a clean worktree on `main` at the verified commit, after the dry run passes.

### Tagging order

Recommended:

1. Ensure version in `Cargo.toml` is final.
2. Run the full local release gate.
3. Run `cargo publish --dry-run`.
4. Publish with `cargo publish`.
5. On success, create and push the tag:
   ```bash
   git tag vX.Y.Z
   git push origin vX.Y.Z
   ```

crates.io releases are immutable. Tagging after publish avoids a tag pointing at a failed attempt.

Alternative: tag before publish if the maintainer explicitly prefers that convention and is prepared to fix failures with a patch version bump. Document the chosen policy.

## Scheduled workflows

| Workflow | Trigger | Purpose |
|----------|---------|---------|
| Python Parity | Weekly (Monday 06:00 UTC) + manual | Tracks eggcalc drift |
| Release Verification | Manual only | Full release gate without publish credentials |

## Package contents

`cargo package --locked` excludes: `plans/`, `data/`, `scripts/`, `build.sh`, `release.sh`, `.github/`, `.skills/`, `deny.toml`, `AGENTS.md`.

Verify with:

```bash
cargo package --locked --list
```

## Post-release

1. Verify the crate appears on [crates.io](https://crates.io/crates/eggsact).
2. Bump version to next development version if needed.

## CI

GitHub Actions runs 12 jobs on push/PR to `main` (plus manual `workflow_dispatch`):

| Job | Command |
|-----|---------|
| Check | `cargo fmt --all -- --check` |
| Generated Docs | `cargo run --locked --bin generate-docs -- --check` |
| Clippy | `cargo clippy --locked --all-targets --all-features -- -D warnings` |
| Test (lib) | `cargo test --locked --all-features --lib` |
| Test (bins) | `cargo test --locked --all-features --bins` |
| Test (integration) | `cargo test --locked --all-features --tests -- --skip parity` |
| Test (doc) | `cargo test --locked --doc` |
| MSRV | `cargo check --locked --all-targets --all-features` + tests on Rust 1.89.0 |
| Windows | build + full non-parity tests |
| macOS | build + full non-parity tests |
| cargo-deny | `cargo deny check advisories bans licenses sources` |
| Package | `cargo package --locked --verbose` |

CI mirrors the local release gate except parity. CI is verification only — it does not publish to crates.io.
