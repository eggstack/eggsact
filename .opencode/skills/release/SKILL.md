---
name: release
description: Use when preparing or performing a release of eggsact, running the release gate, publishing to crates.io, or checking release readiness. The canonical release checklist lives in docs/release.md.
---

## Release policy

- GitHub CI verifies merge correctness. CI does NOT publish to crates.io.
- The maintainer runs `scripts/release-check.sh` locally before publishing.
- The maintainer publishes manually with `cargo publish --locked` from a local authenticated environment.
- The maintainer creates the annotated version tag after successful publication.
- No GitHub Actions workflow publishes, creates release tags, or determines release cadence.

## Release process

1. Ensure clean worktree on `main` at the verified commit.
2. Regenerate confusables data from the pinned Unicode 17.0.0 source:
   `python3 scripts/generate_confusables.py` (this is the only release-step
   network access; CI and the release check use checked-in generated data)
3. Regenerate docs: `cargo run --features dev-tools --bin generate-docs`
4. Run the local release check: `scripts/release-check.sh`
5. Optional parity gate: `cargo test --test lib parity`
6. Publish: `cargo publish --locked`
7. Verify crate is live on crates.io
8. Create and push annotated tag: `git tag -a vX.Y.Z -m "eggsact vX.Y.Z" && git push origin vX.Y.Z`

For the binary distribution line, the tag-triggered
`.github/workflows/release-binaries.yml` then validates crates.io visibility,
builds the qualified matrix, runs staged `--version`/`--help` and MCP smokes,
and creates or updates only a draft GitHub Release. It never publishes crates,
creates tags, or publishes the draft. Run these local checks before pushing a
release tag:

```bash
python3 scripts/check-release-contract.py
bash -n packaging/install.sh
shellcheck packaging/install.sh  # when available
cargo build --locked --release
python3 scripts/smoke-mcp-binary.py target/release/eggsact
```

The release workflow is the authoritative binary proof. It builds and smokes
Linux AArch64 on `ubuntu-24.04-arm`, verifies the runner architecture before
executing any staged binary, downloads Zig 0.14.1 using an architecture-
specific pinned SHA-256, and installs cargo-zigbuild 0.23.3 only in release
tooling. Do not advertise `releases/latest/download/install.*` until a
binary-bearing release has actually been published; v1.2.3 is source-only.

See `docs/release.md` for the canonical release checklist and `docs/verification.md` for the verification doctrine.

## Pre-Release Checklist

- [ ] Version bumped in `Cargo.toml`
- [ ] CHANGELOG.md updated
- [ ] Confusables data regenerated: `python3 scripts/generate_confusables.py`
- [ ] Generated docs current: `cargo run --features dev-tools --bin generate-docs -- --check`
- [ ] `scripts/release-check.sh` passes from clean worktree
- [ ] Target/asset contract and Unix installer syntax checks pass

## Publishing to crates.io

This is a manual process from the maintainer's local machine. Do not automate via CI.

Pre-requisites:
- `cargo login` (or a local crates.io token). Do not commit tokens.
- Clean working tree on `main` at the verified commit.

```bash
cargo publish --locked    # manual; never from CI
```

Tag after publish succeeds:

```bash
git tag -a vX.Y.Z -m "eggsact vX.Y.Z" && git push origin vX.Y.Z
```

crates.io versions are immutable. Tagging after publish avoids a tag pointing at a failed attempt.

## Scheduled Workflows

| Workflow | Trigger | Purpose |
|----------|---------|---------|
| Maintenance (MSRV + cargo-deny + platform checks) | Weekly (Monday 05:00 UTC) + manual | MSRV, dependency policy, and cross-platform compilation |
| Latest Compatible Dependencies | Weekly (Monday 04:00 UTC) + manual | Ecosystem drift detection |
| Python Parity | Weekly (Monday 06:00 UTC) + manual | Reference implementation drift |
| Fuzz Extended | Manual only | Hardening: fuzz + sanitizer matrices |

## CI Pipeline

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

Parity tests are excluded from CI (Python `eggcalc` is not available in CI). CI verifies only — it does not publish to crates.io.

## Cargo.lock

`Cargo.lock` is tracked because eggsact ships binaries. CI uses `--locked` for reproducible builds.

See also: `docs/release.md` for the full canonical release checklist, `docs/verification.md` for the verification doctrine.
