# Release Checklist

This document defines the release process for eggsact. Use it as a gate before tagging and publishing.

## Pre-Release Validation

- [ ] Version in `Cargo.toml` is correct
- [ ] README generated sections are current (`cargo run --bin generate-docs -- --check`)
- [ ] Generated tool cards are current (`generated/tool-cards.md`)
- [ ] Architecture docs reflect current tool count (80) and profile count (11)
- [ ] Route-critical fixture tests pass
- [ ] Schema-boundary invariant tests pass
- [ ] Package contents are correct (`cargo package --list`)
- [ ] Doc tests pass
- [ ] Parity tests run locally or explicitly skipped with rationale

## Canonical Release Gate

Run these commands in order. All must pass before release.

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

Or use the release script:

```bash
./release.sh
```

## Optional: Parity Gate

Requires Python `eggcalc` at `../eggcalc`. Not required for release; run locally if available.

```bash
cargo test --test lib parity
```

See `docs/parity.md` for known gaps and verification status.

## Optional: Full Verification

```bash
cargo run --bin verify-eggsact
```

Emits a Markdown report with pass/fail per step and timing.

## Publish Dry Run

```bash
cargo publish --dry-run
```

Only add to CI if credentials are not required and runtime is acceptable.

## Tagging and Changelog

1. Confirm `CHANGELOG.md` is updated (or document that GitHub releases serve as the changelog).
2. Tag the release: `git tag vX.Y.Z`
3. Push tag: `git push origin vX.Y.Z`
4. Publish: `cargo publish`

If publishing fails, do not tag. Fix the issue, re-run the full gate, and only then tag and publish.

## Rollback

If publishing fails after tagging, the tag must not be deleted from crates.io (it's permanent). Fix the issue in a new commit, bump the patch version, and publish a new release.

## CI Pipeline

CI runs on GitHub Actions on push/PR to `main` (plus `workflow_dispatch`):

| Job | Command |
|-----|---------|
| Check | `cargo fmt --all -- --check` |
| Clippy | `cargo clippy --all-targets --all-features -- -D warnings` |
| Test (lib) | `cargo test --all-features --lib` |
| Test (bins) | `cargo test --all-features --bins` |
| Test (integration) | `cargo test --all-features --tests -- --skip parity` |
| Test (doc) | `cargo test --doc` |
| Generated Docs | `cargo run --bin generate-docs -- --check` |
| Package | `cargo package --verbose` (gates on all above) |

All jobs run in parallel except `package`, which requires all others to pass first.

Parity tests are excluded from CI (Python `eggcalc` is not available in CI).

## Version Location

Version is defined in `Cargo.toml` and referenced in:
- `Cargo.toml` (source of truth)
- `docs/mcp-tools.md` (overview table)
- `CHANGELOG.md`

## Cargo.lock

`Cargo.lock` is tracked in the repository because eggsact ships binaries and requires reproducible CI and packaging evidence. CI uses `--locked` for reproducible builds.
