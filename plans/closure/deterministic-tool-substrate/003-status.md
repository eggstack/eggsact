# Deterministic Tool Substrate Milestone 003 — Closure Status

Status: closed

Source implementation plan:

- `plans/implementation/deterministic-tool-substrate/003-unicode-security-correctness-hardening.md`

Source subsystem roadmap:

- `plans/subsystems/deterministic-tool-substrate-roadmap.md#7` (Milestone 3)

Repository baseline reviewed: `ba8a610` (planning baseline; the working tree
carried uncommitted partial skeleton/script scaffolding which this milestone
completed, corrected where non-conforming, and verified below)

Implementation commits or pull requests:

- `3d67807` — Unicode security correctness hardening (all work packages A–E,
  tests, and documentation; data epoch unchanged)

## 1. Executive finding

The Unicode/confusables security semantic layer now conforms to its documented
standards while the 1.x surface and the Unicode 17.0.0 data epoch are
preserved. Whole-string UTS #39 skeleton collisions replace per-character
heuristic inference (fixing both a missed true positive and a false-positive
class, each demonstrated below); bidi/invisible classification is typed and
no longer depends on display strings; script analysis has one authoritative
source with legitimate-mixture handling; Rust identifier validity follows
Unicode XID rules; normalization/mapping diagnostics are corrected; and the
confusables generator fails closed with `--check` / `--self-test` support.
The ordered merge gate is green, parity shows zero regressions, and no
ToolSpec, profile, audience, surface, or machine-code change occurred.
Milestone 004 (Unicode 18 data qualification) is unblocked.

## 2. Requirement-to-evidence matrix

| Requirement (work package) | Evidence | Result | Notes |
|---|---|---|---|
| A: valid policies/profiles exercised; invalid paths separately covered | `fuzz/fuzz_targets/unicode_inspection.rs`, `tests/property/test_unicode_properties.rs` | pass | All 6 policies × fuzzer input with determinism asserts; all 5 canonicalization profiles with idempotence asserts; `permissive`/`strict`/`nfc`/`nfkc` asserted on the invalid path |
| A: seeded security corpus in fuzz target | `seeded_corpus()` in fuzz target | pass | Bidi controls, canonical equivalents, homoglyphs, Japanese/Korean mixtures, `ß`, supplementary-plane confusable, variation selectors, zero-width controls |
| B: version-pinned whole-string skeleton + comparison helper | `confusable_skeleton`, `are_confusable` in `src/text/confusables.rs` | pass | NFD → mapping → NFD over pinned Unicode 17 data; legacy per-character APIs preserved and documented as source mappings |
| B: skeleton-per-identifier grouping; near-match split | `identifier_inspect`, `identifier_table_inspect` in `src/text/identifier.rs` | pass | O(n) skeleton computation + grouping; edit-distance ≤ 1 reports `near_match`, never `confusable` |
| B: `reverse_confusables` multi-code-point correction | `src/text/unicode_tools.rs` | pass | Single-code-point targets indexed only; `Æ` excluded from reverse(`A`) by unit test |
| C: typed hazard classifier; no display predicates | `UnicodeHazard`, `is_bidi_control`, `classify_hazard`; 3 call sites converted | pass | `grep display.contains("BIDI") src/` returns only a historical comment |
| C: RLO/LRO/RLE/LRE/PDF/LRI/RLI/FSI/PDI correct across all consumers | `tests/text/test_unicode_hardening.rs::bidi_classification_agrees_across_consumers`, unit test covering all 11 + 10 negatives | pass | `text_measure`, `text_inspect`, policy, security composite agree |
| C: one authoritative script source; Japanese/Korean legitimacy; Latin/Cyrillic still spoof | `src/text/script.rs` + delegation in 3 consumers; mixed-script matrix tests | pass | `unicode_tools`, `unicode_policy`, `identifier` range tables deleted |
| D: Rust Unicode XID validity; keyword separation; ASCII distinction retained | `is_valid_rust_identifier` via `unicode-ident`; `is_valid_rust_identifier_ascii` | pass | `café` valid, `fn` keyword-rejected, ASCII helper unit-tested |
| D: normalization-instability correction | `check_identifier_strict` | pass | NFC-stable `café` silent; decomposed `cafe + U+0301` and compat `U+FB01` warn |
| D: mapping alignment without cascade | `build_char_mapping` bounded greedy alignment | pass | `ßtest → sstest` yields exactly 1 entry at position 0 |
| D: code-point positions documented | doc comment on `build_char_mapping` | pass | — |
| E: strict generator validation (duplicates, scalars, surrogates, malformed, empty) | `scripts/generate_confusables.py`, `--self-test` (11 fixtures) | pass | Fail-closed; no outputs written until all checks pass |
| E: generator `--check` freshness guard | `--check` run 2026-09-25 | pass | Fetched pinned source, checksum+version verified, both outputs reproduce exactly |
| E: provenance constants without duplicated hand strings | `CONFUSABLES_UNICODE_VERSION/SOURCE_SHA256/ENTRY_COUNT` + unit tests | pass | Header + exact-length asserts |
| Docs: generated-assets, text-library, library-api, fuzzing, CHANGELOG, ASCII-only corrections | commits in `3d67807` | pass | Module count 25 → 26; Rust XID documented |
| No ToolSpec/profile/audience/surface change | `generate-docs --check` clean; registry-sync tests green | pass | — |
| Data epoch remains 17.0.0 | `--check` output + provenance tests | pass | 17.0.0 / `091c7f82…13ef22a` / 6565 entries |

## 3. Production implementation evidence

- `src/text/confusables.rs`: skeleton primitive + comparison helper,
  provenance constants, semantic-distinction docs (legacy lookup APIs
  behavior-preserved).
- `src/text/script.rs` (new): authoritative script ranges, Common/Inherited
  handling, `is_legitimate_mixture` (Japanese Han/Hiragana/Katakana, Korean
  Hangul/Han/Latin).
- `src/text/unicode_tools.rs`: `UnicodeHazard` + `BIDI_CONTROLS` (11 UAX #9
  controls) + `classify_hazard`; `script_name`/`detect_mixed_scripts`
  delegate to `script` with legitimacy allowance; `reverse_confusables`
  single-target-only.
- `src/text/unicode_policy.rs`: bidi table replaced by shared
  `BIDI_CONTROLS`; `get_script`/`detect_mixed_scripts` delegate with
  legitimacy; instability predicate corrected; `build_char_mapping`
  bounded-alignment rewrite.
- `src/text/identifier.rs`: ASCII regex kept as
  `is_valid_rust_identifier_ascii`; validity uses
  `unicode_ident::is_xid_start/continue`; script table deleted; both inspect
  functions use skeleton grouping with `near_match` split; mixed-script
  warning legitimacy-aware.
- `src/text/mod.rs`: new public re-exports (skeleton API, provenance,
  hazard, script, ASCII helper).
- `src/tools/text.rs`, `src/services/security.rs`: display-string bidi
  predicates replaced by `is_bidi_control`; service pushes `UNICODE_RISK`
  for bidi findings without inventing a wire code.
- `Cargo.toml`/`Cargo.lock`: direct `unicode-ident` 1.0.26 (was transitive).
- `scripts/generate_confusables.py`: strict fail-closed parser, `--check`,
  `--self-test`, single in-memory render path for both outputs.
- Tests updated to corrected behavior (5 pre-existing tests encoded the
  RLO-misgrouping and shared-target/ASCII-only assumptions; each now
  asserts the fixed semantics) plus new unit/integration/property coverage.
- Six pre-existing clippy drift errors (untouched files, newer toolchain
  lints) fixed mechanically to keep the gate green; listed in §10.

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

### Results

- `cargo fmt --check`: pass.
- `generate-docs --check`: pass (no registry/profile/exposure drift).
- `clippy -D warnings`: pass (7 errors fixed: 1 new-code range-contains,
  6 pre-existing drift — see §10).
- Focused `text`: 1004 passed, 0 failed.
- Focused `property`: 66 passed, 0 failed.
- Full suite (`--skip parity`, `--test-threads=4`): unit 664 + integration
  3022 + context-isolation 51 + doc 11, all pass, 0 failed.
- `cargo tree`: `unicode-ident v1.0.26`, zero normal dependencies.
- Generator `--self-test`: all strict-parser fixtures pass (offline).
- Generator `--check`: pass with network — checksum `091c7f82…13ef22a`
  verified, version 17.0.0 verified, 6565 entries parsed, both checked-in
  outputs reproduce exactly.
- Parity (`../eggcalc` available): 381 passed, 0 failed, 40 ignored — zero
  regressions; Unicode/identifier parity cases (policy matrix, confusable
  detection, canonicalization, bidi) all pass against the Python reference.
- `cargo deny check`: advisories / bans / licenses / sources all ok.
- Release size (same host/toolchain): baseline `ba8a610` 11,627,616 B vs
  `3d67807` 11,644,656 B → +17,040 B (+0.15%), explained by the
  tables-only `unicode-ident` dependency plus new code. No stop condition.
- Remote CI: not run from this environment (no `gh` submission in scope);
  merge-gate equivalence established locally per the ordered gate above.

## 5. Invariant review

- One crate, tracked `Cargo.lock`, MSRV 1.89.0 (`unicode-ident` requires
  1.71), `--locked` gates throughout: holds.
- ToolSpec names, profile/audience/exposure, schema fields, machine-code
  vocabulary: unchanged (`generate-docs --check` + registry-sync tests).
- `lookup` / `has_confusables` / `find_confusables` behavior for 1.x
  callers: preserved; semantics clarified in docs.
- Deterministic exact-input/exact-output, no clock/TZ/env/net in touched
  utils: holds (new data is immutable; `LazyLock` reverse index is
  read-only and deterministic; network exists only in the maintainer
  generator, never in builds/tests).
- Checked-in generated data; ordinary builds offline: holds.
- Bounds/cancellation: unchanged; skeleton work is linear in bounded input;
  security cancellation stages untouched.
- `confusables_generated.rs` never hand-edited: holds (byte-identical
  reproduction proven by `--check`).
- Policies remain classifiers, not enforcement engines: holds.
- Detection not weakened: RLO-class findings increased in correctness
  (moved to the right bucket, envelope code preserved).

## 6. Failure and recovery review

- Oversize input: 100k text bound enforced before skeleton/script work in
  all entry paths (`unicode_policy_check`, adapters); fuzz target caps at
  50k and asserts bounded findings.
- Malformed generator input: fail-closed, zero writes (unit-fixtured).
- Stale generation: `--check` fails loudly; ordinary CI stays offline.
- Cancellation: typed composite stages unchanged; no long blocking work
  added between checks.
- Contention: new shared state is read-only (`CONFUSABLES`, `LazyLock`
  reverse index); skeleton determinism/idempotence covered by unit +
  property + fuzz asserts.
- No persisted runtime state introduced; fresh-process reinitialization is
  trivially deterministic.

## 7. Migration and compatibility review

- Additive public API only (`confusable_skeleton`, `are_confusable`,
  provenance constants, hazard/script helpers,
  `is_valid_rust_identifier_ascii`); no signature removed.
- Registration order, `json_query` compat, context isolation: untouched.
- Collision results are more accurate by design: `apple`/`аpple` newly
  collides (previously missed — old code required both sides to carry
  mappings, and plain `apple` has none); `АX`/`ΑY` no longer collides.
  Recorded in CHANGELOG as a correctness fix, not a migration.
- `near_match` is additive/clarifying; consumers keying on `confusable`
  continue to receive true UTS #39 collisions.
- Parity: zero failures; no accepted-failure baseline change needed.
- Data epoch unchanged (17.0.0), so no data-migration concerns.

## 8. Determinism and bounded-execution review

- Skeleton: pure function of input + checked-in table; idempotence proven
  (`skeleton(skeleton(x)) == skeleton(x)` over representative + fuzz
  inputs).
- Script/hazard classifiers: pure per-character functions; cross-consumer
  differential tests lock agreement.
- No clock, TZ, env, locale, or network in any touched runtime path.
- Limits: existing text/request/output bounds apply before new analysis;
  `limits_applied` behavior unchanged.

## 9. Documentation and operations

- `architecture/generated-assets.md`: strict validation, `--check`
  (maintainer-only) vs `--self-test` (offline), provenance constants,
  mapping-vs-skeleton distinction.
- `architecture/text-library.md`: 26 modules, `script` row + authority
  note, skeleton API + four-verdict distinction, hazard classifier table,
  Rust XID validity, `near_match` split.
- `docs/library-api.md`: additive skeleton rows + provenance note.
- `docs/fuzzing.md`: corrected target description and Unicode property
  invariants (valid policy/profile matrix, skeleton idempotence).
- `CHANGELOG.md`: Added/Fixed/Changed entries for the milestone.
- ASCII-only corrections: identifier unit test, text-library language
  table, library docs.

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| low | 6 pre-existing clippy drift errors in untouched files (`list.rs` unused bindings, `inspect_prompt.rs`/`tools/text.rs` redundant closures, `validate.rs`/`helpers.rs` collapsible else-if) | Gate would stay red on the current toolchain without them | Fixed mechanically in `3d67807`; no behavior change |
| low | Full Unicode Script/Script_Extensions property table not adopted; `script.rs` is a conservative standards-shaped range table | Rare scripts report `Other`/`Unknown` (filtered, never spoof-flagged alone) | Revisit only if a real spoof case needs an unlisted script; Milestone 004 dependency review covers the data graph |
| low | Remote CI run not recorded (local-only environment) | No independent runner evidence | Maintainer CI on push covers; gate equivalence established locally |
| low | `text_measure` flat shape vs `text_inspect` rich shape both expose bidi state in different keys (`unicode_risks` vs `bidi_controls`) | Pre-existing surface duality, unchanged by this milestone | None in 003; do not redesign surfaces here |

No critical, high, or medium findings remain.

## 11. Roadmap disposition

Milestone 003 closed; hard dependency for Milestone 004 is satisfied and
004 may proceed (data-only delta, kept independently reviewable from this
semantic pass).

## 12. Registry updates

- `plans/registry.md`: 003 → closed; 004 → ready (unblocked by this
  closure); dependency notes updated.
- `plans/subsystems/deterministic-tool-substrate-roadmap.md`: milestone
  status table updated (003 closed with closure link; 004 ready).
- `plans/implementation/deterministic-tool-substrate/003-*.md`: status
  header → `closed` (implemented in `3d67807`).
- `plans/implementation/deterministic-tool-substrate/004-*.md`: status
  header → `ready` (hard dependency satisfied).
