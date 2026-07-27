# Release 4 Status Note

**Date:** 2026-07-27 UTC
**Final verification baseline:** `3e5b41c6ac5a8daaba11d5dfacb822f6da033464`
**Version:** `1.2.0`

## Closure criteria

| Criterion | Evidence | Status |
|-----------|----------|--------|
| Releases 1–3 final correctness closure | `plans/2026-07-18-releases-1-3-final-correctness-plan.md` | Complete |
| `rust-version` declared and tested | `Cargo.toml`: `rust-version = "1.89.0"`; MSRV CI job passes | Complete |
| `Cargo.lock` tracked, `--locked` used | Lock file in git; all CI jobs and local gates use `--locked` | Complete |
| Linux stable, MSRV, Windows, macOS gates | CI run [30185819114](https://github.com/eggstack/eggsact/actions/runs/30185819114): Check, Clippy, Test (lib/bins/integration/doc), MSRV (1.89.0), Windows, macOS — all success | Complete |
| cargo-deny policy blocking and green | CI run [30185819114](https://github.com/eggstack/eggsact/actions/runs/30185819114): cargo-deny — success | Complete |
| latest-compatible dependency verification | CI run [30285309780](https://github.com/eggstack/eggsact/actions/runs/30285309780) on `50f9132` — success | Complete |
| Python parity verification | CI run [30285310359](https://github.com/eggstack/eggsact/actions/runs/30285310359) on `50f9132` — success | Complete |
| Package contents inspected and constrained | `cargo package --locked --verbose` — 235 files, 4.8 MiB | Complete |
| Package provenance | SHA-256 `23110880...`; records version 1.2.0, commit `50f9132`, MSRV 1.89.0, Rust 1.97.1, lockfile, 236 files | Complete |
| `cargo publish --dry-run --locked` passes | Local gate: pass | Complete |
| No auto-publish path or credentials in CI | `docs/release.md` documents manual maintainer action; no publish step in workflows | Complete |
| Canonical release docs current | `docs/release.md`, `docs/release-readiness.md`, `docs/releases/2026-07-final-closure-evidence.md` | Complete |

## MSRV rationale

Rust 1.89.0 is the minimum version that supports all required features (edition 2021, `tokio` async, `serde` derive, `fancy-regex`). It was selected to maximize compatibility with distribution-packaged Rust toolchains.

## Dependency policy exceptions

None. `cargo deny` passes with no advisory, ban, license, or source violations. The `getrandom` duplicate (0.2 and 0.3) is expected and allowed.

## Verification summary

- **Ordinary CI**: [30185819114](https://github.com/eggstack/eggsact/actions/runs/30185819114) — all 12 jobs success (on CODE_SHA `50f9132`)
- **Release verification**: [30285308354](https://github.com/eggstack/eggsact/actions/runs/30285308354) — Full Release Gate success (on CODE_SHA `50f9132`)
- **Extended fuzz**: [30287151564](https://github.com/eggstack/eggsact/actions/runs/30287151564) — 19/19 success (12 fuzz + 7 sanitizer, on CODE_SHA `3e5b41c`)
- **Latest-compatible**: [30285309780](https://github.com/eggstack/eggsact/actions/runs/30285309780) — success (on CODE_SHA `50f9132`)
- **Python parity**: [30285310359](https://github.com/eggstack/eggsact/actions/runs/30285310359) — success (on CODE_SHA `50f9132`)
- **Local release gate**: fmt, clippy, lib (494), bins (24), integration (3423), doc (11), generate-docs, cargo-deny, package — all pass
- **Clean worktree**: `git worktree add` at CODE_SHA, `git status --porcelain` clean, full gate from worktree passes, worktree remains clean
- **MSRV**: `cargo +1.89.0 check`, `cargo +1.89.0 test` (lib/bins/doc) — all pass
- **Fuzz build**: 12 targets build successfully under `nightly-2026-05-07`

## Publication status

Actual crates.io publication and annotated tag creation remain direct maintainer actions, following the policy in `docs/release.md`.
