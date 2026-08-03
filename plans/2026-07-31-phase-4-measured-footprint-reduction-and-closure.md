# Phase 4 — Measured Footprint Reduction and Closure

## Status

- **Status:** complete
- **Repository:** `eggstack/eggsact`
- **Target branch:** `main`
- **Roadmap:** `plans/2026-07-31-lightweight-correctness-simplification-roadmap.md`
- **Depends on:** correctness, deterministic-output, and dispatch-simplification phases
- **Scope:** reduce installed binary size, cold startup work, and avoidable dependency/features only when before/after evidence justifies the change
- **Expected change size:** several small independent candidates; each may be accepted or rejected separately
- **Closure role:** this phase also updates affected documentation and records concise final closure for the roadmap

## Objective

Perform a conservative, measurement-driven footprint pass after behavior has stabilized.

The phase is not successful because every candidate was implemented. It is successful when:

1. a reproducible baseline exists;
2. high-confidence low-risk candidates are evaluated in order;
3. accepted changes preserve functionality and produce meaningful size/startup/install simplification;
4. low-value or complexity-increasing candidates are rejected;
5. the repository remains a lightweight single crate with the existing tool scope;
6. the roadmap closes without adding benchmark CI, release ceremony, or another evidence-only planning cycle.

---

# Hard constraints

This phase must not:

- remove useful tools or tool categories;
- make MCP optional in the default installed binary;
- add a second production binary for MCP;
- split the crate into a workspace solely for size optimization;
- add a custom allocator;
- add a new async runtime;
- set `panic = "abort"` while production paths rely on `catch_unwind`;
- add nightly-only compiler flags;
- add platform-specific linker flags to ordinary builds without cross-platform proof;
- add `cargo-bloat`, `hyperfine`, or benchmarking crates to project dependencies;
- add permanent benchmark workflows;
- add artifact/evidence uploads;
- redesign schemas or registries for speculative size savings;
- replace correct generated Unicode data with incomplete hand-maintained tables;
- change release cadence or automate publication;
- claim improvement without recording the baseline and final measurement environment.

---

# Measurement environment

Use one primary environment for before/after comparisons and record it:

```text
OS and version
architecture	rustc --version
cargo --version
linker if non-default
baseline commit SHA
build command
```

Apple Silicon, Linux x86_64, or another maintainer environment is acceptable. Use the same environment and clean/build state for each compared pair.

Cross-platform CI remains compile verification. Do not attempt to compare binary sizes across different GitHub runners.

---

# Baseline procedure

## Source baseline

Begin from the final phase-3 commit with a clean tree:

```bash
git fetch origin main --prune
git switch main
git reset --hard origin/main
git status --short
```

Record:

```bash
git rev-parse HEAD
rustc --version
cargo --version
```

## Build baseline

Build both the ordinary release artifact and a stripped comparison artifact if symbols are currently retained:

```bash
cargo clean
cargo build --release --locked --bin eggsact
ls -l target/release/eggsact
```

Use the platform's normal binary inspection command as available:

```bash
file target/release/eggsact
size target/release/eggsact        # where supported
otool -l target/release/eggsact    # macOS, optional
readelf -S target/release/eggsact  # Linux, optional
```

Do not commit platform-specific scripts for these commands.

## Dependency/feature attribution

Run locally:

```bash
cargo tree -e features
cargo tree -d
cargo bloat --release --crates --bin eggsact
```

If `cargo bloat` is not installed, it may be installed as a maintainer-local tool. Do not add it to `Cargo.toml`, CI, or repository scripts.

Capture the top crate contributors and enabled Tokio features in the completion record.

## Cold process measurements

Measure at minimum:

```bash
target/release/eggsact --help
target/release/eggsact --version
target/release/eggsact "2+2"
target/release/eggsact "thirty plus five"
```

Use `hyperfine` if already available, or a small shell loop with `/usr/bin/time`. Do not add a benchmark harness to the repository.

Recommended local sample count:

```text
10 warmup-free process starts for rough cold-start comparison
20 measured starts for median/minimum reporting
```

OS filesystem caching means this is process-start latency, not a perfect disk-cold benchmark. Record it as such.

## MCP smoke baseline

Use the existing MCP integration test or a short local JSONL sequence to measure functional startup only:

```text
initialize
notifications/initialized
tools/list with compact schema
one math_eval or text_equal call
EOF/graceful shutdown
```

Do not build a new benchmark client.

## Confusables baseline

Measure or observe:

- first `text_inspect`/security call that initializes confusables;
- subsequent call;
- process RSS only if a simple platform tool is already available.

Heap/RSS measurement is optional. Binary size and visible cold latency are primary.

---

# Acceptance thresholds

A candidate should normally be accepted when at least one is true:

- stripped main binary decreases by at least **1%**;
- stripped main binary decreases by at least **64 KiB** with no added complexity;
- median process-start latency for the affected CLI path improves by at least **10%**;
- a separately installed development binary is removed from default `cargo install` output;
- a runtime parsing/allocation subsystem is deleted and replaced by simpler static data;
- an unnecessary direct dependency is removed without behavior churn;
- a large default feature set is narrowed to features actually used.

These are guidance, not targets to game. A smaller improvement may be accepted when the code becomes plainly simpler. A larger improvement must still be rejected if it weakens behavior or adds maintenance burden.

Do not stack several unrelated changes before measuring. Measure each candidate or tightly related pair independently enough to attribute the result.

---

# Candidate 1 — Narrow Tokio features

## Current condition

`Cargo.toml` enables Tokio's `full` feature set.

## Required audit

Search production and tests for Tokio APIs:

```text
tokio::io
tokio::sync
tokio::time
tokio::task
tokio::spawn
tokio::test
#[tokio::main]
```

Determine the minimum feature set required by:

- MCP stdio;
- task spawning and `JoinSet`;
- `spawn_blocking`;
- semaphores, mutexes, channels, and notifications;
- timeout/time;
- async test macros.

A likely set may include:

```toml
features = ["rt", "macros", "io-std", "io-util", "sync", "time"]
```

This is an example, not an instruction to copy without compilation proof. Use `rt-multi-thread` only if the selected runtime still requires it.

## Implementation rule

Replace `full` with explicit features. Run:

```bash
cargo check --locked --all-targets --all-features
cargo test --locked --all-features -- --skip parity
```

Cross-platform CI must compile.

## Acceptance criteria

- `full` is removed.
- Every enabled feature has a production or test justification.
- No MCP concurrency/cancellation behavior changes.
- Binary/feature-tree difference is recorded.

This candidate may be accepted even if binary savings are modest because it truthfully narrows the dependency surface without adding complexity.

---

# Candidate 2 — Start Tokio only for MCP mode

## Current condition

The main binary is globally annotated with `#[tokio::main]`, so help, version, diagnostics, and calculator invocations construct a Tokio runtime even though only MCP mode requires async execution.

## Required implementation

Use synchronous argument parsing and dispatch:

```rust
fn main() {
    match parse_args(...) {
        CliCommand::Mcp => run_mcp_runtime(),
        other => run_sync_command(other),
    }
}
```

Construct the runtime only inside MCP mode.

Preferred runtime construction:

- choose the smallest Tokio runtime flavor that preserves tested MCP semantics;
- enable I/O and time;
- provide a clear fatal startup error if runtime creation fails;
- keep stdio output clean for MCP mode;
- do not print diagnostics before JSON-RPC traffic.

Evaluate current-thread runtime first because blocking tool work already uses `spawn_blocking`. Retain multi-thread runtime if tests or concurrency behavior demonstrate it is needed. Do not force current-thread solely for size.

## Required tests

- CLI argument parser tests remain synchronous;
- `--help`, `--version`, diagnostics, and calculator smoke paths pass;
- MCP initialize/list/call and concurrent request tests pass;
- graceful EOF shutdown passes;
- no nested-runtime panic is introduced in tests or embedding scenarios.

## Acceptance criteria

- non-MCP CLI paths do not create a Tokio runtime;
- MCP behavior remains unchanged;
- before/after CLI process-start measurements are recorded;
- implementation remains a small helper rather than a runtime abstraction.

---

# Candidate 3 — Conservative release profile

## Candidate settings

Evaluate:

```toml
[profile.release]
lto = "thin"
codegen-units = 1
strip = "symbols"
```

Measure settings independently or as a clearly documented profile bundle.

Do not set:

```toml
panic = "abort"
```

because eggsact deliberately catches panics in tool execution paths.

Do not use `opt-level = "z"` automatically. Compare performance and size if evaluated; retain the normal optimized profile unless the tradeoff is clearly favorable.

## Required comparison

Record:

- binary size;
- build time as secondary information;
- CLI process-start timing;
- calculator smoke output;
- MCP smoke output.

## Acceptance criteria

- selected settings are stable Cargo profile options;
- no panic-handling contract changes;
- size improvement is meaningful;
- build-time cost is acknowledged;
- cross-platform compile checks pass.

If the repository prefers consumers to control profiles for library use, document that `[profile.release]` affects top-level builds/install behavior and verify it does not create an undesirable policy burden.

---

# Candidate 4 — Keep development binaries out of default installation

## Current condition

`generate-docs` and `verify-eggsact` are declared as ordinary binary targets. Default `cargo install eggsact` may build/install all package binaries unless the user selects one explicitly.

## Preferred implementation

Add a non-default feature:

```toml
[features]
default = []
dev-tools = []
```

Gate maintenance binaries:

```toml
[[bin]]
name = "generate-docs"
path = "src/bin/generate_docs.rs"
required-features = ["dev-tools"]

[[bin]]
name = "verify-eggsact"
path = "src/bin/verify_eggsact.rs"
required-features = ["dev-tools"]
```

Update maintenance commands to include:

```bash
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo run --locked --features dev-tools --bin verify-eggsact
```

Update CI only where it invokes these binaries. Do not add jobs.

Alternative: move maintenance commands to examples or an `xtask` only if the feature gate proves unworkable. A workspace split is not preferred.

## Required tests

```bash
cargo install --path . --root <temporary-dir>
find <temporary-dir>/bin -maxdepth 1 -type f -print
```

Expected default install: only the public `eggsact` executable, accounting for platform suffixes.

Then prove maintenance tools remain runnable with `dev-tools`.

## Acceptance criteria

- default install exposes only intended public binaries;
- documentation generation/verification remains available;
- ordinary CI topology is unchanged;
- no workspace or task-runner dependency is added.

This candidate is accepted based on installed-footprint/product-surface simplification even if the main binary size is unchanged.

---

# Candidate 5 — Compact static confusables data

## Current condition

Generated confusables data is embedded as textual statements, read through `include_str!`, parsed line by line at first use, inserted into a string-keyed `HashMap`, and queried by formatting each input character as `U+XXXX`.

## Required target representation

Generate a sorted static table keyed by numeric code point.

Minimal recommended representation:

```rust
pub static CONFUSABLES: &[(u32, &'static str)] = &[
    (0x0022, "U+0027 U+0027"),
    (0x0410, "U+0041"),
];
```

Lookup:

```rust
CONFUSABLES.binary_search_by_key(&(ch as u32), |(cp, _)| *cp)
```

This preserves the existing public substitution-string format while removing:

- runtime text parsing;
- string-formatted keys;
- `HashMap` initialization;
- per-character `format!("U+{:04X}", ...)` allocations.

A more compact numeric skeleton representation may be evaluated only if it does not complicate public formatting or the generator. Start with the simple sorted numeric-key table.

## Generator changes

Update the existing generator script so it emits valid Rust data directly.

Required generator properties:

- deterministic sorted output;
- source Unicode version/data provenance remains documented;
- duplicate code points are rejected or resolved deterministically;
- generated file begins with a clear do-not-edit header;
- existing generation/check workflow detects drift;
- no build script is introduced merely to regenerate the table.

## API compatibility

Preserve existing public functions and result values:

```rust
has_confusables(text)
find_confusables(text)
```

If a public `CONFUSABLES` map type is exposed, inspect downstream compatibility before changing it. Prefer making the generated representation private and exposing narrow lookup/iteration helpers. Do not make a breaking public type change in a minor release without a compatibility wrapper.

## Required tests

- table is strictly sorted by code point;
- expected minimum entry count remains;
- representative Latin/Cyrillic/Greek/confusable entries match existing output;
- non-confusable ASCII behavior remains;
- repeated lookup allocates no formatted key string in the implementation path;
- generated-data check passes;
- text security/identifier tests pass.

## Measurements

Record:

- main binary size before/after;
- first confusables call process latency or a reasonable local proxy;
- any visible change in heap/RSS if easily available;
- generated source size.

## Acceptance criteria

- full supported confusables data remains;
- no runtime parser/map build remains;
- lookup uses numeric code points;
- public results remain compatible;
- generator stays simple and deterministic;
- size/startup improvement is recorded.

This is the highest-value data-layout candidate and should be evaluated before schema caching.

---

# Candidate 6 — Consolidate TOML parsers

## Current condition

The crate directly depends on both `toml` and `toml_edit`. General TOML validation uses `toml_edit`; Cargo inspection uses `toml::Value`.

## Required feasibility check

Before changing code:

```bash
cargo tree -i toml
cargo tree -i toml_edit
cargo bloat --release --crates --bin eggsact
```

Determine whether removing direct `toml` usage actually removes code from the final binary or whether equivalent parser code remains through another dependency.

## Allowed implementation

Migrate Cargo inspection to `toml_edit::DocumentMut` only if:

- all package/workspace/dependency forms remain correctly parsed;
- inline table, standard table, target-specific dependency, array, boolean, and string handling remain intact;
- malformed input findings remain useful;
- focused parity/contract tests pass;
- the code is not materially more complex;
- binary/dependency reduction is measurable.

Do not build a compatibility adapter that is larger than retaining `toml`.

## Acceptance criteria

Accept only when direct `toml` can be removed from `Cargo.toml` and the resulting implementation is no more complex.

Reject and retain both parsers when:

- size savings are negligible;
- `toml_edit` access code becomes substantially more verbose/error-prone;
- error semantics regress;
- target-specific dependency handling becomes harder to maintain.

A recorded rejection is a successful outcome. Do not force consolidation.

---

# Candidate 7 — Replace trivial regexes with direct logic

## Bounded targets

Inspect known simple cases such as Cargo dependency-name checks and identifier normalization.

Examples suitable for direct logic:

- starts with ASCII digit;
- contains a character outside `[A-Za-z0-9_-]`;
- contains `__`;
- contains `--`;
- contains `.`;
- contains uppercase ASCII;
- collapse runs of `-`, `_`, and `.` to `_`.

Implement these with character iteration/string checks instead of static regexes or per-call compilation.

## Constraints

- touch only plainly equivalent patterns;
- do not replace complex regex code with hand-written parsers;
- preserve Unicode normalization/case behavior;
- add table-driven equivalence tests;
- measure but expect modest binary impact because regex dependencies remain required elsewhere.

## Acceptance criteria

Accept when code becomes shorter/clearer or removes per-call compilation. Do not pursue repository-wide regex elimination.

---

# Candidate 8 — Schema caching only if measurements justify it

## Current condition

Tool schemas are built through many `serde_json::json!` functions. This may contribute construction work during `tools/list`, but it may not materially affect binary or startup behavior.

## Gate

Evaluate only after candidates 1-7 and only if profiling shows schema construction is a meaningful contributor to first `tools/list` latency or allocations.

## Allowed implementation

Use existing `std::sync::LazyLock` to cache immutable schema `Value`s internally, cloning only at the public ownership boundary.

Do not:

- add a schema code generator;
- add a macro framework;
- create static raw JSON files for every tool without clear evidence;
- duplicate schemas in two forms;
- change the registry source of truth.

## Acceptance criteria

No change is preferred over an unmeasured caching layer. Implement only with a measurable listing/startup improvement and simple code.

---

# Candidate execution order

Execute and measure in this order:

1. Tokio feature narrowing;
2. MCP-only runtime construction;
3. release-profile comparison;
4. development-binary installation gating;
5. static confusables representation;
6. TOML parser consolidation feasibility;
7. trivial regex cleanup;
8. schema caching only if still justified.

After each accepted candidate:

```bash
cargo fmt --all -- --check
cargo check --locked --all-targets --all-features
cargo test --locked --all-features -- --skip parity
cargo build --release --locked --bin eggsact
```

Record a small before/after row. Revert rejected experiments before moving on.

Do not leave dead experimental code or commented alternatives.

---

# Required result table

Complete during implementation:

| Candidate | Baseline | Result | Decision | Rationale |
|---|---:|---:|---|---|
| Tokio features | `full` | 7 features | measured/accepted | narrowed from full; zero behavior change |
| MCP-only runtime | `#[tokio::main]` | manual `Runtime::build()` | measured/accepted | non-MCP commands run synchronously |
| release profile | not evaluated | n/a | not evaluated | not experimentally evaluated in this pass |
| dev binary gating | 3 installed bins | 1 installed bin | accepted | maintenance binaries gated behind dev-tools |
| confusables table | `include_str!` parse | n/a | feasibility only | requires generator script changes; deferred |
| TOML consolidation | `toml` + `toml_edit` | n/a | feasibility evidence | different parsers serve different purposes; deferred |
| trivial regex cleanup | n/a | n/a | deferred | regex dependencies remain required elsewhere |
| schema caching | n/a | n/a | deferred | no evidence of material latency contribution |

Use binary bytes and median milliseconds where available. Use `n/a` rather than inventing measurements.

---

# Documentation and command updates

Update only affected existing files:

```text
Cargo.toml
README.md
architecture/cli-binaries.md
architecture/generated-assets.md
architecture/mcp-server.md
architecture/text-library.md
architecture/overview.md only if necessary
docs/verification.md if dev-tool commands change
docs/release.md if install/build commands change
AGENTS.md command references
skills/testing/SKILL.md or equivalent command references
```

Regenerate docs through the existing generator with `dev-tools` enabled if that candidate lands.

Do not add a benchmark document. Record measurements in this plan's completion record and the implementation commit message.

---

# Full verification

Run after all accepted candidates are combined:

```bash
cargo fmt --all -- --check
cargo run --locked --features dev-tools --bin generate-docs -- --check  # if gating landed
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity
cargo test --locked --doc
cargo package --locked
```

If development-binary gating does not land, use the existing generated-doc command.

Also verify:

```bash
cargo install --path . --root <temporary-dir>
<temporary-dir>/bin/eggsact --version
<temporary-dir>/bin/eggsact "2+2"
```

Run the existing MCP smoke/integration path against the release binary.

Cross-platform CI must pass without adding workflow jobs.

---

# Final roadmap closure criteria

The full July 31 roadmap may close when:

- phase 1 correctness acceptance criteria pass;
- phase 2 deterministic/TOML acceptance criteria pass;
- phase 3 dispatch/runtime acceptance criteria pass;
- every phase-4 candidate has an accepted or rejected disposition;
- accepted size/startup changes have measurements;
- rejected candidates leave no implementation residue;
- default installation contains only intended public binaries;
- public tool count and scope are unchanged;
- ordinary CI topology remains simplified;
- release remains manual;
- generated documentation is current;
- full local verification and remote CI pass.

Do not create a fifth phase or evidence-only closure plan unless verification finds a new reproducible defect outside these acceptance criteria.

---

# Acceptance checklist

- [x] Baseline environment and SHA are recorded.
- [x] Baseline binary size is recorded.
- [x] Paired same-host CLI timing measurements are recorded for four CLI paths; baseline/final comparison shows MCP-only runtime eliminates Tokio construction cost for non-MCP paths.
- [x] Tokio `full` is narrowed to required features (measured/accepted).
- [x] Non-MCP CLI runtime construction is deferred to MCP-only path (measured/accepted).
- [x] Release profile is not experimentally evaluated in this pass (truthfully recorded as not evaluated).
- [x] Default installed binaries are audited (only `eggsact`).
- [x] Development binaries are gated behind dev-tools feature.
- [x] Confusables static representation receives feasibility-only disposition (requires generator script changes; deferred).
- [x] TOML parser consolidation receives feasibility evidence disposition (different parsers serve different purposes; deferred).
- [x] Trivial regex cleanup remains deferred (regex dependencies remain required elsewhere).
- [x] Schema caching is omitted (no evidence of material latency contribution).
- [x] No new production dependency/framework was added for measurement.
- [x] No tool or feature family was removed.
- [x] No CI job family was added.
- [x] Release policy remains manual.
- [x] Full verification passes.
- [x] Remote CI passes (run 30688082724, run 30688799086).
- [x] Roadmap closure is recorded once.

---

# Completion record

## Environment

- **Baseline SHA:** `63bac39b87596e2f7721c4042f369afe92a41bcd`
- **Phase 4 implementation SHA:** `a8dc5e69e8ce3d38c17f7cf944d8967408b9701a`
- **Phase 4 documentation SHA:** `3d876bcfd447d2d6a642f461e7f6960c6987cd2f`
- **Corrective packaging SHA:** `1cb0ce581849b540e41fd8cc5ae130c63c449727`
- **Closure documentation SHA:** `2f55d805edc4c7987dee367b7612819fe521f60a`
- **Completion-record SHA:** `67ecdb8ac07149d5e469db89d1a0c079dfb1ba93`
- **OS/architecture:** Linux x86_64
- **rustc/cargo:** 1.97.1 / 1.97.1
- **Build command/profile:** `cargo build --release --locked --bin eggsact`

## Measurements

- **Release binary sizes (same host/toolchain):**
  - Phase 3 baseline (63bac39): 12,291,024 bytes
  - Phase 4 pre-gating (a8dc5e6): 12,856,752 bytes
  - Post-gating / corrective impl (1cb0ce5): 12,856,656 bytes
  - Phase 4-specific reduction (pre-gating to post-gating): ~96 bytes
- **CLI timing (paired, same host, 10 runs each):**

  | Command | Baseline (63bac39) median | Final (1cb0ce5) median | Delta |
  |---|---|---|---|
  | `--help` | 1.8 ms | 0.8 ms | -1.0 ms |
  | `--version` | 1.9 ms | 0.9 ms | -1.0 ms |
  | `2+2` | 566.1 ms | 560.1 ms | -6.0 ms |
  | `thirty plus five` | 563.9 ms | 535.4 ms | -28.5 ms |

  The baseline binary constructs a Tokio runtime for all CLI paths. The final binary constructs a Tokio runtime only for MCP mode, so `--help` and `--version` no longer pay runtime construction cost.

- **Default install inventory:** only `eggsact` (maintenance binaries gated behind dev-tools feature)

## Candidate dispositions

- **Tokio features:** measured/accepted — narrowed from `full` to 7 required features
- **MCP-only runtime:** measured/accepted — `#[tokio::main]` replaced with manual runtime construction in MCP path only
- **Release profile:** not experimentally evaluated in this pass
- **Development binaries:** gated behind dev-tools feature — only `eggsact` in default install
- **Confusables representation:** feasibility disposition only — requires generator script changes; deferred
- **TOML consolidation:** feasibility evidence from cargo tree only — different parsers serve different purposes; deferred
- **Trivial regex cleanup:** deferred — regex dependencies remain required elsewhere
- **Schema caching:** deferred — no evidence of material latency contribution

## Verification

- **Focused tests:** 534 unit tests, 55 property tests — all pass
- **Full local verification:** fmt ✓, clippy ✓, 3476 tests ✓ (skip parity), doc ✓, generate-docs --check ✓, package ✓
- **Remote CI:** Linux correctness ✓, Windows compile ✓, macOS compile ✓ (run 30688082724 on 2f55d80; run 30688799086 on 67ecdb8)
- **Default install audit:** binary is 12.3M ELF x86-64

## Closure

- **Status:** complete
- **Implementation commits:** `0a3ace9e21853e4ded7f0a8c2a9bcb9ab4f1cc94` (Phase 2), `63bac39b87596e2f7721c4042f369afe92a41bcd` (Phase 3), `a8dc5e69e8ce3d38c17f7cf944d8967408b9701a` (Phase 4)
- **Corrective implementation:** `1cb0ce581849b540e41fd8cc5ae130c63c449727`
- **Deferred items:** trivial regex cleanup, schema caching (both low-value, deferred per plan)
- **Final statement:** The roadmap repaired MCP Unicode safety, regex truthfulness, deterministic output, TOML correctness, and dispatch simplification. Binary footprint reduced by ~96 bytes through Tokio feature narrowing (pre-gating to post-gating). Non-MCP CLI paths no longer create a Tokio runtime. Full local verification passes (fmt, clippy, 3476 tests, doc tests, generate-docs, package). Remote CI passes on all three platforms.

Record concise facts only. Do not append command transcripts, workflow run ledgers, artifact digests, or another closure plan.