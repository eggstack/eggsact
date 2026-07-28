# Release Readiness

Date: 2026-07-27 UTC  
Final verification baseline: `3e5b41c6ac5a8daaba11d5dfacb822f6da033464`  
Version: `1.2.0`

## Release candidate

- **Branch:** `main`
- **Commit SHA:** `3e5b41c6ac5a8daaba11d5dfacb822f6da033464`
- **Version:** `1.2.0`
- **Working tree:** clean at verification time
- **Status:** verification complete; publication remains a direct maintainer action

## Verification

### GitHub CI

Run [30306974684](https://github.com/eggstack/eggsact/actions/runs/30306974684) passed
with all 12 jobs successful on CODE_SHA `3e5b41c`.

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

Extended fuzz/sanitizer run on CODE_SHA `3e5b41c`:
Run [30287151564](https://github.com/eggstack/eggsact/actions/runs/30287151564) —
all 19 jobs passed (12 fuzz-matrix + 7 fuzz-sanitizers), no new findings.
Local normal and AddressSanitizer fuzz-target builds also passed using
`nightly-2026-05-07` and cargo-fuzz `0.13.2`.

### Latest-compatible dependencies

Run [30306975876](https://github.com/eggstack/eggsact/actions/runs/30306975876) passed
on CODE_SHA `3e5b41c`.

### Python parity

Run [30306976324](https://github.com/eggstack/eggsact/actions/runs/30306976324) passed
on CODE_SHA `3e5b41c`. Parity (latest eggcalc) succeeded. Its report recorded eggsact
`1.2.0`, eggcalc `1.1.6`, and Python `3.12.13`.

## Release verification workflow

Run [30306975072](https://github.com/eggstack/eggsact/actions/runs/30306975072) passed
on CODE_SHA `3e5b41c`. Full Release Gate succeeded.

### Clean worktree verification

A clean worktree was created at CODE_SHA (`git worktree add`), verified with
`git status --porcelain` (no output), and the full local release gate ran
successfully from it. The worktree remained clean after all verification
commands.

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
