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

The v1.2.4 GitHub Release is the first published binary-bearing release. Its
five qualified target assets, checksums, and installers are live; v1.2.3
remains the earlier source-only release and was not retrofitted. Keep the
post-release checks below current for future releases.

## Release candidate determinism

The five binary candidates are byte-reproducible from a fixed source revision
and toolchain, so a rerun of a release run reuses every already-staged asset
instead of failing closed on a differing one.

Four targets (both Linux, both macOS) are already byte-identical across builds
and need no special handling. `x86_64-pc-windows-msvc` needs the two linker
flags in `.cargo/config.toml`:

- `/BREPRO` makes the MSVC linker derive the PE COFF-header, import-descriptor,
  and bound-import time-date-stamps from a content hash instead of the wall
  clock.
- `/DEBUG:NONE` suppresses the PDB and the PE debug directory, removing the
  CodeView RSDS GUID that the linker otherwise randomizes on every link. It
  also removes any PDB path, so the candidate does not depend on the build
  directory (the pipeline varies it per run as
  `${{ runner.temp }}/eggpack/<run_id>-<run_attempt>`).

`[profile.release] strip = "symbols"` is not a substitute: rustc passes
`/DEBUG` unconditionally for `windows-msvc` and discards `-C strip` for that
target. The flags are target-scoped, so the other four candidates keep their
current bytes. The cost is that Windows debug-profile builds no longer emit a
PDB.

Invariants to preserve:

- Do not set `RUSTFLAGS` in any workflow, script, or shell. It replaces, rather
  than extends, the target-scoped rustflags in `.cargo/config.toml` and would
  silently restore the non-deterministic link.
- Do not add target-scoped rustflags for the other four targets.
- The digest guarantee is not weakened anywhere: the staging job must keep
  refusing to clobber a same-name asset whose digest differs.

Evidence and proof:

```bash
# static guards (also run in CI)
python3 scripts/check-release-contract.py
cargo test --locked --bin eggsact windows_release_link_flags_are_deterministic

# byte-level proof: the `windows-reproducibility` job of
# .github/workflows/maintenance.yml builds the release candidate twice from
# different target directories on windows-latest, compares SHA-256, and
# asserts the candidate carries no CodeView (RSDS) record. Dispatch
# maintenance.yml or wait for the weekly lane.
```

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
5. Manually dispatch the Eggpack-generated `release-binaries.yml` workflow
   with that exact tag. Eggpack is producer authority for the five qualified
   targets, artifact names, checksums, and installers; the workflow builds and
   verifies the targets, checks the staged MCP handshake, and creates or
   reuses a draft GitHub Release without publishing it.
6. Review and publish the draft manually, then verify both the exact-tag and
   `releases/latest/download` installer URLs.

The five-target workflow uses pinned Zig 0.14.1 and cargo-zigbuild 0.23.3
(pinned in `release/eggpack/pack.toml`). Linux x86-64 uses the glibc 2.17
target suffix; Linux AArch64 builds and executes on the `ubuntu-24.04-arm`
runner. macOS and Windows use native runners. ARMv7 is
installer-recognized Cargo fallback only. Cross-toolchain provisioning and
verification are Eggpack-owned; the release contract checker guards the
target/asset mapping against the Eggpack configuration rather than
hand-written shell fragments.

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

`cargo package --locked` excludes: `plans/`, `data/`, `scripts/`, `packaging/`, `.cargo/`, `.github/`, `.opencode/`, `.agents/`, `deny.toml`, `AGENTS.md`.

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

**Windows release reproducibility** (scheduled/manual only):
- builds the `x86_64-pc-windows-msvc` release candidate twice from different
  target directories and requires identical SHA-256 digests
- requires the candidate to carry no CodeView (RSDS) debug record
- fails if `RUSTFLAGS` is set in the job environment

MSRV, cargo-deny, parity, latest-compatible, and fuzz/sanitizer checks are scheduled/manual (not merge-blocking). See `docs/verification.md`.

**Release drift guard** (push/PR to `main`, plus manual dispatch):
- installs the exact pinned Eggpack tool revision from `release/eggpack/github-policy.json` (never floating `main`)
- `eggpack ci check` fails if the checked-in release workflow differs from the Eggpack configuration
- `python3 scripts/check-release-contract.py` checks the Eggpack-sourced target/asset mapping and eggsact-owned installer/updater invariants

Public install wrappers (`packaging/install.sh`, `packaging/install.ps1`)
remain eggsact-owned, as does `eggsact update` (eggsact/Eggup-owned
self-update); Eggpack never publishes the draft and never touches
self-update policy.

Parity tests are excluded from the merge-blocking `ci.yml` job. They are still
verified weekly by `.github/workflows/parity.yml`, which installs `eggcalc` from
PyPI; the local suite instead expects a sibling `../eggcalc` checkout. Run parity
locally with `cargo test --locked --test lib parity`.

GitHub CI verifies merge correctness but does **not** publish to crates.io. The maintainer publishes manually per this document.

See `docs/verification.md` for the full verification doctrine and failure ownership.
