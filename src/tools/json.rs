use crate::mcp::schemas::ToolResponse;
use crate::tools::helpers::*;
use serde::Serialize;
use serde_json::Value;

pub fn json_extract(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'text' parameter",
                None,
                Some("json_extract"),
            )
        }
    };
    let pointer = match args.get("pointer") {
        Some(v) => match v.as_str() {
            Some(s) => s,
            None => {
                return ToolResponse::error(
                    "invalid_arguments",
                    &format!("pointer must be a string, got {}", json_type_name(v)),
                    None,
                    Some("json_extract"),
                )
            }
        },
        None => "",
    };
    let max_output_chars = match args.get("max_output_chars") {
        Some(v) => {
            if let Some(n) = v.as_i64() {
                if n < 0 {
                    return ToolResponse::error(
                        "invalid_arguments",
                        &format!("max_output_chars must be non-negative, got {}", n),
                        None,
                        Some("json_extract"),
                    );
                }
                n as usize
            } else {
                return ToolResponse::error(
                    "invalid_arguments",
                    &format!(
                        "max_output_chars must be a non-negative integer, got {}",
                        json_type_name(v)
                    ),
                    None,
                    Some("json_extract"),
                );
            }
        }
        None => 4000,
    };
    let detail = args
        .get("detail")
        .and_then(|v| v.as_str())
        .unwrap_or("normal");

    if !["summary", "normal", "full"].contains(&detail) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported detail level: {}", detail),
            Some(vec!["Use one of: summary, normal, full".to_string()]),
            Some("json_extract"),
        );
    }

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("json_extract"),
        );
    }

    if pointer.len() > 4096 {
        return ToolResponse::error(
            "input_too_large",
            &format!("pointer length {} exceeds 4096", pointer.len()),
            None,
            Some("json_extract"),
        );
    }

    if max_output_chars > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "invalid_arguments",
            &format!(
                "max_output_chars {} exceeds {}",
                max_output_chars, MAX_TEXT_LENGTH
            ),
            None,
            Some("json_extract"),
        );
    }

    let parsed = match serde_json::from_str::<serde_json::Value>(text) {
        Ok(v) => v,
        Err(e) => {
            return ToolResponse::success(
                serde_json::json!({
                    "valid_json": false,
                    "found": false,
                    "pointer": pointer,
                    "value_type": null,
                    "value": null,
                    "preview": null,
                    "child_keys": null,
                    "array_length": null,
                    "truncated": false,
                    "missing_at": null,
                    "reason": null,
                    "available_keys": null,
                    "error": e.to_string(),
                    "line": e.line(),
                    "column": e.column(),
                    "summary": format!("Invalid JSON: {}", e),
                }),
                Some("json_extract"),
            )
            .with_tool("json_extract");
        }
    };

    if pointer.is_empty() {
        let full_preview = match &parsed {
            serde_json::Value::String(s) => s.clone(),
            other => serde_json::to_string(other).unwrap_or_default(),
        };
        let child_keys = match &parsed {
            serde_json::Value::Object(map) => Some(map.keys().cloned().collect::<Vec<_>>()),
            _ => None,
        };
        let summary = build_extract_summary(&parsed);
        let truncated = full_preview.chars().count() > max_output_chars;
        let preview: String = full_preview.chars().take(max_output_chars).collect();
        let mut result = serde_json::json!({
            "valid_json": true,
            "found": true,
            "pointer": pointer,
            "value": parsed,
            "value_type": get_json_type(&parsed),
            "preview": preview,
            "child_keys": child_keys,
            "array_length": null,
            "truncated": truncated,
            "missing_at": null,
            "reason": null,
            "available_keys": null,
            "error": null,
            "line": null,
            "column": null,
            "summary": summary,
        });
        if let serde_json::Value::Array(ref arr) = parsed {
            result["array_length"] = serde_json::json!(arr.len());
        }
        if detail == "summary" {
            result = serde_json::json!({
                "valid_json": true,
                "found": true,
                "summary": summary,
            });
        }
        return ToolResponse::success(result, Some("json_extract")).with_tool("json_extract");
    }

    let raw_tokens: Vec<&str> = pointer.split('/').collect();
    let tokens: Vec<&str> = if raw_tokens.first() == Some(&"") {
        raw_tokens[1..].to_vec()
    } else {
        raw_tokens
    };
    let mut current = &parsed;

    for token in tokens {
        let decoded = token.replace("~1", "/").replace("~0", "~");
        match current {
            serde_json::Value::Object(map) => match map.get(&decoded) {
                Some(v) => current = v,
                None => {
                    let missing_at = format!("/{}", token);
                    return ToolResponse::success(
                        serde_json::json!({
                            "valid_json": true,
                            "found": false,
                            "pointer": pointer,
                            "value_type": null,
                            "value": null,
                            "preview": null,
                            "child_keys": null,
                            "array_length": null,
                            "truncated": false,
                            "missing_at": missing_at,
                            "reason": "key_not_found",
                            "available_keys": map.keys().collect::<Vec<_>>(),
                            "error": null,
                            "line": null,
                            "column": null,
                            "summary": format!("Key '{}' not found in object at /{}", token, token),
                        }),
                        Some("json_extract"),
                    )
                    .with_tool("json_extract");
                }
            },
            serde_json::Value::Array(arr) => match decoded.parse::<usize>() {
                Ok(idx) if idx < arr.len() => current = &arr[idx],
                _ => {
                    return ToolResponse::success(
                        serde_json::json!({
                            "valid_json": true,
                            "found": false,
                            "pointer": pointer,
                            "value_type": null,
                            "value": null,
                            "preview": null,
                            "child_keys": null,
                            "array_length": arr.len(),
                            "truncated": false,
                            "missing_at": format!("/{}", token),
                            "reason": "index_out_of_range",
                            "available_keys": null,
                            "error": null,
                            "line": null,
                            "column": null,
                            "summary": null,
                        }),
                        Some("json_extract"),
                    )
                    .with_tool("json_extract");
                }
            },
            _ => {
                return ToolResponse::success(
                    serde_json::json!({
                        "valid_json": true,
                        "found": false,
                        "pointer": pointer,
                        "value_type": null,
                        "value": null,
                        "preview": null,
                        "child_keys": null,
                        "array_length": null,
                        "truncated": false,
                        "missing_at": null,
                        "reason": "invalid_pointer_syntax",
                        "available_keys": null,
                        "error": null,
                        "line": null,
                        "column": null,
                        "summary": null,
                    }),
                    Some("json_extract"),
                )
                .with_tool("json_extract");
            }
        }
    }

    let full_preview = match current {
        serde_json::Value::String(s) => s.clone(),
        other => serde_json::to_string(other).unwrap_or_default(),
    };
    let char_count = full_preview.chars().count();
    let truncated = char_count > max_output_chars;
    let preview: String = full_preview.chars().take(max_output_chars).collect();
    let child_keys = match current {
        serde_json::Value::Object(map) => Some(map.keys().cloned().collect::<Vec<_>>()),
        _ => None,
    };
    let summary = build_extract_summary(current);
    let mut result = serde_json::json!({
        "valid_json": true,
        "found": true,
        "pointer": pointer,
        "value": *current,
        "value_type": get_json_type(current),
        "preview": preview,
        "child_keys": child_keys,
        "array_length": null,
        "truncated": truncated,
        "missing_at": null,
        "reason": null,
        "available_keys": null,
        "error": null,
        "line": null,
        "column": null,
        "summary": summary,
    });
    if let serde_json::Value::Array(arr) = current {
        result["array_length"] = serde_json::json!(arr.len());
    }
    if detail == "summary" {
        result = serde_json::json!({
            "valid_json": true,
            "found": true,
            "summary": summary,
        });
    }
    ToolResponse::success(result, Some("json_extract")).with_tool("json_extract")
}

pub fn json_compare(args: &Value) -> ToolResponse {
    let a = match args.get("a").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'a' parameter",
                None,
                Some("json_compare"),
            )
        }
    };
    let b = match args.get("b").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'b' parameter",
                None,
                Some("json_compare"),
            )
        }
    };
    let ignore_object_order = args
        .get("ignore_object_order")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let ignore_array_order = args
        .get("ignore_array_order")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let numeric_string_equivalence = args
        .get("numeric_string_equivalence")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let casefold_keys = args
        .get("casefold_keys")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let treat_missing_null_as_equal = args
        .get("treat_missing_null_as_equal")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let max_diffs = args.get("max_diffs").and_then(|v| v.as_i64()).unwrap_or(50);
    let detail = args
        .get("detail")
        .and_then(|v| v.as_str())
        .unwrap_or("normal");

    if !["summary", "normal", "full"].contains(&detail) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported detail level: {}", detail),
            Some(vec!["Use one of: summary, normal, full".to_string()]),
            Some("json_compare"),
        );
    }

    if max_diffs < 0 {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("max_diffs must be non-negative, got {}", max_diffs),
            None,
            Some("json_compare"),
        );
    }
    if max_diffs > 10_000 {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("max_diffs {} exceeds 10000", max_diffs),
            None,
            Some("json_compare"),
        );
    }
    let max_diffs = max_diffs as usize;

    let parsed_a: Result<serde_json::Value, _> = serde_json::from_str(a);
    let parsed_b: Result<serde_json::Value, _> = serde_json::from_str(b);

    let valid_a = parsed_a.is_ok();
    let valid_b = parsed_b.is_ok();

    if !valid_a || !valid_b {
        let mut diffs: Vec<serde_json::Value> = Vec::new();
        if let Err(ref e) = parsed_a {
            let line = e.line();
            let col = e.column();
            let raw = e.to_string();
            let msg = if let Some(idx) = raw.rfind(" at line ") {
                raw[..idx].to_string()
            } else {
                raw
            };
            diffs.push(serde_json::json!({
                "path": "",
                "kind": "parse_error_a",
                "a_type": Value::Null,
                "b_type": Value::Null,
                "a_preview": format!("Line {}, Col {}: {}", line, col, msg),
                "b_preview": Value::Null,
            }));
        }
        if let Err(ref e) = parsed_b {
            let line = e.line();
            let col = e.column();
            let raw = e.to_string();
            let msg = if let Some(idx) = raw.rfind(" at line ") {
                raw[..idx].to_string()
            } else {
                raw
            };
            diffs.push(serde_json::json!({
                "path": "",
                "kind": "parse_error_b",
                "a_type": Value::Null,
                "b_type": Value::Null,
                "a_preview": Value::Null,
                "b_preview": format!("Line {}, Col {}: {}", line, col, msg),
            }));
        }
        let diff_count = diffs.len();
        return ToolResponse::success(
            serde_json::json!({
                "valid_json_a": valid_a,
                "valid_json_b": valid_b,
                "equal": false,
                "same_type": false,
                "diff_count": diff_count,
                "diffs": diffs,
                "truncated": false,
                "summary": "One or both inputs are not valid JSON",
            }),
            Some("json_compare"),
        )
        .with_tool("json_compare");
    }

    let (parsed_a, parsed_b) = match (parsed_a, parsed_b) {
        (Ok(parsed_a), Ok(parsed_b)) => (parsed_a, parsed_b),
        _ => {
            return ToolResponse::error(
                "parse_error",
                "Invalid JSON input",
                None,
                Some("json_compare"),
            )
        }
    };

    let options = JsonCompareOptions {
        ignore_object_order,
        ignore_array_order,
        numeric_string_equivalence,
        casefold_keys,
        treat_missing_null_as_equal,
        max_diffs,
    };
    let (equal, type_match, diffs) = compare_json_values(&parsed_a, &parsed_b, options);

    let truncated = diffs.len() >= max_diffs;
    let diff_count = diffs.len();
    let summary = if equal {
        "JSON documents are equal".to_string()
    } else {
        format!(
            "JSON documents differ at {} path{}",
            diff_count,
            if diff_count != 1 { "s" } else { "" }
        )
    };

    let diffs_json: Vec<serde_json::Value> = diffs
        .iter()
        .map(|d| {
            serde_json::json!({
                "path": d.path,
                "kind": d.kind,
                "a_type": d.a_type,
                "b_type": d.b_type,
                "a_preview": d.a_preview,
                "b_preview": d.b_preview,
            })
        })
        .collect();

    let result = if detail == "summary" {
        serde_json::json!({
            "valid_json_a": true,
            "valid_json_b": true,
            "equal": equal,
            "same_type": type_match,
            "diff_count": diff_count,
            "summary": summary,
        })
    } else {
        serde_json::json!({
            "valid_json_a": true,
            "valid_json_b": true,
            "equal": equal,
            "same_type": type_match,
            "diff_count": diff_count,
            "diffs": diffs_json,
            "truncated": truncated,
            "summary": summary,
        })
    };

    ToolResponse::success(result, Some("json_compare")).with_tool("json_compare")
}

pub fn json_canonicalize(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'text' parameter",
                None,
                Some("json_canonicalize"),
            )
        }
    };
    let sort_keys = args
        .get("sort_keys")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let indent_raw = args.get("indent").and_then(|v| v.as_i64());
    let indent = match indent_raw {
        Some(v) if v < 0 => {
            return ToolResponse::error(
                "invalid_arguments",
                &format!("indent must be 0-100 or None, got {}", v),
                Some(vec![
                    "Use a value between 0-100 or None for minified".to_string()
                ]),
                Some("json_canonicalize"),
            );
        }
        Some(v) => Some(v as usize),
        None => None,
    };
    let _ensure_ascii = args
        .get("ensure_ascii")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let detect_duplicate_keys = args
        .get("detect_duplicate_keys")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let trailing_newline = args
        .get("trailing_newline")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("json_canonicalize"),
        );
    }

    if let Some(indent_val) = indent {
        if indent_val > 100 {
            return ToolResponse::error(
                "invalid_arguments",
                &format!("indent must be 0-100 or None, got {}", indent_val),
                Some(vec![
                    "Use a value between 0-100 or None for minified".to_string()
                ]),
                Some("json_canonicalize"),
            );
        }
    }

    let mut duplicate_keys: Vec<String> = Vec::new();
    let parsed = match serde_json::from_str::<serde_json::Value>(text) {
        Ok(value) => {
            if detect_duplicate_keys {
                detect_duplicates_in_json(text, &mut duplicate_keys);
            }
            value
        }
        Err(error) => return json_canonicalize_invalid_response(error),
    };

    let (top_level_type, top_level_keys) = match &parsed {
        serde_json::Value::Object(map) => (
            "object".to_string(),
            Some(map.keys().cloned().collect::<Vec<_>>()),
        ),
        serde_json::Value::Array(_) => ("array".to_string(), None),
        other => (get_python_json_type(other).to_string(), None),
    };

    let canonical_data = if sort_keys {
        sort_json_keys(&parsed)
    } else {
        parsed.clone()
    };
    let ensure_ascii = _ensure_ascii;
    let canonical = if let Some(indent_val) = indent {
        let indent_str = " ".repeat(indent_val);
        let formatter = serde_json::ser::PrettyFormatter::with_indent(indent_str.as_bytes());
        let mut buf = Vec::new();
        {
            let mut serializer = serde_json::Serializer::with_formatter(&mut buf, formatter);
            if let Err(e) = canonical_data.serialize(&mut serializer) {
                return ToolResponse::error(
                    "serialization_error",
                    &e.to_string(),
                    None,
                    Some("json_canonicalize"),
                );
            }
        }
        match String::from_utf8(buf) {
            Ok(s) => s,
            Err(e) => {
                return ToolResponse::error(
                    "serialization_error",
                    &format!("invalid UTF-8 output: {}", e),
                    None,
                    Some("json_canonicalize"),
                )
            }
        }
    } else {
        struct PythonStyleFormatter;
        impl serde_json::ser::Formatter for PythonStyleFormatter {
            fn begin_array_value<W: std::io::Write + ?Sized>(
                &mut self,
                writer: &mut W,
                first: bool,
            ) -> std::io::Result<()> {
                if first {
                    Ok(())
                } else {
                    writer.write_all(b", ")
                }
            }
            fn begin_object_key<W: std::io::Write + ?Sized>(
                &mut self,
                writer: &mut W,
                first: bool,
            ) -> std::io::Result<()> {
                if first {
                    Ok(())
                } else {
                    writer.write_all(b", ")
                }
            }
            fn begin_object_value<W: std::io::Write + ?Sized>(
                &mut self,
                writer: &mut W,
            ) -> std::io::Result<()> {
                writer.write_all(b": ")
            }
        }
        let mut buf = Vec::new();
        {
            let mut serializer =
                serde_json::Serializer::with_formatter(&mut buf, PythonStyleFormatter);
            if let Err(e) = canonical_data.serialize(&mut serializer) {
                return ToolResponse::error(
                    "serialization_error",
                    &e.to_string(),
                    None,
                    Some("json_canonicalize"),
                );
            }
        }
        match String::from_utf8(buf) {
            Ok(s) => s,
            Err(e) => {
                return ToolResponse::error(
                    "serialization_error",
                    &format!("invalid UTF-8 output: {}", e),
                    None,
                    Some("json_canonicalize"),
                )
            }
        }
    };
    let canonical = if ensure_ascii {
        escape_ascii(&canonical)
    } else {
        canonical
    };
    let mut canonical_out = canonical.clone();
    if trailing_newline {
        canonical_out.push('\n');
    }

    let minified_raw = if indent.is_some() {
        canonical.clone()
    } else {
        let compact_formatter = serde_json::ser::CompactFormatter;
        let mut buf = Vec::new();
        {
            let mut serializer =
                serde_json::Serializer::with_formatter(&mut buf, compact_formatter);
            if let Err(e) = canonical_data.serialize(&mut serializer) {
                return ToolResponse::error(
                    "serialization_error",
                    &e.to_string(),
                    None,
                    Some("json_canonicalize"),
                );
            }
        }
        match String::from_utf8(buf) {
            Ok(s) => s,
            Err(e) => {
                return ToolResponse::error(
                    "serialization_error",
                    &format!("invalid UTF-8 output: {}", e),
                    None,
                    Some("json_canonicalize"),
                )
            }
        }
    };
    let minified = if ensure_ascii {
        escape_ascii(&minified_raw)
    } else {
        minified_raw
    };

    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(canonical_out.as_bytes());
    let sha256_hash = format!("{:x}", hasher.finalize());

    ToolResponse::success(
        serde_json::json!({
            "valid": true,
            "canonical": canonical_out,
            "minified": minified,
            "sha256": sha256_hash,
            "duplicate_keys": duplicate_keys,
            "top_level_type": top_level_type,
            "top_level_keys": top_level_keys,
            "error": Value::Null,
            "line": Value::Null,
            "column": Value::Null,
        }),
        Some("json_canonicalize"),
    )
    .with_tool("json_canonicalize")
}

pub fn json_query(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'text' parameter",
                None,
                Some("json_query"),
            )
        }
    };
    let pointer = args.get("pointer").and_then(|v| v.as_str()).unwrap_or("");

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("json_query"),
        );
    }

    let parsed = match serde_json::from_str::<serde_json::Value>(text) {
        Ok(v) => v,
        Err(e) => {
            return ToolResponse::success(
                serde_json::json!({
                    "found": false,
                    "pointer": pointer,
                    "reason": "invalid_json",
                    "error": e.to_string(),
                    "line": e.line(),
                    "column": e.column(),
                }),
                Some("json_query"),
            )
            .with_tool("json_query")
            .with_warnings(vec![
                "json_query is deprecated; use json_extract instead".to_string()
            ])
            .with_recommended_next_tool(serde_json::json!("json_extract"));
        }
    };

    if pointer.is_empty() {
        return ToolResponse::success(
            serde_json::json!({
                "found": true,
                "pointer": pointer,
                "value": parsed,
                "type": get_json_type(&parsed),
            }),
            Some("json_query"),
        )
        .with_tool("json_query")
        .with_warnings(vec![
            "json_query is deprecated; use json_extract instead".to_string()
        ])
        .with_recommended_next_tool(serde_json::json!("json_extract"));
    }

    let tokens: Vec<&str> = pointer.split('/').filter(|s| !s.is_empty()).collect();
    let mut current = &parsed;

    for token in tokens {
        let decoded = token.replace("~1", "/").replace("~0", "~");
        match current {
            serde_json::Value::Object(map) => match map.get(&decoded) {
                Some(v) => current = v,
                None => {
                    return ToolResponse::success(
                        serde_json::json!({
                            "found": false,
                            "pointer": pointer,
                            "missing_at": format!("/{}", token),
                            "reason": "key_not_found",
                        }),
                        Some("json_query"),
                    )
                    .with_tool("json_query")
                    .with_warnings(vec![
                        "json_query is deprecated; use json_extract instead".to_string()
                    ])
                    .with_recommended_next_tool(serde_json::json!("json_extract"));
                }
            },
            serde_json::Value::Array(arr) => match decoded.parse::<usize>() {
                Ok(idx) if idx < arr.len() => current = &arr[idx],
                _ => {
                    return ToolResponse::success(
                        serde_json::json!({
                            "found": false,
                            "pointer": pointer,
                            "reason": "index_out_of_range",
                        }),
                        Some("json_query"),
                    )
                    .with_tool("json_query")
                    .with_warnings(vec![
                        "json_query is deprecated; use json_extract instead".to_string()
                    ])
                    .with_recommended_next_tool(serde_json::json!("json_extract"));
                }
            },
            _ => {
                return ToolResponse::success(
                    serde_json::json!({
                        "found": false,
                        "pointer": pointer,
                        "reason": "invalid_pointer_syntax",
                    }),
                    Some("json_query"),
                )
                .with_tool("json_query")
                .with_warnings(vec![
                    "json_query is deprecated; use json_extract instead".to_string()
                ])
                .with_recommended_next_tool(serde_json::json!("json_extract"));
            }
        }
    }

    ToolResponse::success(
        serde_json::json!({
            "found": true,
            "pointer": pointer,
            "value": *current,
            "type": get_json_type(current),
        }),
        Some("json_query"),
    )
    .with_tool("json_query")
    .with_warnings(vec![
        "json_query is deprecated; use json_extract instead".to_string()
    ])
    .with_recommended_next_tool(serde_json::json!("json_extract"))
}

pub fn json_shape_tool(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'text' parameter",
                None,
                Some("json_shape"),
            )
        }
    };
    let max_depth = args.get("max_depth").and_then(|v| v.as_u64()).unwrap_or(4) as usize;
    let max_keys = args.get("max_keys").and_then(|v| v.as_u64()).unwrap_or(100) as usize;
    let max_array_items = args
        .get("max_array_items")
        .and_then(|v| v.as_u64())
        .unwrap_or(5) as usize;

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("json_shape"),
        );
    }

    if max_depth < 1 {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("max_depth must be at least 1, got {}", max_depth),
            Some(vec!["Set max_depth to 1 or higher".to_string()]),
            Some("json_shape"),
        );
    }

    if max_keys < 1 {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("max_keys must be at least 1, got {}", max_keys),
            Some(vec!["Set max_keys to 1 or higher".to_string()]),
            Some("json_shape"),
        );
    }

    if max_array_items < 1 {
        return ToolResponse::error(
            "invalid_arguments",
            &format!(
                "max_array_items must be at least 1, got {}",
                max_array_items
            ),
            Some(vec!["Set max_array_items to 1 or higher".to_string()]),
            Some("json_shape"),
        );
    }

    const MAX_SHAPE_DEPTH: usize = 32;
    const MAX_SHAPE_KEYS: usize = 10_000;
    const MAX_SHAPE_ARRAY_ITEMS: usize = 10_000;

    if max_depth > MAX_SHAPE_DEPTH {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("max_depth {} exceeds {}", max_depth, MAX_SHAPE_DEPTH),
            None,
            Some("json_shape"),
        );
    }

    if max_keys > MAX_SHAPE_KEYS {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("max_keys {} exceeds {}", max_keys, MAX_SHAPE_KEYS),
            None,
            Some("json_shape"),
        );
    }

    if max_array_items > MAX_SHAPE_ARRAY_ITEMS {
        return ToolResponse::error(
            "invalid_arguments",
            &format!(
                "max_array_items {} exceeds {}",
                max_array_items, MAX_SHAPE_ARRAY_ITEMS
            ),
            None,
            Some("json_shape"),
        );
    }

    match crate::text::validate::json_shape(text, max_depth, max_keys, max_array_items) {
        Ok(result) => ToolResponse::success(
            serde_json::json!({
                "valid": result.valid,
                "shape": result.shape,
                "truncated": result.truncated,
                "summary": result.summary,
            }),
            Some("json_shape"),
        )
        .with_tool("json_shape"),
        Err(e) => ToolResponse::error("invalid_arguments", &e, None, Some("json_shape")),
    }
}

pub fn structured_data_compare(args: &Value) -> ToolResponse {
    let a = match args.get("a").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'a' parameter",
                None,
                Some("structured_data_compare"),
            )
        }
    };
    let b = match args.get("b").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'b' parameter",
                None,
                Some("structured_data_compare"),
            )
        }
    };
    let format = args
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("json");
    let ignore_object_order = args
        .get("ignore_object_order")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let ignore_array_order = args
        .get("ignore_array_order")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let max_diffs = args.get("max_diffs").and_then(|v| v.as_u64()).unwrap_or(50) as usize;

    if a.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Input 'a' exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("structured_data_compare"),
        );
    }
    if b.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Input 'b' exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("structured_data_compare"),
        );
    }

    let valid_formats = ["json"];
    if !valid_formats.contains(&format) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("format must be 'json' (got '{}')", format),
            None,
            Some("structured_data_compare"),
        );
    }

    let mut subresults = serde_json::Map::new();
    let mut findings: Vec<serde_json::Value> = Vec::new();

    let vj_a = crate::tools::validate_json(&serde_json::json!({"text": a}));
    let vj_b = crate::tools::validate_json(&serde_json::json!({"text": b}));

    let valid_a = vj_a
        .result
        .as_ref()
        .and_then(|r| r.get("valid"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let valid_b = vj_b
        .result
        .as_ref()
        .and_then(|r| r.get("valid"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let error_a = vj_a
        .result
        .as_ref()
        .and_then(|r| r.get("error"))
        .and_then(|v| v.as_str())
        .unwrap_or("Invalid JSON in a")
        .to_string();
    let error_b = vj_b
        .result
        .as_ref()
        .and_then(|r| r.get("error"))
        .and_then(|v| v.as_str())
        .unwrap_or("Invalid JSON in b")
        .to_string();

    subresults.insert(
        "validate_a".to_string(),
        serde_json::json!({"valid": valid_a}),
    );
    subresults.insert(
        "validate_b".to_string(),
        serde_json::json!({"valid": valid_b}),
    );

    if !valid_a {
        findings.push(serde_json::json!({
            "code": "INVALID_JSON_A",
            "severity": "error",
            "message": error_a,
        }));
    }
    if !valid_b {
        findings.push(serde_json::json!({
            "code": "INVALID_JSON_B",
            "severity": "error",
            "message": error_b,
        }));
    }

    if !valid_a || !valid_b {
        let result = serde_json::json!({
            "equal": false,
            "valid_a": valid_a,
            "valid_b": valid_b,
            "findings": findings,
            "machine_code": "INVALID_INPUT",
            "summary": "One or both inputs are not valid JSON",
        });
        return ToolResponse::success(result, Some("structured_data_compare"))
            .with_tool("structured_data_compare")
            .with_findings(findings)
            .with_machine_code("INVALID_INPUT");
    }

    let equal = if valid_a && valid_b {
        let jc_result = json_compare(&serde_json::json!({
            "a": a,
            "b": b,
            "ignore_object_order": ignore_object_order,
            "ignore_array_order": ignore_array_order,
            "max_diffs": max_diffs,
        }));
        if jc_result.error.is_some() {
            findings.push(serde_json::json!({
                "code": "COMPARE_ERROR",
                "severity": "error",
                "message": jc_result.error.as_deref().unwrap_or("json_compare failed"),
            }));
            false
        } else {
            let jc_equal = jc_result
                .result
                .as_ref()
                .and_then(|r| r.get("equal"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if let Some(jc_res) = &jc_result.result {
                if let Some(diffs) = jc_res.get("diffs").and_then(|d| d.as_array()) {
                    for d in diffs.iter().take(max_diffs) {
                        let path = d.get("path").and_then(|v| v.as_str()).unwrap_or("/");
                        let kind = d.get("kind").and_then(|v| v.as_str()).unwrap_or("unknown");
                        findings.push(serde_json::json!({
                            "code": "VALUE_DIFF",
                            "severity": "info",
                            "message": format!("{}: {}", path, kind),
                        }));
                    }
                }
            }
            let eq = jc_equal;
            subresults.insert(
                "json_compare".to_string(),
                serde_json::json!({
                    "equal": eq,
                    "diff_count": jc_result.result.as_ref()
                        .and_then(|r| r.get("diff_count"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0),
                }),
            );
            eq
        }
    } else {
        false
    };

    let shape_a = json_shape_tool(&serde_json::json!({"text": a}));
    let shape_b = json_shape_tool(&serde_json::json!({"text": b}));
    if let (Some(sa), Some(sb)) = (&shape_a.result, &shape_b.result) {
        let type_a = sa.get("type").and_then(|v| v.as_str());
        let type_b = sb.get("type").and_then(|v| v.as_str());
        if type_a.is_some() && type_b.is_some() && type_a != type_b {
            findings.push(serde_json::json!({
                "code": "TYPE_MISMATCH",
                "severity": "warn",
                "message": format!("Type mismatch: a={}, b={}", type_a.unwrap_or("?"), type_b.unwrap_or("?")),
            }));
        }
        subresults.insert(
            "shape_a".to_string(),
            shape_a.result.unwrap_or(serde_json::Value::Null),
        );
        subresults.insert(
            "shape_b".to_string(),
            shape_b.result.unwrap_or(serde_json::Value::Null),
        );
    }

    let machine_code = if !valid_a || !valid_b {
        "INVALID_INPUT".to_string()
    } else if equal {
        "DATA_EQUAL".to_string()
    } else {
        "DATA_DIFF".to_string()
    };

    let diff_count = findings
        .iter()
        .filter(|f| f.get("code").and_then(|v| v.as_str()).unwrap_or("") == "VALUE_DIFF")
        .count();
    let summary = if equal {
        "Equal".to_string()
    } else {
        format!(
            "Different ({} diff(s), {} finding(s))",
            diff_count,
            findings.len()
        )
    };

    let mut result = serde_json::json!({
        "equal": equal,
        "valid_a": valid_a,
        "valid_b": valid_b,
        "findings": findings,
        "machine_code": machine_code,
        "summary": summary,
    });
    if !subresults.is_empty() {
        result["subresults"] = serde_json::Value::Object(subresults);
    }

    let mut resp = ToolResponse::success(result, Some("structured_data_compare"))
        .with_tool("structured_data_compare");
    resp = resp.with_machine_code(&machine_code);
    if !findings.is_empty() {
        resp = resp.with_findings(findings);
    }
    resp
}
