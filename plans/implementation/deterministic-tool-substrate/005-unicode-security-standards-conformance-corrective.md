# Deterministic Tool Substrate Milestone 005 — Unicode Security Standards-Conformance Corrective

Status: closed

Repository baseline: `b7fcc004dab0b2550eae3ffd31a1aa99fbdbb276`

Source roadmap:

- `plans/subsystems/deterministic-tool-substrate-roadmap.md#7`

Corrects:

- `plans/implementation/deterministic-tool-substrate/003-unicode-security-correctness-hardening.md`
- `plans/closure/deterministic-tool-substrate/003-status.md`
- `plans/implementation/deterministic-tool-substrate/004-unicode18-security-data-qualification.md`
- `plans/closure/deterministic-tool-substrate/004-status.md`

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

Normative references:

- Unicode Technical Standard #39, Unicode Security Mechanisms, Version 18.0.0,
  Revision 34: <https://www.unicode.org/reports/tr39/tr39-34.html>
- Unicode Standard Annex #24, Unicode Script Property:
  <https://www.unicode.org/reports/tr24/>
- Unicode Standard Annex #9, Unicode Bidirectional Algorithm:
  <https://www.unicode.org/reports/tr9/>
- Unicode 18.0.0 data/components:
  <https://www.unicode.org/versions/Unicode18.0.0/>

## 1. Objective

Close the standards-fidelity gaps left after Milestones 003 and 004 while
preserving their successful hardening and Unicode 18 confusables-data refresh.

This corrective must make Eggsact's APIs match the semantics they claim:

1. `confusable_skeleton()` must implement the current UTS #39
   `skeleton(X) = bidiSkeleton(LTR, X)` relation, not only the internal
   NFD + confusables substitution stage;
2. the internal skeleton stage must remove
   `Default_Ignorable_Code_Point` characters as required by UTS #39;
3. mixed-script detection must use Script_Extensions and the UTS #39 resolved
   script-set algorithm, including the Unicode 18 augmented-script rules,
   rather than a hand-maintained Script table plus Japanese/Korean allowlists;
4. `identifier_inspect(..., "rust", ...)` must actually validate Rust
   identifiers instead of falling through as valid;
5. remaining Unicode security predicates should delegate to the typed property
   layer rather than maintain parallel invisible/control sets.

The Unicode 18.0.0 confusables source/checksum and all public tool identities
remain in place.

## 2. Why this milestone is ready

No architecture decision is required. The failures are local correctness gaps
inside the existing deterministic text/security substrate.

Milestone 003 established:

- additive public skeleton/comparison APIs;
- typed bidi hazard classification;
- central script module ownership;
- Rust XID validation in `identifier_analyze`;
- repaired property/fuzz paths;
- a strict generated-data pipeline.

Milestone 004 established:

- authoritative Unicode 18.0.0 `confusables.txt`;
- checksum/version provenance;
- reproducible generated outputs;
- explicit mixed Unicode-provider epoch documentation.

A follow-up audit against UTS #39 Revision 34 showed that implementation and
documentation still diverge in the areas listed below. The original 003/004
closure records remain immutable per planning policy; this corrective carries
the remediation and later closure evidence.

## 3. Current implementation evidence

At the baseline:

### 3.1 Skeleton semantics are incomplete

`src/text/confusables.rs::confusable_skeleton()` currently does:

```text
NFD(input)
  -> per-code-point confusables.txt substitution
  -> NFD(output)
```

and documents this as the whole-string UTS #39 skeleton.

Current UTS #39 distinguishes the internal skeleton transformation from the
public `skeleton()` operation. The internal stage removes
`Default_Ignorable_Code_Point` after normalization before confusables
substitution. The public `skeleton(X)` is defined through
`bidiSkeleton(LTR, X)`, which incorporates bidirectional reordering and
mirroring semantics before/around the internal skeleton transformation.

The repository contains no Default_Ignorable property table and no bidi
skeleton implementation. A repository search finds neither
`Default_Ignorable_Code_Point` nor `bidiSkeleton`.

Concrete missed class: variation selectors such as U+FE0F and other default
ignorables currently survive the skeleton when they are not present as
confusables-table sources.

### 3.2 Mixed-script detection is an approximation, not UTS #39

`src/text/script.rs` explicitly states that it is not a full
Script_Extensions implementation. It uses a small hand-maintained range table
and `is_legitimate_mixture()` allowlists for:

- Han/Hiragana/Katakana;
- Hangul/Han/Latin.

Current UTS #39 instead defines an augmented Script_Extensions set per
character, treats Common/Inherited as ALL, augments scripts with writing-system
codes such as Jpan/Kore/Hanb/Hntl, and defines the resolved script set as the
intersection over the string. Mixed-script is determined by whether that
resolved set is empty.

The current Korean+Latin allowlist therefore conflates restriction-level
acceptability with the narrower mixed-script predicate.

### 3.3 Rust identifier validation is inconsistent across APIs

`identifier_analyze()` correctly uses `unicode-ident` XID rules plus keyword
handling.

`identifier_inspect()` accepts `language = "rust"` in the MCP schema and
adapter, but its validation match only handles Python and
JavaScript/TypeScript. Rust falls through with `valid = true`, so invalid Rust
identifiers may be reported as valid.

### 3.4 Hazard ownership is only partially consolidated

`unicode_tools.rs` owns `UnicodeHazard`, `BIDI_CONTROLS`, and typed
classification, but:

- `unicode_policy.rs::find_invisibles()` still builds an independent static
  `HashSet<char>`;
- identifier analysis still has an independent `INVISIBLE_CHARS` path.

This leaves future drift possible even though the original display-string bug
was fixed.

### 3.5 Planning state contains stale closed-work text

The subsystem dependency graph still labels 003 as ready and 004 as blocked
despite both being closed. `plans/registry.md` also retains a closed 004 row
under "Blocked work".

This plan registration corrects those planning defects without rewriting the
immutable 003/004 closure records.

## 4. Invariants that must not regress

- One Rust crate; `Cargo.lock` tracked; MSRV 1.89.0; all gates use
  `--locked`.
- The 86-tool/23-category registry, registration order, ToolSpecs, schemas,
  profiles, audiences, exposure, machine codes, and direct/discovery behavior
  remain unchanged.
- `CONFUSABLES_UNICODE_VERSION` stays `18.0.0`,
  `CONFUSABLES_ENTRY_COUNT` stays 6,712 unless authoritative Unicode data
  itself is intentionally changed in a separate milestone.
- Existing per-character APIs `lookup()`, `has_confusables()`, and
  `find_confusables()` remain source-compatible and retain their documented
  source-mapping meaning.
- `confusable_skeleton()` and `are_confusable()` remain public and
  source-compatible; this corrective fixes their standards semantics.
- No runtime network, locale, clock, filesystem lookup, or external service is
  introduced into deterministic Unicode operations.
- Existing input/output bounds, cancellation semantics, and concurrency
  behavior remain contractual.
- Generated Unicode property data, if added, is checked in, version/checksum
  pinned, and never fetched during normal builds or ordinary CI.
- The correct Unicode 18 data-refresh work from Milestone 004 must not be
  reverted to force compatibility with older helper crates.

## 5. Scope

### In scope

- exact UTS #39 Revision 34 internal skeleton semantics for Unicode 18,
  including removal of Default_Ignorable_Code_Point;
- exact UTS #39 public skeleton/bidiSkeleton(LTR, X) semantics for the existing
  `confusable_skeleton()` API;
- a private/additive internal-skeleton helper if implementation clarity
  requires one;
- Unicode 18 property provenance needed by the skeleton, including
  Default_Ignorable and bidi mirroring/class data where current dependencies
  cannot provide the required epoch;
- Script_Extensions-based resolved script sets with Unicode 18 augmented-script
  semantics (including Hntl support);
- replacement of the hand-written "legitimate mixture" decision with resolved
  script-set logic;
- a Rust validation branch in `identifier_inspect()` using the same XID +
  keyword rules as `identifier_analyze()`;
- consolidation of remaining security-sensitive invisible/bidi/join predicates
  behind the typed Unicode-property layer;
- conformance/regression/property/fuzz tests for the corrected semantics;
- documentation and planning-state reconciliation.

### Explicitly out of scope

- changing the Unicode 18 `confusables.txt` source/version/checksum;
- IDNA/UTS #46;
- introducing a user-facing UTS #39 restriction-level policy;
- automatically blocking all mixed-script text;
- source-language tokenization or identifier extraction;
- redesigning JavaScript/TypeScript identifier semantics;
- adding new MCP tools or changing existing ToolSpec metadata;
- adopting `unicode-security` as an authoritative one-line replacement while
  it ships an older Unicode data epoch than Eggsact's current security data;
- weakening UTS #39 semantics to avoid a dependency or generated table.

## 6. Required production changes

### Core/service/adapter

#### 6.1 Split internal skeleton mechanics from public skeleton semantics

Implement one clearly named internal helper equivalent to the UTS #39
internal-skeleton operation for the pinned Unicode 18 epoch:

```text
NFD
-> remove Default_Ignorable_Code_Point
-> map each remaining code point through confusables.txt prototype mapping
-> NFD
```

The exact operation ordering must follow UTS #39 Revision 34, not this
shorthand if the normative algorithm contains additional conditions.

Then make the existing public `confusable_skeleton()` implement UTS #39
`skeleton(X) = bidiSkeleton(LTR, X)`.

`are_confusable()` must compare the corrected public skeletons. Preserve its
application-level "distinct raw strings only" behavior unless normative review
shows that behavior is materially misleading; if so, add a lower-level
skeleton-equality helper rather than silently changing unrelated callers.

Do not use display-oriented string rendering as a substitute for the normative
bidi-skeleton algorithm. Implement the algorithm's specified directional
status, UBA processing, mirroring, and normalization sequence.

#### 6.2 Add version-correct supporting property data

Current ecosystem review provides useful components but not a drop-in
Unicode-18 UTS #39 implementation:

- `unicode-security` 0.1.2 exposes skeleton and mixed-script helpers, but its
  documented data epoch is older than Eggsact's Unicode 18 security data and
  cannot be the authoritative implementation for this corrective;
- `unicode-script` 0.5.8 exposes Script and Script_Extension APIs and is a
  useful reference/candidate, but its exact `UNICODE_VERSION` must be
  verified against the required Unicode 18 semantics before adoption;
- `unicode-bidi` 0.3.18 provides the UAX #9 algorithm and a data-source
  abstraction with an MSRV below Eggsact's, but its bundled Unicode data epoch
  must be verified before use;
- `unicode-bidi-mirroring` 0.4.0 is known to ship Unicode 16 mirroring data
  and therefore must not be used as the authoritative Unicode 18 mirroring
  table.

Preferred approach: generate the small security-relevant Unicode 18 property
tables Eggsact needs from checksum-pinned authoritative UCD inputs and use a
well-tested algorithm crate only where its algorithm can consume version-correct
data. Candidate inputs include:

- `DerivedCoreProperties.txt` for Default_Ignorable_Code_Point;
- `Scripts.txt` and `ScriptExtensions.txt` for Script / Script_Extensions;
- `DerivedBidiClass.txt`, `BidiBrackets.txt`, and
  `BidiMirroring.txt` as required by the chosen UAX #9/bidi-skeleton path.

Do not generate a broad duplicate Unicode database. Keep generated data limited
to properties actually required by the normative algorithms.

If a crate is verified to ship the exact needed Unicode 18 property epoch with
acceptable MSRV/license/footprint, using it is preferable to duplicating that
property table.

#### 6.3 Implement resolved script sets

Replace `is_legitimate_mixture()` as the security decision with an
authoritative resolved-script-set operation:

1. obtain each character's Script_Extensions set;
2. apply the UTS #39 augmented-script rules for Unicode 18, including
   Jpan/Kore/Hanb/Hntl relationships;
3. treat Common/Inherited according to the specification;
4. intersect augmented sets over the string;
5. report mixed-script only when the resolved script set is empty.

Retain `script_name()`/per-character display information where it is useful
for diagnostics, but do not derive the security verdict from the old range
table.

If downstream DTOs currently return a simple list of observed scripts, preserve
that output shape and separately derive the mixed-script boolean from the
resolved set. Do not serialize pseudo-script implementation details unless an
additive field is justified and documented.

#### 6.4 Fix Rust validation parity

Refactor Rust identifier validity/keyword evaluation into one shared typed
helper and use it from both:

- `identifier_analyze()`;
- `identifier_inspect()`.

For `identifier_inspect(language = "rust")`, normalized identifiers must use
Rust XID_Start/XID_Continue syntax and reserved-keyword handling rather than the
current fallthrough.

Keep `is_valid_rust_identifier_ascii()` as an explicit stricter helper where
needed; do not restore ASCII-only language validity.

#### 6.5 Finish typed hazard consolidation

Make `unicode_policy` and `identifier` delegate security-sensitive
invisible/bidi/join/default-ignorable membership to the central typed property
layer.

Presentation helpers such as display labels may remain local. Security verdicts
must not maintain parallel static membership lists.

Default-Ignorable is a Unicode property, not synonymous with Eggsact's
"user-visible invisible hazard" category. Model these separately where the
standard requires it; do not simply classify every Default_Ignorable as the same
user-facing hazard.

### Registry, profile, audience, surface

No changes. If the implementation appears to require a new tool/profile field,
stop and report.

### Protocol and DTOs

No breaking DTO change.

Existing findings may change because standards-correct skeleton/script
semantics can add previously missed detections or remove false mixed-script
findings. Treat those as correctness changes and fixture them explicitly.

### Runtime and concurrency

Property lookups and bidi/skeleton processing must be deterministic,
thread-safe, and bounded by existing text limits.

Avoid per-call construction of large hash maps. Generated range tables should
use binary-search/range lookup or another compact deterministic representation.

Any UAX #9 helper must not assume terminal locale or actual display width.

### Operator surface

No CLI/update/integrate change.

### Documentation and static guards

Update:

- `architecture/text-library.md`;
- `architecture/generated-assets.md`;
- `docs/library-api.md`;
- `docs/fuzzing.md`;
- `CHANGELOG.md`.

Correct wording that currently describes NFD + mapping + NFD as the complete
UTS #39 skeleton.

Generated property assets must include:

- Unicode version;
- authoritative source URL(s);
- SHA-256 for each source;
- generation command;
- entry/range counts where useful.

Extend the generator check/self-test approach from
`scripts/generate_confusables.py` rather than adding unrelated generation
conventions.

## 7. Ordered work packages

### Work package A — Pin conformance fixtures before implementation

Intent:

Create tests that fail for the known standards gaps before changing production
logic.

Required changes:

- add Default-Ignorable skeleton cases, including variation selectors and
  representative join/format characters mandated by UTS #39;
- add bidi-skeleton fixtures containing RTL text and mirrored punctuation;
- add current UTS #39 mixed-script examples/resolved-script-set cases,
  including:
  - plain Latin single-script;
  - Cyrillic single-script;
  - Latin/Cyrillic spoof mixture;
  - Common characters that must not create a false mixture;
  - Japanese/Han cases resolved through augmented script sets;
  - Korean cases that distinguish mixed-script detection from restriction-level
    permissibility;
  - Hntl-sensitive cases introduced/clarified in Unicode 18;
- add invalid Rust `identifier_inspect(language="rust")` cases and valid
  non-ASCII XID cases.

Acceptance evidence:

Tests demonstrate at least one current false negative for skeleton semantics,
one current mixed-script misclassification, and one invalid Rust identifier
currently accepted by `identifier_inspect`.

### Work package B — Unicode 18 property generation/provenance

Intent:

Supply the exact property data required by UTS #39 without regressing to older
crate data.

Required changes:

- qualify candidate crate data epochs first;
- generate only missing Unicode 18 property tables from authoritative,
  checksum-pinned UCD files;
- add strict parser validation, duplicate/range/scalar checks, and offline
  self-tests;
- add maintainer `--check` or equivalent freshness verification;
- ensure no normal build/test performs network I/O.

Acceptance evidence:

Generated assets reproduce byte-for-byte, malformed miniature fixtures fail
closed, and code/doc provenance reports the exact provider epoch for every
security-semantic property.

### Work package C — Standards-correct skeleton and bidi skeleton

Intent:

Make `confusable_skeleton` match its UTS #39 claim.

Required changes:

- implement internal skeleton with Default-Ignorable removal;
- implement the UTS #39 Revision 34 bidiSkeleton path for LTR direction;
- apply version-correct bidi class/bracket/mirroring data;
- preserve Unicode 18 confusables mappings;
- make `are_confusable` consume the corrected public skeleton;
- avoid retaining an old "fast path" that has different semantics.

Acceptance evidence:

- variation-selector/default-ignorable fixtures collapse as required;
- RTL/mirrored fixtures match normative/reference vectors;
- existing `apple`/Cyrillic-`аpple`, `Æ`/AE, and non-collision fixtures
  still behave correctly;
- skeleton determinism is preserved;
- any claimed idempotence property is retained only if the current standard
  guarantees it and the normative vectors confirm it.

### Work package D — Resolved Script_Extensions semantics

Intent:

Replace policy allowlists with the UTS #39 resolved script-set algorithm.

Required changes:

- implement Script_Extensions lookup and augmented script sets;
- implement Unicode 18 Jpan/Kore/Hanb/Hntl augmentation exactly;
- replace `is_legitimate_mixture()` in security decisions;
- keep diagnostic per-character script reporting stable where practical;
- make `unicode_tools`, `unicode_policy`, and identifier analysis consume
  one resolved-script implementation.

Acceptance evidence:

Conformance fixtures match UTS #39 examples and demonstrate that
restriction-level-friendly combinations are not automatically redefined as
single-script.

### Work package E — Rust validation and hazard ownership closure

Intent:

Close adjacent correctness/drift defects while the Unicode substrate is open.

Required changes:

- add the Rust branch to `identifier_inspect` via shared XID/keyword helper;
- remove security decisions based on `unicode_policy`'s private invisible
  HashSet and identifier's private invisible list;
- route bidi/join/invisible decisions through typed helpers;
- preserve presentation-specific labels and DTOs.

Acceptance evidence:

Cross-consumer differential tests show consistent classification; invalid Rust
identifiers cannot pass `identifier_inspect(language="rust")`.

### Work package F — Documentation, compatibility, and closure reconciliation

Intent:

Align claims with the corrected implementation without rewriting historical
closure records.

Required changes:

- update architecture/library/fuzzing docs and CHANGELOG;
- keep 003/004 closure records immutable;
- write a new 005 closure record with the standards-conformance evidence;
- update roadmap/registry to guard-only only after 005 closes;
- record any output changes caused by normative fixes.

Acceptance evidence:

No document claims full UTS #39 conformance for a narrower internal operation;
the active registry accurately reflects 005 until closure.

## 8. Failure, cancellation, truncation, and contention semantics

- Inputs remain bounded by the existing 100k text/tool limits.
- Generated-data validation is fail-closed and atomic: no partial generated
  output on source/checksum/parser failure.
- Bidi processing must reject or deterministically handle unsupported internal
  conditions; never fall back silently to the old non-bidi skeleton.
- Cancellation behavior of composite security inspection remains unchanged.
- Property tables are immutable after process start.
- No cache correctness may depend on request order.
- If a candidate dependency's data epoch cannot meet Unicode 18 semantics,
  stop and use a pinned generated-data path rather than mixing epochs silently.

## 9. Compatibility and migration

This corrective preserves the 1.x API surface while correcting semantics behind
newly added Unicode-security APIs.

Expected observable changes:

- `confusable_skeleton()` outputs can change for Default-Ignorable and bidi
  inputs;
- `are_confusable()` can gain true-positive RTL/default-ignorable pairs;
- mixed-script booleans can change where the hand allowlists disagreed with
  resolved Script_Extensions;
- invalid Rust identifiers can switch from `valid=true` to `false` in
  `identifier_inspect`.

These are correctness fixes, not schema breaks. Record representative before /
after fixtures in the closure.

Do not change `CONFUSABLES` per-character lookup contents to compensate for
skeleton behavior; Milestone 004 data qualification remains valid.

## 10. Required tests

### Focused unit tests

- Default_Ignorable property boundaries and representative members/nonmembers;
- internal skeleton ordering;
- public bidi skeleton LTR vectors;
- mirroring/bracket behavior;
- Script_Extensions lookup;
- augmented Jpan/Kore/Hanb/Hntl sets;
- resolved-script intersection;
- Rust XID/keyword shared helper;
- central hazard membership.

### Integration tests

- `identifier_inspect` Rust valid/invalid matrix;
- `unicode_policy_check` mixed-script matrix;
- `text_measure` / `text_security_inspect` bidi/invisible consistency;
- identifier collision tests with default ignorables and RTL examples;
- unchanged ToolSpec/profile/audience/registration tests.

### Restart and recovery tests

No persisted state. If generated tables use LazyLock indexes, exercise fresh
initialization and deterministic repeated calls.

### Contention and cancellation tests

- concurrent skeleton/resolved-script calls if lazy indexes are added;
- existing security composite cancellation tests remain green.

### Negative and boundary tests

- empty/ASCII inputs;
- max-bound input;
- combining-mark-heavy text;
- variation selectors;
- ZWJ/ZWNJ/default ignorables;
- malformed generated Unicode property data;
- supplementary-plane scripts;
- bidi embeddings/isolates and mirrored punctuation;
- strings made only of Common/Inherited characters.

### Property/fuzz tests

Extend `unicode_inspection` and property tests to assert:

- valid policy/profile paths still execute;
- internal/public skeleton determinism;
- resolved script-set determinism;
- Common/Inherited handling;
- no panic for arbitrary valid UTF-8 and max-bounded generated inputs;
- central hazard predicates agree across consumers.

Do not assert a skeleton algebraic property unless it is guaranteed by UTS #39
Revision 34.

## 11. Required verification commands

```bash
cargo fmt --all -- --check
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --test lib text
cargo test --locked --test lib property
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
cargo deny --locked check
cargo tree --locked -e normal
```

Also run:

- every new Unicode-property generator `--self-test` and `--check` command;
- existing `python3 scripts/generate_confusables.py --self-test`;
- existing `python3 scripts/generate_confusables.py --check`;
- parity when `../eggcalc` is available.

If dependencies are added:

- verify their exact `UNICODE_VERSION` or source-data epoch;
- verify MSRV and licenses;
- record transitive dependency delta;
- compare release binary size on the same host/toolchain.

Remote CI must be green before closure.

## 12. Documentation updates

- `architecture/generated-assets.md`: new UCD property sources/checksums and
  exact provider epochs;
- `architecture/text-library.md`: distinguish internal skeleton,
  bidiSkeleton, public skeleton, resolved script set, and restriction levels;
- `docs/library-api.md`: corrected semantics for
  `confusable_skeleton`/`are_confusable`;
- `docs/fuzzing.md`: new standards-conformance invariants;
- `CHANGELOG.md`: observable correctness changes;
- comments in `src/text/script.rs` and `confusables.rs` that currently
  overstate/approximate the standard.

## 13. Acceptance criteria

Milestone 005 may close only when:

- `confusable_skeleton()` implements UTS #39 Revision 34
  `skeleton(X) = bidiSkeleton(LTR, X)`;
- the internal skeleton removes Default_Ignorable_Code_Point as specified;
- at least one previous variation-selector/default-ignorable false negative is
  fixed by regression test;
- at least one RTL/mirrored confusable case is covered by normative/reference
  evidence;
- mixed-script detection uses Unicode 18 Script_Extensions + augmented
  resolved-script-set semantics rather than allowlists;
- Hntl/Jpan/Kore/Hanb augmentation is covered;
- Rust `identifier_inspect` shares the correct XID + keyword validity path;
- security-sensitive Unicode membership has one typed source of truth;
- Unicode 18 confusables provenance remains unchanged;
- no ToolSpec/profile/audience/surface/schema break occurred;
- focused tests, property/fuzz guards, parity (when available), cargo-deny,
  merge gate, and remote CI are green;
- a new 005 closure record explicitly reconciles the residual findings from
  the post-004 audit.

## 14. Stop conditions

The agent must stop and report rather than improvise when:

- UTS #39 Revision 34 requires a public policy decision beyond correcting the
  existing claimed skeleton/mixed-script semantics;
- version-correct UAX #9 processing cannot be achieved with a small dependency
  and/or bounded generated property data;
- a candidate Unicode crate's data epoch is unknown or older than required for
  a security-semantic property and no authoritative generated fallback is
  practical;
- implementing bidi skeleton would require a large shaping/layout stack rather
  than the Unicode algorithm/data actually required by UTS #39;
- generated Unicode data materially bloats the binary and a more compact
  standards-correct representation has not been evaluated;
- fixing Rust identifier inspection would require changing a documented schema
  rather than only correcting the validation result;
- repository evidence after the baseline materially changes these findings.

## 15. Closure evidence required

The 005 closure record must contain:

- implementation commit(s);
- exact UTS #39/UAX #24/UAX #9 versions used;
- Unicode provider/data-epoch inventory after the corrective;
- source URLs and SHA-256 checksums for every new generated Unicode input;
- requirement-to-evidence matrix for work packages A-F;
- normative/reference skeleton vectors, including Default-Ignorable and RTL /
  mirrored cases;
- resolved-script-set examples including Hntl/Jpan/Kore/Hanb;
- before/after evidence for the Rust `identifier_inspect` bug;
- cross-consumer hazard consistency evidence;
- focused/property/fuzz/merge-gate results;
- parity result or explicit unavailability;
- cargo-deny/MSRV/dependency and binary-size evidence for any dependency delta;
- remote CI run ID;
- residual findings by severity;
- recommendation: closed, conditionally closed, corrective pass required, or
  blocked.

## 16. Handoff notes

Do not rewrite the 003 or 004 closure files. They remain historical records of
what was believed closed at those baselines. Milestone 005 is the corrective
trace.

Start with failing conformance fixtures before choosing dependencies. In
particular, do not adopt `unicode-security` solely because it exposes the
desired API names; its documented Unicode data epoch does not match the
repository's Unicode 18 security-data baseline.

Prefer a compact generated Unicode 18 property layer plus proven algorithm
machinery over another hand-maintained range table. Preserve all unrelated user
changes and retain `--test-threads=4` for the integration gate.
