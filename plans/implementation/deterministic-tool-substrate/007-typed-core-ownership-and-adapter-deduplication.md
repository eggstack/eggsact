# Deterministic Tool Substrate Milestone 007 — Typed-Core Ownership and Adapter Deduplication

Status: ready

Repository baseline: 38aa6da3e8745ed4599362e2ecb0837bc20fca90

Source roadmap:

- plans/subsystems/deterministic-tool-substrate-roadmap.md#7

Historical dependencies:

- Typed-first composition guard — closed/continuous
- Shared analysis anti-drift — closed
- Milestones 003–006 Unicode correctness/conformance line — closed historical evidence

Long-term requirements:

- plans/000-long-term-specification.md#1
- plans/000-long-term-specification.md#2
- plans/000-long-term-specification.md#4.2
- plans/000-long-term-specification.md#5
- plans/000-long-term-specification.md#7
- plans/001-terminology-and-domain-model.md#2
- plans/001-terminology-and-domain-model.md#4
- plans/001-terminology-and-domain-model.md#9
- plans/002-long-term-roadmap.md#phase-0
- plans/003-planning-process.md#6

Applicable ADRs:

- plans/adrs/ADR-0001-planning-conventions-adoption.md

Primary class: invariant

## 1. Objective

Restore the repository's documented typed-first ownership invariant at the remaining adapter boundaries without changing any public API, MCP capability, ToolSpec, schema, profile, audience, machine code, or observable tool contract.

This milestone has two concrete closure targets:

1. make src/text/inspect_prompt.rs the single implementation of prompt-input inspection by removing the parallel prompt-inspection algorithm from src/tools/text.rs and turning prompt_input_inspect_tool into an input-validation/wire-projection adapter; and
2. eliminate all four remaining same-module handler-to-handler production calls documented in architecture/tools.md by routing those compositions through existing typed cores and private typed-result projection helpers.

At closure, production code under src/tools must contain no handler-to-handler composition. A tool adapter may share private serializers/projection helpers with a sibling adapter, but it must obtain semantic facts from text/calc/services rather than from another ToolResponse-producing handler.

This is a maintenance/correctness milestone, not a capability expansion.

## 2. Why this milestone is ready

The September 27, 2026 repository audit found a bounded implementation mismatch between the documented architecture and current production code. The required typed cores already exist:

- src/text/inspect_prompt.rs exposes prompt_input_inspect();
- src/text/validate.rs exposes json_compare() and json_shape();
- src/text/toml.rs exposes toml_shape();
- src/text/shell.rs exposes shell_split().

No dependency selection, protocol design, profile/audience decision, or ToolSpec ownership change is required. The work can therefore proceed under the existing typed-first architecture without a new ADR.

A separate future effort to move the catalog/runtime ownership boundary out of mcp would change the ToolSpec/catalog architecture and is explicitly not part of 007; that work requires its own ADR before implementation planning.

## 3. Current implementation evidence

At baseline 38aa6da3e8745ed4599362e2ecb0837bc20fca90:

### 3.1 Duplicate prompt-input inspection implementation

src/text/inspect_prompt.rs is the typed prompt-inspection core and has dedicated tests in tests/text/test_inspect_prompt.rs.

src/tools/text.rs nevertheless contains another prompt-inspection implementation. The adapter file defines a parallel _pi_* helper family for hidden Unicode, bidi controls, HTML comments, Markdown links, ANSI/terminal controls, base64-like blobs, long minified lines, instruction phrases, risk scoring, summary generation, and next-tool recommendations.

Fifteen semantic helper names are duplicated between the adapter and typed core:

- _pi_char_span
- _pi_hidden_char_display
- _pi_hidden_char_category
- _pi_find_unicode_hidden
- _pi_find_bidi
- _pi_find_html_comments
- _pi_find_markdown_links
- _pi_find_ansi_escapes
- _pi_find_terminal_controls
- _pi_find_base64_like_blobs
- _pi_find_long_minified_lines
- _pi_find_instruction_phrases
- _pi_compute_risk_score
- _pi_build_summary
- _pi_recommend_next_tool

prompt_input_inspect_tool() currently validates the MCP arguments and then executes this adapter-local implementation instead of calling text::inspect_prompt::prompt_input_inspect().

This creates two independently mutable security-analysis paths even though architecture/tools.md identifies prompt_input_inspect as a typed deterministic core.

### 3.2 Remaining handler-to-handler composition

architecture/tools.md currently documents four same-module handler calls:

- structured_data_compare -> json_compare
- structured_data_compare -> json_shape_tool
- config_preflight -> toml_shape_tool
- command_preflight -> shell_split

The surrounding architectural prose and canonical specification still refer in places to three deliberate reuses, so the documentation is internally inconsistent as well as short of the target typed-first boundary.

All four calls have existing typed-core equivalents. They remain historical adapter reuse rather than a missing capability.

### 3.3 Existing qualification coverage

The repository already has substantial behavior coverage that must remain green:

- tests/text/test_inspect_prompt.rs exercises typed prompt inspection;
- tests/mcp/test_tool_coverage.rs exercises prompt_input_inspect, command_preflight, config_preflight, and structured_data_compare through the tool surface;
- tests/mcp/test_composite_tools.rs provides deeper command/config/structured-data composite cases;
- typed preflight wrappers exercise command/config behavior through ToolRegistry;
- generated documentation and registry/profile snapshots are already merge-gated.

The missing evidence is explicit differential/anti-drift coverage proving that the wire adapters and typed cores cannot silently diverge again.

## 4. Invariants that must not regress

- One Rust crate; Cargo.lock remains tracked; MSRV stays 1.89.0.
- Preserve all 86 ToolSpecs across 23 categories and the existing registration order.
- Preserve every public Rust path and signature in 1.x, including raw tools::* handlers.
- Preserve all tool names, descriptions, schemas, output schemas, profiles, audiences, exposure values, costs, stability markers, aliases, tags, and route-critical classification.
- Preserve machine codes, verdicts, findings, warnings, limits_applied, recommended_next_tool, and subresults wire shapes.
- Preserve legacy and 2026-07-28 MCP behavior, direct/discovery presentation, era pinning, and tool_search/tool_invoke behavior.
- Preserve ToolRegistry policy, ExecutionContext semantics, typed preflight behavior, cancellation, deadlines, and output truncation.
- Preserve Python-parity behavior and the accepted C1–C6 parity baseline. No existing accepted failure may be silently rewritten to make this refactor pass.
- Preserve heuristic-only YAML behavior and all existing TOML/JSON/shell semantics.
- Preserve the Unicode 18 / UTS #39 behavior closed in Milestones 003–006.
- No new runtime dependency is expected or justified.
- Deterministic cores/services remain independent of ToolResponse, MCP registry policy, and JSON-schema validation.
- Adapter code may parse/validate wire input and project typed results, but must not call sibling ToolResponse-producing handlers after 007.

## 5. Scope

### In scope

- baseline differential fixtures for each affected adapter/core seam;
- prompt_input_inspect_tool delegation to the canonical typed core;
- deletion of adapter-local prompt semantic helpers that become unreachable;
- structured_data_compare migration from json_compare/json_shape_tool handlers to typed JSON cores;
- config_preflight migration from toml_shape_tool to the typed TOML shape core;
- command_preflight migration from shell_split handler to typed shell_split;
- private projection/serialization helpers where two adapters need the same wire representation of one typed result;
- a source-level/static guard preventing reintroduction of the four known handler-to-handler compositions and the duplicate prompt implementation;
- documentation reconciliation after the code proves zero handler-to-handler composition;
- ordinary closure record and merge-gate evidence.

### Explicit non-goals

- no new tool or capability;
- no ToolSpec, schema, profile, audience, exposure, discovery, or protocol changes;
- no public visibility reduction for tools::*, services::*, or mcp::*;
- no transport-neutral catalog/runtime extraction from mcp; that requires a later ADR;
- no broad rewrite of tools/helpers.rs;
- no config_file_inspect parser consolidation;
- no dependency_edit_preflight parser/model redesign;
- no module-splitting campaign for agent/mod.rs, preflight/mod.rs, mcp/server.rs, or mcp/execution.rs;
- no updater/Eggup cleanup;
- no new YAML parser or other dependency;
- no performance optimization unless required to avoid a demonstrated regression introduced by this work.

## 6. Compatibility, profile, audience, and surface effects

Expected public effect: none.

The direct MCP definitions, discovery definitions, tool search index, profile snapshots, audience filtering, and generated tool cards must be byte/semantically unchanged except for documentation prose that explicitly describes internal composition.

No ToolSpec file or schema file should require a production change. If an implementation agent believes a schema or registry edit is necessary, stop and reassess because that exceeds the planned boundary.

The raw tools::* functions remain public for 1.x compatibility and continue returning ToolResponse. Only their internal source of semantic facts changes.

## 7. Architectural rule for this milestone

The post-007 production flow must be:

~~~text
typed core / typed service
        |
        +--> private typed-result projection helper, when needed
        |
        --> tools/* JSON adapter -> ToolResponse
~~~

A projection helper may accept a typed result and produce serde_json::Value or populate a ToolResponse field. It must not call a sibling handler and must not become a second semantic implementation.

For composite tools, policy/verdict derivation may remain in the owning adapter when that policy is specific to the composite contract. Neutral parsing/classification/comparison belongs in the typed core/service.

## 8. Ordered work packages

### Work package A — Pin baseline behavior and layering guards

Intent:

Make the current external contracts explicit before deleting any duplicate implementation.

Required work:

1. Add focused adapter/core differential fixtures for prompt_input_inspect covering at least:
   - clean text;
   - zero-width/hidden Unicode;
   - bidi controls;
   - ANSI escape;
   - terminal control;
   - instruction phrase;
   - custom checks subset;
   - custom phrase patterns;
   - CRLF long-line line numbering;
   - a case that exercises finding deduplication and recommended_next_tool.
2. Pin structured_data_compare wire behavior for:
   - equal objects;
   - differing values;
   - nested/array data;
   - invalid A;
   - invalid B;
   - max_diffs behavior;
   - current shape subresults;
   - the BUG-006 dead TYPE_MISMATCH compatibility behavior.
3. Pin config_preflight TOML behavior for:
   - valid flat TOML;
   - nested tables/arrays where toml_shape contributes diagnostics/subresults;
   - malformed TOML;
   - warning/verdict/machine-code behavior.
4. Pin command_preflight parse-stage behavior for:
   - safe argv;
   - quotes/escapes;
   - pipe/redirection;
   - unbalanced quotes / parse error;
   - risky-feature detection;
   - auto/posix platform handling.
5. Add a named layering guard test, e.g. adapter_layering_has_no_handler_to_handler_composition, that will become green after B–E. It should inspect source structurally enough to reject the four known direct handler calls and the adapter-local prompt semantic helper family without depending on line numbers.

The guard must not forbid ordinary calls to typed functions with the same conceptual names. It should target module-qualified paths, known handler symbols, or another stable source invariant rather than a broad substring such as json_compare.

Acceptance evidence:

- new behavior fixtures pass on baseline where they describe current external behavior;
- the new layering guard fails on baseline for the intended known composition sites, or is introduced together with the final refactor with a closure note showing the baseline inventory;
- no production code is changed merely to make a poorly scoped guard pass.

### Work package B — Make typed prompt inspection authoritative

Intent:

Delete the highest-risk duplicate semantic implementation.

Required work:

- keep prompt_input_inspect_tool argument validation, bounds checks, unknown-check rejection, custom phrase parsing, ToolResponse construction, and existing machine/error codes at the adapter boundary;
- invoke crate::text::inspect_prompt::prompt_input_inspect() exactly once for the semantic inspection;
- project PromptInspectResult into the existing MCP/tool result shape without changing field names, ordering expectations, finding payloads, risk_score semantics, checks_run, text_length, summary, or recommended_next_tool;
- remove the duplicate _pi_* semantic helpers and any adapter-local regex/statics/imports that become unused;
- do not weaken or bypass the typed core's existing tests;
- keep text_security_inspect on its existing services/security.rs -> typed prompt-core path.

Critical compatibility rule:

Before deleting the old implementation, compare it against the typed core on the Work Package A matrix. Wire-only differences may be handled in the adapter projection. If there is a semantic difference in findings, severity, spans, risk score, or recommendations between the two public surfaces, do not silently pick one. Characterize the discrepancy and either:

1. prove one path is an accidental stale implementation and preserve the documented/public behavior through a bounded correction; or
2. stop and register a corrective plan if reconciling the paths would intentionally change an established public typed or MCP contract.

Acceptance evidence:

- only one prompt-inspection semantic implementation remains;
- typed and MCP tests cover the same representative cases;
- no duplicate _pi_* semantic helper family remains in src/tools/text.rs.

### Work package C — Remove structured_data_compare handler reuse

Intent:

Make structured_data_compare consume typed JSON facts directly.

Required work:

- replace the call to the json_compare ToolResponse handler with crate::text::json_compare() or the canonical re-export;
- replace both json_shape_tool handler calls with crate::text::json_shape();
- preserve structured_data_compare's existing validation order, budget/cancellation checkpoints, findings, machine code, summary, and subresults;
- if json_compare and json_shape adapters need the same serialization as the composite, extract private projection helpers that accept typed results;
- do not move structured_data_compare policy into the neutral typed JSON core;
- preserve BUG-006 parity behavior exactly unless a separate corrective is registered.

Acceptance evidence:

- structured_data_compare contains no direct call to json_compare() or json_shape_tool() handler symbols;
- its existing and new MCP fixtures are unchanged;
- standalone json_compare and json_shape tools remain unchanged.

### Work package D — Remove config_preflight handler reuse

Intent:

Make the TOML stage of config_preflight consume the typed TOML core.

Required work:

- replace config_preflight -> toml_shape_tool with crate::text::toml_shape();
- preserve the current TOML shape wire projection/subresult through a private helper shared with toml_shape_tool if necessary;
- preserve format auto-detection, JSON/schema behavior, dotenv/INI/Cargo behavior, YAML exclusion, cancellation points, findings, verdicts, and machine codes;
- do not change the typed TOML parser or add YAML support.

Acceptance evidence:

- config_preflight no longer invokes toml_shape_tool;
- standalone toml_shape and config_preflight TOML outputs remain contract-compatible;
- YAML behavior is unchanged.

### Work package E — Remove command_preflight handler reuse

Intent:

Make command_preflight's parse stage consume typed shell facts.

Required work:

- replace command_preflight -> shell_split ToolResponse composition with crate::text::shell_split();
- preserve shell selection, detect_risky_features behavior, parse-error handling, subresults["shell_split"] shape, matched rules, command classification, policy_config semantics, cancellation points, machine codes, verdicts, and recommendations;
- share only a private projection helper with the standalone shell_split adapter if needed;
- do not broaden Windows support or alter policy classification.

Acceptance evidence:

- command_preflight contains no call to the shell_split handler;
- existing command policy tests and typed shell tests remain green;
- standalone shell_split behavior is unchanged.

### Work package F — Close the layering invariant and reconcile documentation

Intent:

Turn a historical exception list into an enforceable zero-reuse rule.

Required work:

- make the layering guard from Work Package A green;
- update architecture/tools.md so Deliberate Remaining Handler-to-Handler Reuse is removed/replaced by a statement that there are no production handler-to-handler compositions;
- correct the stale three-vs-four wording in architecture/tools.md;
- after implementation proves the zero-reuse state, make the minimal factual reconciliation in:
  - plans/000-long-term-specification.md typed-first/invariant wording;
  - plans/001-terminology-and-domain-model.md JSON-adapter wording;
  replacing the historical exception count with the zero-reuse rule;
- update architecture/text-library.md or docs/library-api.md only if their prompt-core ownership wording is inaccurate after the change;
- do not hand-edit generated tool cards/profile blocks;
- run generate-docs --check and expect no registry/profile-generated diff.

The canonical long-term edits above are allowed as contradiction/factual reconciliation under plans/003-planning-process.md#2.1; they must not introduce a new architecture.

Acceptance evidence:

- architecture and terminology agree with production code;
- zero same-module or cross-module handler-to-handler production composition remains under src/tools;
- generated registry/tool/profile facts are unchanged.

### Work package G — Qualification and closure

Intent:

Demonstrate that the refactor removed maintenance risk without changing capability.

Required evidence:

- focused tests from Section 11;
- full ordered merge gate;
- parity when ../eggcalc is available;
- cargo-deny/maintenance evidence if dependency metadata changes unexpectedly;
- remote ordinary CI run ID;
- git diff review proving no ToolSpec/schema/profile/audience changes;
- closure record plans/closure/deterministic-tool-substrate/007-status.md;
- registry/roadmap return to guard-only only after closure evidence is accepted.

## 9. Failure, cancellation, truncation, and contention semantics

No failure or resource semantics may change.

- Adapter argument validation must happen at the same logical stage as before.
- Typed-core delegation must not bypass existing MAX_* input bounds.
- Composite BudgetContext polling points must remain at least as frequent as baseline.
- Cancellation must still short-circuit before later expensive stages.
- ToolResponse truncation and limits_applied reporting remain owned by the existing execution/response layers.
- The refactor must not introduce locks, mutable globals, environment reads, clock reads, network access, or filesystem access.
- Shared projection helpers must be pure functions of typed results/options.
- Deterministic ordering of findings, checks_run, map keys, and subresults must remain stable.

If a typed core returns a representation that would require nondeterministic reconstruction of the historical wire shape, stop and fix the typed result/projection boundary rather than using HashMap iteration or handler recursion.

## 10. Compatibility and migration

No consumer migration is expected.

The following must remain valid unchanged:

- existing MCP client configuration;
- direct/discovery surfaces;
- ToolRegistry calls;
- typed preflight wrappers;
- raw public tools::* handler imports;
- typed text::* imports;
- tests and downstream code expecting current machine codes and response fields.

This milestone is internal ownership consolidation only. There is no deprecation or removal.

## 11. Required focused tests

Run focused tests first:

~~~bash
cargo test --locked --test lib prompt_input_inspect
cargo test --locked --test lib structured_data_compare
cargo test --locked --test lib config_preflight
cargo test --locked --test lib command_preflight
cargo test --locked --test lib shell_split
cargo test --locked --test lib toml_shape
cargo test --locked --test lib json_compare
cargo test --locked --test lib test_inspect_prompt
cargo test --locked --test lib adapter_layering
cargo test --locked --test lib tool_registration_tables_are_in_sync
~~~

If filter names differ after inspecting tests/lib.rs module wiring, use the narrowest equivalent filters and record the actual commands in closure.

Also run the typed preflight wrapper tests covering command/config and the route-contract/machine-code tests for affected route-critical tools.

## 12. Required broad verification

Run the repository merge gate in order:

~~~bash
cargo fmt --all -- --check
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
~~~

When ../eggcalc is available:

~~~bash
cargo test --locked --test lib parity -- --nocapture
~~~

Only non-listed parity failures are regressions. Do not alter tests/fixtures/accepted_parity_failures.txt merely to close 007.

If Cargo.toml or Cargo.lock changes despite the no-dependency expectation, also run:

~~~bash
cargo deny check advisories bans licenses sources
cargo check --locked --all-targets --all-features
~~~

and explain why dependency metadata changed. A dependency addition requires explicit review and is presumptively out of scope.

## 13. Static guards

At closure the repository must contain an automated guard for this invariant.

The guard must at minimum detect reintroduction of:

- structured_data_compare calling the json_compare handler;
- structured_data_compare calling json_shape_tool;
- config_preflight calling toml_shape_tool;
- command_preflight calling the shell_split handler;
- adapter-local prompt semantic helper functions duplicating the typed prompt core.

Prefer a small source-architecture test with explicit forbidden edges over a new script/dependency.

The guard must not forbid:

- standalone handler definitions;
- typed calls such as crate::text::json_compare or crate::text::shell_split;
- private projection helpers shared within one adapter module.

## 14. Documentation updates

Required after production changes:

- architecture/tools.md — zero handler-to-handler composition and prompt-core ownership;
- plans/000-long-term-specification.md — minimal factual reconciliation from historical exception count to zero;
- plans/001-terminology-and-domain-model.md — same factual reconciliation;
- CHANGELOG.md — maintenance note if repository convention records internal architectural cleanup;
- closure record for 007.

Update architecture/text-library.md and docs/library-api.md only when needed to keep ownership statements accurate.

No registry-generated docs should change. A generated docs diff in tool names/counts/profiles/schemas is a stop signal.

## 15. Acceptance criteria

Milestone 007 may close only when all of the following are true:

- prompt_input_inspect has one semantic implementation, owned by src/text/inspect_prompt.rs;
- prompt_input_inspect_tool validates/projects results but does not reimplement detectors/risk scoring;
- the duplicate adapter _pi_* semantic helper family is removed;
- structured_data_compare uses typed JSON cores rather than json_compare/json_shape_tool handlers;
- config_preflight uses typed toml_shape rather than toml_shape_tool;
- command_preflight uses typed shell_split rather than the shell_split handler;
- no other production handler-to-handler composition is discovered under src/tools;
- a static/source guard prevents regression of the known edges;
- standalone and composite wire behavior remains compatible on the required fixture matrix;
- all 86 ToolSpecs, 23 categories, schemas, profiles, audiences, exposure values, machine codes, and registration order are unchanged;
- direct/discovery behavior and route-critical contracts are unchanged;
- focused tests and the full merge gate pass;
- parity is green against the accepted baseline when available, or unavailability is recorded;
- remote ordinary CI is green;
- plans/closure/deterministic-tool-substrate/007-status.md records requirement-to-evidence mapping and residual findings.

## 16. Stop conditions

Stop and report rather than expanding scope when:

- the typed prompt core and MCP prompt adapter have a real semantic disagreement that cannot be reconciled without intentionally changing one public contract;
- preserving structured_data_compare/config_preflight/command_preflight wire shapes would require changing ToolSpec schemas or machine codes;
- eliminating a handler call exposes a missing neutral typed fact model larger than a private projection helper;
- the refactor requires moving ToolSpec/catalog ownership out of mcp;
- a new dependency appears necessary;
- parity reveals behavior changes outside the affected composite/tool boundary;
- generated registry/profile/tool-card facts change;
- repository changes after the baseline invalidate the inventory of four handler-to-handler calls.

When a stop condition is hit, preserve completed independent work, document the evidence, and register a bounded corrective or successor plan rather than broadening 007.

## 17. Deferred follow-up inventory

The audit identified additional worthwhile work that is intentionally deferred from 007:

1. Config/repository facts: config_file_inspect in src/tools/repo.rs still owns substantial format detection and JSON/TOML/dotenv/INI/YAML extraction logic. A later milestone may establish a typed ConfigFacts service while preserving heuristic-only YAML behavior.
2. Dependency facts: dependency_edit_preflight intentionally has structured parsers plus malformed-document heuristic fallbacks. A later milestone may normalize both paths into one typed dependency representation with parse provenance.
3. Catalog/runtime layering: agent::ToolRegistry still consumes mcp::registry and ToolHandler returns mcp::response::ToolResponse. Decoupling this is an architectural ownership change and requires an ADR before planning.
4. Large-module decomposition: preflight/mod.rs, agent/mod.rs, mcp/server.rs, mcp/execution.rs, and update.rs may be split privately after semantic ownership is stable.
5. tools/helpers.rs drain-down: continue moving domain logic toward canonical typed cores as those areas are otherwise touched; do not turn 007 into a catch-all helper rewrite.

None of these deferred items is required to close 007.

## 18. Closure evidence required

The 007 closure record must contain:

- implementation commit(s);
- baseline commit;
- before/after inventory of handler-to-handler edges;
- before/after inventory of prompt _pi_* semantic helpers;
- adapter/core differential fixture matrix and results;
- proof that ToolSpec/schema/profile/audience/generated-doc facts did not change;
- focused test commands and outcomes;
- full ordered merge-gate outcome;
- parity outcome or explicit unavailability;
- remote CI run ID;
- documentation reconciliation list;
- dependency/Cargo.lock status;
- residual findings classified by severity;
- recommendation: closed, conditionally closed, corrective pass required, or blocked.

## 19. Handoff notes

Start with Work Package A. Do not delete the duplicate prompt code first.

For prompt inspection, treat src/text/inspect_prompt.rs as the intended architectural owner, but verify public behavior before removing the adapter implementation.

For structured_data_compare, config_preflight, and command_preflight, prefer a private typed-result-to-wire projection helper over copying the sibling handler's implementation. The goal is one semantic computation plus multiple presentations, not replacing handler recursion with duplicated serialization logic.

Do not touch discovery evaluation 03c or distribution M005. Both remain independent planning lines and their blockers/status must be preserved.
