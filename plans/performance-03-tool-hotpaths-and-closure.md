# Performance 03 — Deterministic Tool Hot Paths and Closure

Planning baseline: 23af42219ef1088ddeff080a2e07a3361e8468c3 (main, 2026-09-20)
Status: planned
Priority: P1
Depends on: performance-01-mcp-boundary-and-baseline.md and performance-02-patch-correctness-and-linear-apply.md

## Objective

Use the performance harness and evidence from Performance 01/02 to optimize the remaining deterministic-tool hot paths that have clear avoidable repeated work, while preserving exact public/MCP behavior and refusing speculative micro-optimizations that do not measure.

This is the closure pass for the 2026-09-20 performance campaign.

## Standing constraints

Preserve:

- all 86 tools and current schemas;
- deterministic exact-input/exact-output behavior;
- public typed/raw APIs;
- current input/output limits;
- Unicode/codepoint/byte/line-column semantics;
- JSON preserve_order semantics;
- regex engine-selection semantics and safety policy;
- discovery ranking semantics unless a regression demonstrates a real semantic defect;
- repo-analysis output ordering called out as byte-stable in current comments/tests;
- calculator parity and accepted parity-failure policy;
- no new runtime dependency solely for speed.

Each optimization must have a focused correctness test and before/after evidence from the same host/toolchain where practical.

## Part A — Make text_replace_check linear in source plus matches

Primary file:

- src/text/replace.rs

The current implementation repeatedly rescans text to convert codepoint positions to byte offsets/line-column positions and repeatedly rescans codepoints while reconstructing replacement output.

Build one source index during the initial pass that can answer:

- codepoint index -> byte offset;
- codepoint index -> line/column using existing CR/LF/CRLF semantics.

Use it for all PositionInfo values.

For exact mode, preserve direct byte-search behavior and map byte offsets to codepoint positions through the precomputed index rather than text[..byte_idx].chars().count per match.

For normalized modes, retain normalize_with_map semantics and use its original-codepoint boundaries with the same source index.

Build replaced_text into one String:

- reserve a reasonable capacity;
- append source byte slices for unchanged ranges;
- append new;
- append the final suffix.

Do not build Vec<String> parts and join them.

### Edge cases to preserve

- empty old string matches at every codepoint boundary;
- Unicode expansion/contraction under NFC/NFKC/casefold;
- grapheme mapping used by normalized modes;
- CRLF line/column semantics;
- newline_policy transformations;
- preview truncation;
- fingerprint bytes;
- allow_multiple and expected_count findings/order.

Use existing regex_finditer indexing work as a reference, but do not merge helpers unless their semantics are demonstrably identical.

## Part B — Remove repeated JSON serialization inside sorting

Primary files:

- src/tools/helpers.rs
- src/tools/json.rs

### B1. json_compare unordered arrays

When ignore_array_order=true, the current sort comparator serializes both compared Values on every comparator call.

Precompute one stable serialized sort key per element:

    Vec<(String, &Value)>

Sort by the cached String and recurse on the paired Values.

This must preserve the exact current ordering/equality behavior, including preserve_order-derived serialization.

Add cases with:

- scalars;
- nested objects;
- nested arrays;
- duplicate equivalent values;
- non-ASCII strings;
- object key ordering;
- large nested elements.

Measure arrays large enough for comparator serialization to be visible.

### B2. Object comparison fast path

When casefold_keys=false, avoid cloning every object key into HashMap<String, String> solely to compare key sets.

Use borrowed map keys/set views while retaining deterministic sorted output of missing/common keys.

Keep the existing casefold mapping/collision behavior when casefold_keys=true.

Do not alter diff ordering.

## Part C — Give json_extract a true summary path

Primary file:

- src/tools/json.rs

After parsing and resolving the JSON Pointer, if detail=summary:

- compute only the summary fields required by the existing response;
- return before serializing a full preview;
- do not collect child_keys;
- do not build a full result containing the selected Value only to discard it.

Apply this to both empty/root pointer and non-root pointer success.

Failure responses must remain unchanged.

Add a large-object/nested-array regression showing that summary output is byte-equivalent to baseline while avoiding the discarded full payload work.

## Part D — Precompute the static discovery lexical index

Primary file:

- src/mcp/discovery.rs

Tool metadata is process-static, but tool_search currently lowercases/tokenizes names, aliases, tags, categories and descriptions on every query.

Add a private LazyLock-backed search index keyed in registry order. Each entry may precompute:

- lowercase canonical name;
- lowercase aliases;
- canonical-name tokens;
- tag/category tokens;
- description tokens;
- required argument names;
- registry/spec reference or stable index;
- deprecation metadata needed by scoring.

At query time:

- lowercase/tokenize only the query;
- score against the static index;
- preserve relevance classes, matched-token weighting, tie-breaking and registry order exactly;
- preserve deprecated-tool containment;
- construct schemas only when detail=schema.

Do not add embeddings, fuzzy-search dependencies, persistent indices, telemetry, or change ranking weights to improve benchmark scores.

Run existing discovery intent/scenario tests unchanged. Any ranking delta requires an explicit explanation and fixture review.

## Part E — Compute RepoFacts from one normalized per-path record

Primary files:

- src/services/repo.rs
- src/tools/repo.rs

repo_facts currently repeats path normalization/lowercasing/basename extraction and calls detect_project_types twice through detect_ecosystems.

Introduce a private internal path-fact record or equivalent one-pass analysis containing only information already derived today, for example:

    original
    normalized
    lowercase normalized/path
    basename
    extension
    bucket
    hidden/dotfile
    language/ecosystem hints

Derive RepoFacts projections from those records without changing public RepoFacts fields.

Preserve explicitly documented input ordering for buckets, entrypoints and high-leverage paths and existing sorted/dedup behavior for tool_hints.

Where a tools/repo adapter owns RepoFacts and then clones vectors/maps solely to move them into serde_json output, move owned fields or destructure RepoFacts instead.

Do not create a second public path-classification API.

## Part F — Remove repeated UTF-8 offset scans from diff_spans

Primary file:

- src/text/diff.rs

diff_spans already has appropriate DP/cell bounds and coarse fallback. Keep them.

Replace char_slice prefix summation with a precomputed char-index -> byte-offset table so each emitted span slices source strings in O(1) offset lookup.

Do not replace the bounded LCS algorithm in this pass.

Optionally simplify first_diff/common_prefix_suffix Vec<char> allocation only if benchmark evidence shows value and tests prove Unicode index semantics.

## Part G — Reuse regex capture-name metadata per compiled pattern

Primary files:

- src/text/regex_engine.rs
- src/text/validate.rs

Capture names are properties of the compiled regex, not an individual match. Avoid rebuilding owned capture-name maps/strings for every captures/captures_from_pos result or every emitted regex_finditer match.

Preferred direction:

- cache name -> group-index metadata inside or alongside CompiledRegex for the invocation;
- populate result groupdict from that static metadata.

Preserve:

- regex vs fancy-regex engine selection;
- absolute span behavior;
- unsupported-feature classification;
- runtime execution errors;
- named-group output ordering;
- zero-length match advancement.

Do not add a process-global arbitrary-pattern compiled-regex cache in this campaign. That requires an eviction/memory policy and changes adversarial memory behavior.

## Part H — Tighten encoding primitives

Primary file:

- src/tools/encoding.rs

Replace per-byte format allocation in lowercase hex encoding with:

- String::with_capacity(bytes.len() * 2);
- a fixed hexadecimal lookup table or equivalent direct nibble writes.

Preserve exact lowercase output.

For codec conversions, avoid a redundant bytes.to_vec when UTF-8 output can consume the already-owned decoded Vec through an internal ownership-aware helper.

Where an output expansion is exactly predictable, reject an over-limit result before allocating it, while preserving the current error classification/text where contractual.

Do not broaden codec support.

## Part I — Benchmark-gated optional optimizations

Only implement these if the campaign harness identifies a material contribution after Parts A–H.

### I1. list_compare near-match candidate bucketing

Current code already has length pruning, a 1,000,000-comparison cap, and bounded Levenshtein.

If measured, bucket candidates by codepoint length so a string is compared only with lengths within near_match_threshold.

Preserve:

- first/ordering behavior;
- seen-pair dedup;
- comparison cap;
- distance semantics;
- returned near_matches shape/order.

### I2. Calculator normalization fast path

Calculator normalization contains many precompiled regex passes. Do not add a canonical-expression fast path unless benchmark evidence shows normalization is a meaningful warm-call cost.

Any fast path must be conservative and differential-tested against the existing path over:

- literals/operators;
- Unicode operators;
- constants;
- units/conversions;
- functions;
- implicit multiplication;
- stateful/context operations;
- invalid expressions/error text.

Fallback to the current normalization whenever classification is uncertain.

Do not remove fancy-regex support merely because many patterns can use regex; the existing engine mix and parity are more important than a speculative dependency simplification.

### I3. Runtime config locks or sync-pool transport

Do not change ACTIVE_PROFILE/schema/audience/surface synchronization or the sync_pool mpsc/Mutex unless measurement demonstrates contention in a realistic workload.

No crossbeam dependency should be added based only on source comments.

## Part J — Campaign closure and regression accounting

After implementation:

1. rerun the complete Performance 01 benchmark matrix on the same host/toolchain where feasible;
2. compare against the planning baseline and intermediate candidates;
3. record wins, neutral changes, and regressions;
4. revert optional changes that add complexity without measurable benefit;
5. record stripped release binary and Cargo.lock package-count deltas;
6. run the full merge/release qualification appropriate to touched surfaces;
7. update plans/roadmap.md with durable results;
8. prune Performance 01/02/03 plan files only after all completion criteria and evidence are present, following the repository planning convention.

No percentage improvement target is mandatory. The success criterion is lower measured overhead with no API/capability regression and no unjustified complexity.

## Verification

Run focused suites for text/JSON/discovery/repo/diff/regex/encoding, then the full AGENTS.md merge gate:

    cargo fmt --all -- --check
    cargo run --locked --features dev-tools --bin generate-docs -- --check
    cargo clippy --locked --all-targets --all-features -- -D warnings
    cargo test --locked --all-features -- --skip parity --test-threads=4
    cargo test --locked --doc

Also run:

- scripts/check-release-contract.py;
- cargo deny check advisories bans licenses sources when dependencies/lockfile move;
- cargo build --locked --release;
- eggsact --version;
- MCP binary smoke;
- relevant parity tests for any behavior-adjacent text/JSON/calculator change;
- existing discovery retrieval/context gates unchanged.

## Evidence to record at closure

At minimum:

    text_replace many-match before/after
    json_compare unordered-array before/after
    json_extract summary before/after
    tool_search before/after
    repo_facts before/after
    diff_spans Unicode/multi-span before/after
    regex named-capture workload before/after
    codec hex near-limit before/after
    patch multi-hunk result from Performance 02
    MCP boundary result from Performance 01
    stripped release bytes before/after campaign
    resolved Cargo.lock package count before/after
    full merge/release gate status

If an optional optimization is rejected, record the measured reason briefly so it is not repeatedly rediscovered.

## Completion criteria

This plan and the performance campaign are complete when:

- text_replace_check no longer performs repeated source-wide position/reconstruction scans per match;
- unordered JSON-array comparison serializes each element at most once for sort-key purposes;
- json_extract summary does not build discarded full-detail data;
- discovery search reuses static tokenized tool metadata while preserving ranking/containment;
- RepoFacts avoids repeated whole-path normalization/classification passes without changing output ordering;
- diff_spans uses indexed UTF-8 offsets;
- regex named-capture metadata is reused within an invocation;
- encoding hex avoids per-byte formatting allocation;
- optional calculator/list/runtime changes are either evidence-backed or explicitly declined;
- all API/MCP/tool schemas and capability counts remain unchanged;
- benchmark evidence shows the net campaign effect;
- full repository qualification passes;
- plans/roadmap.md records the durable closure evidence before active plan files are pruned.
