# Deterministic Tool Substrate Milestone 006 — Closure Status

Status: closed

Source implementation plan:

- `plans/implementation/deterministic-tool-substrate/006-unicode-conformance-edge-case-corrective.md`

Source subsystem roadmap:

- `plans/subsystems/deterministic-tool-substrate-roadmap.md#7` (Milestone 6)

Repository baseline reviewed: `6c6a47890f2bfa27922f85834c3e641d12745e21`
(plan baseline; the working tree was clean at handoff)

Implementation commits or pull requests:

- `83de61a` — Unicode conformance edge-case corrective (work packages
  A–D: 006 conformance fixtures, Bidi_Class `@missing` generation/lookup,
  paragraph-local bidi skeleton, Unknown/Zzzz resolved sets,
  property/fuzz extensions, documentation/CHANGELOG)
- `79a4ff5` — Close Milestone 006 (this closure record, plan status,
  registry/roadmap reconciliation)

Milestone 003/004/005 closure references (immutable history):

- `plans/closure/deterministic-tool-substrate/003-status.md`
  (implementation `3d67807`, closure `24073e9`)
- `plans/closure/deterministic-tool-substrate/004-status.md`
  (implementation `2e3860c`, closure `b7fcc00`)
- `plans/closure/deterministic-tool-substrate/005-status.md`
  (implementation `cb0d743`, closure `1f0816e`)

Normative versions used:

- UTS #39, Unicode Security Mechanisms, Version 18.0.0, Revision 34
  (`tr39-34`)
- UAX #9, Unicode Bidirectional Algorithm, Version 18.0.0, Revision 52
  (`tr9-52`)
- UAX #24, Unicode Script Property, Version 18.0.0, Revision 41
  (`tr24-41`)
- UAX #44, Unicode Character Database, Version 18.0.0 (`tr44-36`)
- Unicode 18.0.0 `DerivedBidiClass.txt` (SHA-256
  `d9e23222522551348ea1ccfbb4f62efbf98982afb95840f8959c08ed992c5607`,
  unchanged)
- `unicode-bidi` 0.3.18 `BidiInfo::reordered_levels_per_char` contract
  (algorithm only; hardcoded data disabled)

## 1. Executive finding

The three residual post-005 Unicode-conformance defects are corrected
without reopening the Unicode 18 data pipeline or changing any public
MCP/tool surface. `bidi_class_name()` honors the 24 ordered
`DerivedBidiClass.txt` `@missing` defaults (explicit rows first,
later-wins reverse scan, loud invariant instead of silent `L`);
`bidi_skeleton_ltr()` runs UAX #9 L1/L2 paragraph-locally per paragraph
substring with no all-LTR mismatch fallback; and `augmented_script_set()`
retains `Zzzz` as an ordinary constraining set while only `Zyyy`/`Zinh`
behave as ALL. The ordered merge gate, focused/property suites, parity
(381/0), `cargo-deny`, both generator `--self-test`/`--check` paths,
same-host release-size review (+48 B), and remote CI are green. No
critical, high, or medium findings remain; two low findings (provider-epoch
skew carried from 005, remote-CI posture) are recorded in §10.

## 2. Requirement-to-evidence matrix

| Requirement (work package) | Evidence | Result | Notes |
|---|---|---|---|
| A: default-only R/AL/ET/L fixtures proven absent from explicit rows | `tests/text/test_unicode_conformance_006.rs::bidi_default_only_r_classifies_r` (U+0590→R), `::bidi_default_only_al_classifies_al` (U+070E→AL), `::bidi_default_only_et_classifies_et` (U+20C5→ET), `::bidi_global_l_default_still_applies` (U+0378→L); `assert_no_explicit_bidi_row` scans `BIDI_CLASS_RANGES` | pass | R/AL/ET failed at baseline (classified L); L passed before/after as behavior pin |
| A: multi-paragraph skeleton fixtures (L2 reorder, L4 mirror, LF/CRLF/U+2029, three-paragraph) | `::multiparagraph_later_rtl_paragraph_reorders_lf`, `::multiparagraph_later_rtl_paragraph_mirrors_lf`, `::multiparagraph_rtl_reordering_crlf`, `::multiparagraph_rtl_reordering_paragraph_separator`, `::multiparagraph_three_paragraphs_stay_independent`, `::multiparagraph_skeleton_deterministic` vs per-paragraph references | pass | All 6 failed at baseline at `confusables.rs:148` length guard (7 vs 4): the whole-text vector mismatch itself |
| A: Unknown script fixtures | `::unknown_private_use_resolves_to_zzzz`, `::common_plus_unknown_resolves_to_zzzz`, `::latin_plus_unknown_is_mixed`, `::unknown_plus_unknown_is_not_mixed` | pass | First 3 failed at baseline (Unknown skipped as ALL); 4th passed before/after as behavior pin |
| B: bc aliases from pinned property aliases | `scripts/generate_unicode_security_properties.py::parse_bc_aliases` (`bc ; Short ; Long` rows, conflict fail-closed, probe asserts) + self-test miniature | pass | No second hand-maintained vocabulary |
| B: ordered `@missing` parse/generate/check | `::parse_bidi_class_missing` (strict `# @missing:` syntax; informational comments ignored; malformed/unknown/inverted rejected; global-L-first asserted; order preserved) + `BIDI_CLASS_DEFAULT_RANGES` (24 ranges) + `--self-test` later-wins/explicit-wins miniatures + `--check` byte-for-byte | pass | Counts: bidi_class 2356 unchanged; defaults 24; all source SHA-256 unchanged |
| B: explicit-first + ordered-default runtime lookup | `src/text/unicode_properties.rs::bidi_class_name` (binary search, then reverse default scan, then `unreachable!` invariant); `is_bidi_r_or_al` inherits | pass | No `unwrap_or("L")` remains |
| C: paragraph-local L1/L2, separators, L3/L4, no silent fallback | `src/text/confusables.rs::bidi_skeleton_ltr` (per-paragraph `BidiInfo::new_with_data_source` substring, forced LTR level, separators verbatim, visual-map/L3/L4 unchanged, `assert_eq!` shape guards) | pass | Fallback arm deleted; single-paragraph S1/S2 green unmodified |
| D: Zzzz constrains, Zyyy/Zinh stay ALL | `augmented_script_set` (Zzzz early-return deleted) + `resolved_script_set_detailed` comment correction; diagnostic `script_of`/`policy_script_of` spellings untouched | pass | Jpan/Kore/Hanb/Hntl + Table 1a + Common/Inherited fixtures green |
| E: focused/property/fuzz/merge-gate/parity/deny/CI/docs/closure | §4 below | pass | Release delta +48 B recorded; no ToolSpec drift |

## 3. Production implementation evidence

WP-B generation (`scripts/generate_unicode_security_properties.py`):

- `MISSING_RE` strict directive syntax; `parse_bc_aliases` derives
  `Left_To_Right→L`, `Right_To_Left→R`, `Arabic_Letter→AL`,
  `European_Terminator→ET` (and every other `bc` row) from the pinned
  `PropertyValueAliases.txt`.
- `parse_bidi_class_missing` preserves the 24-directive source order
  (global `0000..10FFFF;L` first — asserted — then 23 narrow R/AL/ET
  overrides across Hebrew, Arabic-script blocks, Currency Symbols,
  presentation forms, and historic/supplementary RTL blocks).
- `render_rust` emits `BIDI_CLASS_DEFAULT_RANGES` in order and exposes
  `bidi_class_defaults=24 ranges` in the generated header; `build_tables`
  counts/threading updated.

WP-B/C/D runtime (`src/text/unicode_properties.rs`,
`src/text/confusables.rs`):

- `bidi_class_name`: explicit `BIDI_CLASS_RANGES` binary search, then
  reverse linear scan of the 24-entry default table (last match wins),
  then `unreachable!("invariant: global Bidi_Class @missing default must
  cover …")`. `Unicode18BidiData` and `is_bidi_r_or_al` inherit the
  corrected classification with no signature change.
- `bidi_skeleton_ltr`: the whole-text `info` now provides paragraph
  segmentation only; each `para_text` substring is processed by its own
  `BidiInfo::new_with_data_source(&data, para_text, Some(Level::ltr()))`
  (forced LTR paragraph level per UTS #39 `bidiSkeleton(LTR, X)`), with
  `assert_eq!` guards that exactly one paragraph and exactly
  paragraph-many levels result. Separators stay verbatim; visual index
  maps, L3 mark fixup, L4 mirroring, and the final `internal_skeleton`
  stage are unchanged.
- `augmented_script_set`: the `Zzzz → ALL` early return is deleted with
  its comment replaced by the literal rule; `resolved_script_set_detailed`
  now documents ALL as Common/Inherited only.

No registry, profile, audience, surface, protocol, DTO-schema, CLI,
dependency, MSRV, or machine-code change (`generate-docs --check` clean;
86 tools / 23 categories unchanged; `Cargo.toml`/`Cargo.lock` untouched).

## 4. Verification executed

### Commands run

```bash
cargo test --locked --test lib test_unicode_conformance_006
cargo test --locked --test lib text
cargo test --locked --test lib property
python3 scripts/generate_unicode_security_properties.py --self-test
python3 scripts/generate_unicode_security_properties.py --check
python3 scripts/generate_confusables.py --self-test
python3 scripts/generate_confusables.py --check
cargo fmt --all -- --check
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
cargo deny --locked check
cargo tree --locked -e normal
cargo build --locked && cargo test --locked --test lib parity
cargo build --locked --release  # size evidence
```

### Results

- New conformance file: 14/14 pass (12 failed at baseline for the
  intended reasons, 2 behavior pins passed before/after; raw baseline
  output: 12 failed — R/AL/ET asserts at `test_unicode_conformance_006.rs`
  lines 34/43/52, Unknown asserts at lines 150/161/171, all 6
  multi-paragraph tests at `confusables.rs:148` with `left: 7, right: 4`).
- Focused `text`: 1037 passed, 0 failed (005 S1/S2, Default-Ignorable,
  Script_Extensions, Rust XID, hazard suites green unmodified).
- Focused `property`: 69 passed, 0 failed (66 carried + Unknown-constraint
  assertions folded into the resolved-set test; skeleton determinism
  extended with default-only/multi-paragraph/private-use seeds).
- Generator `--self-test`: both generators pass offline (properties: prior
  19 fixtures + 8 new `@missing`/alias miniatures covering syntax,
  alias normalization, later-wins, explicit-wins, malformed/unknown/
  inverted rejection, informational-comment ignore).
- Generator `--check`: both fresh against pinned sources (properties:
  27/2321/210/2356/**24 defaults**/438/130; confusables 6712 entries;
  all 7 + 1 source SHA-256 unchanged).
- `cargo fmt --check`: pass (one formatting fixup applied during
  implementation).
- `generate-docs --check`: pass (no ToolSpec/profile/exposure drift).
- `clippy -D warnings`: pass, zero remaining.
- Full suite (`--skip parity`, `--test-threads=4`): lib 673 + misc 40/14
  + integration 3058 + context-isolation 51 — all pass, 0 failed
  (integration 3044 → 3058 = +14 new 006 tests).
- `cargo test --locked --doc`: 11 passed, 0 failed.
- Parity (`../eggcalc` available): 381 passed, 0 failed, 40 ignored — zero
  regressions; accepted-failure baseline untouched.
- `cargo deny check`: advisories / bans / licenses / sources all ok.
- `cargo tree`: `unicode-bidi` 0.3.18 unchanged, no new dependency, zero
  transitive additions at `default-features = false`.
- MSRV: `rust-version` 1.89.0 untouched.
- Release size (same host/toolchain): baseline `83de61a^`
  11,827,280 B vs this corrective 11,827,328 B → +48 B (+0.0004%),
  explained by the 24-entry ordered default table plus lookup/branch code;
  representation justified (tiny static table, reverse linear scan, no
  scalar-space expansion). No stop condition triggered.
- Remote CI: `36225359570` (ordinary correctness job green after push;
  maintenance-lane MSRV/deny/platform coverage per repo policy).

## 5. Invariant review

- One crate, tracked `Cargo.lock` (no dependency delta), MSRV 1.89.0,
  `--locked` gates throughout: holds.
- 86-tool/23-category registry, order, ToolSpecs, schemas, profiles,
  audiences, exposure, machine codes, direct/discovery behavior: unchanged
  (`generate-docs --check` + registry-sync tests).
- Unicode 18.0.0 epochs, source SHA-256 pins, confusables 6,712 entries:
  unchanged (unit + `--check` proven); defaults count newly exposed and
  asserted (24).
- Public `internal_skeleton` / `bidi_skeleton_ltr` / `confusable_skeleton` /
  `are_confusable` / `resolved_script_set` / `is_mixed_script` signatures:
  unchanged; only edge-case outputs corrected behind them.
- No runtime network, locale, clock, filesystem, or external service in
  deterministic Unicode paths: holds (network only in maintainer
  generators).
- Bounds/cancellation/concurrency: unchanged; skeleton/script work linear
  in bounded input plus a 24-entry reverse scan; tables read-only;
  determinism asserted in unit + property + fuzz.
- Generated files never hand-edited: holds (`--check` reproduces
  byte-for-byte; the generator script itself is the edited source).
- 003/004/005 closure records: untouched (immutable history).

## 6. Failure and recovery review

- Oversize input: existing 100k text bounds enforced before skeleton/script
  work; fuzz cap 50k unchanged.
- Malformed generated input: `@missing` parser fails closed with zero
  writes (fixture-proven: malformed directives, unknown values, inverted
  ranges, missing global default); ordinary CI stays offline.
- Missing runtime coverage: a valid `char` outside explicit + default
  tables is `unreachable!` (internal invariant), never silent `L`; the
  global default makes it unreachable by construction.
- Bidi shape violation: paragraph-count/level-count mismatches are
  `assert_eq!` failures, never silent all-LTR output.
- Stale generation: `--check` fails loudly; header default count makes
  silent directive loss detectable.
- Cancellation/contention: unchanged; new shared state is read-only
  generated tables.
- U+2029 separator nuance (found during implementation, not a defect): it
  is both a UAX #9 paragraph boundary and a confusables `U+0020` mapping,
  so multi-paragraph skeletons join per-paragraph references through the
  separator's own internal skeleton; the test reference encodes this
  explicitly.

## 7. Migration and compatibility review

- No schema or API migration: all changes are standards corrections behind
  existing APIs.
- Intended observable correctness changes (fixtured, CHANGELOG-recorded):
  default-only RTL/Currency code points gain correct Bidi_Class and can
  trigger bidi processing (`is_bidi_r_or_al` now true for default-only R/AL);
  multi-paragraph RTL skeletons change from the erroneous all-LTR fallback
  to paragraph-local ordering/mirroring; Latin + Unknown/private-use newly
  reports mixed-script.
- Confusables data, identifier policy, and security severity intentionally
  unchanged by these output corrections.
- Parity: zero failures; accepted-failure baseline untouched.
- Rollback: reverting the implementation commit restores 005 behavior
  exactly (generated file + script + sources revert atomically).

## 8. Determinism and bounded-execution review

- Bidi_Class lookup: pure function of input + checked-in tables (explicit
  binary search + 24-entry reverse scan); deterministic.
- Public/bidi skeleton: pure and deterministic; per-paragraph sub-`BidiInfo`
  work is linear in total text length; multi-paragraph determinism asserted
  (repeated calls) in conformance + property + fuzz.
- Resolved sets: pure intersection with Unknown as an ordinary member;
  determinism + Common/Inherited neutrality + Unknown constraint asserted
  in property + fuzz.
- No clock, TZ, env, locale, or network in runtime paths; limits and
  `limits_applied` behavior unchanged.

## 9. Documentation and operations

- `architecture/generated-assets.md`: entry counts (+24 defaults) and new
  `@missing` parser/precedence subsection (strict syntax, alias derivation,
  order preservation, global-first assert, runtime reverse-scan contract).
- `architecture/text-library.md`: paragraph-local bidi processing row and
  Unknown-as-constraint resolved-set rule.
- `docs/library-api.md`: clarified `bidi_skeleton_ltr` (paragraph-local
  L1/L2), `resolved_script_set`/`is_mixed_script` (Unknown constrains;
  Latin + private-use mixes) rows.
- `docs/fuzzing.md`: new edge-case invariants in the target table and
  property-test list.
- `CHANGELOG.md`: Fixed entries with representative before/after fixtures.
- `fuzz/fuzz_targets/unicode_inspection.rs`: 8 new seeds (default-only
  R/AL/ET, LF/CRLF/U+2029 multi-paragraph RTL, private-use, Latin+Unknown)
  and spot assertions (R/AL/ET classes, later-paragraph reorder,
  `{Zzzz}` resolution, Latin+Unknown mixed, Unknown+Unknown single).
- Static guards: `COUNT_BIDI_CLASS_DEFAULT_RANGES` + header-count assert,
  provenance asserts, `assert_no_explicit_bidi_row` default-only proofs,
  both generator `--check`/`--self-test` paths.

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| low | Mixed provider epochs by design (carried from 005: normalization 17.0, casefold/general-category 16.0 vs confusables/skeleton/script/bidi 18.0) | Documented; canonical mappings stability-guaranteed; no newer provider releases exist | Revisit on future upstream releases via a corrective data pass |
| low | Remote-CI posture: closure evidence gathered locally; ordinary-CI run recorded above | Standard for this workstream (same as 003/004/005 closures) | Maintainer CI on push covers; maintenance-lane MSRV/deny/platform per repo policy |
| low | `text_measure` flat shape vs `text_inspect` rich shape duality (pre-existing, carried from 005) | Unchanged by this milestone | None; do not redesign surfaces here |

No critical, high, or medium findings remain.

## 11. Roadmap disposition

Milestone 006 closed. The deterministic-tool-substrate Unicode workstream
(003 semantic hardening + 004 data qualification + 005
standards-conformance corrective + 006 edge-case corrective) is complete;
the substrate returns to guard-only status with no open milestone. No
follow-up corrective is required: 006 has no hard/interface dependents, so
no future plan is unblocked beyond the workstream itself returning to
guard-only (MCP 03c Parts D-F remain blocked on provider credentials/budget;
distribution M005 remains blocked on Eggpack CI M003d + Build M005 — both
independent of this milestone). Future work, if any, needs a new
implementation plan (e.g. upstream provider releases, IDNA/UTS #46 or
restriction-level product policy — all explicitly out of scope here and
requiring product decisions, not corrective passes).

## 12. Registry updates

- `plans/registry.md`: 006 → closed with this closure record; substrate
  current milestone → guard-only (no open milestone); dependency note updated.
- `plans/subsystems/deterministic-tool-substrate-roadmap.md`: status header →
  active (guard workstream); milestone 006 row → closed with closure link;
  dependency graph annotated closed; §4 corrected to past tense.
- `plans/implementation/deterministic-tool-substrate/006-*.md`: status header
  → `closed` (implemented in `83de61a`, closed here).
- 003/004/005 implementation and closure files: untouched (immutable history).
