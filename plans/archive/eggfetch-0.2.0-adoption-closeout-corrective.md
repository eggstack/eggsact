# Eggfetch 0.2.0 Adoption Corrective Closeout

Planning baseline: `d7e51d0a7b6ada3730b6c9f560fe8f7da7b097b5` (`main`, 2026-09-22)
Implementation commit: `bfe12d760554f053dcc6028f32fb3e41dff58a5f`
Prior adoption plan: `eggfetch-0.2.0-updater-adoption.md`
Status: closed

## Objective

Close the remaining evidence and planning-state gaps in the already-implemented
`eggfetch-core 0.2.0` updater adoption without reopening the transport design.

The implementation itself is currently accepted. The corrective exists because
the adoption plan requires supported-platform qualification, while its closure
record deferred the Windows check to the remote `maintenance.yml` workflow.
The recorded remote run `35734609288` proves ordinary Linux CI only. It does
not prove the Windows maintenance job. The roadmap nevertheless currently
describes the adoption as fully landed and the original plan still carries an
`active implementation handoff` header with an unchecked completion list.

This pass must make the evidence and planning state agree with reality.

## Current accepted implementation

Do not change these facts unless qualification exposes a real defect:

- `Cargo.toml` requires `eggfetch-core 0.2.0`.
- The feature set remains exactly
  `http1,tls-rustls,tls-native-roots,proxy`.
- `Cargo.lock` resolves `eggfetch-core 0.2.0` and
  `eggfetch-http-connect 0.2.0`.
- No `compression-*` feature is enabled.
- `.automatic_decompression(false)` remains configured.
- `src/update.rs` required no behavioral compatibility adaptation.
- The targeted lockfile movement is limited to the two eggfetch package
  versions/checksums; resolved package count remains 166.
- The measured stripped release delta is +48 bytes.
- The 19 updater regressions are recorded green.
- Tier 1, release-contract/MCP smoke, local Rust 1.89, local cargo-deny,
  native macOS, and latest-compatible qualification are already recorded
  green.
- Ordinary remote Linux CI run `35734609288` is recorded green.

The missing evidence is the remote supported-platform maintenance gate,
especially native `windows-latest`.

## Defect statement

The current closure state has three inconsistencies:

1. The adoption plan completion criteria require the Windows supported-platform
   compile check to pass.
2. The closure record says the local Windows check was blocked by the absence
   of an MSVC C compiler and explicitly defers Windows evidence to
   `.github/workflows/maintenance.yml`.
3. The roadmap already labels the line `landed` and describes Windows/macOS
   supported-platform qualification as complete even though the recorded
   remote run is `ci.yml`, whose only job is Linux correctness.

Two documentation-state defects accompany that evidence gap:

- `plans/eggfetch-0.2.0-updater-adoption.md` still says
  `Status: active implementation handoff`;
- its completion checklist remains unchecked despite the appended closure
  record.

These are closure defects, not evidence of an updater implementation defect.

## Part A — Freeze the corrective scope

Before running qualification, verify that current `main` still contains the
accepted adoption state:

```sh
git status --short
git log -5 --oneline
git diff bfe12d760554f053dcc6028f32fb3e41dff58a5f..HEAD --   Cargo.toml Cargo.lock src/update.rs .github/workflows/maintenance.yml
```

Planning-only commits after `bfe12d7` are acceptable. If current `main`
contains later changes to the dependency graph, updater implementation, or
maintenance workflow, record them and qualify the actual current head rather
than claiming the old evidence covers changed code.

Do not modify updater code, dependencies, features, timeouts, redirects, proxy
policy, compression policy, tests, or workflows merely to make this corrective
look active.

## Part B — Run the remote maintenance gate

Dispatch the repository's existing `Maintenance` workflow against current
`main`.

The required workflow is:

```text
.github/workflows/maintenance.yml
```

It must execute the existing jobs:

- `MSRV` on Ubuntu with Rust 1.89.0;
- `cargo-deny` on Ubuntu;
- `Check (windows-latest)`;
- `Check (macos-latest)`.

The platform jobs must continue to run:

```sh
cargo check --locked --all-targets --all-features
```

Do not replace the Windows job with cross-compilation from macOS/Linux. The
purpose of this corrective is to obtain the native MSVC evidence that the
original adoption closure explicitly deferred.

A normal execution sequence with GitHub CLI is:

```sh
gh workflow run maintenance.yml --ref main
gh run list --workflow maintenance.yml --branch main --limit 5
gh run watch <run-id> --exit-status
```

Record:

```text
workflow run id:
workflow URL:
event:
head branch:
head SHA:
MSRV:
cargo-deny:
Windows:
macOS:
overall conclusion:
```

The recorded head SHA must be the intended corrective/current-main revision.
If main advances before dispatch, inspect the intervening diff before accepting
the run as evidence for this line.

## Part C — Failure handling

If all maintenance jobs pass, proceed directly to documentation normalization.

If a job fails:

1. inspect the failed job and logs before changing code;
2. classify the failure as:
   - transient GitHub/registry/network infrastructure;
   - pre-existing repository/toolchain failure unrelated to eggfetch;
   - eggfetch 0.2.0 adoption regression;
   - corrective/planning-only mistake;
3. for a clearly transient infrastructure failure, rerun the failed job once
   and retain both run IDs in the closure record;
4. for an eggfetch/adoption regression, reopen implementation scope only as far
   as necessary to fix the demonstrated failure;
5. after any implementation/dependency change, rerun the focused updater tests,
   Tier 1, release contract, and the full remote maintenance workflow before
   closure.

Do not waive a reproducible Windows failure because the 0.1.7 baseline had a
similar local cross-compilation limitation. Native Windows CI is the evidence
this plan exists to obtain.

Do not weaken `maintenance.yml`, remove `--locked`, drop
`--all-targets --all-features`, or exclude eggfetch/updater code to manufacture
a green result.

## Part D — Reconcile the original adoption plan

After the required remote maintenance evidence is green, update
`plans/eggfetch-0.2.0-updater-adoption.md`.

Make the minimum status corrections:

1. change the header from:
   ```text
   Status: active implementation handoff
   ```
   to a completed/closed status consistent with repository convention;
2. mark completion-criteria checkboxes complete only where the existing closure
   record plus this corrective provides evidence;
3. replace the deferred Windows wording in the closure record with the native
   maintenance run result and run ID/URL;
4. record the remote macOS/MSRV/cargo-deny results from the same maintenance
   run;
5. retain the local cross-compilation limitation as historical context only if
   useful, making clear it is no longer the qualification boundary;
6. do not rewrite the implementation SHA, footprint measurements, lockfile
   facts, upstream issue #24 references, or historical 0.1.6/0.1.7 records.

If any completion criterion genuinely remains unproven, leave it unchecked and
do not close this corrective.

## Part E — Correct the roadmap state

Update `plans/roadmap.md` only after Part B is green.

The eggfetch 0.2.0 section may remain `landed`, but its qualification statement
must cite or identify the native maintenance evidence rather than implying that
ordinary Linux CI supplied Windows/macOS proof.

Add a concise corrective-closeout note containing:

- implementation commit `bfe12d7`;
- ordinary Linux CI run `35734609288`;
- maintenance workflow run ID;
- native Windows result;
- native macOS result;
- remote MSRV result;
- remote cargo-deny result;
- statement that no updater/dependency implementation change was needed, if
  true.

If qualification finds a real implementation defect, temporarily change the
roadmap line back to active/corrective until the fix and rerun are green.

## Part F — Corrective verification

For a documentation-only closeout after a green maintenance run, run the
documentation-sensitive local checks:

```sh
cargo fmt --all -- --check
cargo run --locked --features dev-tools --bin generate-docs -- --check
python3 scripts/check-release-contract.py
git diff --check
```

If this corrective changes Rust code, dependencies, tests, or workflow logic,
the documentation-only gate is insufficient. Rerun the full adoption gate:

```sh
cargo fmt --all -- --check
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
python3 scripts/check-release-contract.py
bash -n packaging/install.sh
cargo build --locked --release
./target/release/eggsact --version
python3 scripts/smoke-mcp-binary.py ./target/release/eggsact
```

Then rerun `maintenance.yml` on the resulting commit.

## Expected file changes

For the expected evidence-only path:

- `plans/eggfetch-0.2.0-updater-adoption.md`;
- `plans/roadmap.md`;
- this corrective plan.

No `Cargo.toml`, `Cargo.lock`, `src/update.rs`, test, workflow, installer,
MCP/tool-surface, or public-library change is expected.

Any such change requires an explicit defect found by qualification and must be
documented in this corrective's closure record.

## Explicit non-goals

Do not use this closeout to:

- redesign the updater;
- alter the eggfetch feature set;
- enable compression/decompression;
- add retries, HTTP/2, or HTTP/3;
- change proxy/TLS/redirect/timeout policy;
- update unrelated dependencies;
- optimize binary size;
- redesign CI;
- add a second Windows qualification mechanism;
- rerun issue #24's upstream decoder matrix inside Eggsact;
- publish a new Eggsact release.

## Completion criteria

This corrective is complete only when:

- [x] Current main is confirmed to retain the accepted eggfetch 0.2.0 adoption
  state or any intervening relevant changes are explicitly requalified.
- [x] A remote `maintenance.yml` run is recorded with its head SHA and URL.
- [x] Remote Rust 1.89 MSRV job passes.
- [x] Remote cargo-deny job passes.
- [x] Native `windows-latest` platform check passes.
- [x] Native `macos-latest` platform check passes.
- [x] No maintenance gate was weakened to obtain green evidence.
- [x] Any transient rerun is documented with both run IDs.
- [x] Any discovered implementation defect is fixed narrowly and the full
  required qualification is rerun.
- [x] The original adoption plan status no longer says active.
- [x] The original adoption completion checklist matches the actual evidence.
- [x] The original closure record includes the native Windows evidence.
- [x] The roadmap qualification statement matches the actual workflow evidence.
- [x] Documentation-sensitive local checks pass after state normalization.
- [x] No unrelated implementation, dependency, MCP, release, or workflow
  changes are introduced.
- [x] No release is published by this corrective.

## Closure record template

Append before marking this corrective complete:

```text
Corrective baseline: d7e51d0a7b6ada3730b6c9f560fe8f7da7b097b5
Eggfetch implementation: bfe12d760554f053dcc6028f32fb3e41dff58a5f
Corrective implementation/docs commit:
Qualified head SHA:

Ordinary Linux CI: 35734609288 — green
Maintenance run:
Maintenance URL:
Maintenance event:
Maintenance head SHA:
MSRV 1.89:
cargo-deny:
Windows native:
macOS native:
Rerun(s), if any:

Relevant code/dependency diff since bfe12d7:
Updater/dependency changes required by corrective:
Focused updater/Tier 1/release-contract rerun required?:
Documentation-sensitive local gate:

Original adoption plan status:
Original completion checklist:
Original closure Windows evidence:
Roadmap state/evidence:

Known limitations:
Release performed: no
Final disposition:
```

## Exit criterion

The eggfetch 0.2.0 adoption is fully closed only when native Windows and macOS,
remote MSRV, and remote dependency-policy evidence exist for the accepted
implementation/current head and the adoption plan plus roadmap accurately
reflect that evidence. If the maintenance workflow is green without code
changes, this corrective should remain a documentation/evidence closeout and
end there.

## Closure record

```text
Corrective baseline: d7e51d0a7b6ada3730b6c9f560fe8f7da7b097b5
Eggfetch implementation: bfe12d760554f053dcc6028f32fb3e41dff58a5f
Corrective implementation/docs commit: docs-only closeout on top of 0c30b1d
  (see git log; no code/dependency/workflow changes)
Qualified head SHA: 0c30b1d552f067f8eb5661124d4833fa52fd2611

Ordinary Linux CI: 35734609288 — green
Maintenance run: 35740940881
Maintenance URL: https://github.com/eggstack/eggsact/actions/runs/35740940881
Maintenance event: workflow_dispatch
Maintenance head SHA: 0c30b1d552f067f8eb5661124d4833fa52fd2611
MSRV 1.89: success (MSRV job)
cargo-deny: success (cargo-deny job)
Windows native: success (Check windows-latest, cargo check --locked
  --all-targets --all-features)
macOS native: success (Check macos-latest, cargo check --locked
  --all-targets --all-features)
Rerun(s), if any: none (first dispatch green)

Relevant code/dependency diff since bfe12d7: none in Cargo.toml, Cargo.lock,
  src/update.rs, or .github/workflows/maintenance.yml (verified via
  git diff bfe12d7..HEAD -- Cargo.toml Cargo.lock src/update.rs
  .github/workflows/maintenance.yml, empty). Intervening commits are
  planning/docs only (corrective plan addition, roadmap qualification-state
  correction).
Updater/dependency changes required by corrective: none
Focused updater/Tier 1/release-contract rerun required?: no (no implementation/
  dependency change; documentation-sensitive local gate suffices per Part F)
Documentation-sensitive local gate: green (cargo fmt --check, generate-docs
  --check, check-release-contract.py, git diff --check; plus full merge gate
  green locally: clippy -D warnings, cargo test --skip parity, cargo test --doc)

Original adoption plan status: closed (was "active implementation handoff")
Original completion checklist: all checked (Windows/macOS/MSRV/cargo-deny now
  evidenced by Maintenance 35740940881)
Original closure Windows evidence: native windows-latest success via
  Maintenance 35740940881 (URL above); local MSVC-blocked cross-check retained
  as historical context only, no longer the qualification boundary
Roadmap state/evidence: 0.2.0 adoption section back to "landed" with native
  Maintenance evidence cited (not Linux CI); closeout corrective section marked
  "closed" with run IDs

Known limitations: local Windows cross-check from macOS still requires an MSVC
  toolchain (historical only); live crates.io updater smoke remains optional
  and not run
Release performed: no
Final disposition: evidence/state-only closeout; adoption fully closed with no
  implementation change
```
