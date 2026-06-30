//! eggsact - Natural Language Math Calculator with MCP Server
//!
//! A high-performance Rust implementation for parsing and evaluating mathematical
//! expressions in natural language (e.g., "thirty plus five" → 35) with full
//! MCP (Model Context Protocol) server support for AI coding agents.
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

pub mod calc;
pub mod mcp;
pub mod text;
pub mod tools;

// Re-export commonly used functions
pub use calc::evaluate;
pub use calc::run;
pub use calc::split_at_operators;
