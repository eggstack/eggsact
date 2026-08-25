> **Historical document.** This records the v1.2.0 release verification process.
> Future releases follow `docs/release.md` and do not require equivalent evidence
> ledgers. See `docs/verification.md` for the current verification doctrine.

# Release Readiness

Date: 2026-07-28 UTC  
Final verification baseline: `75ea50369510d98617741d4025fc626a0983b2e0` (corrective pass on `3e5b41c`)  
Version: `1.2.0`

## Release candidate

- **Branch:** `main`
- **Commit SHA:** `75ea50369510d98617741d4025fc626a0983b2e0`
- **Version:** `1.2.0`
- **Working tree:** clean at verification time
- **Status:** verification complete; corrective pass for calculator normalization backtrack limit closed; publication remains a direct maintainer action

## Verification

### GitHub CI

Run [30367423228](https://github.com/eggstack/eggsact/actions/runs/30367423228) passed
with all 12 jobs successful on corrective-pass SHA `75ea503`.

| Job | Result |
|-----|--------|
| Check | success |
| Generated Docs | success |
| Clippy | success |
| Test (lib) | success — 494 passed, 1 ignored |
| Test (bins) | success — 24 passed |
| Test (integration) | success |
| Test (doc) | success |
| MSRV (1.89.0) | success |
| Windows | success |
| macOS | success |
| cargo-deny | success |
| Package | success |

### Local release gate

The same clean-checkout code baseline passed:

| Step | Result | Details |
|------|--------|---------|
| `cargo fmt --all -- --check` | pass | no diffs |
| `cargo clippy --locked --all-targets --all-features -- -D warnings` | pass | no errors |
| `cargo test --locked --all-features --lib` | pass | 494 passed, 1 ignored |
| `cargo test --locked --all-features --bins` | pass | 24 passed |
| `cargo test --locked --all-features --tests -- --skip parity` | pass | 3423 passed, 1 ignored, 418 filtered |
| `cargo test --locked --doc` | pass | 11 passed |
| `cargo run --locked --features dev-tools --bin generate-docs -- --check` | pass | generated docs current |
| `cargo deny check advisories bans licenses sources` | pass | no advisories or policy failures |
| `cargo package --locked --list` | pass | 236 package files |
| `cargo package --locked --verbose` | pass | package build succeeded — 236 files, 4.8 MiB |
| `cargo publish --locked --dry-run` | pass | no upload performed |

Focused proof also passed: lifecycle tests 9 passed (1 ignored), single-threaded
lifecycle tests 9 passed (1 ignored), sync-pool tests 24 passed, lifecycle and
sync-pool repeated loops 100/100, ordinary full-library parallel fallback 25/25,
and the ignored exact-interleaving test 500/500 (250 completion-wins and 250
timeout-wins).

### MSRV

Rust `1.89.0` passed all-target check, library tests (494 passed, 1 ignored),
binary tests (24 passed), and doc tests (11 passed).

### Fuzz and sanitizer verification

Extended fuzz/sanitizer run on corrective SHA `75ea503`:
Run [30373991584](https://github.com/eggstack/eggsact/actions/runs/30373991584) —
all 19 jobs passed (12 fuzz-matrix + 7 fuzz-sanitizers), no crash artifacts.
Local normal and AddressSanitizer fuzz-target builds also passed using
`nightly-2026-05-07` and cargo-fuzz `0.13.2`.

The earlier extended fuzz run on `3e5b41c` (Run `30287151564`) passed all
19/19 jobs. A re-dispatch on `3e5b41c` (Run `30306975485`) found a
`calculator_normalization` backtrack-limit crash (`32E73 33`). This was
fixed in corrective pass `75ea503` (see `release-5-status-v1.2.0.md` and
`plans/2026-07-28-calculator-normalization-backtrack-limit-corrective-pass.md`).

### Latest-compatible dependencies

Run [30373996030](https://github.com/eggstack/eggsact/actions/runs/30373996030) passed
on corrective SHA `75ea503`.

### Python parity

Run [30373998127](https://github.com/eggstack/eggsact/actions/runs/30373998127) passed
on corrective SHA `75ea503`. Parity (latest eggcalc) succeeded. Its report
recorded eggsact `1.2.0`, eggcalc `1.1.6`, and Python `3.12.13`.

## Release verification workflow

Run [30373993751](https://github.com/eggstack/eggsact/actions/runs/30373993751) passed
on corrective SHA `75ea503`. Full Release Gate succeeded. The provenance
artifact records commit `75ea503`, package version `1.2.0`, Rust stable
`1.97.1`, MSRV `1.89.0`, lockfile SHA-256
`5dd9396665d264fb406c4e9295f6caae2696916650db33a25e7dd2c31d04cec7`, and
236 package files.

### Clean worktree verification

A clean worktree was created at CODE_SHA (`git worktree add`), verified with
`git status --porcelain` (no output), and the full local release gate ran
successfully from it. The worktree remained clean after all verification
commands.

## Actual publish

`cargo publish --locked` was executed from a detached worktree at exactly
`75ea50369510d98617741d4025fc626a0983b2e0` on 2026-07-28. crates.io accepted
the upload and `eggsact 1.2.0` is live (non-yanked).

- **Publication timestamp**: 2026-07-28T19:10:10.018107Z
- **crates.io checksum**: `4aaf92c56c3b7d468364cfbb7d88631d9f9c4c06e5cfa6ede447c90d2fd6a83f`
- **Annotated tag**: `v1.2.0` pointing to `75ea50369510d98617741d4025fc626a0983b2e0`

The annotated tag `v1.2.0` was created and pushed after publication verification.
Tag dereferences to the exact publish SHA both locally and on the remote.

## Package metadata

- `name = "eggsact"`
- `version = "1.2.0"`
- `edition = "2021"`
- `rust-version = "1.89.0"`
- `license = "MIT"`
- `repository = "https://github.com/eggstack/eggsact"`

## Publish checklist status

- [x] Final verification SHA recorded
- [x] GitHub CI passes
- [x] Local release gate passes with `--locked`
- [x] Generated docs current
- [x] cargo-deny passes
- [x] Package contents and build pass
- [x] Crates.io metadata reviewed
- [x] `cargo publish --dry-run --locked` passes
- [x] `docs/release.md` remains the canonical release policy
- [x] Calculator normalization backtrack limit corrective pass closed (`75ea503`)
- [x] `cargo publish --locked` — direct maintainer action (2026-07-28T19:10:10Z)
- [x] `git tag v1.2.0 && git push origin v1.2.0` — after successful publication
