# Distribution, Update, and Release Milestone 007 — Eggup Registry Adoption

Status: closed — registry adoption, clean-tree release qualification, and Maintenance passed; see `plans/closure/distribution-update-release/007-status.md`.

Dependency update landed in `a8416e9` on `audit/security-followups-006-009` (PR #1); no crate publication or tag was performed.

Source roadmap:

- `plans/subsystems/distribution-update-release-roadmap.md#7`
- `plans/subsystems/distribution-update-release-roadmap.md#10`

Long-term requirements:

- `plans/000-long-term-specification.md#6` (distribution and release)
- `plans/000-long-term-specification.md#3` (self-update exception)
- `plans/001-terminology-and-domain-model.md#7` (targets, transport, contract)
- `plans/002-long-term-roadmap.md#phase-3`

Applicable ADRs:

- `plans/adrs/ADR-0001-planning-conventions-adoption.md`

Primary class: infrastructure

## 1. Objective

Replace the four git-pinned Eggup dependencies with registry version
requirements from the coordinated release carrying the ZIP security refresh,
remove the interim source allow-list from milestone 006, and restore a working
`cargo package` / `cargo publish --dry-run` path. This objective is complete;
clean-tree qualification passed on `a8416e9` and is recorded in the closure record.

The fixed outcome is a publishable crate whose dependency graph resolves
`zip 9.6.1` or newer without relying on a git revision.

## 2. Why this milestone is ready

The upstream entry criteria were satisfied on 2026-11-22:

- crates.io publishes all four coordinated Eggup crates at 0.2.2; they are
  unyanked and declare Rust 1.89. Their exact checksums are recorded in the
  closure record.
- The locked graph resolves `zip 9.6.1` through `eggup-archive 0.2.2`; the
  prior published 0.2.0 line's `zip 6.x` constraint is no longer used.
- The published API integrates with `src/update.rs`; the full local test gate
  passed.

All local interfaces are known: `Cargo.toml:50-53`, the lockfile source
entries, `deny.toml` sources section, and the release gate.

## 3. Current implementation evidence

- `Cargo.toml:50-53` uses exact registry requirements for all four Eggup
  crates at 0.2.2; `deny.toml` has `allow-git = []`. Published versions and
  checksums are listed in `plans/closure/distribution-update-release/007-status.md`.
- `cargo tree --locked -i zip` resolves one `zip v9.6.1` through
  `eggup-archive 0.2.2` → `eggup-eggpack 0.2.2`; the same locked graph
  resolves `eggfetch-core 0.4.0` and `eggpack-manifest 0.1.2`.
- crates.io reports Eggfetch 0.4.0, Eggpack manifest 0.1.2, and Eggpack CLI
  0.1.2 as current stable releases; the binary workflow pins the CLI separately
  at the revision in `release/eggpack/github-policy.json`.
- `cargo deny check advisories bans licenses sources`, the provenance
  preflight and its fixture self-test, the full merge gate,
  `cargo package --locked --allow-dirty`, and
  `cargo publish --locked --dry-run --allow-dirty` all pass.
- The first deny run identified RUSTSEC-2026-0592 in `url 2.5.8`; the lockfile
  now resolves `url 2.5.9` and `form_urlencoded 1.2.3`, after which deny passed.
- `scripts/release-check.sh` passed on the clean commit `a8416e9`; the hosted
  Maintenance run `45089096709` also passed all five jobs. See the closure record.

## 4. Invariants that must not regress

- The resolved `zip` version stays at or above the security refresh on every
  target; no transitive downgrade through a second Eggup dependency line.
- No advisory is reintroduced; `[advisories] ignore` remains empty.
- `--locked` is used for every build, test, package, and publish-dry-run.
- Release determinism flags in `.cargo/config.toml` are untouched.
- `eggsact update` behavior, transport policy (HTTP/1, rustls, no retries),
  and staging semantics are unchanged by the source swap.
- No new updater features, retries, or HTTP surface.
- The source allow-list returns to empty; no wildcard exception remains.

## 5. Scope

### In scope

- `Cargo.toml` declarations and the targeted `Cargo.lock` update.
- Removal of the interim `allow-git` entry.
- Re-qualification of package/publish-dry-run and the updater test suite.
- Documentation of the adoption and its security rationale.

### Explicitly out of scope

- Signature verification (008).
- Local updater hardening (009).
- Any transport, retry, timeout, or feature change.
- Refactoring `src/update.rs` beyond the mechanical adaptation required by a
  published API change.
- Publishing or tagging (maintainer decision, separate action).

## 6. Required production changes

### Operator surface (CLI, update, integrate)

Only the mechanical adaptation if the published API differs from the pinned
commit. No behavior change. If adaptation exceeds a narrow mechanical delta,
stop: that becomes a separate plan.

### Core/service/adapter

None.

### Registry, profile, audience, surface

None.

### Protocol and DTOs

None.

### Runtime and concurrency

None.

### Documentation and static guards

- `docs/release.md`: dependency provenance now satisfied by registry
  versions; record the minimum Eggup version and ZIP floor.
- `AGENTS.md`: remove the interim-allow-list gotcha once `allow-git` is
  empty again.
- `scripts/check-release-contract.py`: no change required.

## 7. Ordered work packages

### Work package A — Verify the upstream release

Intent: prove the registry release actually carries the security refresh
before touching this repository.

Required changes: none in-tree. Record:

- published crate versions and checksums for all four Eggup packages;
- `zip` resolution from a scratch project or `cargo tree` against the
  published versions;
- changelog/release notes tying the release to the ZIP advisory fix;
- any API delta relevant to `src/update.rs`.

Acceptance evidence: a recorded tree showing `zip >= 9.6.1` and the exact
published versions.

### Work package B — Switch declarations and lock

Intent: consume the registry release.

Required changes:

- Replace `{ git = ..., rev = ... }` with `version = "<coordinated
  release>"` for all four crates.
- Update the lock with a targeted `cargo update` for the Eggup packages.
- Confirm no duplicate Eggup versions and no second `zip` major remain.

Acceptance evidence: `Cargo.lock` diff contains only the Eggup source/version
lines and their dependency closure; `cargo tree --locked -i zip` shows one
`zip` line at 9.6.1 or newer.

### Work package C — Restore the strict source policy

Intent: remove the interim exception now that no git source is needed.

Required changes:

- Return `allow-git` to `[]` in `deny.toml` (keep `unknown-git = "deny"`).
- Re-run the full deny check.

Acceptance evidence: `cargo deny check advisories bans licenses sources` is
green with an empty allow-list.

### Work package D — Re-qualify the release surfaces

Intent: prove the crate is releasable again.

Required changes:

- `cargo package --locked` (clean tree) and `cargo publish --locked
  --dry-run`.
- Updater-focused tests and the ordinary merge gate.
- `scripts/release-check.sh` on a clean tree, including the provenance
  preflight from 006.

Acceptance evidence: all commands green; package contents list contains no
unexpected files; publish dry-run resolves the registry Eggup versions.

### Work package E — Documentation and closure

Record the new minimum versions, the ZIP floor, the removed allow-list, and
the release-gate evidence.

## 8. Failure, cancellation, truncation, and contention semantics

- If the published release still resolves `zip 6.x` on any target, stop and
  keep the git pin; do not ship a downgrade.
- If the published API requires more than mechanical adaptation, stop and
  open a bounded adaptation plan; do not mix a source swap with behavior
  changes.
- If `cargo update` touches unrelated packages, discard and retry with
  package-scoped updates.
- Network failure during fetch/package is an error, not a pass; no offline
  claim without evidence.
- The lockfile is the single source of truth; never hand-edit it.

## 9. Compatibility and migration

- The published crate's consumers receive the security fix through the
  normal registry resolution.
- The binary's behavior is unchanged; only dependency provenance changes.
- A binary-size delta is expected and must be recorded, not minimized by
  changing features.
- Old eggsact releases remain vulnerable until a new release is published;
  this plan does not publish.

## 10. Required tests

### Focused unit tests

No new unit tests are required for a source swap. If API adaptation is
needed, the already-existing updater tests are the contract.

### Integration tests

- Full updater test module with the published dependencies.
- `cargo test --locked --all-features -- --skip parity --test-threads=4`.

### Restart and recovery tests

No change; confirm the staged Windows replacement tests still pass.

### Contention and cancellation tests

No change; confirm the existing cancellation/timeout suites.

### Negative and boundary tests

- `cargo tree --locked -i zip` must show exactly one `zip` line.
- A scratch resolution of the published versions must not pull `zip 6.x`.

### Parity and compatibility tests

Ordinary parity policy applies only if tool behavior changes, which it must
not. Record that parity was not run for a dependency-only change if that is
the case.

## 11. Required verification commands

```bash
cargo update -p eggup-core -p eggup-acquisition -p eggup-eggfetch -p eggup-eggpack
cargo tree --locked -i zip
cargo deny check advisories bans licenses sources
cargo package --locked
cargo publish --locked --dry-run
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
scripts/release-check.sh           # clean tree only
```

## 12. Documentation updates

- `docs/release.md` — minimum Eggup/ZIP versions and provenance rationale.
- `AGENTS.md` — remove the interim allow-list note; keep the packaging rule.
- Distribution roadmap and registry — status and closure links.

## 13. Acceptance criteria

- `Cargo.toml` has no git dependencies.
- `deny.toml` has `allow-git = []` and the full deny check is green.
- `zip` resolves at or above 9.6.1 with no second ZIP line.
- `cargo package` and `cargo publish --dry-run` succeed on a clean tree.
- The provenance preflight from 006 passes without modification.
- No updater behavior, transport policy, or feature set changed.

## 14. Stop conditions

Stop and report when:

- upstream has not published, or the published release still pins ZIP 6.x;
- the published API differs materially from the pinned commit;
- the only way to keep the security fix is to retain a git pin (then 006's
  allow-list stays and this plan remains blocked);
- a feature or transport change would be required to make the swap compile;
- the release gate exposes an unrelated failure that cannot be bounded to
  this milestone.

## 15. Closure evidence collected

- Published version/checksum record for all four crates.
- `cargo tree --locked -i zip` output.
- `Cargo.lock` source diff.
- Green deny output with an empty allow-list.
- Package and publish-dry-run output.
- Merge-gate, clean-tree release-check, and updater test results; Maintenance run evidence.
- A statement of the remaining path to actually publishing (maintainer-owned).

## 16. Handoff notes

- Entry criteria were met by the published Eggup 0.2.2 releases and their
  recorded dependency graph; a tag alone was not treated as evidence.
- Keep the lockfile churn minimal and reviewable.
- Do not edit the generated release workflow or Eggpack configuration for a
  dependency bump.
- Do not publish; publication remains a separate maintainer action.
