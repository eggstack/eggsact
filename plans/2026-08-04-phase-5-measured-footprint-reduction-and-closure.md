# Phase 5 — Measured Footprint Reduction and Closure

## Status

- **Status:** planned
- **Repository:** `eggstack/eggsact`
- **Target branch:** `main`
- **Roadmap:** `plans/2026-08-04-bounded-correctness-simplification-roadmap.md`
- **Roadmap commit:** `2211ebb3adae4df6551023676047d018e113a4f7`
- **Depends on:** Phases 1 through 4
- **Priority:** medium; all candidates are measurement-gated
- **Scope:** reduce installed binary size, cold initialization work, and avoidable dependency/features without removing tools or changing intended behavior
- **Expected change size:** several small independent candidates; each may be accepted or rejected separately
- **Closure role:** record one concise completion statement for the entire roadmap and stop

## Objective

Perform a conservative footprint pass after behavior, tests, and release commands are stable.

This phase is successful when:

1. one reproducible release-artifact baseline exists;
2. the confusables representation is evaluated first because it combines binary, allocation, and startup simplification;
3. compiler-profile and dependency candidates are tested independently enough to attribute effects;
4. accepted changes preserve all 80 tools and existing response contracts;
5. candidates with negligible benefit or compatibility cost are rejected;
6. no benchmark harness, optimization framework, or CI burden is added;
7. the five-phase roadmap receives one final closure update and no evidence-only follow-up cycle.

The goal is not the smallest possible binary at any cost. The goal is a meaningfully smaller or simpler binary with no feature reduction.

---

# Hard constraints

This phase must not:

- remove any tool, tool category, supported regex backend, path mode, hash algorithm, calculator function, unit, or Unicode-security capability;
- make MCP optional in the default installed binary;
- split CLI and MCP into separate installed production binaries;
- split the crate into a workspace for size optimization;
- add dynamic loading or plugins;
- add a custom allocator;
- add a compression/decompression runtime solely to shrink embedded data;
- add nightly-only compiler flags;
- add platform-specific linker flags to normal builds without cross-platform proof;
- use `panic = "abort"` while production code relies on `catch_unwind` for structured error conversion;
- add `cargo-bloat`, `hyperfine`, `twiggy`, or benchmark crates to `Cargo.toml`;
- add permanent benchmark or size-regression workflows;
- upload binary artifacts or evidence;
- weaken deterministic output or Unicode correctness for size;
- replace full Unicode confusables data with a partial hand-maintained list;
- adopt every candidate merely because it was listed;
- create another optimization roadmap after closure.

---

# Measurement environment

Use one primary maintainer environment for before/after comparisons and record:

```text
OS and version
architecture
rustc --version
cargo --version
linker if non-default
baseline commit SHA
build command
strip state
```

Apple Silicon or Linux x86_64 is acceptable. Use the same machine, toolchain, target, and build procedure for a candidate pair.

Do not compare absolute binary sizes across different GitHub-hosted runners.

---

# Baseline procedure

Begin from the completed Phase 4 commit with a clean tree:

```bash
git fetch origin main --prune
git switch main
git reset --hard origin/main
git status --short
git rev-parse HEAD
rustc --version
cargo --version
```

Build the canonical release artifact:

```bash
cargo clean
cargo build --release --locked --bin eggsact
ls -l target/release/eggsact
```

Record at minimum:

- on-disk binary bytes;
- whether symbols are present;
- `cargo tree -e features` output summary;
- `cargo tree -d` duplicate-package summary;
- top crate contributors from `cargo bloat --release --crates --bin eggsact` if `cargo-bloat` is already installed or installed locally by the maintainer;
- cold process-start medians for representative paths.

Representative process paths:

```bash
target/release/eggsact --help
target/release/eggsact --version
target/release/eggsact "2+2"
target/release/eggsact "thirty plus five"
```

Use `hyperfine` only as a local tool. Otherwise use a simple shell loop and `/usr/bin/time`. Twenty samples after a small warmup are sufficient. Record medians, not spurious precision.

MCP functional baseline:

```text
initialize
notifications/initialized
tools/list with compact schema
one text_equal call
one math_eval call
EOF/graceful shutdown
```

Use the existing test/client helper. Do not add a benchmark client.

---

# Acceptance thresholds

A candidate should normally be retained when at least one is true:

- stripped release binary decreases by at least **1%**;
- stripped release binary decreases by at least **64 KiB** with no added complexity;
- median process-start or first-use latency for the affected path improves by at least **10%**;
- a runtime parser, map build, or repeated allocation path is deleted and replaced by a plainly simpler static representation;
- an unnecessary direct dependency or feature is removed with no behavior churn;
- source and maintenance complexity clearly decrease even when binary savings are modest.

Reject a candidate when:

- output compatibility changes without a correctness reason;
- build time increases materially for negligible size savings;
- runtime performance regresses materially;
- the implementation adds more code than it removes;
- cross-platform behavior becomes uncertain;
- the measurement difference is within ordinary noise;
- the change requires a new abstraction or dependency.

Do not combine unrelated candidates before measuring.

---

# Candidate 1 — Generate a compact static confusables table

## Current condition

The generator writes textual pseudo-Rust statements such as:

```rust
m.insert("U+0410", "U+0041");
```

Production uses `include_str!`, reparses every line on first use, builds a runtime string-keyed `HashMap`, formats each input character into a new `U+XXXX` key, and then performs a hash lookup.

This preserves data but incurs avoidable embedded syntax, startup work, heap allocation, hashing, and per-character formatting.

## Required representation

Generate a sorted static table keyed by numeric code point. Recommended shape:

```rust
pub static CONFUSABLES: &[(u32, &'static str)] = &[
    (0x0022, "U+0027 U+0027"),
    (0x0410, "U+0041"),
];
```

Then use:

```rust
CONFUSABLES.binary_search_by_key(&(ch as u32), |(cp, _)| *cp)
```

An equivalent `char`-keyed table is acceptable. Preserve the current substitution string exactly if it is part of tool output.

Required properties:

- all existing entries remain present;
- table order is deterministic;
- no runtime parser is needed;
- no runtime `HashMap` is built;
- no per-character `format!("U+...")` allocation is needed;
- `has_confusables()` and `find_confusables()` preserve behavior and output order;
- generated source remains understandable and regenerateable;
- no perfect-hash or code-generation dependency is added.

Do not generate a giant `match` unless measurement proves it smaller than the static table. A sorted data table is the default because it minimizes code generation.

## Pin Unicode source identity

Replace the moving `security/latest/confusables.txt` input with explicit metadata:

- selected Unicode security-data version;
- versioned URL or vendored source path;
- expected SHA-256 checksum;
- generator comment recording version and checksum.

The implementation agent must discover the version represented by the current generated data or deliberately update it as a separate documented data refresh. Do not silently change Unicode versions while changing representation.

A network download remains acceptable for deliberate regeneration when the checksum is verified. The canonical release check must not download it.

## Required tests

- generated table is strictly sorted and contains no duplicate source code point;
- entry count matches the pre-change dataset;
- representative ASCII, Greek, Cyrillic, supplementary-plane, and multi-code-point substitutions match exactly;
- full old/new lookup parity is established during implementation by parsing the baseline generated file in a test or temporary local script;
- existing Unicode/confusables tool tests pass;
- generated-doc/source freshness checks remain deterministic.

Do not retain the old runtime parser after parity is established.

## Measurement

Measure independently:

- release binary bytes;
- first confusables-using call latency;
- source/generated file size;
- removal of runtime map/parser allocations qualitatively or with a simple local profiler if already available.

This candidate may be retained even below 64 KiB if it deletes the runtime parser and allocation path with simpler code.

## Acceptance criteria

- full dataset parity passes;
- moving `latest` input is removed;
- runtime pseudo-Rust parsing and string-key formatting are removed;
- no new dependency is added;
- size/startup result is recorded.

---

# Candidate 2 — Add a conservative release profile

## Evaluate settings independently

Candidate settings:

```toml
[profile.release]
strip = "symbols"
lto = "thin"
codegen-units = 1
```

Test in this order:

1. `strip = "symbols"`;
2. thin LTO;
3. `codegen-units = 1` together with thin LTO only if attribution remains clear.

Do not add all settings at once before measuring.

## Required checks

For each retained setting:

- build succeeds on the primary environment;
- ordinary tests pass;
- canonical release check passes;
- MCP panic conversion tests still pass;
- process-start and representative tool runtime do not materially regress;
- compile time is noted qualitatively;
- supported-platform maintenance checks compile after push.

`strip = "symbols"` is likely a straightforward packaging improvement. Thin LTO and one codegen unit may be rejected if build-time cost is disproportionate.

## Forbidden setting

Do not set:

```toml
panic = "abort"
```

Production intentionally catches handler panics and converts them to structured responses.

Do not use `opt-level = "z"` by default. It may be evaluated locally only if the preceding settings leave a clear need; retain it only with runtime and size evidence.

---

# Candidate 3 — Evaluate a current-thread Tokio runtime

## Current condition

The MCP path creates a multi-thread Tokio runtime while tool work is already sent through `spawn_blocking` and bounded by a semaphore.

## Evaluation question

Can the async coordination layer use:

```rust
Builder::new_current_thread()
```

while preserving:

- concurrent request tasks;
- cancellation notification processing;
- writer progress;
- semaphore behavior;
- `spawn_blocking` tool execution;
- graceful shutdown;
- out-of-order response correlation by ID?

## Required procedure

1. change only runtime builder/features;
2. remove `rt-multi-thread` only if no target/test requires it;
3. run lifecycle, cancellation, concurrent request, writer serialization, and shutdown tests;
4. run an MCP smoke with multiple concurrent tool requests and a cancellation notification;
5. measure binary size and startup;
6. inspect code complexity.

## Retention rule

Retain only if:

- behavior is unchanged;
- feature tree is smaller;
- binary/startup improvement meets normal thresholds or the runtime configuration becomes plainly simpler;
- no test-only special casing is added.

Reject if savings are negligible or coordination semantics become harder to reason about.

Do not replace Tokio.

---

# Candidate 4 — Evaluate removal of `serde_json/preserve_order`

## Risk

Without `preserve_order`, `serde_json::Map` ordering changes. Eggsact has explicit deterministic-output work and Python-compatibility behavior, so deterministic does not automatically mean compatible.

## Required audit

Before editing:

- identify externally visible JSON objects whose insertion order is tested or documented;
- snapshot representative MCP envelopes and tool results;
- inspect direct use of `serde_json::Map` where field order is intentional;
- determine actual dependency/binary contribution through `cargo tree` and `cargo bloat`.

## Retention rule

Remove `preserve_order` only when:

- all public wire-order contracts remain acceptable;
- representative parity tests pass or order differences are proven irrelevant;
- no replacement ordered-map dependency is needed;
- size/dependency simplification is meaningful.

Otherwise record `rejected: wire-order compatibility risk exceeds measured benefit` and retain it.

Do not refactor every response into custom serializers for this candidate.

---

# Candidate 5 — Evaluate TOML parser dependency consolidation

## Current condition

Both `toml` and `toml_edit` are direct dependencies.

## Required audit

Map exact production uses:

```text
toml::
toml_edit::
```

Classify requirements:

- serde deserialization;
- syntax validation;
- spans/positions;
- document/table structure;
- formatting preservation.

Attempt consolidation only if one existing crate can satisfy all current behavior with less code.

## Retention rule

Retain consolidation when:

- one direct dependency is removed;
- TOML position, table, validation, Cargo manifest, and config tests pass unchanged;
- no custom parser or compatibility layer is introduced;
- binary/dependency tree improves measurably or maintenance plainly simplifies.

Reject if replacing a direct parse call requires broad conversion code or worsens error positions.

---

# Candidate 6 — Stop after high-value candidates

Do not continue dependency churn after the listed candidates unless `cargo bloat` reveals one obvious, removable feature with no functional effect.

Explicitly out of scope:

- replacing all hash crates with a new omnibus crate;
- writing cryptographic hashes in-tree;
- replacing `fancy-regex`;
- removing Unicode name/category functionality;
- feature-gating individual tool categories;
- compressing schemas or generated data at runtime;
- custom linker scripts;
- UPX or post-build packers;
- platform-specific binary surgery.

---

# Closure documentation

Update only affected files:

```text
Cargo.toml
CHANGELOG.md
architecture/generated-assets.md
architecture/text-library.md
architecture/cli-binaries.md
architecture/overview.md
README.md
```

Update the roadmap and phase plans with one concise completion record.

The final roadmap record should contain:

- implementation commit range;
- Phase 1 through 4 completion status;
- baseline and final binary bytes/environment;
- accepted candidate list with deltas;
- rejected candidate list with one-line reasons;
- final ordinary verification result;
- final canonical release-check result;
- closure statement that no further plan is needed absent a new reproducible defect.

Do not create a separate release-evidence document.

---

# Execution order for a smaller implementation agent

1. Sync to latest `origin/main`; confirm Phases 1 through 4 are complete.
2. Produce and record the clean release baseline.
3. Implement the static confusables table and pinned data identity.
4. Prove full lookup parity and measure independently.
5. Evaluate release-profile settings one at a time.
6. Evaluate current-thread Tokio independently.
7. Audit `preserve_order`; retain or reject without broad serializer work.
8. Audit TOML dependencies; retain or reject without broad parser work.
9. Run ordinary verification and the canonical release check.
10. Record final measurements and dispositions.
11. Mark all phase plans and the roadmap complete in one bounded documentation update.
12. Stop.

Do not combine all candidates into one implementation commit. Keep attribution possible.

---

# Verification

After each accepted candidate, run the nearest focused tests. Before closure, run:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity
cargo test --locked --doc
cargo run --locked --features dev-tools --bin generate-docs -- --check
scripts/release-check.sh
git status --short
```

Run targeted parity only if a candidate affects calculator behavior or public JSON ordering.

Use manual fuzzing only if confusables/path/parser implementation meaningfully changes an existing fuzzed surface. Do not add new ordinary CI jobs.

---

# Acceptance checklist

- [ ] Reproducible baseline environment and binary bytes are recorded.
- [ ] Confusables data uses a sorted static representation or is explicitly rejected with measurement.
- [ ] Full confusables dataset parity is proven.
- [ ] Unicode data version and checksum are pinned.
- [ ] Runtime confusables parsing/map construction/per-character key formatting are removed if the candidate is accepted.
- [ ] Release-profile settings are evaluated independently.
- [ ] `panic = "abort"` is not used.
- [ ] Current-thread Tokio is accepted or rejected with behavioral and size evidence.
- [ ] `serde_json/preserve_order` is accepted or rejected without broad serializer churn.
- [ ] TOML dependency consolidation is accepted or rejected without a custom parser layer.
- [ ] All 80 tools and documented capabilities remain available.
- [ ] No benchmark dependency, workflow, artifact, workspace split, or new subsystem was added.
- [ ] Ordinary verification passes.
- [ ] Canonical local release check passes and leaves a clean tree.
- [ ] Final roadmap/phase completion records are updated once.
- [ ] No further polish/evidence plan is created.

---

# Completion record

Fill once when implementation lands:

- **Implementation commit range:** pending
- **Measurement environment:** pending
- **Baseline binary bytes:** pending
- **Final binary bytes:** pending
- **Cold-start baseline/final:** pending
- **Confusables candidate:** pending
- **Release-profile candidates:** pending
- **Current-thread Tokio candidate:** pending
- **`preserve_order` candidate:** pending
- **TOML consolidation candidate:** pending
- **Ordinary verification:** pending
- **Canonical release check:** pending
- **Roadmap closure commit:** pending
- **Final phase disposition:** pending
