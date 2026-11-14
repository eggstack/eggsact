# Deterministic Tool Substrate Milestone 003 — Unicode Security Correctness Hardening

Status: closed (implemented in `3d67807`; see `plans/closure/deterministic-tool-substrate/003-status.md`)

Repository baseline: `43971e7c1af7f936acfd876f9bff246e72866f2d`

Source roadmap:

- `plans/subsystems/deterministic-tool-substrate-roadmap.md#7`

Long-term requirements:

- `plans/000-long-term-specification.md#2`
- `plans/000-long-term-specification.md#4.2`
- `plans/000-long-term-specification.md#4.3`
- `plans/000-long-term-specification.md#4.4`
- `plans/000-long-term-specification.md#5`
- `plans/000-long-term-specification.md#7`
- `plans/001-terminology-and-domain-model.md#4`
- `plans/002-long-term-roadmap.md#phase-0`

Applicable ADRs:

- `plans/adrs/ADR-0001-planning-conventions-adoption.md`

Primary class: invariant

## 1. Objective

Correct and harden eggsact's Unicode/confusables security semantics without
removing current capabilities, changing MCP tool identity, or advancing the
pinned Unicode Security data epoch.

The milestone must leave the current Unicode 17.0.0 confusables data in place
while fixing the semantic layer above it: whole-string confusable comparison,
bidi classification, script analysis, Rust identifier validity, Unicode test
coverage, normalization diagnostics, mapping diagnostics, and generated-data
validation.

## 2. Why this milestone is ready

The required ownership boundaries already exist:

- `src/text/confusables.rs` owns the generated UTS #39 lookup table;
- `src/text/unicode_tools.rs`, `unicode_policy.rs`, and
  `identifier.rs` own deterministic leaf analysis;
- `src/services/security.rs` owns typed security composition;
- `src/tools/*` are adapters and need no architectural redesign;
- `scripts/generate_confusables.py` already pins source version and SHA-256.

The audit found correctness defects within those boundaries, not an unresolved
product or protocol decision. No new MCP capability, profile, audience, surface,
transport, or execution authority is required.

Milestone 004 is a hard downstream dependency and must not update the Unicode
data epoch until this milestone closes.

## 3. Current implementation evidence

At the baseline:

- `CONFUSABLES` contains 6,565 sorted Unicode 17.0.0 source mappings and
  `lookup()` performs deterministic binary search.
- `find_confusables()` reports per-code-point mappings but there is no
  whole-string UTS #39 skeleton primitive.
- `identifier_inspect()` and `identifier_table_inspect()` infer
  confusable collisions from shared per-character targets and substring
  checks. This can miss real whole-identifier confusables and produce false
  positives.
- `src/services/security.rs` and `src/tools/text.rs` infer bidi membership
  from presentation strings such as `display.contains("BIDI")`; ordinary
  RLO/LRO/LRI/RLI/etc. displays do not contain that text.
- script classification is independently implemented in
  `unicode_tools.rs`, `unicode_policy.rs`, and `identifier.rs` with
  different hand-maintained ranges.
- Rust identifier validity is ASCII-only even though Rust accepts Unicode XID
  identifiers. `unicode-ident` is already present transitively in
  `Cargo.lock`.
- the Unicode fuzz target calls obsolete policy names
  (`permissive`/`strict`) and obsolete canonicalization profile names
  (`nfc`/`nfkc`), so substantial intended coverage currently exercises
  invalid-input return paths instead of the security logic.
- `tests/property/test_unicode_properties.rs` repeats the obsolete policy
  and profile names.
- `identifier_strict` calls ordinary NFC-vs-NFD decomposability
  "normalization instability".
- `build_char_mapping()` aligns original/canonical characters by raw index,
  so one-to-many transforms can shift every later diagnostic mapping.
- the confusables generator checksum/version-pins its source, but malformed
  data rows and duplicate source entries are skipped/overwritten rather than
  rejected and there is no generator `--check` freshness guard.

## 4. Invariants that must not regress

- Keep one crate, tracked `Cargo.lock`, MSRV 1.89.0, and `--locked` gates.
- Preserve all existing Unicode/identifier `ToolSpec` names, profile
  membership, audience, exposure, schema fields, and machine-code vocabulary
  unless a field addition is strictly backward-compatible and justified.
- Preserve the public `lookup(char) -> Option<&'static str>`,
  `has_confusables()`, and `find_confusables()` behavior for 1.x callers.
  Clarify their semantics rather than silently redefining them.
- Preserve deterministic exact-input/exact-output behavior with no runtime
  network access, locale, clock, or environment dependence.
- Keep checked-in generated Unicode data; ordinary builds/tests must not fetch
  unicode.org.
- Keep text/request/output bounds and cancellation semantics unchanged.
- Do not hand-edit `src/text/confusables_generated.rs`.
- Do not convert the source-code/domain-like policies into enforcement engines;
  they remain deterministic classifiers.
- Do not weaken detection merely to reduce warning counts.

## 5. Scope

### In scope

- a version-pinned whole-string confusable skeleton primitive and exact
  skeleton-equality comparison;
- collision detection based on whole-string skeletons rather than individual
  mapping overlap;
- explicit separation of UTS #39 confusable collisions from edit-distance
  near matches;
- one typed/internal Unicode hazard classification source for bidi controls,
  join controls, default/invisible formatting characters, variation selectors,
  combining marks, and controls used by current consumers;
- elimination of security decisions based on human-readable display strings;
- consolidation of script detection around standards-backed Script /
  Script_Extensions semantics, including legitimate multi-script writing
  systems;
- Unicode-correct Rust identifier validity while preserving keyword behavior
  and any separately documented ASCII/security policy;
- repair and strengthening of Unicode property/fuzz coverage;
- correction of normalization-instability and character-mapping diagnostics;
- strict deterministic validation and `--check` support for the confusables
  generator;
- documentation of the semantic distinctions between "has a confusable
  mapping", "has a confusable skeleton collision", "mixed scripts", and
  "near match".

### Explicitly out of scope

- changing the Unicode Security data pin from 17.0.0;
- adding IDNA/UTS #46 domain validation;
- creating a new MCP Unicode tool solely for this work;
- changing policy severity defaults without a demonstrated correctness reason;
- JavaScript/TypeScript Unicode-identifier redesign unless required to preserve
  an existing contract touched by this milestone;
- adding a large ICU-style dependency or a network-backed Unicode database;
- changing discovery/profile/audience behavior;
- broad prompt-injection or source-language lexical parsing work;
- performance claims without same-host measurements.

## 6. Required production changes

### Core/service/adapter

Add a whole-string skeleton primitive under `src/text/confusables.rs` (or an
equally narrow leaf module) and a comparison helper. The implementation must
follow the algorithm appropriate to the pinned UTS #39 / Unicode 17 security
data, including required normalization/ignorable handling, rather than inventing
an eggsact-specific homoglyph transform.

Suggested public additions are:

- `confusable_skeleton(text: &str) -> String`;
- `are_confusable(a: &str, b: &str) -> bool`.

Names may vary if existing library naming conventions require it, but the
semantic distinction from `has_confusables()` must be explicit.

Keep `lookup()`, `has_confusables()`, and `find_confusables()` as
compatibility primitives. Document that they report source mappings, not a
whole-string collision verdict.

Update `identifier_inspect()` and `identifier_table_inspect()` to compute
one skeleton per normalized identifier and group exact skeleton matches. Avoid
recomputing per-character mapping sets for every O(n^2) pair. Only distinct
raw identifiers sharing the same applicable skeleton should become UTS #39
`confusable` collisions. Existing edit-distance <=1 behavior, if retained,
must be reported under a distinct near-match kind rather than
`confusable`.

Introduce one non-presentation Unicode hazard classifier in
`unicode_tools.rs` (enum, flags, or equivalent typed helper). Make
`text_measure`, `inspect_text_security`, and policy code consume that
classification. Human-readable `display` and Unicode names must never be
used as security predicates.

Consolidate script classification so `unicode_tools`, `unicode_policy`,
and `identifier` do not maintain independent range tables. Use a
standards-backed Script/Script_Extensions source. Prefer a small Rust dependency
only if it passes MSRV, license, dependency-count, and release-size review;
otherwise generate the required property table from a pinned Unicode source.
The implementation must account for legitimate Japanese/Korean/Han-associated
script combinations rather than treating every multi-script string as
automatically suspicious.

For Rust syntax validity, use the language's XID rules rather than ASCII-only
ranges. Promoting the already-resolved `unicode-ident` crate to a direct
dependency is preferred if its exposed version/data contract matches the Rust
identifier rules used by the repository. Preserve keyword handling separately.

Correct `normalization_instability` so ordinary precomposed characters are
not warned merely because NFC differs from NFD. Preserve the existing rule name
for 1.x compatibility if practical, but make the predicate describe an actual
input/profile normalization change or another documented instability.

Replace naive character-index zipping in `build_char_mapping()` with a
bounded alignment or operation-derived mapping that does not cascade all later
positions after one-to-many transformations. Preserve the current DTO shape
unless a backward-compatible additive field is necessary.

### Registry, profile, audience, surface

No ToolSpec, registry order, profile, audience, exposure, direct/discovery, or
tool-count change is expected. If implementation evidence suggests one is
needed, stop and report rather than silently changing the surface.

### Protocol and DTOs

Existing wire fields must remain parseable and retain their meaning. Do not
rename `pass_`, findings fields, collision fields, or machine codes in this
milestone.

If a typed internal hazard enum is added, it need not be serialized. Prefer
keeping wire output unchanged unless an additive diagnostic field has clear
consumer value and corresponding compatibility tests.

### Runtime and concurrency

All new data is immutable/read-only. Lazy indexes, if used, must be
thread-safe and deterministic. No global mutable caches, locale-sensitive
behavior, runtime downloads, or request-order dependence.

Exact confusable collision detection should be keyed/grouped by precomputed
skeletons rather than pairwise repeated mapping scans.

### Operator surface (CLI, update, integrate)

No CLI/update/integrate behavior change.

### Documentation and static guards

Update the Unicode/text library documentation and
`architecture/generated-assets.md` to distinguish source mappings from
skeleton comparison and document the exact Unicode Security epoch.

Harden `scripts/generate_confusables.py` so malformed data rows, invalid
Unicode scalar values, empty substitutions, and duplicate source mappings fail
loudly. Add a `--check` mode that regenerates in memory and fails when both
checked-in generated outputs differ. Do not fetch Unicode data during ordinary
Rust compilation.

Integrate the generator freshness check into an appropriate maintainer/static
verification lane only if it can run deterministically from a vendored or
explicitly supplied source; ordinary merge CI must not become dependent on
unicode.org availability. If `--check` necessarily downloads the pinned file,
document it as a maintainer/release check rather than adding network CI.

## 7. Ordered work packages

### Work package A — Repair Unicode verification coverage

Intent:

Make the existing tests exercise valid production paths before changing
semantics.

Required changes:

- replace obsolete `permissive`/`strict` policy inputs with valid policy
  names;
- replace obsolete `nfc`/`nfkc` canonicalization-profile calls with the
  current profile API;
- exercise every valid Unicode policy and every canonicalization profile in the
  fuzz target where meaningful;
- add deterministic assertions that invalid-policy/profile paths remain
  separately covered;
- seed the fuzz corpus with bidi controls, canonical-equivalent forms,
  whole-identifier homoglyphs, legitimate Japanese mixed-script examples,
  one-to-many casefolds, supplementary-plane confusables, and variation
  selectors.

Acceptance evidence:

Focused Unicode/property tests demonstrably enter valid policy/profile logic;
the fuzz harness no longer depends on obsolete names.

### Work package B — Whole-string confusable semantics

Intent:

Replace heuristic per-character collision inference with the pinned UTS #39
whole-string relation.

Required changes:

- implement and document a Unicode-17-compatible skeleton;
- preserve legacy per-character lookup APIs;
- compute skeletons once per identifier;
- group equal skeletons for collision detection;
- split edit-distance near matches from UTS #39 confusable findings;
- fix `reverse_confusables()` so multi-code-point targets are not represented
  as single-code-point equivalence, or explicitly rename/document component
  semantics if compatibility prevents a behavioral correction.

Acceptance evidence:

At minimum, tests cover Latin `apple` versus Cyrillic-`аpple`, identifiers
with multi-code-point mappings, identical non-confusable inputs, unrelated
characters that previously shared one mapping target, and skeleton
determinism/idempotence required by the pinned algorithm.

### Work package C — Typed bidi/invisible classification and script consolidation

Intent:

Make security decisions depend on Unicode properties, not display labels or
duplicated range heuristics.

Required changes:

- centralize bidi-control membership;
- make RLO/LRO/RLE/LRE/PDF/LRI/RLI/FSI/PDI classify correctly across
  `text_measure`, `text_security_inspect`, and Unicode policy paths;
- retain distinctions for invisible mathematical operators, variation
  selectors, join controls, combining marks, and ordinary controls;
- remove duplicated script tables or make all consumers delegate to one source;
- use Script_Extensions/allowed-set semantics sufficient to avoid obvious
  false positives for Japanese and Korean text while still detecting
  Latin/Cyrillic spoof mixtures.

Acceptance evidence:

Cross-consumer differential tests prove the same code point receives the same
hazard/script classification everywhere.

### Work package D — Identifier and normalization correctness

Intent:

Correct language validity and diagnostics without turning security policy into
syntax policy.

Required changes:

- accept valid Rust Unicode XID identifiers and continue rejecting invalid
  starts/continues and reserved keywords;
- retain an explicit ASCII-only/security distinction where callers need it;
- correct normalization-instability detection;
- correct mapping behavior for expansion/contraction cases such as Unicode
  casefolding;
- ensure character positions are documented as code-point positions unless an
  existing contract explicitly says otherwise.

Acceptance evidence:

Tests include Unicode Rust identifiers, keywords, combining forms, `ß -> ss`
style expansion, canonical-equivalent identifiers, and no mapping cascade
after a local expansion.

### Work package E — Generated-data parser hardening

Intent:

Make the pinned generated asset fail closed on malformed generation input.

Required changes:

- reject duplicate sources rather than last-write-wins;
- reject invalid scalar values/surrogates;
- reject malformed non-comment data lines and malformed substitutions;
- preserve exact version and checksum verification;
- add deterministic generated-output comparison/check support;
- expose stable generated metadata constants if useful without duplicating
  hand-maintained version strings.

Acceptance evidence:

Generator unit/self-tests or a small deterministic fixture suite demonstrates
failures for duplicate, malformed, and invalid-scalar input and exact success
for a known-good miniature fixture.

## 8. Failure, cancellation, truncation, and contention semantics

The MCP/tool input bounds remain the controlling resource limits. New skeleton
or script analysis must not bypass existing text limits.

A malformed generated source is a maintainer-generation failure, never a
runtime partial-success condition. Generation must write no outputs until all
validation succeeds.

Security inspection cancellation points remain at the typed composite stages
already documented. Do not add long blocking work between cancellation checks.

Read-only lazy indexes must be initialized atomically and be safe under
concurrent calls. Results must not depend on which request initialized them.

If a standards-backed dependency cannot satisfy MSRV, license, deterministic
data, or footprint constraints, stop that work package and use the documented
generated-data fallback rather than weakening script semantics.

## 9. Compatibility and migration

This is a correctness-hardening pass inside the existing 1.x surface.

- existing per-character confusable APIs remain available;
- new skeleton helpers are additive;
- existing Unicode/identifier ToolSpecs and registration order stay unchanged;
- wire DTOs and machine codes remain backward-compatible;
- collision results may become more accurate: false positives may disappear
  and previously missed true confusable pairs may appear. Document this as a
  correctness fix in CHANGELOG/compatibility notes;
- a distinct near-match collision kind is additive/clarifying; consumers that
  key only on `confusable` must continue to receive true UTS #39 collisions;
- generated Unicode data remains 17.0.0 until Milestone 004.

Do not preserve known-wrong security behavior merely for output parity with
the Python reference. If parity changes, classify and document the correctness
difference explicitly.

## 10. Required tests

### Focused unit tests

- confusable skeleton representative mappings and whole-string equality;
- multi-code-point target handling;
- reverse-confusable sequence semantics;
- bidi membership for every supported bidi control;
- invisible/operator/variation-selector classification;
- Script_Extensions/allowed mixed-script cases;
- Rust XID validity and keyword separation;
- normalization diagnostic predicate;
- character mapping for expansion/contraction;
- generator parser malformed/duplicate/scalar checks.

### Integration tests

- `unicode_policy_check` policy matrix;
- `identifier_inspect` and `identifier_table_inspect` collision matrix;
- `text_measure` bidi flag;
- `text_security_inspect` warning/verdict behavior;
- profile/audience/tool registration unchanged.

### Restart and recovery tests

Not applicable beyond deterministic lazy-index reinitialization in a fresh
process. No persisted runtime state is introduced.

### Contention and cancellation tests

Exercise concurrent skeleton/security calls if a LazyLock/index is introduced;
existing security-service cancellation tests must remain green.

### Negative and boundary tests

- empty/ASCII input;
- 100k tool-boundary input;
- combining-mark-heavy text;
- supplementary-plane code points;
- default-ignorable and variation-selector sequences;
- malformed generator fixtures;
- non-confusable identifiers sharing one mapped component;
- legitimate multi-script writing systems.

### Parity and compatibility tests

Run existing parity locally when `../eggcalc` is available. Any intentional
Unicode correctness divergence must be recorded rather than added blindly to
the accepted-failure baseline.

Add property tests for determinism and, where mandated by the pinned UTS #39
algorithm, skeleton idempotence.

## 11. Required verification commands

```bash
cargo fmt --all -- --check
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --test lib text
cargo test --locked --test lib property
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
cargo tree --locked -e normal
```

Also run the hardened confusables-generator fixture/check command defined by the
implementation. If `../eggcalc` is available, run the Unicode/identifier
parity subset and classify intentional differences.

If a new direct dependency is introduced, run `cargo deny check` and compare
release binary size against the baseline before/after on one stable target.
Binary-size evidence is descriptive, not a host-specific pass threshold; a
large unexplained increase is a stop condition.

## 12. Documentation updates

- `architecture/generated-assets.md`: generation validation/check workflow
  and Unicode data provenance;
- `architecture/text-library.md`: skeleton vs per-character mapping, script
  semantics, identifier security model;
- `docs/library-api.md`: additive skeleton API if public;
- `docs/fuzzing.md`: corrected Unicode target/property invariants;
- `CHANGELOG.md`: correctness notes, including any parity-visible collision
  changes;
- update tests/docs that currently describe Rust identifiers as ASCII-only.

## 13. Acceptance criteria

The milestone is ready for closure only when:

- valid Unicode policies/profiles are actually exercised by property/fuzz
  harnesses;
- whole-string confusable comparisons use the pinned UTS #39 skeleton relation;
- `apple` and the corresponding Cyrillic-`аpple` spoof collide through the
  identifier APIs;
- unrelated strings no longer collide merely because they share one mapped
  component;
- bidi controls are correctly reported across text measurement, policy, and
  security composite paths without display-string predicates;
- script analysis has one authoritative implementation and handles documented
  legitimate mixed-script cases;
- valid Unicode Rust identifiers are not rejected solely for being non-ASCII;
- normalization diagnostics do not label ordinary canonical decomposition as
  instability;
- mapping diagnostics remain aligned after one-to-many transformations;
- generated confusables parsing fails closed on malformed/duplicate data;
- Unicode Security data remains pinned to 17.0.0;
- no ToolSpec/profile/audience/surface change occurred;
- the ordered merge gate is green.

## 14. Stop conditions

The agent must stop and report rather than improvise when:

- implementing correct skeleton semantics would require changing a public
  `lookup` contract rather than adding an internal/additive path;
- a proposed Unicode property dependency fails MSRV/license/security review or
  creates a material unexplained binary-size increase and no small generated
  fallback is practical;
- correct Rust identifier semantics conflict with a documented public
  ASCII-only contract not identified in this plan;
- script-policy correctness requires choosing a new public restriction-level
  contract;
- a fix requires IDNA/domain parsing, source-language tokenization, or another
  subsystem;
- the Unicode 17 source/checksum no longer verifies;
- repository evidence shows a concurrent Unicode-security change after the
  baseline that materially changes the plan.

## 15. Closure evidence required

The closure record must contain:

- implementation commit(s);
- requirement-to-evidence mapping for each work package;
- focused Unicode/text/property test outcomes;
- full merge-gate outcome and remote CI run if available;
- exact confusables data version/checksum confirming it remained 17.0.0;
- examples proving one formerly missed true collision and one formerly possible
  false-positive class;
- bidi cross-consumer evidence;
- script legitimate-mixture and spoof-mixture evidence;
- Rust Unicode identifier evidence;
- generator malformed-input evidence;
- dependency/MSRV/license/binary-size note for any new direct dependency;
- parity result or explicit statement that the external Python reference was
  unavailable;
- residual findings classified by severity;
- recommendation: closed, conditionally closed, corrective pass required, or
  blocked.

## 16. Handoff notes

Start by repairing the invalid property/fuzz inputs before trusting fuzz
results. Keep the Unicode 17 source epoch fixed throughout this milestone so
semantic changes and data changes remain reviewable independently.

Prefer typed leaf helpers consumed by policy/security/identifier code over new
adapter logic. Preserve unrelated user changes. Integration tests that touch the
full suite must retain `--test-threads=4`.

Do not implement Milestone 004 in the same commit unless this milestone is
already independently closed and the data-only delta remains separately
reviewable.
