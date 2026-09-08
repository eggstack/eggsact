//! Typed fingerprint facts for composite tools.
//!
//! Thin typed wrapper over [`crate::text::text_fingerprint`] with the
//! `raw`/`raw` defaults used by newline detection and fingerprint
//! verification. Returns only the fields composites need, without any
//! `ToolResponse` or JSON envelope.

use serde::{Deserialize, Serialize};

/// Minimal fingerprint facts shared by composites.
///
/// `sha256` is the hex SHA-256 of the canonical form and `newline_style`
/// is one of `"LF"`, `"CRLF"`, `"CR"`, `"mixed"`, or `"none"`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FingerprintFacts {
    pub sha256: String,
    pub newline_style: String,
}

/// Compute fingerprint facts for `text`.
///
/// Uses `unicode="raw"`, `newline="raw"`, no final-newline trimming and no
/// casefolding — the same inputs `edit_preflight` newline detection uses
/// when it calls `text_fingerprint` with `{"unicode": "raw",
/// "newline": "raw"}`.
pub fn fingerprint_facts(text: &str) -> FingerprintFacts {
    let result = crate::text::text_fingerprint(text, "raw", "raw", false, false);
    FingerprintFacts {
        sha256: result.sha256,
        newline_style: result.newline_style,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_facts_without_json() {
        let facts = fingerprint_facts("hello\nworld\n");
        assert_eq!(facts.newline_style, "LF");
        assert_eq!(facts.sha256.len(), 64);
    }

    #[test]
    fn fingerprint_facts_mixed_newlines() {
        let facts = fingerprint_facts("a\r\nb\n");
        assert_eq!(facts.newline_style, "mixed");
    }

    #[test]
    fn fingerprint_facts_empty() {
        let facts = fingerprint_facts("");
        assert_eq!(facts.newline_style, "none");
        assert_eq!(facts.sha256.len(), 64);
    }
}
