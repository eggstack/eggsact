# Distribution, Update, and Release Milestone 005a Closure — Deterministic Windows Release Artifacts

Status: closed

Source implementation plan:

- `plans/implementation/distribution-update-release/005a-deterministic-windows-release-artifacts.md`

Source subsystem roadmap:

- `plans/subsystems/distribution-update-release-roadmap.md#m005a--deterministic-windows-release-artifacts`

Source finding: `plans/closure/distribution-update-release/005-status.md` ("Rerun reuse"), live run `36652731202` attempt 2.

Repository baseline reviewed: `75839d2`

Implementation commits:

- `f135210` — target-scoped MSVC link determinism flags, three guards, and the Windows double-build CI proof.

## 1. Executive finding

M005a is closed. The `x86_64-pc-windows-msvc` release candidate is now
byte-reproducible for a fixed source revision and toolchain, proven twice and at
two levels: an independent double build on `windows-latest` produced two
identical SHA-256 digests from two different target directories, and a full
Eggpack pipeline rehearsal of the same tag reused all fifteen staged assets on
rerun with zero digest-mismatch refusals. The published asset contract, the
five-target matrix, the single write-authorized staging job, and Eggpack's
no-clobber refusal are all unchanged; the milestone removed the reason for that
refusal rather than relaxing the check.

Option 1 plus option 2 from the source plan is the landed design, and option 2
turned out to be mandatory rather than cosmetic: `[profile.release] strip =
"symbols"` is silently a no-op for `windows-msvc`, so debug-info removal had to
be expressed as an explicit MSVC linker flag.

The work was entirely product-side. No Eggpack change was required and the
generated release workflow is byte-unchanged (`eggpack ci check` still reports
a match), because the determinism policy lives in a checked-in
`.cargo/config.toml` that Cargo reads without any workflow, environment, or
Eggpack support.

## 2. Requirement-to-evidence matrix

| Requirement | Evidence | Result | Notes |
|---|---|---|---|
| Two consecutive builds of one revision/toolchain produce identical Windows candidate digests, demonstrated in CI | maintenance run `36880110434`, job `Windows reproducibility` (`110429432988`): both candidates `dc1eda1f0f6927806db2d25e06360161776869bcbea6eadc2487ad3d09a52175`, 16,606,720 B, built from `.../eggpack/36880110434-1` and `.../-2` | pass | Mirrors the attempt-1/attempt-2 `CARGO_TARGET_DIR` pattern that broke v1.2.7 |
| Debug-info normalization actually took effect (option 2, not just hoped for) | Same job asserts the candidate contains no CodeView `RSDS` record; the assertion ran and passed | pass | Proves `/DEBUG:NONE` overrode rustc's unconditional `/DEBUG` |
| Option 1 accepted by the toolchain | `link.exe` completed both links; run green | pass | `/BREPRO` is not silently ignored |
| Rerun of the same tag stages/reuses all five assets with zero digest-mismatch refusals | Rehearsal run `36886042696`: attempt 1 → draft `401132612`, `created: true, uploaded: 15, reused: 0`; attempt 2 (`gh run rerun`) → same draft id, `created: false, uploaded: 0, reused: 15`, all 15 digests identical. Both attempts 20/20 jobs success | pass | The v1.2.7 attempt-2 failure shape, now green |
| Cross-check between an independent build and the release pipeline | Rehearsal Windows digest equals the maintenance job's digest from a different run | pass | Same bytes from two unrelated runner invocations |
| No change to asset names, sidecar format, installer behavior, updater mapping, five-target matrix, or the single write-authorized staging job | `git show --stat f135210`: no change under `release/eggpack/`, `.github/workflows/release-binaries.yml`, or `packaging/`; `src/update.rs` gained one test and no runtime change; `eggpack ci check` match (`36880067370`); contract script green | pass | Generated workflow is byte-unchanged |
| Eggpack's no-clobber refusal is not weakened | Generated workflow still contains no `--clobber`; refusal path untouched; the rehearsal demonstrates it was simply not triggered | pass | Guarded by `scripts/check-release-contract.py` |
| Merge gate | fmt, `generate-docs --check`, `clippy -D warnings`, 3,912 tests + 42 binary unit tests + 11 doc tests, `scripts/release-check.sh` (incl. cargo-deny, `cargo package`, publish dry-run) — all green | pass | Parity skipped, as in the standing gate |
| Ordinary CI and drift guard on the implementation commit | CI `36880067269` (Linux correctness) success; Release drift guard `36880067370` success | pass | |

## 3. Production implementation evidence

### Root cause, corrected and made precise

The M005 closure recorded the symptom (24 differing bytes) without the cause.
The cause is that rustc does not implement `-C strip` for MSVC targets:

- `compiler/rustc_codegen_ssa/src/back/linker.rs:1078` — `MsvcLinker::debuginfo`
  takes `_strip: Strip`, discards it, and unconditionally pushes `/DEBUG` and
  `/PDBALTPATH:%_PDB%`.
- `compiler/rustc_codegen_ssa/src/back/link.rs:1250-1310` — the post-link
  `strip_with_external_utility` path has `is_like_darwin`, `is_like_solaris`,
  and `is_like_aix` branches and no `is_like_windows` branch.

Verified against stable `1.98.1`, the toolchain in use for the v1.2.7 build.
Cargo does forward `-C strip=symbols` (proved with a `-v` build of a synthetic
crate); rustc discards it. So the existing `[profile.release] strip = "symbols"`
never applied to the published Windows candidate, and every link kept a
CodeView debug directory whose RSDS GUID `link.exe` randomizes. A comment in
`Cargo.toml` now records this instead of leaving the profile line to imply
coverage it does not have.

Second ordering fact, from the same source: `-C link-arg` lands in
`cg.link_args`, emitted by `add_user_defined_link_args` (`link.rs:2129`) after
`add_order_independent_options` has already pushed `/DEBUG`. That is what makes
a user `/DEBUG:NONE` an override rather than a duplicate.

### Landed change

`.cargo/config.toml` (new, product-owned, target-scoped):

```toml
[target.x86_64-pc-windows-msvc]
rustflags = [
    "-C", "link-arg=/BREPRO",
    "-C", "link-arg=/DEBUG:NONE",
]
```

- `/BREPRO` derives the PE COFF-header, import-descriptor, and bound-import
  time-date-stamps from a content hash instead of the wall clock.
- `/DEBUG:NONE` removes the PDB and the whole PE debug directory, which deletes
  the random RSDS GUID and any PDB path. Because the release pipeline points
  `CARGO_TARGET_DIR` at `${{ runner.temp }}/eggpack/<run_id>-<run_attempt>`, this
  also makes the candidate independent of the build directory; the two
  mechanisms are deliberately redundant so neither has to be trusted alone.
- The candidate is 512 bytes smaller than the published v1.2.7 Windows asset
  (16,606,720 vs 16,607,232 B), consistent with the removed debug directory.
- The flags are scoped to one target, so the four already-reproducible
  candidates keep their link inputs and their bytes.

`Cargo.toml` also adds `.cargo/` to `exclude`, so the crates.io tarball does not
carry a link-flag configuration that a downstream `cargo install --path` from an
extracted tarball would otherwise pick up. Verified: `cargo package --locked
--list` contains no `.cargo/` entry (only cargo's own `.cargo_vcs_info.json`).

### Why no Eggpack change was needed

Eggpack's producer config has no seam for link or environment settings, and the
generated workflow's build step sets exactly one variable (`CARGO_TARGET_DIR`).
`PackConfig`, `TargetPolicy`, and `ToolchainRequirement` are
`deny_unknown_fields` with no `env`/`flags`/`rustflags` field, and
`project_ci_plan` drops `CommandSpec.env` entirely. Cargo reads
`.cargo/config.toml` from the workspace directory hierarchy, which is the
release pipeline's working directory, so product-owned configuration is
sufficient. The generated workflow is therefore untouched and its drift guard
still matches; the fix is not a producer-side feature request.

## 4. Verification executed

### Commands run

```bash
cargo fmt --all -- --check
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
python3 scripts/check-release-contract.py
scripts/release-check.sh
```

Plus dispatched CI: `maintenance.yml` (run `36880110434`) and, for the rerun
evidence, the Eggpack-generated `release-binaries.yml` (run `36886042696`,
attempts 1 and 2).

### Results

- Merge gate: all five steps green. `cargo test --locked --all-features --skip
  parity --test-threads=4` → 3,912 passed, 2 ignored; binary unit tests → 42
  passed (includes `windows_release_link_flags_are_deterministic`); doc tests →
  11 passed.
- `scripts/release-check.sh` → `release-check: passed; no publication was
  performed`, with cargo-deny advisories/bans/licenses/sources, `cargo package`
  (291 files), and `cargo publish --locked --dry-run` all green.
- `scripts/check-release-contract.py` → `release target/asset contract passed`.
  Negative control: injecting `RUSTFLAGS: "-C debuginfo=2"` into
  `maintenance.yml` makes the script fail with `maintenance.yml must not set
  RUSTFLAGS`, and the guard ignores the read-only `${RUSTFLAGS:-}` assertion the
  job itself performs.
- Maintenance run `36880110434` (all five jobs success: MSRV, cargo-deny,
  Windows/macOS platform checks, Windows reproducibility). The reproducibility
  job log: `Finished release profile in 3m 58s` then
  `dc1eda1f0f6927806db2d25e06360161776869bcbea6eadc2487ad3d09a52175` for
  `36880110434-1`, `Finished release profile in 3m 43s` then the same digest for
  `36880110434-2`, both 16,606,720 bytes, and finally
  `x86_64-pc-windows-msvc candidate is byte-identical across two builds`.
- Rehearsal run `36886042696` (temporary tag, see §6): attempt 1 all 20 jobs
  success, draft release `401132612` created with the exact 15-asset inventory;
  attempt 2 all 20 jobs success with `stage-github-draft: release 401132612 draft
  staged` and a receipt recording `created: false, uploaded: 0, reused: 15`.
  Receipt digests compared programmatically: identical for all 15 assets.
- CI `36880067269` and Release drift guard `36880067370` on `f135210`: success.
  The drift guard re-ran `eggpack ci check` against the pinned tool revision
  `e5c81f2` and reported a byte match, proving the generated workflow is
  unchanged.

### Not run

- Parity tests (Python `eggcalc`), excluded from the standing gate.
- Byte-identity proof for the other four targets in this milestone. They were
  already proven byte-identical by the v1.2.7 attempt-1/attempt-2 evidence, and
  this change does not touch their link inputs; re-proving them would quintuple
  the maintenance lane cost for no new information.

## 5. Invariant review

- **Eggpack stays producer authority.** No `release/eggpack/` file and no
  generated-workflow byte changed; the drift guard confirms it.
- **Draft-only staging with exactly one write-authorized job.** Unchanged; the
  rehearsal ended with a draft and no publication, tag creation, or clobber.
- **Eggpack's digest refusal is intact.** Nothing in the guard set or the
  generated workflow was relaxed; the rehearsal shows the refusal is no longer
  reached.
- **Published asset contract.** Asset names, `.exe` convention, sidecar names,
  installer presentation, and updater mapping are untouched; only the Windows
  binary's bytes changed, and only by removing debug information.
- **`strip = "symbols"` intent preserved.** Still the release default for ELF
  and Mach-O; the Cargo.toml comment prevents it being read as Windows
  coverage.
- **Updater self-containment.** `src/update.rs` runtime behavior unchanged; the
  only change in that file is one unit test.
- **No `RUSTFLAGS`.** Now guarded in every workflow, because it would replace
  the target-scoped rustflags and silently restore the old behavior.
- **Deterministic-reproducibility boundary.** Held: the fix is target-scoped, so
  no other candidate's bytes move.

## 6. Failure and recovery review

- **Fail-closed paths are unchanged.** `/BREPRO` or `/DEBUG:NONE` being
  unsupported by a future runner image would fail the link loudly rather than
  silently produce a non-reproducible candidate. Both flags are accepted by the
  toolchain used for every run recorded here.
- **Rerun reuse is idempotent.** Attempt 2 reused the same release object
  (`created: false`) and every asset (`reused: 15`); no second release object
  and no re-upload occurred.
- **Mismatched digests still refuse.** Not exercised by this milestone, but
  unchanged: the rehearsal would have failed exactly as v1.2.7 attempt 2 did if
  any candidate had differed.
- **Rehearsal footprint and cleanup.** Obtaining the rerun receipt required a
  real tag, because `verify_tag_source` resolves `git/ref/tags/*` and a branch
  name 404s (`repro/m005a-rerun-rehearsal` failed `resolve` with `ci projection
  failed`, because contract expansion's `validate_version` also rejects `/`).
  A temporary annotated tag `m005a-rehearsal-1` and a temporary draft release
  were therefore used, and both were deleted afterwards, together with the
  rehearsal branch. Verified after cleanup: five releases, all published, v1.2.7
  still latest with its original 15 assets; nine release tags, no rehearsal
  tag; no rehearsal refs on the remote; local tree clean. Nothing was published,
  no tag remains, and the v1.2.7 release was never targeted.
- **Unrelated v1.2.7 evidence preserved.** The first rehearsal dispatch
  (`36881760510`) failed closed in `resolve` before any build, on a tag shape
  the contract cannot express; no artifact was created.

## 7. Migration and compatibility review

- **No protocol, DTO, or API change.** No MCP/library surface, tool, schema,
  profile, audience, or registry change.
- **Binary compatibility.** The Windows asset still runs the same code; it is
  simply 512 bytes smaller with no PDB reference. `eggsact update` verification
  and sidecar checks are unaffected.
- **Configuration compatibility.** The new `.cargo/config.toml` declares only a
  `windows-msvc` target section, so Linux/macOS builds and `cargo check` on those
  hosts are unaffected. It is excluded from the published crate.
- **Operational cost accepted.** Windows debug-profile builds no longer emit a
  PDB, because `/DEBUG:NONE` is target-scoped rather than profile-scoped. This
  is recorded in `docs/release.md` and `AGENTS.md`. Profile-scoping the flag
  would need a build script (reading `PROFILE`), which this milestone judged not
  worth a new build-time surface for a project whose interactive debugging host
  is not Windows.
- **Rollback.** Revert `f135210` (or drop just the two flags). Nothing
  published depends on them; the next release would simply regain a
  non-reproducible Windows candidate, and the maintenance job would fail loudly.

## 8. Determinism and bounded-execution review

- The exact-input/exact-output check is the milestone's purpose and is proven
  by construction plus CI: two independent link invocations of the same
  revision and toolchain, in different build directories, produce the same
  SHA-256.
- The three inputs that varied are now all content-derived or absent: PE
  time-date-stamps (`/BREPRO`), the RSDS GUID (debug directory removed), and the
  PDB path (debug directory removed). No clock, environment variable, or build
  path reaches the candidate.
- No new bounded-execution surface: this change adds no input parsing, no
  network, no clock, no environment lookup, and no unbounded work.
- The proof job's own scope is bounded: two release builds, one digest
  comparison, one byte-pattern assertion, 60-minute job timeout.

## 9. Documentation and operations

- `docs/release.md` — new "Release candidate determinism" section: what the two
  flags do, why `strip` cannot substitute, the three invariants to preserve
  (no `RUSTFLAGS`, no other target flags, no digest relaxation), how to run the
  static guards, and the CI proof. Package-contents and CI sections updated.
- `docs/verification.md` — Windows release reproducibility added to the
  scheduled-check table with its own subsection.
- `AGENTS.md` — gotcha line naming `.cargo/config.toml` as the only release
  link flags, the `RUSTFLAGS` hazard, and the three guards.
- `CHANGELOG.md` — `Unreleased` entry describing the fix, the 24-byte root
  cause, the `strip` finding, the guards, and the unchanged contract surfaces.
- `Cargo.toml` — comment on the release profile and `.cargo/` in `exclude`.
- Static guards added: `windows_release_link_flags_are_deterministic`
  (`src/update.rs`, runs in the ordinary gate via the binary target),
  `scripts/check-release-contract.py` (flag presence, target scoping, job
  presence, `RUSTFLAGS` ban), and the `windows-reproducibility` CI job.
- Operator action: none. The next release just works; if its Windows asset ever
  needs a manual replacement, the reproducibility job will already have failed
  the weekly lane.

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| Low | Windows debug-profile builds produce no PDB (`/DEBUG:NONE` is target-scoped, not profile-scoped). | Windows developers lose symbol-based debugging in debug builds. | Accepted and documented. If it ever matters, gate the flag on `PROFILE` via a build script. |
| Low | Byte-identity proof runs weekly/manual, not per PR. | A change to the effective link inputs could go up to seven days before CI proves it. | Accepted: the per-PR static guards cover flag presence, target scoping, and the `RUSTFLAGS` hazard, which is the realistic regression path. |
| Low | `/BREPRO` requires Windows SDK ≥ 10.0.10046.1. | A future `windows-latest` image without it would fail the release link loudly. | None. Loud failure is the correct outcome; no pin is available for a hosted runner image. |
| Info | Eggpack's producer config still has no seam for product link/environment settings. | Any future product that needs `RUSTFLAGS` must use `.cargo/config.toml` or an upstream producer change. | No action. Recorded here so the boundary is known; M005a did not require an upstream change. |
| Info | `verify_tag_source` resolves tags only, and contract expansion rejects `/` in a release id. | A rehearsal cannot use a slash-bearing branch name as `release_tag`. | No action. Both are intentional fail-closed contract properties; use a single-component tag. |

M005's previously recorded operational findings (single 60-minute timeout, no
`rust-cache` step, `workflow_dispatch`-only trigger with the exact tag) are
unchanged and remain deferred; M005a did not touch them.

## 11. Roadmap disposition

Milestone closed and next dependency may proceed. M005a's condition is
discharged with real evidence, so Milestone 005's conditional closure becomes an
unconditional one and the distribution, update, and release workstream returns
to maintenance-only.

No future plan is unblocked by this milestone. The only remaining open
milestone in the registry, MCP presentation surface `03c`, stays blocked on
provider credentials and evaluation budget and was independent of M005a; its
closure record is unchanged. M005's other deferred items (deeper
manifest-driven updater mapping, tag-push trigger ergonomics) remain deferred and
are not registered as new milestones.

## 12. Registry updates

- `plans/registry.md`
  - Distribution row in "Active subsystem roadmaps": M005a closed; the
    distribution workstream is guard/maintenance-only.
  - M005a row moves from "Dependency-ready implementation plans" to "Recently
    closed work (control points)" with the run IDs as controlling evidence.
  - M005 row: "closed (conditionally)" → "closed", condition discharged by
    `005a-status.md`.
  - Closure-work summary: M005 fully closed; the `03c` note updated to say
    M005a closed without unblocking it.
- `plans/subsystems/distribution-update-release-roadmap.md`
  - Status line, §4 current state, §5 target architecture, §6 dependency-graph
    tail, §7 milestone table row, §11 completion definition, §12 milestone
    status table.
  - §8 M005's deferred-work line: the M005a deferral is discharged; the other
    two deferrals stay.
- `plans/implementation/distribution-update-release/005a-deterministic-windows-release-artifacts.md`
  - Status line points at this closure record.
- `plans/closure/distribution-update-release/005-status.md` is left immutable;
  its one open condition is discharged by this record and by the registry.
