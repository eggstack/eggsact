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
2. Regenerate confusables data from the pinned Unicode 18.0.0 source:
   `python3 scripts/generate_confusables.py` (this is the only release-step
   network access; CI and the release check use checked-in generated data)
3. Regenerate docs: `cargo run --locked --features dev-tools --bin generate-docs`
4. Run the local release check: `scripts/release-check.sh`
5. Optional parity gate: `cargo test --locked --test lib parity`
6. Publish: `cargo publish --locked`
7. Verify `max_stable_version` on crates.io shows that version
8. Create and push annotated tag: `git tag -a vX.Y.Z -m "eggsact vX.Y.Z" && git push origin vX.Y.Z`
9. Binary line only: dispatch `release-binaries.yml` with that exact tag (see below)

## Binary release line (Eggpack-generated)

`release-binaries.yml` is **Eggpack-generated and `workflow_dispatch`-only** —
it is *not* tag-triggered. It takes one required input, `release_tag`, naming an
already-pushed tag. Eggpack is producer authority: the target set, artifact
names, checksums, and installer presentation all come from `release/eggpack/`
(`distribution.toml`, `pack.toml`, `build-bindings.toml`,
`qualification-bindings.toml`, `consumer-validators.json`, `github-policy.json`,
`github-template.json`, `installer-presentation.json`, `install-policy.toml`,
`workflow-shape.json`). Never hand-edit the generated workflow; change the
Eggpack configuration and regenerate.

The workflow requires an existing tag and never creates, moves, or publishes a
tag, never calls `cargo publish`, and never publishes the GitHub draft. Top-level
`permissions` is `contents: read`; only the `stage` job holds `contents: write`.
It builds the five qualified targets, runs staged `--version`/`--help` and MCP
smokes, and creates or reuses a draft GitHub Release. The maintainer publishes
the draft manually afterwards.

`release-drift.yml` ("Release drift guard") runs on push/PR to `main` and fails
if the checked-in workflow diverges from the Eggpack configuration. It installs
the Eggpack tool at the revision pinned in `release/eggpack/github-policy.json`
(never floating `main`), then runs `eggpack ci check` and
`scripts/check-release-contract.py`. Treat a drift failure as a real contract
regression, not a workflow formatting nit.

Pinned cross-toolchain: Zig 0.14.1 and cargo-zigbuild 0.23.3, both declared in
`release/eggpack/pack.toml`. Linux AArch64 builds *and* smokes on the native
`ubuntu-24.04-arm` runner; the workflow verifies runner architecture before
executing any staged binary. Linux x86-64 targets the glibc 2.17 floor. ARMv7 is
installer-recognized Cargo fallback only and is not published until it has its
own qualification gate.

`eggsact update` (Eggup/eggfetch) and `packaging/install.*` remain
eggsact-owned. Eggpack never touches self-update policy and never publishes the
draft.

Run these local checks before pushing a release tag:

```bash
python3 scripts/check-release-contract.py
bash -n packaging/install.sh
shellcheck packaging/install.sh  # when available
cargo build --locked --release
python3 scripts/smoke-mcp-binary.py target/release/eggsact
```

The release workflow is the authoritative binary proof — the local checks above
are pre-flight only. The verified Zig archive is extracted into a fixed
temporary directory with its wrapper directory stripped, and
`scripts/check-release-contract.py` guards that the same path is used for
`GITHUB_PATH` and `zig version`.

Release history: v1.2.4 was the first binary-bearing GitHub Release; v1.2.3
remains source-only and was not retrofitted. v1.2.7 was published through the
Eggpack line with the public installer, `releases/latest/download`, and
`eggsact update` smokes all verified. Windows release-candidate bytes are
reproducible: `.cargo/config.toml` carries target-scoped `/BREPRO` and
`/DEBUG:NONE` for `x86_64-pc-windows-msvc`, which is what allows a same-tag
rerun to reuse staged assets instead of failing closed. Never set `RUSTFLAGS` —
it replaces rather than extends those flags. See `docs/release.md` for the
canonical release checklist and `docs/verification.md` for the verification
doctrine.

## Pre-Release Checklist

- [ ] Version bumped in `Cargo.toml`
- [ ] CHANGELOG.md updated
- [ ] Confusables data regenerated: `python3 scripts/generate_confusables.py`
- [ ] Generated docs current: `cargo run --locked --features dev-tools --bin generate-docs -- --check`
- [ ] `scripts/release-check.sh` passes from clean worktree
- [ ] Target/asset contract and Unix installer syntax checks pass
- [ ] Binary line only: `release-drift.yml` green on `main` (generated workflow matches Eggpack config)

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

## Workflow Inventory

| Workflow | Trigger | Purpose |
|----------|---------|---------|
| CI | push/PR to `main` + manual | Linux correctness gate (the only merge gate) |
| Release drift guard | push/PR to `main` + manual | `eggpack ci check` + `check-release-contract.py` |
| Eggpack candidate builds (`release-binaries.yml`) | manual, requires `release_tag` | Builds the five qualified targets, draft release only |
| Maintenance | Weekly (Monday 05:00 UTC) + manual | MSRV, cargo-deny, platform check, `windows-reproducibility` |
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

**Release drift guard** (also on push/PR to `main`):
- installs the Eggpack tool at the revision pinned in `release/eggpack/github-policy.json`
- `eggpack ci check` fails if `.github/workflows/release-binaries.yml` diverges from the Eggpack configuration
- `python3 scripts/check-release-contract.py` checks the Eggpack-sourced target/asset mapping plus eggsact-owned installer/updater invariants

**Maintenance jobs** (scheduled/manual only, not merge-blocking):
- `msrv` (1.89.0), `cargo-deny`, `platform-check` (Windows + macOS `cargo check --locked --all-targets --all-features`)
- `windows-reproducibility` builds the `x86_64-pc-windows-msvc` release candidate twice from different target directories, requires identical SHA-256, asserts no CodeView (RSDS) record, and fails if `RUSTFLAGS` is set in the job environment

Parity tests are excluded from the merge-blocking `ci.yml` job; the scheduled
`parity.yml` workflow still verifies them weekly against `eggcalc` from PyPI,
whereas the local suite expects the `../eggcalc` sibling checkout. CI verifies
only — it does not publish to crates.io.

## Cargo.lock

`Cargo.lock` is tracked because eggsact ships binaries. CI uses `--locked` for reproducible builds.

See also: `docs/release.md` for the full canonical release checklist, `docs/verification.md` for the verification doctrine.
