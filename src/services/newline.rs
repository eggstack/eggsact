//! Typed newline facts for composite tools.
//!
//! Computes the composite newline style used by `edit_preflight` without
//! constructing JSON arguments or parsing `ToolResponse` envelopes.

use serde::{Deserialize, Serialize};

use super::fingerprint::fingerprint_facts;

/// Newline style facts shared by composites.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewlineFacts {
    /// Composite style: `"LF"`, `"CRLF"`, `"CR"`, `"mixed"`, or `"none"`.
    pub style: String,
    /// Style detected in the original text.
    pub original_style: String,
    /// Style detected in the replacement text, when available.
    pub replacement_style: Option<String>,
    /// Whether the composite style is mixed.
    pub mixed: bool,
    /// Recommended normalization target for the policy, if any.
    pub recommended_normalization: Option<String>,
    /// Policy that was applied (`check`, `normalize_lf`, `normalize_crlf`, ...).
    pub policy: String,
}

/// Compute newline facts for `original` and an optional replacement text.
///
/// Mirrors the `edit_preflight` newline stage exactly:
/// - `mixed` when the original is `"mixed"`, the replacement is `"mixed"`,
///   or both are concrete non-`none` styles that differ;
/// - otherwise the composite style is the original style;
/// - `recommended_normalization` is `Some("lf")` for `normalize_lf`,
///   `Some("crlf")` for `normalize_crlf`, else `None`.
pub fn newline_facts(original: &str, replacement: Option<&str>, policy: &str) -> NewlineFacts {
    let orig_style = fingerprint_facts(original).newline_style;
    let repl_style = replacement.map(|text| fingerprint_facts(text).newline_style);

    let composite_style = if orig_style == "mixed" {
        "mixed".to_string()
    } else if let Some(ref rs) = repl_style {
        if rs == "mixed" || (orig_style != "none" && *rs != "none" && orig_style != *rs) {
            "mixed".to_string()
        } else {
            orig_style.clone()
        }
    } else {
        orig_style.clone()
    };
    let mixed = composite_style == "mixed";
    let recommended_normalization = match policy {
        "normalize_lf" => Some("lf".to_string()),
        "normalize_crlf" => Some("crlf".to_string()),
        _ => None,
    };

    NewlineFacts {
        style: composite_style,
        original_style: orig_style,
        replacement_style: repl_style,
        mixed,
        recommended_normalization,
        policy: policy.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newline_facts_same_style_not_mixed() {
        let facts = newline_facts("a\nb\n", Some("c\nd\n"), "check");
        assert_eq!(facts.original_style, "LF");
        assert_eq!(facts.replacement_style.as_deref(), Some("LF"));
        assert!(!facts.mixed);
        assert_eq!(facts.style, "LF");
    }

    #[test]
    fn newline_facts_differing_styles_mixed() {
        let facts = newline_facts("a\nb\n", Some("c\r\nd\r\n"), "check");
        assert!(facts.mixed);
        assert_eq!(facts.style, "mixed");
    }

    #[test]
    fn newline_facts_normalize_policy() {
        let facts = newline_facts("a\n", None, "normalize_lf");
        assert_eq!(facts.recommended_normalization.as_deref(), Some("lf"));
        let facts = newline_facts("a\n", None, "normalize_crlf");
        assert_eq!(facts.recommended_normalization.as_deref(), Some("crlf"));
        let facts = newline_facts("a\n", None, "check");
        assert_eq!(facts.recommended_normalization, None);
    }
}
