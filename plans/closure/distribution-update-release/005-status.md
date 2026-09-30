# Distribution, Update, and Release Milestone 005 Closure — Eggpack Producer Adoption and Live Draft Qualification

Status: closed (conditionally on the rerun-reuse condition in §Unresolved; see "Rerun reuse" below)

Source plan: `plans/implementation/distribution-update-release/005-eggpack-producer-adoption-and-live-draft-qualification.md`

Source roadmap: `plans/subsystems/distribution-update-release-roadmap.md`

Upstream milestone: Eggpack `plans/closure/ecosystem-adoption/001-status.md` (Eggpack Ecosystem M001), which owns the producer-side correction chain CI M003e/M003f/M003g.

Implementation commits: `a264bd4` (Eggpack producer configuration), `417149a` (workflow cutover, contract-script refactor, drift-guard CI, release docs, 1.2.7 release-prep), `a401ae6`, `6af1bc5`, `5b157c7`, `56a1761`, `2478f5f`, `3af6af2`, `41762e8`, `66705f1`.

## Executive finding

M005 is closed. Eggpack is now the producer authority for eggsact's five-target
binary release: the checked-in workflow is generated from
`release/eggpack/` configuration, drift-guarded in CI against the pinned
Eggpack tool revision, and a real authorized release (`v1.2.7`) produced a
complete draft through the full generated pipeline — five cross/native
builds, five qualifications, five consumer MCP validations, gate, aggregate,
and draft staging — with exactly one write-authorized job and no publication,
tag, or clobber authority anywhere in the generated workflow.

The rerun of the same tag reused the draft without clobbering it, exactly as
required, but failed closed on one asset: the Windows binary is not
byte-reproducible because the MSVC linker embeds a build timestamp and a random
PDB GUID. This is a genuine, proven product-side determinism gap, not an
Eggpack defect; Eggpack's refusal to clobber the differing asset is the
correct fail-closed behavior and is recorded as an explicit outstanding
condition (follow-up M005a).

## Eggpack tool pin

Final pin: `e5c81f28bd328d4aea41c3f061a0ed9944306262` (CI M003g F12/F13; hosted run 36652203168 green on all four lanes). Earlier pins in this milestone's history: `398cd43` (M006), `b9062d4` (M003e), `c190e77` (M003f), `5acda73` and `4b28820` (M003g F9/F10), `5ac5b83` (M003g F11). Every live-dispatch failure below was a producer defect found by the real pipeline, corrected upstream, and re-pinned here.

## Predecessor vs generated workflow authority map

| Authority | Before | After |
|---|---|---|
| Target set (5 triples) | `.github/workflows/release-binaries.yml` matrix | `release/eggpack/distribution.toml` + `pack.toml` |
| Asset names / `.exe` convention | workflow + installer scripts | contract `{product}-{target}[.exe]`, checked against installers |
| Checksum sidecars | workflow `shasum`/`sha256sum` steps | `finalize` + `prepare-stage` (5 binaries + 5 sidecars) |
| Cross toolchain pins (Zig 0.14.1, cargo-zigbuild 0.23.3, digests) | workflow shell | `pack.toml` toolchain policy + `github-policy.cross_tools` |
| Qualification (per-target smoke) | workflow smoke step | `qualification-bindings.toml` |
| Consumer validation (MCP handshake) | workflow step | `consumer-validators.json` + `scripts/smoke-mcp-binary.py` |
| Draft staging | workflow `gh` upload | `stage` job (single `contents: write`) |
| crates.io publish | manual | manual (unchanged) |
| Version tag creation | manual, after publish | manual, after publish (unchanged) |
| Installers `install.sh`/`install.ps1` | eggsact-owned | eggsact-owned (unchanged) |
| `eggsact update` target map | eggsact-owned | eggsact-owned, now contract-parity-gated |

## Static configuration inventory (checked in at `release/eggpack/`)

`distribution.toml`, `pack.toml`, `build-bindings.toml`, `qualification-bindings.toml`, `consumer-validators.json`, `installer-presentation.json`, `install-policy.toml`, `github-template.json`, `github-policy.json`, `workflow-shape.json`.

## Five-target parity matrix (live evidence, run 36652731202)

| Target | Strategy | Runner | Job | Qualify | Consumer validation |
|---|---|---|---|---|---|
| `x86_64-unknown-linux-gnu` | CargoZigbuild, glibc 2.17 | ubuntu-24.04 x86-64 | success | Passed | passed |
| `aarch64-unknown-linux-gnu` | CargoZigbuild, glibc 2.17 | ubuntu-24.04-arm | success | Passed | passed |
| `x86_64-apple-darwin` | NativeCargo | macos | success | Passed | passed |
| `aarch64-apple-darwin` | NativeCargo | macos | success | Passed | passed |
| `x86_64-pc-windows-msvc` | NativeCargo | windows | success | Passed | passed |

All five asset names match the predecessor names exactly, including `eggsact-x86_64-pc-windows-msvc.exe`.

## Linux cross-tool provisioning evidence

Both Linux cross builds provisioned Zig 0.14.1 from the official archive with SHA-256 verification and cargo-zigbuild 0.23.3 from the isolated `${{ runner.temp }}` install root, then built with the 2.17 glibc target suffix. One transient `curl` exit-28 (ziglang.org) on a 600s bounded download was resolved by a job retry (`gh run rerun --failed`) with no code change; it is an external-network condition, not a configuration defect.

## Wrapper semantic parity

`packaging/install.sh` and `packaging/install.ps1` are byte-unchanged from the predecessor; the generated workflow ships the same four installers (`install.sh`, `install.ps1`, `install-exact.sh`, `install-exact.ps1`) from `install-policy.toml` + `installer-presentation.json`. The contract script still asserts the Unix/PowerShell mapping fragments, the Bash-guard ordering, the ARMv7 fallback recognition, and the latest-download URLs.

## Updater parity

`src/update.rs` is unchanged in behavior. The new test `published_targets_match_eggpack_contract` (M005 §17) parses `release/eggpack/distribution.toml` (no new dependency) and asserts `RELEASE_TARGETS` equals the contract-expanded asset names and `.exe` convention, with ARMv7 intentionally absent from both tables.

## Local / CI runs

- `scripts/release-check.sh` on the release-prep commit: passed (fmt, docs check, clippy, tests, cargo-deny, package, publish dry-run; refuses dirty tree, publishes nothing).
- Main CI (`ci.yml`) on every cutover/re-pin commit: passed.
- New drift guard (`release-drift.yml`): installs the pinned Eggpack revision, runs `eggpack ci check`, then the contract script; green on every push after the initial `-p` fix (runs 36634393651, 36638688521, 36642619885, 36647154115, 36652508444).
- `python3 scripts/check-release-contract.py`: passed.

## Live draft run, tag, receipts, inventory

- Tag `v1.2.7` (annotated) at `d8014cfe68503fceb2448838fd22a5da5a6cfa27`, pushed after `cargo publish --locked` succeeded and crates.io reported `max_stable_version` 1.2.7.
- Live run **36652731202**, attempt 1: all 20 jobs success including `stage`. Draft `eggsact v1.2.7` created; run artifacts (17) retained.
- Staging receipt (attempt 1): draft id `RE_kwDOTGg0Mc4X0gk6`, created 2026-09-29T21:04:39Z (idempotent reuse on every later attempt; no second release object was ever created).
- Exact staged inventory, 15 assets: 5 binaries, 5 `.sha256` sidecars, `release-manifest.json`, `install.sh`, `install.ps1`, `install-exact.sh`, `install-exact.ps1`.
- Eggpack left the release as a draft; the `stage` job is the only writer and the workflow contains no `--clobber`, no publish command, and no tag mutation. The contract script and drift guard both enforce this.
- Publication was performed by the maintainer via `gh release edit --draft=false` (not by Eggpack), then the §16 checks below.

## Rerun reuse (§15 step 9) — conditional

Attempt 2 of run 36652731202 rebuilt all five targets (19/20 jobs success) and failed only in `stage` with Eggpack's fail-closed message `same-name remote asset digest mismatch`, leaving the 15-asset draft intact.

Proven root cause (byte-level, not inferred): the Windows candidate differs between attempts by exactly 24 bytes, in five regions — PE header timestamp (2 bytes at `0x108`), three import/descriptor time-date-stamps (`0xdcad24/40/5c`), and the CodeView RSDS PDB GUID+age block (16 bytes at `0xdcae74`). The four other targets are byte-identical. MSVC embeds a build timestamp and a fresh PDB GUID on every link, so a Windows candidate can never match a previous attempt's digest. The sidecar diff confirms it (`eggsact-x86_64-pc-windows-msvc.exe` digest changes; the other four do not).

This is the correct fail-closed outcome: Eggpack reused the exact draft and exact assets, and refused to overwrite the one asset whose bytes genuinely differ. It is not an Eggpack defect. It is recorded as follow-up M005a (deterministic Windows artifacts: MSVC `/Brepro`, PDB path/age normalization, and a documented product decision on debug info).

## Post-publication evidence (§16, complete)

- Exact-tag public installer URLs: `releases/download/v1.2.7/install.sh` (200, 4998 B) and `install.ps1` (200, 4438 B).
- `releases/latest/download/install.sh` and `install.ps1` (200, same sizes) — `v1.2.7` is latest.
- Pinned wrapper install smoke on linux-x86_64: `bash install.sh` installed to `~/.local/bin/eggsact`; `eggsact --version` reports `eggsact 1.2.7` (the prior 1.2.4 binary was backed up first).
- Cargo fallback policy unchanged (`packaging/*` untouched).
- `eggsact update` resolves the published version: `eggsact 1.2.7 is already current (latest stable: 1.2.7)`.

## Unresolved findings

| Severity | Finding | Disposition |
|---|---|---|
| Medium | Windows release binary is not byte-reproducible (PE timestamp + random PDB GUID), so a rerun cannot reuse the existing Windows asset. | Product-side determinism gap, not an Eggpack defect; Eggpack's no-clobber refusal is correct. Follow-up M005a registered. Does not affect §15 inventory, first-run correctness, or publication. |
| Operational | The generated workflow pins one GitHub Policy `timeout_minutes` (60) for every job, where the predecessor used 10/45/10/15. | Recorded M003 parity delta; all live jobs completed well inside 60 min. Accepted. |
| Operational | The generated workflow has no `Swatinem/rust-cache` step and no `windows-installer-check` job, because GitHub Policy pins a fixed action set and one timeout. | Recorded M003 parity delta; caching is an optimization, and installer/consumer behavior is covered by the per-target consumer validation. Accepted. |
| Operational | The initial trigger is `workflow_dispatch` with the exact `release_tag` (per M001 §9); the predecessor also fired on tag push. | Intentional for first adoption; a follow-up may move eggsact to RefName/tag-push after this closure. |
| Low | Artifact transfer strips POSIX exec bits, so qualify/consumer steps restore them (Eggpack CI M003g F10a). | Corrected upstream; the qualify job still records the executed digest, so the restore cannot change identity. |
| Info | `actions/upload-artifact` and `download-artifact` emit an "Unexpected input 'if-no-files-found'" annotation. | Cosmetic third-party warning; does not affect the run. |

## Eggpack CI M003b / Phase 8 disposition

The live draft qualification that M003b was waiting on is now recorded. The M003b draft/asset evidence exists (run 36652731202, attempt 1, 15-asset draft, receipt `RE_kwDOTGg0Mc4X0gk6`). The only unmet M003b step is byte-identical rerun reuse, which is blocked by the product-side Windows determinism gap above, not by Eggpack. Ecosystem M001 records the same boundary; both are closed conditionally on M005a.

## Rollback

The predecessor workflow is recoverable from Git history at `d8014cf^:.github/workflows/release-binaries.yml` (commit `85869ac` is the last release-prep on the legacy path). Reverting the cutover restores it; `release/eggpack/` can remain checked in unused. No published artifact depends on the generated workflow's continued presence.
