# Release Readiness

Date: 2026-07-25 UTC  
Final verification baseline: `fa6a6e92ad183061b01ca710d4cbfbf6932a1067`  
Version: `1.2.0`

## Release candidate

- **Branch:** `main`
- **Commit SHA:** `fa6a6e92ad183061b01ca710d4cbfbf6932a1067`
- **Version:** `1.2.0`
- **Working tree:** clean at verification time
- **Status:** verification complete; publication remains a direct maintainer action

## Verification

### GitHub CI

Run [30138542368](https://github.com/eggstack/eggsact/actions/runs/30138542368) passed
with all 12 jobs successful on the final verification baseline.

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
| `cargo run --locked --bin generate-docs -- --check` | pass | generated docs current |
| `cargo deny check advisories bans licenses sources` | pass | no advisories or policy failures |
| `cargo package --locked --list` | pass | 235 package files |
| `cargo package --locked --verbose` | pass | package build succeeded |
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

The [Fuzz Extended run 30138546987](https://github.com/eggstack/eggsact/actions/runs/30138546987)
passed 19/19 jobs on the final SHA: 12 extended fuzz targets and 7 AddressSanitizer
jobs. Local normal and AddressSanitizer fuzz-target builds also passed using
`nightly-2026-05-07` and cargo-fuzz `0.13.2`.

### Latest-compatible dependencies

The [Latest Compatible run 30138547661](https://github.com/eggstack/eggsact/actions/runs/30138547661)
passed on the final SHA.

### Python parity

The [Python Parity run 30138548267](https://github.com/eggstack/eggsact/actions/runs/30138548267)
passed with `381 passed, 0 failed, 37 ignored, 2867 filtered out`. Its report
records eggsact `1.2.0`, eggcalc `1.1.6`, and Python `3.12.13`.

## Release verification workflow

The [Release Verification run 30138546415](https://github.com/eggstack/eggsact/actions/runs/30138546415)
passed all package, publish-dry-run, and provenance steps. Its provenance
artifact is ID `8613958617`, SHA-256
`9df4ee7a493904a3026be94219e33409356dfeaf17fe75c718825c49da6b4337`.
The artifact records 235 package files and lockfile SHA-256
`5dd9396665d264fb406c4e9295f6caae2696916650db33a25e7dd2c31d04cec7`.

## Actual publish

`cargo publish --locked` has not been run by this workflow. Publication and
annotated tag creation remain direct maintainer actions, following the policy
in `docs/release.md`.

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
- [ ] `cargo publish --locked` — direct maintainer action
- [ ] `git tag v1.2.0 && git push origin v1.2.0` — after successful publication
