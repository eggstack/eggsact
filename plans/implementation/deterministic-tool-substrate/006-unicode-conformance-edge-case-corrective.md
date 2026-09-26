# Deterministic Tool Substrate Milestone 006 — Unicode Conformance Edge-Case Corrective

Status: closed

Repository baseline: `6c6a47890f2bfa27922f85834c3e641d12745e21`

Implemented in `83de61a`; closed in `plans/closure/deterministic-tool-substrate/006-status.md`.

Source roadmap:

- `plans/subsystems/deterministic-tool-substrate-roadmap.md#7`

Corrects:

- `plans/implementation/deterministic-tool-substrate/005-unicode-security-standards-conformance-corrective.md`
- `plans/closure/deterministic-tool-substrate/005-status.md`

Historical dependencies:

- Milestone 003 Unicode security semantic correctness hardening — closed
- Milestone 004 Unicode 18 security data qualification — closed
- Milestone 005 Unicode security standards-conformance corrective — closed historical evidence

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
- Unicode Standard Annex #9, Unicode Bidirectional Algorithm, Version 18.0.0,
  Revision 52: <https://www.unicode.org/reports/tr9/tr9-52.html>
- Unicode Standard Annex #24, Unicode Script Property, Version 18.0.0,
  Revision 41: <https://www.unicode.org/reports/tr24/tr24-41.html>
- Unicode Standard Annex #44, Unicode Character Database, Version 18.0.0:
  <https://www.unicode.org/reports/tr44/tr44-36.html>
- Unicode 18.0.0 `DerivedBidiClass.txt`:
  <https://www.unicode.org/Public/18.0.0/ucd/extracted/DerivedBidiClass.txt>
- `unicode-bidi` 0.3.18 `BidiInfo::reordered_levels_per_char` contract:
  <https://docs.rs/unicode-bidi/0.3.18/unicode_bidi/struct.BidiInfo.html>

## 1. Objective

Correct three residual Unicode-conformance defects discovered after the
Milestone 005 closure, without reopening the successfully landed Unicode 18
data pipeline or changing any public MCP/tool surface:

1. honor the ordered `@missing` defaults in Unicode 18
   `DerivedBidiClass.txt` instead of treating every explicit-data miss as
   `Bidi_Class=L`;
2. make `bidi_skeleton_ltr()` apply L1/L2 levels paragraph-locally for
   multi-paragraph text rather than falling back to all-LTR levels when
   `unicode-bidi` returns a whole-text per-character vector;
3. treat `Script_Extensions={Unknown}` / `Zzzz` as a real augmented-script
   set, not as the `ALL` identity used only for Common/Inherited.

The corrective must preserve the core Milestone 005 architecture: Unicode 18
generated property data, `unicode-bidi` as algorithm-only machinery with
Eggsact's custom data source, UTS #39 internal/public skeleton split,
Script_Extensions-based resolved sets, shared Rust XID validation, and typed
hazard ownership.

## 2. Why this milestone is ready

The remaining failures are bounded implementation defects with normative
answers. No new product policy, dependency selection, wire contract, or
architecture decision is required.

Authoritative evidence is explicit:

- UAX #44 defines machine-readable `@missing` lines as property defaults and
  states that later `@missing` ranges override earlier ones. Bidi_Class uses
  complex defaults.
- Unicode 18 `DerivedBidiClass.txt` begins with a global L default and then
  overrides ranges for R, AL, ET, and other context-specific defaults. The
  file explicitly states that unassigned code points in RTL-script blocks and
  Currency Symbols do not all default to L.
- UAX #9 requires separation into paragraphs and applies the algorithm
  independently within each paragraph.
- `unicode-bidi` documents that `reordered_levels_per_char()` returns a
  vector containing characters outside the requested line/range; the current
  Eggsact length-equality assumption is therefore invalid for multi-paragraph
  input.
- UTS #39 treats augmented sets containing `Zyyy` (Common) or `Zinh`
  (Inherited) as ALL. It does not include `Zzzz` (Unknown).
- UAX #24 defines `Script_Extensions` for unassigned/private-use/noncharacter
  code points as the non-empty singleton set `{Unknown}`.

Milestone 005's closure record remains immutable historical evidence. Milestone
006 is the corrective trace required by `plans/003-planning-process.md#7`.

## 3. Why Milestone 005 verification did not catch these defects

The 005 verification was substantial but its fixtures did not exercise these
specific boundaries:

- Bidi property tests covered explicitly listed R/AL/L/etc. characters, not
  code points whose Bidi_Class comes only from `@missing` defaults.
- The UTS #39 S1/S2 skeleton vector is a single paragraph. No regression case
  used two or more paragraphs with RTL content after the first separator, so
  the whole-text/per-paragraph vector-length mismatch was not exposed.
- Script conformance fixtures covered Common, Inherited, Latin/Cyrillic,
  Japanese, Korean, Hanb, and Hntl, but not `Script_Extensions={Unknown}`
  using a private-use/unassigned/noncharacter code point.

The new tests must pin all three failure classes before production changes.

## 4. Current implementation evidence

At baseline `6c6a47890f2bfa27922f85834c3e641d12745e21`:

### 4.1 Bidi_Class defaults

`scripts/generate_unicode_security_properties.py::parse_bidi_class()` skips
all comment lines before parsing data rows. Because `@missing` directives are
comment lines, none are retained.

`src/text/unicode_properties.rs::bidi_class_name()` performs a binary search
over explicit `BIDI_CLASS_RANGES` and then does:

```rust
.unwrap_or("L")
```

This is correct only for the global default, not for the ordered override
ranges in Unicode 18 `DerivedBidiClass.txt`.

The error affects both:

- the custom `Unicode18BidiData` supplied to `unicode-bidi`;
- `is_bidi_r_or_al()`, which controls UTS #39's permitted fast path.

A default-only R/AL code point can therefore be classified L and cause the
public skeleton to bypass required bidi processing.

### 4.2 Multi-paragraph level handling

`bidi_skeleton_ltr()` creates one `BidiInfo` for the complete input, then
loops over `info.paragraphs`.

For each paragraph it calls:

```rust
info.reordered_levels_per_char(para, para.range.clone())
```

and assumes that the returned vector contains exactly one level per character
in that paragraph.

`unicode-bidi` 0.3.18 documents the opposite: the returned vector includes
characters outside the requested line/range, although those levels are not
adjusted. Its implementation obtains the per-character vector by iterating the
full text.

The current length guard therefore falls into the all-LTR fallback for a
paragraph whenever the whole-text character count differs from the paragraph
character count. That can suppress correct L2 ordering and L4 mirroring in
later RTL paragraphs.

### 4.3 Unknown Script_Extensions

`script_extensions()` correctly falls back to `{Zzzz}` for an unlisted
code point.

`augmented_script_set()`, however, currently treats `Zzzz` as if it were
Common/Inherited and returns the same empty marker used to represent ALL.

Under UTS #39 only sets containing `Zyyy` or `Zinh` become ALL. `Zzzz`
remains `{Zzzz}`.

Consequences at baseline:

- an Unknown-only string is treated as unconstrained rather than resolving to
  `{Zzzz}`;
- Latin + private-use/Unknown can avoid a mixed-script verdict because Unknown
  is skipped as an identity element rather than intersected.

This affects the mixed-script predicate only; UTS #39 identifier-status policy
for unassigned/private-use characters remains a separate concern and is out of
scope.

## 5. Invariants that must not regress

- One Rust crate; tracked `Cargo.lock`; MSRV 1.89.0; `--locked` gates.
- No new dependency is expected or justified for this corrective.
- Keep `unicode-bidi = 0.3.18` algorithm-only with hardcoded data disabled.
- Keep all Unicode security/UCD property sources pinned to Unicode 18.0.0 and
  their existing authoritative SHA-256 values.
- Keep `confusables.txt` at Unicode 18.0.0 with 6,712 mappings and its
  existing checksum.
- Preserve public functions and signatures introduced in 005:
  `internal_skeleton`, `bidi_skeleton_ltr`, `confusable_skeleton`,
  `are_confusable`, `resolved_script_set`, `is_mixed_script`, and typed
  hazard helpers.
- Preserve the 86-tool/23-category ToolSpec registry, order, schemas, profiles,
  audiences, exposure, machine codes, and direct/discovery behavior.
- No runtime network, locale, clock, environment, or filesystem dependence.
- Generated data remains checked in and normal builds/ordinary CI remain
  offline.
- Existing text/request/output bounds, cancellation semantics, and concurrency
  behavior remain unchanged.
- Do not rewrite the 003/004/005 closure records.

## 6. Scope

### In scope

- parsing and validating `DerivedBidiClass.txt` `@missing` directives;
- ordered `@missing` override semantics;
- source-driven alias normalization for long Bidi_Class names appearing in
  `@missing` rows;
- compact generation of Bidi_Class default/override ranges;
- runtime explicit-row-first + ordered-default fallback;
- generator self-tests for generic + overridden `@missing` ranges;
- authoritative default-only R, AL, ET, and global-L regression cases;
- paragraph-local L1/L2 level extraction for arbitrary multi-paragraph input;
- LF, CRLF, U+2029, leading/trailing separator, and multiple RTL-paragraph
  tests where applicable;
- removal of the silent all-LTR mismatch fallback;
- correct `Zzzz` augmented/resolved-script behavior;
- private-use/Unknown mixed-script fixtures;
- property/fuzz coverage and documentation/CHANGELOG updates;
- planning/closure reconciliation after implementation.

### Explicitly out of scope

- changing Unicode/UCD/confusables source versions;
- changing the `unicode-bidi` dependency;
- implementing a separate bidi engine;
- new restriction-level or Identifier_Status policy;
- IDNA/UTS #46;
- rejecting private-use/unassigned characters globally;
- changing Rust/JavaScript/Python identifier syntax;
- new MCP tools, schemas, profile membership, or machine codes;
- reworking L3/L4 beyond defects demonstrated by this corrective;
- changing 005's generated Script/Script_Extensions source files except where
  regenerated output changes mechanically because generator metadata/counts
  are extended.

## 7. Required production changes

### 7.1 Parse and preserve Bidi_Class `@missing` defaults

Extend `generate_unicode_security_properties.py` with a dedicated strict
parser for the machine-readable Unicode `@missing` convention.

Requirements:

- recognize only valid lines matching the UAX #44 `@missing` syntax;
- parse ranges with the same scalar/range validation discipline used by
  ordinary property rows;
- normalize long Bidi_Class values such as `Left_To_Right`,
  `Right_To_Left`, `Arabic_Letter`, and `European_Terminator` to the
  validated short aliases used by runtime code;
- prefer deriving Bidi_Class aliases from the already-pinned
  `PropertyValueAliases.txt` rather than adding an unvalidated second
  hand-maintained vocabulary;
- preserve `@missing` source order because later directives override earlier
  directives;
- reject malformed ranges, unknown values, or ambiguous parser states;
- do not treat arbitrary comments containing the text `@missing` as valid
  directives.

Generate a compact table such as:

```rust
pub static BIDI_CLASS_DEFAULT_RANGES: &[(u32, u32, &str)] = ...;
```

Do not expand the entire Unicode scalar space into per-character entries.

Runtime lookup order must be:

1. exact/explicit `BIDI_CLASS_RANGES` lookup;
2. matching `BIDI_CLASS_DEFAULT_RANGES` lookup with last matching
   `@missing` directive winning;
3. internal invariant failure if no default covers a valid Rust `char`.

Do not silently retain `unwrap_or("L")` as a semantic fallback. The global
Unicode `@missing: 0000..10FFFF; Left_To_Right` directive must provide L.

A linear reverse scan of the small default table is acceptable and likely
preferable to complicating the generated representation. If a different
representation is chosen, preserve ordered override semantics and document it.

### 7.2 Fix paragraph-local bidi level extraction

Remove the current assumption that
`reordered_levels_per_char(para, range)` returns paragraph-length output.

Use one of these bounded approaches:

- slice/filter the whole-text per-character level vector by the paragraph byte
  range with a correct byte-to-character mapping; or
- process each paragraph substring independently with
  `BidiInfo::new_with_data_source(..., Some(Level::ltr()))`, preserving UAX
  #9 paragraph-separator semantics.

Whichever implementation is chosen must:

- apply L1 and L2 only to characters in that paragraph;
- retain the forced LTR paragraph level required by UTS #39
  `bidiSkeleton(LTR, X)`;
- preserve paragraph separators in the correct logical relationship to the
  preceding paragraph;
- feed the correct resolved level for each visual-order character to L4
  mirroring;
- not replace a mismatch with all-LTR levels;
- not let characters from one paragraph affect another.

If an internal invariant is violated after the fix, fail loudly in tests/debug
rather than silently producing a weaker skeleton.

### 7.3 Correct Unknown/Zzzz resolved-script semantics

Change `augmented_script_set()` so:

- a set containing `Zyyy` or `Zinh` still returns/represents ALL;
- `Zzzz` remains the ordinary singleton set `{Zzzz}`;
- a `Script_Extensions={Unknown}` character therefore constrains the resolved
  intersection like any other non-ALL set.

Expected semantics include:

- private-use U+E000 alone resolves to `{Zzzz}` and is not mixed-script;
- Common + U+E000 also resolves to `{Zzzz}`;
- Latin + U+E000 has an empty resolved set and is mixed-script;
- two Unknown characters remain single-script under this predicate;
- any separate policy that classifies private-use/unassigned characters as
  restricted remains independent and unchanged.

Do not special-case Unknown as suspicious inside the mixed-script algorithm;
implement the standard intersection literally.

### 7.4 Documentation and static guards

Update:

- `architecture/generated-assets.md` with Bidi_Class `@missing` parser and
  precedence semantics;
- `architecture/text-library.md` with paragraph-local bidi processing and
  Unknown resolved-set behavior;
- `docs/library-api.md` only if current API semantics need clarification;
- `docs/fuzzing.md` for new edge-case invariants;
- `CHANGELOG.md` with representative correctness changes.

Generated metadata should expose/count the default ranges so silent loss of
`@missing` directives becomes detectable.

No ToolSpec documentation regeneration should produce changes; run
`generate-docs --check` as a no-surface-drift guard.

## 8. Ordered work packages

### Work package A — Pin failing regression fixtures

Intent:

Demonstrate all three current defects before production edits.

Required tests:

1. Bidi defaults:
   - one code point whose class is supplied only by an R `@missing` range;
   - one supplied only by an AL `@missing` range;
   - one supplied only by ET `@missing`;
   - one ordinary global-L default;
   - tests must prove the chosen code points are absent from the explicit parsed
     Bidi_Class rows so they actually exercise defaults.
2. Multi-paragraph skeleton:
   - first paragraph LTR, second paragraph containing R/AL content that requires
     L2 visual reordering;
   - at least one case where an odd resolved level requires L4 mirroring;
   - LF plus one additional paragraph boundary representation supported by
     UAX #9/library behavior (CRLF and/or U+2029);
   - a three-paragraph deterministic case to prevent a "second paragraph only"
     fix.
3. Unknown script:
   - `resolved_script_set("\u{E000}") == {"Zzzz"}`;
   - Common + U+E000 remains `{Zzzz}`;
   - Latin + U+E000 is mixed;
   - Unknown + Unknown is not mixed.

Acceptance evidence:

Each test must fail for the intended reason against baseline
`6c6a47890f2bfa27922f85834c3e641d12745e21`; record those before/after observations in the 006 closure.

### Work package B — Bidi `@missing` generation and lookup

Intent:

Make Eggsact's Unicode 18 Bidi_Class table complete according to UAX #44/UCD.

Required changes:

- parse Bidi_Class aliases from pinned property aliases;
- parse ordered `@missing` directives;
- render/check their generated representation;
- update provenance/count constants;
- make `bidi_class_name()` explicit-first then ordered-default;
- make `is_bidi_r_or_al()` inherit the corrected result;
- add generator self-tests with overlapping/default override examples.

Acceptance evidence:

- the global + overriding defaults reproduce the Unicode 18 semantics;
- `--self-test` covers later-default-wins;
- `--check` reproduces the checked-in file byte-for-byte;
- default-only R/AL/ET test points classify correctly;
- no runtime fallback invents L.

### Work package C — Paragraph-local bidi skeleton

Intent:

Eliminate cross-paragraph indexing and all-LTR fallback behavior.

Required changes:

- scope L1/L2 levels to each paragraph correctly;
- keep visual index maps aligned to paragraph-local characters;
- preserve separators;
- apply L3/L4 with the paragraph's actual resolved levels;
- remove the current length-mismatch fallback.

Acceptance evidence:

Multi-paragraph fixtures agree with independent per-paragraph reference
construction and remain deterministic. Existing single-paragraph UTS #39 S1/S2
fixtures stay unchanged and green.

### Work package D — Unknown resolved-script correction

Intent:

Implement UTS #39 augmented/resolved sets exactly for Unknown.

Required changes:

- remove the `Zzzz -> ALL` treatment;
- preserve `Zyyy/Zinh -> ALL`;
- update any comments/docs claiming Unknown does not constrain;
- keep diagnostic `script_of(...)=Other/Unknown` compatibility where the
  public DTO already uses those spellings.

Acceptance evidence:

Private-use/Unknown test matrix passes and all existing Jpan/Kore/Hanb/Hntl,
Common, Inherited, Latin/Cyrillic fixtures remain green.

### Work package E — Qualification and closure

Intent:

Prove the corrective is bounded and did not disturb the successful 005 work.

Required changes/evidence:

- run focused Unicode conformance/property suites;
- run both Unicode generators' self-test/check paths;
- run parity when available;
- run the full ordered merge gate;
- record remote CI;
- update docs/CHANGELOG;
- create `plans/closure/deterministic-tool-substrate/006-status.md`;
- only after closure, return the substrate to guard-only.

No dependency/MSRV/binary-size comparison is required if the dependency graph
and generated-data footprint change only trivially. If generated
`@missing` representation materially increases release size, record a
same-host before/after measurement and justify the representation.

## 9. Failure, cancellation, truncation, and contention semantics

- Generator parse/check remains fail-closed and writes no partial output.
- A malformed or unknown Bidi_Class `@missing` value is a generation error.
- A valid runtime `char` missing both explicit and default Bidi_Class coverage
  is an internal invariant violation, not permission to assume L.
- Multi-paragraph handling must never degrade silently to all-LTR behavior.
- Existing 100k tool text bounds remain the resource boundary.
- Runtime work remains linear/bounded in text length plus tiny static default
  range lookup.
- Static tables remain immutable and safe under concurrency.
- Cancellation behavior in composite security services is unchanged.

## 10. Compatibility and migration

No schema or API migration is expected.

Observable correctness changes are limited to edge cases:

- default-only Unicode code points in RTL/Currency ranges can acquire the
  correct Bidi_Class and trigger correct bidi skeleton processing;
- multi-paragraph RTL skeleton outputs can change from the previous erroneous
  all-LTR fallback;
- strings mixing Unknown/Private-Use with an explicit script can newly report
  mixed-script.

These are standards corrections behind existing APIs.

Do not change confusables data, identifier policy, or security severity solely
because these outputs change.

## 11. Required tests

### Generator/unit tests

- `@missing` syntax parsing;
- long Bidi_Class alias normalization;
- source-order override precedence;
- malformed `@missing` rejection;
- explicit-row precedence over defaults;
- generated default-range count/provenance;
- runtime default-only R/AL/ET/L lookup.

### Skeleton tests

- existing UTS #39 S1/S2;
- two and three paragraphs;
- LF;
- CRLF and/or U+2029 as supported by the library/UAX behavior;
- later RTL paragraph L2 ordering;
- later RTL paragraph L4 mirroring;
- paragraph independence;
- deterministic repeated calls.

### Script tests

- Unknown-only;
- Common + Unknown;
- Latin + Unknown;
- Unknown + Unknown;
- existing Common/Inherited identity;
- existing Table 1a, Jpan/Kore/Hanb/Hntl matrix.

### Property/fuzz tests

Extend the Unicode property/fuzz suites with:

- default-only Bidi_Class seeds;
- multi-paragraph RTL seeds;
- private-use/Unknown seeds;
- deterministic classification/skeleton/resolved-set assertions.

Do not add unbounded generated test loops to ordinary CI. Generator coverage
checks should prove range semantics structurally.

## 12. Required verification commands

Run focused checks first:

```bash
cargo test --locked --test lib test_unicode_conformance_006
cargo test --locked --test lib text
cargo test --locked --test lib property
python3 scripts/generate_unicode_security_properties.py --self-test
python3 scripts/generate_unicode_security_properties.py --check
python3 scripts/generate_confusables.py --self-test
python3 scripts/generate_confusables.py --check
```

Then the repository merge gate:

```bash
cargo fmt --all -- --check
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
```

Also run:

```bash
cargo deny --locked check
cargo tree --locked -e normal
```

Run parity with `../eggcalc` when available. Record remote ordinary CI run ID
before closure.

## 13. Documentation updates

At minimum:

- `architecture/generated-assets.md`;
- `architecture/text-library.md`;
- `docs/fuzzing.md`;
- `CHANGELOG.md`.

Update `docs/library-api.md` only if current wording incorrectly describes
the corrected edge cases.

No generated ToolSpec/profile docs should change.

## 14. Acceptance criteria

Milestone 006 may close only when:

- Unicode 18 Bidi_Class `@missing` directives are parsed, validated,
  generated, and applied with UAX #44 ordered override semantics;
- explicit Bidi_Class rows take precedence over defaults;
- a valid runtime `char` no longer semantically defaults to L merely because
  it lacks an explicit row;
- default-only R, AL, ET, and ordinary-L regression points pass;
- `is_bidi_r_or_al()` recognizes default-only R/AL values;
- `bidi_skeleton_ltr()` uses paragraph-local L1/L2 levels for every
  paragraph and contains no silent all-LTR mismatch fallback;
- multi-paragraph RTL/reordering/mirroring regressions pass;
- `Zzzz` remains `{Zzzz}` in the augmented/resolved set while only
  `Zyyy`/`Zinh` behave as ALL;
- Latin + private-use/Unknown reports mixed-script;
- all 005 S1/S2, Default-Ignorable, Script_Extensions, Rust XID, and hazard
  conformance tests remain green;
- both Unicode generators reproduce checked-in output;
- no dependency, ToolSpec, schema, profile, audience, or machine-code change
  occurred;
- focused suites, merge gate, cargo-deny, parity when available, and remote CI
  are green;
- a 006 closure record documents the baseline failures and corrected evidence.

## 15. Stop conditions

Stop and report rather than improvise when:

- the authoritative Unicode 18 `@missing` semantics cannot be represented
  without changing the existing public property APIs;
- a correct paragraph-local fix would require replacing `unicode-bidi` or
  introducing a new runtime dependency rather than correcting current indexing;
- tests reveal a broader L3/L4 algorithm defect beyond the paragraph-scoping
  issue documented here;
- correcting `Zzzz` materially conflicts with an existing documented public
  mixed-script contract rather than simply changing a correctness result;
- generator regeneration changes unrelated Unicode property tables or source
  checksums;
- repository changes after the baseline materially invalidate these findings.

If a broader standards issue is discovered, close 006 only for the bounded
requirements that are actually satisfied and register a separate corrective;
do not expand this milestone opportunistically.

## 16. Closure evidence required

The 006 closure record must contain:

- implementation commit(s);
- exact UTS #39 / UAX #9 / UAX #24 / UAX #44 versions used;
- before/after failing fixtures for all three defect classes;
- requirement-to-evidence matrix for work packages A-E;
- generated `@missing` range count and unchanged source SHA-256;
- proof of ordered default precedence and explicit-row precedence;
- representative default-only R/AL/ET/L code points;
- multi-paragraph skeleton vectors and paragraph-boundary variants;
- Unknown/private-use resolved-set matrix;
- focused/property/fuzz/merge-gate outcomes;
- both generator self-test/check outcomes;
- parity result or explicit unavailability;
- cargo-deny/dependency graph evidence;
- remote CI run ID;
- residual findings by severity;
- recommendation: closed, conditionally closed, corrective pass required, or
  blocked.

## 17. Handoff notes

Start by adding the failing 006 conformance file. Do not begin by regenerating
the full Unicode table.

For Bidi_Class, the preferred representation is a tiny generated ordered
default table layered beneath the existing explicit binary-search table.
Unicode 18's global L `@missing` plus narrower later overrides makes this both
compact and faithful to UAX #44.

For paragraph handling, read the `unicode-bidi` 0.3.18 method contract before
editing: `reordered_levels_per_char()` includes characters outside the
requested line/range. Correct the indexing/scoping; do not remove the proven
algorithm dependency.

For script resolution, the rule is intentionally simple: only Common
(`Zyyy`) and Inherited (`Zinh`) are ALL. Unknown (`Zzzz`) is an ordinary
non-empty set.

Preserve unrelated user work, especially the separate distribution M005
Eggpack plan currently registered in parallel.
