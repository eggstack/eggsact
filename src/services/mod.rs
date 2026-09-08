//! Typed deterministic service layer for composite tools.
//!
//! Canonical flow:
//!
//! ```text
//! Typed deterministic core (`crate::text::*`)
//!         |
//!         +--> typed composite/service (this module)
//!         |
//!         +--> `tools/*` JSON adapter -> `ToolResponse`
//!                               |
//!                     MCP / `ToolRegistry`
//! ```
//!
//! Composite implementation code calls these typed functions, not sibling
//! tool adapters. JSON serialization happens only at the outer boundary
//! (`src/tools/*`). These services never touch `ToolResponse`, the MCP
//! registry, profile/audience policy, or JSON-schema validation. Callers
//! that need cooperative cancellation pass a lightweight
//! `should_stop` view instead of an MCP `BudgetContext`.

pub mod fingerprint;
pub mod newline;
pub mod security;

pub use fingerprint::{fingerprint_facts, FingerprintFacts};
pub use newline::{newline_facts, NewlineFacts};
pub use security::{
    inspect_text_security, SecurityFinding, SecurityInspection, SecurityInspectionCancelled,
};
