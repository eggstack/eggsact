# Release 4 Status Note

**Date:** 2026-07-25 UTC
**Final verification baseline:** `06f7a0bd7c1005439e9de229c37cb34d988b42e4`
**Version:** `1.2.0`

## Closure criteria

| Criterion | Evidence | Status |
|-----------|----------|--------|
| Releases 1–3 final correctness closure | `plans/2026-07-18-releases-1-3-final-correctness-plan.md` | Complete |
| `rust-version` declared and tested | `Cargo.toml`: `rust-version = "1.89.0"`; MSRV CI job passes | Complete |
| `Cargo.lock` tracked, `--locked` used | Lock file in git; all CI jobs and local gates use `--locked` | Complete |
| Linux stable, MSRV, Windows, macOS gates | CI run [30162970273](https://github.com/eggstack/eggsact/actions/runs/30162970273): Check, Clippy, Test (lib/bins/integration/doc), MSRV (1.89.0), Windows, macOS — all success | Complete |
| cargo-deny policy blocking and green | CI run [30162970273](https://github.com/eggstack/eggsact/actions/runs/30162970273): cargo-deny — success | Complete |
| latest-compatible dependency verification | CI run [30138547661](https://github.com/eggstack/eggsact/actions/runs/30138547661) on `fa6a6e9` — success | Complete |
| Python parity verification | CI run [30138548267](https://github.com/eggstack/eggsact/actions/runs/30138548267) on `fa6a6e9` — 381 passed, 0 failed, 37 ignored | Complete |
| Package contents inspected and constrained | `cargo package --locked --verbose` — 235 files, 4.8 MiB | Complete |
| Package provenance | Artifact ID `8613958617`, SHA-256 `9df4ee7a...`; records version, commit, MSRV, toolchain, lockfile, 235 files | Complete |
| `cargo publish --dry-run --locked` passes | Local gate: pass | Complete |
| No auto-publish path or credentials in CI | `docs/release.md` documents manual maintainer action; no publish step in workflows | Complete |
| Canonical release docs current | `docs/release.md`, `docs/release-readiness.md`, `docs/releases/2026-07-final-closure-evidence.md` | Complete |

## MSRV rationale

Rust 1.89.0 is the minimum version that supports all required features (edition 2021, `tokio` async, `serde` derive, `fancy-regex`). It was selected to maximize compatibility with distribution-packaged Rust toolchains.

## Dependency policy exceptions

None. `cargo deny` passes with no advisory, ban, license, or source violations. The `getrandom` duplicate (0.2 and 0.3) is expected and allowed.

## Verification summary

- **Ordinary CI**: [30162970273](https://github.com/eggstack/eggsact/actions/runs/30162970273) — all 12 jobs success
- **Release verification**: [30138546415](https://github.com/eggstack/eggsact/actions/runs/30138546415) — success
- **Extended fuzz**: [30138546987](https://github.com/eggstack/eggsact/actions/runs/30138546987) — 19/19 success (12 fuzz + 7 sanitizer)
- **Latest-compatible**: [30138547661](https://github.com/eggstack/eggsact/actions/runs/30138547661) — success
- **Python parity**: [30138548267](https://github.com/eggstack/eggsact/actions/runs/30138548267) — success
- **Local release gate**: fmt, clippy, lib (494), bins (24), integration (3423), doc (11), generate-docs, cargo-deny, package — all pass
- **MSRV**: `cargo +1.89.0 check`, `cargo +1.89.0 test` (lib/bins/doc) — all pass
- **Fuzz build**: 12 targets build successfully under `nightly-2026-05-07`

## Publication status

Actual crates.io publication and annotated tag creation remain direct maintainer actions, following the policy in `docs/release.md`.
