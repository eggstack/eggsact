# eggsact Agent Substrate Roadmap

## Purpose

`eggsact` has grown beyond a Rust rewrite of `eggcalc`. It is becoming a deterministic local utility layer for coding agents: calculator parity remains useful, but the higher-value role is to provide low-entropy, machine-checkable operations that codegg can run before, during, and after model reasoning. This roadmap treats MCP as one transport adapter over a more general deterministic tool substrate.

The long-term target is a crate that can be used in three modes:

1. As a CLI calculator and utility binary.
2. As an MCP stdio server exposing curated tools to agents.
3. As an in-process Rust library that codegg can call directly for harness-only validation, preflight, and safety checks.

The desired end state is a single-source tool registry, category-local tool implementations, generated MCP schemas and docs, typed in-process APIs, stable machine-readable result contracts, curated model-facing profiles, and deterministic codegg preflight workflows for edits, commands, configs, paths, Unicode, and repo manifests.

## Current Architectural Pressure

The current MCP implementation is functionally strong but structurally at the point where additional feature work will become expensive. The main issue is duplicated registration state. Tool handlers, metadata, raw tool definitions, output schemas, profile membership, docs, and tests are maintained across separate surfaces. The existing sync tests are valuable, but they compensate for a design that should become single-source.

The second pressure point is module size and responsibility mixing. `src/mcp/server.rs` owns protocol handling, registry data, filtering, validation, and response wrapping. `src/mcp/tools.rs` contains shared runtime helpers, validators, constants, concurrency controls, and many tool implementations. This makes local reasoning harder and increases the risk that future tool additions introduce schema drift, inconsistent errors, or unexpected runtime behavior.

The third pressure point is codegg integration shape. MCP text responses are acceptable for broad interoperability, but codegg should not need to parse JSON out of text content for routine deterministic checks. It should be able to call typed Rust APIs directly and reserve MCP exposure for model-facing tool use.

## Design Principles

Single source of truth. A tool should be declared once, including handler, schemas, metadata, profiles, examples, and expected machine codes. MCP listing, dispatch, docs, validation, and tests should derive from that declaration.

MCP is an adapter, not the core. The deterministic tool API should be usable without stdio JSON-RPC. This enables codegg to use eggsact cheaply and reliably inside harness flows.

Machine-readable first. Human text is useful for display, but codegg should route on stable codes, verdicts, severity, confidence, locations, and recommended next actions.

Curated exposure. Most eggsact value should be harness-only. Model-facing tool lists must be small, contextual, and profile-driven. The model should not see the full tool universe in ordinary coding sessions.

Local and deterministic. Avoid external services. Favor bounded, reproducible, side-effect-free helpers that reduce model hallucination and prevent avoidable edit/command/config mistakes.

## Roadmap Overview

### Phase 1: Tool Registry Single Source of Truth

Introduce a `ToolSpec` registry that owns every tool's name, description, handler, input schema, output schema, metadata, profiles, exposure level, cost, stability, examples, and code expectations. Replace manually synchronized handler, metadata, definition, and output schema tables with generated views over this registry.

This is the foundation phase. It should preserve current MCP behavior while eliminating the four-table drift hazard.

### Phase 2: MCP/Runtime/Tool Module Split

Split the MCP implementation into protocol, runtime, registry, schema validation, response, and category-specific tool modules. The goal is not behavior change; the goal is maintainability, testability, and clearer extension boundaries.

`server.rs` should become primarily protocol orchestration. Tool implementations should live in category modules. Shared validation and runtime helpers should no longer be embedded inside implementation-heavy files.

### Phase 3: Stable Response Contracts and Machine Codes

Make stable machine codes first-class. Every non-OK response should include a machine code. Important warnings and findings should have structured `kind`, `severity`, `location`, and `message` fields. Composite tools should return a top-level verdict plus child tool summaries.

This phase makes eggsact reliable as a codegg decision substrate rather than merely a text-emitting helper.

### Phase 4: codegg-Native In-Process API

Add a typed Rust API over the registry so codegg can call deterministic tools directly. MCP remains available, but codegg should be able to use `ToolRegistry`, typed input/output structs, and higher-level preflight APIs without parsing MCP text payloads.

This phase establishes eggsact as an embedded harness dependency.

### Phase 5: Profile and Exposure Hardening

Formalize exposure levels as enums and enforce clear separation between model-visible tools and harness-only tools. Fix documentation drift around exposure semantics. Ensure profiles such as `codegg_core_min`, `codegg_preflight`, `codegg_patch`, `codegg_config`, `codegg_shell`, `codegg_unicode_security`, and `codegg_repo_audit` are generated and test-gated.

This phase reduces model tool overload and prevents harness-only tools from leaking into ordinary model-visible MCP lists.

### Phase 6: First-Class Edit and Patch Preflight

Promote the existing edit, patch, line-range, text replacement, path, and Unicode primitives into an opinionated codegg edit-preflight workflow. The workflow should accept original text, intended edit, expected old text or diff, path, language hint, line-ending policy, and optional fingerprints. It should emit verdicts such as `safe_to_apply`, `stale_context`, `old_text_not_found`, `multiple_matches`, `line_ending_change`, `unicode_risk`, `scope_escape`, and `needs_human_review`.

This should become a harness-only check before codegg applies model-authored edits.

### Phase 7: Command Preflight and Shell Policy

Turn command preflight into a structured command policy engine for codegg. Classify shell commands by risk: filesystem mutation, destructive deletion, package install, network access, credential exposure, privilege escalation, background process, long-running process, shell injection risk, recursive traversal, test/build command, and unknown executable.

The output should support policy verdicts such as `allow`, `allow_with_confirmation`, `deny`, and `needs_review`.

### Phase 8: Repo, Config, and Dependency Inspectors

Expand manifest/config inspection around codegg's planning and review needs. Start with Cargo/Rust, then add `pyproject.toml`, `requirements.txt`, `package.json`, lockfiles, Dockerfiles, GitHub Actions workflows, dotenv, INI, JSON, TOML, and eventually YAML if a dependency is accepted.

The goal is structural extraction and safe review, not full semantic build-system replication.

### Phase 9: Unicode, Identifier, and Prompt Ingress Hardening

Use text security, Unicode policy, identifier inspection, and prompt input inspection as deterministic ingress/egress filters for codegg. Run these checks on user prompts, fetched content, pasted code, file paths, identifiers in diffs, and tool outputs that may enter model context.

Most findings should remain invisible unless actionable. Surface only suspicious or workflow-relevant issues.

### Phase 10: Runtime Concurrency and Cancellation Semantics

Decide whether MCP stdio should remain mostly serial or become truly concurrent. If concurrent MCP is needed, decouple request reading from response writing and support out-of-order JSON-RPC responses by ID. Add cooperative cancellation to heavy tools and consider subprocess isolation for high-risk operations where timeout must terminate underlying work.

This phase should be driven by codegg's actual call pattern.

### Phase 11: Generated Docs and Agent Tool Cards

Generate README tool tables, MCP architecture docs, profile docs, schema docs, and compact agent-facing tool cards from the registry. Tool cards should describe when to use a tool, when not to use it, required fields, common machine codes, and examples.

This prevents docs from drifting and allows codegg to inject small contextual tool descriptions instead of large static tool lists.

### Phase 12: Golden Fixtures, Fuzzing, and Compatibility Gates

Create fixture-based tests per tool and per profile. Each tool should have at least one success fixture, one invalid argument fixture, one limit fixture, and one compact schema/listing fixture. Add property/fuzz tests for path normalization, line/column mapping, Unicode normalization, JSON extraction, patch parsing, and shell splitting.

Add codegg compatibility snapshots for critical profiles and machine codes.

### Phase 13: codegg Integration Pass

Integrate eggsact into codegg in three layers: small model-visible MCP profiles, harness-only preflight calls, and manager/reviewer repo-audit tools. Use eggsact automatically before edits, risky shell commands, config changes, and suspicious text ingress.

The core rule is that eggsact should reduce model burden and prevent avoidable mistakes, not become another broad model-facing tool pile.

### Phase 14: Performance and Operational Polish

Add benchmarks and lightweight telemetry for high-frequency tools: text equality, text measurement, fingerprinting, path scope checks, JSON/TOML validation, text replacement checks, edit preflight, and command preflight. Expose counters for calls, failures by machine code, average latency, timeouts, and profile/tool usage.

The target is cheap, local, deterministic checks that can run frequently without visible codegg latency.

## Recommended Execution Order

The first five phases should be implemented before major feature expansion. Registry consolidation and module splitting reduce change risk. Stable response contracts and typed in-process APIs make codegg integration practical. Profile/exposure hardening prevents model-facing ergonomics from degrading as more tools are added.

After those foundations land, feature work should prioritize edit preflight, command policy, repo/config inspection, and Unicode ingress hardening, because those directly improve codegg's reliability and safety.

## Non-Goals

Do not turn eggsact into a general sandbox. It can classify command risk and preflight inputs, but enforcement belongs to codegg's execution layer or OS sandboxing.

Do not make every tool model-visible. Many tools are more valuable as harness-only deterministic checks.

Do not require external services. The value proposition is local, deterministic, low-latency computation.

Do not block feature work forever on perfect schema generation. Preserve behavior first, centralize declarations, then improve schemas incrementally.

## Overall Completion Criteria

The roadmap is complete when codegg can use eggsact as a deterministic local substrate with these properties:

- Tool declarations are single-source and generated into MCP listings, dispatch, docs, and tests.
- MCP remains compatible with existing clients.
- codegg can call eggsact directly through typed Rust APIs.
- All tool errors and important warnings have stable machine codes.
- Model-visible tool exposure is profile-driven and small by default.
- Harness-only edit, command, config, path, Unicode, and repo checks are available.
- Docs and agent tool cards are generated from the registry.
- Profile and response contracts are protected by compatibility tests.
