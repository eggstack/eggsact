# MCP Stdio Era-Classification Corrective Pass

Status: complete
Priority: P1
Scope: stdio opening-message classification, cross-era routing errors, notification-era enforcement, protocol documentation/tests; no discovery-ranking or model-evaluation work

## Objective

Bring the connection-pinning implementation added by `mcp-surface-02c` into exact behavioral alignment with the current official MCP TypeScript SDK v2 stdio serving model before starting plan 03.

The `02c` implementation fixed the important architectural problem: eggsact now has explicit connection-scoped `ConnectionEra::{Undecided, Legacy, Modern20260728}`, race-safe one-time era selection, isolated legacy `SessionState`, and no false generic `tool_invoke` output schema. Those changes should remain.

A post-implementation conformance review found three narrower routing mismatches:

1. eggsact currently leaves claim-less pre-opening traffic such as `ping` and unversioned `server/discover` in `ConnectionEra::Undecided`, while official `serveStdio` treats an `initialize` request **or any claim-less message** as a 2025-era/legacy opening when legacy serving is enabled;
2. eggsact currently rejects cross-era requests with eggsact-specific `-32600` / `ERA_MISMATCH`, while the current SDK treats an edge/instance era-classification mismatch as `-32022 Unsupported protocol version` for requests;
3. notification handling is still partly outside the era boundary (`notifications/cancelled` can currently act without matching the pinned era), whereas the current SDK drops era-mismatched notifications and reports the mismatch out-of-band.

This is a protocol-edge correction only. Do not redesign the MCP server, discovery facade, registry, execution budgets, cancellation mechanism, profiles/audiences, or tool schemas. Do not start plan 03's retrieval/model evaluation in this pass.

## Current source of truth

Reconfirm these sources immediately before implementation because MCP v2 remains actively maintained:

- MCP TypeScript SDK v2 `serveStdio` API:
  `https://ts.sdk.modelcontextprotocol.io/v2/api/@modelcontextprotocol/server/server/serveStdio.html`
- MCP TypeScript SDK v2 `2026-07-28` migration/support guide:
  `https://ts.sdk.modelcontextprotocol.io/v2/migration/support-2026-07-28`
- MCP TypeScript SDK v2 protocol-version guide:
  `https://ts.sdk.modelcontextprotocol.io/v2/protocol-versions`

As verified on 2026-09-10, the relevant official behavior is:

- `serveStdio` pins one server instance/era per stdio connection;
- with legacy serving enabled (the compatibility posture eggsact wants), a 2025-era opening is an `initialize` request **or any claim-less message**;
- modern negotiation uses a `server/discover` probe carrying the per-request modern `_meta` envelope; with the official `StdioClientTransport`, that probe runs in a disposable sibling process before the real connection is opened/pinned;
- once an instance is pinned, inbound edge classification is validated against the instance era and does not switch the era per message;
- an era-classification mismatch is an entry/routing error: requests receive `-32022 Unsupported protocol version`; notifications are dropped and surfaced through the host's error path rather than dispatched;
- once modern is negotiated, the SDK automatically attaches the modern envelope to every outgoing request and notification.

If the official SDK/source changes before implementation, follow the then-current released behavior and record the difference in the implementation commit and roadmap closure evidence. Do not preserve this plan's wording over a newer official contract.

## Part A — Separate edge era classification from method/envelope semantics

### A1. Keep the existing connection state, replace the opening policy

Retain the current connection-owned state:

```rust
ConnectionEra::Undecided
ConnectionEra::Legacy
ConnectionEra::Modern20260728
```

Retain race-safe selection with a short-held connection mutex and do not merge this state into legacy `SessionState`.

Change only the classification/opening policy so the first classifiable inbound message determines the connection era in the same way as official `serveStdio`.

Conceptually:

```text
first inbound JSON-RPC message
    |
    +-- explicit supported modern protocol claim --> Modern20260728
    |
    +-- initialize request ------------------------> Legacy
    |
    +-- otherwise claim-less ----------------------> Legacy
```

The exact classifier should be derived from the official SDK source/contract rather than inferred from method names beyond the explicit `initialize` rule.

### A2. Add one small inbound classification helper

Create or extract one testable helper responsible only for classifying an already structurally valid JSON-RPC message at the stdio edge. Suggested conceptual output:

```rust
enum InboundEraClassification {
    Legacy,
    Modern20260728,
    Unsupported { requested: String },
}
```

The exact shape may differ. The important property is that classification is centralized and occurs before method-specific dispatch/lifecycle side effects.

Do not make `parse_modern_request_meta()` carry two responsibilities. It should remain responsible for validating the modern request envelope once traffic has been classified into the modern era. Edge classification should answer only which era/revision the message claims, or whether that claim is unsupported.

This separation matters for malformed modern messages: the connection-era decision and the per-request schema/error decision are distinct concepts in the v2 SDK. Re-check the official classifier implementation to establish whether a message that explicitly claims `2026-07-28` but has other malformed modern envelope fields pins Modern before returning `-32602`, or remains unpinned. Encode the official behavior in focused tests rather than preserving the previous `02c` assumption that every malformed envelope must leave `Undecided`.

Likewise, re-check unsupported-version opening behavior. Use the official transport/dispatcher semantics for whether `Undecided` remains open for a retry after an unsupported claim. Do not guess.

### A3. Claim-less traffic must no longer remain neutral

Remove the current special cases that allow claim-less messages to be handled while leaving `ConnectionEra::Undecided`.

At minimum, the following first messages must be treated as legacy openings under eggsact's dual-era/legacy-serving compatibility mode:

- claim-less `ping`;
- claim-less `tools/list` / `tools/call` / `profiles/list`;
- claim-less unknown methods;
- claim-less `server/discover`;
- claim-less notifications, if the current official `serveStdio` classifier treats them as connection-opening traffic under `legacy: "serve"`.

After such an opening, existing legacy lifecycle rules decide the method result. For example, a first claim-less `tools/list` may still return `NOT_INITIALIZED`; the important correction is that the connection is now legacy-pinned, so a later modern claim cannot switch it.

### A4. Remove the unversioned `server/discover` neutrality exception

Delete the current rule that unversioned `server/discover` always answers successfully without pinning, including after the connection is already pinned.

The official auto-negotiation probe already carries the modern protocol envelope. Therefore:

- an enveloped `server/discover` opening may select Modern and return the modern discover result;
- a claim-less `server/discover` is classified as legacy under dual-era `serve` behavior and must then be handled according to the legacy instance's lifecycle/method registry;
- `server/discover` must not be an era-neutral escape hatch on an already legacy-pinned connection;
- a claim-less `server/discover` must never silently turn into a modern success response merely because eggsact implements that method in its other era.

Do not add a new fallback probe method.

## Part B — Use the official mismatch error contract

### B1. Replace `ERA_MISMATCH` request responses with `-32022`

Remove the eggsact-specific cross-era request contract added by `02c`:

```text
-32600
error.data.code = "ERA_MISMATCH"
```

For a request whose inbound classification does not match the connection's pinned era, return the same unsupported-protocol-version class used by the official SDK: `-32022 Unsupported protocol version`.

Reuse/refactor the existing `unsupported_protocol_version(...)` helper where possible. Do not create a second semantically equivalent error builder.

The error should preserve the original JSON-RPC `id` and expose only fields consistent with eggsact's existing `-32022` contract and the released MCP behavior. If the current helper includes `supported` / `requested`, populate them truthfully for an era mismatch; do not fabricate a requested version for a genuinely claim-less legacy-classified message unless the official contract does so.

If needed, introduce a narrow internal helper that feeds the standard unsupported-version response without adding a new public machine-code vocabulary.

### B2. Remove stale `ERA_MISMATCH` maintenance surface

After tests are migrated, remove `ERA_MISMATCH_CODE`, `era_mismatch()`, and documentation/skills text that presents `ERA_MISMATCH` as the wire contract, unless another real consumer still requires them.

Do not retain dead compatibility code merely because it existed for one corrective commit. This is pre-rollout protocol work, and the goal is to converge on the released contract before users are told to depend on it.

### B3. Keep method-level `-32601` separate from era routing

Do not conflate an era-classification mismatch with a method that simply does not exist in the correctly pinned era.

Required distinction:

- wrong era/revision at the stdio routing boundary -> `-32022` for requests;
- correctly classified/pinned message invoking a method removed or absent in that era -> normal method semantics, generally `-32601` where applicable;
- legacy lifecycle violations inside a correctly legacy-pinned connection -> existing `NOT_INITIALIZED` / duplicate-initialize behavior as appropriate.

This distinction should be explicit in tests.

## Part C — Route notifications through the same era boundary

### C1. Classify every inbound notification before side effects

Move notification handling behind the same connection-era classification/pinning check used for requests.

Today `notifications/cancelled` can reach `apply_cancellation()` without proving that the notification belongs to the pinned era. Correct that.

For a notification whose classification matches the pinned era, preserve the existing notification semantics. For a mismatched notification:

- emit no JSON-RPC response;
- perform no lifecycle transition;
- perform no cancellation side effect;
- drop the notification;
- optionally emit one deterministic stderr diagnostic analogous to the SDK's `onerror` path, if useful for debugging and consistent with existing eggsact stderr policy.

Do not add a new stdout protocol message for a dropped notification.

### C2. Preserve cancellation in both supported stdio eras

Current official guidance states that stdio at either era still uses `notifications/cancelled` (the stream-close change applies to modern Streamable HTTP, not stdio).

Therefore verify both paths:

- legacy-pinned connection + claim-less/legacy cancellation notification -> existing cancellation flag behavior;
- modern-pinned connection + correctly enveloped modern cancellation notification -> existing cancellation flag behavior;
- legacy cancellation sent to Modern or modern-enveloped cancellation sent to Legacy -> dropped, no cancellation side effect.

Do not remove stdio cancellation from modern mode.

### C3. Correct `notifications/initialized` behavior at the edge

A claim-less `notifications/initialized` belongs to the legacy era. A modern-enveloped version must not transition legacy `SessionState`.

If received on the wrong pinned era, treat it as a classification mismatch notification and drop it rather than dispatching method-specific lifecycle behavior first.

Keep legacy `AwaitingInitialized -> Ready` behavior unchanged once classification has admitted the notification to a legacy-pinned connection.

## Part D — Regression test matrix

Prefer state-machine/unit tests for deterministic classification transitions and subprocess tests for wire behavior. Do not rely on sleeps or scheduler timing.

### D1. Opening classification tests

Add/replace tests proving at least:

1. fresh -> claim-less `ping` pins Legacy; subsequent modern-enveloped request gets `-32022`;
2. fresh -> claim-less `tools/list` pins Legacy and returns the existing legacy pre-init error; subsequent modern request gets `-32022`;
3. fresh -> claim-less unknown method pins Legacy; later valid modern request cannot take over;
4. fresh -> claim-less `server/discover` does **not** return a modern discovery success and pins/behaves as Legacy per official semantics;
5. fresh -> correctly enveloped `server/discover` selects Modern and succeeds;
6. fresh -> direct correctly enveloped modern `tools/list` selects Modern if the official SDK permits direct modern opening;
7. Modern-pinned -> claim-less request is rejected with `-32022` and original id preserved;
8. Legacy-pinned -> modern-classified request is rejected with `-32022` and original id preserved.

Remove or rewrite tests that assert claim-less `ping` or unversioned `server/discover` leaves `Undecided`.

### D2. Invalid/unsupported opening tests

Use the official v2 source to pin exact behavior for:

- explicit supported modern revision with malformed required envelope members;
- unsupported protocol revision;
- malformed/non-string protocol-version claim;
- malformed legacy `initialize` after the message has already classified the connection as Legacy.

The previous `02c` blanket rule "invalid envelopes and failed initialize never pin" must not survive unless it matches the official classifier. Classification should occur where the official server performs it, and semantic validation should occur after classification where appropriate.

Record the chosen behavior in `architecture/mcp-server.md` so future refactors do not reintroduce ambiguity.

### D3. Notification mismatch tests

Add focused tests proving:

- legacy cancellation cancels an active legacy request;
- modern-enveloped cancellation cancels an active modern request;
- claim-less cancellation sent after Modern pin is dropped and does not set the target cancel flag;
- modern-enveloped cancellation sent after Legacy pin is dropped and does not set the target cancel flag;
- wrong-era `notifications/initialized` cannot transition legacy lifecycle state;
- dropped notifications produce no stdout JSON-RPC response.

If stderr diagnostics are retained, test only stable machine-relevant text or behavior; do not overfit full prose messages.

### D4. Preserve the useful `02c` tests

Keep the existing deterministic single-winner connection-era race test, but update its surrounding assumptions to the corrected classifier.

Keep the `tool_invoke` target-diverse structured-content tests and no-output-schema assertions unchanged. They solved a separate valid `02c` issue and must not regress during this routing correction.

Keep representative legacy-vs-modern tool semantic goldens on separate connections.

## Part E — Official SDK interoperability smoke

After the correction, rerun a current official TypeScript SDK v2 stdio smoke without adding Node/npm as a permanent runtime or ordinary CI dependency.

Record at minimum:

```text
versionNegotiation: auto     -> disposable enveloped discover probe -> modern connection
versionNegotiation: default  -> initialize opening -> legacy connection
modern tools/list            -> succeeds on modern connection
legacy tools/list            -> succeeds after legacy initialize/initialized
```

Add one small raw-wire or SDK-assisted conformance check demonstrating that a claim-less first message follows the legacy opening path. If practical, compare eggsact's behavior to `serveStdio` directly for the same opening transcript.

Record SDK package/version and date in the implementation commit or roadmap closure evidence.

Do not add an SDK interoperability job to ordinary CI unless future breakage demonstrates recurring value.

## Part F — Documentation cleanup

Audit and correct all lifecycle/error wording introduced by `02c`, especially:

- `AGENTS.md`;
- `architecture/mcp-server.md`;
- `architecture/compatibility.md`;
- `docs/mcp-tools.md`;
- `docs/compatibility-policy.md`;
- README protocol examples;
- `.opencode/skills/debugging/SKILL.md`;
- `.opencode/skills/mcp-tools/SKILL.md`.

Required documentation points:

- one stdio process/connection is still pinned to exactly one era;
- `ConnectionEra` remains separate from legacy `SessionState`;
- the first supported modern claim selects Modern; `initialize` or other claim-less opening traffic selects Legacy under eggsact's compatibility-serving posture;
- the official modern stdio probe is enveloped and uses a disposable sibling process with `StdioClientTransport` auto-negotiation;
- claim-less `server/discover` is not era-neutral;
- cross-era/classification-mismatch requests use `-32022`, not `ERA_MISMATCH`;
- mismatched notifications are dropped before side effects;
- modern per-request `_meta` remains mandatory/validated for modern traffic after pinning;
- `tool_invoke` still intentionally has no facade-level output schema.

Avoid presenting TypeScript SDK implementation details as universal transport requirements where they are only SDK behavior. This corrective pass is specifically aligning eggsact's stdio dual-era compatibility behavior with the current official reference implementation.

## Part G — Scope and non-goals

Preserve all of the following:

- stdio-only MCP transport;
- 86 underlying deterministic tools and existing canonical names;
- `ToolSpec` registry architecture;
- `McpSurface::Direct` as the default;
- explicit/opt-in `Discovery` surface;
- five pinned discovery front doors plus `tool_search` / `tool_invoke` (at most seven `full`/Model advertised definitions);
- profile/audience containment through `ToolRegistry`;
- existing budget, semaphore, timeout, cancellation, truncation, and request-id tracking mechanisms;
- no new runtime dependencies;
- no daemon/process-manager work;
- no HTTP transport work;
- no Tasks/MRTR/subscriptions implementation unless separately planned;
- no embeddings/vector search/provider SDK/model calls;
- no plan 03 ranking or model-evaluation work;
- no change to generated client integration defaults.

Do not introduce a generic transport abstraction merely to express this correction. A small stdio-edge classifier plus existing `ConnectionEra` is sufficient.

## Part H — Verification

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

Ordinary CI must be green on the final implementation commit.

Focused acceptance matrix:

```text
fresh -> enveloped modern discover                 MODERN / PASS
fresh -> direct valid modern request               MODERN / PASS (if official source permits)
fresh -> initialize                                LEGACY / PASS
fresh -> claim-less ping                           LEGACY
fresh -> claim-less tools/list                     LEGACY + existing pre-init error
fresh -> claim-less server/discover                LEGACY semantics; NOT modern neutral success
legacy-pinned -> modern-classified request         -32022, id preserved
modern-pinned -> claim-less/legacy request         -32022, id preserved
wrong-era notification                             DROPPED; no stdout; no side effect
legacy cancellation                                PASS
modern stdio cancellation with modern envelope     PASS
connection opening race                            exactly one era winner
tool_invoke facade outputSchema                    ABSENT
discovery full/Model advertised count              <= 7
direct default                                     UNCHANGED
profile/audience containment                       PASS
ordinary CI                                        PASS
full local release gate                            PASS
o new runtime dependency                           PASS
```

Where invalid/malformed opening rows depend on the exact official classifier, add them after source confirmation and record the final expected state explicitly.

## Implementation order

1. Re-read the current official `serveStdio` implementation/docs and capture exact classification rules for supported, malformed, unsupported, claim-less, request, and notification openings.
2. Extract one stdio inbound era-classification helper separate from modern envelope validation.
3. Change opening semantics so `initialize`/claim-less traffic selects Legacy and a supported modern claim selects Modern.
4. Remove the unversioned `server/discover` neutrality exception.
5. Replace `ERA_MISMATCH` request handling with the standard `-32022` unsupported-protocol-version path; remove stale helper/code/docs.
6. Route notifications through classification/pinned-era validation before lifecycle or cancellation side effects.
7. Rewrite/add deterministic state and subprocess regressions, preserving `02c` race and `tool_invoke` coverage.
8. Update architecture/user/maintainer documentation.
9. Run the full verification gate and current official TypeScript SDK stdio smoke.
10. Record closure evidence in `plans/roadmap.md`; only then proceed to `mcp-surface-03-agent-evaluation-and-rollout.md`.

## Completion criteria

This pass is complete when eggsact's dual-era stdio entry behavior matches the current official v2 reference semantics: one connection pins one era; a supported modern claim selects Modern; `initialize` or other claim-less opening traffic selects Legacy under compatibility serving; unversioned `server/discover` is no longer an era-neutral bypass; cross-era/classification-mismatch requests return the standard `-32022` routing error with id preserved; mismatched notifications are dropped before any lifecycle/cancellation side effect; valid legacy and modern cancellation still work; the useful `02c` race and generic-invoke corrections remain intact; discovery remains opt-in and <=7 definitions; direct remains default; no new runtime dependencies are introduced; ordinary CI/full verification pass; official SDK auto/default stdio smokes select modern/legacy respectively; and plan 03 can begin without a known stdio-era conformance discrepancy.

## Closure evidence

Implementation and test evidence is recorded in the final commit. The edge
classifier is `classify_inbound_era` in `src/mcp/runtime.rs`; request routing
uses the standard `-32022` unsupported-version constructors in
`src/mcp/protocol.rs`; notifications are classified and pinned before
`notifications/initialized` or `notifications/cancelled` side effects in
`src/mcp/server.rs`. Focused regressions live in
`tests/mcp/test_era_pinning.rs`, `tests/mcp/test_modern_protocol.rs`, and the
runtime state-machine tests. The official reference checked on 2026-09-10
was MCP TypeScript SDK v2 package `@modelcontextprotocol/server@2.0.0`,
including its `serveStdio` source and 2026-07-28 migration guidance. Local
verification on 2026-09-10 passed `cargo fmt --all -- --check`, generated-doc
freshness, clippy with `-D warnings`, cargo-deny, and the full
`cargo test --locked --all-features -- --skip parity --test-threads=4` gate:
3,002 integration/unit tests, 51 context-isolation tests, and 11 doc tests,
with zero failures. The ephemeral official SDK smoke against
`@modelcontextprotocol/client@2.0.0` reported
`versionNegotiation=auto -> modern (77 tools)` and
`versionNegotiation=default -> legacy (77 tools)`.
