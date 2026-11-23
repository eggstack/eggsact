# Distribution, Update, and Release Milestone 006 — Dependency Provenance and Source Policy

Status: implemented — local merge gate, provenance checks, and `cargo deny` green; M007 has restored registry packaging and removed the temporary source exception. Closure remains pending a green maintenance run and a clean-tree `release-check.sh`.

Repository baseline reviewed: `b299d49` on `audit/security-followups-006-009` plus the uncommitted audit/dependency refresh (`Cargo.toml`, `Cargo.lock`, API adaptations).

Source roadmap:

- `plans/subsystems/distribution-update-release-roadmap.md#7`
- `plans/subsystems/distribution-update-release-roadmap.md#10`

Long-term requirements:

- `plans/000-long-term-specification.md#6` (distribution and release)
- `plans/000-long-term-specification.md#7` (invariants)
- `plans/001-terminology-and-domain-model.md#7` (targets, transport, contract)
- `plans/002-long-term-roadmap.md#phase-3`

Applicable ADRs:

- `plans/adrs/ADR-0001-planning-conventions-adoption.md`
- No new ADR is required for an interim, exact-source, exact-SHA allow-list entry. A durable policy that permits git-sourced **published** artifacts would require an ADR before this plan is amended.

Primary class: infrastructure + invariant

## 1. Objective

Restore a truthful dependency-source signal and make the release gate fail
fast with an actionable message when a publishable dependency cannot be
packaged.

Two bounded outcomes:

1. `cargo deny check sources` passes because the one reviewed Eggup git
   source is explicitly allow-listed, instead of the current state where the
   scheduled maintenance job is red for a known-and-accepted source.
2. A release preflight refuses to package or publish any dependency declared
   with `git` but without a `version`, naming milestone 007 as the only
   remediation.

At M006 implementation time, this plan did **not** restore releasability;
`cargo package` was intentionally blocked until M007 replaced the git pins.
M007 has since restored registry packaging and removed the temporary source
allow-list; see `plans/closure/distribution-update-release/007-status.md`.

## 2. Why this milestone is ready

- Both control points are local: `deny.toml` and `scripts/release-check.sh`.
- The failures are reproducible on the current baseline and were verified
  during the audit; no external service or credential is involved.
- The upstream dependency work is isolated in 007 and does not block the
  policy/guard work here.
- No runtime, tool, schema, or protocol behavior changes, so the ordinary
  merge gate is unaffected.

## 3. Evidence at M006 implementation time (historical; M007 supersedes the package blocker)

- `Cargo.toml:50-53` declares `eggup-acquisition`, `eggup-core`,
  `eggup-eggfetch`, and `eggup-eggpack` as git dependencies with `rev` and no
  `version`. `HEAD` (`e336b320…`) has the same shape at an older revision, so
  this predates the current uncommitted refresh.
- `cargo package --locked --no-verify --allow-dirty` fails with:
  `error: all dependencies must have a version specified when packaging.
  dependency 'eggup-acquisition' does not specify a version`.
- `deny.toml:34-38` sets `unknown-registry = "deny"`,
  `unknown-git = "deny"`, and `allow-git = []`.
- `cargo deny check sources` reports `source-not-allowed` once per
  git-sourced Eggup package.
- `.github/workflows/maintenance.yml:30-40` runs
  `cargo deny check advisories bans licenses sources`, so the maintenance job
  fails on the current baseline. `bans` and `licenses` are otherwise green.
- `scripts/release-check.sh:28-35` runs the same four deny checks, and
  `scripts/release-check.sh:37-41` runs `cargo package` and
  `cargo publish --dry-run` after the full test gate.
- The lock resolves `zip 9.6.1` through `eggup-archive`; the published Eggup
  0.2.0 tag still resolved `zip 6.0.0` (established by the prior dependency
  audit). Avoiding a silent downgrade is the reason `version` keys must not
  be added to the git declarations as a shortcut.
- `scripts/check-release-contract.py` is the release-matrix guard run by
  `.github/workflows/release-drift.yml:43`; it does not inspect dependency
  provenance today.

## 4. Invariants that must not regress

- `[advisories] ignore = []` stays empty; no advisory, license, or ban
  exception is added.
- Exactly one reviewed git source may be allow-listed. Any second,
  unlisted git source must still be denied.
- Every Cargo invocation in the gate keeps `--locked`.
- No `version` key is added to a git dependency in this milestone: that
  would let packaging succeed while `cargo publish` resolves the vulnerable
  registry 0.2.0 release.
- Published artifacts remain registry-sourced; git pins are an explicit,
  temporary, exact-SHA exception.
- `update`/`integrate` remain verified and read-only; nothing here edits
  client configuration or installs a daemon.
- Release determinism flags in `.cargo/config.toml` are untouched.

## 5. Scope

### In scope

- `deny.toml` source allow-list entry for the reviewed Eggup repository.
- A dependency-provenance preflight invoked by `scripts/release-check.sh`
  before formatting/tests so a release attempt fails in seconds.
- A negative/positive verification of that preflight against fixture
  manifests, without mutating the repository.
- Documentation of the provenance policy and the interim exception.

### Explicitly out of scope

- Changing dependency versions, revisions, or lockfile contents (007).
- Publishing, tagging, or running a live release.
- Signature verification or trust-root changes (008).
- Local updater hardening (009).
- Any MCP tool, schema, registry, profile, or protocol change.
- Adding the provenance preflight to merge-gating CI while the condition is
  unmet; that would redden every pull request instead of the release path.

## 6. Required production changes

### Operator surface (CLI, update, integrate)

Add the provenance preflight to the release path. It must run after the
clean-tree check and before the test gate, exit non-zero, and print:

- which dependency is versionless and git-sourced;
- why this cannot be repaired in place (registry publication resolves the
  registry version, not the pinned revision);
- the owning plan (`007`) and the upstream dependency that gates it.

### Core/service/adapter

None.

### Registry, profile, audience, surface

None.

### Protocol and DTOs

None.

### Runtime and concurrency

None.

### Documentation and static guards

- `docs/release.md`: state that published artifacts must be registry-sourced
  and that git pins are interim, exact-SHA, and allow-listed.
- `AGENTS.md`: add the provenance preflight to the release-gate command list
  and note that git-sourced publishable dependencies fail packaging.
- Keep `scripts/check-release-contract.py` behavior unchanged; the drift
  guard must not start failing on dependency provenance.

## 7. Ordered work packages

### Work package A — Explicit source allow-list

Intent: make the source policy truthful for the accepted interim state.

Required changes:

- Add the canonical Eggup repository URL to `allow-git` in `deny.toml`.
  Confirm cargo-deny 0.19 URL matching (no `.git` suffix, no revision
  query); do not use a wildcard or a prefix that would admit forks.
- Leave `unknown-git = "deny"` and `unknown-registry = "deny"` intact.

Acceptance evidence:

- `cargo deny check advisories bans licenses sources` is green on the
  current lock.
- A scratch copy with a second, unlisted git dependency still fails
  `sources` (record the command and output; do not commit the scratch change).

### Work package B — Provenance preflight

Intent: fail a release attempt immediately, with a message an operator can
act on.

Required changes:

- Add a small, dependency-free check (new script or a mode of an existing
  guard) that parses `Cargo.toml` with `tomllib` and rejects any dependency
  table entry containing `git` without `version`.
- Fail closed on unreadable or malformed manifests, with a distinct message.
- Invoke it from `scripts/release-check.sh` before formatting.
- Keep it out of the merge-blocking CI commands.

Acceptance evidence:

- Running the check against the current `Cargo.toml` exits non-zero and
  names `eggup-acquisition`, the registry-publication reason, and plan 007.
- Running the same check against a fixture manifest with
  `version = "0.2.1"` and no `git` exits zero.
- A malformed-manifest fixture exits non-zero with a parse message, not a
  traceback.

### Work package C — Documentation

Intent: make the interim exception and removal criterion explicit.

Required changes:

- `docs/release.md`: provenance policy, why packaging fails, and the
  removal criterion (007).
- `AGENTS.md`: release-gate gotcha plus the new preflight.

Acceptance evidence: links from the plan are valid and the wording matches
the observed commands.

### Work package D — Closure preparation

Collect command outputs, the diff, and the maintenance run that turns green
after work package A. Do not close 006 before the maintenance `cargo-deny`
job is green and 007 is registered as blocked.

## 8. Failure, cancellation, truncation, and contention semantics

- Preflight exits non-zero and prints no partial success. It never edits
  `Cargo.toml`, the lockfile, or `deny.toml`.
- `cargo deny` failures list the offending source and package; no retry,
  no fallback registry.
- Concurrent local runs are read-only and idempotent.
- A missing `python3`/`tomllib` (Python < 3.11) fails closed with an
  explicit toolchain message rather than skipping the check.
- Network unavailability during `cargo deny` is an error, not a pass.

## 9. Compatibility and migration

- No library or MCP API change.
- `release-check.sh` gains an earlier failure point; this is intentional and
  documented.
- CI remains green except the maintenance source check, which this milestone
  fixes.
- 007 remains the only migration path; no temporary publishing mode is
  introduced.

## 10. Required tests

### Focused unit tests

- Manifest parser: registry-only dependency passes; `git` without `version`
  fails; `git` plus `version` passes (that combination is legal Cargo but
  must still be reported as an interim source by the source policy, not by
  this preflight).
- Malformed TOML and missing `[dependencies]` behavior.

### Integration tests

- Run the check against the repository manifest and assert the expected
  failure text on the baseline.
- Run it against a temporary fixture manifest and assert success.

### Restart and recovery tests

Not applicable; the check is stateless.

### Contention and cancellation tests

Not applicable; no shared state.

### Negative and boundary tests

- Empty manifest, dependency table with only workspace/path entries, and
  target-specific dependency tables (`[target.'cfg(unix)'.dependencies]`)
  must be handled explicitly: the preflight must scan all dependency tables,
  or must document why a subset is sufficient.

### Parity and compatibility tests

None; no tool behavior changes.

## 11. Required verification commands

```bash
cargo deny check advisories bans licenses sources
python3 scripts/check-release-contract.py
python3 <new provenance preflight>        # expected non-zero on this baseline
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
```

`scripts/release-check.sh` requires a clean tree, so it can only be run after
the change is committed; do not claim a release-check run that did not
happen. No Rust test suite is required for a script/doc-only change, but run
the ordinary merge gate if any Rust file is touched.

## 12. Documentation updates

- `docs/release.md` — provenance policy and interim exception.
- `AGENTS.md` — release-gate gotcha and command list.
- `plans/registry.md` and the distribution roadmap — status only, per the
  planning process.

## 13. Acceptance criteria

- The maintenance `cargo-deny` job is green with all four checks enabled and
  no advisory/license/ban exceptions.
- The release path fails in seconds with a message naming the versionless
  dependency, the registry-substitution reason, and plan 007.
- A registry-only fixture manifest passes the preflight.
- No `version` key is added to a git dependency; no lockfile dependency
  version changes.
- Publishing remains impossible until 007; no temporary publishing mode,
  `--allow-dirty` workflow, or gate bypass is introduced.

## 14. Stop conditions

Stop and report rather than improvise when:

- cargo-deny cannot express the exact source without a wildcard or a broader
  prefix;
- the only way to green the check is to relax `unknown-git`,
  `unknown-registry`, advisories, licenses, or bans;
- maintainers reject an interim git-source exception (then the maintenance
  job stays red until 007 and this plan is superseded);
- the guard would need to run in merge-gating CI to be useful, which would
  redden every pull request;
- scope expands into dependency versions, update behavior, or signing.

## 15. Closure evidence required

The closure record must contain:

- the `deny.toml` diff and the exact `cargo deny check advisories bans
  licenses sources` output;
- the preflight's failing output on the baseline and passing output on a
  registry-only fixture;
- the scratch second-source denial evidence;
- a green maintenance run URL for the `cargo-deny` job;
- links to the documentation changes;
- an explicit statement that `cargo package` remains blocked and 007 owns it.

## 16. Handoff notes

- The working tree is dirty from the dependency refresh; do not run
  `scripts/release-check.sh` before committing and expect it to progress.
- Do not "repair" packaging by adding `version` keys to the four git
  dependencies. That is the trap this plan exists to prevent.
- Keep `zip 9.6.1` in the lock; a source-policy change must not alter the
  dependency graph.
- `python3` on the development host must provide `tomllib` (3.11+); record
  the version used.
- Implementation note (2026-11-17): the host runs `cargo-deny 0.20.2` (the
  gate requires 0.19+). `allow-git = ["https://github.com/eggstack/eggup"]`
  matches the lock source `git+https://github.com/eggstack/eggup.git?rev=…`
  without a `.git` suffix or revision query. A scratch project with an
  unlisted `anyhow` git dependency still fails with `source-not-allowed`.
  Python 3.14.2 supplies `tomllib`; the preflight and its 10-case self-test
  pass on the current host.
