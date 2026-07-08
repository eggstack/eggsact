# Release Readiness

Date: 2026-07-08
Commit: 92156cff44dcb2747377fcbb040c145204735fc7
Version: 1.1.3 (current in `Cargo.toml`)

## Release candidate

- **Branch:** `main`
- **Commit SHA:** `a33ab8b2ef4772d8db18a22c37ef329c02f74aea`
- **Version:** `1.1.3`
- **Working tree:** clean
- **Status:** **PUBLISHED**

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

`cargo publish --dry-run` ran on the verified commit (`a33ab8b`) against a clean worktree:

```
Packaged 217 files, 4.1MiB (695.8KiB compressed)
Compiling eggsact v1.1.3
Finished `dev` profile
Uploading eggsact v1.1.3
warning: aborting upload due to dry run
```

Dry run passed.

### Actual publish

`cargo publish` was run from the maintainer's local machine against commit `a33ab8b`:

```
Updating crates.io index
Packaging eggsact v1.1.3
Packaged 217 files, 4.1MiB (695.8KiB compressed)
Verifying eggsact v1.1.3
Compiling eggsact v1.1.3
Finished `dev` profile
Uploading eggsact v1.1.3
Uploaded eggsact v1.1.3 to registry `crates-io`
Published eggsact v1.1.3 at registry `crates-io`
```

Confirmed live on crates.io via API:

```
name: eggsact
max_version: 1.1.3
max_stable_version: 1.1.3
newest_version: 1.1.3
license: MIT
repository: https://github.com/eggstack/eggsact
```

### Tag

Tag `v1.1.3` (annotated) created on commit `a33ab8b` and pushed to `origin` after
publish succeeded, per the tag-after-publish policy in `docs/release.md`.

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
- [x] `cargo publish --dry-run` run on final candidate — passed
- [x] `cargo publish` run from clean worktree — published `eggsact 1.1.3` to crates.io
- [x] `git tag vX.Y.Z && git push origin vX.Y.Z` — tag `v1.1.3` pushed to `origin`

Maintainer: this release is complete. Next steps are post-release hygiene only:
bump `Cargo.toml` to the next development version if desired, and start the
1.1.4 / 1.2.0 milestone cycle.