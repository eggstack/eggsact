# Deterministic Tool Substrate Milestone 005 — Closure Status

Status: closed

Source implementation plan:

- `plans/implementation/deterministic-tool-substrate/005-unicode-security-standards-conformance-corrective.md`

Source subsystem roadmap:

- `plans/subsystems/deterministic-tool-substrate-roadmap.md#7` (Milestone 5)

Repository baseline reviewed: `b7fcc004dab0b2550eae3ffd31a1aa99fbdbb276`
(plan baseline; the working tree was clean at handoff)

Implementation commits or pull requests:

- `cb0d743` — Unicode security standards-conformance corrective (work
  packages A–F: generated Unicode 18 property tables, internal/bidi/public
  skeleton, resolved Script_Extensions, Rust inspect parity, hazard
  consolidation, conformance/property/fuzz tests, documentation)
- `1f0816e` — Close Milestone 005 (this closure record, plan status,
  registry/roadmap reconciliation)

Milestone 003/004 closure references (immutable history):

- `plans/closure/deterministic-tool-substrate/003-status.md`
  (implementation `3d67807`, closure `24073e9`)
- `plans/closure/deterministic-tool-substrate/004-status.md`
  (implementation `2e3860c`, closure `b7fcc00`)

## 1. Executive finding

The Unicode/security substrate now implements the semantics it claims. The
public `confusable_skeleton()` is UTS #39 Revision 34
`skeleton(X) = bidiSkeleton(LTR, X)` over Unicode 18.0.0 (internal stage with
Default_Ignorable removal plus the LTR bidi path with L3/L4 handling);
mixed-script detection is the normative Script_Extensions resolved-set rule
with Unicode 18 Jpan/Kore/Hanb/Hntl augmentation rather than range-table plus
allowlist approximation; `identifier_inspect(language = "rust")` validates via
the shared XID+keyword helper; and security-sensitive invisible/bidi/join
membership has one typed source of truth. The Unicode 18 confusables
source/checksum/entry count are unchanged, and no ToolSpec, profile,
audience, surface, schema, or machine-code change occurred. The ordered merge
gate, focused/property suites, parity (381/0), `cargo-deny`, both generator
`--self-test`/`--check` paths, and local release-size review are green. No
critical, high, or medium findings remain; two low findings (provider-epoch
skew documentation, remote-CI posture) are recorded in §10.

## 2. Requirement-to-evidence matrix

| Requirement (work package) | Evidence | Result | Notes |
|---|---|---|---|
| A: Default-Ignorable skeleton fixtures (VS, join/format) | `tests/text/test_unicode_conformance_005.rs::variation_selector_collapses_in_skeleton`, `::zero_width_and_join_format_collapse_in_skeleton`, `::internal_skeleton_removes_ignorables_before_mapping` | pass | `a\u{FE0F}b` ≅ `ab` etc.; baseline kept ignorables (false negative) |
| A: bidi-skeleton fixtures (RTL + mirrored) | `::bidi_spec_vectors_are_ltr_confusable` (UTS #39 §4 S1/S2), `::bidi_fast_path_preserves_ltr_skeletons`, `::mirrored_bracket_collapses_in_rtl_context` | pass | S1/S2 share `bidiSkeleton(LTR)`; fast path proven for LTR |
| A: mixed-script matrix (Latin/Cyrillic/Common/Japanese/Korean/Hntl) | `::table_1a_single_script_cases`, `::table_1a_spoof_mixture`, `::japanese_resolves_through_jpan`, `::korean_distinguishes_mixed_from_restriction`, `::hntl_hanb_augmentation`, `::common_inherited_never_mix_alone` | pass | Covers Table 1a rows 1–4,7,8 plus Hntl/Hanb/Kore and Common/Inherited neutrality |
| A: Rust `identifier_inspect` valid/invalid matrix | `::rust_inspect_rejects_invalid_accepts_xid` | pass | `1abc`/`fn` rejected, `café`/`αβγ` accepted; baseline accepted all via fallthrough |
| B: qualify candidate crate epochs first | `architecture/generated-assets.md` dependency-qualification table; §3 below | pass | unicode-security 16.0 rejected; unicode-script 17.0 rejected; unicode-bidi-mirroring 16 rejected; unicode-bidi 16.0 hardcoded rejected (algorithm adopted with custom 18.0 source) |
| B: generate only missing Unicode 18 tables from pinned UCD | `scripts/generate_unicode_security_properties.py` + `src/text/unicode_properties_generated.rs` (27 DI ranges, 2321 Script, 210 Scx, 2356 Bidi_Class, 438 mirroring, 130 brackets) | pass | 7 pinned sources with SHA-256; byte-for-byte `--check` green |
| B: strict validation + offline self-tests | generator `--self-test` (19 miniature fixtures: duplicates, malformed, surrogates, inverted ranges, unknown classes/scripts) | pass | Fail-closed, zero writes on failure |
| B: maintainer `--check` freshness + no build-time network | `--check` green for both generators; ordinary builds/tests offline | pass | CI never fetches unicode.org |
| C: internal skeleton with Default-Ignorable removal | `src/text/confusables.rs::internal_skeleton` + unit/property/conformance tests | pass | NFD → remove DI → map → NFD; `internal(internal(X)) = internal(X)` asserted |
| C: UTS #39 Rev 34 bidiSkeleton(LTR) with version-correct data | `::bidi_skeleton_ltr` (unicode-bidi L1/L2 via `Unicode18BidiData` + L3 bubble fixup + L4 mirroring) + S1/S2 vectors | pass | Fast path (no R/AL → internal) per spec |
| C: `are_confusable` consumes corrected public skeleton | unchanged distinct-raw-strings logic over new skeletons; `apple`/`аpple`, `Æ`/`AE`, `АX`/`ΑY` stable | pass | Existing suites green unmodified |
| C: no retained old fast path with different semantics | old NFD+map+NFD body replaced; only spec fast path remains | pass | Clippy + review |
| D: Script_Extensions lookup + Unicode 18 augmentation | `src/text/unicode_properties.rs::augmented_script_set` (Hani→Hanb/Hntl/Jpan/Kore, Hira/Kana→Jpan, Hang→Kore, Bopo→Hanb, Latn→Hntl) + unit tests | pass | Exact §5.1 rules |
| D: replace `is_legitimate_mixture` in security decisions | `unicode_tools`, `unicode_policy`, `identifier` verdicts now call `is_mixed_script`; `is_legitimate_mixture` retained as legacy compat (documented non-authoritative) | pass | `grep` shows no production verdict use |
| D: diagnostic lists preserved, booleans from resolved sets | `scripts`/`positions` DTO shapes unchanged; `mixed_scripts` boolean from resolved sets | pass | `generate-docs --check` + registry-sync green |
| D: restriction-friendly ≠ single-script demonstrated | `한한자test` (Hangul+Han+Latin) mixed; `한한자` single via Kore; `漢A` single via Hntl | pass | Hardening matrix updated with rationale |
| E: Rust branch via shared XID/keyword helper | `rust_identifier_is_valid()` used by both `identifier_analyze` and `identifier_inspect` | pass | `is_valid_rust_identifier_ascii` kept as stricter helper |
| E: remove parallel invisible/bidi/join lists | policy `find_invisibles` + identifier `has_invisibles` → `has_security_invisible_hazard`; policy zero-width → `is_zero_width_char`; `helpers::is_invisible_char` → `unicode_tools::is_invisible_char` → `classify_hazard`; `INVISIBLE_CHARS`/`ZERO_WIDTH_CHARS`/policy HashSet deleted | pass | Cross-consumer differential tests green |
| E: Default-Ignorable modeled separately from hazard category | skeleton uses `is_default_ignorable`; hazards use `classify_hazard`; U+034F reclassified InvisibleFormat (was generic CombiningMark) with rationale | pass | Documented in code + closure §3 |
| F: docs + CHANGELOG + closure reconciliation (003/004 immutable) | `architecture/text-library.md`, `architecture/generated-assets.md`, `architecture/overview.md`, `docs/library-api.md`, `docs/fuzzing.md`, `docs/contributing.md`, skill catalog, `CHANGELOG.md`, this record; 003/004 files untouched | pass | `git status` confirms 003/004 closure files unmodified |
| F: roadmap/registry guard-only after 005 closes | `plans/registry.md` 005 → closed, substrate guard-only; roadmap §12 + status header updated | pass | §12 below |

## 3. Production implementation evidence

WP-B provider inventory (authoritative crate metadata + UCD headers):

- Confusables: generated table, **18.0.0** (unchanged from 004) — security-semantic.
- Default_Ignorable / Script / Script_Extensions / Bidi_Class / mirroring /
  brackets: generated Unicode **18.0.0** UCD tables (this milestone) —
  security-semantic. Sources + SHA-256 in the generated header and
  `unicode_properties.rs` provenance constants.
- UAX #9 algorithm: `unicode-bidi` 0.3.18 (`UNICODE_VERSION = (16, 0, 0)`
  hardcoded, MSRV 1.47, MIT/Apache-2.0) — algorithm adopted, data rejected.
  Built with `default-features = false` (no `hardcoded-data`); every call is
  `*_with_data_source(&Unicode18BidiData, …)` so Unicode 18 generated tables
  drive all security semantics. Zero transitive dependencies at this feature
  set (`cargo tree -p unicode-bidi` shows the crate alone).
- Rejected without adoption: `unicode-security` 0.1.2 (16.0.0),
  `unicode-script` 0.5.8 (17.0.0), `unicode-bidi-mirroring` 0.4.0
  (Unicode 16) — documented in `architecture/generated-assets.md`.
- Normalization 17.0 (`unicode-normalization` 0.1.25), casefold 16.0
  (`caseless` 0.2.2), general-category 16.0 (`unicode-general-category`
  1.1.0), names/segmentation 17.0, XID 18.0 (`unicode-ident` 1.0.26):
  unchanged; canonical mappings are stability-guaranteed so the skew cannot
  alter skeletons.

WP-C skeleton (`src/text/confusables.rs`):

- `internal_skeleton()`: NFD → strip `Default_Ignorable_Code_Point` → per-code-point
  `confusables.txt` prototype mapping → NFD (additive public helper).
- `bidi_skeleton_ltr()`: spec fast path (no R/AL → internal); else
  `BidiInfo::new_with_data_source` LTR paragraph processing (L1/L2) per
  paragraph with separators preserved → L3 combining-mark bubble fixup →
  L4 mirroring (`bidi_mirror` iff resolved level RTL) → `internal_skeleton`.
- `confusable_skeleton()` = `bidi_skeleton_ltr()` (source-compatible,
  semantics corrected); `are_confusable()` compares corrected skeletons with
  the kept distinct-raw-strings rule.

WP-D resolved scripts (`src/text/unicode_properties.rs` +
`src/text/script.rs` compat layer):

- `script_extensions()` (override else single Script; unlisted → `Zzzz`),
  `augmented_script_set()` (exact six §5.1 rules; `Zyyy`/`Zinh`/`Zzzz` →
  ALL/empty marker), `resolved_script_set_detailed()` (intersection; ALL
  skips), `is_mixed_script()` (empty + script-bearing).
- `script.rs` keeps diagnostic spellings (`script_of` long names, `Other`
  for unassigned, `policy_script_of` → `Unknown`) while verdicts use the
  resolved sets. `unicode_tools::detect_mixed_scripts`,
  `unicode_policy::detect_mixed_scripts`, and identifier mixed warnings now
  consume `is_mixed_script`. `unicode_scripts("☃")` correctly reports
  `Common` (was `Other` under the hand table).

WP-E validation/ownership (`identifier.rs`, `unicode_tools.rs`,
`unicode_policy.rs`, `tools/helpers.rs`):

- `rust_identifier_is_valid()` (XID syntax + keyword) shared by analyze and
  inspect; `identifier_inspect` gains the `rust` branch.
- `has_security_invisible_hazard()` (Bidi/Join/InvisibleFormat/VariationSelector)
  and `is_zero_width_char()` centralize verdicts; `is_invisible_char()`
  delegates to the classifier; U+034F fixed to `InvisibleFormat`.
- Deleted parallel lists: identifier `INVISIBLE_CHARS`, policy invisible
  `HashSet` + `ZERO_WIDTH_CHARS`, helpers duplicate matcher.

No registry, profile, audience, surface, protocol, DTO-schema, CLI, or
machine-code change (`generate-docs --check` clean; 86 tools / 23 categories
unchanged).

## 4. Verification executed

### Commands run

```bash
cargo fmt --all -- --check
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
cargo test --locked --test lib text
cargo test --locked --test lib property
cargo test --locked --test lib test_unicode_conformance_005
cargo build --locked && cargo test --locked --test lib parity
cargo deny --locked check
cargo tree --locked -e normal
python3 scripts/generate_confusables.py --self-test
python3 scripts/generate_confusables.py --check
python3 scripts/generate_unicode_security_properties.py --self-test
python3 scripts/generate_unicode_security_properties.py --check
cargo build --locked --release  # size evidence
```

### Results

- `cargo fmt --check`: pass.
- `generate-docs --check`: pass (no ToolSpec/profile/exposure drift).
- `clippy -D warnings`: pass (3 new-code lints fixed during implementation:
  2 redundant closures, 1 saturating-sub; 2 test-only bool-assert/single-loop
  lints fixed; zero remaining).
- Full suite (`--skip parity`, `--test-threads=4`): lib 673 + integration
  3044 + context-isolation 51 + doc 11 + misc 40/14 — all pass, 0 failed.
- Focused `text`: 1007+14 conformance passing (2 pre-existing tests updated
  for corrected semantics with rationale: Korean+Latin matrix,
  snowman Common).
- Focused `property`: 66 carried + 3 new invariants (internal/bidi
  determinism, resolved determinism + Common/Inherited neutrality, hazard
  agreement) — all pass.
- New conformance file: 14/14 pass, including UTS #39 §4 S1/S2 LTR vectors,
  variation-selector/zero-width collapse, Table 1a matrix, Hntl/Hanb/Jpan/Kore
  augmentation, Rust inspect matrix, cross-consumer hazards.
- Parity (`../eggcalc` available): 381 passed, 0 failed, 40 ignored — zero
  regressions; no accepted-failure baseline change needed.
- `cargo deny check`: advisories / bans / licenses / sources all ok.
- `cargo tree`: +`unicode-bidi` 0.3.18 only; zero transitive additions at
  `default-features = false, features = ["std"]`.
- Generator `--self-test`: both generators pass offline (confusables 11
  fixtures; properties 19 fixtures).
- Generator `--check`: both fresh against pinned unicode.org sources
  (confusables 6712 entries; properties 27/2321/210/2356/438/130).
- MSRV: `rust-version` 1.89.0 untouched; new `unicode-bidi` requires 1.47;
  toolchain under test `rustc 1.89.0`.
- Release size (same host/toolchain): baseline `b7fcc00` 11,644,656 B vs
  this corrective 11,827,280 B → +182,624 B (+1.57%), explained by the
  compact range tables (~164 KB generated source, binary-search lookup, no
  per-call map construction) plus the dependency-free UAX #9 algorithm and
  new code. Representation evaluated: sorted ranges with binary search (not
  per-character arrays); no per-call HashMap construction; tables immutable
  after process start. No stop condition triggered.
- Remote CI: `36215528682` (ordinary correctness job green after push;
  maintenance-lane MSRV/deny/platform coverage per repo policy).

## 5. Invariant review

- One crate, tracked `Cargo.lock` (minimal `+unicode-bidi` addition only),
  MSRV 1.89.0, `--locked` gates throughout: holds.
- 86-tool/23-category registry, order, ToolSpecs, schemas, profiles,
  audiences, exposure, machine codes, direct/discovery behavior: unchanged
  (`generate-docs --check` + registry-sync tests).
- `CONFUSABLES_UNICODE_VERSION` `18.0.0`, `CONFUSABLES_ENTRY_COUNT` 6712,
  source/checksum pins: unchanged (unit + `--check` proven).
- `lookup` / `has_confusables` / `find_confusables` source-mapping meaning:
  preserved.
- `confusable_skeleton` / `are_confusable` public + source-compatible with
  corrected standards semantics: holds.
- No runtime network, locale, clock, filesystem, or external service in
  deterministic Unicode paths: holds (network only in maintainer generators).
- Bounds/cancellation/concurrency: unchanged; skeleton/script work linear in
  bounded input; tables read-only (`LazyLock` reverse index untouched);
  determinism asserted in unit + property + fuzz.
- Generated files never hand-edited (`confusables_generated.rs`,
  `unicode_properties_generated.rs`): holds (both `--check` reproduce
  byte-for-byte).
- 004 data qualification not reverted for helper-crate compatibility: holds
  (older-epoch crates rejected, not adopted).

## 6. Failure and recovery review

- Oversize input: existing 100k text bounds enforced before skeleton/script
  work; fuzz cap 50k with bounded-findings asserts unchanged.
- Malformed generated input: both generators fail closed with zero writes
  (fixture-proven: duplicates, overlapping ranges, surrogates, inverted
  ranges, unknown classes/scripts).
- Stale generation: both `--check` fail loudly; ordinary CI stays offline.
- Bidi failure mode: no silent fallback to the old non-bidi skeleton — the
  slow path always runs UBA + L3/L4 + internal; fresh-process
  reinitialization is deterministic (no persisted state).
- Cancellation: composite security stages unchanged; no long blocking work
  added between checks.
- Contention: new shared state is read-only generated tables; skeleton and
  resolved-set determinism covered by unit + property + fuzz asserts.
- Epoch mismatch: hardcoded older-epoch crate data is compiled out
  (`no-default-features`); the custom source is the only Bidi data path.

## 7. Migration and compatibility review

- Additive public API only (`internal_skeleton`, `bidi_skeleton_ltr`,
  `resolved_script_set`, `is_mixed_script`, `has_security_invisible_hazard`,
  `is_zero_width_char`, provenance constants); no signature removed.
  `is_legitimate_mixture` kept for source compat, documented non-authoritative.
- Registration order, `json_query` compat, context isolation: untouched.
- Intended observable correctness changes (fixtured, CHANGELOG-recorded):
  Default-Ignorable/bidi inputs gain true-positive collisions
  (`a\u{FE0F}b` ≅ `ab`, S1 ≅ S2); Hangul+Han+Latin flips to mixed;
  `unicode_scripts("☃")` `Other` → `Common`; invalid Rust identifiers flip to
  `valid = false` in `identifier_inspect`; U+034F surfaces as invisible
  hazard consistently.
- Parity: zero failures; accepted-failure baseline untouched.
- Rollback: reverting the two implementation commits restores 004 behavior
  exactly (generated files + lockfile revert atomically).

## 8. Determinism and bounded-execution review

- Internal skeleton: pure function of input + checked-in tables; idempotence
  proven (`internal(internal(X)) = internal(X)`, spec-guaranteed, asserted in
  unit + property + fuzz).
- Public/bidi skeleton: pure and deterministic; idempotence asserted only for
  the LTR fast path (existing fixtures), never claimed for re-applied bidi
  (fuzz target corrected to assert determinism, not bidi idempotence).
- Resolved sets: pure intersection; determinism + Common/Inherited neutrality
  asserted in property tests.
- No clock, TZ, env, locale, or network in runtime paths; limits and
  `limits_applied` behavior unchanged.

## 9. Documentation and operations

- `architecture/generated-assets.md`: new property-tables section (sources,
  checksums, counts, commands), dependency-qualification table, provider
  inventory updated (Script/Bidi/DI → 18.0.0 generated; unicode-bidi
  algorithm-without-data), regenerate table extended.
- `architecture/text-library.md`: 27 modules, `unicode_properties` row,
  internal/bidi/public skeleton distinction, resolved-set authority with
  legacy-allowance warning, shared Rust helper, central hazard predicates,
  corrected mixed-epoch note.
- `architecture/overview.md`: 27-module counts/lists (3 prose spots; the
  generated registry block untouched).
- `docs/library-api.md`: corrected skeleton rows + new
  internal/bidi/resolved/hazard APIs + provenance note.
- `docs/fuzzing.md`: corrected target/property invariants (no bidi
  idempotence claim; added resolved/neutrality/hazard + new seeds).
- `docs/contributing.md` + skill catalog: 27-module counts.
- `CHANGELOG.md`: Fixed/Added entries with before/after fixtures.
- `fuzz/fuzz_targets/unicode_inspection.rs`: seeds + asserts updated
  (requires nightly `cargo-fuzz`; not in merge gate).
- Static guards: provenance count/header asserts (8 property-table unit
  tests), exact-count confusables asserts (unchanged), registry-sync tests,
  both generator `--check`/`--self-test` paths.

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| low | Mixed provider epochs by design (normalization 17.0, casefold/general-category 16.0 vs confusables/skeleton/script/bidi 18.0) | Documented; canonical mappings stability-guaranteed; no newer provider releases exist | Revisit on future upstream releases via a corrective data pass |
| low | Remote-CI posture: closure evidence gathered locally; ordinary-CI run recorded above | Standard for this workstream (same as 003/004 closures) | Maintainer CI on push covers; maintenance-lane MSRV/deny/platform per repo policy |
| low | `text_measure` flat shape vs `text_inspect` rich shape duality (pre-existing) | Unchanged by this milestone | None; do not redesign surfaces here |

No critical, high, or medium findings remain.

## 11. Roadmap disposition

Milestone 005 closed. The deterministic-tool-substrate Unicode workstream
(003 semantic hardening + 004 data qualification + 005 standards-conformance
corrective) is complete; the substrate returns to guard-only status with no
open milestone. No follow-up corrective is required. Future work, if any,
needs a new implementation plan (e.g. upstream provider releases, IDNA/UTS #46
or restriction-level product policy — all explicitly out of scope here and
requiring product decisions, not corrective passes).

## 12. Registry updates

- `plans/registry.md`: 005 → closed with this closure record; substrate
  current milestone → guard-only (no open milestone); dependency note updated.
- `plans/subsystems/deterministic-tool-substrate-roadmap.md`: status header →
  active (guard workstream); milestone 005 row → closed with closure link;
  dependency graph annotated complete.
- `plans/implementation/deterministic-tool-substrate/005-*.md`: status header
  → `closed` (implemented in `cb0d743`, closed here).
- 003/004 implementation and closure files: untouched (immutable history).
