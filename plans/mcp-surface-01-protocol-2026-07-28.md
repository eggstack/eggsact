# MCP 2026-07-28 Dual-Era Modernization

Status: planned
Priority: P1
Scope: MCP protocol lifecycle, wire compatibility, cacheable tool catalogs, structured output, standard metadata, conformance coverage

## Objective

Add first-class support for the released MCP `2026-07-28` protocol revision while preserving eggsact's existing `2025-11-25` and `2024-11-05` stdio behavior for legacy clients.

The implementation must remain local, stdio-only, bounded, deterministic, and lightweight. This is a protocol-adapter modernization, not a rewrite around an MCP SDK and not a reason to add HTTP transport, Tasks, sampling, elicitation, resources, prompts, or other capabilities eggsact does not need.

This plan should land before the discovery-surface rollout plan so the new agent-facing surface is built on the current MCP lifecycle and response contracts rather than immediately needing a second wire migration.

## Research basis — 2026-09-10

Primary sources reviewed for this plan:

- MCP 2026-07-28 release: https://blog.modelcontextprotocol.io/posts/2026-07-28/
- MCP 2026-07-28 release candidate / full JSON Schema notes: https://blog.modelcontextprotocol.io/posts/2026-07-28-release-candidate/
- Current MCP roadmap: https://blog.modelcontextprotocol.io/posts/mcp-roadmap/
- SEP-2575 stateless MCP: https://modelcontextprotocol.io/seps/2575-stateless-mcp
- Current official TypeScript SDK migration notes: https://ts.sdk.modelcontextprotocol.io/v2/migration/support-2026-07-28
- Current official TypeScript SDK protocol-era matrix: https://ts.sdk.modelcontextprotocol.io/v2/protocol-versions

Relevant released changes:

- `2026-07-28` removes the protocol-level `initialize` / `notifications/initialized` handshake and protocol sessions.
- Requests are self-describing through reserved request `_meta`, including protocol version and client capabilities; client identity is advisory rather than an authorization primitive.
- `server/discover` is the modern capability/version discovery RPC and should be supported even though modern requests can be made without it.
- Modern list/read results are cacheable through `ttlMs` and `cacheScope`, and deterministic ordering is explicitly important for caching and prompt-cache stability.
- Server identity is attached to modern responses through reserved result `_meta` rather than depending on legacy `InitializeResult.serverInfo`.
- Tool input/output schemas are JSON Schema 2020-12. Input roots remain objects; output schemas and `structuredContent` may be any conforming JSON value.
- Roots, sampling, and protocol logging are deprecated. Eggsact does not need replacements because it does not use those features.
- Multi Round-Trip Requests replace server-initiated request patterns, but eggsact currently has no workflow requiring them.

## Current eggsact state

`src/mcp/runtime.rs` currently treats protocol lifecycle as connection state:

- supported versions: `2025-11-25`, `2024-11-05`;
- `SessionState::{Uninitialized, AwaitingInitialized, Ready}`;
- `initialize` followed by `notifications/initialized` is required before normal methods;
- client capabilities are retained in `NegotiatedProtocol` but not used;
- `ping` is always allowed.

`src/mcp/server.rs` owns the JSONL stdio loop and request dispatch. `tools/list` and `tools/call` are already separated from registry lookup/validation and bounded execution, so the modern lifecycle should reuse those paths rather than fork a second tool server.

`src/mcp/registry/types.rs` currently serializes eggsact-specific discovery metadata (`tier`, `tags`, `deprecated`, `category`, `llm_exposure`, `cost`) as custom top-level Tool fields.

`src/mcp/response.rs` currently serializes the full `ToolResponse` envelope into `content[0].text`. Tool `outputSchema` values describe the inner success `result`, but no `structuredContent` is emitted.

## Compatibility constraints

1. Existing `initialize` clients must continue to work byte/semantics-compatibly unless a change is required by a documented bug fix.
2. Do not remove `2025-11-25` or `2024-11-05` in this line.
3. Do not change MCP tool names, tool input contracts, machine codes, route-critical verdicts, or existing profile membership.
4. Do not switch `Profile::default()` or the existing `EGGCALC_MCP_PROFILE` default as part of protocol work.
5. Keep `eggsact --mcp` stdio-only. HTTP header routing requirements are irrelevant to the shipped transport and should not be implemented speculatively.
6. Preserve the existing bounded JSONL reader, request-size limits, concurrent execution, ID correlation, timeout budgets, and cooperative cancellation architecture.
7. Prefer era-specific serialization over globally changing legacy response shapes.

## Part A — Introduce an explicit protocol-era model

### A1. Separate legacy connection state from modern request metadata

Refactor `src/mcp/runtime.rs` so lifecycle state does not imply that every request uses the legacy handshake era.

Introduce a small protocol-era representation such as:

```rust
pub enum ProtocolEra {
    Legacy,
    Modern20260728,
}
```

Keep the existing `SessionState` only for legacy-handshake traffic. Modern requests must not transition or consult `SessionState` as an authorization gate.

A modern request is identified from the released reserved request metadata contract, not from hidden process state. Parse and validate the `io.modelcontextprotocol/protocolVersion` request `_meta` field and retain client capabilities/client info for the duration of that request.

Do not infer security policy from self-reported client name/version.

### A2. Extend the supported-version table

Add `2026-07-28` as the preferred current revision while preserving the two legacy revisions.

Version negotiation must be era-aware:

- legacy `initialize` negotiates only among initialize-capable versions;
- `server/discover` advertises all supported revisions in deterministic preference order;
- a modern request explicitly identifying an unsupported modern revision returns the standard unsupported-version behavior rather than silently entering legacy state.

Do not make a `2026-07-28` client perform the old initialization handshake.

### A3. Keep request context request-scoped

Represent modern protocol version, client capabilities, and optional client info in a request-scoped value passed through server dispatch. Do not add new process-global mutable state or reuse `ACTIVE_PROFILE`/`ACTIVE_AUDIENCE` as a place to store per-request protocol data.

The current global profile/audience values remain startup configuration until a separate configuration redesign is justified.

## Part B — Implement `server/discover`

### B1. Add typed protocol structures

Add current `DiscoverRequest` / `DiscoverResult` wire structures in `src/mcp/protocol.rs` based on the final `2026-07-28` schema, not the release-candidate shape.

At minimum the response must expose:

- supported protocol revisions;
- server capabilities;
- concise server instructions when configured;
- required cache hints for the modern result;
- modern server identity in the final reserved response `_meta` location.

Use the final released schema and official SDK fixtures as the source of truth if blog prose and older SEP examples differ.

### B2. Keep instructions concise and functional

Add a short server instruction string suitable for both legacy initialize responses and modern discovery. It should explain only cross-tool behavior that individual descriptions do not convey, for example that eggsact is local/deterministic and that preflight tools inspect rather than execute changes.

Do not turn server instructions into a manual or duplicate the 86 tool descriptions. Target a few hundred bytes, not paragraphs.

### B3. Test discover without prior state

A fresh stdio process must accept `server/discover` before `initialize`, return a modern result, and remain able to serve subsequent modern requests without `notifications/initialized`.

A legacy process path must still accept `initialize` as before.

## Part C — Modern request dispatch

### C1. Make normal methods era-aware

Refactor `handle_request_async` so lifecycle enforcement occurs only for legacy requests.

Modern `tools/list` and `tools/call` requests with valid modern request metadata should enter the existing registry/dispatch logic directly.

Do not duplicate tool validation, audience checks, profile checks, budget selection, or execution code between eras. Era branching should happen at the protocol envelope/serialization boundary.

### C2. Preserve concurrent request behavior

Modern statelessness must not reduce the current concurrency guarantees. Continue to correlate responses by JSON-RPC ID and retain `MAX_IN_FLIGHT_REQUESTS`, `MAX_TOOL_WORKERS`, bounded input/output, and generation-aware cleanup.

Review modern cancellation semantics for stdio against the final spec/conformance fixtures before changing `notifications/cancelled`. Preserve the internal `Arc<AtomicBool>` cancellation signal regardless of wire differences. Do not copy Streamable HTTP cancellation behavior into stdio.

### C3. Reject era-incompatible methods deliberately

Add tests for methods that are legacy-only or modern-only. In particular, do not accidentally accept `initialize` as part of the modern lifecycle or require `ping` if the current modern schema does not define it.

Use protocol-version-specific tests rather than a single permissive dispatcher that happens to accept every historical method.

## Part D — Cacheable deterministic tool catalogs

### D1. Add modern cache hints

For `2026-07-28`, include `ttlMs` and `cacheScope` on `tools/list` as required by the released protocol.

Eggsact's catalog is local, deterministic, process-configured, and authorization-free, so a public cache scope is reasonable provided the response contains no user-specific material. Choose a conservative explicit TTL (for example one hour) and define it as a named constant so it can be tuned without touching serialization logic.

Legacy responses must remain unchanged.

### D2. Guarantee deterministic order

The current registry order is deterministic and compatibility-sensitive. Add a regression test that serializing the same modern `tools/list` request twice produces the same ordered tool names and stable response bytes for the relevant result payload.

Do not sort tools alphabetically if that would change established profile order. Preserve registry order unless a later evaluation plan justifies a new presentation surface with its own deterministic ordering.

### D3. Keep `listChanged` truthful

The existing static server advertises `listChanged: false`. Keep that value for the direct catalog. Do not introduce conversation-dependent tool-list mutation; the current MCP tools specification explicitly forbids a tool set that varies per connection or as a side effect of other requests on that connection.

The progressive-discovery plan must therefore use a stable presentation surface plus search/invoke routing, not mutate `tools/list` after a search call.

## Part E — Correct modern Tool and CallToolResult shapes

### E1. Add standard tool annotations

Extend the protocol Tool serialization model to support MCP `annotations`.

Eggsact tools are local deterministic computations/classifiers and do not modify the external environment or access an open world. For tools where that statement is exact, advertise at least:

- `readOnlyHint: true`;
- `openWorldHint: false`.

Do not set annotations mechanically if any tool violates the claim. Add a registry invariant test over all model-visible tools.

Remember that annotations are hints, not enforcement; existing audience/profile/budget rules remain the deterministic controls.

### E2. Move eggsact-only Tool metadata under `_meta` for modern responses

For the modern era, stop relying on nonstandard top-level Tool keys for `tier`, `tags`, `category`, `llm_exposure`, `cost`, and similar registry metadata.

Serialize protocol-standard Tool fields at the top level and place any metadata that must cross the wire under one stable namespaced `_meta` object, e.g. a documented `io.github.eggstack/eggsact` namespace.

Keep legacy serialization compatible in this line. Do not remove fields legacy consumers may already inspect until compatibility policy permits it.

### E3. Emit `structuredContent` correctly

For successful modern tool calls with an existing `outputSchema`, emit `structuredContent` that actually conforms to that schema.

The existing schemas describe `ToolResponse.result`, not the outer envelope, so the least disruptive contract is:

- `structuredContent = tool_response.result` on successful responses;
- retain serialized JSON text content as the backward-compatible fallback;
- retain existing `isError` behavior for tool-level errors.

Do not place the full `ToolResponse` envelope into `structuredContent` unless the output schemas are also intentionally wrapped. Avoid rewriting 86 output schemas merely to duplicate the existing text envelope.

Route-critical `verdict` is already stored inside `result`; preserve that. Existing outer fields such as `machine_code`, findings, warnings, limits, and recommended-next-tool remain available in the text envelope and in-process API. If modern clients need them structurally, add them only under a namespaced result `_meta` contract in a separately tested additive change rather than silently changing every tool's output schema.

### E4. Do not add a general JSON Schema engine unnecessarily

The modern spec permits full JSON Schema 2020-12, but eggsact controls its own tool schemas and currently emits a valid restricted subset with object input roots. Protocol compliance does not require accepting arbitrary third-party schemas.

Keep the existing bounded validator for eggsact-authored schemas unless this line introduces schema constructs it cannot validate. If later facade schemas require `oneOf`, `$defs`, or other composition, evaluate the smallest bounded implementation then.

Add a test that all emitted schemas are valid under the supported server-authored subset and satisfy the modern input-root constraint.

## Part F — Protocol tests and conformance fixtures

### F1. Add a dedicated modern protocol test module

Create focused tests under `tests/mcp/` for:

- `server/discover` before any handshake;
- direct modern `tools/list` without initialize;
- direct modern `tools/call` without initialize;
- required request `_meta` parsing and malformed reserved metadata;
- unsupported protocol version behavior;
- modern response server identity metadata;
- modern `tools/list` cache hints;
- deterministic list order;
- `structuredContent` conformance for representative simple and route-critical tools;
- legacy initialize lifecycle still working in the same binary;
- no modern request mutates legacy `SessionState`.

### F2. Add cross-era golden scenarios

For representative calls (`math_eval`, `validate_json`, `edit_preflight`), run equivalent legacy and modern sessions and assert that the semantic tool result, verdict, machine code, and errors remain equivalent even though the wire envelope differs.

### F3. Check against official SDK behavior

Before closure, smoke the built binary with at least one current official MCP SDK that supports automatic modern/legacy negotiation over stdio. This may be an external/manual release check if adding an SDK dependency to the Rust repo would be disproportionate.

Record the SDK/version and observed negotiation path in the implementation commit/roadmap closure evidence. Do not add a permanent Node/Python dependency solely for this smoke unless it proves useful in CI.

## Part G — Documentation and generated assets

Update:

- `architecture/mcp-server.md` — dual-era lifecycle and modern request flow;
- `architecture/compatibility.md` — era-specific wire compatibility;
- `docs/mcp-tools.md` — supported revisions and structured output behavior;
- `docs/compatibility-policy.md` — clarify that protocol-era-specific standard serialization can differ while tool semantic contracts remain stable;
- `AGENTS.md` — remove the statement that every MCP session requires initialize;
- `README.md` only where the supported protocol summary is user-relevant.

Update the doc generator if Tool serialization metadata/annotations are generated. Do not hand-edit generated profile/tool-card blocks.

## Implementation order

1. Add final 2026-07-28 protocol constants/types and protocol-era model.
2. Implement request `_meta` parsing and request-scoped modern context.
3. Add `server/discover` and modern response identity/cache metadata.
4. Make `tools/list` and `tools/call` reuse the existing dispatch path without legacy lifecycle gating.
5. Add modern cache hints and deterministic-list tests.
6. Add era-specific Tool serialization and standard annotations.
7. Add correct modern `structuredContent` while retaining text fallback.
8. Add cross-era and official-SDK smoke coverage.
9. Update documentation/generated assets and run the full verification sequence.

## Verification

Run the full verification sequence from `AGENTS.md`.

Additionally require:

```text
legacy initialize -> initialized -> tools/list PASS
legacy initialize -> initialized -> tools/call PASS
modern server/discover before initialize PASS
modern tools/list without initialize PASS
modern tools/call without initialize PASS
modern list cache hints present PASS
modern structuredContent validates against outputSchema PASS
legacy semantic parity for representative tools PASS
```

If an official MCP conformance runner is practical without introducing runtime dependencies, add it as scheduled/manual CI. Otherwise retain fixture-based Rust tests plus a documented official-SDK smoke.

## Completion criteria

This plan is complete when the same shipped stdio binary serves both MCP eras correctly, `2026-07-28` requests require no legacy handshake, `server/discover` works from a fresh process, modern tool catalogs are deterministic/cacheable, modern tools use standard metadata/annotations, successful modern calls expose schema-conforming `structuredContent`, legacy clients retain their existing lifecycle and semantics, and no unrelated transport or agent feature has been added.