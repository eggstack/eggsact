# Release Checklist

This document defines the release process for eggsact. Use it as a gate before tagging and publishing.

## Pre-Release Validation

- [ ] Version in `Cargo.toml` is correct
- [ ] README generated sections are current (`cargo run --bin generate-docs -- --check`)
- [ ] Generated tool cards are current (`generated/tool-cards.md`)
- [ ] Architecture docs reflect current tool count (80) and profile count (11)
- [ ] Route-critical fixture tests pass
- [ ] Schema-boundary invariant tests pass
- [ ] Package contents are correct (`cargo package --locked --list`)
- [ ] Doc tests pass
- [ ] Parity tests run locally or explicitly skipped with rationale

## Canonical Release Gate

Run the release check script:

```bash
scripts/release-check.sh
```

Or run commands manually:

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

## Optional: Parity Gate

Requires Python `eggcalc` at `../eggcalc`. Not required for release; run locally if available.

```bash
cargo test --test lib parity
```

See `docs/parity.md` for known gaps and verification status.

## Tagging and Changelog

1. Confirm `CHANGELOG.md` is updated.
2. Publish: `cargo publish --locked`
3. On success, tag the release: `git tag -a vX.Y.Z -m "eggsact vX.Y.Z"`
4. Push tag: `git push origin vX.Y.Z`

If publishing fails, do not tag. Fix the issue, re-run the full gate, and only then tag and publish.

## Rollback

If publishing fails after tagging, the tag must not be deleted from crates.io (it's permanent). Fix the issue in a new commit, bump the patch version, and publish a new release.

## CI Pipeline

CI runs on GitHub Actions on push/PR to `main` (plus `workflow_dispatch`):

| Job | Platform | What It Runs |
|-----|----------|-------------|
| Linux correctness | Linux | fmt, generated-docs, clippy, tests (parity excluded), doc tests, package |
| Check (windows-latest) | Windows | `cargo check --locked --all-targets --all-features` |
| Check (macos-latest) | macOS | `cargo check --locked --all-targets --all-features` |

Scheduled/manual workflows:

| Workflow | Trigger | Purpose |
|----------|---------|---------|
| Maintenance | Weekly + manual | MSRV and cargo-deny checks |
| Latest Compatible | Weekly + manual | Ecosystem drift detection |
| Python Parity | Weekly + manual | Reference implementation drift |
| Fuzz Extended | Manual only | Hardening: fuzz + sanitizer matrices |

Parity tests are excluded from CI (Python `eggcalc` is not available in CI).

## Version Location

Version is defined in `Cargo.toml` and referenced in:
- `Cargo.toml` (source of truth)
- `docs/mcp-tools.md` (overview table)
- `CHANGELOG.md`

## Cargo.lock

`Cargo.lock` is tracked in the repository because eggsact ships binaries and requires reproducible CI and packaging. CI uses `--locked` for reproducible builds.
