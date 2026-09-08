//! Typed text-security inspection shared by composites.
//!
//! Implements the `text_security_inspect` pipeline over typed
//! [`crate::text::*`] cores without constructing JSON arguments or parsing
//! `ToolResponse` envelopes. Both the `text_security_inspect` adapter and
//! `edit_preflight` call [`inspect_text_security`]; the adapters only
//! convert the typed result into the existing wire shape at the boundary.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use unicode_normalization::UnicodeNormalization;

/// A single typed security finding.
///
/// Mirrors the `finding(code, severity, message, disposition, None)` shape
/// used by the adapters. Adapters convert via the shared `finding()`
/// helper at the boundary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityFinding {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub disposition: Option<String>,
}

impl SecurityFinding {
    pub fn new(
        code: &str,
        severity: &str,
        message: impl Into<String>,
        disposition: Option<&str>,
    ) -> Self {
        Self {
            code: code.to_string(),
            severity: severity.to_string(),
            message: message.into(),
            disposition: disposition.map(str::to_string),
        }
    }
}

/// Typed result of [`inspect_text_security`].
#[derive(Clone, Debug)]
pub struct SecurityInspection {
    /// Overall verdict: `"allow"`, `"review"`, or `"block"`.
    pub verdict: String,
    /// Primary machine code (`TEXT_SECURITY_OK` when clean).
    pub machine_code: String,
    /// All unique machine codes in first-seen order.
    pub machine_codes: Vec<String>,
    /// Typed findings in pipeline order.
    pub findings: Vec<SecurityFinding>,
    /// Whether `normalize != "none"` changed the text.
    pub normalized_changed: bool,
    /// Human-readable summary (same text as the adapter).
    pub summary: String,
    /// Recommended follow-up action (same text as the adapter).
    pub recommended_action: String,
    /// Diagnostic sub-results for `detail == "normal" | "full"`.
    pub subresults: BTreeMap<String, serde_json::Value>,
}

/// Cancellation signal for [`inspect_text_security`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecurityInspectionCancelled;

/// Inspect `text` for Unicode/prompt/identifier risks.
///
/// `policy` is one of `default`, `source_code`, `prompt`, `markdown`,
/// `identifier`. `normalize` is one of `none`, `NFC`, `NFD`, `NFKC`,
/// `NFKD`. `detail` is one of `summary`, `normal`, `full` and only affects
/// how much diagnostic detail is retained in [`SecurityInspection::subresults`]
/// and the truncation of invisible/confusable lists (10 for `summary`,
/// 100 otherwise — matching `text_inspect`).
///
/// `should_stop` is a lightweight cancellation view. It is checked at the
/// same four pipeline stages where the adapter checks its `BudgetContext`.
/// Returns `Err(SecurityInspectionCancelled)` when cancellation is observed.
pub fn inspect_text_security(
    text: &str,
    policy: &str,
    normalize: &str,
    detail: &str,
    should_stop: &dyn Fn() -> bool,
) -> Result<SecurityInspection, SecurityInspectionCancelled> {
    // Severity/disposition/verdict/machine-code vocab mirrors the adapter so
    // the wire output stays byte-identical.
    const SEV_HIGH: &str = "high";
    const SEV_MEDIUM: &str = "medium";
    const SEV_INFO: &str = "info";
    const DISP_BLOCKING: &str = "blocking";
    const DISP_CAUTION: &str = "caution";
    const DISP_INFORMATIONAL: &str = "informational";
    const VERDICT_ALLOW: &str = "allow";
    const VERDICT_REVIEW: &str = "review";
    const VERDICT_BLOCK: &str = "block";

    let mut subresults: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    let mut all_findings: Vec<SecurityFinding> = Vec::new();
    let mut code_list: Vec<String> = Vec::new();
    let push_code = |list: &mut Vec<String>, code: &str| {
        if !list.iter().any(|c| c == code) {
            list.push(code.to_string());
        }
    };

    // --- Stage 1: text_inspect essentials (warnings / invisibles / confusables) ---
    //
    // The adapter calls the `text_inspect` handler and derives three signals:
    // every `warnings` entry becomes TEXT_INSPECT_WARNING, a non-empty
    // `invisibles` list becomes HIDDEN_CHARS, and a non-empty `confusables`
    // list becomes CONFUSABLES. We compute the same signals directly from
    // the typed cores with identical detail limits and warning text.
    let max_items = if detail == "summary" { 10 } else { 100 };
    let all_invisibles = crate::text::unicode_tools::find_invisibles(text);
    let mut invisibles_json: Vec<serde_json::Value> = Vec::new();
    let mut bidi_json: Vec<serde_json::Value> = Vec::new();
    for inv in &all_invisibles {
        let item = serde_json::json!({
            "index": inv.index,
            "char": inv.char.to_string(),
            "codepoint": inv.codepoint,
            "name": inv.name,
            "category": inv.category,
            "display": inv.display,
        });
        if inv.display.contains("BIDI") {
            bidi_json.push(item);
        } else {
            invisibles_json.push(item);
        }
    }
    let confusables_json: Vec<serde_json::Value> = text
        .chars()
        .enumerate()
        .filter_map(|(i, c)| {
            crate::text::confusables::lookup(c).map(|sub| {
                let name = unicode_names2::name(c)
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "<unknown>".to_string());
                let confusable_chars: String = sub
                    .split_whitespace()
                    .filter_map(|cp| cp.strip_prefix("U+"))
                    .filter_map(|hex| u32::from_str_radix(hex, 16).ok())
                    .filter_map(char::from_u32)
                    .collect();
                let confusable_name: String = confusable_chars
                    .chars()
                    .map(|ch| {
                        unicode_names2::name(ch)
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| ch.to_string())
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                serde_json::json!({
                    "index": i,
                    "char": format!("{c}"),
                    "codepoint": format!("U+{:04X}", c as u32),
                    "name": name,
                    "confusable_with": confusable_chars,
                    "confusable_name": confusable_name,
                })
            })
        })
        .collect();
    let mixed_result = crate::text::unicode_tools::detect_mixed_scripts(text);
    let mixed_scripts = mixed_result.mixed_scripts;
    let scripts = mixed_result.scripts.clone();

    let invisibles_limited: Vec<serde_json::Value> =
        invisibles_json.iter().take(max_items).cloned().collect();
    let confusables_limited: Vec<serde_json::Value> =
        confusables_json.iter().take(max_items).cloned().collect();

    // Warnings mirror text_inspect exactly (invisible, bidi, mixed-script,
    // confusable) so TEXT_INSPECT_WARNING counts stay identical.
    let mut warnings: Vec<serde_json::Value> = Vec::new();
    for inv in &invisibles_limited {
        let name_str = inv.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let idx = inv.get("index").and_then(|v| v.as_u64()).unwrap_or(0);
        warnings.push(serde_json::json!({
            "severity": "warning",
            "kind": "invisible_character",
            "message": format!("Text contains {name_str} at index {idx}"),
            "codepoint": inv.get("codepoint").cloned().unwrap_or(serde_json::Value::Null),
        }));
    }
    for bc in &bidi_json {
        let name_str = bc.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let idx = bc.get("index").and_then(|v| v.as_u64()).unwrap_or(0);
        warnings.push(serde_json::json!({
            "severity": "danger",
            "kind": "bidi_control",
            "message": format!("Text contains bidirectional control character {name_str} at index {idx}"),
            "codepoint": bc.get("codepoint").cloned().unwrap_or(serde_json::Value::Null),
        }));
    }
    if mixed_scripts {
        warnings.push(serde_json::json!({
            "severity": "info",
            "kind": "mixed_scripts",
            "message": format!("Text contains mixed scripts: {}", scripts.join(", ")),
        }));
    }
    for conf in &confusables_limited {
        let char_str = conf.get("char").and_then(|v| v.as_str()).unwrap_or("");
        let confusable_str = conf
            .get("confusable_with")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        warnings.push(serde_json::json!({
            "severity": "warning",
            "kind": "confusable",
            "message": format!("Text contains confusable character '{char_str}' (looks like '{confusable_str}')"),
            "codepoint": conf.get("codepoint").cloned().unwrap_or(serde_json::Value::Null),
        }));
    }

    // Record a text_inspect-shaped subresult for diagnostics. Only the three
    // fields the pipeline reads are needed, but keep the same keys.
    subresults.insert(
        "text_inspect".to_string(),
        serde_json::json!({
            "warnings": warnings,
            "invisibles": invisibles_limited,
            "confusables": confusables_limited,
        }),
    );
    for w in &warnings {
        if !w.is_null() {
            let msg = w
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("text inspection warning");
            all_findings.push(SecurityFinding::new(
                "TEXT_INSPECT_WARNING",
                SEV_MEDIUM,
                msg,
                Some(DISP_CAUTION),
            ));
        }
    }
    if !invisibles_json.is_empty() {
        push_code(&mut code_list, "UNICODE_RISK");
        all_findings.push(SecurityFinding::new(
            "HIDDEN_CHARS",
            SEV_MEDIUM,
            format!("Found {} invisible character(s)", invisibles_json.len()),
            Some(DISP_CAUTION),
        ));
    }
    if !confusables_json.is_empty() {
        push_code(&mut code_list, "UNICODE_RISK");
        all_findings.push(SecurityFinding::new(
            "CONFUSABLES",
            SEV_MEDIUM,
            format!("Found {} confusable character(s)", confusables_json.len()),
            Some(DISP_CAUTION),
        ));
    }

    if should_stop() {
        return Err(SecurityInspectionCancelled);
    }

    // --- Stage 2: unicode_policy_check ---
    let uc_policy = if policy == "source_code" {
        "source_code"
    } else {
        "human_text"
    };
    let uc_result = crate::text::unicode_policy_check(text, uc_policy, None);
    subresults.insert(
        "unicode_policy_check".to_string(),
        serde_json::json!({
            "pass": uc_result.pass,
            "policy": uc_result.policy,
            "normalized_form": uc_result.normalized_form,
            "findings": uc_result.findings,
            "summary": uc_result.summary,
        }),
    );
    for f in &uc_result.findings {
        let raw_sev = f.severity.as_str();
        let (sev, disp) = match raw_sev {
            "error" | "critical" => (SEV_HIGH, Some(DISP_BLOCKING)),
            "warn" | "warning" => (SEV_MEDIUM, Some(DISP_CAUTION)),
            "danger" => (SEV_HIGH, Some(DISP_BLOCKING)),
            "info" => (SEV_INFO, Some(DISP_INFORMATIONAL)),
            other => (other, Some(DISP_INFORMATIONAL)),
        };
        all_findings.push(SecurityFinding::new(&f.rule, sev, &f.message, disp));
        if raw_sev == "error" {
            push_code(&mut code_list, "UNICODE_RISK");
        }
    }

    if should_stop() {
        return Err(SecurityInspectionCancelled);
    }

    // --- Stage 3: normalization check ---
    if normalize != "none" {
        let normalized: String = match normalize {
            "NFC" => text.nfc().collect(),
            "NFD" => text.nfd().collect(),
            "NFKC" => text.nfkc().collect(),
            "NFKD" => text.nfkd().collect(),
            _ => text.to_string(),
        };
        let changed = normalized != text;
        subresults.insert(
            "canonicalize_text".to_string(),
            serde_json::json!({
                "changed": changed,
                "form": normalize,
            }),
        );
        if changed {
            push_code(&mut code_list, "NORMALIZATION_DIFF");
        }
    }

    // --- Stage 4: prompt_input_inspect ---
    if matches!(policy, "prompt" | "markdown" | "default") {
        let pi_result = crate::text::inspect_prompt::prompt_input_inspect(text, None, None);
        subresults.insert(
            "prompt_input_inspect".to_string(),
            serde_json::json!({
                "findings": pi_result.findings,
                "summary": pi_result.summary,
                "risk_score": pi_result.risk_score,
            }),
        );
        for f in &pi_result.findings {
            let code = f
                .get("code")
                .and_then(|v| v.as_str())
                .unwrap_or("PROMPT_RISK");
            let raw_sev = f.get("severity").and_then(|v| v.as_str()).unwrap_or("warn");
            let (sev, disp) = match raw_sev {
                "error" | "critical" => (SEV_HIGH, Some(DISP_BLOCKING)),
                "warn" | "warning" => (SEV_MEDIUM, Some(DISP_CAUTION)),
                "danger" => (SEV_HIGH, Some(DISP_BLOCKING)),
                "info" => (SEV_INFO, Some(DISP_INFORMATIONAL)),
                other => (other, Some(DISP_INFORMATIONAL)),
            };
            let msg = f.get("message").and_then(|v| v.as_str()).unwrap_or("");
            all_findings.push(SecurityFinding::new(code, sev, msg, disp));
        }
        if pi_result.findings.iter().any(|f| {
            let sev = f.get("severity").and_then(|v| v.as_str()).unwrap_or("");
            sev == "warn" || sev == "error"
        }) {
            push_code(&mut code_list, "PROMPT_INJECTION_RISK");
        }
    }

    if should_stop() {
        return Err(SecurityInspectionCancelled);
    }

    // --- Stage 5: identifier_inspect ---
    if matches!(policy, "identifier" | "default") {
        let words: Vec<String> = text
            .split_whitespace()
            .filter(|w| !w.is_empty())
            .filter(|w| {
                let chars: Vec<char> = w.chars().collect();
                if chars.is_empty() {
                    return false;
                }
                let first = chars[0];
                if first != '_' && !first.is_alphabetic() {
                    return false;
                }
                chars[1..].iter().all(|c| c.is_alphanumeric() || *c == '_')
            })
            .map(|w| w.to_string())
            .collect();
        if !words.is_empty() {
            let id_result = crate::text::identifier_inspect(&words, "generic", "NFC", false, true);
            subresults.insert(
                "identifier_inspect".to_string(),
                serde_json::json!({
                    "identifiers": id_result.identifiers,
                    "collisions": id_result.collisions,
                }),
            );
            // Mirror the identifier_inspect adapter's envelope findings
            // (IDENT_WARNING / IDENT_COLLISION, both severity "warn") and the
            // security adapter's mapping (warn -> medium/caution).
            let mut id_findings: Vec<SecurityFinding> = Vec::new();
            for ident_info in &id_result.identifiers {
                for warning in &ident_info.warnings {
                    id_findings.push(SecurityFinding::new(
                        "IDENT_WARNING",
                        SEV_MEDIUM,
                        warning,
                        Some(DISP_CAUTION),
                    ));
                }
            }
            for collision in &id_result.collisions {
                let msg = format!(
                    "{}: '{}' collides with '{}'",
                    collision.kind, collision.a, collision.b
                );
                id_findings.push(SecurityFinding::new(
                    "IDENT_COLLISION",
                    SEV_MEDIUM,
                    msg,
                    Some(DISP_CAUTION),
                ));
            }
            if !id_findings.is_empty() {
                all_findings.extend(id_findings);
                push_code(&mut code_list, "IDENTIFIER_COLLISION_RISK");
            }
        }
    }

    if should_stop() {
        return Err(SecurityInspectionCancelled);
    }

    // --- Verdict + machine codes ---
    let has_error = all_findings.iter().any(|f| f.severity == SEV_HIGH);
    let has_warn = all_findings.iter().any(|f| f.severity == SEV_MEDIUM);
    let verdict = if has_error {
        VERDICT_BLOCK
    } else if has_warn {
        VERDICT_REVIEW
    } else {
        VERDICT_ALLOW
    }
    .to_string();

    // Identifier findings from the adapter path use per-finding codes; ensure
    // the envelope code matches the adapter's IDENTIFIER_COLLISION_RISK even
    // when collisions were reported under a different per-finding code.
    // (The mapping above already pushes the envelope code.)

    let primary_machine_code = if code_list.is_empty() {
        "TEXT_SECURITY_OK".to_string()
    } else {
        code_list[0].clone()
    };

    let n_findings = all_findings.len();
    let summary = if verdict == VERDICT_ALLOW {
        format!("No security issues found ({n_findings} findings).")
    } else if verdict == VERDICT_REVIEW {
        format!("Review recommended: {n_findings} finding(s) require attention.")
    } else {
        format!("Block: {n_findings} finding(s) indicate security risk.")
    };
    let recommended_action = if verdict == VERDICT_ALLOW {
        "allow".to_string()
    } else if verdict == VERDICT_REVIEW {
        "review content for hidden instructions".to_string()
    } else {
        "do not trust this text without manual inspection".to_string()
    };
    let normalized_changed = subresults
        .get("canonicalize_text")
        .and_then(|v| v.get("changed"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    Ok(SecurityInspection {
        verdict,
        machine_code: primary_machine_code,
        machine_codes: code_list,
        findings: all_findings,
        normalized_changed,
        summary,
        recommended_action,
        subresults,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_cancel() -> impl Fn() -> bool {
        || false
    }

    #[test]
    fn clean_text_allows_without_json() {
        let inspection =
            inspect_text_security("Hello, world!", "default", "none", "summary", &no_cancel())
                .unwrap();
        assert_eq!(inspection.verdict, "allow");
        assert_eq!(inspection.machine_code, "TEXT_SECURITY_OK");
        assert!(inspection.findings.is_empty());
    }

    #[test]
    fn invisible_chars_review_without_json() {
        let inspection = inspect_text_security(
            "hi\u{200b}there",
            "default",
            "none",
            "summary",
            &no_cancel(),
        )
        .unwrap();
        assert!(inspection.verdict == "review" || inspection.verdict == "block");
        assert!(inspection
            .machine_codes
            .contains(&"UNICODE_RISK".to_string()));
        assert!(!inspection.findings.is_empty());
    }

    #[test]
    fn cancellation_is_observed() {
        let result = inspect_text_security("hello", "default", "none", "summary", &|| true);
        assert!(matches!(result, Err(SecurityInspectionCancelled)));
    }
}
