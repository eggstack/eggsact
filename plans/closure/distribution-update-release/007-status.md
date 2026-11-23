# Distribution, Update, and Release Milestone 007 — Closure Status

Status: closed

Source implementation plan:

- `plans/archive/implementation/distribution-update-release/007-eggup-registry-adoption.md`

Source subsystem roadmap:

- `plans/subsystems/distribution-update-release-roadmap.md#7`

Dependency update commit: `a8416e9868e419f72942bb42795f19d4cbc9c9a2` on `audit/security-followups-006-009`. The follow-up closure and archival changes are recorded in the current PR.

Implementation commit and pull request:

- [`a8416e9`](https://github.com/eggstack/eggsact/commit/a8416e9868e419f72942bb42795f19d4cbc9c9a2); [PR #1](https://github.com/eggstack/eggsact/pull/1). No crate publication or release tag was performed.

## 1. Executive finding

M007's registry adoption is implemented and its local qualification passed. The four coordinated Eggup crates are now sourced from crates.io at 0.2.2, the updater transport resolves Eggfetch 0.4.0, Eggpack's manifest consumer resolves 0.1.2, and the locked graph contains one `zip 9.6.1`. The source exception is removed (`allow-git = []`), the provenance preflight remains active, and the updated API passes the full local merge gate.

The clean-tree `scripts/release-check.sh` passed on commit `a8416e9`. Maintenance run [45089096709](https://github.com/eggstack/eggsact/actions/runs/45089096709) passed all five jobs: macOS and Windows checks, `cargo-deny`, MSRV, and Windows reproducibility. Package and publish dry-run checks passed; the dry run aborted before upload, and no crate was published.

## 2. Requirement-to-evidence matrix

| Requirement | Evidence | Result | Notes |
|---|---|---|---|
| Four coordinated Eggup registry releases exist and are compatible | crates.io exact-version API records for `eggup-acquisition`, `eggup-core`, `eggup-eggfetch`, and `eggup-eggpack` 0.2.2; all unyanked and declare Rust 1.89 | pass | Checksums below; all four latest stable versions as of 2026-11-22 |
| Eggup release graph carries the ZIP security refresh | `cargo tree --locked -i zip` | pass | Exactly one `zip v9.6.1`, via `eggup-archive v0.2.2` → `eggup-eggpack v0.2.2` |
| Current Eggfetch and Eggpack dependencies are current | crates.io latest-version API; `cargo tree --locked -i eggfetch-core` and `-i eggpack-manifest` | pass | Eggfetch 0.4.0, Eggpack manifest 0.1.2; producer CLI is separately pinned to Eggpack CLI 0.1.2 |
| No Git source exception remains; deny gate is green | `deny.toml` has `allow-git = []`; `cargo deny check advisories bans licenses sources` | pass | All four deny categories green after URL advisory fix |
| Dependency provenance guard accepts the production manifest and its regression fixtures | `python3 scripts/check-dependency-provenance.py`; `python3 scripts/check-dependency-provenance.py --self-test` | pass | Manifest accepted; all 10 self-tests passed |
| Ordinary merge gate passes | fmt, generated-docs check, Clippy, full non-parity test suite, doc tests | pass | Full gate rerun after the final lockfile update; parity is excluded per standing CI gate |
| Package and publish dry-run qualify | `cargo package --locked --allow-dirty`; `cargo publish --locked --dry-run --allow-dirty` | pass | Publish command ended with “aborting upload due to dry run”; nothing was published |
| Strict clean-tree release gate | `scripts/release-check.sh` | pass | Passed on clean commit `a8416e9`. |
| Hosted Maintenance | [Run 45089096709](https://github.com/eggstack/eggsact/actions/runs/45089096709) | pass | All five jobs passed: macOS, Windows, cargo-deny, MSRV, and Windows reproducibility. |
| Package file inventory | `cargo package --locked --allow-dirty --list` | pass with observation | 290 files listed; 10 existing `release/eggpack/` configuration files are included because that directory is not excluded. They were not introduced by M007. |

### Published Eggup 0.2.2 checksums

| Crate | SHA-256 checksum | crates.io record |
|---|---|---|
| `eggup-acquisition` | `4000935b1d4d384eaba066c9742e70f91d5cb3dd12618ac3aa79d654e2aab865` | [0.2.2](https://crates.io/api/v1/crates/eggup-acquisition/0.2.2) |
| `eggup-core` | `a0b0dba6035ec7d403ddf0298f1021a5bd544764f9bb4e3011cb31b2dacd614b` | [0.2.2](https://crates.io/api/v1/crates/eggup-core/0.2.2) |
| `eggup-eggfetch` | `f683e0c3b377a1713a5af4a64257f785da24f17771a927c07508b0d73ad35df1` | [0.2.2](https://crates.io/api/v1/crates/eggup-eggfetch/0.2.2) |
| `eggup-eggpack` | `f24df78cf088e665dd69db1b319d7168007175ebc1ec5d2df76a069e31242e49` | [0.2.2](https://crates.io/api/v1/crates/eggup-eggpack/0.2.2) |

## 3. Production implementation evidence

- `Cargo.toml` now declares all four Eggup dependencies as exact registry version `=0.2.2`; `eggfetch-core` is `0.4.0` in dev-dependencies. The lockfile resolves Eggpack manifest 0.1.2 and ZIP 9.6.1.
- `deny.toml` has an empty `allow-git` list. The provenance preflight still rejects any future versionless Git dependency rather than masking it.
- `src/update.rs` contains the narrow compatibility adaptation for the published Eggup API. The updater tests pass as part of the full test suite; no transport, retry, feature, or product-policy change was made.
- The workflow's Eggpack CLI is a separate producer tool, pinned to version 0.1.2 at revision `35b3fc26de63437947aa4f36764c17bd0ebb9f65`; its generated workflow and release-contract checks passed earlier in this task.

## 4. Verification executed

### Commands run

```bash
cargo fmt --all -- --check
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
cargo deny check advisories bans licenses sources
python3 scripts/check-dependency-provenance.py --self-test
python3 scripts/check-dependency-provenance.py
cargo package --locked --allow-dirty
cargo publish --locked --dry-run --allow-dirty
cargo package --locked --allow-dirty --list
cargo tree --locked -i zip
cargo tree --locked -i eggfetch-core
cargo tree --locked -i eggpack-manifest
scripts/release-check.sh
```

### Results and failure correction

All commands listed above passed. A first deny run found `RUSTSEC-2026-0592` in `url 2.5.8` (stack overflow in nested `blob:` URL origin parsing). `cargo update -p url --precise 2.5.9` updated `url` to 2.5.9 and `form_urlencoded` to 1.2.3; the full deny check then passed. The advisory fix is included in `Cargo.lock`.

The merge gate was run after that final lockfile update. Formatting, generated docs, Clippy, the complete non-parity test suite, and doc tests all exited successfully. The updater tests are included in that suite. The package dry-run did not publish or upload anything.

`cargo package --list` listed 290 files, including 10 checked-in `release/eggpack/` policy/configuration files. The package exclusion list was not changed by M007; this inclusion is recorded transparently and is not newly introduced by the dependency refresh.

### Hosted checks and not-run scope

- Maintenance run `45089096709` passed all five jobs, including native Windows/macOS checks and Windows reproducibility.
- PR #1 CI and Release drift checks passed on `a8416e9`.
- Parity tests were excluded from the ordinary merge gate; no behavior change was intended.
- Actual `cargo publish`, tag, or release was not performed.

## 5. Invariant review

- All Eggup dependencies are registry-sourced with versions; `allow-git` is empty and deny sources pass.
- ZIP resolves to 9.6.1; the lockfile contains one ZIP major/version line in the Eggup graph.
- No advisory exception was added. The `url` fix is a lockfile update to the patched release.
- Eggfetch feature selection and HTTP/1/TLS/proxy policy remain unchanged; no retries or new transport surface were introduced.
- Release determinism flags and updater publication/tag policy were not changed by M007.
- Cargo validation commands use `--locked`. The initial package and dry-run used `--allow-dirty`; the strict clean-tree release-check also passed on `a8416e9`.

## 6. Failure, cancellation, truncation, and contention review

No new updater runtime path was introduced beyond the published API compatibility adaptation. Existing checksum, timeout, redirect, streaming, staging, cancellation, and Windows replacement tests passed within the full test suite. No new persistent state or retry behavior was added.

## 7. Migration and compatibility review

The source migration is from exact Git revisions to coordinated crates.io Eggup 0.2.2 versions. Existing eggsact releases remain unchanged; users receive these dependencies only through a later maintainer-authorized eggsact release. Maintenance passed the Windows/macOS checks and Windows reproducibility job; the full merge gate ran locally on macOS. No release was published or tagged.

## 8. Determinism and bounded-execution review

The Cargo.lock is authoritative and checked with `--locked`. The ZIP and Eggfetch graph is deterministic for this lockfile. No clock, timezone, environment, network, or retry behavior was added to eggsact runtime code. crates.io access was used only to verify published version/checksum metadata and Cargo resolution.

## 9. Documentation and operations

Updated release/dependency documentation, `AGENTS.md`, M006/M007 implementation status, the distribution roadmap, and registry status. The Eggpack-generated workflow was regenerated using the pinned CLI 0.1.2 and passed `eggpack ci check` plus `scripts/check-release-contract.py` earlier in this task. The provenance preflight remains in the release path.

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| Informational | Cargo package inventory includes 10 existing `release/eggpack/` config files | No dependency-update behavior impact; files were already included by the package policy | No M007 change; consider only in a separate package-content review if scope warrants |

## 11. Roadmap disposition

**Milestone closed.** The source switch, ZIP floor, deny/source policy, provenance guard, package verification, publish dry-run, ordinary merge gate, clean-tree `scripts/release-check.sh`, and hosted Maintenance evidence are complete. This closeout does not publish or imply a user-facing eggsact release.

## 12. Registry updates

- `plans/registry.md` and `plans/subsystems/distribution-update-release-roadmap.md` record M006 and M007 as closed with their collected evidence.
- `plans/archive/implementation/distribution-update-release/007-eggup-registry-adoption.md`: archived with completed status and evidence; no closure condition remains.
