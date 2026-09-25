# Deterministic Tool Substrate Milestone 004 — Unicode 18 Security Data Qualification

Status: ready for handoff (unblocked by 003 closure `plans/closure/deterministic-tool-substrate/003-status.md`; rebase onto post-003 HEAD before implementing)

Repository baseline: `43971e7c1af7f936acfd876f9bff246e72866f2d` (planning baseline; implementation must rebase after Milestone 003 closure)

Source roadmap:

- `plans/subsystems/deterministic-tool-substrate-roadmap.md#7`

Long-term requirements:

- `plans/000-long-term-specification.md#2`
- `plans/000-long-term-specification.md#4.3`
- `plans/000-long-term-specification.md#4.4`
- `plans/000-long-term-specification.md#5`
- `plans/000-long-term-specification.md#7`
- `plans/001-terminology-and-domain-model.md#4`
- `plans/002-long-term-roadmap.md#phase-0`

Applicable ADRs:

- `plans/adrs/ADR-0001-planning-conventions-adoption.md`

Primary class: infrastructure

Hard dependency:

- Milestone 003 Unicode Security Correctness Hardening must be closed first.

## 1. Objective

After the Unicode-security semantics are corrected and qualified, advance the
checked-in confusables/security data from Unicode 17.0.0 to Unicode 18.0.0
without changing the public tool surface or conflating data-epoch changes with
algorithm changes.

The milestone must make Unicode-version provenance auditable across the
confusables source, normalization/casefold/script dependencies, generated
assets, diagnostics, tests, and documentation. It must not claim that the
entire pipeline implements Unicode 18 unless every relevant property source
supports that statement.

## 2. Why this milestone is blocked

The current collision algorithm and Unicode verification coverage have known
correctness gaps. Updating the source data first would make it difficult to
separate semantic fixes from data changes and could produce false confidence in
newer data processed by old heuristics.

Milestone 003 is therefore a hard dependency.

After 003 closes, the remaining work is bounded: qualify data-provider epochs,
update the pinned Unicode Security source/checksum, regenerate checked-in
assets, update golden fixtures, and record compatibility evidence.

## 3. Current implementation evidence

At the planning baseline:

- `scripts/generate_confusables.py` pins Unicode Security 17.0.0 and a
  SHA-256 checksum;
- `src/text/confusables_generated.rs` identifies Unicode 17.0.0 and is
  checked into the crate;
- `tests/text/test_confusables.rs` asserts the current 6,565-entry table;
- normalization, casefold, names, general-category, segmentation, and future
  script analysis come from independent Rust crates with their own Unicode data
  epochs;
- repository docs currently describe the confusables asset as Unicode 17.0.0;
- ordinary builds do not require network access.

Unicode 18.0.0 is the intended next security-data epoch, but version alignment
must be established from the post-003 dependency graph before implementation.

## 4. Invariants that must not regress

- Milestone 003 behavior and regression tests remain green.
- One crate, MSRV 1.89.0, tracked lockfile, deterministic local runtime.
- No runtime or ordinary-build Unicode download.
- Source bytes are version-pinned and checksum-pinned before generation.
- Generated outputs are never hand-edited.
- Public Unicode/identifier ToolSpecs, schemas, profile/audience membership,
  registry order, and machine codes remain unchanged.
- Existing 1.x per-character lookup APIs remain source-compatible.
- Data provenance must be explicit; never collapse several independent Unicode
  data epochs into one misleading "Unicode version" field.

## 5. Scope

### In scope

- qualification of the Unicode data epoch used by every dependency relevant to
  security semantics;
- intentional update of `confusables.txt` to the Unicode 18.0.0 security
  release with pinned URL/version/checksum;
- regeneration of checked-in confusables assets through the hardened
  generator;
- stable exported/internal provenance constants for the confusables version,
  checksum, and entry count where useful;
- golden/regression updates caused by legitimate Unicode 18 mapping changes;
- documentation and changelog updates;
- compatibility/parity review;
- optional dependency updates only when necessary to align security-property
  data and only after MSRV/license/size qualification.

### Explicitly out of scope

- changing skeleton/policy semantics established in Milestone 003;
- IDNA/UTS #46 implementation;
- inventing local Unicode normalization data if the ecosystem cannot yet
  support a coherent Unicode 18 pipeline;
- adopting ICU or another large runtime Unicode database;
- new MCP tools or profile changes;
- changing security severity policy because the data set changed;
- opportunistic unrelated dependency upgrades.

## 6. Required production changes

### Core/service/adapter

Do not redesign core algorithms. Replace only versioned data/provider inputs
needed for the qualified epoch.

Expose confusables provenance from one source of truth, for example generated
or generator-synchronized constants equivalent to:

- Unicode Security/confusables version;
- source SHA-256;
- generated entry count.

Do not maintain duplicate manually edited version strings across modules.

If Milestone 003 adopted a Script/Script_Extensions dependency or generated
property asset, determine its actual Unicode data version and record it
explicitly. Do the same for normalization, casefolding, Unicode names/general
category where those versions can affect a security result.

### Registry, profile, audience, surface

No changes.

### Protocol and DTOs

No required wire changes. If runtime diagnostics expose Unicode provenance,
make it an additive diagnostics field and keep existing response fields stable.

### Runtime and concurrency

No new runtime I/O or mutable state. Regenerated data remains static.

### Operator surface (CLI, update, integrate)

No required behavior change. `--diagnostics` may report additive Unicode data
provenance if this can be done without changing stable text output contracts;
otherwise keep provenance in library constants/docs.

### Documentation and static guards

Update `architecture/generated-assets.md` with the Unicode 18 source,
checksum, generator command, and provenance rules.

Update any exact table-count assertion to the newly generated count, but retain
independent structural guards (strict sort/no duplicates/representative
mappings) so count equality is not the only integrity check.

Document the versions of independent Unicode-data providers and explicitly
state whether the overall pipeline is fully Unicode-18-aligned or whether only
the confusables asset is Unicode 18.

## 7. Ordered work packages

### Work package A — Qualify the post-003 Unicode data graph

Intent:

Know exactly which Unicode standard version each security-relevant component
implements before changing the confusables pin.

Required changes:

- inventory direct/transitive versions for normalization, casefolding,
  identifier XID, Script/Script_Extensions, general category, names, and
  segmentation as applicable;
- verify their documented Unicode data epochs from authoritative crate/project
  metadata;
- classify each dependency as security-semantic, diagnostic-only, or
  presentation-only;
- determine whether a coherent Unicode 18 claim is supportable.

Acceptance evidence:

A small table in the implementation/closure evidence records crate/version,
Unicode-data epoch, and whether it affects skeleton/policy results.

Stop if a security-semantic dependency is materially incompatible with the
intended Unicode 18 algorithm and fixing it would require a large custom Unicode
runtime. In that case leave 17.0.0 pinned and mark this milestone blocked rather
than shipping a misleading mixed-epoch claim.

### Work package B — Update and regenerate the confusables asset

Intent:

Advance the checked-in UTS #39 data through the existing deterministic trust
path.

Required changes:

- update the pinned Unicode Security version/URL/checksum to 18.0.0;
- fetch once as a maintainer action and verify exact source bytes;
- run the hardened parser/generator;
- regenerate `src/text/confusables_generated.rs` and
  `data/confusables.rs`;
- review the generated diff for additions/removals/changed mappings;
- update exact-count fixtures and representative mappings from the generated
  result, not by assumption.

Acceptance evidence:

Generator version/checksum checks pass, generated outputs match `--check`,
strict sort/no-duplicate tests pass, and the closure record includes the exact
new checksum and entry count.

### Work package C — Qualify behavior changes

Intent:

Ensure data changes improve coverage without regressing established semantics.

Required changes:

- run the full Milestone 003 skeleton/collision/property suite against the new
  data;
- add targeted fixtures for materially changed Unicode 18 mappings;
- verify skeleton determinism/idempotence and collision grouping;
- compare representative Unicode 17 vs 18 outputs and classify each expected
  difference;
- ensure legitimate mixed-script handling and bidi classification are
  unaffected unless an underlying standards-data update intentionally changes
  them.

Acceptance evidence:

All differences attributable to versioned Unicode data are documented; no
algorithmic behavior changed accidentally in the data-refresh commit.

### Work package D — Provenance, compatibility, and release evidence

Intent:

Make the shipped Unicode-data epoch auditable by downstream users.

Required changes:

- update architecture/library/fuzzing documentation;
- add CHANGELOG entry describing the Unicode Security data refresh;
- expose provenance constants or diagnostics where justified;
- run parity and classify Unicode differences without weakening correctness;
- run dependency/license/MSRV review for any version bumps;
- compare release binary size if new direct Unicode dependencies were added.

Acceptance evidence:

A downstream caller can determine the confusables data epoch from stable
library/docs metadata, and documentation does not overclaim the epoch of
independent Unicode providers.

## 8. Failure, cancellation, truncation, and contention semantics

Generation failure is fail-closed and writes no partial output.

Runtime bounds, cancellation, and concurrency semantics are unchanged from
Milestone 003. Static data refresh must not introduce request-time loading.

If unicode.org is unavailable during maintainer regeneration, use no fallback
mirror silently. Stop and retain the currently verified source until the pinned
authoritative bytes can be obtained and verified.

## 9. Compatibility and migration

This is a data-version update, not a surface migration.

Existing APIs remain source-compatible. Some inputs may gain/lose/change
confusable findings because Unicode Security data changed; those are expected
versioned correctness changes and must be documented.

Do not preserve a stale mapping solely to retain exact old output. If a
downstream parity fixture depends on a corrected Unicode mapping, update or
classify that fixture with the version provenance.

The project may accurately state "confusables data: Unicode 18.0.0" even if a
diagnostic-only provider is older. It must not state "all Unicode processing:
18.0.0" unless the security-semantic data graph supports that claim.

## 10. Required tests

### Focused unit tests

- generated version/checksum metadata;
- exact generated entry count;
- sort/no-duplicate/scalar validity;
- representative added/removed/changed Unicode 18 mappings;
- skeleton fixtures affected by the refresh.

### Integration tests

- all Unicode policy/identifier/security composite tests from Milestone 003;
- runtime diagnostics/provenance field if added;
- unchanged registry/profile/audience tests.

### Restart and recovery tests

Not applicable; all data is static.

### Contention and cancellation tests

Re-run any concurrent lazy-index and security cancellation tests from
Milestone 003. No new state should be introduced.

### Negative and boundary tests

- malformed generator fixtures still fail;
- stale generated-output `--check` fails;
- authoritative source checksum mismatch fails;
- boundary text and supplementary-plane mappings remain safe.

### Parity and compatibility tests

Run parity where available and record intentional Unicode-data differences.
Do not expand the accepted-failure list without an explicit correctness
classification.

## 11. Required verification commands

```bash
cargo fmt --all -- --check
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --test lib text
cargo test --locked --test lib property
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
cargo deny check
cargo tree --locked -e normal
```

Run the confusables generator/check command defined by Milestone 003 and record
the exact Unicode 18.0.0 checksum. If parity prerequisites exist, run the
Unicode/identifier subset. If dependency versions change, include MSRV
qualification and one release-size comparison.

## 12. Documentation updates

- `architecture/generated-assets.md`;
- `architecture/text-library.md`;
- `docs/library-api.md` if provenance constants are public;
- `docs/fuzzing.md` if golden/property fixtures change;
- `CHANGELOG.md`;
- any diagnostics documentation if provenance becomes visible there.

## 13. Acceptance criteria

The milestone may close when:

- Milestone 003 is already closed;
- the Unicode data-provider graph is explicitly inventoried;
- the Unicode Security confusables source is pinned to authoritative 18.0.0
  bytes and checksum;
- generated assets reproduce exactly under the hardened generator;
- all structural and Unicode-security regression tests pass;
- Unicode 18 behavior differences are reviewed and documented;
- project documentation accurately distinguishes confusables-data version from
  other Unicode provider epochs;
- no runtime network access or new MCP surface was introduced;
- dependency/MSRV/license/size review passes for any required upgrades;
- the ordered merge gate and remote CI are green.

## 14. Stop conditions

Stop and report rather than improvise when:

- Milestone 003 is not closed;
- a security-semantic dependency cannot support the required Unicode data epoch
  without a large custom Unicode implementation;
- the authoritative Unicode 18 source/header/checksum cannot be verified;
- regeneration produces unexplained semantic changes outside versioned data;
- a dependency bump breaks MSRV 1.89.0 or licensing policy;
- the implementation would require a new public restriction-level/IDNA policy;
- a material binary-size increase cannot be explained/justified;
- unrelated repository changes invalidate the post-003 baseline.

## 15. Closure evidence required

The closure record must include:

- Milestone 003 closure reference;
- implementation commit(s);
- Unicode-provider/version inventory;
- authoritative Unicode Security 18.0.0 URL/version/SHA-256;
- generated entry count and generated-output check result;
- summary of material mapping changes exercised by regression tests;
- focused and full merge-gate results;
- remote CI run if available;
- cargo-deny/MSRV evidence for dependency changes;
- release-size before/after note when applicable;
- parity result/availability;
- exact wording used to describe the shipped Unicode epoch;
- residual limitations and recommendation.

## 16. Handoff notes

Do not begin by editing the version string. Begin by reading the closed
Milestone 003 evidence and inventorying the post-hardening Unicode data graph.

Keep the data-refresh diff independently reviewable from semantic refactors.
If dependency support makes a coherent Unicode 18 pipeline impossible, retaining
a correctly qualified Unicode 17 implementation is preferable to a mixed-epoch
implementation described inaccurately.
