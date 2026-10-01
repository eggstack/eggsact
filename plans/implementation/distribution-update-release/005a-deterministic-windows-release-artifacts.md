# Distribution, Update, and Release Milestone 005a — Deterministic Windows Release Artifacts

Status: closed — see `plans/closure/distribution-update-release/005a-status.md`

Source finding: `plans/closure/distribution-update-release/005-status.md` ("Rerun reuse"), live run `36652731202` attempt 2.

Roadmap: `plans/subsystems/distribution-update-release-roadmap.md`

## 1. Objective

Make the `x86_64-pc-windows-msvc` release candidate byte-reproducible for a
fixed source revision and toolchain, so a rerun of the same tag can reuse the
already-staged draft asset without Eggpack's (correct) refusal to clobber a
differing asset.

## 2. Finding — medium: MSVC link output embeds build-time and random data

Proven at byte level by comparing attempt-1 (draft) and attempt-2 (rerun)
Windows candidates from run `36652731202`, which differ in exactly 24 bytes:

- PE COFF header `TimeDateStamp` at `0x108` (2 bytes);
- three import/descriptor time-date-stamps at `0xdcad24`, `0xdcad40`, `0xdcad5c`;
- the CodeView debug-directory RSDS block at `0xdcae74` (16 bytes: a fresh
  random PDB GUID plus age).

The other four targets are byte-identical between attempts, which is why only
the Windows asset blocks exact reuse.

## 3. Boundaries

- In scope: the product's Windows link/PDB settings and whatever workflow
  inputs they require, chosen and recorded here.
- Out of scope: Eggpack internals, release-policy relaxation, any change that
  weakens the artifact-digest guarantee, and any change to the five-target
  matrix or the draft-only staging contract.
- Eggpack must keep refusing to clobber a same-name asset whose digest differs;
  this milestone removes the reason for that refusal rather than relaxing the
  check.

## 4. Options to evaluate

1. `RUSTFLAGS`/linker `/Brepro` for the Windows target: makes the PE timestamp
   and PDB deterministic (hash-derived) instead of wall-clock and random.
2. Normalize debug info: strip or fully path-stabilize PDB references so the
   RSDS GUID is derived from content rather than a fresh random value.
3. If reproducibility is judged not worth the debuggability cost, record an
   explicit product decision that a Windows asset may be re-staged on rerun,
   and either (a) accept a documented one-time manual draft asset replacement
   during the maintainer review window, or (b) accept that reruns need a fresh
   version tag. This must be an explicit, recorded choice — never a silent
   clobber.

Option 1 plus option 2 is the expected outcome; option 3 is a legitimate
product fallback but must be recorded as a deliberate policy change.

## 5. Acceptance criteria

- Two consecutive builds of the same source revision and toolchain produce
  identical Windows candidate digests, demonstrated in CI and recorded here.
- A rerun of the same tag stages/reuses all five assets with zero
  digest-mismatch refusals.
- No change to asset names, sidecar format, installer behavior, updater
  mapping, or the single write-authorized staging job.

## 6. Verification

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity
python3 scripts/check-release-contract.py
scripts/release-check.sh
```

Plus a CI job (or documented local loop) that builds the Windows target twice
and compares SHA-256 of the two candidates.

## 7. Stop conditions

Stop and re-plan if reproducibility requires changing the published asset
contract, weakening Eggpack's digest checks, or dropping the Windows target.

## 8. Closure evidence

Create `plans/closure/distribution-update-release/005a-status.md` recording the
chosen option, the two-build digest evidence, and a rerun receipt showing full
asset reuse.
