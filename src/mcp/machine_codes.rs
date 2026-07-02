//! Machine-readable response codes for tool results.
//!
//! Every non-OK `ToolResponse` should include a `machine_code` field using one of
//! these constants. This module is the single source of truth for all machine codes
//! emitted by this crate.
//!
//! Code naming convention: `CATEGORY_SPECIFIC_DETAIL` (UPPER_SNAKE_CASE).
//! Category prefixes group related codes and match the Python `eggcalc` server.
//!
//! # Category-Prefixed Aliases
//!
//! Common error codes have category-prefixed aliases (e.g. `COMMON_INVALID_ARGUMENTS`
//! for `INVALID_ARGUMENTS`). These are wire-compatible: the Rust constant name differs
//! but the string value is identical, so callers can use either name interchangeably.
//! The aliases exist so that codegg and other orchestration layers can reference
//! codes using a consistent `CATEGORY_DETAIL` pattern even for server-level errors.
//!
//! # Design Notes
//!
//! The plan's dotted taxonomy (`edit.safe_to_apply`, etc.) is documented as a
//! forward-looking design in `architecture/machine-codes.md`. The UPPERCASE codes
//! here are the current wire-format constants that match the Python `eggcalc`
//! server for parity compatibility.

/// Success — no findings or errors to report.
pub const OK: &str = "OK";

// ---------------------------------------------------------------------------
// Common error codes (MCP server level)
// ---------------------------------------------------------------------------

/// Request was cancelled before execution.
pub const CANCELLED: &str = "CANCELLED";
/// Tool timed out (exceeded MAX_TOOL_TIMEOUT_SECONDS).
pub const TIMEOUT: &str = "TIMEOUT";
/// Output exceeded MAX_OUTPUT_BYTES and was truncated.
pub const OUTPUT_TOO_LARGE: &str = "OUTPUT_TOO_LARGE";
/// Input text/argument exceeded the size limit for this tool.
pub const INPUT_TOO_LARGE: &str = "INPUT_TOO_LARGE";
/// Failed to serialize the tool response to JSON.
pub const SERIALIZATION_ERROR: &str = "SERIALIZATION_ERROR";
/// The requested operation is not supported.
pub const UNSUPPORTED_FEATURE: &str = "UNSUPPORTED_FEATURE";
/// An internal error occurred (unexpected / unreachable).
pub const INTERNAL_ERROR: &str = "INTERNAL_ERROR";
/// The tool received arguments that do not match its schema.
pub const INVALID_ARGUMENTS: &str = "INVALID_ARGUMENTS";

// ---------------------------------------------------------------------------
// Category-prefixed aliases for common error codes
// ---------------------------------------------------------------------------

pub const COMMON_CANCELLED: &str = "CANCELLED";
pub const COMMON_TIMEOUT: &str = "TIMEOUT";
pub const COMMON_OUTPUT_TOO_LARGE: &str = "OUTPUT_TOO_LARGE";
pub const COMMON_INPUT_TOO_LARGE: &str = "INPUT_TOO_LARGE";
pub const COMMON_INTERNAL_ERROR: &str = "INTERNAL_ERROR";
pub const COMMON_INVALID_ARGUMENTS: &str = "INVALID_ARGUMENTS";

// ---------------------------------------------------------------------------
// Category-specific aliases for codegg routing
// ---------------------------------------------------------------------------

// Edit/Patch category
pub const EDIT_SAFE_TO_APPLY: &str = "EDIT_OK";
pub const EDIT_OLD_TEXT_NOT_FOUND: &str = "AMBIGUOUS_REPLACEMENT";
pub const EDIT_MULTIPLE_MATCHES: &str = "AMBIGUOUS_REPLACEMENT";
pub const EDIT_STALE_CONTEXT: &str = "FINGERPRINT_MISMATCH";

// Shell/Command category
pub const SHELL_SAFE_COMMAND: &str = "COMMAND_OK";
pub const SHELL_DESTRUCTIVE_COMMAND: &str = "SHELL_RISK";
pub const SHELL_NETWORK_ACCESS: &str = "SHELL_RISK";

// Config category
pub const CONFIG_VALID: &str = "CONFIG_OK";
pub const CONFIG_INVALID: &str = "CONFIG_PARSE_FAILED";

// Unicode/Safety category
pub const UNICODE_BIDI_DETECTED: &str = "BIDI_DETECTED";

// Path category
pub const PATH_SCOPE_ESCAPE: &str = "PATH_HAS_TRAVERSAL";

// ---------------------------------------------------------------------------
// Edit / Patch
// ---------------------------------------------------------------------------

/// Edit is safe to apply — unique match, no policy concerns.
pub const EDIT_OK: &str = "EDIT_OK";
/// Edit failed to apply (patch parse error, internal failure, etc.).
pub const EDIT_FAILED: &str = "EDIT_FAILED";
/// old_text matched multiple locations — needs disambiguation.
pub const AMBIGUOUS_REPLACEMENT: &str = "AMBIGUOUS_REPLACEMENT";
/// Patch failed to parse or apply.
pub const PATCH_FAILED: &str = "PATCH_FAILED";
/// Line range is invalid (out of bounds or inverted).
pub const LINE_RANGE_INVALID: &str = "LINE_RANGE_INVALID";
/// Content fingerprint did not match — source may have changed.
pub const FINGERPRINT_MISMATCH: &str = "FINGERPRINT_MISMATCH";
/// Newline style is inconsistent across the file (mixed CRLF/LF).
pub const NEWLINE_INCONSISTENCY: &str = "NEWLINE_INCONSISTENCY";

// ---------------------------------------------------------------------------
// Shell / Command
// ---------------------------------------------------------------------------

/// Command is safe to execute (no risky features detected).
pub const COMMAND_OK: &str = "COMMAND_OK";
/// Command contains features that require review (pipe, redirect, etc.).
pub const SHELL_RISK: &str = "SHELL_RISK";
/// Shell command could not be parsed.
pub const SHELL_PARSE_ERROR: &str = "SHELL_PARSE_ERROR";
/// Regex pattern in the command has safety concerns (ReDoS, etc.).
pub const REGEX_RISK: &str = "REGEX_RISK";

// ---------------------------------------------------------------------------
// JSON
// ---------------------------------------------------------------------------

/// JSON input is valid.
pub const JSON_VALID: &str = "JSON_VALID";
/// JSON input is invalid.
pub const JSON_INVALID: &str = "JSON_INVALID";

// ---------------------------------------------------------------------------
// Structured Data Compare
// ---------------------------------------------------------------------------

/// Compared structures are equal.
pub const DATA_EQUAL: &str = "DATA_EQUAL";
/// Compared structures are different.
pub const DATA_DIFF: &str = "DATA_DIFF";
/// One or both inputs are invalid and cannot be compared.
pub const INVALID_INPUT: &str = "INVALID_INPUT";

// ---------------------------------------------------------------------------
// Path
// ---------------------------------------------------------------------------

/// Path is within expected scope.
pub const PATH_WITHIN_SCOPE: &str = "PATH_WITHIN_SCOPE";
/// Path traverses outside the expected scope.
pub const PATH_HAS_TRAVERSAL: &str = "PATH_HAS_TRAVERSAL";
/// Path points to a hidden file/directory.
pub const PATH_IS_HIDDEN: &str = "PATH_IS_HIDDEN";

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Config file is valid.
pub const CONFIG_OK: &str = "CONFIG_OK";
/// Config file failed to parse.
pub const CONFIG_PARSE_FAILED: &str = "CONFIG_PARSE_FAILED";
/// Config file does not match the expected schema.
pub const CONFIG_SCHEMA_MISMATCH: &str = "CONFIG_SCHEMA_MISMATCH";
/// Config file has warnings but is structurally valid.
pub const CONFIG_HAS_WARNINGS: &str = "CONFIG_HAS_WARNINGS";

// ---------------------------------------------------------------------------
// Identifier / Naming
// ---------------------------------------------------------------------------

/// Naming collisions detected across identifiers.
pub const IDENT_COLLISIONS: &str = "IDENT_COLLISIONS";
/// One or more identifiers are invalid.
pub const IDENT_INVALID: &str = "IDENT_INVALID";
/// One or more identifiers are reserved keywords.
pub const RESERVED_KEYWORDS: &str = "RESERVED_KEYWORDS";
/// Mixed naming styles detected.
pub const IDENT_WARNING: &str = "IDENT_WARNING";

// ---------------------------------------------------------------------------
// Text / Prompt Inspection
// ---------------------------------------------------------------------------

/// Prompt text has hidden or suspicious content.
pub const PROMPT_HIDDEN_CONTENT: &str = "PROMPT_HIDDEN_CONTENT";
/// Prompt text contains suspicious flags or control sequences.
pub const PROMPT_HAS_FLAGS: &str = "PROMPT_HAS_FLAGS";
/// Prompt text may contain an injection attempt.
pub const PROMPT_INJECTION_RISK: &str = "PROMPT_INJECTION_RISK";
/// Identifier collisions were detected during prompt inspection.
pub const IDENTIFIER_COLLISION_RISK: &str = "IDENTIFIER_COLLISION_RISK";

// ---------------------------------------------------------------------------
// Unicode / Safety
// ---------------------------------------------------------------------------

/// Invisible characters detected.
pub const INVISIBLES_DETECTED: &str = "INVISIBLES_DETECTED";
/// Bidi control characters detected.
pub const BIDI_DETECTED: &str = "BIDI_DETECTED";
/// Confusable characters detected.
pub const CONFUSABLES_DETECTED: &str = "CONFUSABLES_DETECTED";
/// Unicode policy violation detected.
pub const UNICODE_RISK: &str = "UNICODE_RISK";
/// Normalization changed the text.
pub const NORMALIZATION_DIFF: &str = "NORMALIZATION_DIFF";
/// Text security inspection passed — no concerns.
pub const TEXT_SECURITY_OK: &str = "TEXT_SECURITY_OK";

// ---------------------------------------------------------------------------
// Regex
// ---------------------------------------------------------------------------

/// Regex pattern is safe.
pub const REGEX_SAFE: &str = "REGEX_SAFE";
/// Regex pattern has safety concerns (catastrophic backtracking, etc.).
pub const REGEX_UNSAFE: &str = "REGEX_UNSAFE";

// ---------------------------------------------------------------------------
// Version / Cargo
// ---------------------------------------------------------------------------

/// Version satisfies the constraint.
pub const CONSTRAINT_NOTE: &str = "CONSTRAINT_NOTE";
/// Version does not satisfy the constraint.
pub const CONSTRAINT_NOT_SATISFIED: &str = "CONSTRAINT_NOT_SATISFIED";
/// Cargo.toml parsed successfully.
pub const CARGO_OK: &str = "CARGO_OK";
/// Cargo.toml failed to parse.
pub const CARGO_PARSE_FAILED: &str = "CARGO_PARSE_FAILED";
/// Cargo.toml has findings (suspicious names, structural issues, etc.).
pub const CARGO_HAS_FINDINGS: &str = "CARGO_HAS_FINDINGS";

// ---------------------------------------------------------------------------
// TOML
// ---------------------------------------------------------------------------

/// TOML input is valid.
pub const TOML_VALID: &str = "TOML_VALID";
/// TOML input is invalid.
pub const TOML_INVALID: &str = "TOML_INVALID";

// ---------------------------------------------------------------------------
// Text comparison / transform
// ---------------------------------------------------------------------------

/// Texts are equal.
pub const TEXT_EQUAL: &str = "TEXT_EQUAL";
/// Texts are not equal.
pub const TEXT_NOT_EQUAL: &str = "TEXT_NOT_EQUAL";

// ---------------------------------------------------------------------------
// Finding severity levels
// ---------------------------------------------------------------------------

pub mod severity {
    //! Standard severity levels for structured findings.

    /// Purely informational; no action required.
    pub const INFO: &str = "info";
    /// Minor concern; safe to act on but worth noting.
    pub const LOW: &str = "low";
    /// Caution required; may need review before acting.
    pub const MEDIUM: &str = "medium";
    /// Significant concern; likely requires investigation.
    pub const HIGH: &str = "high";
    /// Critical issue; do not act without resolving.
    pub const CRITICAL: &str = "critical";
}

// ---------------------------------------------------------------------------
// Finding disposition values
// ---------------------------------------------------------------------------

pub mod disposition {
    //! Disposition categories for structured findings.

    /// Informational only; no blocking behavior.
    pub const INFORMATIONAL: &str = "informational";
    /// Caution — user or model should review before acting.
    pub const CAUTION: &str = "caution";
    /// Blocking — tool result should not be acted upon.
    pub const BLOCKING: &str = "blocking";
}

// ---------------------------------------------------------------------------
// Verdict values (composite tools)
// ---------------------------------------------------------------------------

pub mod verdict {
    //! Verdict constants for composite preflight tools.

    /// Allowed / safe to proceed.
    pub const ALLOW: &str = "allow";
    /// Needs human or model review before proceeding.
    pub const REVIEW: &str = "review";
    /// Blocked — do not proceed.
    pub const BLOCK: &str = "block";

    /// Config is valid.
    pub const VALID: &str = "valid";
    /// Config is valid but has warnings.
    pub const VALID_WITH_WARNINGS: &str = "valid_with_warnings";
    /// Config is invalid.
    pub const INVALID: &str = "invalid";

    /// Safe to apply.
    pub const SAFE_TO_APPLY: &str = "safe_to_apply";
    /// Safe with warnings.
    pub const SAFE_WITH_WARNINGS: &str = "safe_with_warnings";
}

/// All machine code constants, grouped by domain.
/// Useful for programmatic enumeration and testing.
pub const ALL: &[&str] = &[
    OK,
    CANCELLED,
    TIMEOUT,
    OUTPUT_TOO_LARGE,
    INPUT_TOO_LARGE,
    SERIALIZATION_ERROR,
    UNSUPPORTED_FEATURE,
    INTERNAL_ERROR,
    INVALID_ARGUMENTS,
    EDIT_OK,
    EDIT_FAILED,
    AMBIGUOUS_REPLACEMENT,
    PATCH_FAILED,
    LINE_RANGE_INVALID,
    FINGERPRINT_MISMATCH,
    NEWLINE_INCONSISTENCY,
    COMMAND_OK,
    SHELL_RISK,
    SHELL_PARSE_ERROR,
    REGEX_RISK,
    JSON_VALID,
    JSON_INVALID,
    DATA_EQUAL,
    DATA_DIFF,
    INVALID_INPUT,
    PATH_WITHIN_SCOPE,
    PATH_HAS_TRAVERSAL,
    PATH_IS_HIDDEN,
    CONFIG_OK,
    CONFIG_PARSE_FAILED,
    CONFIG_SCHEMA_MISMATCH,
    CONFIG_HAS_WARNINGS,
    IDENT_COLLISIONS,
    IDENT_INVALID,
    RESERVED_KEYWORDS,
    IDENT_WARNING,
    PROMPT_HIDDEN_CONTENT,
    PROMPT_HAS_FLAGS,
    PROMPT_INJECTION_RISK,
    IDENTIFIER_COLLISION_RISK,
    INVISIBLES_DETECTED,
    BIDI_DETECTED,
    CONFUSABLES_DETECTED,
    UNICODE_RISK,
    NORMALIZATION_DIFF,
    TEXT_SECURITY_OK,
    REGEX_SAFE,
    REGEX_UNSAFE,
    CONSTRAINT_NOTE,
    CONSTRAINT_NOT_SATISFIED,
    CARGO_OK,
    CARGO_PARSE_FAILED,
    CARGO_HAS_FINDINGS,
    TOML_VALID,
    TOML_INVALID,
    TEXT_EQUAL,
    TEXT_NOT_EQUAL,
    COMMON_CANCELLED,
    COMMON_TIMEOUT,
    COMMON_OUTPUT_TOO_LARGE,
    COMMON_INPUT_TOO_LARGE,
    COMMON_INTERNAL_ERROR,
    COMMON_INVALID_ARGUMENTS,
    EDIT_SAFE_TO_APPLY,
    EDIT_OLD_TEXT_NOT_FOUND,
    EDIT_MULTIPLE_MATCHES,
    EDIT_STALE_CONTEXT,
    SHELL_SAFE_COMMAND,
    SHELL_DESTRUCTIVE_COMMAND,
    SHELL_NETWORK_ACCESS,
    CONFIG_VALID,
    CONFIG_INVALID,
    UNICODE_BIDI_DETECTED,
    PATH_SCOPE_ESCAPE,
];
