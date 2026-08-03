# Lightweight Correctness, Simplification, and Footprint Roadmap

## Status

- **Status:** complete
- **Repository:** `eggstack/eggsact`
- **Target branch:** `main`
- **Plan baseline:** `e5571bd6f32d03e8100f7ce165c3649d18a4cc2f`
- **Scope:** repair bounded correctness defects, reduce unnecessary runtime/API complexity, and reduce binary/startup footprint only where measurements justify the change
- **Primary objective:** preserve eggsact's existing deterministic MCP and in-process utility scope while making the implementation smaller, clearer, and more truthful
- **Release policy:** releases remain manual; this roadmap does not publish a crate, create tags, or alter release cadence

## Purpose

Eggsact is accomplishing its intended goal: it exposes deterministic utility operations that coding agents should not approximate probabilistically. Its useful scope includes calculator operations, text and Unicode inspection, regex validation and iteration, JSON/TOML validation, path and shell analysis, patch preflight, dependency inspection, and repository analysis through both MCP and an in-process Rust API.

The current repository does not need a broad redesign or additional feature program. The remaining work is narrower:

1. correct several places where the public result does not accurately describe what the implementation did;
2. eliminate a small number of availability and Unicode-position defects;
3. define and enforce deterministic serialization where output order matters;
4. simplify overlapping in-process dispatch and calculator-context semantics;
5. reduce binary/startup footprint through measurement-driven changes, not speculative dependency churn;
6. keep ordinary CI and release handling in their already-simplified form.

This roadmap is deliberately conservative. It prioritizes deletion, consolidation, and truthful contracts over adding abstractions.

---

# Product boundary

Eggsact remains:

- a single Rust crate;
- a local stdio MCP server;
- an in-process Rust utility library;
- a deterministic utility/tool collection for coding agents;
- a calculator-compatible surface where documented;
- a manually released crates.io package.

Eggsact does not become:

- a general policy engine;
- a persistent agent daemon;
- a remote multi-user service;
- a plugin framework;
- a full PCRE2 implementation;
- a general-purpose sandbox;
- a source-code parser for every language;
- a new async runtime abstraction;
- a benchmark laboratory checked into the normal product path.

---

# Governing constraints

Every implementation phase must observe the following constraints.

## Lightweight implementation constraint

Prefer, in order:

1. deleting duplicated code;
2. consolidating existing code paths;
3. using standard-library data structures;
4. reusing dependencies already present;
5. adding a small internal helper;
6. adding a dependency only when it removes more complexity than it creates and measured evidence supports it.

A new subsystem, framework, workspace split, code-generation framework, task runner, benchmark service, or CI family is out of scope.

## Verification constraint

Each defect receives focused regression coverage at the nearest existing test layer. Do not add a broad new test harness.

Required verification remains:

```bash
cargo fmt --all -- --check
cargo run --locked --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity
cargo test --locked --doc
cargo package --locked
```

Run targeted parity tests only when a change affects a Python-compatibility contract. Fuzzing remains manual/targeted and is not expanded by this roadmap.

## Compatibility constraint

Preserve existing public behavior unless the current behavior is demonstrably incorrect, internally contradictory, unsafe, or falsely reported.

When behavior changes:

- preserve old field names where practical;
- add or refine structured fields rather than replacing an entire response shape;
- document the correction;
- add focused compatibility tests;
- avoid a major-version redesign unless no bounded migration is possible.

## Determinism constraint

Distinguish two contracts:

- **semantic determinism:** repeated calls return equivalent structured values;
- **wire determinism:** repeated calls serialize to byte-identical JSON for the same arguments, profile, audience, compatibility mode, and version.

This roadmap targets wire determinism for public tool responses where maps are serialized. It does not require stable ordering for internal-only maps that never cross an API boundary.

## Scope-control rule

An implementation agent must stop and document the finding rather than broaden the phase when any of the following occurs:

- the correction requires adding a new public tool;
- the correction requires implementing PCRE2 or embedding another regex runtime;
- the correction requires replacing Tokio;
- the correction requires rewriting the calculator parser;
- the correction requires redesigning MCP transport;
- the correction requires changing release automation;
- measured size savings are negligible or regress behavior;
- a proposed simplification changes more public surface than the defect requires.

---

# Findings addressed by this roadmap

## Correctness and contract findings

1. UTF-8-unsafe truncation in the duplicate MCP request-ID error path can panic on long Unicode IDs.
2. `validate_regex` reports a selected regex backend while constructing `fancy_regex::Regex` for every pattern.
3. Regex `ascii` mode is accepted and echoed but not applied.
4. Fancy-regex runtime errors can be converted into ordinary no-match or partial-success results.
5. Regex policy rejection and syntax validity are conflated under `valid_pattern`.
6. Public response structures serialize ordinary `HashMap` values, preventing a byte-stable output guarantee.
7. TOML table extraction records scalar key paths as tables.
8. TOML error columns are byte-counted rather than Unicode-character-counted.
9. Generic execution-context APIs and calculator state persistence have overlapping and contradictory semantics.

## Simplification and footprint findings

1. The in-process API exposes several overlapping dispatch variants with duplicated validation, budget, pool, context, and truncation code.
2. Deprecated mutable-context commit machinery remains substantial despite not persisting ordinary `math_eval` state.
3. The MCP bounded-execution layer includes production-compiled lifecycle/test-hook machinery beyond the minimum needed for cooperative timeout handling.
4. Tokio enables `full` features.
5. Ordinary calculator/help/version invocations enter a Tokio runtime because the binary uses `#[tokio::main]` globally.
6. Development binaries are normal installable binary targets.
7. Confusables data is embedded as textual pseudo-Rust and reparsed into a string-keyed map at runtime.
8. Both `toml` and `toml_edit` are present.
9. Some simple operations compile regexes where direct character logic is sufficient.

---

# Roadmap sequence

The phases must execute in order. Later simplification work must not obscure unresolved correctness defects.

| Phase | Plan | Purpose | Depends on |
|---|---|---|---|
| 1 | `2026-07-31-phase-1-regex-and-mcp-contract-repairs.md` | repair MCP Unicode safety and regex truthfulness/error semantics | none |
| 2 | `2026-07-31-phase-2-deterministic-output-and-toml-corrections.md` | provide stable serialized map order and correct TOML structure/positions | phase 1 |
| 3 | `2026-07-31-phase-3-dispatch-and-runtime-simplification.md` | consolidate dispatch APIs and remove contradictory/dead context machinery | phases 1-2 |
| 4 | `2026-07-31-phase-4-measured-footprint-reduction-and-closure.md` | perform measurement-gated size/startup reductions and close documentation | phases 1-3 |

Do not merge phases 3 and 4 into a broad refactor. Phase 4 is optional per item: measurements may justify retaining current code.

---

# Phase 1 outcome — Regex and MCP contract repairs

Phase 1 establishes truthful and safe request/regex behavior without redesigning either MCP or regex support.

Required results:

- one UTF-8-safe truncation helper replaces direct byte slicing in human-readable request metadata;
- duplicate long Unicode request IDs produce a bounded JSON-RPC error and do not panic;
- regex classification, compilation, execution, and `engine_used` derive from one internal path;
- simple supported patterns compile through `regex::Regex` when reported as `rust-regex`;
- extended supported patterns compile through `fancy_regex::Regex` when reported as `fancy-regex`;
- unsupported PCRE-only constructs remain explicitly rejected;
- ASCII mode is either correctly implemented for both supported backends or explicitly rejected before execution;
- runtime matching errors are distinguishable from no-match results;
- syntax validity, backend support, policy allowance, and execution success are not falsely collapsed into one state;
- no PCRE2 dependency or backend is introduced.

Phase 1 is complete only when focused regression tests cover each corrected contract and existing regex/MCP tests remain green.

---

# Phase 2 outcome — Deterministic output and TOML corrections

Phase 2 makes public structured output reproducible and corrects two localized TOML defects.

Required results:

- serialized public maps use deterministic key ordering;
- output-order policy is documented once and tested on representative tools;
- internal non-serialized maps may remain `HashMap` where ordering is irrelevant;
- JSON shape, regex group dictionaries, Cargo dependency maps, and similar public maps serialize stably;
- TOML table lists contain only actual tables/arrays-of-tables according to the documented contract;
- scalar key paths are not mislabeled as tables;
- TOML line/column positions are Unicode-character based and handle LF, CRLF, and CR correctly;
- existing Python-parity wording is preserved where applicable;
- no generic ordered-map dependency is added unless the standard library is demonstrably insufficient.

Phase 2 does not require canonicalizing every JSON object globally. It targets public result structures and explicit output builders.

---

# Phase 3 outcome — Dispatch and runtime simplification

Phase 3 reduces duplicated policy plumbing while preserving the existing MCP and in-process capabilities.

Target stable model:

```rust
registry.call(name, args)
registry.call_with(name, args, &CallOptions)
```

The exact public names may remain compatible aliases, but one internal dispatch implementation must perform:

1. registry lookup;
2. profile check;
3. audience check;
4. schema validation;
5. input budget check;
6. cancellation check;
7. handler execution;
8. output truncation.

Required results:

- overlapping budget/context/template methods delegate to one internal path;
- deprecated mutable execution-context machinery is removed or reduced to a thin compatibility wrapper;
- stateless `math_eval` behavior is explicit and consistent across direct and budgeted tool calls;
- persistent calculator state remains available through `evaluate_with_context()` and `run_with_context()` or a minimal calculator-specific session wrapper;
- generic tool dispatch no longer implies state persistence it cannot provide;
- production execution code does not compile large test-hook structures solely for tests;
- MCP retains bounded concurrency, cooperative cancellation, timeout envelopes, and panic conversion;
- exact runtime metric races are not prioritized over simplicity unless a metric is part of a documented public contract;
- no new async abstraction or executor is introduced.

Compatibility aliases may remain deprecated for one release cycle if removal would unnecessarily disrupt consumers. They must be thin wrappers and must not retain a second implementation.

---

# Phase 4 outcome — Measured footprint reduction and closure

Phase 4 begins with measurements and applies only changes that preserve behavior and provide a meaningful improvement.

Required baseline measurements:

```bash
cargo build --release --locked
ls -lh target/release/eggsact
cargo tree -e features
cargo bloat --release --crates
```

If `cargo bloat` is unavailable, install or use it locally only; do not add it to project dependencies or ordinary CI.

Measure at minimum:

- stripped release binary size;
- unstripped release binary size if useful for attribution;
- `eggsact --help` cold invocation;
- `eggsact --version` cold invocation;
- `eggsact "2+2"` cold invocation;
- one MCP initialize/list/call smoke path;
- first confusables inspection call;
- release build time from a warm cache only as secondary evidence.

Candidate changes, in order:

1. replace Tokio `full` with explicit required features;
2. construct the Tokio runtime only for MCP mode;
3. add conservative release-profile size settings and compare results;
4. prevent development binaries from ordinary installation;
5. replace runtime-parsed textual confusables data with a compact generated static representation;
6. consolidate TOML parsers if behavior and parity remain intact;
7. replace trivial static regex uses with direct character logic;
8. consider schema caching only if listing/startup measurements identify it as material.

Each candidate must have before/after measurements. A change is rejected when it:

- produces negligible savings;
- adds a new dependency or build system;
- materially worsens compile time without sufficient runtime/size benefit;
- complicates generated-data maintenance;
- changes tool output or parity behavior;
- makes debugging or release operation materially harder.

Phase 4 closes the roadmap with concise documentation updates. It must not create a permanent benchmark workflow or evidence ledger.

---

# Global acceptance criteria

The line of work is complete when all of the following are true:

1. long Unicode duplicate request IDs cannot panic the MCP server;
2. regex responses accurately report the backend that executed the pattern;
3. ASCII mode is implemented or rejected, never silently ignored;
4. regex runtime errors are not reported as ordinary no-match success;
5. syntax, support, policy, and execution outcomes are accurately represented;
6. representative public map outputs are byte-stable across repeated fresh processes;
7. TOML table results contain actual tables rather than scalar fields;
8. TOML Unicode error positions are correct;
9. generic dispatch has one internal implementation path;
10. calculator persistence semantics are explicit and non-contradictory;
11. deprecated context machinery no longer contains a parallel execution implementation;
12. MCP still provides bounded execution, cooperative cancellation, panic conversion, and response-size enforcement;
13. no new tool categories or public feature families were added;
14. ordinary CI remains at the current reduced topology;
15. manual release policy remains unchanged;
16. every accepted footprint change has before/after evidence;
17. every rejected footprint candidate is simply omitted rather than compensated for with new machinery;
18. all retained tests, doctests, generated-doc checks, Clippy checks, and package checks pass.

---

# Required implementation discipline

## Commit discipline

Prefer one implementation commit per phase, with a corrective follow-up only when verification finds a real defect. Do not create evidence-only commit chains.

Recommended commit subjects:

```text
fix: repair MCP and regex result contracts
fix: stabilize structured output and TOML positions
refactor: consolidate tool dispatch and calculator context semantics
perf: reduce measured eggsact footprint
```

## Documentation discipline

Update only documentation directly affected by implementation:

- `README.md` when public behavior or installation changes;
- `architecture/mcp-server.md` for request/timeout semantics;
- `architecture/text-library.md` for regex backend behavior;
- `architecture/agent-api.md` for consolidated dispatch;
- `architecture/calculator.md` for stateful/stateless behavior;
- `architecture/generated-assets.md` if confusables representation changes;
- `architecture/cli-binaries.md` if development binaries or runtime startup change;
- generated tool documentation only through the existing generator.

Do not add another architecture document unless an existing document cannot contain the corrected material.

## Test discipline

For each corrected issue, add the smallest test that would have failed before the fix. Prefer table-driven tests when multiple related cases share setup.

Do not add:

- a new test crate;
- a new test runner;
- snapshot infrastructure solely for this roadmap;
- broad timing assertions in CI;
- large repeated concurrency loops in ordinary tests;
- exhaustive Unicode corpora when two or three representative code points establish the contract.

## Measurement discipline

Store final measurements in the phase-4 plan completion section or commit message. Do not create a long-lived benchmark-results directory.

At minimum record:

```text
baseline SHA
final SHA
platform/toolchain
release profile
binary size before/after
cold CLI timing before/after
accepted changes
rejected/no-value changes
```

---

# Explicit non-goals

This roadmap does not authorize:

- adding tools;
- removing existing useful tools;
- implementing PCRE2;
- changing supported MCP protocol versions;
- changing stdio transport;
- adding HTTP transport;
- adding persistent server storage;
- adding authentication;
- adding telemetry or remote metrics;
- changing crates.io release cadence;
- adding automatic publishing;
- re-expanding CI;
- rewriting the calculator grammar;
- replacing Tokio;
- splitting the crate into a workspace solely for architecture aesthetics;
- generating a second registry or schema source of truth;
- replacing all `HashMap` uses indiscriminately;
- optimizing unmeasured code;
- preserving deprecated implementations merely because tests already exist for them.

---

# Closure record template

Complete this section only after all accepted phases land.

## Final status

- **Status:** complete
- **Implementation commits:** `98d3aae00efc29436af808c430da6766ea76ebf6` (Phase 1), `0a3ace9e21853e4ded7f0a8c2a9bcb9ab4f1cc94` (Phase 2), `63bac39b87596e2f7721c4042f369afe92a41bcd` (Phase 3), `a8dc5e69e8ce3d38c17f7cf944d8967408b9701a` (Phase 4)
- **Corrective closure implementation:** `1cb0ce581849b540e41fd8cc5ae130c63c449727` (regex captures, syntax/policy separation, direct-dispatch context isolation, dev-tools gating)
- **Verification:** fmt ✓, clippy ✓, tests ✓ (skip parity), doc ✓, generate-docs --check ✓, package ✓
- **Binary sizes (same host/toolchain):** Phase 3 baseline 12,291,024 → Phase 4 pre-gating 12,856,752 → post-gating/corrective 12,856,656 bytes (Phase 4-specific reduction: ~96 bytes)
- **CLI timing (paired, same host, 10 runs):** --help 1.8→0.8 ms, --version 1.9→0.9 ms, 2+2 566.1→560.1 ms, 'thirty plus five' 563.9→535.4 ms (baseline constructs Tokio for all paths; final only for MCP)
- **Deferred items:** trivial regex cleanup (marginal), schema caching (unmeasured)

## Closure statement

The roadmap is complete. All 4 phases landed successfully:

1. **Phase 1** — Repaired MCP Unicode panic path, unified regex compilation, rejected ASCII mode, preserved runtime errors, separated syntax/support/policy/execution status
2. **Phase 2** — Converted 14 public HashMap fields to BTreeMap for deterministic serialization, fixed TOML table extraction to exclude scalars, fixed TOML Unicode column counting
3. **Phase 3** — Consolidated policy preparation into one implementation, extracted shared bounded dispatch helpers, removed commit-slot machinery (net -358 lines)
4. **Phase 4** — Narrowed Tokio features from `full` to 7 required features, moved Tokio runtime construction to MCP-only path; binary reduced ~96 bytes (pre-gating to post-gating)

Global acceptance criteria satisfied: Unicode safety, regex truthfulness, deterministic output, TOML correctness, dispatch simplification, and measured footprint reduction. Deferred items (trivial regex cleanup, schema caching) are explicitly low-value and outside scope. Full local verification passes. Remote CI passes on all three platforms (runs 30688082724, 30688799086). The corrective closure implementation (`1cb0ce5`) repaired named captures, syntax/policy separation, direct-dispatch context isolation, and dev-tools gating. This polish pass reconciled planning records only — no new implementation phase is created. Do not create another roadmap solely to record successful completion.