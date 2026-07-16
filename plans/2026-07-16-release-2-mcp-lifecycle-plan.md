# Release 2 — MCP Lifecycle and Protocol Negotiation

## Purpose

This release replaces the current permissive, stateless MCP startup behavior with a typed connection lifecycle and explicit protocol-version negotiation. It builds on Release 1 request tracking and cancellation guarantees.

The current server returns one hard-coded protocol revision, accepts initialize requests without typed parameters, and permits tools/list and tools/call before initialization. That is adequate for local compatibility testing but not for a mature MCP server.

## Goals

1. Parse and validate typed initialize parameters.
2. Maintain an explicit list of supported MCP protocol revisions.
3. Negotiate a protocol revision with the client.
4. Enforce a per-connection initialization lifecycle.
5. Track negotiated client capabilities and implementation metadata.
6. Advertise eggsact-specific protocol extensions explicitly.
7. Preserve intentional support for legacy clients through tested compatibility behavior.
8. Replace isolated one-request protocol tests with realistic multi-message sessions.

## Non-goals

- Do not implement every capability in the latest MCP specification.
- Do not add HTTP or SSE transport in this release.
- Do not silently claim unsupported capabilities.
- Do not retain pre-initialization tool calls merely for old tests; update the tests and provide an explicit compatibility option only if a real consumer requires it.
- Do not combine this release with the global calculator-state migration planned for Release 3.

# Workstream 1 — Typed protocol structures

## Initialize parameters

Add serde structures in `src/mcp/protocol.rs` for at least:

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub protocol_version: String,
    pub capabilities: ClientCapabilities,
    pub client_info: ImplementationInfo,
}
```

Use defaults only where the selected MCP revision allows omission. Missing required fields must produce `-32602 Invalid params`, not panic or silently substitute values.

Add structures for:

- client implementation name and version;
- client capabilities supported by eggsact's selected protocol revisions;
- server capabilities;
- optional instructions or metadata if eggsact chooses to emit them;
- negotiated session data.

Unknown capability fields should normally be ignored for forward compatibility, while known fields must be type-checked.

## Protocol error structure

Extend JSON-RPC errors with optional structured `data` where useful. Preserve existing helper call sites by adding builders or optional fields rather than forcing an immediate rewrite of all errors.

Use structured data for unsupported protocol versions and invalid lifecycle transitions where it helps harnesses recover deterministically.

# Workstream 2 — Supported version table and negotiation

## Version representation

Replace the single `MCP_PROTOCOL_VERSION` constant with:

- an ordered static list of supported revisions;
- a preferred/current revision;
- helper functions for support checks and negotiation.

Example:

```rust
pub const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &[
    "2025-11-25",
    "2024-11-05",
];

pub const PREFERRED_PROTOCOL_VERSION: &str = SUPPORTED_PROTOCOL_VERSIONS[0];
```

The exact revisions included must reflect behavior eggsact actually implements. Do not add a revision solely because it is newer.

## Negotiation behavior

Implement and document the selected MCP negotiation rule:

1. If the requested revision is supported, return it.
2. Otherwise return eggsact's preferred supported revision when the protocol permits this response pattern.
3. If no interoperable revision can be established, return a structured initialization error and keep the session uninitialized.

Store the negotiated revision in session state. All later protocol decisions must consult that value rather than global constants.

## Revision-specific behavior

Introduce a small compatibility layer rather than scattering string comparisons throughout `server.rs`.

Suggested methods:

```rust
impl NegotiatedProtocol {
    fn supports_initialized_notification(&self) -> bool;
    fn server_capabilities(&self) -> ServerCapabilities;
    fn allows_extension_capabilities(&self) -> bool;
}
```

Keep this layer narrow until revision differences require more behavior.

# Workstream 3 — Connection lifecycle state machine

## State model

Add per-stdio-session lifecycle state, for example:

```rust
enum SessionState {
    Uninitialized,
    AwaitingInitialized {
        negotiated: NegotiatedSession,
    },
    Ready {
        negotiated: NegotiatedSession,
    },
    ShuttingDown,
}
```

The state belongs to the server connection, not a process-global static. It may be protected by a lightweight async mutex or owned by the serial read loop, with immutable negotiated data cloned into request tasks after readiness.

## Allowed methods by state

### Uninitialized

Allow:

- `initialize`;
- protocol-safe ping if the selected interpretation permits it.

Reject:

- `tools/list`;
- `tools/call`;
- `profiles/list`;
- ordinary eggsact extensions;
- duplicate initialized notifications.

### AwaitingInitialized

Allow:

- `notifications/initialized`;
- cancellation for an initialization request if relevant;
- protocol-safe ping.

Decide and document whether ordinary requests are rejected or queued. Preferred behavior: reject until readiness rather than buffering unbounded work.

### Ready

Allow normal negotiated operations.

Reject duplicate initialize calls with a lifecycle error.

### ShuttingDown

Reject new work and drain or cancel active requests according to documented shutdown behavior.

## Atomic transitions

Initialize must transition the session exactly once. Two concurrent initialize requests must not both succeed. Because the read loop is serial, keep the transition in the read loop where possible rather than introducing avoidable locking.

# Workstream 4 — Capabilities and extensions

## Server capabilities

Advertise only capabilities eggsact implements. At minimum, tools capability remains present with the correct `listChanged` value.

If eggsact does not emit dynamic tool-list change notifications, keep `listChanged: false`.

## Client capabilities

Store client capabilities in `NegotiatedSession`. Do not emit optional notifications or extension behavior unless permitted by the negotiated protocol and client declaration.

## eggsact extensions

`profiles/list` is useful but not part of the base MCP tool methods. Advertise it through an explicit experimental or eggsact-namespaced capability.

Recommended structure:

```json
{
  "capabilities": {
    "tools": { "listChanged": false },
    "experimental": {
      "eggsact": {
        "profiles": true,
        "schemaDetail": true,
        "audienceFiltering": true
      }
    }
  }
}
```

Use the shape appropriate to the supported protocol revision. Document that extension methods remain additive and may evolve independently from base MCP.

Consider namespacing the method as `eggsact/profiles/list` in a future breaking release. For this release, preserving `profiles/list` may be preferable, but advertise and document it explicitly.

# Workstream 5 — Server integration

## Request dispatch context

After readiness, create a per-request protocol context containing:

- negotiated revision;
- client implementation info;
- client capabilities;
- active eggsact profile and audience;
- request ID and source.

Pass this to protocol-sensitive dispatch rather than rereading process-global state.

## Initialize handling

Move initialization out of the generic tool request handler if necessary. Initialization changes connection state and should be handled in a lifecycle-aware path.

The initialize result must include:

- negotiated protocol version;
- server capabilities for that revision;
- server implementation name and crate version;
- optional instructions only if deliberately supported.

## Initialized notification

Validate state before accepting the notification. It must not produce a response. Unknown or duplicate initialized notifications should be ignored or logged according to the specification, but must not corrupt state.

## Error behavior

Add deterministic errors for:

- method before initialization;
- duplicate initialize;
- initialized notification before initialize;
- malformed initialize params;
- unsupported/non-interoperable protocol revision;
- capability type errors.

Ensure error messages remain bounded and sanitized.

# Workstream 6 — Compatibility strategy

## Legacy protocol support

Retain `2024-11-05` only if eggsact can test and document its behavior. Add a compatibility matrix to `architecture/mcp-server.md` listing:

- supported revisions;
- preferred revision;
- initialization fields required per revision;
- extension capability shape;
- known deliberate differences.

## Pre-initialization compatibility escape hatch

Do not preserve permissive tool calls by default. If codegg or another real client still depends on them, add a temporary opt-in environment variable such as:

```text
EGGSACT_MCP_LEGACY_NO_INIT=1
```

Requirements for any escape hatch:

- disabled by default;
- clearly warned on stderr;
- omitted from normal examples;
- covered by tests;
- marked deprecated with a removal target.

Prefer fixing consumers instead of adding the escape hatch.

## Environment variable naming

Current variables use `EGGCALC_*` compatibility names. Do not rename them casually in this release. If new lifecycle variables are needed, prefer `EGGSACT_*` and document the naming transition for a later compatibility release.

# Workstream 7 — Test harness redesign

## Multi-message session helper

Add a reusable subprocess test harness that:

- starts one eggsact MCP process;
- writes multiple requests and notifications;
- keeps stdin open until the scenario completes;
- reads responses concurrently;
- correlates responses by ID;
- verifies notifications have no response;
- supports expected out-of-order tool responses;
- captures stderr separately for lifecycle diagnostics.

Avoid spawning a fresh process for every individual request in lifecycle tests.

## Required scenarios

### Successful initialization

- Client requests preferred revision.
- Client requests supported legacy revision.
- Response returns negotiated version, capabilities, and server info.
- Initialized notification transitions session to ready.
- tools/list and tools/call then succeed.

### Invalid ordering

- tools/list before initialize is rejected.
- tools/call before initialize is rejected.
- profiles/list before initialize is rejected.
- initialized notification before initialize produces no response and does not make session ready.
- tool call after initialize response but before initialized notification is rejected.
- second initialize request is rejected.

### Version negotiation

- Supported requested revision is echoed.
- Unsupported revision follows the documented fallback/error rule.
- Malformed revision type is rejected.
- Negotiated revision remains stable for the session.

### Capability validation

- Missing required client info is rejected.
- Wrong capability types are rejected.
- Unknown capability fields are tolerated.
- eggsact extension capability is present in the initialize response.

### Interaction with Release 1

- Cancellation remains available during lifecycle transitions where appropriate.
- Duplicate request IDs are still rejected.
- Rate limits do not prevent initialized notification processing.
- Active requests drain correctly on EOF.

# Documentation updates

Update:

- `README.md` MCP quick start to show initialize, initialized notification, tools/list, and tools/call sequence;
- `architecture/mcp-server.md` with lifecycle states, version table, capability negotiation, and extension contract;
- `architecture/compatibility.md` with revision compatibility policy;
- `architecture/testing.md` with the multi-message harness;
- generated docs source if protocol metadata is generated;
- `CHANGELOG.md` with the lifecycle behavior change.

Include a migration note for clients that previously called tools without initialization.

# Validation sequence

Focused:

```bash
cargo test --all-features --test lib mcp::test_protocol -- --nocapture
cargo test --all-features --test lib mcp::test_lifecycle_and_gaps -- --nocapture
cargo test --all-features --test lib mcp::test_comprehensive_parity -- --nocapture
```

Full:

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

Where available, run at least one external MCP client or inspector against the initialized session and record the evidence in a status note.

# Acceptance criteria

Release 2 is complete only when:

- initialize parameters are typed and validated;
- the negotiated protocol revision is stored per connection;
- ordinary methods are rejected until initialization completes;
- duplicate initialization cannot succeed;
- client capabilities and implementation info are retained;
- server capabilities accurately describe implemented behavior;
- eggsact extension methods are explicitly advertised;
- modern and legacy supported revisions have conformance fixtures;
- all lifecycle tests use realistic persistent sessions;
- migration documentation is complete;
- the full verification gate passes.

# Handoff notes

Implement the protocol structures and pure negotiation functions first. Add unit tests before modifying the read loop. Next introduce the lifecycle state and migrate initialize handling. Only after lifecycle enforcement is stable should tests and documentation be switched from permissive one-shot calls to initialized sessions. Keep revision-specific behavior centralized so future MCP revisions do not spread conditional logic throughout the server.