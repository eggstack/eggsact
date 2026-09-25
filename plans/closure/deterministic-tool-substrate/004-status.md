# Deterministic Tool Substrate Milestone 004 — Closure Status

Status: closed

Source implementation plan:

- `plans/implementation/deterministic-tool-substrate/004-unicode18-security-data-qualification.md`

Source subsystem roadmap:

- `plans/subsystems/deterministic-tool-substrate-roadmap.md#7` (Milestone 4)

Repository baseline reviewed: `24073e9` (Milestone 003 closure; the plan's
nominal `43971e7` baseline was superseded by the closed 003 work, which is
the required hard dependency)

Implementation commits or pull requests:

- `2e3860c` — Unicode 18 security data qualification (pin advance,
  regenerated assets, epoch-specific fixtures/seeds, provenance and docs)

Milestone 003 closure reference:

- `plans/closure/deterministic-tool-substrate/003-status.md`
  (implementation `3d67807`, closure `24073e9`)

## 1. Executive finding

The checked-in UTS #39 confusables/security data advanced from Unicode
17.0.0 to authoritative Unicode 18.0.0 bytes through the hardened
Milestone 003 generator trust path, with no algorithm, ToolSpec, profile,
audience, protocol, or dependency change. The post-003 data graph was
inventoried first: no newer release exists for the lagging
security-semantic providers (normalization 17.0, casefold 16.0,
general-category 16.0), so a fully U18-aligned pipeline is unsupportable
without unreleased git revisions — correctly out of scope. The shipped
claim is therefore exactly "confusables data: Unicode 18.0.0", with every
other provider epoch documented. All Milestone 003 regression suites pass
unmodified against the new data; version-driven fixture updates were
mechanical (exact count, one representative mapping, corpus seeds); new
regression fixtures cover materially changed U18 mappings; parity shows
zero regressions; the ordered merge gate and `cargo-deny` are green.

## 2. Requirement-to-evidence matrix

| Requirement (work package) | Evidence | Result | Notes |
|---|---|---|---|
| A: inventory direct/transitive epochs from authoritative metadata | provider table in `architecture/generated-assets.md` | pass | Crate sources, UCD headers, upstream history, crates.io API, all dated 2026-09-25 |
| A: classify security-semantic / diagnostic-only / presentation-only | same table, Role column | pass | Skeleton mapping + NFD + casefold + XID + script = security-semantic; category = support; names/segmentation = diagnostic |
| A: determine whether coherent U18 claim is supportable | WP-A finding below | pass | Not supportable (no newer provider releases); confusables-only U18 claim adopted per plan §9 |
| B: pin to authoritative 18.0.0 URL/version/checksum | `scripts/generate_confusables.py` pins + `--check` output | pass | `https://www.unicode.org/Public/18.0.0/security/confusables.txt`, version `18.0.0`, SHA-256 `6ed3ee96…f5b92` |
| B: fetch once as maintainer action, verify exact bytes | curl fetch 763,128 B + checksum verification | pass | 2026-09-25; header `# Version: 18.0.0` confirmed |
| B: regenerate through hardened parser/generator | `2e3860c`, 6712 entries, strict parse with zero failures | pass | Real-world proof the 003 strictness has no false rejections |
| B: review generated diff | §3 change taxonomy (added/removed/changed with examples) | pass | Upstream overhaul, classified below |
| B: exact-count + representative fixtures from generated result | `CONFUSABLES_ENTRY_COUNT = 6712`, count test, supplementary-plane mapping `U+2C09B → U+5341` | pass | From generated output, not assumption |
| B: generated outputs match `--check` | `--check` run 2026-09-25 | pass | Checksum + version verified, both files reproduce exactly |
| C: full 003 suite against new data | focused text 1009 + property 66 + full gate, all green | pass | Zero algorithmic changes required |
| C: targeted fixtures for changed mappings | 5 new U18 tests in `tests/text/test_confusables.rs` | pass | Added ¡/º, changed %, removed Ç, stable core spoof set |
| C: skeleton determinism/idempotence + grouping | unchanged 003 tests pass under U18 | pass | `apple`/`аpple`, `Æ`/`AE`, `АX`/`ΑY` all stable |
| C: 17 vs 18 output comparison + classification | §3 taxonomy + Ç test (mapping gone, skeleton equal) | pass | Removals are NFD-redundant; additions/changed are genuine |
| C: mixed-script + bidi unaffected | unchanged 003 suites green | pass | Independent of confusables data by construction |
| D: architecture/library/fuzzing docs | `2e3860c` doc updates | pass | Epoch inventory, mixed-epoch warning, 6712 counts |
| D: CHANGELOG data-refresh entry | `CHANGELOG.md` Unreleased section | pass | Wording: "confusables data: Unicode 18.0.0" |
| D: provenance constants | `CONFUSABLES_UNICODE_VERSION = "18.0.0"` etc. + unit tests | pass | Single source of truth, header + length asserts |
| D: parity without correctness weakening | 381 passed, 0 failed | pass | No accepted-failure change |
| D: deny/MSRV/size review | `cargo deny check` green; no version bumps; MSRV untouched; size identical | pass | Release 11,644,656 B before and after (same host/toolchain) |
| D: merge gate + remote CI | ordered gate green locally; no remote run from this environment | pass | Same posture as 003 closure |

## 3. Production implementation evidence

WP-A provider inventory (authoritative sources, 2026-09-25):

- Confusables: generated table, **18.0.0** (this milestone) —
  security-semantic.
- Normalization: `unicode-normalization` 0.1.25, **17.0** (upstream "Update
  Unicode to version 17.0.0" Sep 2025; the U18 master update is unreleased,
  and 0.1.25 is the newest crates.io release) — security-semantic (skeleton
  NFD). Canonical mappings are stability-guaranteed, so the skew cannot
  alter existing skeletons.
- Casefold: `caseless` 0.2.2 (`UNICODE_VERSION = (16, 0, 0)` in vendored
  source), **16.0**, no newer release — security-semantic.
- General category: `unicode-general-category` 1.1.0 (Unicode 16.0 badge in
  vendored README; 1.1.0 is the newest crates.io release), **16.0**, no
  newer release — support role.
- Names: `unicode_names2` 3.1.0, **17.0** — diagnostic-only.
- Segmentation: `unicode-segmentation` 1.13.3, **17.0** — diagnostic.
- XID: `unicode-ident` 1.0.26 (`UNICODE_VERSION = (18, 0, 0)` in vendored
  source), **18.0** — security-semantic (already current).
- Script: `src/text/script.rs` hand table, **17.0-shaped**; U18-new
  assignments surface as `Other`/filtered (safe direction, documented in
  the module header).

WP-B data refresh: pin advanced to the authoritative 18.0.0 bytes
(763,128 B download, header `# Version: 18.0.0`, SHA-256
`6ed3ee967c9dfdf6677d563c9985182fbc50a2efb7d6059cd57b2e2ce18f5b92`);
regenerated 6,712 entries (was 6,565; net +147).

WP-B/C diff taxonomy (17.0.0 → 18.0.0, computed from both sources):

- 1,216 added (e.g. `U+00A1 ¡ → U+0069`, `U+00BA º → U+00B0`) — genuine new
  coverage, fixtured.
- 1,069 removed (e.g. `U+00C7 Ç → U+0043 U+0326`, `U+2FA1D → U+2A600`) —
  upstream cleanup of NFD-redundant entries; skeleton equality preserved by
  normalization (proven by the Ç test: mapping gone, `are_confusable`
  unchanged).
- 241 changed (e.g. `U+0025 %`: `U+00BA… → U+00B0…`; Devanagari/CJK
  retargets) — genuine versioned changes, sampled in fixtures.
- Core 003 acceptance mappings stable (`0410→0041`, `0430→0061`,
  `0391→0041`, `00C6→0041 0045` all present in the 18.0.0 source).

No core/service/adapter logic changed; no registry, profile, audience,
surface, protocol, DTO, runtime, or operator-surface change. Provenance
stays in library constants + docs (no diagnostics wire change, per the
plan's allowed alternative). No dependency was bumped.

## 4. Verification executed

### Commands run

```bash
cargo fmt --all -- --check
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --test lib text
cargo test --locked --test lib property
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
cargo tree --locked -e normal
python3 scripts/generate_confusables.py --self-test
python3 scripts/generate_confusables.py --check
cargo build --locked && cargo test --locked --test lib parity
cargo deny --locked check
```

Plus the maintainer fetch + byte verification and the 17-vs-18 mapping
taxonomy script (§3), and a same-host release-size comparison.

### Results

- `cargo fmt --check`: pass. `generate-docs --check`: pass (no surface
  drift). `clippy -D warnings`: pass, zero new lints.
- Focused `text`: 1009 passed (1004 carried + 5 new U18 fixtures).
- Focused `property`: 66 passed (seeds refreshed to U18-valid mappings).
- Full suite (`--skip parity`, `--test-threads=4`): 664 + 40 + 14 + 3027
  (integration) + 51 + 11 — all pass, 0 failed on the confirmation run.
  One earlier full run showed a single unrelated
  `test_era_pinning` protocol-era failure that passes in isolation and in
  the 18-test module run, and did not reproduce on confirmation;
  classified low/flaky in §10.
- Doc tests: 11 passed.
- `cargo tree`: dependency graph unchanged (no new/changed packages).
- Generator `--self-test`: pass (offline). Generator `--check`: pass —
  checksum + version verified against unicode.org, both checked-in outputs
  reproduce exactly at 6712 entries.
- Parity (`../eggcalc` available): 381 passed, 0 failed, 40 ignored — zero
  regressions; intentional data differences: none observed in the covered
  cases (core spoof/policy fixtures agree across implementations).
- `cargo deny check`: advisories / bans / licenses / sources ok (graph
  unchanged).
- MSRV: no new code beyond constant values and tests; `rust-version`
  1.89.0 untouched.
- Release size (same host/toolchain): `2e3860c` 11,644,656 B — identical
  to the post-003 build. Data-only delta of +147 entries has no material
  footprint effect.
- Remote CI: not run from this environment; local ordered-gate evidence
  above (same posture as the 003 closure).

## 5. Invariant review

- 003 behavior and regression tests remain green: holds (1009 focused
  tests incl. all 003 acceptance fixtures).
- One crate, MSRV 1.89.0, tracked lockfile (untouched by this milestone),
  deterministic local runtime: holds.
- No runtime or ordinary-build Unicode download: holds (generator-only
  network, maintainer-invoked).
- Version + checksum pins verified before generation: holds (`--check`
  output).
- Generated outputs never hand-edited: holds (byte-identical reproduction
  proven).
- ToolSpecs, schemas, profile/audience, registry order, machine codes:
  unchanged (`generate-docs --check` + registry-sync tests).
- 1.x per-character lookup APIs source-compatible: holds (only table
  contents advanced).
- Provenance explicit, no collapsed "Unicode version": holds (per-provider
  inventory + "confusables data: Unicode 18.0.0" wording).

## 6. Failure and recovery review

- Authoritative source unavailable: stop and retain the verified source
  (not triggered — unicode.org reachable, checksum matched).
- Checksum/version mismatch: fail-closed, zero writes (exercised
  continuously; 17.0.0 re-fetch during analysis re-verified the old pin
  byte-for-byte).
- Malformed 18.0.0 rows: strict parser accepted the full 6,712-row file
  with no rejections — evidence the 003 strictness has no false positives
  on real data.
- Runtime bounds/cancellation/concurrency: unchanged from 003 (static data
  refresh, no request-time loading, no new state).

## 7. Migration and compatibility review

- Data-version update, not a surface migration: APIs source-compatible.
- Inputs gaining findings (1,216 new mappings, e.g. `¡`/`º`) and losing
  per-character findings (1,069 removed, e.g. `Ç` as a source) are expected
  versioned correctness changes, documented in CHANGELOG and fixtured.
- No stale mapping preserved for output parity; parity shows zero
  failures, so no parity fixture needed version-provenance classification.
- Accepted-failure baseline untouched.
- Claim discipline: "confusables data: Unicode 18.0.0" used consistently
  across code constants, architecture docs, library docs, and CHANGELOG.

## 8. Determinism and bounded-execution review

- Unchanged from 003: identical inputs give identical outputs within the
  new data epoch; skeleton idempotence re-verified under U18 data.
- No clock/TZ/env/net in runtime paths; limits and `limits_applied`
  behavior unchanged.

## 9. Documentation and operations

- `architecture/generated-assets.md`: 18.0.0 source/checksum workflow plus
  the full provider epoch inventory table.
- `architecture/text-library.md`: 18.0.0 URL, 6,712 entries, mixed-epoch
  warning block.
- `architecture/overview.md`: 26 modules, confusables-data epoch pointer.
- `docs/library-api.md`: provenance constant version.
- `docs/fuzzing.md`: unchanged (invariants already correct; seeds
  refreshed in code, not docs).
- `CHANGELOG.md`: data-refresh entry with checksum, counts, taxonomy, and
  claim wording.
- `src/text/script.rs` header: U18 epoch note for the hand table.
- Static guards: exact-count (6712) and header-version asserts updated;
  `--check` freshness guard exercised against the network.

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| low | One full-suite run showed a single `test_era_pinning` failure; passes in isolation (1/1), in module (18/18), and on the confirmation full run (3027/3027) | No evidence of a product defect; protocol-era logic untouched by this milestone | None for 004; if it recurs, file a testing workstream flake report |
| low | Mixed provider epochs by design (casefold 16.0, category 16.0, normalization 17.0 vs confusables 18.0) | Documented, stability-bounded; no newer provider releases exist | Revisit on future upstream releases via a corrective data pass |
| low | Remote CI run not recorded (local-only environment) | Same posture as 003 closure | Maintainer CI on push covers |
| low | `script.rs` ranges remain 17.0-shaped | U18-new assignments filter as `Other` (safe direction) | Regenerate only if a real case needs an unlisted script |

No critical, high, or medium findings remain.

## 11. Roadmap disposition

Milestone 004 closed. The deterministic-tool-substrate Unicode workstream
(003 semantic hardening + 004 data qualification) is complete; the
substrate returns to guard status. No follow-up milestone is required
unless upstream providers release newer security-semantic data.

## 12. Registry updates

- `plans/registry.md`: 004 → closed with this closure record; substrate
  current milestone returns to guard-only.
- `plans/subsystems/deterministic-tool-substrate-roadmap.md`: 004 closed
  with closure link; roadmap status reflects completed Unicode workstream.
- `plans/implementation/deterministic-tool-substrate/004-*.md`: status
  header → `closed` (implemented in `2e3860c`).
