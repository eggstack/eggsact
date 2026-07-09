//! eggsact - Deterministic MCP and In-Process Utility Tools for Coding Agents
//!
//! A high-performance Rust crate providing 80 deterministic tools across 20 categories
//! for AI coding agents: math evaluation, text processing, JSON analysis, regex validation,
//! path operations, Unicode safety, shell command preflight, config inspection, and more.
//! Includes a full MCP (Model Context Protocol) server and an in-process agent API.
//!
//! # Quick Start
//!
//! ```
//! use eggsact::{run, evaluate};
//!
//! // Natural language math — returns (result, type)
//! let (result, _typ) = run("thirty plus five").unwrap();
//! assert_eq!(result, "35");
//!
//! // Direct math evaluation
//! let (result, _typ) = evaluate("2 ** 10").unwrap();
//! assert_eq!(result, "1024");
//! ```
//!
//! # In-Process Agent API
//!
//! Call tools directly without starting an MCP server:
//!
//! ```
//! use eggsact::agent::{ToolRegistry, Profile};
//!
//! let registry = ToolRegistry::default();
//! let response = registry.call_json("text_equal", serde_json::json!({
//!     "a": "hello",
//!     "b": "hello",
//! })).unwrap();
//! assert!(response.ok);
//! ```
//!
//! # Typed Preflight
//!
//! Use typed wrappers for common workflows:
//!
//! ```
//! use eggsact::preflight::{ConfigPreflight, ConfigPreflightInput, ConfigFormat};
//!
//! let input = ConfigPreflightInput {
//!     text: r#"{"key": "value"}"#.to_string(),
//!     format: ConfigFormat::Json,
//!     schema: None,
//!     strict: false,
//! };
//! let output = ConfigPreflight::run(&input).unwrap();
//! assert!(output.valid);
//! assert!(!output.machine_code.is_empty());
//! ```
//!
//! Wrappers return `Result<Output, PreflightError>`. Missing mandatory fields
//! produce `ContractViolation` errors instead of silently defaulting.
//!
//! # MCP Server
//!
//! Run as an MCP server via stdio:
//!
//! ```bash
//! eggsact --mcp
//! ```
//!
//! The server accepts JSON-RPC 2.0 requests and provides MCP tools for math,
//! text processing, structured data, paths, Unicode safety, and more.

pub mod agent;
pub mod calc;
pub mod mcp;
pub mod preflight;
pub mod text;
pub mod tools;

// Re-export commonly used functions
pub use calc::evaluate;
pub use calc::evaluate_with_context;
pub use calc::run;
pub use calc::run_with_context;
pub use calc::split_at_operators;
pub use calc::EvalContext;
