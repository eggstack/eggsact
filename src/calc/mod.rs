//! Calculator module for natural language and mathematical expression evaluation.
//!
//! This module provides the core functionality for:
//! - Parsing natural language math expressions ("thirty plus five")
//! - Evaluating mathematical expressions
//! - Unit conversions
//!
//! # Example
//!
//! ```
//! use eggsact::calc::{run, evaluate};
//!
//! // Natural language — returns (result_string, type_string)
//! assert_eq!(run("thirty plus five").unwrap(), ("35".to_string(), "int".to_string()));
//!
//! // Direct evaluation
//! assert_eq!(evaluate("5 + 3").unwrap(), ("8".to_string(), "int".to_string()));
//! ```

pub mod evaluator;
pub mod normalize;
pub mod units;

pub use evaluator::evaluate;
pub use evaluator::is_mcp_mode;
pub use evaluator::set_mcp_mode;
pub use evaluator::EvaluateResult;
pub use normalize::run;
pub use normalize::split_at_operators;
pub use normalize::RunError;
pub use normalize::RunResult;
