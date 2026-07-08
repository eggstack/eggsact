# Skill: Releasing eggsact

Use this when preparing or performing a release. The canonical release checklist lives in `docs/release.md` — read it first.

## Release policy

- GitHub CI verifies release readiness. CI does NOT publish to crates.io.
- The maintainer publishes manually with `cargo publish` from a local authenticated environment.
- Crates.io tokens must not be placed in GitHub Actions secrets for this release line.

## Release process

The pipeline is `./release.sh`, which runs the canonical release gate. Steps in order:

1. Regenerate confusables data: `python3 scripts/generate_confusables.py`
2. Regenerate docs: `cargo run --bin generate-docs`
3. Check formatting: `cargo fmt --all -- --check`
4. Run clippy: `cargo clippy --all-targets --all-features -- -D warnings`
5. Run all tests: `cargo test --all-features`
6. Check generated docs freshness: `cargo run --bin generate-docs -- --check`
7. Check crates.io packaging: `cargo package --verbose`

See `docs/release.md` for the canonical command list and full verification order.

## Canonical Release Gate

The canonical release gate (used by CI, release.sh, and this document):

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --lib
cargo test --all-features --bins
cargo test --all-features --tests -- --skip parity
cargo test --doc
cargo run --bin generate-docs -- --check
cargo package --verbose
```

Optional, environment-dependent:

```bash
cargo test --test lib parity    # requires Python eggcalc at ../eggcalc
```

## Pre-Release Checklist

- [ ] All tests pass: `cargo test --all-features --lib` and `cargo test --all-features --bins`
- [ ] Integration tests pass (parity excluded): `cargo test --all-features --tests -- --skip parity`
- [ ] Doc tests pass: `cargo test --doc`
- [ ] No formatting issues: `cargo fmt --all -- --check`
- [ ] No clippy warnings: `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] Generated docs current: `cargo run --bin generate-docs -- --check`
- [ ] Parity tests pass (when Python `eggcalc` is available at `../eggcalc`): `cargo test --test lib parity`
  - Note: As of 2026-07-08, the Rust parity suite has 33 known failures (out of 418 tests) documented in `docs/parity.md` (`Verification status` and `Known parity gaps` sections). The Rust `full` profile ships 80 tools; Python defines 67. Category A (23 failures) was fixed by adding `EGGCALC_MCP_AUDIENCE` env var and updating test helpers. Categories C1–C6 (33 failures) are accepted behavioral differences tracked for follow-up. Closing these gaps is out of scope for release-polish and is tracked for follow-up work.
- [ ] Confusables data regenerated: `python3 scripts/generate_confusables.py`
- [ ] Crate packaging succeeds: `cargo package --verbose`
- [ ] Run `cargo publish --dry-run` before any `cargo publish`. Do not publish from CI.
- [ ] Version bumped in `Cargo.toml`
- [ ] CHANGELOG.md updated

## Release Candidate Procedure

1. Ensure clean worktree on `main` at the verified commit.
2. Run canonical release gate: `./release.sh`
3. Run optional parity gate if Python reference is available: `cargo test --test lib parity`
4. Run package audit: `cargo package --verbose`
5. Run publish dry run: `cargo publish --dry-run`
6. Confirm version and changelog.
7. Publish only after package dry-run passes: `cargo publish`
8. Create and push tag AFTER publish succeeds: `git tag vX.Y.Z && git push origin vX.Y.Z`

Rollback: if publishing fails, do not tag. Fix the issue, re-run the full gate, and only then tag and publish.

## Publishing to crates.io

This is a manual process from the maintainer's local machine. Do not automate via CI.

Pre-requisites:
- `cargo login` (or a local crates.io token). Do not commit tokens.
- Clean working tree on `main` at the verified commit.

```bash
cargo publish --dry-run    # must succeed before any publish
cargo publish              # manual; never from CI for this release line
```

Tag after publish succeeds:

```bash
git tag vX.Y.Z && git push origin vX.Y.Z
```

`cargo package` must be run on a clean worktree. If regenerating confusables changes
tracked files, commit those generated updates before packaging or publishing.

## Version Location

Version is defined in `Cargo.toml` and referenced in:
- `Cargo.toml` (source of truth)
- `docs/mcp-tools.md` (overview table)
- `CHANGELOG.md`

## CI Pipeline

CI runs on GitHub Actions on push/PR to `main` (plus `workflow_dispatch`):
- Check formatting
- Run clippy with warnings denied
- Run unit tests (`--all-features --lib`)
- Run binary tests (`--all-features --bins`)
- Run integration tests (`--all-features --tests -- --skip parity`)
- Run doc tests (`--doc`)
- Verify generated docs are current
- Run `cargo package --verbose`

Parity tests are not run in CI (Python `eggcalc` is not available in the CI environment) and must be run locally. CI verifies only — it does not publish to crates.io.

## Cargo.lock

`Cargo.lock` is gitignored but present. Do not commit it — this is a binary crate convention.

See also: `docs/release.md` for the full canonical release checklist.
