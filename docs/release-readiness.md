# Release Readiness

Date: TBD
Commit: TBD
Version: TBD

## Release candidate

- **Branch:** `main`
- **Commit SHA:** TBD
- **Version:** TBD
- **Working tree:** clean
- **Status:** TBD

## Verification

### GitHub CI

Run ID: TBD

Result: **12/12 jobs passing** (including MSRV, Windows, macOS, cargo-deny).

| Job | Result |
|-----|--------|
| Check (`cargo fmt --all -- --check`) | success |
| Generated Docs (`cargo run --locked --bin generate-docs -- --check`) | success |
| Clippy (`cargo clippy --locked --all-targets --all-features -- -D warnings`) | success |
| Test (lib) (`cargo test --locked --all-features --lib`) | success |
| Test (bins) (`cargo test --locked --all-features --bins`) | success |
| Test (integration) (`cargo test --locked --all-features --tests -- --skip parity`) | success |
| Test (doc) (`cargo test --locked --doc`) | success |
| MSRV (`cargo check/test --locked` on Rust 1.89.0) | success |
| Windows (`cargo check/test --locked`) | success |
| macOS (`cargo check/test --locked`) | success |
| cargo-deny (`cargo deny check advisories bans licenses sources`) | success |
| Package (`cargo package --locked --verbose`) | success |

### Local release gate

Run locally against the same commit:

| Step | Result | Details |
|------|--------|---------|
| `cargo fmt --all -- --check` | pass | |
| `cargo clippy --locked --all-targets --all-features -- -D warnings` | pass | 0 warnings |
| `cargo test --locked --all-features --lib` | pass | |
| `cargo test --locked --all-features --bins` | pass | |
| `cargo test --locked --all-features --tests -- --skip parity` | pass | |
| `cargo test --locked --doc` | pass | |
| `cargo run --locked --bin generate-docs -- --check` | pass | generated docs are current |
| `cargo deny check advisories bans licenses sources` | pass | no advisories, all licenses allowed |
| `cargo package --locked --verbose` | pass | crate produced |
| `cargo publish --dry-run --locked` | pass | |

### Parity gate

Not run as part of this readiness pass (Python `eggcalc` reference is not in this environment).
See `docs/parity.md` for the latest verification status and scheduled CI runs.

### Publish dry run

`cargo publish --dry-run --locked` ran on the verified commit against a clean worktree:

```
TBD
```

### Actual publish

`cargo publish --locked` was run from the maintainer's local machine:

```
TBD
```

### Tag

Tag `vX.Y.Z` (annotated) created on commit after publish succeeded, per the tag-after-publish policy in `docs/release.md`.

## Publishing

**Publishing is a direct maintainer action.** GitHub CI verifies release readiness but does **not**
publish to crates.io. The maintainer publishes manually with `cargo publish --locked` from a local
authenticated environment.

- Crates.io tokens must **not** be placed in GitHub Actions secrets for this release line.
- `cargo publish` runs only after `cargo publish --dry-run` passes.
- Tags are created only after a successful publish (tag-after-publish policy in `docs/release.md`).

## Package status

`cargo package --locked --list` includes the source tree, tests, docs, and architecture docs. The
following are excluded and will not appear on crates.io:

`plans/`, `data/`, `scripts/`, `build.sh`, `release.sh`, `.github/`, `.opencode/`, `.agents/`, `deny.toml`,
`AGENTS.md`.

## Crates.io metadata (in `Cargo.toml`)

- `name = "eggsact"`
- `version = "TBD"`
- `edition = "2021"`
- `rust-version = "1.89.0"`
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

- **Python parity:** Accepted failures tracked in `docs/parity.md`; scheduled CI runs weekly.
- **crates.io publish workflow:** intentionally not added. Publishing remains a direct maintainer
  action; CI is a verification gate, not a publish mechanism.

## Publish checklist status

- [ ] Latest commit SHA recorded
- [ ] GitHub CI 12/12 passing
- [ ] Local release gate passing (--locked)
- [ ] Generated docs current
- [ ] cargo-deny passing
- [ ] `cargo package --locked` succeeds
- [ ] Package excludes audited and tightened
- [ ] Crates.io metadata reviewed (including rust-version)
- [ ] Canonical release doc in `docs/release.md`
- [ ] `docs/release-readiness.md` reflects this candidate
- [ ] `cargo publish --dry-run --locked` run on final candidate — passed
- [ ] `cargo publish --locked` run from clean worktree — published
- [ ] `git tag vX.Y.Z && git push origin vX.Y.Z` — tag pushed to `origin`
