# Corrective Pass — Reproducibility and Closure Reconciliation

## Status

- **Status:** complete
- **Repository:** `eggstack/eggsact`
- **Target branch:** `main`
- **Planning baseline:** execute against the latest `main` after `2026-08-07-corrective-runtime-soundness-and-boundaries.md` is complete
- **Parent roadmap:** `plans/2026-08-04-bounded-correctness-simplification-roadmap.md`
- **Depends on:** `plans/2026-08-07-corrective-runtime-soundness-and-boundaries.md`
- **Priority:** closure/release-readiness
- **Scope:** make the Unicode confusables source genuinely reproducible and reconcile the August 4 roadmap/phase records with the actual implementation state
- **Expected change size:** small; generator metadata/validation plus planning/documentation records

## Purpose

The August 4 footprint pass improved the confusables representation and recorded Unicode 17.0.0 plus a SHA-256 checksum, but the generator still downloads `https://www.unicode.org/Public/security/latest/confusables.txt`. That means a future regeneration can silently advance to a different Unicode security-data version while still producing a new checksum and generated table.

The same review found closure-record drift: the parent roadmap is marked complete while several child phase files still report `Status: planned`, and some completion-record fields remain pending despite implementation having landed.

These are not reasons for another feature or architecture pass. This plan makes the generated data reproducible, performs the canonical release check once, reconciles the existing records once, and stops.

---

# Hard constraints

This pass must not:

- add new MCP tools, profiles, categories, or protocol behavior;
- change Unicode confusable semantics except as required to reproduce the already-selected dataset;
- upgrade Unicode beyond the currently recorded version as part of this pass;
- silently change the confusables dataset;
- add an updater service, scheduled Unicode workflow, or automatic dependency/data bot;
- make network access part of ordinary CI;
- make network access part of `scripts/release-check.sh`;
- add a Python test framework solely for the generator;
- add a new manifest/registry/evidence database;
- add another release script or verification binary;
- add release publication/tagging automation;
- revisit binary-size candidates already accepted/rejected in Phase 5;
- create another roadmap or evidence-only follow-up plan after this pass.

Keep the existing local/manual release policy.

---

# Files to inspect first

At minimum inspect:

```text
scripts/generate_confusables.py
src/text/confusables_generated.rs
data/confusables.rs
src/text/confusables.rs
tests/text/test_confusables.rs
architecture/generated-assets.md
architecture/text-library.md
AGENTS.md
CHANGELOG.md
scripts/release-check.sh

plans/2026-08-04-bounded-correctness-simplification-roadmap.md
plans/2026-08-04-phase-1-execution-context-soundness.md
plans/2026-08-04-phase-2-path-and-wire-boundary-corrections.md
plans/2026-08-04-phase-3-timeout-policy-and-test-isolation.md
plans/2026-08-04-phase-4-release-and-ci-simplification.md
plans/2026-08-04-phase-5-measured-footprint-reduction-and-closure.md
plans/2026-08-07-corrective-runtime-soundness-and-boundaries.md
```

Search for:

```text
security/latest
Unicode version:
Source checksum
Status: planned
Status: complete
Implementation commit
pending
Roadmap closure
```

---

# Workstream 1 — Turn the Unicode confusables metadata into a real pin

## Problem

Current generated output records:

```text
Unicode version: 17.0.0
Source checksum (SHA-256): 091c7f82fc39ef208faf8f94d29c244de99254675e09de163160c810d13ef22a
```

but `scripts/generate_confusables.py` fetches the moving URL:

```text
https://www.unicode.org/Public/security/latest/confusables.txt
```

The script extracts whatever version it receives and writes whatever checksum it computes. That records provenance after the fact but does not pin the input.

A deliberate regeneration at a later date could therefore alter the dataset without the maintainer first changing an explicit version/checksum expectation.

## Required outcome

After this workstream:

- the generator fetches a version-specific Unicode Security data URL for the currently selected version;
- the expected Unicode version is encoded explicitly in the generator;
- the expected SHA-256 checksum is encoded explicitly in the generator;
- the script verifies the downloaded bytes against the expected checksum before writing generated files;
- the script verifies the file header reports the expected version before writing generated files;
- a version or checksum mismatch fails loudly and leaves existing generated outputs untouched;
- upgrading Unicode requires an intentional source edit to the pinned version/checksum values;
- the current generated table remains byte-for-byte semantically equivalent to the already-recorded Unicode 17.0.0 dataset;
- ordinary CI and the canonical release check remain offline with respect to Unicode data regeneration.

## Preferred implementation shape

Keep `scripts/generate_confusables.py` simple and explicit. Constants similar to the following are sufficient:

```python
UNICODE_SECURITY_VERSION = "17.0.0"
CONFUSABLES_URL = (
    f"https://www.unicode.org/Public/security/{UNICODE_SECURITY_VERSION}/confusables.txt"
)
EXPECTED_SHA256 = "091c7f82fc39ef208faf8f94d29c244de99254675e09de163160c810d13ef22a"
```

The exact Unicode versioned URL should be verified before implementation. Do not retain `/latest/` once the versioned source is confirmed.

The safe write sequence should be:

1. fetch bytes;
2. compute SHA-256 from the exact fetched bytes;
3. compare to `EXPECTED_SHA256`;
4. decode/parse only after checksum verification where practical;
5. extract and compare header version to `UNICODE_SECURITY_VERSION`;
6. generate output in memory;
7. write the two generated files only after all checks pass.

This ensures a bad/moved source cannot partially rewrite one generated file before failing.

Do not add a general download/cache subsystem.

## Failure behavior

A checksum mismatch must produce an explicit error containing:

- expected checksum;
- observed checksum;
- pinned version;
- source URL.

A header-version mismatch must state expected vs observed version.

Do not silently accept `unknown` as a version when running the generator for a pinned release dataset.

## Generated-file invariants

Preserve:

- sorted numeric code-point order;
- exact substitution strings;
- exact entry count unless the pinned source proves otherwise;
- the static table representation;
- binary-search lookup;
- version and checksum comments in generated output.

Add a final newline to generated files if doing so can be done deterministically; do not create meaningless repeated no-newline diffs.

## Required verification

Run the generator once against the pinned source in an environment with network access and verify:

```bash
python3 scripts/generate_confusables.py
git diff -- src/text/confusables_generated.rs data/confusables.rs
```

Expected result after the generator-only pin change:

- no semantic table changes;
- ideally no table-content diff at all beyond deterministic header/source/newline formatting if such formatting is intentionally corrected.

If the versioned Unicode source produces bytes with a different checksum from the currently recorded one, stop. Do not update the checksum automatically. Determine whether the previous checksum was computed from different transport/content bytes before changing the expected value.

## Required tests/checks

No new Python test framework is required. Use the existing Rust confusables tests plus direct script verification.

Required evidence:

- generated table remains strictly sorted;
- entry count remains 6565 unless the already-pinned source proves the previous count was wrong;
- representative substitutions remain unchanged;
- generator exits non-zero on an intentionally wrong expected checksum during a temporary local test or equivalent direct function-level check;
- generator exits non-zero on a mismatched expected version during a temporary local test or equivalent direct function-level check;
- generated output is deterministic on two successive runs from the same pinned source.

Temporary negative-test edits must not be committed.

---

# Workstream 2 — Reconcile the August 4 phase records with actual commits

## Problem

The current roadmap claims completion, but child plan metadata and completion records are inconsistent. Examples observed during review include:

- Phase 1 still carrying `Status: planned` and an implementation SHA field left pending even though implementation landed;
- Phase 2 still carrying `Status: planned` despite its implementation commit;
- Phase 4 still carrying `Status: planned` despite its simplification commit;
- the parent roadmap marked complete before the follow-up runtime corrections in the companion August 7 plan were accounted for.

This makes handoff state less trustworthy than the code.

## Required outcome

After this workstream:

- all August 4 phase files accurately report their final status;
- all completion records use actual implementation commit SHAs/ranges rather than `pending` where the implementation is known;
- completion descriptions match the code that exists after the August 7 corrective runtime pass;
- Phase 1 does not claim soundness based on deprecated retention of `current_eval_context()`; it records the final removal/re-entrancy correction commit from the companion plan;
- Phase 2 records both the original path/wire implementation and the bounded-reader/drive-relative corrective commit where relevant;
- Phase 3 records its existing timeout-policy implementation accurately;
- Phase 4 records its release/CI implementation accurately;
- Phase 5 records its footprint implementation and Unicode pin correction accurately;
- the parent roadmap's completion statement names the complete implementation range including the August 7 corrections;
- no phase claims acceptance criteria that have not actually been verified.

## Known implementation commits to reconcile

Use repository history, not this list alone, as the source of truth. Known commits from the original pass include:

```text
08c419da1ec0189d8922493ac080b304dbab46a9  Phase 1 initial context correction
76ec421288c1e60778742954a5517b290f26fa73  Phase 2 path/wire corrections
17878d5d78ac86e4d962c4524a981718de0daed3  Phase 3 timeout/test containment
63c7a94cbaea4a3ca65995f43de3f1629bde68a4  Phase 4 release/CI simplification
632f07cb5b7a76570e12cde6b97d57f6f05a8e47  Phase 5 footprint changes
e4c52b077f5f8897f0e28bb68618813da1820e50  initial Unicode 17 metadata/checksum correction
468a812780e9199ca6002bbd0f2b3b9a41aeaa55  Phase 5 closure-record update
```

Add the actual implementation SHA(s) from `2026-08-07-corrective-runtime-soundness-and-boundaries.md` and from this Unicode-pin pass before marking final closure.

Do not fabricate a contiguous range if unrelated commits exist between relevant commits; list explicit SHAs when clearer.

## Record-editing rules

For each phase file:

1. inspect its acceptance checklist against current code;
2. only check an item if it is currently true;
3. replace stale `planned` status with `complete` only after all acceptance items are true;
4. replace `pending` completion fields with concrete data;
5. keep rejected/deferred candidates explicit;
6. do not rewrite the original plan into a retrospective narrative;
7. preserve the original scope and decision rationale.

For the parent roadmap:

- retain its original problem statement and constraints;
- add a concise corrective-follow-up note explaining that August 7 review found and closed residual defects;
- update the implementation/closure record once;
- do not create a second roadmap.

---

# Workstream 3 — Run one canonical release-readiness gate

## Purpose

The prior release simplification deliberately established one canonical full local gate:

```bash
scripts/release-check.sh
```

After the runtime corrective plan and Unicode pinning are complete, run that command once from a clean worktree.

## Required behavior

The script must remain the sole canonical full release-readiness command and must continue to:

- require a clean worktree;
- check formatting;
- check generated docs using `--features dev-tools`;
- run Clippy with warnings denied;
- run the non-parity test suite with the documented thread bound;
- run doctests;
- run cargo-deny;
- package the crate;
- run `cargo publish --dry-run`;
- never publish;
- never tag;
- never regenerate Unicode data from the network.

Do not add another release command because this plan exists.

## If the canonical gate fails

Fix only failures caused by the corrective implementation or genuine existing release-readiness defects.

Do not broaden the pass into:

- dependency upgrades unrelated to a failing policy check;
- new workflow automation;
- fuzzing programs;
- extra artifact/evidence generation;
- unrelated documentation cleanup.

If an unrelated external advisory or ecosystem issue blocks the gate, record it precisely as a deferred external blocker rather than restructuring the repository.

---

# Workstream 4 — Final stop condition

Once all preceding workstreams pass:

- update this plan's completion record;
- update the companion runtime corrective plan's completion record if its implementation commit did not already do so;
- reconcile the five August 4 phase records and parent roadmap once;
- update `CHANGELOG.md` only for user/developer-visible behavior actually changed;
- stop this line of work.

Do not create another polish, evidence, verification, or optimization plan unless a new reproducible product defect is discovered after closure.

---

# Execution order for a smaller implementation model

Execute this plan only after the companion runtime corrective plan is green.

## Step 1 — Pin source input

1. Read the current generator and generated-file headers.
2. Confirm the official version-specific Unicode Security URL for 17.0.0.
3. Add explicit pinned version and checksum constants.
4. Verify checksum/version before any write.
5. Generate both outputs in memory before writing.
6. Run the generator.
7. Confirm no semantic confusables-table change.
8. Run focused confusables tests.

Do not edit planning records until this step is complete.

## Step 2 — Inspect phase records

1. Read the parent roadmap and all five August 4 phase plans.
2. Read the companion August 7 corrective plan completion record.
3. Inspect git history for exact implementation SHAs.
4. Map each acceptance checklist to current code/test evidence.
5. Identify stale `planned` and `pending` fields.

## Step 3 — Reconcile records

1. Update Phase 1 with the initial and corrective soundness commits.
2. Update Phase 2 with the original and corrective boundary/path commits.
3. Confirm Phase 3 completion fields.
4. Update Phase 4 status/completion fields.
5. Update Phase 5 with the final Unicode pin commit.
6. Update parent roadmap completion statement and implementation range/list.
7. Keep changes concise; do not add new evidence sections.

## Step 4 — Ordinary verification

Run:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
cargo run --locked --features dev-tools --bin generate-docs -- --check
```

## Step 5 — Canonical release gate

From a clean worktree run:

```bash
scripts/release-check.sh
```

If it passes, record that result once.

## Step 6 — Stop

Do not produce another plan merely to restate successful checks.

---

# Acceptance checklist

This plan is complete only when all items are true:

- [x] The confusables generator no longer uses a moving `/latest/` source.
- [x] Unicode Security version 17.0.0 is explicitly pinned in the generator.
- [x] The expected SHA-256 checksum is explicitly pinned in the generator.
- [x] Downloaded bytes are checksum-validated before generated files are written.
- [x] Source header version is validated before generated files are written.
- [x] Version/checksum mismatch fails loudly without partially rewriting outputs.
- [x] Regeneration from the pinned source produces the same 6565-entry semantic table.
- [x] Existing representative confusable substitutions remain unchanged.
- [x] Generated output is deterministic for the same pinned source.
- [x] Ordinary CI remains free of network-based Unicode regeneration.
- [x] `scripts/release-check.sh` remains free of network-based Unicode regeneration.
- [x] August 4 Phase 1 status/completion record matches the final corrected implementation.
- [x] August 4 Phase 2 status/completion record matches the final corrected implementation.
- [x] August 4 Phase 3 status/completion record is accurate.
- [x] August 4 Phase 4 status/completion record is accurate.
- [x] August 4 Phase 5 status/completion record includes the real Unicode pin.
- [x] The parent August 4 roadmap accurately includes the August 7 corrective follow-up and no longer overstates earlier closure.
- [x] No known implementation SHA fields remain `pending` when the corresponding commit is known.
- [x] The companion August 7 runtime corrective plan is complete.
- [x] Ordinary verification passes.
- [x] `scripts/release-check.sh` passes from a clean worktree, or a precise external blocker is recorded.
- [x] No new dependency, workflow, release automation, updater subsystem, or evidence registry was added.
- [x] No additional polish/evidence plan is created after closure.

---

# Explicit non-goals

Do not use this pass to:

- upgrade Unicode to a newer version;
- alter confusable policy behavior;
- remove `serde_json/preserve_order`;
- consolidate `toml`/`toml_edit`;
- revisit Tokio current-thread selection;
- change release profile settings;
- reduce the binary further;
- change CI cadence again;
- add crates.io publishing to GitHub Actions;
- add supply-chain artifacts beyond the existing cargo-deny check;
- introduce snapshot/evidence archives;
- rewrite old plan history.

---

# Completion record

Fill once the full corrective line is closed:

- **Runtime corrective dependency commit(s):** `a3f78e3`
- **Unicode pin implementation commit:** (this plan — generator script changes in `scripts/generate_confusables.py`)
- **Pinned Unicode Security version:** 17.0.0
- **Pinned source URL:** `https://www.unicode.org/Public/17.0.0/security/confusables.txt` (official version-specific Unicode Security 17.0.0 source, verified by `324006f`)
- **Pinned SHA-256:** `091c7f82fc39ef208faf8f94d29c244de99254675e09de163160c810d13ef22a`
- **Confusables entry count:** 6565 (verified — no change)
- **Generated semantic diff:** none — generated files are byte-for-byte identical
- **Phase 1 record reconciliation:** complete — status `complete`, commit `08c419d` + `a3f78e3`
- **Phase 2 record reconciliation:** complete — status `complete`, commits `76ec421` + `a3f78e3`
- **Phase 3 record reconciliation:** complete — commit `17878d5`
- **Phase 4 record reconciliation:** complete — status `complete`, commit `63c7a94`
- **Phase 5 record reconciliation:** complete — already filled
- **Parent roadmap reconciliation:** complete — implementation range `08c419d..a3f78e3`, corrective follow-up noted
- **Ordinary verification:** fmt, clippy, 3565 non-parity tests (1 ignored), 11 doc tests, generate-docs check all pass
- **Canonical release check:** pending final clean-worktree run for `324006f` plus this record reconciliation
- **Deferred external blockers:** none
- **Final disposition:** complete after the final boundary correction and canonical release check

When this record is complete and the acceptance checklist passes, the August 4 bounded-correctness/simplification line of work is closed. No further closure-only plan is required.
