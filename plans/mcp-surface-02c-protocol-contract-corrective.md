# MCP Surface Protocol/Contract Corrective Closure

Status: planned
Priority: P1
Scope: stdio protocol-era pinning, discovery facade output-contract correction, cross-era regression coverage, closure sequencing before agent evaluation

## Objective

Correct two concrete issues found in the post-implementation audit of the MCP `2026-07-28` modernization and progressive-discovery work without reopening the broader architecture.

The current implementation is directionally sound: modern and legacy requests share registry/validation/execution paths, discovery is an opt-in presentation surface, `tool_search` and `tool_invoke` preserve profile/audience policy, and ordinary CI is green. This corrective pass should preserve those properties while tightening two protocol/contract edges:

1. a single stdio process currently determines protocol era independently for each request, which permits modern and legacy traffic to be mixed on one connection even though the official stdio negotiation model pins the connection to one era after its opening exchange;
2. `tool_invoke_output_schema()` cannot truthfully describe the generic router's arbitrary target result and currently declares `additionalProperties: false` around only `tool`/`ok`, despite the router returning the selected target's normal tool response semantics. The schema is currently not advertised, but leaving a contradictory dormant contract is a maintenance trap.

Do not add new tools, dependencies, transports, profile semantics, model-provider integration, or broad schema machinery in this pass. Do not perform plan 03's model/retrieval evaluation here except for regression checks needed to prove these corrections do not change the discovery surface.

## Current implementation facts

At the audited `main` implementation:

- `src/mcp/runtime.rs` has `ProtocolEra::{Legacy, Modern20260728}` and parses modern per-request `_meta` into `ModernRequestContext`;
- `src/mcp/server.rs::handle_request_async` decides modern vs legacy per request: a valid modern `_meta` bypasses legacy `SessionState`, otherwise the request takes the legacy lifecycle path;
- `server/discover` is accepted from a fresh process and does not mutate `SessionState`;
- `McpSurface::Direct` remains the process default and `Discovery` is opt-in;
- `src/mcp/discovery.rs` advertises at most five permitted pinned front doors plus `tool_search` and `tool_invoke` in discovery mode;
- `tool_invoke` delegates target authorization/validation to `ToolRegistry::prepare_tool_call`, derives the target budget, and delegates bounded execution to `execution::execute_tool_bounded`;
- `tool_invoke_output_schema()` exists but discovery listings intentionally omit output schemas for the facades;
- `tests/mcp/test_modern_protocol.rs` and `tests/mcp/test_discovery.rs` already cover modern/legacy semantics, capability containment, discovery count, deterministic exact-name search, and router reachability.

The corrective implementation must work with these structures rather than replacing them.

## Part A — Pin protocol era per stdio connection

### A1. Add explicit connection-era state

Introduce a minimal connection-scoped state distinct from legacy handshake state, for example:

```rust
pub enum ConnectionEra {
    Undecided,
    Legacy,
    Modern20260728,
}
```

The exact type/module placement may differ, but the invariant must be explicit: one stdio server process represents one protocol connection and, once its era is selected, later requests cannot switch eras.

Do not overload `SessionState` for this. `SessionState::{Uninitialized, AwaitingInitialized, Ready}` remains the lifecycle state machine inside a connection already pinned to the legacy era. Modern traffic remains stateless with respect to protocol session/handshake data after the connection has been pinned modern.

The result should conceptually be:

```text
ConnectionEra::Undecided
    ├─ legacy initialize/opening traffic -> ConnectionEra::Legacy
    │      └─ SessionState controls initialize -> initialized -> Ready
    └─ modern discover/opening request -> ConnectionEra::Modern20260728
           └─ each modern request still carries and validates required request _meta
```

Do not store client identity/capabilities globally just because the connection era is pinned. Modern request metadata remains request-scoped and advisory.

### A2. Define opening-exchange rules from official protocol/SDK behavior

Before coding, re-check the final MCP `2026-07-28` specification and current official SDK stdio negotiation behavior, especially the official TypeScript SDK protocol-version/stdio migration guidance.

Implement the smallest rules consistent with that model. The expected shape is:

- `Undecided` accepts the opening discovery probe required by modern stdio negotiation without creating legacy `SessionState`;
- a successful modern opening exchange pins `Modern20260728`;
- an `initialize` opening exchange pins `Legacy` and continues existing legacy negotiation only among initialize-capable revisions;
- once `Legacy`, requests that attempt to use a modern per-request protocol envelope must be rejected rather than bypassing `SessionState`;
- once `Modern20260728`, legacy handshake traffic must be rejected rather than creating/changing `SessionState`;
- modern requests must still validate the `2026-07-28` request `_meta` contract independently; connection pinning is not permission to omit required modern metadata on ordinary modern calls.

Do not invent a new eggsact-specific wire error if the released protocol or official SDK fixtures provide an appropriate standard behavior. Add a helper for the chosen mismatch error so tests and implementation do not scatter string literals.

If `server/discover` without modern `_meta` is the official opening probe form, allow it only where the protocol permits it (normally `Undecided`; possibly modern after pinning if the final schema permits). Do not let an unversioned discover call silently switch an already legacy connection to modern.

### A3. Keep era pinning concurrency-safe

The stdio loop dispatches request tasks concurrently. Connection-era selection must therefore be race-safe for two opening requests arriving close together.

Use the existing async synchronization model (e.g. the same connection-owned `Arc<Mutex<...>>` pattern already used for `SessionState`) rather than new global atomics or process-wide singletons.

Required invariant: exactly one opening era wins. A racing incompatible request must receive a deterministic error; it must not observe a half-transitioned state or mutate the other era's lifecycle.

Do not serialize all tool execution behind the era lock. Hold the lock only long enough to inspect/select the connection era, then continue through the existing concurrent dispatch path.

### A4. Preserve legacy and modern semantics after pinning

Within a legacy-pinned connection:

- existing `initialize` negotiation behavior remains compatible;
- `notifications/initialized` and `SessionState` requirements remain unchanged;
- legacy `ping`, `tools/list`, `tools/call`, `profiles/list`, cancellation, request limits, concurrency, and response shape remain unchanged.

Within a modern-pinned connection:

- no initialize handshake is introduced;
- request `_meta` validation remains per request;
- modern `tools/list`, `tools/call`, `profiles/list`, cancellation, cache hints, annotations, `_meta` server identity, and `structuredContent` remain unchanged;
- modern traffic never transitions or consults legacy `SessionState` as the readiness gate.

The correction is connection negotiation hygiene, not a second protocol rewrite.

## Part B — Add mixed-era and opening-race regression tests

### B1. Extend modern protocol integration coverage

Add focused subprocess/protocol tests proving at least:

1. fresh process -> legacy `initialize` -> connection is legacy-pinned;
2. after legacy pinning, a modern-envelope `tools/list`/`tools/call` cannot bypass legacy lifecycle or switch the connection to modern;
3. fresh process -> modern discover/opening exchange -> connection is modern-pinned;
4. after modern pinning, `initialize` is rejected and cannot create legacy state;
5. fresh process -> direct valid modern request, if permitted by the final stdio rules, pins modern and succeeds without legacy handshake;
6. an unsupported modern version is rejected without pinning the connection to a usable modern state;
7. malformed modern opening metadata is rejected without leaving a poisoned/pseudo-pinned connection;
8. connection-era rejection responses preserve the original JSON-RPC request id.

Use exact official behavior for whether an invalid opening attempt leaves the era `Undecided` or pins/rejects subsequent traffic. Record that choice in architecture docs and tests.

### B2. Add a deterministic opening-race test at the state-machine layer

Do not try to make a flaky subprocess timing test prove the race behavior.

Extract or expose a small testable era-selection helper/state object and add a deterministic concurrent/unit test showing two competing opening attempts cannot both transition `Undecided` to different eras.

The test should prove the state transition, not rely on scheduler timing or sleeps.

### B3. Keep existing cross-era semantic goldens

Do not delete the existing legacy-vs-modern goldens for representative tools. They remain useful: separate connections for each era should still produce equivalent underlying tool semantics even though a single connection can no longer switch eras.

If any current test intentionally mixes eras inside one process, rewrite it into two separate connection cases unless it is specifically testing rejection of era switching.

## Part C — Correct the generic invoke output-schema contract

### C1. Remove the misleading dormant schema unless a truthful generic schema is required

`tool_invoke` routes to arbitrary underlying tools whose `structuredContent` shape is target-specific. There is no single narrow output schema describing all targets without introducing a large union of every tool result, defeating the low-context design.

Preferred correction:

- remove `tool_invoke_output_schema()` if it has no active consumer;
- keep `tool_invoke`'s MCP Tool definition without `outputSchema`;
- document that the target's normal result shape is returned and the caller obtains the target input contract through `tool_search(detail="schema")` or direct/full mode.

Do **not** build an 86-way `oneOf` or generic mega-schema. Do **not** duplicate every target output schema into the discovery facade.

If code structure requires a schema builder to remain, use the least-committal valid schema permitted by the current MCP contract and make its semantics explicit. Do not claim required `tool`/`ok` fields if modern `structuredContent` is actually the target tool's `ToolResponse.result`.

### C2. Keep `tool_search` output schema specific

`tool_search` has a stable result shape (`query` plus bounded ranked `matches`) and can keep a concrete output schema. Correct only the generic invoke contract; do not strip useful schemas indiscriminately.

### C3. Add a schema/serialization regression test

Add a test that fails if a future refactor starts advertising a contradictory `tool_invoke.outputSchema`.

At minimum prove:

- discovery `tools/list` omits `tool_invoke.outputSchema` in both legacy and modern shapes;
- a modern `tool_invoke` targeting two tools with materially different results returns target-appropriate `structuredContent` without claiming a facade-level schema that those values violate;
- `tool_search` may continue to expose/validate its own stable result schema where the current presentation policy chooses to advertise it.

Prefer representative targets already used in protocol goldens (`math_eval` plus a route-critical preflight tool) rather than adding new fixtures solely for this check.

## Part D — Documentation and planning-state correction

### D1. Update dual-era wording from per-request selection to per-connection pinning

Audit and correct wording in:

- `AGENTS.md`;
- `architecture/mcp-server.md`;
- `architecture/compatibility.md`;
- `docs/mcp-tools.md`;
- `docs/compatibility-policy.md` where relevant;
- `.opencode/skills/debugging/SKILL.md` and `.opencode/skills/mcp-tools/SKILL.md` if they teach lifecycle behavior.

Current text saying the same stdio process can select era "per request" should be replaced with the final connection-pinning contract. Preserve the important distinction that modern **request metadata remains per request** even though the connection era is pinned.

### D2. Document generic invoke's output behavior

State explicitly in the coding-agent/MCP documentation that `tool_invoke` is a routing facade with target-specific results and therefore intentionally has no facade-level output schema. This is a feature of the low-context design, not an omission to be "fixed" with a huge union later.

### D3. Keep plan 03 as the remaining rollout gate

Do not duplicate `mcp-surface-03-agent-evaluation-and-rollout.md` in this corrective plan.

After this correction passes verification, plan 03 remains responsible for:

- exact direct-vs-discovery context measurements and the <=25% rollout target;
- complete natural-language intent fixtures;
- overlap-cluster hard negatives;
- top-1/top-3/top-5 deterministic retrieval gates;
- recorded model/client direct-vs-discovery evaluation;
- deciding whether generated `integrate` instructions should recommend discovery.

Until those gates pass, `Direct` remains the default and discovery remains explicit/opt-in.

## Part E — Verification and closure

### E1. Required automated verification

Run the repository verification sequence from `AGENTS.md`:

```text
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features --lib
cargo test --locked --all-features --bins
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo deny check advisories bans licenses sources
cargo package --locked --list
cargo package --locked --verbose
cargo publish --locked --dry-run
```

The ordinary merge CI subset should also be green on the final corrective commit.

Parity failures already documented as accepted/pre-existing remain outside this protocol correction unless this pass creates a new unexpected parity difference.

### E2. Focused acceptance matrix

Record evidence equivalent to:

```text
fresh -> legacy initialize                       PASS
legacy-pinned -> legacy initialized/list/call    PASS
legacy-pinned -> modern envelope switch          REJECTED
fresh -> modern discover/opening                 PASS
modern-pinned -> modern list/call                PASS
modern-pinned -> legacy initialize switch        REJECTED
invalid/unsupported opening -> no state poison   PASS
opening-era race -> single deterministic winner  PASS
modern tool_invoke math target                   PASS
modern tool_invoke preflight target              PASS
tool_invoke facade outputSchema advertised       NO
discovery full/Model advertised count            7 or fewer
direct default unchanged                         PASS
profile/audience router containment              PASS
```

If official stdio negotiation semantics require a slightly different invalid-opening behavior, update the matrix to match the final source-of-truth behavior and document the reason in the implementation commit.

### E3. Official SDK smoke

Because the defect concerns stdio era negotiation rather than only internal serialization, run one current official MCP SDK smoke after the correction if practical without adding a permanent runtime dependency.

Prefer the official TypeScript SDK's current stdio auto-negotiation path because it exercises the disposable discover probe / connection-pinning behavior that motivated this corrective pass. Record SDK version/date and observed selected era in the implementation commit or roadmap closure evidence.

Do not add Node/npm to ordinary Rust CI solely for this smoke unless repeated interoperability failures justify that maintenance cost.

## Implementation order

1. Reconfirm final official stdio era-negotiation semantics and mismatch behavior.
2. Introduce connection-scoped `Undecided | Legacy | Modern20260728` state alongside, not inside, legacy `SessionState`.
3. Route opening exchanges through one race-safe era-selection helper.
4. Reject cross-era switching after pinning while preserving modern per-request `_meta` validation.
5. Add deterministic state-machine and subprocess mixed-era regressions.
6. Remove or neutralize the misleading `tool_invoke_output_schema()` contract; keep the facade output schema omitted.
7. Add target-diverse `tool_invoke` structured-result regressions.
8. Update lifecycle/facade documentation and maintainer skills.
9. Run focused tests, ordinary CI, full local verification, and one official-SDK stdio smoke.
10. Record corrective evidence in `plans/roadmap.md`; then continue with `mcp-surface-03-agent-evaluation-and-rollout.md`.

## Completion criteria

This corrective pass is complete when a stdio connection can select exactly one MCP protocol era and cannot switch eras afterward, modern request metadata remains correctly validated per request, legacy `SessionState` remains isolated to legacy connections, concurrent opening attempts cannot produce conflicting era state, `tool_invoke` no longer carries a false generic output-schema contract, discovery still advertises at most the intended seven full/Model definitions, profile/audience containment remains intact, direct mode remains the default, ordinary CI and the full local verification gate pass, and plan 03 can proceed without carrying known protocol/contract defects.