# Distribution, Update, and Release Roadmap

Status: closed; maintenance only (new work requires a corrective plan)

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

Closed. Binary distribution qualified (C8 Zig correction `6658702`,
v1.2.4 matrix, workflow `33944943782`, exact-tag installer checks).
Self-update consolidated on `eggfetch-core 0.2.0` with the pinned
feature/trust profile; lockfile 166 packages; +48-byte stripped delta
on adoption; 19 updater tests green; ordinary CI `35734609288` +
maintenance `35740940881` (MSRV, cargo-deny, native Windows, native
macOS) green. No release published by the bump/corrective lines; the
dependency reaches users with the next normal release.

## 5. Target architecture

Unchanged closed end state. Future dependency or installer work is a
new corrective milestone, not a silent roadmap edit.

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

All hard/operational dependencies are closed.

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

No open risks. HTTP surface expansion, retries, or extra eggfetch
features require measurement and an ADR first.

## 11. Completion definition

Workstream closed. It reopens only via a new corrective implementation
plan referencing the archive record and adding regression evidence.

## 12. Milestone status

| Milestone | Status | Implementation plan | Closure record | Blockers |
|---|---|---|---|---|
| M001 binary distribution | closed | — (archive `plans/archive/roadmap.md` + commits) | — | — |
| M002 self-update 0.1.6 | closed | — (history in `plans/archive/roadmap.md`) | — | — |
| M003 0.1.7 bump | closed | `plans/archive/eggfetch-0.1.7-updater-dependency-bump.md` | in-file closure | — |
| M004 0.2.0 adoption | closed | `plans/archive/eggfetch-0.2.0-updater-adoption.md` | in-file closure | — |
| M004 corrective closeout | closed | `plans/archive/eggfetch-0.2.0-adoption-closeout-corrective.md` | in-file closure | — |
