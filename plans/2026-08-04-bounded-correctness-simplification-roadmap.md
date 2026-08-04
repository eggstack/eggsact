# Bounded Correctness, Simplification, and Footprint Roadmap

## Status

- **Status:** planned
- **Repository:** `eggstack/eggsact`
- **Target branch:** `main`
- **Planning baseline:** `48159ec4b829561f8569a7d130488ddeefb97a1c`
- **Scope:** close the current soundness and correctness findings, remove test-driven production behavior, repair release/verification inconsistencies, and reduce footprint only where measured evidence supports a simple change
- **Primary objective:** preserve eggsact's existing deterministic utility scope while making its implementation safer, smaller, and easier to maintain
- **Release policy:** crates.io publication remains a direct manual maintainer action; this roadmap does not publish, tag, or determine release cadence

## Purpose

Eggsact is accomplishing its intended product goal. It is a local stdio MCP server and in-process Rust library that gives coding agents deterministic operations they should not approximate probabilistically. The existing tool catalog remains useful and in scope.

The repository does not need another feature program. It needs a bounded corrective pass addressing defects that remain after the July 31 lightweight-correctness work:

1. remove an unsound thread-local `EvalContext` bridge that can manufacture aliased or dangling `&'static mut` references;
2. correct Windows UNC/share-boundary handling in path normalization and scope checks;
3. ensure request-size and response-truncation limits describe and enforce what they claim;
4. remove a 120-second production timeout override introduced solely to make parallel integration tests pass;
5. repair feature-gated development-binary invocations and collapse duplicate release verification entry points;
6. simplify ordinary CI for a locally deployed, manually released utility without weakening focused correctness coverage;
7. evaluate high-confidence binary/startup reductions, especially generated confusables representation and release-profile settings;
8. stop once these findings are closed rather than extending the project into a general execution framework.

This roadmap favors deletion, explicit ownership, and focused regression tests. It does not authorize broad redesign.

---

# Product boundary

Eggsact remains:

- one Rust crate rather than a workspace;
- a local stdio MCP server;
- an in-process Rust utility library;
- a deterministic collection of existing coding-agent tools;
- compatible with documented eggcalc behavior where that compatibility is intentional;
- manually published to crates.io;
- suitable for local workstation, SBC, and LAN-adjacent agent workflows rather than an internet-facing multi-tenant service.

Eggsact does not become:

- a new tool catalog expansion;
- a remote service or daemon;
- a plugin framework;
- a generalized policy engine;
- a sandbox or process supervisor;
- a full PCRE2 implementation;
- a replacement async runtime;
- a multi-crate architecture;
- a benchmark or evidence-generation system;
- a release automation platform.

---

# Governing constraints

## Scope constraint

No phase may add:

- a new MCP tool;
- a new tool category;
- a new built-in profile or audience;
- a new public protocol extension;
- a new background service;
- a new CI workflow family;
- a new mandatory verification harness;
- a new dependency unless it clearly deletes more code and maintenance burden than it introduces.

If a finding appears to require one of these, stop and record it as deferred rather than broadening implementation.

## Architecture constraint

The following are explicitly out of scope for this line of work:

- wholesale unification or replacement of the MCP and synchronous execution engines;
- rewriting the calculator parser or evaluator;
- replacing Tokio;
- changing the MCP transport from stdio;
- redesigning every handler to accept a new context type;
- changing the full public response schema;
- redesigning all path utilities around a third-party virtual-filesystem abstraction.

A small internal closure-based context helper, focused path-prefix type, or shared release script is acceptable. A framework is not.

## Compatibility constraint

Preserve the existing 80-tool functional surface. Public behavior may change only when the current behavior is:

- unsound;
- demonstrably incorrect;
- internally contradictory;
- falsely reported;
- explicitly documented as a test-only workaround;
- part of a broken command path.

Corrections must retain existing field names where practical and receive focused regression tests. Do not use this roadmap to make unrelated breaking API changes.

## Verification constraint

Use the nearest existing test layer. Add only tests that prove the corrected contract.

For ordinary implementation commits, the expected gate is:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity
cargo test --locked --doc
cargo run --locked --features dev-tools --bin generate-docs -- --check
```

Use focused commands during development. Run `cargo package --locked` and `cargo publish --locked --dry-run` only in the canonical local release check, not as a new per-commit burden.

Parity runs are required only when calculator/Python-compatibility behavior changes. Fuzzing remains manual and targeted to affected parser/path/regex surfaces.

## Evidence constraint

Completion evidence is concise:

- implementation commit SHA;
- targeted test names or commands;
- ordinary verification result;
- measured before/after data only for footprint changes;
- explicit disposition for rejected measurement candidates.

Do not create artifact archives, run-ID registries, package-file counts, exact-SHA evidence loops, or repeated documentation-only closure commits.

## Stop rule

Stop this roadmap when all acceptance criteria in phases 1 through 5 are satisfied or explicitly rejected under their measurement gates. Do not create another polish or evidence-only plan unless a new reproducible product defect is found.

---

# Findings covered

## Critical

### F1 — Unsound `EvalContext` retrieval

`current_eval_context()` is a safe public function that returns `Option<&'static mut EvalContext>` from a thread-local raw pointer. Safe callers can obtain multiple mutable references to the same object or retain a reference beyond the enclosing scope, creating undefined behavior.

The paired `PREV_EVAL_CONTEXT` and `PREV_CANCEL_FLAG` single-slot restoration mechanism also does not correctly support arbitrary nesting despite documenting nested restoration.

## High

### F2 — UNC share-boundary traversal

Windows UNC handling identifies protected components by component text rather than position. Dot-segment collapse can traverse above `\\host\share`, and `path_scope_check()` consumes the resulting lexical normalization.

### F3 — Test load changes production timeout policy

`math_eval`, `text_diff_explain`, and `regex_finditer` receive a 120-second MCP budget because parallel integration tests can starve blocking workers. The documented moderate budget is 30 seconds, and the synchronous API does not apply the same override. Test infrastructure is therefore changing production behavior.

### F4 — Canonical release commands are inconsistent or broken

`generate-docs` and `verify-eggsact` are feature-gated behind `dev-tools`, but multiple scripts and diagnostics invoke them without the required feature. `release.sh`, `scripts/release-check.sh`, and `verify-eggsact` duplicate overlapping verification paths.

## Medium

### F5 — Request-size limit does not bound line allocation

The stdio server reads an entire line before comparing it with `MAX_REQUEST_BYTES`. An unterminated oversized line can allocate beyond the declared limit.

### F6 — Findings truncation misreports omitted count

The truncation path reserves one slot for the synthetic notice but calculates omitted findings as though all `max_findings` slots contained real findings.

### F7 — Resource pressure is reported as invalid request

Rate-limit and in-flight-capacity failures use JSON-RPC `-32600` even though the requests are structurally valid. The fixed 10 request/second limiter may also be unnecessary for a local stdio server once bounded in-flight and worker limits are enforced.

## Maintenance and footprint

### F8 — Confusables data is embedded and reparsed inefficiently

Generated pseudo-Rust text is embedded with `include_str!`, parsed into a runtime `HashMap`, and queried using per-character `format!` allocations. A generated sorted static table can preserve the full dataset while deleting runtime parsing and allocation.

### F9 — Release builds have no measured project-specific size profile

The crate should evaluate `strip`, thin LTO, and codegen-unit settings without adopting settings that undermine panic conversion or materially regress runtime.

### F10 — Remaining dependency/feature candidates require measurement

Potential candidates include a current-thread Tokio runtime, `serde_json/preserve_order`, and duplicate TOML parser usage. These are evaluation candidates, not mandated refactors. Reject them when compatibility, complexity, or savings do not justify change.

---

# Roadmap sequence

The phases execute in order:

| Phase | Plan | Purpose | Depends on |
|---|---|---|---|
| 1 | `2026-08-04-phase-1-execution-context-soundness.md` | remove undefined behavior and make nested context restoration structurally correct | none |
| 2 | `2026-08-04-phase-2-path-and-wire-boundary-corrections.md` | correct UNC scope semantics, bounded request reading, truncation accounting, and resource error classification | phase 1 |
| 3 | `2026-08-04-phase-3-timeout-policy-and-test-isolation.md` | remove test-driven production budgets and stabilize tests without expanding execution architecture | phases 1-2 |
| 4 | `2026-08-04-phase-4-release-and-ci-simplification.md` | restore one canonical local release gate and reduce routine CI ceremony | phases 1-3 |
| 5 | `2026-08-04-phase-5-measured-footprint-reduction-and-closure.md` | evaluate simple measured footprint changes and close the roadmap once | phases 1-4 |

Do not merge phases 1 through 3 into a general runtime rewrite. Phase 5 candidates are independently accepted or rejected.

---

# Phase outcomes

## Phase 1 — Execution-context soundness

Required outcome:

- no safe API returns `&'static mut EvalContext` from thread-local state;
- context access occurs through a closure whose mutable borrow cannot escape;
- restoration state is owned by each guard or represented by an actual stack;
- nesting of at least three levels restores the exact previous context and cancellation flag;
- panic/unwind restoration is covered;
- `math_eval` and direct registry dispatch preserve current deterministic behavior;
- no handler-signature redesign or broad context framework is introduced.

## Phase 2 — Path and wire boundaries

Required outcome:

- UNC host/share prefixes are represented by position and cannot be popped by `..`;
- path scope checks reject traversal above a share root;
- drive-relative paths remain distinguishable from drive-rooted absolute paths;
- oversized stdio requests are rejected before unbounded line allocation;
- truncation notices report the exact number of omitted findings, including `max_findings = 0` and `1` edge cases;
- capacity and rate-limit errors use a server/resource classification rather than `-32600`;
- the necessity of the 10 request/second limiter is decided narrowly, with removal preferred if in-flight/worker bounds are sufficient.

## Phase 3 — Timeout policy and test isolation

Required outcome:

- production MCP budgets derive from declared tool cost and explicit product policy only;
- no production constant or branch is justified solely by parallel test load;
- MCP and in-process APIs agree on the default elapsed budget for the same tool/cost unless a documented transport-specific reason exists;
- affected tests use deterministic gates, isolated pools/semaphores, or bounded test-local overrides rather than global production inflation;
- existing cooperative cancellation and panic conversion remain;
- no wholesale execution-engine unification is attempted.

## Phase 4 — Release and CI simplification

Required outcome:

- `scripts/release-check.sh` is the sole canonical full local release gate;
- every feature-gated development binary invocation includes `--features dev-tools`;
- `release.sh` and `verify-eggsact` are deleted or reduced to trivial delegation only when compatibility requires retention;
- diagnostics advertise commands that actually work in a source checkout and do not imply source-tree files should exist after `cargo install`;
- ordinary CI is limited to merge-relevant correctness;
- package and publish dry-run checks remain local release actions;
- cross-platform checks and maintenance workflows use a cadence proportional to the published support promise;
- no publication, tagging, provenance, or artifact-upload automation is added.

## Phase 5 — Measured footprint reduction and closure

Required outcome:

- establish one reproducible binary/startup baseline;
- replace runtime confusables-text parsing with a generated compact static representation if parity and measurement pass;
- pin the Unicode source version/checksum used by the generator;
- evaluate conservative release-profile settings;
- evaluate current-thread Tokio, preserve-order removal, and parser dependency consolidation only as isolated candidates;
- retain candidates that save meaningful space or plainly delete complexity without behavior loss;
- reject speculative candidates explicitly rather than forcing every optimization;
- record one concise closure statement and stop.

---

# Priority and dependency rules

1. Soundness work blocks every later phase.
2. Path/wire correctness blocks timeout and release cleanup because it changes runtime contracts.
3. Test isolation must land before changing CI cadence, so reduced CI is not used to conceal flaky behavior.
4. Release command repair must land before footprint work uses release artifacts for measurement.
5. Footprint changes occur last so measurements compare stable behavior.

---

# Required implementation discipline

Each phase implementation should use small commits with one behavioral theme. A smaller execution model should be able to follow the plan without inventing design:

1. inspect the listed files and rejection searches;
2. add the focused failing regression test where feasible;
3. implement the smallest correction;
4. run targeted tests;
5. run the ordinary verification gate;
6. update affected documentation in the same implementation commit or one bounded companion commit;
7. fill the phase completion record once.

No phase should produce a second roadmap. If a phase uncovers an unrelated issue, record it under `Deferred findings` and continue only when it blocks the listed acceptance criteria.

---

# Global acceptance criteria

This roadmap is complete when:

- safe Rust cannot obtain an escaping or aliased mutable evaluation context from the dispatch bridge;
- nested context and cancellation restoration are correct under normal return and unwind;
- UNC share roots cannot be traversed lexically;
- request allocation is bounded by the declared request limit;
- output truncation accounting is exact;
- production timeouts no longer contain test-load exceptions;
- one canonical release check works with feature-gated development binaries;
- routine CI is proportionate to this crate's local/manual-release scope;
- the complete 80-tool surface remains available;
- accepted footprint changes have recorded before/after measurements;
- rejected footprint candidates have concise reasons;
- no new subsystem, tool family, workflow family, or release ceremony was added;
- all five phase completion records are filled and the roadmap receives one final status update.

## Final stop condition

After the global criteria pass, mark this roadmap `complete` with the implementation commit range and a short closure statement. Do not create additional evidence-only plans or commits.