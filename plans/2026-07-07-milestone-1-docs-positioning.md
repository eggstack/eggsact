# Milestone 1: Documentation and Positioning Cleanup

Date: 2026-07-07

Parent roadmap: `plans/2026-07-07-coding-agent-hardening-roadmap.md`

## Objective

Make the public and maintainer-facing documentation accurately describe `eggsact` as a deterministic MCP and in-process utility substrate for coding agents. The repo still carries some legacy calculator-centered framing from the original `eggcalc` lineage. That history should remain visible, but it should not dominate the crate identity.

The target result is a coherent documentation set where README, crate docs, architecture docs, generated tool docs, metadata, examples, and diagnostics all agree on what the crate does, how tools are exposed, and how coding-agent integrations should consume it.

## Rationale

The crate already exposes a broad utility surface: math, text, JSON, regex, path, shell, Markdown, config, Unicode, patch, dependency, repo, diagnostics, and typed preflight helpers. The README correctly frames the project as deterministic MCP and in-process utility tools for coding agents, but `src/lib.rs` still opens with “Natural Language Math Calculator with MCP Server.” That mismatch can mislead users browsing docs.rs or crates.io.

There is also visible architecture-doc drift. For example, one section lists the path module without `path_batch_scope_check`, while a later category table includes it. Config and TOML categorization is also inconsistent across docs. This kind of drift is especially harmful for agent tooling because downstream harnesses may rely on profile/category documentation when deciding what to expose.

## Scope

In scope:

- README positioning and consistency pass.
- `src/lib.rs` crate-level documentation rewrite.
- Architecture doc cleanup, especially `architecture/mcp-server.md`.
- Generated docs verification.
- New maintainer docs for generated assets and parity workflow.
- New integration guide for coding agents and codegg-style harnesses.
- Cross-check of tool counts, categories, profile names, and exposure semantics.

Out of scope:

- Tool implementation changes.
- Registry restructuring.
- New tool additions.
- MCP protocol behavior changes.
- CI workflow changes, except where docs-generation checks require minor updates.

## Files likely to change

- `README.md`
- `src/lib.rs`
- `architecture/mcp-server.md`
- `architecture/compatibility.md` if terminology needs alignment
- `architecture/machine-codes.md` if route-critical wording needs cross-links
- `architecture/generated-assets.md` or similarly named new file
- `architecture/coding-agent-integration.md` or similarly named new file
- Any generated documentation block produced by `cargo run --bin generate-docs`

## Implementation plan

### 1. Reframe crate-level documentation

Update `src/lib.rs` so the first paragraph says that `eggsact` provides deterministic MCP and in-process utility tools for coding agents. Preserve calculator examples, but present them as one category of deterministic tools.

Suggested opening direction:

```rust
//! eggsact - deterministic MCP and in-process utility tools for coding agents.
//!
//! This crate provides local, deterministic helpers for math, text processing,
//! structured data, paths, Unicode safety, shell preflight, config validation,
//! patch review, dependency inspection, and repository preflight workflows.
```

Keep the quick start examples, but add at least one non-math example near the top, such as `text_equal`, `command_preflight`, or `config_preflight`. The doc examples should compile.

### 2. Normalize README positioning

Review the README introduction and ensure it explicitly states:

- `eggsact` is deterministic and local.
- It exposes both MCP stdio and in-process Rust APIs.
- It is useful for coding agents because it reduces avoidable hallucination around exact text, paths, JSON/TOML, shell command structure, Unicode hazards, and patch/config preflight.
- The natural-language calculator is retained as a useful deterministic tool category, not the project’s only purpose.

Avoid making claims about full static analysis, full JSON Schema, full shell safety, or full language parsing.

### 3. Fix architecture doc drift

Audit `architecture/mcp-server.md` against the generated tool list. At minimum:

- Ensure path tools list includes `path_batch_scope_check` wherever path tools are enumerated.
- Ensure patch tools list includes `diff_risk_classify` wherever patch tools are enumerated.
- Ensure TOML/config categorization is consistent with current registry and README generated output.
- Ensure dependency and repo categories are listed if present in the registry.
- Ensure route-critical tool list matches `ROUTE_CRITICAL_TOOLS`.
- Ensure audience/exposure behavior matches current `ToolAudience` and `ToolListAudience` implementation.

Prefer generated docs as the source of truth for counts. If a manually maintained table is too prone to drift, replace exact long lists with a short description plus a pointer to generated docs.

### 4. Add generated-assets and parity workflow doc

Create a short maintainer document explaining:

- How generated tool docs are produced.
- What `cargo run --bin generate-docs -- --check` validates.
- Which generated source files are required at build/package time.
- What `src/text/confusables_generated.rs` is and how it should be regenerated if applicable.
- Why parity tests against Python `eggcalc` are skipped in CI.
- How to run parity tests locally when the Python reference package is available.
- How `eggsact --diagnostics` and `eggsact --diagnostics --format json` help inspect generated-data status.

This document should be concrete enough that a new maintainer can update generated assets without guessing.

### 5. Add coding-agent integration guide

Create a focused integration guide for codegg-like harnesses. It should cover:

- MCP stdio use via `eggsact --mcp`.
- In-process use via `eggsact::agent::ToolRegistry`.
- When to use `ToolAudience::Model` vs `ToolAudience::Harness`.
- Recommended profiles by workflow:
  - `codegg_core_min` for minimal model-visible tools.
  - `codegg_core` for normal coding sessions.
  - `codegg_preflight` for harness-driven safety checks.
  - `codegg_patch` for edit and patch workflows.
  - `codegg_config` for config validation.
  - `codegg_unicode_security` for suspicious text/identifier review.
  - `codegg_shell` for command planning and preflight.
  - `codegg_repo_audit` for repository inspection.
- Why harness-only tools should not be presented directly to the model in ordinary sessions.
- How to use `available_tools_model_safe()` and `available_tools_for_current_audience()`.
- How to correlate concurrent MCP responses by JSON-RPC `id` instead of arrival order.

### 6. Regenerate docs and check generated block

Run:

```bash
cargo run --bin generate-docs -- --check
```

If the check fails because manual docs changed the generated region, run the generation command without `--check`, inspect the diff, and commit regenerated output only if the generated changes are expected.

### 7. Verify examples and docs

Run:

```bash
cargo fmt --all -- --check
cargo test --all-features --doc
cargo test --all-features --lib
cargo test --all-features --bins
cargo test --all-features --tests -- --skip parity
```

If doctests fail because examples use harness-only tools with model audience, either adjust the example audience or choose a model-safe tool.

## Testing requirements

This milestone is documentation-heavy, but it should still leave the repository mechanically verified.

Required checks:

- `cargo fmt --all -- --check`
- `cargo run --bin generate-docs -- --check`
- `cargo test --all-features --doc`
- `cargo test --all-features --lib`
- `cargo test --all-features --bins`
- `cargo test --all-features --tests -- --skip parity`
- `cargo package --verbose`

Optional but useful:

- `cargo clippy --all-targets --all-features -- -D warnings`

## Acceptance criteria

- `src/lib.rs` no longer presents `eggsact` primarily as a natural-language calculator.
- README, crate docs, and architecture docs consistently describe the coding-agent utility substrate role.
- Tool category counts and names do not conflict across README and architecture docs.
- Generated-doc check passes.
- A new generated-assets/parity workflow doc exists.
- A new coding-agent integration guide exists.
- Profile and audience guidance is explicit enough for codegg handoff.
- No docs imply full JSON Schema support, full shell safety, full language parsing, or network-backed analysis.

## Review checklist

Before marking the milestone complete, verify:

- All examples compile or are intentionally marked non-compiling.
- All references to `eggcalc` are historical or compatibility-specific.
- All profile names are exact registry names.
- All route-critical tool names match the registry helper.
- The MCP concurrency contract says responses may arrive out of order and clients must correlate by ID.
- The docs explain that harness-oriented checks should use harness audience rather than exposing harness-only tools to ordinary model-facing sessions.

## Handoff notes

This milestone should be done before adding new tools. Otherwise new tool docs may inherit stale framing and category drift. Keep edits scoped to documentation and generated-doc alignment unless a small code-doc comment change is needed for correctness.
