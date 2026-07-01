use crate::mcp::machine_codes;
use crate::mcp::schemas::ToolResponse;
use crate::tools::helpers::*;
use serde_json::Value;

pub fn unicode_policy_check(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'text' parameter",
                None,
                Some("unicode_policy_check"),
            )
        }
    };
    let policy = match args.get("policy").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'policy' parameter",
                None,
                Some("unicode_policy_check"),
            )
        }
    };
    let normalization = args.get("normalization").and_then(|v| v.as_str());

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("unicode_policy_check"),
        );
    }

    let valid_policies = [
        "identifier_strict",
        "filename_safe",
        "source_code",
        "human_text",
        "json_key",
        "domain_like",
    ];
    if !valid_policies.contains(&policy) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported policy: {}", policy),
            Some(vec![format!("Use one of: {}", valid_policies.join(", "))]),
            Some("unicode_policy_check"),
        );
    }

    if let Some(ref n) = normalization {
        let valid_normalizations = ["raw", "NFC", "NFD", "NFKC", "NFKD"];
        if !valid_normalizations.contains(n) {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                &format!("Unsupported normalization form: {}", n),
                Some(vec![format!(
                    "Use one of: {}",
                    valid_normalizations.join(", ")
                )]),
                Some("unicode_policy_check"),
            );
        }
    }

    let result = crate::text::unicode_policy_check(text, policy, normalization);

    ToolResponse::success(
        serde_json::json!({
            "pass_": result.pass,
            "policy": result.policy,
            "normalized_form": result.normalized_form,
            "findings": result.findings,
            "summary": result.summary,
        }),
        Some("unicode_policy_check"),
    )
    .with_tool("unicode_policy_check")
}

pub fn canonicalize_text(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'text' parameter",
                None,
                Some("canonicalize_text"),
            )
        }
    };
    let profile = match args.get("profile").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'profile' parameter",
                None,
                Some("canonicalize_text"),
            )
        }
    };
    let return_mapping = args
        .get("return_mapping")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("canonicalize_text"),
        );
    }

    let valid_profiles = [
        "source_file_identity",
        "identifier_compare",
        "human_label_compare",
        "json_key_compare",
        "path_segment_compare",
    ];
    if !valid_profiles.contains(&profile) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported profile: {}", profile),
            Some(vec![format!("Use one of: {}", valid_profiles.join(", "))]),
            Some("canonicalize_text"),
        );
    }

    let result = crate::text::canonicalize_text(text, profile, return_mapping);

    ToolResponse::success(
        serde_json::json!({
            "text": result.base.text,
            "changed": result.base.changed,
            "operations_applied": result.base.operations_applied,
            "fingerprint_before": result.base.fingerprint_before,
            "fingerprint_after": result.base.fingerprint_after,
            "findings": result.base.findings,
            "mapping": result.mapping,
        }),
        Some("canonicalize_text"),
    )
    .with_tool("canonicalize_text")
}
