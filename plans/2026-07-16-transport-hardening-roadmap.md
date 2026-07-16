# eggsact Transport Hardening and Protocol Maturity Roadmap

## Status

Handoff roadmap for the post-v1.1.4 hardening line of work identified during the 2026-07-16 repository review.

## Context

eggsact has achieved its primary functional objective: it provides deterministic MCP and in-process utility tools for coding agents, with a registry-driven tool catalog, machine-readable routing codes, typed preflight wrappers, profile/audience controls, extensive regression coverage, and Python eggcalc compatibility fixtures.

The next development cycle should not optimize for adding tools. The highest-risk remaining gaps are concentrated in the MCP transport and execution boundary:

- timed-out `spawn_blocking` handlers may continue after the worker permit has been released;
- cancellation notifications can be rate-limited, answered, or silently dropped under mutex contention;
- duplicate in-flight JSON-RPC IDs can overwrite active-request tracking;
- MCP initialization is permissive and does not negotiate protocol versions or capabilities;
- MCP operation still relies partly on process-global calculator state;
- registry metadata lookup and execution policy are not fully audience-consistent;
- CI does not yet enforce MSRV, cross-platform behavior, dependency policy, or scheduled parity evidence.

This roadmap moves eggsact from a feature-complete deterministic utility server toward a bounded, lifecycle-correct, auditable execution substrate suitable for codegg and external MCP clients.

## Strategic objective

At roadmap completion, eggsact should provide:

1. Strictly bounded blocking execution even after client-facing timeouts.
2. Reliable, response-free, quota-independent cancellation.
3. Unambiguous active-request identity and cleanup.
4. Typed MCP initialization, protocol negotiation, lifecycle enforcement, and capability tracking.
5. Per-request and optional persistent state without MCP-induced process-global mutations.
6. Consistent profile/audience policy across listing, metadata inspection, and execution.
7. Reproducible releases with MSRV, multi-platform, dependency-policy, parity, fuzz, and performance evidence.
8. A documented stable API boundary for Rust consumers and MCP extensions.

## Guiding constraints

- Preserve the existing tool schemas and route-critical machine-code behavior unless a documented protocol correction requires a change.
- Do not add a second asynchronous runtime or an unbounded helper-process architecture.
- Do not weaken input limits, safety checks, or deterministic defaults to improve compatibility.
- Prefer explicit state objects and RAII guards over process-global switches and manual cleanup.
- Keep the in-process API first-class; MCP hardening must not force IPC overhead onto codegg.
- New tools are out of scope unless required to close a demonstrated workflow or diagnostics gap.

# Release sequence

## Release 1 — Execution Safety and Concurrent Request Integrity

### Scope

- Retain worker permits until blocking handlers actually terminate.
- Separate response timeout from worker occupancy.
- Correct cancellation ordering, quota handling, locking, and notification semantics.
- Reject duplicate active request IDs.
- Make active-request cleanup panic-safe and shutdown-safe.
- Add adversarial timeout, cancellation, and duplicate-ID tests.

### Primary files

- `src/mcp/server.rs`
- `src/mcp/runtime.rs`
- `src/mcp/budget.rs`
- `src/tools/helpers.rs`
- `tests/mcp/test_cancellation.rs`
- `tests/mcp/test_protocol.rs`
- `tests/mcp/test_determinism_concurrency.rs`
- `tests/mcp/test_hardening_and_gaps.rs`
- `architecture/mcp-server.md`
- `architecture/testing.md`

### Exit gate

The configured worker limit must be a hard upper bound on actual running blocking handlers, and cancellation must remain reliable under rate saturation, lock contention, timeout, and concurrent completion.

## Release 2 — MCP Lifecycle and Protocol Negotiation

### Scope

- Add typed initialize parameters and client metadata.
- Maintain an explicit supported-protocol-version table.
- Negotiate protocol revision rather than returning one hard-coded value.
- Add a per-connection lifecycle state machine.
- Reject ordinary tool operations before initialization completes.
- Track negotiated client capabilities.
- Advertise eggsact-specific extensions explicitly.
- Add realistic multi-message lifecycle conformance sessions.

### Primary files

- `src/mcp/protocol.rs`
- `src/mcp/server.rs`
- `src/mcp/runtime.rs`
- `tests/mcp/test_protocol.rs`
- `tests/mcp/test_lifecycle_and_gaps.rs`
- `tests/mcp/test_comprehensive_parity.rs`
- `architecture/mcp-server.md`
- `README.md`

### Exit gate

Every MCP session must have an explicit lifecycle and negotiated protocol revision before normal operation. Legacy compatibility must be intentional and tested rather than implicit.

## Release 3 — State Isolation and Policy Consistency

### Scope

- Remove MCP dependence on global evaluator mode.
- Distinguish immutable execution templates from mutable persistent execution contexts.
- Add mutable context dispatch for calculator state.
- Make thread-local context restoration panic-safe.
- Make `get_tool` and `has_tool` audience-aware.
- Consolidate profile/audience/schema validation into one policy path.
- Add mixed-surface and policy-invariant tests.

### Primary files

- `src/calc/context.rs`
- `src/calc/evaluator.rs`
- `src/agent/mod.rs`
- `src/mcp/server.rs`
- `src/mcp/runtime.rs`
- `src/mcp/budget.rs`
- `tests/test_context_isolation.rs`
- `tests/mcp/test_hardening_and_gaps.rs`
- `architecture/agent-api.md`
- `architecture/calculator.md`
- `architecture/compatibility.md`

### Exit gate

MCP calls must not mutate process-global calculator behavior, execution-context persistence semantics must be explicit, and any tool visible through registry metadata must be executable under the same profile/audience policy.

## Release 4 — Verification Infrastructure

### Scope

- Declare and test an MSRV.
- Add targeted Windows and macOS CI jobs.
- Run `cargo deny check` in CI and release verification.
- Track `Cargo.lock` for repository and binary reproducibility.
- Add a latest-compatible-dependencies job.
- Add scheduled Python parity validation.
- Add package-content and release provenance checks.

### Exit gate

A release commit must be reproducibly validated across supported Rust and operating-system targets with dependency, license, parity, and package evidence.

## Release 5 — Fuzzing and Property Testing

### Scope

Add fuzz and property coverage for calculator normalization, expression parsing, unified diffs, shell tokenization and quoting, regex classification, JSON pointers, TOML/config parsing, Unicode inspection, Markdown fences, and glob matching.

### Exit gate

Parser-heavy surfaces have persistent fuzz corpora, minimized regression fixtures, and reversible/idempotent transformation properties where applicable.

## Release 6 — Performance Characterization

### Scope

Add repeatable benchmarks for registry lookup, schema generation, in-process dispatch, MCP stdio overhead, diffing, regex, Unicode inspection, structured data comparison, composite preflights, and repository analysis. Measure cold/warm behavior, scaling, concurrent load, and cancellation latency.

### Exit gate

Operational limits and timeout tiers are supported by measured scaling data, and material regressions are visible before release.

## Release 7 — Feature and Dependency Decomposition

### Scope

Introduce Cargo features for calculator, text, agent, preflight, MCP, and full builds; reduce Tokio features; make subsystem dependencies optional; consider a crate split only if measurements justify it.

### Exit gate

Consumers can build smaller subsets without changing default behavior, and Tokio is absent when MCP support is disabled.

## Release 8 — CLI and Diagnostics Maturity

### Scope

Use strict argument parsing, reject invalid diagnostics formats, separate installed-runtime diagnostics from source-checkout diagnostics, version the JSON diagnostic schema, and add an MCP self-check command.

### Exit gate

The installed binary behaves predictably outside a checkout and exposes stable machine-readable operational diagnostics.

## Release 9 — Public API and Stable Release Closure

### Scope

Audit public visibility, classify stable/experimental/deprecated surfaces, improve structured public errors, stabilize machine-code policy, version eggsact MCP extensions, write migration guidance, and complete a release-candidate evidence pass.

### Exit gate

The Rust API, MCP extensions, machine-code guarantees, migration paths, and release evidence are explicit enough for a stable long-lived public baseline.

# Dependency order

Release 1 must land before Release 2 because lifecycle enforcement depends on trustworthy request tracking and cancellation. Release 2 should land before Release 3 so the final state model can be designed around a negotiated MCP session rather than the current permissive transport. Releases 4–6 may begin in parallel after Release 1, but their final gates should validate the Release 2 and Release 3 architecture. Feature decomposition should remain later work to avoid complicating correctness changes with conditional compilation.

# Validation baseline for every release

Each release must run at minimum:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --lib
cargo test --all-features --bins
cargo test --all-features --tests -- --skip parity
cargo test --doc
cargo run --bin generate-docs -- --check
cargo package --verbose
```

Where the Python reference is available, run the parity suite and verify that all failures are either fixed or already listed in `tests/fixtures/accepted_parity_failures.txt`. New accepted failures require a documented decision record; they must not be added merely to make the suite green.

# Roadmap completion definition

This line of work is complete when eggsact can be embedded or run as an MCP server under concurrent, malformed, or adversarial workloads without escaping configured execution bounds; when protocol lifecycle and state behavior are explicit; and when releases are supported by reproducible cross-platform, dependency, parity, fuzz, and performance evidence.