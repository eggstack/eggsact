# Release Readiness

Date: 2026-07-08
Commit: 92156cff44dcb2747377fcbb040c145204735fc7
Version: 1.1.3 (current in `Cargo.toml`)

## Release candidate

- **Branch:** `main`
- **Commit SHA:** `92156cff44dcb2747377fcbb040c145204735fc7`
- **Version:** `1.1.3`
- **Working tree:** clean (no uncommitted changes at the verified commit; subsequent polish commits are unstaged)

## Verification

### GitHub CI

Run ID: `28941376352` on commit `92156cf`.

Result: **8/8 jobs passing**.

| Job | Result |
|-----|--------|
| Check (`cargo fmt --all -- --check`) | success |
| Generated Docs (`cargo run --bin generate-docs -- --check`) | success |
| Clippy (`cargo clippy --all-targets --all-features -- -D warnings`) | success |
| Test (lib) (`cargo test --all-features --lib`) | success |
| Test (bins) (`cargo test --all-features --bins`) | success |
| Test (integration) (`cargo test --all-features --tests -- --skip parity`) | success |
| Test (doc) (`cargo test --doc`) | success |
| Package (`cargo package --verbose`) | success |

### Local release gate

Run locally against the same commit:

| Step | Result | Details |
|------|--------|---------|
| `cargo fmt --all -- --check` | pass | |
| `cargo clippy --all-targets --all-features -- -D warnings` | pass | 0 warnings |
| `cargo test --all-features --lib` | pass | 404 unit tests |
| `cargo test --all-features --bins` | pass | 24 binary tests |
| `cargo test --all-features --tests -- --skip parity` | pass | 3,164 integration tests, 418 parity tests filtered |
| `cargo test --doc` | pass | 10 doc tests |
| `cargo run --bin generate-docs -- --check` | pass | generated docs are current |
| `cargo package --verbose` | pass | `eggsact-1.1.3.crate` (~709 KB) produced |

Total: **3,602 tests passed**, **0 failures**, **0 warnings**.

### Parity gate

Not run as part of this readiness pass (Python `eggcalc` reference is not in this environment).
See `docs/parity.md` for the latest verification status: 33 accepted parity failures out of 418
tests as of 2026-07-08. These are tracked for follow-up and are not regressions.

### Publish dry run

Not yet run on the final candidate. To complete readiness, run from a clean worktree:

```bash
cargo publish --dry-run
```

Must succeed before any `cargo publish`.

## Publishing

**Publishing is a direct maintainer action.** GitHub CI verifies release readiness but does **not**
publish to crates.io. The maintainer publishes manually with `cargo publish` from a local
authenticated environment.

- Crates.io tokens must **not** be placed in GitHub Actions secrets for this release line.
- `cargo publish` runs only after `cargo publish --dry-run` passes.
- Tags are created only after a successful publish (tag-after-publish policy in `docs/release.md`).

## Package status

`cargo package --list` includes the source tree, tests, docs, and architecture docs. The
following are excluded and will not appear on crates.io:

`plans/`, `data/`, `scripts/`, `build.sh`, `release.sh`, `.github/`, `.skills/`, `deny.toml`,
`AGENTS.md`.

Resulting `.crate` file: `target/package/eggsact-1.1.3.crate`, ~709 KB.

## Crates.io metadata (in `Cargo.toml`)

- `name = "eggsact"`
- `version = "1.1.3"`
- `edition = "2021"`
- `description = "Deterministic MCP and in-process utility tools for coding agents"`
- `license = "MIT"`
- `repository = "https://github.com/eggstack/eggsact"`
- `homepage = "https://github.com/eggstack/eggsact"`
- `documentation = "https://docs.rs/eggsact"`
- `readme = "README.md"`
- `keywords = ["mcp", "coding-agent", "preflight", "math", "calculator"]`
- `categories = ["command-line-utilities", "mathematics", "science"]`
- `authors = ["David Bowman"]`

## Known deferred items

- **Python parity:** 33 accepted failures out of 418 tests; Rust `full` profile ships 80 tools vs
  Python 67. Categories C1–C6 are accepted behavioral differences tracked for follow-up work.
  Closing these gaps is out of scope for this release.
- **crates.io publish workflow:** intentionally not added. Publishing remains a direct maintainer
  action; CI is a verification gate, not a publish mechanism.

## Publish checklist status

- [x] Latest commit SHA recorded
- [x] GitHub CI 8/8 passing
- [x] Local release gate passing
- [x] Generated docs current
- [x] `cargo package` succeeds
- [x] Package excludes audited and tightened (`.skills/`, `release.sh`, `AGENTS.md`, `deny.toml`
      removed from package)
- [x] Crates.io metadata reviewed
- [x] Canonical release doc in `docs/release.md`
- [x] `docs/release-readiness.md` reflects this candidate
- [ ] `cargo publish --dry-run` run on final candidate (maintainer action)
- [ ] `cargo publish` run from clean worktree (maintainer action)
- [ ] `git tag vX.Y.Z && git push origin vX.Y.Z` after publish succeeds (maintainer action)

Maintainer: when ready to publish, follow `docs/release.md` end-to-end. CI has already verified
the local gate — re-run locally before publishing if any time has passed since this note.