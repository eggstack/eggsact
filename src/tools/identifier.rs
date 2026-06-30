use crate::mcp::schemas::ToolResponse;
use crate::tools::helpers::*;
use serde_json::Value;

pub fn identifier_analyze(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'text' parameter",
                None,
                Some("identifier_analyze"),
            )
        }
    };
    let languages = args.get("languages").and_then(|v| v.as_array()).map(|arr| {
        arr.iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect::<Vec<_>>()
    });
    let detail = args
        .get("detail")
        .and_then(|v| v.as_str())
        .unwrap_or("normal");

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("identifier_analyze"),
        );
    }

    let valid_details = ["summary", "normal", "full"];
    if !valid_details.contains(&detail) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported detail level: {}", detail),
            Some(vec![format!("Use one of: {}", valid_details.join(", "))]),
            Some("identifier_analyze"),
        );
    }

    if let Some(ref langs) = languages {
        let valid_languages = ["python", "rust", "javascript", "env"];
        for lang in langs {
            if !valid_languages.contains(&lang.as_str()) {
                return ToolResponse::error(
                    "invalid_arguments",
                    &format!("Unsupported language: {}", lang),
                    Some(vec![format!("Use one of: {}", valid_languages.join(", "))]),
                    Some("identifier_analyze"),
                );
            }
        }
    }

    let lang_refs: Option<Vec<&str>> = languages
        .as_ref()
        .map(|v| v.iter().map(|s| s.as_str()).collect());
    let result = crate::text::identifier_analyze(text, lang_refs);

    if detail == "summary" {
        ToolResponse::success(
            serde_json::json!({
                "text": result.text,
                "classification": result.classification,
                "python_valid": result.python_valid,
                "python_keyword": result.python_keyword,
                "env_valid": result.env_valid,
                "summary": result.summary,
            }),
            Some("identifier_analyze"),
        )
        .with_tool("identifier_analyze")
    } else {
        ToolResponse::success(
            serde_json::json!({
                "text": result.text,
                "classification": result.classification,
                "python_valid": result.python_valid,
                "python_keyword": result.python_keyword,
                "rust_valid": result.rust_valid,
                "javascript_valid": result.javascript_valid,
                "env_valid": result.env_valid,
                "suggestions": result.suggestions,
                "warnings": result.warnings,
                "summary": result.summary,
            }),
            Some("identifier_analyze"),
        )
        .with_tool("identifier_analyze")
    }
}

pub fn identifier_inspect(args: &Value) -> ToolResponse {
    let identifiers_val = args.get("identifiers");
    let identifiers = match identifiers_val.and_then(|v| v.as_array()) {
        Some(arr) => {
            for item in arr.iter() {
                if !item.is_string() {
                    return ToolResponse::error(
                        "invalid_arguments",
                        &format!(
                            "Each identifier must be a string, got {}",
                            json_type_name(item)
                        ),
                        None,
                        Some("identifier_inspect"),
                    );
                }
            }
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect::<Vec<_>>()
        }
        None => {
            let type_name = match identifiers_val {
                Some(v) => json_type_name(v),
                None => "NoneType",
            };
            return ToolResponse::error(
                "invalid_arguments",
                &format!("identifiers must be a list, got {}", type_name),
                None,
                Some("identifier_inspect"),
            );
        }
    };
    let language = args
        .get("language")
        .and_then(|v| v.as_str())
        .unwrap_or("generic");
    let normalization = args
        .get("normalization")
        .and_then(|v| v.as_str())
        .unwrap_or("NFC");
    let casefold = args
        .get("casefold")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let check_confusables = args
        .get("check_confusables")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    if identifiers.len() > MAX_LIST_ITEMS {
        return ToolResponse::error(
            "input_too_large",
            &format!(
                "Number of identifiers {} exceeds MAX_LIST_ITEMS {}",
                identifiers.len(),
                MAX_LIST_ITEMS
            ),
            None,
            Some("identifier_inspect"),
        );
    }

    for ident in &identifiers {
        if ident.chars().count() > MAX_TEXT_LENGTH {
            return ToolResponse::error(
                "input_too_large",
                &format!(
                    "Identifier length {} exceeds MAX_TEXT_LENGTH {}",
                    ident.chars().count(),
                    MAX_TEXT_LENGTH
                ),
                Some(vec![format!(
                    "Maximum identifier length is {}",
                    MAX_TEXT_LENGTH
                )]),
                Some("identifier_inspect"),
            );
        }
    }

    let valid_languages = [
        "generic",
        "python",
        "rust",
        "javascript",
        "typescript",
        "json_key",
    ];
    if !valid_languages.contains(&language) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported language: {}", language),
            Some(vec![format!("Use one of: {}", valid_languages.join(", "))]),
            Some("identifier_inspect"),
        );
    }

    let valid_normalizations = ["raw", "NFC", "NFD", "NFKC", "NFKD"];
    if !valid_normalizations.contains(&normalization) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported normalization form: {}", normalization),
            Some(vec![format!(
                "Use one of: {}",
                valid_normalizations.join(", ")
            )]),
            Some("identifier_inspect"),
        );
    }

    let result = crate::text::identifier_inspect(
        &identifiers,
        language,
        normalization,
        casefold,
        check_confusables,
    );

    let has_collisions = !result.collisions.is_empty();
    let has_invalid = result.identifiers.iter().any(|id| !id.valid);

    let mut envelope_findings: Vec<serde_json::Value> = Vec::new();
    for ident_info in &result.identifiers {
        for warning in &ident_info.warnings {
            envelope_findings.push(serde_json::json!({
                "code": "IDENT_WARNING",
                "severity": "warn",
                "message": warning,
                "details": {"identifier": ident_info.raw},
            }));
        }
    }
    for collision in &result.collisions {
        envelope_findings.push(serde_json::json!({
            "code": "IDENT_COLLISION",
            "severity": "warn",
            "message": format!("{}: '{}' collides with '{}'", collision.kind, collision.a, collision.b),
            "details": {"kind": collision.kind, "a": collision.a, "b": collision.b},
        }));
    }

    let mut resp = ToolResponse::success(
        serde_json::json!({
            "identifiers": result.identifiers,
            "collisions": result.collisions,
        }),
        Some("identifier_inspect"),
    )
    .with_tool("identifier_inspect");

    if !envelope_findings.is_empty() {
        resp = resp.with_findings(envelope_findings);
    }
    if has_collisions {
        resp = resp.with_machine_code("IDENT_COLLISIONS");
    } else if has_invalid {
        resp = resp.with_machine_code("IDENT_INVALID");
    }
    resp
}

pub fn identifier_table_inspect(args: &Value) -> ToolResponse {
    let identifiers = match args.get("identifiers").and_then(|v| v.as_array()) {
        Some(arr) => {
            let mut bad_entries: Vec<String> = Vec::new();
            let mut valid_entries: Vec<crate::text::TableIdentifierEntry> = Vec::new();
            for (i, v) in arr.iter().enumerate() {
                match v.as_object() {
                    Some(obj) => match obj.get("name") {
                        Some(name_val) => match name_val.as_str() {
                            Some(name_str) => {
                                if name_str.chars().count() > MAX_TEXT_LENGTH {
                                    bad_entries.push(format!(
                                        "[{}] 'name' length {} exceeds MAX_TEXT_LENGTH {}",
                                        i,
                                        name_str.chars().count(),
                                        MAX_TEXT_LENGTH
                                    ));
                                } else {
                                    if let Some(kind_val) = obj.get("kind") {
                                        if kind_val.as_str().is_none() {
                                            bad_entries.push(format!(
                                                "[{}] 'kind' must be a string, got {}",
                                                i,
                                                json_type_name(kind_val)
                                            ));
                                            continue;
                                        }
                                    }
                                    if let Some(file_val) = obj.get("file") {
                                        if file_val.as_str().is_none() {
                                            bad_entries.push(format!(
                                                "[{}] 'file' must be a string, got {}",
                                                i,
                                                json_type_name(file_val)
                                            ));
                                            continue;
                                        }
                                    }
                                    if let Some(line_val) = obj.get("line") {
                                        if line_val.as_i64().is_none()
                                            || line_val.is_boolean()
                                            || line_val.as_i64().unwrap_or(0) < 0
                                        {
                                            bad_entries.push(format!("[{}] 'line' must be a non-negative integer, got {}", i, json_type_name(line_val)));
                                            continue;
                                        }
                                    }
                                    if let Some(lang_val) = obj.get("language") {
                                        if lang_val.as_str().is_none() {
                                            bad_entries.push(format!(
                                                "[{}] 'language' must be a string, got {}",
                                                i,
                                                json_type_name(lang_val)
                                            ));
                                            continue;
                                        }
                                    }
                                    let kind = obj
                                        .get("kind")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let file = obj
                                        .get("file")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let line = obj
                                        .get("line")
                                        .and_then(|v| v.as_i64())
                                        .unwrap_or(0)
                                        .min(i32::MAX as i64)
                                        as i32;
                                    valid_entries.push(crate::text::TableIdentifierEntry {
                                        name: name_str.to_string(),
                                        kind,
                                        file,
                                        line,
                                    });
                                }
                            }
                            None => {
                                bad_entries.push(format!(
                                    "[{}] 'name' must be a string, got {}",
                                    i,
                                    json_type_name(name_val)
                                ));
                            }
                        },
                        None => {
                            bad_entries.push(format!("[{}] missing required 'name' field", i));
                        }
                    },
                    None => {
                        bad_entries.push(format!("[{}] is {}, not dict", i, json_type_name(v)));
                    }
                }
            }
            if !bad_entries.is_empty() {
                return ToolResponse::error(
                    "invalid_arguments",
                    "Malformed identifier entries",
                    Some(bad_entries.into_iter().take(10).collect()),
                    Some("identifier_table_inspect"),
                );
            }
            valid_entries
        }
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'identifiers' parameter",
                None,
                Some("identifier_table_inspect"),
            )
        }
    };
    let language = args
        .get("language")
        .and_then(|v| v.as_str())
        .unwrap_or("python");
    let valid_checks = [
        "casefold",
        "normalization",
        "confusable",
        "style",
        "reserved",
        "mixed_style",
    ];
    let checks = if let Some(arr) = args.get("checks").and_then(|v| v.as_array()) {
        let invalid: Vec<&str> = arr
            .iter()
            .filter_map(|v| v.as_str())
            .filter(|c| !valid_checks.contains(c))
            .collect();
        if !invalid.is_empty() {
            return ToolResponse::error(
                "invalid_arguments",
                &format!("Unknown check(s): {}", invalid.join(", ")),
                Some(vec![format!("Valid checks: {}", valid_checks.join(", "))]),
                Some("identifier_table_inspect"),
            );
        }
        Some(
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(String::from)
                .collect::<Vec<_>>(),
        )
    } else {
        None
    };

    if identifiers.len() > MAX_LIST_ITEMS {
        return ToolResponse::error(
            "input_too_large",
            &format!(
                "Number of identifiers {} exceeds MAX_LIST_ITEMS",
                identifiers.len()
            ),
            None,
            Some("identifier_table_inspect"),
        );
    }

    let valid_languages = [
        "generic",
        "python",
        "rust",
        "javascript",
        "typescript",
        "json_key",
    ];
    if !valid_languages.contains(&language) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported language: {}", language),
            Some(vec![format!("Use one of: {}", valid_languages.join(", "))]),
            Some("identifier_table_inspect"),
        );
    }

    let checks_ref = checks
        .as_ref()
        .map(|v| v.iter().map(|s| s.as_str()).collect::<Vec<_>>());
    let result = crate::text::identifier_table_inspect(&identifiers, language, checks_ref);

    let has_reserved = !result.reserved_keyword_hits.is_empty();
    let has_collisions = !result.collisions.is_empty();

    let mut envelope_findings: Vec<serde_json::Value> = Vec::new();
    for collision in &result.collisions {
        envelope_findings.push(serde_json::json!({
            "code": format!("COLLISION_{}", collision.kind.to_uppercase()),
            "severity": "warn",
            "message": collision.detail,
            "details": {"names": collision.names},
        }));
    }
    for hit in &result.reserved_keyword_hits {
        let mut details = serde_json::json!({});
        if !hit.file.is_empty() {
            details["file"] = serde_json::json!(hit.file);
        }
        if hit.line > 0 {
            details["line"] = serde_json::json!(hit.line);
        }
        envelope_findings.push(serde_json::json!({
            "code": "RESERVED_KEYWORD",
            "severity": "warn",
            "message": format!("'{}' is a reserved keyword in {}", hit.name, hit.language),
            "details": details,
        }));
    }
    for group in &result.mixed_style_groups {
        envelope_findings.push(serde_json::json!({
            "code": "MIXED_STYLE",
            "severity": "info",
            "message": format!("Mixed styles for '{}': {}", group.stripped, group.styles.join(", ")),
            "details": {"names": group.names},
        }));
    }

    let mut resp = ToolResponse::success(
        serde_json::json!({
            "count": result.count,
            "collisions": result.collisions,
            "reserved_keyword_hits": result.reserved_keyword_hits,
            "mixed_style_groups": result.mixed_style_groups,
            "findings": result.findings,
        }),
        Some("identifier_table_inspect"),
    )
    .with_tool("identifier_table_inspect");

    if !envelope_findings.is_empty() {
        resp = resp.with_findings(envelope_findings);
    }

    if has_reserved {
        resp = resp.with_machine_code("RESERVED_KEYWORDS");
    } else if has_collisions {
        resp = resp.with_machine_code("IDENT_COLLISIONS");
    }
    resp
}
