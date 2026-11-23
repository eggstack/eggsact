# Distribution, Update, and Release Roadmap

Status: active; M005a, M006, and M007 closed; M009 implemented pending closure; M008 proposed

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

- Eggfetch updater line: historical 0.1.6 -> 0.1.7 -> 0.2.0 adoptions, now
  registry 0.4.0 through M007, with lockfile deltas and updater regressions.
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

M001-M004 remain closed historical control points. M005 replaced duplicated producer release construction with Eggpack while preserving eggsact-owned installer, updater, crates.io, tag, and publication policy, and qualified a real draft for v1.2.7. M005a then closed the one condition M005 was waiting on: the Windows release candidate is byte-reproducible, proven both by an independent double build and by a full pipeline rerun that reused all fifteen staged assets.

The November 2026 audit opened M006-M009. On 2026-11-22, all four coordinated Eggup crates were published at 0.2.2; the current locked graph resolves `zip 9.6.1`, `eggfetch-core 0.4.0`, and `eggpack-manifest 0.1.2`. `Cargo.toml` now uses registry versions and `deny.toml` keeps `allow-git = []`. M007's full local merge gate, cargo-deny, provenance checks, package, and publish dry-run passed. The clean-tree `scripts/release-check.sh` passed on `a8416e9`; Maintenance run [45089096709](https://github.com/eggstack/eggsact/actions/runs/45089096709) passed all five jobs. M006 and M007 are closed.

M009 closure is pending; M008 remains proposed pending ADR-0002. Update authenticity still rests on TLS plus a same-origin SHA-256 without a signature.

Binary distribution qualified (C8 Zig correction `6658702`, v1.2.4 matrix, workflow `33944943782`, exact-tag installer checks). M004's Eggfetch 0.2.0 adoption remains historical; M007 advances the active updater to Eggfetch 0.4.0 while preserving its feature/trust profile. No release was published by this local dependency update; users receive it only in a future maintainer-authorized eggsact release.

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

Implementation plan: `plans/archive/implementation/distribution-update-release/005a-deterministic-windows-release-artifacts.md`.

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

Implementation plan: `plans/archive/implementation/distribution-update-release/005-eggpack-producer-adoption-and-live-draft-qualification.md`.

Exit conditions: generated workflow parity, exact five-target/toolchain coverage, MCP candidate validation, public wrapper/updater parity, real draft + rerun evidence, and no automatic publication. Closure record: `plans/closure/distribution-update-release/005-status.md`.

Met on release `v1.2.7`: cutover at `d8014cf`, Eggpack producer pin M003g `e5c81f2`, live run 36652731202 (all 20 jobs, 15-asset draft, receipt `RE_kwDOTGg0Mc4X0gk6`), rerun reused the draft and every asset except the Windows binary, which Eggpack correctly refused to clobber because MSVC embeds a build timestamp and a random PDB GUID (24 bytes, proven). Release published by the maintainer; public installer, `latest/download`, and `eggsact update` smoke all verified. M005 closed **conditionally** on M005a, and that condition is now discharged: M005a closed with real rerun-reuse evidence (`plans/closure/distribution-update-release/005a-status.md`), so M005 is closed outright.

Deferred work: none outstanding from M005. Still deferred, unchanged: deeper manifest-driven updater mapping and automatic tag-trigger ergonomics (the generated workflow stays `workflow_dispatch` with the exact tag for now).

### M006 — Dependency provenance and source policy

Status: closed (see `plans/closure/distribution-update-release/006-status.md`)

Class: infrastructure + invariant

Objective: make the dependency source policy truthful for the interim git pin, and make a release attempt fail fast when a publishable dependency cannot be packaged.

Dependencies: none (local).

Deliverable boundary: exact-source `allow-git` entry, release provenance preflight, documentation; no dependency version, lockfile, or runtime change.

Exit conditions: `cargo deny check advisories bans licenses sources` green; the provenance preflight fails with an actionable message and passes on a registry-only fixture. M007 subsequently restored registry packaging and removed the temporary source exception.

Deferred work: none; the provenance preflight remains active to reject future versionless Git dependencies.

Implementation plan: `plans/archive/implementation/distribution-update-release/006-dependency-provenance-and-source-policy.md`.

Closure record: `plans/closure/distribution-update-release/006-status.md`.

### M007 — Eggup registry adoption

Status: closed — clean-tree release qualification and Maintenance passed (see closure record).

Class: infrastructure

Objective: replace the four git pins with registry version requirements for the coordinated Eggup release resolving `zip >= 9.6.1`, remove the interim source allow-list, and restore package/publish qualification. Completed locally on 2026-11-22.

Dependencies: satisfied by published Eggup 0.2.2 crates and a compatible public API; see the closure record for checksums and registry evidence.

Deliverable boundary: `Cargo.toml`/targeted `Cargo.lock` source change, empty `allow-git`, release-gate re-qualification. No updater behavior or feature change.

Exit conditions: the locked graph shows one `zip 9.6.1`; deny is green with an empty allow-list; package and publish-dry-run checks passed, and clean-tree `scripts/release-check.sh` passed on `a8416e9`.

Deferred work: actual publication remains a separate maintainer-owned action; this dependency update did not publish or tag a release.

Implementation plan: `plans/archive/implementation/distribution-update-release/007-eggup-registry-adoption.md`.

Closure record: `plans/closure/distribution-update-release/007-status.md`.

### M008 — Release authenticity via signature verification

Status: proposed

Class: invariant + infrastructure

Objective: verify a cryptographic signature over the release manifest before staging, so a compromised release channel cannot install bytes with a matching self-declared digest.

Dependencies: ADR-0002 (scheme, key custody, rotation, bootstrap) and an upstream Eggup signature carrier; the current signing non-goal must be explicitly lifted for this scope.

Deliverable boundary: ADR, producer signing through Eggpack configuration, updater verification with fail-closed semantics, negative fixtures, transition matrix.

Exit conditions: a signed rehearsal draft verifies end-to-end; unsigned/tampered/wrong-key/truncated inputs abort before staging; no key material in the repository.

Deferred work: installer signing, PKI/HSM procurement, non-update channels.

Implementation plan: `plans/implementation/distribution-update-release/008-release-authenticity-signature-verification.md`.

### M009 — Operator-surface hardening

Status: implemented (local merge gate green; closure record pending; Windows behavior reviewed statically)

Class: invariant + polish

Objective: close three local gaps: Windows PowerShell resolved through the OS search order, a staging directory created before it is restricted, and a Windows `integrate` escape that `cmd.exe` does not honor.

Dependencies: none.

Deliverable boundary: absolute PowerShell resolution, `0700`-at-creation on Unix, safe Windows rendering, tests and docs. No transport, retry, MCP, or read-only-guarantee change.

Exit conditions: no relative PowerShell lookup; mode is `0700` immediately on Unix; Windows output cannot expand a path-borne `%VAR%`; full merge gate green.

Deferred work: signature verification (M008).

Implementation plan: `plans/implementation/distribution-update-release/009-operator-surface-hardening.md`.

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

M006/M007 decision point: the exact-source Git allow-list was an interim security-refresh exception, not a publishable dependency policy. M007 replaced those pins with coordinated registry versions and returned `allow-git` to empty. The provenance preflight remains as a guard against future versionless Git dependencies; do not reintroduce git pins or add misleading `version` keys to them.

M008 decision point: signing is a current non-goal. It requires an accepted ADR covering scheme, key custody, rotation, and bootstrap before any implementation. Without that decision, update authenticity remains TLS plus a same-origin digest, and the milestone stays proposed.

## 11. Completion definition

M001-M005a remain closed historical milestones. M005 is closed with Eggpack producer authority qualified against a real draft/release, all eggsact-owned installer/updater/release-order semantics preserved, and its condition discharged by M005a. M006-M009 are the November 2026 audit follow-up: M006 and M007 are closed; M009 closure evidence remains pending, and M008 remains proposed. The workstream returns to maintenance-only once M009 closure evidence is recorded (weekly MSRV/cargo-deny/platform checks, Windows release reproducibility, Eggpack drift, and release-contract guards).

## 12. Milestone status

| Milestone | Status | Implementation plan | Closure record | Blockers |
|---|---|---|---|---|
| M001 binary distribution | closed | — (archive `plans/archive/roadmap.md` + commits) | — | — |
| M002 self-update 0.1.6 | closed | — (history in `plans/archive/roadmap.md`) | — | — |
| M003 0.1.7 bump | closed | `plans/archive/eggfetch-0.1.7-updater-dependency-bump.md` | in-file closure | — |
| M004 0.2.0 adoption | closed | `plans/archive/eggfetch-0.2.0-updater-adoption.md` | in-file closure | — |
| M004 corrective closeout | closed | `plans/archive/eggfetch-0.2.0-adoption-closeout-corrective.md` | in-file closure | — |
| M005 Eggpack producer adoption + live draft qualification | closed | `plans/archive/implementation/distribution-update-release/005-eggpack-producer-adoption-and-live-draft-qualification.md` | `plans/closure/distribution-update-release/005-status.md` | Condition discharged by M005a; all former hard dependencies closed |
| M005a Deterministic Windows release artifacts | closed | `plans/archive/implementation/distribution-update-release/005a-deterministic-windows-release-artifacts.md` | `plans/closure/distribution-update-release/005a-status.md` | None; M006-M009 opened by the November 2026 security audit |
| M006 dependency provenance and source policy | closed | `plans/archive/implementation/distribution-update-release/006-dependency-provenance-and-source-policy.md` | `plans/closure/distribution-update-release/006-status.md` | Maintenance `cargo-deny` and clean-tree release-check passed |
| M007 Eggup registry adoption | closed | `plans/archive/implementation/distribution-update-release/007-eggup-registry-adoption.md` | `plans/closure/distribution-update-release/007-status.md` | Registry dependency graph, package qualification, clean-tree release-check, and Maintenance passed |
| M008 release authenticity via signature verification | proposed | `plans/implementation/distribution-update-release/008-release-authenticity-signature-verification.md` | — | ADR-0002 acceptance and an upstream signature carrier |
| M009 operator-surface hardening | implemented | `plans/implementation/distribution-update-release/009-operator-surface-hardening.md` | — | Local merge gate green; closure record pending |
