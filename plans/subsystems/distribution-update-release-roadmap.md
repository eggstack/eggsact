# Distribution, Update, and Release Roadmap

Status: active; M005a closed; distribution is maintenance-only

Long-term references:

- `plans/000-long-term-specification.md#6` (distribution and release)
- `plans/000-long-term-specification.md#3` (self-update exception, non-goals)
- `plans/001-terminology-and-domain-model.md#7` (targets, transport, contract)
- `plans/001-terminology-and-domain-model.md#8` (merge gate, parity, bench)
- `plans/002-long-term-roadmap.md#phase-3`

Related ADRs:

- `plans/adrs/ADR-0001-planning-conventions-adoption.md`

## 1. Purpose and ownership boundary

Own binary distribution, installers, the self-update transport, and
release qualification: five-target matrix, checksum sidecars, exact-tag
installer verification, in-process `eggfetch-core` updater policy, the
release-contract script, cargo-deny/MSRV, and the manual release gate.

Consumes: substrate + MCP presentation stability. Must not own:
capability semantics, harness API selection, or MCP defaults.

## 2. Work classification

### Invariants

- `eggsact update` needs no post-install `curl`; bootstrap
  `packaging/install.*` still does.
- Updater transport confined to `src/update.rs` with
  `http1,tls-rustls,tls-native-roots,proxy`, HTTP/1 only, strict
  HTTPS-downgrade rejection, explicit env proxy (fail-closed), 10s
  connect / 120s total through body EOF, request-local small-body caps,
  streamed binaries, no retries, no external `curl`.
- Release contract guards hold, including the `src/update.rs` no-`curl`
  rule and `eggfetch-core` presence.
- `update`/`integrate` remain verified/read-only; never install a
  daemon or edit client config.

### Capabilities

- Self-update against crates.io authority with exact GitHub asset,
  SHA-256 sidecar, candidate `--version`, 404-to-Cargo fallback, and
  staged Windows replacement.
- Installer flows for five qualified targets.

### Infrastructure

- `eggfetch-core` adoption line (0.1.6 -> 0.1.7 -> 0.2.0) with
  lockfile/binary deltas and updater regressions.
- Zig 0.14.1 + cargo-zigbuild 0.23.3 as release-only tooling.

### Polish

- Footprint documentation (expected growth from in-process
  hyper/rustls/ring/webpki/url/icu stack, not a size reduction).

## 3. Non-goals

- No HTTP in library/MCP API, no retries, no extra eggfetch features
  without measurement.
- No automatic publishing/tagging; no apt/deb/rpm, Homebrew, winget,
  Chocolatey, MSI, container, signing, or Windows ARM64 release without
  separate qualification.
- No `standard-http1` alias switch mixed into correctness bumps.

## 4. Current state

M001-M004 remain closed historical control points. M005 replaced duplicated producer release construction with Eggpack while preserving eggsact-owned installer, updater, crates.io, tag, and publication policy, and qualified a real draft for v1.2.7. M005a then closed the one condition M005 was waiting on: the Windows release candidate is byte-reproducible, proven both by an independent double build and by a full pipeline rerun that reused all fifteen staged assets. No open distribution milestone remains; the workstream is maintenance-only.

Binary distribution qualified (C8 Zig correction `6658702`,
v1.2.4 matrix, workflow `33944943782`, exact-tag installer checks).
Self-update consolidated on `eggfetch-core 0.2.0` with the pinned
feature/trust profile; lockfile 166 packages; +48-byte stripped delta
on adoption; 19 updater tests green; ordinary CI `35734609288` +
maintenance `35740940881` (MSRV, cargo-deny, native Windows, native
macOS) green. No release published by the bump/corrective lines; the
dependency reaches users with the next normal release.

## 5. Target architecture

M005 established Eggpack as producer authority while keeping eggsact as product-policy authority. The public installers and self-update semantics remain local; only build/qualification/artifact/checksum/generated-CI/draft-staging authority migrated. M005a closed the remaining Windows byte-reproducibility condition without changing that ownership boundary, entirely product-side: the determinism policy is a checked-in `.cargo/config.toml`, so the generated workflow and Eggpack configuration are untouched. M001-M004 remain historical closed evidence.

## 6. Dependency graph

```text
Binary distribution (closed)
    |
    +--> Eggfetch self-update 0.1.6 (closed, historical)
              |
              +--> 0.1.7 bump (closed)
                        |
                        +--> 0.2.0 adoption (closed)
                                  |
                                  `--> 0.2.0 closeout corrective (closed)
```

Historical M001-M004 dependencies are closed. Eggpack CI M003d, Build M005, Build M006, and the CI M003e/M003f/M003g corrective chain are also closed. The ADR-0005 and Build M006 hard dependencies that once blocked M005 are resolved; M005 is implemented and its live draft qualification ran on release v1.2.7.

The tail of the dependency graph is closed:

```text
M005 Eggpack producer adoption (closed)
    |
    `--> M005a deterministic Windows artifacts (closed)
```

### M005a — Deterministic Windows release artifacts

Status: closed

Class: corrective

Objective: make the `x86_64-pc-windows-msvc` candidate byte-reproducible for a fixed source revision and toolchain, so a rerun of the same tag reuses all five staged assets instead of failing closed on the one asset whose bytes differ.

Implementation plan: `plans/implementation/distribution-update-release/005a-deterministic-windows-release-artifacts.md`.

Dependencies: none hard; implemented in `f135210`, ahead of the next release.

Closure: `plans/closure/distribution-update-release/005a-status.md`. Landed in `f135210` as target-scoped MSVC linker flags in `.cargo/config.toml` (`/BREPRO` for content-derived time-date-stamps, `/DEBUG:NONE` to drop the debug directory holding the random RSDS GUID), guarded by a binary unit test, the release-contract script, and a `windows-reproducibility` maintenance job. Evidence: maintenance run `36880110434` (two builds from different target directories, identical SHA-256) and pipeline rehearsal run `36886042696` attempts 1-2 (same draft, 15 assets reused, zero refusals). Root-cause refinement: `[profile.release] strip` is a no-op for `windows-msvc`, so debug-info removal had to be an explicit linker flag.

No open dependency. The workstream is maintenance-only.

## 7. Milestones

### M001 — Binary distribution qualification

Class: capability + infrastructure

Objective: five-target stripped binaries with installer verification.

Dependencies: none.

Deliverable boundary: release workflow, checksums, installers, smoke
(77 tools).

User or operator value: installable standalone binaries.

Exit conditions: closed (v1.2.4, `33944943782`).

Deferred work: ARMv7 source-only; Windows deferred self-update as
staged replacement; MCP Bundle/Registry non-blocking future.

### M002 — Eggfetch self-update consolidation

Class: infrastructure + invariant

Objective: in-process updater with no post-install curl.

Dependencies: M001 (soft).

Deliverable boundary: `src/update.rs` only; 16 updater tests;
release-contract guards; footprint review (+38.9% accepted as
consolidation tradeoff).

User or operator value: self-contained `eggsact update`.

Exit conditions: closed (0.1.6 line, historical).

Deferred work: none.

### M003 — Eggfetch 0.1.7 bump

Class: infrastructure (corrective)

Objective: consume authoritative small-body limits; prove post-header
total timeout with regressions.

Dependencies: M002 (hard).

Deliverable boundary: version + targeted lockfile + binary delta
(-224 bytes); no feature/policy change.

User or operator value: correct body-lifecycle deadlines.

Exit conditions: closed (archive record).

Deferred work: none.

### M004 — Eggfetch 0.2.0 adoption + closeout corrective

Class: infrastructure (adoption + paperwork corrective)

Objective: adopt synchronized 0.2.0 with no source change beyond the
version reference; close the Windows qualification paperwork gap.

Dependencies: M003 (hard); maintenance-lane native runners
(operational, now closed).

Deliverable boundary: lockfile limited to eggfetch packages; +48 bytes;
all 19 updater tests; ordinary + maintenance CI green; no behavior,
feature, dependency-policy, or gate change in the corrective.

User or operator value: maintained updater on a supported dependency.

Exit conditions: closed (`bfe12d7`, `35734609288`, `35740940881`).

Deferred work: none.

### M005 — Eggpack producer adoption + live draft qualification

Class: corrective adoption + capability + infrastructure

Objective: replace the hand-maintained five-target producer workflow with Eggpack-generated release construction and draft staging while preserving crates.io-first/tag-after-publish ordering, public installer latest/version/Cargo fallback semantics, self-update policy, and human publication.

Dependencies: historical prerequisites satisfied: Eggpack CI M003d, Build M005, ADR-0005 Option A, Build M006, and the M003e/M003f/M003g corrective chain are closed. The v1.2.7 maintainer-authorized release supplied the required live qualification evidence; no hard dependency remains.

Implementation plan: `plans/implementation/distribution-update-release/005-eggpack-producer-adoption-and-live-draft-qualification.md`.

Exit conditions: generated workflow parity, exact five-target/toolchain coverage, MCP candidate validation, public wrapper/updater parity, real draft + rerun evidence, and no automatic publication. Closure record: `plans/closure/distribution-update-release/005-status.md`.

Met on release `v1.2.7`: cutover at `d8014cf`, Eggpack producer pin M003g `e5c81f2`, live run 36652731202 (all 20 jobs, 15-asset draft, receipt `RE_kwDOTGg0Mc4X0gk6`), rerun reused the draft and every asset except the Windows binary, which Eggpack correctly refused to clobber because MSVC embeds a build timestamp and a random PDB GUID (24 bytes, proven). Release published by the maintainer; public installer, `latest/download`, and `eggsact update` smoke all verified. M005 closed **conditionally** on M005a, and that condition is now discharged: M005a closed with real rerun-reuse evidence (`plans/closure/distribution-update-release/005a-status.md`), so M005 is closed outright.

Deferred work: none outstanding from M005. Still deferred, unchanged: deeper manifest-driven updater mapping and automatic tag-trigger ergonomics (the generated workflow stays `workflow_dispatch` with the exact tag for now).

## 8. Cross-cutting requirements

### Determinism and bounded execution

Small metadata/checksum bodies under authoritative request-local caps;
absent/false `Content-Length` cannot cause unbounded buffering.

### Protocol and compatibility

No MCP/library network API added at any milestone.

### Profile, audience, and surface

Not applicable; updater is binary-only application functionality.

### Documentation and generated assets

README, installation, CLI, architecture, AGENTS, CHANGELOG, and
release-contract script state the self-contained vs bootstrap
distinction.

### Release and qualification

Full local gate via `scripts/release-check.sh` (clean tree +
`cargo-deny`; never publishes/tags); cross-platform compilation needs
native runners; dependency cross-platform CI plus unchanged platform
paths as evidence with `maintenance.yml` as the gate.

## 9. Verification strategy

Updater unit/integration suites, release-contract script, release
build + MCP smoke, MSRV/cargo-deny, native Windows/macOS checks,
targeted lockfile review, controlled live smoke (`already current`
without replacement).

## 10. Risks and decision points

M005 must not generalize eggsact-specific release selection/fallback policy into Eggpack. The principal migration risks are loss of public installer semantics, weaker MCP candidate qualification, mutable cross-tool provisioning, or duplicate active release workflows. HTTP surface expansion, retries, or extra updater eggfetch features remain out of scope without measurement and an ADR.

## 11. Completion definition

M001-M004 stay closed historical milestones. M005 is closed with Eggpack producer authority qualified against a real draft/release, all eggsact-owned installer/updater/release-order semantics preserved, and its one conditional condition discharged by M005a. No open distribution milestone remains; the workstream is maintenance-only (weekly MSRV/cargo-deny/platform checks, the Windows release reproducibility job, the Eggpack drift guard, and the release-contract guard).

## 12. Milestone status

| Milestone | Status | Implementation plan | Closure record | Blockers |
|---|---|---|---|---|
| M001 binary distribution | closed | — (archive `plans/archive/roadmap.md` + commits) | — | — |
| M002 self-update 0.1.6 | closed | — (history in `plans/archive/roadmap.md`) | — | — |
| M003 0.1.7 bump | closed | `plans/archive/eggfetch-0.1.7-updater-dependency-bump.md` | in-file closure | — |
| M004 0.2.0 adoption | closed | `plans/archive/eggfetch-0.2.0-updater-adoption.md` | in-file closure | — |
| M004 corrective closeout | closed | `plans/archive/eggfetch-0.2.0-adoption-closeout-corrective.md` | in-file closure | — |
| M005 Eggpack producer adoption + live draft qualification | closed | `plans/implementation/distribution-update-release/005-eggpack-producer-adoption-and-live-draft-qualification.md` | `plans/closure/distribution-update-release/005-status.md` | Condition discharged by M005a; all former hard dependencies closed |
| M005a Deterministic Windows release artifacts | closed | `plans/implementation/distribution-update-release/005a-deterministic-windows-release-artifacts.md` | `plans/closure/distribution-update-release/005a-status.md` | None; workstream is maintenance-only |
