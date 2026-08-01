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
2. Regenerate confusables data: `python3 scripts/generate_confusables.py`
3. Regenerate docs: `cargo run --features dev-tools --bin generate-docs`
4. Run the local release check: `scripts/release-check.sh`
5. Optional parity gate: `cargo test --test lib parity`
6. Publish: `cargo publish --locked`
7. Verify crate is live on crates.io
8. Create and push annotated tag: `git tag -a vX.Y.Z -m "eggsact vX.Y.Z" && git push origin vX.Y.Z`

See `docs/release.md` for the canonical release checklist and `docs/verification.md` for the verification doctrine.

## Pre-Release Checklist

- [ ] Version bumped in `Cargo.toml`
- [ ] CHANGELOG.md updated
- [ ] Confusables data regenerated: `python3 scripts/generate_confusables.py`
- [ ] Generated docs current: `cargo run --features dev-tools --bin generate-docs -- --check`
- [ ] `scripts/release-check.sh` passes from clean worktree

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
| Maintenance (MSRV + cargo-deny) | Weekly (Monday 05:00 UTC) + manual | MSRV and dependency policy |
| Latest Compatible Dependencies | Weekly (Monday 04:00 UTC) + manual | Ecosystem drift detection |
| Python Parity | Weekly (Monday 06:00 UTC) + manual | Reference implementation drift |
| Fuzz Extended | Manual only | Hardening: fuzz + sanitizer matrices |

## CI Pipeline

CI runs 3 jobs on push/PR to `main` (plus `workflow_dispatch`):

| Job | Platform | What It Runs |
|-----|----------|-------------|
| Linux correctness | Linux | fmt, generated-docs, clippy, tests (parity excluded), doc tests, package |
| Check (windows-latest) | Windows | `cargo check --locked --all-targets --all-features` |
| Check (macos-latest) | macOS | `cargo check --locked --all-targets --all-features` |

Parity tests are excluded from CI (Python `eggcalc` is not available in CI). CI verifies only — it does not publish to crates.io.

## Cargo.lock

`Cargo.lock` is tracked because eggsact ships binaries. CI uses `--locked` for reproducible builds.

See also: `docs/release.md` for the full canonical release checklist, `docs/verification.md` for the verification doctrine.
