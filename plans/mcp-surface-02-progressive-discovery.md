# Low-Context MCP Tool Discovery Surface

Status: planned
Priority: P1
Scope: agent-facing tool presentation, deterministic tool discovery, generic invocation, context reduction, description/alias cleanup, client integration rendering

## Objective

Preserve all existing eggsact capabilities while reducing the number and size of tool definitions an ordinary model must consider at once.

The target architecture is a two-layer MCP surface:

1. the existing capability registry remains authoritative for the 86 deterministic utility tools;
2. an optional discovery presentation exposes a very small stable front door plus MCP-only search/invoke facades that can reach every tool allowed by the active profile and audience.

Do not merge the 86 underlying tools into a handful of giant enum-driven category tools. Do not mutate `tools/list` as a side effect of conversation state. Do not add embeddings, a vector database, network search, or a new runtime dependency for an 86-item static catalog.

This plan follows `mcp-surface-01-protocol-2026-07-28.md` so the presentation layer can use current MCP metadata, cache behavior, and structured output cleanly.

## Research basis — 2026-09-10

Primary sources reviewed for this plan:

- MCP current roadmap: https://blog.modelcontextprotocol.io/posts/mcp-roadmap/
- MCP Tools specification, including the rule that available tools must not vary per connection or as a side effect of another request: https://modelcontextprotocol.io/specification/draft/server/tools
- Anthropic advanced tool use / Tool Search measurements: https://www.anthropic.com/engineering/advanced-tool-use
- MCP server-instructions guidance: https://blog.modelcontextprotocol.io/posts/2025-11-03-using-server-instructions/
- MCP tool annotation guidance: https://blog.modelcontextprotocol.io/posts/2026-03-16-tool-annotations/

Important findings:

- Large always-loaded tool catalogs consume substantial prompt context and increase wrong-tool/wrong-parameter failures.
- Anthropic reports that a 50+ tool setup used roughly 72K tool-definition tokens in one example; deferred search reduced loaded definitions to a few relevant tools and cut tool-definition token use by about 85%, with substantial tool-selection accuracy gains in its MCP evaluations.
- Anthropic recommends deferred discovery when there are roughly 10+ tools or tool definitions exceed roughly 10K tokens. Eggsact exposes far more than that in `full` model mode.
- The MCP maintainers now explicitly identify large catalogs (on the order of 100 tools) as a protocol scaling problem and are exploring standardized progressive discovery.
- Current MCP does not yet provide a server-side standard primitive that lets eggsact dynamically add tool definitions to one conversation. The available-tool set must not change per connection or as a side effect of tool calls.
- Server instructions can improve cross-tool workflow selection, but should be concise and must not substitute for a well-designed tool surface.

## Current eggsact state

The capability layer is already suitable for deterministic discovery:

- `ToolSpec` has canonical name, description, category, tier, profiles, tags, exposure, aliases, cost, stability, and composite metadata;
- `ALL_TOOLS_VEC` is deterministic and complete;
- profile + audience filtering already determines which underlying tools a caller is permitted to use;
- the model-facing `full` profile contains 77 tools while `default` has 25 and `codegg_core_min` has 6;
- `ToolExposure::Model` filtering currently excludes HarnessOnly/Hidden capabilities but intentionally leaves Contextual and ExpertOnly capabilities available;
- `find_close_match` already supplies lightweight Levenshtein-based typo recovery;
- composites such as `edit_preflight`, `command_preflight`, `config_preflight`, and `text_security_inspect` already provide good workflow-level front doors.

The main missing abstraction is a separation between **capability scope** and **presentation scope**. Profiles currently control both what appears in `tools/list` and what `tools/call` can execute. Discovery needs a smaller listing without making the long-tail capabilities unauthorized.

## Part A — Separate capability policy from presentation policy

### A1. Introduce a presentation/surface mode

Add a small MCP presentation enum, for example:

```rust
pub enum McpSurface {
    Direct,
    Discovery,
}
```

`Direct` preserves current behavior: `tools/list` advertises the profile/audience-filtered canonical tool set.

`Discovery` changes only what is advertised to the model. The active profile and audience remain the capability boundary used by search and execution.

Do not reuse `Profile` for this distinction. A profile answers “which capabilities are available?” while a surface answers “which of those capabilities are advertised up front?”. Mixing those concepts will recreate the current coupling.

### A2. Keep current defaults compatible during implementation

Add an explicit startup configuration path such as `EGGSACT_MCP_SURFACE=direct|discovery` and/or a CLI flag such as `--mcp-surface direct|discovery`.

During this plan, preserve the current `eggsact --mcp` behavior unless the evaluation/rollout plan explicitly approves a default change. Existing profiles and environment variables remain valid.

Prefer an `EGGSACT_` prefix for new eggsact-native configuration instead of extending the historical `EGGCALC_` compatibility namespace, but document the naming decision and keep parsing centralized.

### A3. Presentation must never become authorization

In discovery mode:

- a canonical tool omitted from `tools/list` is still an allowed underlying capability if the active profile/audience permits it;
- `tool_search` may only return underlying tools permitted by the active profile/audience;
- `tool_invoke` may only execute underlying tools permitted by the same active profile/audience;
- HarnessOnly/Hidden tools must not become reachable to Model audience through the router;
- a narrower profile such as `codegg_patch` must constrain discovery/invocation to that profile even when the server surface is discovery.

Add explicit negative tests for profile and audience bypass attempts.

## Part B — Add a small stable discovery presentation

### B1. Start with a fixed, reviewable pinned set

Do not add a `presentation` field to all 86 `ToolSpec` entries unless evidence shows it is needed. Keep the initial always-visible list as one small explicit policy constant owned by the MCP presentation layer.

Start evaluation with these canonical front doors:

- `math_eval`;
- `edit_preflight`;
- `command_preflight`;
- `config_preflight`;
- `text_security_inspect`.

Add the two discovery facades described below. This gives an initial target of seven advertised definitions in discovery mode.

The pinned set is provisional and must be validated by plan 03. It should remain small enough to review as policy rather than emerge indirectly from tier/exposure arithmetic.

### B2. Do not force every pinned tool into every capability profile

A pinned tool is only advertised when the active profile/audience allows that underlying tool. The discovery facades themselves remain available when discovery mode is enabled.

This keeps `codegg_patch`, `human_math`, and other scoped profiles meaningful.

### B3. Keep discovery `tools/list` deterministic and compact

The discovery list should use a model-oriented serialization distinct from the existing `compact` compatibility mode if necessary.

For each pinned canonical tool, include only fields that materially help selection/invocation:

- name/title;
- concise discriminating description;
- input schema;
- standard annotations.

Do not emit output schemas, internal tier/tags/cost, compatibility metadata, or verbose implementation descriptions unless the current protocol requires them.

The search facade can expose long-tail metadata on demand.

## Part C — Implement `tool_search`

### C1. Make search deterministic and local

Implement the search index directly over the active `ToolSpec` slice after profile/audience filtering. For 86 tools, do not add embeddings, SQLite, a vector index, or network access.

Use a deterministic weighted lexical scorer. Recommended precedence:

1. exact canonical-name match;
2. exact alias match;
3. canonical-name token/prefix match (split snake_case and punctuation);
4. tag/category token overlap;
5. description token overlap;
6. bounded Levenshtein/close-match recovery for typo-like queries.

Tie-break deterministically by relevance class, then current registry order.

Do not expose unstable floating-point “confidence” as if it were calibrated probability. If the implementation uses numeric scores internally, return only stable ranking and a short match reason if useful.

### C2. Keep the input contract small

A suitable initial schema is:

```json
{
  "query": "string, required",
  "limit": "integer 1..10, default 5",
  "detail": "summary | schema, default summary",
  "include_deprecated": "boolean, default false"
}
```

Avoid category/tier/tag filter knobs in the model-facing search tool until evaluation demonstrates a need. Those filters already exist internally and adding more choices increases prompt burden.

### C3. Return compact actionable matches

Summary results should contain only high-signal fields, for example:

```json
{
  "matches": [
    {
      "name": "patch_summary",
      "purpose": "Summarize files, hunks, additions, deletions, renames, and changed ranges in a unified diff.",
      "category": "patch",
      "required_arguments": ["patch"]
    }
  ]
}
```

When `detail="schema"`, include the selected tool's full input schema (bounded by `limit`). Do not include every output schema or internal metadata by default.

If useful, return a minimal invocation skeleton that can be copied into `tool_invoke`, but avoid duplicating large user payloads in the search result.

### C4. Handle deprecated tools without losing compatibility

Deprecated tools such as `json_query` should be absent from ordinary search results unless:

- the query exactly names the deprecated tool; or
- `include_deprecated=true`.

Direct invocation by canonical deprecated name remains possible if the capability profile allows it. Search should recommend the documented replacement where available.

### C5. Activate existing aliases through search

Index `ToolSpec.aliases`; today the field exists but contributes little to discovery because most entries are empty and direct lookup is canonical-name based.

Populate aliases only when they represent meaningful user/model vocabulary or an observed evaluation failure. Do not create large synonym lists speculatively.

## Part D — Implement `tool_invoke` as an MCP facade, not a nested utility handler

### D1. Keep the generic invocation contract minimal

A suitable input schema is:

```json
{
  "name": "string, required",
  "arguments": "object, optional/default {}"
}
```

`tool_invoke` must not require the model to wrap the target response in another arbitrary domain schema.

### D2. Reuse the existing target dispatch path

Do not implement `tool_invoke` by calling a sibling `tools::*` handler from inside another handler. That would violate the typed-composition layering and could create nested worker/budget behavior.

Instead, handle the discovery facade in the MCP orchestration layer:

1. parse target name/arguments;
2. resolve the active capability profile and audience;
3. call the same `ToolRegistry::prepare_tool_call`/schema-validation path used by direct `tools/call`;
4. resolve the **target tool's** cost/budget;
5. execute the target through the existing bounded `execution::execute_tool_bounded` path;
6. serialize the target result using the same era-specific response adapter as a direct call.

The target tool name should remain visible in the resulting `ToolResponse`/metadata so diagnostics and machine-code routing are not obscured.

### D3. Keep facades outside the underlying 86-tool utility registry if necessary

`tool_search` and `tool_invoke` are MCP presentation/orchestration facades, not deterministic domain utilities intended for the Rust library API.

Prefer a small `src/mcp/discovery.rs` (or similarly named module) that owns:

- facade Tool definitions/schemas;
- pinned presentation policy;
- deterministic search over `ToolSpec`;
- parsing helpers for generic invocation.

Do not force these two facades into `src/tools/` and `ALL_TOOLS_VEC` if doing so introduces registry→handler→registry recursion or falsely advertises them as in-process utility capabilities.

Document the distinction clearly: eggsact still has 86 underlying deterministic tools; discovery mode additionally advertises two MCP-only facades.

If implementation finds a clean registry abstraction that can represent MCP-only facades without layering violations, use it, but preserve a single authoritative registry for the 86 utility capabilities.

### D4. Block recursive/meta invocation

`tool_invoke` must reject attempts to invoke `tool_invoke` or `tool_search` through itself. This is not useful and complicates budgeting/error reporting.

Return a stable invalid-arguments error rather than recurse.

## Part E — Description and metadata cleanup for agent selection

### E1. Rewrite descriptions around discriminating semantics

Audit model-visible tool descriptions, especially overlap clusters:

- edit/patch/replacement;
- shell/argv/preflight;
- config validation/inspection;
- text/unicode/security;
- semantic comparison tools;
- repo manifest/language/tree classification;
- `text_hash` vs `text_fingerprint`;
- JSON extraction/deprecated query.

A good description should answer:

1. what result this tool produces;
2. when to choose it instead of the nearest competing tool;
3. one critical limitation if selection depends on it.

Remove internal call-graph prose such as “calls X, Y, Z” unless that implementation fact changes how the agent should select or interpret the tool.

### E2. Keep cross-tool workflow guidance in server instructions

Use concise server instructions for relationships that should not be repeated on every tool, e.g. “Prefer preflight composites for proposed edits/commands/configs; use tool_search for long-tail deterministic utilities in discovery mode.”

Do not exceed a small fixed instruction-size budget and do not rely on instructions for authorization or safety.

### E3. Do not expose internal ranking metadata in ordinary MCP Tool definitions

`tier`, `tags`, `cost`, exposure, stability, and search-specific metadata are primarily server-side discovery inputs. Keep them in `ToolSpec` and namespaced `_meta` only where a client has a real reason to consume them.

The model should not pay prompt tokens for server implementation bookkeeping.

## Part F — CLI and integration wiring

### F1. Add explicit surface selection to the CLI

Extend the current exact-match CLI parser to support MCP options without turning it into a large argument framework. A concrete target is:

```text
eggsact --mcp
eggsact --mcp --mcp-surface discovery
eggsact --mcp --mcp-surface direct
```

If profile/schema/audience also need CLI overrides for clean integration rendering, add only the minimal validated flags needed and map them onto the same startup configuration functions used by environment variables.

Do not mutate process environment after threads may exist; pass validated startup configuration explicitly where practical.

### F2. Teach `integrate` renderers about discovery mode

Add a renderer option or internal rendering path that can emit discovery-mode startup arguments for Zed, Codex, Claude Code, Cursor, VS Code, and OpenCode.

Do **not** switch the default rendered configuration until plan 03's evaluation/rollout criteria pass. During implementation, make the discovery rendering testable explicitly.

### F3. Preserve direct/full escape hatch

Document the canonical direct configuration for clients/harnesses that already implement deferred tool loading themselves or require first-class canonical tool definitions.

No capability should require the discovery facade when direct mode is selected.

## Part G — Context-budget and security regression tests

Add tests that serialize actual `tools/list` responses and measure bytes rather than relying on README tool counts.

At minimum assert:

- discovery mode advertises no more than the expected pinned+facade count;
- the discovery listing is substantially smaller than `full` Model direct listing;
- all search results are within active profile/audience;
- every model-safe stable underlying tool is invokable through the facade by canonical name;
- HarnessOnly/Hidden targets are rejected for Model audience;
- deprecated search behavior follows policy;
- recursive facade invocation is rejected;
- repeated search queries return byte-stable ordered results;
- search result size is bounded for maximum `limit` + `detail="schema"`.

Use relative size targets initially; plan 03 will set final rollout thresholds from measured baselines.

## Part H — Documentation

Update:

- `architecture/registry-profiles.md` — capability profile vs MCP surface distinction;
- `architecture/mcp-server.md` — direct vs discovery list/call flow;
- `architecture/coding-agent-integration.md` — when to choose discovery vs direct;
- `docs/mcp-tools.md` — discovery facade contracts and direct-capability compatibility;
- `docs/cli.md` / `docs/installation.md` — explicit surface startup examples;
- `.opencode/skills/mcp-tools/SKILL.md` — agent-facing guidance for the new surface;
- `AGENTS.md` — adding capabilities must include discovery/eval considerations.

Generated tool-card/profile assets should continue to be produced from `ToolSpec`. Do not treat the two MCP-only discovery facades as ordinary utility categories unless the generator is intentionally taught the distinction.

## Implementation order

1. Add `McpSurface` startup configuration with `Direct` preserving all current behavior.
2. Add discovery presentation policy and deterministic compact list serialization.
3. Implement deterministic `tool_search` over profile/audience-filtered `ToolSpec` metadata.
4. Implement MCP-layer `tool_invoke` that reuses target validation/budget/execution.
5. Add security/profile/deprecated/recursion regression tests.
6. Audit overlapping descriptions and activate aliases only where useful.
7. Add CLI/integration rendering for explicit discovery mode.
8. Add context-size measurements and documentation.
9. Hand the measured surface to plan 03 before changing any user-facing default.

## Verification

Run the full verification sequence from `AGENTS.md` plus focused checks equivalent to:

```text
direct/full Model listing unchanged PASS
discovery listing <= provisional 7 tools for full Model PASS
all stable full/Model canonical tools searchable PASS
all stable full/Model canonical tools invokable through router PASS
HarnessOnly target through router rejected PASS
narrow profile cannot escape through router PASS
deprecated tools hidden from ordinary search PASS
search order deterministic PASS
discovery tools/list bytes materially below direct/full PASS
```

## Completion criteria

This plan is complete when eggsact can run in an explicit discovery surface that advertises a single-digit set of stable front-door definitions, every capability allowed by the active profile/audience remains reachable through deterministic local search/invoke routing, no model can use the router to bypass audience/profile rules, direct mode remains compatible, search adds no heavyweight dependency or network state, and the resulting serialized catalog/context measurements are ready for the rollout evaluation plan.