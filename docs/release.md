# Release Checklist

This is the canonical release document for the eggsact crate and its optional
GitHub binary release. Crates.io publishing is a manual maintainer action --
GitHub CI verifies merge correctness but does not publish crates or create
source tags.

## Release policy

- GitHub CI establishes merge correctness (Tier 1).
- The maintainer runs the local release check against the selected source revision.
- The maintainer publishes directly to crates.io with `cargo publish --locked`.
- The maintainer creates the annotated version tag after successful publication.
- No GitHub Actions workflow publishes to crates.io, creates a release tag, or approves a release candidate.

## Pre-release

1. Working tree clean: `git status` shows no uncommitted changes.
2. On `main` branch.
3. Version in `Cargo.toml` matches intended release.
4. `CHANGELOG.md` entry for the release exists.
5. Confusables data regenerated:
   ```bash
   python3 scripts/generate_confusables.py
   ```
6. Generated docs regenerated:
   ```bash
   cargo run --features dev-tools --bin generate-docs
   ```

7. Release target/asset contract checked:
   ```bash
   python3 scripts/check-release-contract.py
   bash -n packaging/install.sh
   ```

Before the first binary-bearing release, the Cargo install path remains the
only current installation promise. Do not point users at the GitHub
`latest/download` installer URLs until a tagged binary workflow has produced
and the maintainer has published those assets. The existing v1.2.3 release is
source-only and must not be retrofitted.

## Release verification

Run the local release check from a clean worktree:

```bash
scripts/release-check.sh
```

This runs formatting, generated-docs, Clippy, tests, cargo-deny, package, and publish dry-run. It refuses a dirty worktree and never publishes or tags.

Optional parity gate (requires Python `eggcalc` at `../eggcalc`):

```bash
cargo build
cargo test --test lib parity
```

## Manual crates.io publishing

Publishing is a direct maintainer action. Do not run from CI.

### Prerequisites

- Maintainer logged in locally with `cargo login` or has a valid local crates.io token.
- Do not commit tokens.
- Clean working tree on `main` at the verified commit.
- Local Rust toolchain stable and current.

### Publish

```bash
cargo publish --locked
```

### Tagging order

1. Ensure version in `Cargo.toml` is final.
2. Run the local release check: `scripts/release-check.sh`.
3. Publish with `cargo publish --locked`.
4. On success, create and push the annotated tag:
   ```bash
   git tag -a vX.Y.Z -m "eggsact vX.Y.Z"
   git push origin vX.Y.Z
   ```

crates.io releases are immutable. Tagging after publish avoids a tag pointing at a failed attempt.

### Binary release ordering

Binary assembly is separate from ordinary CI and follows the crates-first
authority chain:

1. Run `scripts/release-check.sh` on clean `main`.
2. Publish the exact version with `cargo publish --locked`.
3. Verify that `max_stable_version` on crates.io shows that version.
4. Create and push the annotated `vX.Y.Z` tag.
5. The tag-triggered `release-binaries.yml` workflow builds and verifies the
   five qualified targets, checks the staged MCP handshake, and creates or
   updates a draft GitHub Release.
6. Review and publish the draft manually.

The five-target workflow uses pinned Zig 0.14.1 (SHA-256 checked for the
runner's x86-64 or AArch64 archive) and cargo-zigbuild 0.23.3. Linux x86-64
uses the glibc 2.17 target suffix; Linux AArch64 builds and executes on the
`ubuntu-24.04-arm` runner. macOS and Windows use native runners. ARMv7 is
installer-recognized Cargo fallback only. Each verified Zig archive is
extracted into a fixed temporary directory with the archive's top-level
directory stripped; the workflow uses that same path for `GITHUB_PATH` and
`zig version`, and the release contract checker guards this invariant.

The workflow requires an existing tag and never creates, moves, or publishes a
tag. It also never calls `cargo publish` or publishes the GitHub draft. ARMv7
is recognized by the installers but is omitted until a separate executable or
QEMU qualification gate is added.

### Immutable version guidance

- crates.io does not permit replacing an uploaded version.
- After a successful upload, any correction requires a new version.
- Do not move a published version tag to different source.
- If publication fails before acceptance, correct the cause and rerun only after confirming whether crates.io accepted the version.

## Post-release

1. Verify the crate appears on [crates.io](https://crates.io/crates/eggsact).
2. Bump version to next development version if needed.

## Package contents

`cargo package --locked` excludes: `plans/`, `data/`, `scripts/`, `packaging/`, `.github/`, `.opencode/`, `.agents/`, `deny.toml`, `AGENTS.md`.

Verify with:

```bash
cargo package --locked --list
```

## CI

GitHub Actions CI runs on push/PR to `main` (plus manual `workflow_dispatch`):

**Linux correctness** (single job, one cache):
- `cargo fmt --all -- --check`
- `cargo run --locked --features dev-tools --bin generate-docs -- --check`
- `cargo clippy --locked --all-targets --all-features -- -D warnings`
- `cargo test --locked --all-features -- --skip parity --test-threads=4`
- `cargo test --locked --doc`

**Supported-platform compilation** (matrix, scheduled/manual only):
- Windows: `cargo check --locked --all-targets --all-features`
- macOS: `cargo check --locked --all-targets --all-features`

MSRV, cargo-deny, parity, latest-compatible, and fuzz/sanitizer checks are scheduled/manual (not merge-blocking). See `docs/verification.md`.

Parity tests are excluded from CI because Python `eggcalc` is not available in the CI environment. Run parity locally with `cargo test --test lib parity`.

GitHub CI verifies merge correctness but does **not** publish to crates.io. The maintainer publishes manually per this document.

See `docs/verification.md` for the full verification doctrine and failure ownership.
