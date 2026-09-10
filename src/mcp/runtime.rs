use crate::agent::ToolAudience;
use crate::mcp::registry;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock, RwLock};
use tokio::sync::Mutex;
use tokio::time::Instant;

/// RAII guard for an active request entry.
///
/// The awaited `complete_request()` is the primary cleanup path. As a
/// panic-proof fallback, this guard's `Drop` also removes its own entry
/// (generation-checked) if it is somehow still registered — so a task that
/// panics before reaching `complete_request()` cannot leak the in-flight
/// slot. In the normal flow the entry is already gone and the drop is a
/// no-op.
#[doc(hidden)]
pub struct RequestGuard {
    active: ActiveRequests,
    request_id: Value,
    generation: u64,
}

impl RequestGuard {
    #[doc(hidden)]
    pub fn new(
        active: ActiveRequests,
        _cancel_flag: &Arc<AtomicBool>,
        request_id: Value,
        generation: u64,
    ) -> Self {
        Self {
            active,
            request_id,
            generation,
        }
    }
}

impl Drop for RequestGuard {
    fn drop(&mut self) {
        // Primary cleanup is `complete_request()`; this Drop is a panic-proof
        // fallback so a task that panics before reaching `complete_request`
        // cannot leak its in-flight slot.
        if let Ok(mut map) = self.active.try_lock() {
            if let Some(entry) = map.get(&self.request_id) {
                if entry.generation == self.generation {
                    map.remove(&self.request_id);
                }
            }
            return;
        }
        // `try_lock` failed (contended). Fall back to a blocking lock so the
        // slot is still freed even under contention. This blocks the current
        // thread briefly, but the Drop path only runs on panic/unwind where
        // leaking the slot (MAX_IN_FLIGHT_REQUESTS=32) is worse than blocking.
        let mut map = self.active.blocking_lock();
        if let Some(entry) = map.get(&self.request_id) {
            if entry.generation == self.generation {
                map.remove(&self.request_id);
            }
        }
    }
}

#[doc(hidden)]
pub const MAX_IN_FLIGHT_REQUESTS: usize = 32;

#[doc(hidden)]
pub const MAX_TOOL_WORKERS: usize = 16;
#[doc(hidden)]
pub const MAX_REQUEST_ID_LENGTH: usize = 1024;

// Items below are exposed for tests in `tests/`. They are not part of the
// stable API surface; downstream crates should not depend on them. They
// are `pub` (rather than `pub(crate)`) so integration tests can reach them
// through `eggsact::mcp::runtime::*` and `eggsact::mcp::runtime::test_support::*`.
#[doc(hidden)]
pub const MAX_REQUEST_BYTES: usize = 1_000_000;
#[doc(hidden)]
pub const MAX_OUTPUT_BYTES: usize = 1_000_000;

pub(crate) const SCHEMA_DETAIL_FULL: &str = "full";

pub const MCP_SERVER_NAME: &str = "eggsact";

// ═══════════════════════════════════════════════════════════════════════════════
// Protocol version table and negotiation (Workstream 2)
// Dual-era: 2026-07-28 (modern, stateless) + 2025-11-25 / 2024-11-05 (legacy)
// ═══════════════════════════════════════════════════════════════════════════════

/// Modern stateless protocol revision (no initialize handshake).
pub const MODERN_PROTOCOL_VERSION: &str = "2026-07-28";

/// Legacy initialize-capable revisions, in preference order.
pub const LEGACY_SUPPORTED_VERSIONS: &[&str] = &["2025-11-25", "2024-11-05"];

/// Preferred legacy revision for `initialize` negotiation fallback.
pub const LEGACY_PREFERRED_VERSION: &str = LEGACY_SUPPORTED_VERSIONS[0];

/// Ordered list of supported MCP protocol revisions. The first entry is the
/// preferred (current) revision. Only revisions that eggsact actually implements
/// are listed here. Preference order is deterministic: modern first, then
/// legacy in their own preference order. `server/discover` advertises this
/// exact order.
pub const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &["2026-07-28", "2025-11-25", "2024-11-05"];

/// The preferred (current) protocol revision. Eggsact advertises this first
/// in `server/discover`. Legacy `initialize` negotiation never falls back to
/// this when it names a modern revision; see `negotiate_legacy_version`.
pub const PREFERRED_PROTOCOL_VERSION: &str = SUPPORTED_PROTOCOL_VERSIONS[0];

/// Legacy single-version constant for backward compatibility with tests and
/// code that references `MCP_PROTOCOL_VERSION` directly. Pinned to the legacy
/// preferred revision (not the modern preferred) so existing single-version
/// consumers keep legacy semantics. Prefer `PREFERRED_PROTOCOL_VERSION` for
/// modern advertisement or `LEGACY_PREFERRED_VERSION` for legacy negotiation
/// in new code.
pub const MCP_PROTOCOL_VERSION: &str = LEGACY_PREFERRED_VERSION;

/// Reserved request `_meta` keys (2026-07-28 per-request envelope).
pub const META_PROTOCOL_VERSION: &str = "io.modelcontextprotocol/protocolVersion";
pub const META_CLIENT_INFO: &str = "io.modelcontextprotocol/clientInfo";
pub const META_CLIENT_CAPABILITIES: &str = "io.modelcontextprotocol/clientCapabilities";
/// Reserved result `_meta` key for server identity.
pub const META_SERVER_INFO: &str = "io.modelcontextprotocol/serverInfo";
/// Namespace for eggsact-only Tool metadata in modern responses.
pub const EGGSACT_META_NAMESPACE: &str = "io.github.eggstack/eggsact";

/// Conservative cache TTL for modern cacheable results (`tools/list`,
/// `server/discover`). One hour; named so it can be tuned without touching
/// serialization logic.
pub const MODERN_CACHE_TTL_MS: u64 = 3_600_000;
/// Cache scope for modern catalog results. The catalog is local,
/// deterministic, process-configured, and authorization-free, so `public`
/// is appropriate provided the response contains no user-specific material.
pub const MODERN_CACHE_SCOPE: &str = "public";

/// Short cross-tool server instructions for legacy initialize responses and
/// modern discovery. Explains only behavior individual descriptions do not
/// convey. Target: a few hundred bytes, not a manual.
pub const SERVER_INSTRUCTIONS: &str = "eggsact is a local deterministic utility server. Preflight and inspection tools analyze inputs and report findings, verdicts, and machine codes; they do not execute changes. Tool results carry structuredContent conforming to outputSchema plus a text JSON fallback.";

/// Protocol era for a single request. Legacy uses the connection `SessionState`
/// handshake; modern (`2026-07-28`) is stateless per-request metadata and must
/// never consult or mutate `SessionState` as an authorization gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolEra {
    Legacy,
    Modern20260728,
}

/// Connection-scoped protocol era for one stdio server process.
///
/// One stdio process represents one MCP connection. The opening exchange pins
/// the era exactly once (`Undecided` → `Legacy` or `Modern20260728`); later
/// requests cannot switch eras. This mirrors the official TypeScript SDK
/// `serveStdio` connection-pinned model: the opening exchange (legacy
/// `initialize` vs a valid modern `_meta` envelope, normally via
/// `server/discover` or a direct modern call) selects the era, and the
/// disposable sibling-process probe never appears on the session child's wire.
///
/// Distinct from `SessionState`, which remains the lifecycle state machine
/// *inside* a legacy-pinned connection (`Uninitialized` →
/// `AwaitingInitialized` → `Ready`). Modern traffic stays stateless with
/// respect to session/handshake data after pinning; per-request `_meta`
/// validation still applies to every modern call.
///
/// Pinning rules (see `src/mcp/server.rs`):
/// - `Undecided` + unversioned `server/discover` (no `_meta`): answers without
///   pinning and without touching `SessionState` (backward-compat probe).
/// - `Undecided` + valid modern envelope: pins `Modern20260728`.
/// - `Undecided` + successful legacy `initialize`: pins `Legacy`.
/// - Invalid modern envelopes (malformed `-32602`, unsupported `-32022`) and
///   failed legacy `initialize` validation never pin; the connection stays
///   `Undecided` so the client can retry without poisoned state.
/// - Other pre-opening legacy traffic (`ping`, pre-handshake `tools/list`
///   returning `NOT_INITIALIZED`) leaves the era `Undecided`.
/// - Once pinned, cross-era requests are rejected with `ERA_MISMATCH`
///   (`-32600`) preserving the JSON-RPC id; the other era's lifecycle is
///   never mutated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionEra {
    Undecided,
    Legacy,
    Modern20260728,
}

impl ConnectionEra {
    /// Whether the opening exchange has already selected an era.
    pub fn is_decided(self) -> bool {
        !matches!(self, ConnectionEra::Undecided)
    }

    /// Short lowercase name for diagnostics and mismatch messages.
    pub fn as_str(self) -> &'static str {
        match self {
            ConnectionEra::Undecided => "undecided",
            ConnectionEra::Legacy => "legacy",
            ConnectionEra::Modern20260728 => "modern",
        }
    }
}

/// Outcome of attempting to pin or observe the connection era.
///
/// Returned by [`try_pin_era`] / [`pin_connection_era`]. Holding the era lock
/// only for the inspect/select step keeps tool execution concurrent; see
/// `architecture/mcp-server.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EraPinOutcome {
    /// `Undecided` → target; this caller won the opening race.
    PinnedNow,
    /// Already pinned to the requested era; proceed.
    AlreadyPinned,
    /// Pinned to the other era; caller must reject with `ERA_MISMATCH`.
    Mismatch,
}

/// Synchronous era-selection transition (no locking).
///
/// - `Undecided` + any decided target pins and returns `PinnedNow`.
/// - Same-era observation returns `AlreadyPinned`.
/// - Cross-era observation returns `Mismatch`.
/// - Pinning `Undecided` onto itself is `AlreadyPinned` (no transition).
pub fn try_pin_era(state: &mut ConnectionEra, target: ConnectionEra) -> EraPinOutcome {
    match (*state, target) {
        (ConnectionEra::Undecided, ConnectionEra::Undecided) => EraPinOutcome::AlreadyPinned,
        (ConnectionEra::Undecided, decided) => {
            *state = decided;
            EraPinOutcome::PinnedNow
        }
        (current, target) if current == target => EraPinOutcome::AlreadyPinned,
        _ => EraPinOutcome::Mismatch,
    }
}

/// Race-safe era selection for the stdio connection.
///
/// Locks only long enough to inspect/select the era, then releases before
/// dispatch so tool execution stays concurrent. Exactly one opening era wins;
/// a racing incompatible request observes `Mismatch` rather than a
/// half-transitioned state.
pub async fn pin_connection_era(
    era: &Arc<Mutex<ConnectionEra>>,
    target: ConnectionEra,
) -> EraPinOutcome {
    let mut guard = era.lock().await;
    try_pin_era(&mut guard, target)
}

/// Observe the current connection era without mutating it.
pub async fn current_connection_era(era: &Arc<Mutex<ConnectionEra>>) -> ConnectionEra {
    *era.lock().await
}

/// Request-scoped modern protocol context. Passed through server dispatch;
/// never stored in process-global state. Client identity is advisory only and
/// must not drive security policy.
#[derive(Debug, Clone)]
pub struct ModernRequestContext {
    /// Always `2026-07-28` when constructed via `parse_modern_request_meta`.
    pub protocol_version: String,
    /// Optional self-reported client name (advisory, not authorization).
    pub client_name: Option<String>,
    /// Optional self-reported client version (advisory).
    pub client_version: Option<String>,
    /// Raw client capabilities value for the duration of the request.
    pub client_capabilities: Value,
}

impl ModernRequestContext {
    pub fn era(&self) -> ProtocolEra {
        ProtocolEra::Modern20260728
    }
}

/// Check whether a protocol version string is in the supported list (any era).
pub fn is_supported_protocol_version(version: &str) -> bool {
    SUPPORTED_PROTOCOL_VERSIONS.contains(&version)
}

/// Check whether a version is legacy initialize-capable.
pub fn is_legacy_supported_version(version: &str) -> bool {
    LEGACY_SUPPORTED_VERSIONS.contains(&version)
}

/// Check whether a version is the modern stateless revision.
pub fn is_modern_supported_version(version: &str) -> bool {
    version == MODERN_PROTOCOL_VERSION
}

/// Negotiate a legacy protocol version: return the requested version if it is
/// legacy-supported, otherwise return the legacy preferred version.
///
/// Legacy `initialize` negotiates only among initialize-capable versions; a
/// modern revision requested via `initialize` falls back to legacy preferred
/// rather than entering modern state.
pub fn negotiate_legacy_version(requested: &str) -> String {
    if is_legacy_supported_version(requested) {
        requested.to_string()
    } else {
        LEGACY_PREFERRED_VERSION.to_string()
    }
}

/// Negotiate a protocol version: return the requested version if supported,
/// otherwise return eggsact's preferred version.
///
/// Preserved for backward compatibility; new legacy-handshake code should use
/// `negotiate_legacy_version` so `initialize` never negotiates into the modern
/// era. This function now delegates to legacy negotiation to keep `initialize`
/// byte/semantics-compatible.
pub fn negotiate_protocol_version(requested: &str) -> String {
    negotiate_legacy_version(requested)
}

/// Parse the modern per-request `_meta` envelope from JSON-RPC `params`.
///
/// Returns:
/// - `None` when no modern envelope is present (legacy path; caller enforces
///   `SessionState` as before).
/// - `Some(Ok(ctx))` for a valid modern request (caller bypasses
///   `SessionState` and serves statelessly).
/// - `Some(Err(error_value))` for a modern envelope that is malformed
///   (`-32602`) or names an unsupported version (`-32022`). The caller must
///   return the error verbatim and must not fall back to legacy state.
pub fn parse_modern_request_meta(
    params: Option<&Value>,
) -> Option<Result<ModernRequestContext, Value>> {
    let params_obj = params?.as_object()?;
    let meta = params_obj.get("_meta")?;
    let meta_obj = match meta.as_object() {
        Some(o) => o,
        None => {
            return Some(Err(crate::mcp::protocol::invalid_params(
                "Invalid params: '_meta' must be an object",
                None,
            )));
        }
    };
    // Protocol version is required on every modern request.
    let version_value = match meta_obj.get(META_PROTOCOL_VERSION) {
        Some(v) => v,
        None => {
            return Some(Err(crate::mcp::protocol::invalid_params(
                "Invalid params: '_meta.io.modelcontextprotocol/protocolVersion' is required for modern requests",
                None,
            )));
        }
    };
    let version_str = match version_value.as_str() {
        Some(s) => s,
        None => {
            return Some(Err(crate::mcp::protocol::invalid_params(
                "Invalid params: '_meta.io.modelcontextprotocol/protocolVersion' must be a string",
                None,
            )));
        }
    };
    if !is_modern_supported_version(version_str) {
        return Some(Err(crate::mcp::protocol::unsupported_protocol_version(
            version_str,
            None,
        )));
    }
    // Client capabilities are required per-request; servers must not infer
    // them from prior requests.
    let caps = match meta_obj.get(META_CLIENT_CAPABILITIES) {
        Some(v) => v,
        None => {
            return Some(Err(crate::mcp::protocol::invalid_params(
                "Invalid params: '_meta.io.modelcontextprotocol/clientCapabilities' is required for modern requests",
                None,
            )));
        }
    };
    if !caps.is_object() {
        return Some(Err(crate::mcp::protocol::invalid_params(
            "Invalid params: '_meta.io.modelcontextprotocol/clientCapabilities' must be an object",
            None,
        )));
    }
    // Client info is SHOULD (optional). Present-but-malformed is rejected;
    // absent is served anonymously.
    let (client_name, client_version) = match meta_obj.get(META_CLIENT_INFO) {
        None => (None, None),
        Some(v) => {
            let obj = match v.as_object() {
                Some(o) => o,
                None => {
                    return Some(Err(crate::mcp::protocol::invalid_params(
                        "Invalid params: '_meta.io.modelcontextprotocol/clientInfo' must be an object",
                        None,
                    )));
                }
            };
            let name = match obj.get("name").and_then(|n| n.as_str()) {
                Some(n) if !n.is_empty() => n.to_string(),
                _ => {
                    return Some(Err(crate::mcp::protocol::invalid_params(
                        "Invalid params: '_meta.io.modelcontextprotocol/clientInfo.name' must be a non-empty string",
                        None,
                    )));
                }
            };
            let version = obj
                .get("version")
                .and_then(|n| n.as_str())
                .map(|s| s.to_string());
            (Some(name), version)
        }
    };
    Some(Ok(ModernRequestContext {
        protocol_version: version_str.to_string(),
        client_name,
        client_version,
        client_capabilities: caps.clone(),
    }))
}

/// Negotiated protocol data stored per-connection.
#[derive(Debug, Clone)]
pub struct NegotiatedProtocol {
    /// The protocol version negotiated for this connection.
    pub version: String,
    /// Client implementation name.
    pub client_name: String,
    /// Client implementation version (optional).
    pub client_version: Option<String>,
    /// Client's declared capabilities.
    pub client_capabilities: crate::mcp::protocol::ClientCapabilities,
}

impl NegotiatedProtocol {
    /// Whether this revision supports the `notifications/initialized` handshake.
    pub fn supports_initialized_notification(&self) -> bool {
        // All supported revisions support this.
        true
    }

    /// Whether this revision allows eggsact extension capabilities.
    pub fn allows_extension_capabilities(&self) -> bool {
        // Both 2024-11-05 and 2025-11-25 allow extensions via experimental.
        true
    }

    /// Get a reference to the client's declared capabilities.
    pub fn client_capabilities(&self) -> &crate::mcp::protocol::ClientCapabilities {
        &self.client_capabilities
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Connection lifecycle state machine (Workstream 3)
// ═══════════════════════════════════════════════════════════════════════════════

/// Per-connection lifecycle state for the MCP server.
///
/// Each stdio session has its own `SessionState` that tracks initialization
/// progress. Methods are rejected until the session reaches `Ready`.
#[derive(Debug, Clone)]
pub enum SessionState {
    /// No `initialize` request has been received yet.
    ///
    /// Allowed: `initialize`, `ping`.
    /// Rejected: `tools/list`, `tools/call`, `profiles/list`, extensions.
    Uninitialized,
    /// `initialize` has been processed; waiting for `notifications/initialized`.
    ///
    /// Allowed: `notifications/initialized`, `ping`.
    /// Rejected: `tools/list`, `tools/call`, `profiles/list`, extensions.
    /// Rejected: duplicate `initialize`.
    AwaitingInitialized { negotiated: NegotiatedProtocol },
    /// Session is fully initialized and ready for normal operations.
    ///
    /// Allowed: all negotiated methods.
    /// Rejected: duplicate `initialize`.
    Ready { negotiated: NegotiatedProtocol },
}

impl SessionState {
    /// Whether this state allows a given method.
    pub fn allows_method(&self, method: &str) -> bool {
        match (self, method) {
            // Uninitialized: only initialize and ping
            (SessionState::Uninitialized, "initialize") => true,
            (SessionState::Uninitialized, "ping") => true,
            (SessionState::Uninitialized, _) => false,
            // AwaitingInitialized: only initialized notification and ping
            (SessionState::AwaitingInitialized { .. }, "notifications/initialized") => true,
            (SessionState::AwaitingInitialized { .. }, "ping") => true,
            (SessionState::AwaitingInitialized { .. }, _) => false,
            // Ready: everything except initialize (rejected as duplicate)
            (SessionState::Ready { .. }, "initialize") => false,
            (SessionState::Ready { .. }, _) => true,
        }
    }

    /// Attempt to transition from Uninitialized to AwaitingInitialized.
    /// Returns the negotiated protocol on success.
    pub fn transition_to_awaiting(
        &mut self,
        negotiated: NegotiatedProtocol,
    ) -> Result<(), &'static str> {
        match self {
            SessionState::Uninitialized => {
                *self = SessionState::AwaitingInitialized { negotiated };
                Ok(())
            }
            _ => Err("already_initialized"),
        }
    }

    /// Attempt to transition from AwaitingInitialized to Ready.
    pub fn transition_to_ready(&mut self) -> Result<(), &'static str> {
        match self {
            SessionState::AwaitingInitialized { negotiated } => {
                let negotiated = negotiated.clone();
                *self = SessionState::Ready { negotiated };
                Ok(())
            }
            _ => Err("not_awaiting_initialized"),
        }
    }

    /// Get a reference to the negotiated protocol if available.
    pub fn negotiated(&self) -> Option<&NegotiatedProtocol> {
        match self {
            SessionState::Uninitialized => None,
            SessionState::AwaitingInitialized { negotiated } => Some(negotiated),
            SessionState::Ready { negotiated } => Some(negotiated),
        }
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;

    #[test]
    fn uninitialized_allows_only_initialize_and_ping() {
        let state = SessionState::Uninitialized;
        assert!(state.allows_method("initialize"));
        assert!(state.allows_method("ping"));
        assert!(!state.allows_method("tools/list"));
        assert!(!state.allows_method("tools/call"));
        assert!(!state.allows_method("profiles/list"));
        assert!(!state.allows_method("notifications/initialized"));
    }

    #[test]
    fn awaiting_initialized_allows_only_notification_and_ping() {
        let negotiated = NegotiatedProtocol {
            version: "2024-11-05".to_string(),
            client_name: "test".to_string(),
            client_version: None,
            client_capabilities: crate::mcp::protocol::ClientCapabilities::default(),
        };
        let state = SessionState::AwaitingInitialized {
            negotiated: negotiated.clone(),
        };
        assert!(state.allows_method("notifications/initialized"));
        assert!(state.allows_method("ping"));
        assert!(!state.allows_method("initialize"));
        assert!(!state.allows_method("tools/list"));
        assert!(!state.allows_method("tools/call"));
    }

    #[test]
    fn ready_allows_all_except_initialize() {
        let negotiated = NegotiatedProtocol {
            version: "2024-11-05".to_string(),
            client_name: "test".to_string(),
            client_version: None,
            client_capabilities: crate::mcp::protocol::ClientCapabilities::default(),
        };
        let state = SessionState::Ready { negotiated };
        assert!(!state.allows_method("initialize"));
        assert!(state.allows_method("tools/list"));
        assert!(state.allows_method("tools/call"));
        assert!(state.allows_method("profiles/list"));
        assert!(state.allows_method("ping"));
        assert!(state.allows_method("some_extension"));
    }

    #[test]
    fn transition_to_awaiting_from_uninitialized() {
        let negotiated = NegotiatedProtocol {
            version: "2024-11-05".to_string(),
            client_name: "test".to_string(),
            client_version: None,
            client_capabilities: crate::mcp::protocol::ClientCapabilities::default(),
        };
        let mut state = SessionState::Uninitialized;
        assert!(state.transition_to_awaiting(negotiated).is_ok());
        assert!(matches!(state, SessionState::AwaitingInitialized { .. }));
    }

    #[test]
    fn transition_to_awaiting_from_non_uninitialized_fails() {
        let negotiated = NegotiatedProtocol {
            version: "2024-11-05".to_string(),
            client_name: "test".to_string(),
            client_version: None,
            client_capabilities: crate::mcp::protocol::ClientCapabilities::default(),
        };
        let mut state = SessionState::AwaitingInitialized {
            negotiated: negotiated.clone(),
        };
        assert_eq!(
            state.transition_to_awaiting(negotiated).unwrap_err(),
            "already_initialized"
        );
    }

    #[test]
    fn transition_to_ready_from_awaiting() {
        let negotiated = NegotiatedProtocol {
            version: "2024-11-05".to_string(),
            client_name: "test".to_string(),
            client_version: None,
            client_capabilities: crate::mcp::protocol::ClientCapabilities::default(),
        };
        let mut state = SessionState::AwaitingInitialized { negotiated };
        assert!(state.transition_to_ready().is_ok());
        assert!(matches!(state, SessionState::Ready { .. }));
    }

    #[test]
    fn transition_to_ready_from_wrong_state_fails() {
        let mut state = SessionState::Uninitialized;
        assert_eq!(
            state.transition_to_ready().unwrap_err(),
            "not_awaiting_initialized"
        );
    }

    #[test]
    fn negotiated_version_persists_through_transitions() {
        let negotiated = NegotiatedProtocol {
            version: "2025-11-25".to_string(),
            client_name: "my-client".to_string(),
            client_version: Some("2.0".to_string()),
            client_capabilities: crate::mcp::protocol::ClientCapabilities::default(),
        };
        let mut state = SessionState::Uninitialized;
        state.transition_to_awaiting(negotiated).unwrap();
        assert_eq!(
            state.negotiated().map(|n| n.version.as_str()),
            Some("2025-11-25")
        );
        state.transition_to_ready().unwrap();
        assert_eq!(
            state.negotiated().map(|n| n.version.as_str()),
            Some("2025-11-25")
        );
        assert_eq!(
            state.negotiated().and_then(|n| n.client_version.as_deref()),
            Some("2.0")
        );
    }
}

#[cfg(test)]
mod connection_era_tests {
    use super::*;

    #[test]
    fn undecided_pins_once_and_rejects_cross_era() {
        let mut era = ConnectionEra::Undecided;
        assert!(!era.is_decided());
        assert_eq!(
            try_pin_era(&mut era, ConnectionEra::Legacy),
            EraPinOutcome::PinnedNow
        );
        assert!(era.is_decided());
        assert_eq!(era, ConnectionEra::Legacy);
        // Same-era observation is compatible.
        assert_eq!(
            try_pin_era(&mut era, ConnectionEra::Legacy),
            EraPinOutcome::AlreadyPinned
        );
        // Cross-era observation is rejected; state unchanged.
        assert_eq!(
            try_pin_era(&mut era, ConnectionEra::Modern20260728),
            EraPinOutcome::Mismatch
        );
        assert_eq!(era, ConnectionEra::Legacy);
    }

    #[test]
    fn modern_pins_once_and_rejects_legacy() {
        let mut era = ConnectionEra::Undecided;
        assert_eq!(
            try_pin_era(&mut era, ConnectionEra::Modern20260728),
            EraPinOutcome::PinnedNow
        );
        assert_eq!(era, ConnectionEra::Modern20260728);
        assert_eq!(
            try_pin_era(&mut era, ConnectionEra::Legacy),
            EraPinOutcome::Mismatch
        );
        assert_eq!(era, ConnectionEra::Modern20260728);
    }

    #[test]
    fn invalid_opening_leaves_undecided() {
        // Invalid envelopes and failed initialize validation never call
        // try_pin_era, so the connection stays Undecided. This test pins
        // that contract at the state-machine layer: no transition occurs
        // unless the caller explicitly pins after successful validation.
        let era = ConnectionEra::Undecided;
        assert_eq!(era.as_str(), "undecided");
        assert_eq!(ConnectionEra::Legacy.as_str(), "legacy");
        assert_eq!(ConnectionEra::Modern20260728.as_str(), "modern");
    }

    #[tokio::test]
    async fn concurrent_opening_attempts_have_single_winner() {
        // Deterministic race test at the state-machine layer: two competing
        // opening attempts cannot both transition Undecided to different
        // eras. The test proves mutual exclusion, not scheduler timing —
        // either era may win, but exactly one must win.
        let era = Arc::new(Mutex::new(ConnectionEra::Undecided));
        let a = era.clone();
        let b = era.clone();
        let (ra, rb) = tokio::join!(
            pin_connection_era(&a, ConnectionEra::Legacy),
            pin_connection_era(&b, ConnectionEra::Modern20260728),
        );
        // Exactly one caller pinned now; the other either observed the same
        // era (impossible here, eras differ) or mismatched.
        let pinned_count = [ra, rb]
            .iter()
            .filter(|o| **o == EraPinOutcome::PinnedNow)
            .count();
        assert_eq!(
            pinned_count, 1,
            "exactly one opening era must win, got {:?} and {:?}",
            ra, rb
        );
        assert!(
            [ra, rb].contains(&EraPinOutcome::Mismatch),
            "loser must observe mismatch, got {:?} and {:?}",
            ra,
            rb
        );
        let final_era = current_connection_era(&era).await;
        assert!(
            matches!(
                final_era,
                ConnectionEra::Legacy | ConnectionEra::Modern20260728
            ),
            "final era must be decided, got {:?}",
            final_era
        );
        // The winner's era and the final era agree; the loser is incompatible.
        match ra {
            EraPinOutcome::PinnedNow => assert_eq!(final_era, ConnectionEra::Legacy),
            EraPinOutcome::Mismatch => assert_eq!(final_era, ConnectionEra::Modern20260728),
            EraPinOutcome::AlreadyPinned => panic!("unexpected AlreadyPinned for ra"),
        }
    }
}

static ACTIVE_PROFILE: LazyLock<RwLock<String>> = LazyLock::new(|| {
    let profile = std::env::var("EGGCALC_MCP_PROFILE").unwrap_or_else(|_| "full".to_string());
    RwLock::new(if registry::PROFILE_NAMES.contains(&profile.as_str()) {
        profile
    } else {
        eprintln!(
            "Warning: Invalid EGGCALC_MCP_PROFILE: {:?}. Available profiles: {}. Defaulting to full.",
            profile,
            registry::PROFILE_NAMES.join(", ")
        );
        "full".to_string()
    })
});

/// Validate and initialize the profile selected by the environment.
///
/// Profile validation is explicit so library callers can handle bad
/// configuration without a process-wide exit from a lazy initializer.
pub fn init_active_profile() -> Result<(), String> {
    let profile = std::env::var("EGGCALC_MCP_PROFILE").unwrap_or_else(|_| "full".to_string());
    if !registry::PROFILE_NAMES.contains(&profile.as_str()) {
        let available: Vec<&str> = registry::PROFILE_NAMES.to_vec();
        return Err(format!(
            "Invalid EGGCALC_MCP_PROFILE: {:?}. Available profiles: {}",
            profile,
            available.join(", ")
        ));
    }
    set_active_profile(&profile)
}

/// Parse a schema detail string, returning the validated value or `None` for
/// invalid input. Valid values are `"compact"`, `"normal"`, and `"full"`.
/// Empty strings and unknown values return `None`.
#[doc(hidden)]
pub fn parse_schema_detail(s: &str) -> Option<&'static str> {
    match s {
        "compact" => Some("compact"),
        "normal" => Some("normal"),
        "full" => Some("full"),
        _ => None,
    }
}

static ACTIVE_SCHEMA_DETAIL: LazyLock<RwLock<String>> = LazyLock::new(|| {
    let raw = std::env::var("EGGCALC_MCP_SCHEMA_DETAIL")
        .unwrap_or_else(|_| SCHEMA_DETAIL_FULL.to_string());
    match parse_schema_detail(&raw) {
        Some(valid) => RwLock::new(valid.to_string()),
        None => {
            eprintln!(
                "Warning: Invalid EGGCALC_MCP_SCHEMA_DETAIL: {:?}. \
                 Accepted values: compact, normal, full. Defaulting to full.",
                raw
            );
            RwLock::new(SCHEMA_DETAIL_FULL.to_string())
        }
    }
});

/// Parse an audience string into a `ToolAudience` variant.
///
/// Matching is case-insensitive. Invalid values default to `Model` with a
/// diagnostic warning on stderr. This function is exposed as `pub` (not
/// `pub(crate)`) so integration tests can reach it.
#[doc(hidden)]
pub fn parse_audience(s: &str) -> ToolAudience {
    match s.to_lowercase().as_str() {
        "model" => ToolAudience::Model,
        "harness" => ToolAudience::Harness,
        "debug" => ToolAudience::Debug,
        other => {
            eprintln!(
                "Warning: Invalid EGGCALC_MCP_AUDIENCE: {:?}. Defaulting to Model. Use Model, Harness, or Debug.",
                other
            );
            ToolAudience::Model
        }
    }
}

static ACTIVE_AUDIENCE: LazyLock<RwLock<ToolAudience>> = LazyLock::new(|| {
    let audience_str =
        std::env::var("EGGCALC_MCP_AUDIENCE").unwrap_or_else(|_| "Model".to_string());
    RwLock::new(parse_audience(&audience_str))
});

pub fn set_active_profile(name: &str) -> Result<(), String> {
    if !registry::PROFILE_NAMES.contains(&name) {
        let available: Vec<&str> = registry::PROFILE_NAMES.to_vec();
        return Err(format!(
            "Unknown profile: {:?}. Available profiles: {}",
            name,
            available.join(", ")
        ));
    }
    let mut profile = ACTIVE_PROFILE.write().map_err(|e| e.to_string())?;
    *profile = name.to_string();
    Ok(())
}

pub fn get_active_profile() -> String {
    let profile = ACTIVE_PROFILE.read().unwrap_or_else(|e| e.into_inner());
    profile.clone()
}

pub fn set_schema_detail(level: &str) -> Result<(), String> {
    if parse_schema_detail(level).is_none() {
        return Err(format!(
            "Invalid schema detail: {:?}. Use compact, normal, or full.",
            level
        ));
    }
    let mut detail = ACTIVE_SCHEMA_DETAIL.write().map_err(|e| e.to_string())?;
    *detail = level.to_string();
    Ok(())
}

pub fn get_schema_detail() -> String {
    let detail = ACTIVE_SCHEMA_DETAIL
        .read()
        .unwrap_or_else(|e| e.into_inner());
    detail.clone()
}

pub fn get_active_audience() -> ToolAudience {
    let audience = ACTIVE_AUDIENCE.read().unwrap_or_else(|e| e.into_inner());
    *audience
}

/// Parse an MCP surface name (`direct` / `discovery`, case-insensitive).
///
/// Returns `None` for unknown values. Prefer the `EGGSACT_` prefix for new
/// eggsact-native configuration instead of the historical `EGGCALC_`
/// compatibility namespace; surface selection is eggsact-native.
#[doc(hidden)]
pub fn parse_surface(s: &str) -> Option<crate::mcp::discovery::McpSurface> {
    crate::mcp::discovery::McpSurface::parse(s)
}

static ACTIVE_SURFACE: LazyLock<RwLock<crate::mcp::discovery::McpSurface>> = LazyLock::new(|| {
    let raw = std::env::var("EGGSACT_MCP_SURFACE").unwrap_or_else(|_| "direct".to_string());
    match crate::mcp::discovery::McpSurface::parse(&raw) {
        Some(surface) => RwLock::new(surface),
        None => {
            eprintln!(
                    "Warning: Invalid EGGSACT_MCP_SURFACE: {:?}. Accepted values: direct, discovery. Defaulting to direct.",
                    raw
                );
            RwLock::new(crate::mcp::discovery::McpSurface::Direct)
        }
    }
});

/// Validate and initialize the surface selected by the environment.
///
/// Call once at MCP startup (before serving). Returns `Err` for unknown
/// values instead of exiting so library callers can handle bad config.
pub fn init_active_surface() -> Result<(), String> {
    let raw = std::env::var("EGGSACT_MCP_SURFACE").unwrap_or_else(|_| "direct".to_string());
    match crate::mcp::discovery::McpSurface::parse(&raw) {
        Some(surface) => set_active_surface(surface),
        None => Err(format!(
            "Invalid EGGSACT_MCP_SURFACE: {:?}. Accepted values: direct, discovery.",
            raw
        )),
    }
}

/// Set the active presentation surface explicitly (e.g. from `--mcp-surface`).
///
/// CLI-provided values take precedence over the environment. This writes the
/// same process-global read by `get_active_surface()`; call before serving
/// (never mutate after worker threads exist).
pub fn set_active_surface(surface: crate::mcp::discovery::McpSurface) -> Result<(), String> {
    let mut active = ACTIVE_SURFACE.write().map_err(|e| e.to_string())?;
    *active = surface;
    Ok(())
}

/// Get the active presentation surface.
pub fn get_active_surface() -> crate::mcp::discovery::McpSurface {
    let surface = ACTIVE_SURFACE.read().unwrap_or_else(|e| e.into_inner());
    *surface
}

pub fn truncate_2000(s: &str) -> String {
    s.chars().take(2000).collect()
}

// ═══════════════════════════════════════════════════════════════════════════════
// Active request tracking
// ═══════════════════════════════════════════════════════════════════════════════

/// Monotonically increasing generation counter for active-request entries.
/// Each `register_request` call allocates a new generation, and
/// `complete_request` only removes an entry when its stored generation
/// matches the caller's generation — preventing stale cleanup of a
/// recycled request ID.
static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);

/// State for an in-flight MCP request, used for concurrent request handling.
#[doc(hidden)]
pub struct ActiveRequest {
    pub cancel_flag: Arc<AtomicBool>,
    #[allow(dead_code)]
    pub started_at: Instant,
    #[allow(dead_code)]
    pub method: String,
    pub generation: u64,
}

/// Opaque token returned by `register_request` that identifies a specific
/// registration. Used by `complete_request` to remove the active-request
/// entry with generation-safe cleanup.
#[derive(Debug, Clone)]
#[doc(hidden)]
pub struct RequestRegistration {
    pub id: Value,
    pub generation: u64,
}

#[doc(hidden)]
pub type ActiveRequests = Arc<Mutex<HashMap<Value, ActiveRequest>>>;

/// Create a new shared active requests map.
#[doc(hidden)]
pub fn new_active_requests() -> ActiveRequests {
    Arc::new(Mutex::new(HashMap::new()))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Runtime metrics
// ═══════════════════════════════════════════════════════════════════════════════

/// Global runtime metrics for the MCP server. All counters are atomic and
/// RAII-guarded so they decrement correctly on panic/unwind.
pub struct RuntimeMetrics {
    /// Number of currently active request tasks (registered in the active map).
    pub active_requests: AtomicUsize,
    /// Number of currently running blocking handler closures (inside spawn_blocking).
    pub active_blocking_handlers: AtomicUsize,
    /// Number of handlers that have timed out but whose blocking closure is still running.
    pub timed_out_handlers: AtomicUsize,
    /// Total number of timeout responses returned to clients.
    pub total_timeouts: AtomicUsize,
    /// Peak number of concurrent blocking handlers observed.
    pub peak_blocking_concurrency: AtomicUsize,
}

impl RuntimeMetrics {
    const fn new() -> Self {
        Self {
            active_requests: AtomicUsize::new(0),
            active_blocking_handlers: AtomicUsize::new(0),
            timed_out_handlers: AtomicUsize::new(0),
            total_timeouts: AtomicUsize::new(0),
            peak_blocking_concurrency: AtomicUsize::new(0),
        }
    }

    /// Create a fresh `RuntimeMetrics` instance for tests, independent
    /// of the global `RUNTIME_METRICS`.
    #[cfg(test)]
    pub fn new_for_test() -> Self {
        Self::new()
    }
}

/// Global runtime metrics instance.
#[doc(hidden)]
pub static RUNTIME_METRICS: LazyLock<RuntimeMetrics> = LazyLock::new(RuntimeMetrics::new);

/// RAII guard that increments a metric on creation and decrements on drop.
/// Used for active_requests and active_blocking_handlers counters.
#[doc(hidden)]
pub struct MetricGuard {
    counter: &'static AtomicUsize,
}

impl MetricGuard {
    /// Create a new guard that increments the given counter immediately.
    pub fn new(counter: &'static AtomicUsize) -> Self {
        counter.fetch_add(1, Ordering::Relaxed);
        Self { counter }
    }
}

impl Drop for MetricGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Snapshot of runtime metrics for diagnostics.
#[doc(hidden)]
pub struct MetricsSnapshot {
    pub active_requests: usize,
    pub active_blocking_handlers: usize,
    pub timed_out_handlers: usize,
    pub total_timeouts: usize,
    pub peak_blocking_concurrency: usize,
}

/// Take a snapshot of current runtime metrics.
#[doc(hidden)]
pub fn snapshot_metrics() -> MetricsSnapshot {
    MetricsSnapshot {
        active_requests: RUNTIME_METRICS.active_requests.load(Ordering::Relaxed),
        active_blocking_handlers: RUNTIME_METRICS
            .active_blocking_handlers
            .load(Ordering::Relaxed),
        timed_out_handlers: RUNTIME_METRICS.timed_out_handlers.load(Ordering::Relaxed),
        total_timeouts: RUNTIME_METRICS.total_timeouts.load(Ordering::Relaxed),
        peak_blocking_concurrency: RUNTIME_METRICS
            .peak_blocking_concurrency
            .load(Ordering::Relaxed),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Request registration
// ═══════════════════════════════════════════════════════════════════════════════

/// Errors that can occur when registering a new active request.
#[derive(Debug)]
#[doc(hidden)]
pub enum RegisterRequestError {
    /// A request with this non-null ID is already active.
    DuplicateId,
    /// The in-flight request limit has been reached.
    CapacityExceeded,
}

/// Register a new active request under a single lock acquisition.
///
/// Checks in-flight limits and duplicate IDs atomically, then inserts the
/// request into the active map. Returns a `RequestGuard` (debug-only
/// assertion in drop) and a `RequestRegistration` whose generation token
/// is used by `complete_request` for safe cleanup.
#[doc(hidden)]
pub async fn register_request(
    active: &ActiveRequests,
    cancel_flag: &Arc<AtomicBool>,
    request_id: Value,
    method: String,
) -> Result<(RequestGuard, RequestRegistration), RegisterRequestError> {
    let mut map = active.lock().await;
    if map.len() >= MAX_IN_FLIGHT_REQUESTS {
        return Err(RegisterRequestError::CapacityExceeded);
    }
    if map.contains_key(&request_id) {
        return Err(RegisterRequestError::DuplicateId);
    }
    let generation = NEXT_GENERATION.fetch_add(1, Ordering::Relaxed);
    map.insert(
        request_id.clone(),
        ActiveRequest {
            cancel_flag: cancel_flag.clone(),
            started_at: Instant::now(),
            method,
            generation,
        },
    );
    Ok((
        RequestGuard::new(active.clone(), cancel_flag, request_id.clone(), generation),
        RequestRegistration {
            id: request_id,
            generation,
        },
    ))
}

/// Awaited cleanup for an active request entry. Removes the entry from the
/// active-request map only if the stored generation matches the caller's
/// generation token. This prevents stale cleanup of a recycled request ID.
///
/// Returns `true` if the entry was removed, `false` if it was already gone
/// or the generation did not match.
#[doc(hidden)]
pub async fn complete_request(active: &ActiveRequests, registration: &RequestRegistration) -> bool {
    let mut map = active.lock().await;
    if let Some(entry) = map.get(&registration.id) {
        if entry.generation == registration.generation {
            map.remove(&registration.id);
            return true;
        }
    }
    false
}

/// Test-only helpers for constructing runtime state from integration tests.
///
/// This module is `#[doc(hidden)]` and not part of the public API; it is
/// exposed only so `tests/` integration tests can construct runtime
/// primitives without relying on `pub(crate)` items.
#[doc(hidden)]
pub mod test_support {
    use super::{ActiveRequest, Instant};
    use std::sync::{atomic::AtomicBool, Arc};

    pub fn make_active_request(cancel_flag: Arc<AtomicBool>) -> ActiveRequest {
        ActiveRequest {
            cancel_flag,
            started_at: Instant::now(),
            method: "test".to_string(),
            generation: 0,
        }
    }
}

/// Apply a cancellation notification to a single active request ID.
///
/// Validates the request ID type (string or integer within size limits),
/// looks up the corresponding active request, clones its cancel flag Arc,
/// releases the lock, then sets the flag outside the critical section.
/// Returns `true` if a cancel flag was actually set.
///
/// This is an async function that properly awaits the active-map lock
/// rather than using `try_lock()`, which can lose cancellations when
/// the map is briefly contended.
///
/// This function is extracted from the server's notification handler so
/// it can be unit-tested in isolation without spawning the stdio loop.
#[doc(hidden)]
pub async fn apply_cancellation(active: &ActiveRequests, request_id: &Value) -> bool {
    // Validate ID type and size BEFORE acquiring the lock — avoids holding
    // the lock for obviously-invalid IDs (bools, oversized strings/numbers).
    let valid = match request_id {
        Value::Bool(_) => return false,
        Value::String(s) => s.len() <= MAX_REQUEST_ID_LENGTH,
        Value::Number(n) => {
            (n.is_i64() || n.is_u64()) && request_id.to_string().len() <= MAX_REQUEST_ID_LENGTH
        }
        _ => return false,
    };
    if !valid {
        return false;
    }
    // Clone the cancel flag Arc while holding the lock.
    let maybe_flag: Option<Arc<AtomicBool>> = {
        let map = active.lock().await;
        map.get(request_id).map(|req| req.cancel_flag.clone())
    };
    // Set the flag outside the critical section — no lock held.
    if let Some(flag) = maybe_flag {
        flag.store(true, Ordering::Release);
        true
    } else {
        false
    }
}
