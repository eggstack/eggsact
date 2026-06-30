use crate::mcp::schemas::ToolResponse;
use crate::text::{CheckBracketsResult, ValidateJsonResult};
use crate::tools::helpers::*;
use serde_json::Value;

pub fn validate_brackets(args: &Value) -> ToolResponse {
    let text = match _require_str(args, "text", "validate_brackets") {
        Ok(s) => s,
        Err(e) => return *e,
    };
    let pairs_val = args.get("pairs");

    // Build pairs map from args or use defaults
    let pairs: std::collections::HashMap<char, char> = if let Some(pairs_obj) =
        pairs_val.and_then(|v| v.as_object())
    {
        // Validate pairs: max 64 entries, keys/values must be strings, length <= 16
        const MAX_PAIRS: usize = 64;
        const MAX_PAIR_LEN: usize = 16;
        if pairs_obj.len() > MAX_PAIRS {
            return ToolResponse::error(
                "input_too_large",
                &format!(
                    "pairs dict length {} exceeds maximum of 64",
                    pairs_obj.len()
                ),
                None,
                Some("validate_brackets"),
            );
        }
        let mut map = std::collections::HashMap::new();
        for (k, v) in pairs_obj {
            let val_str = match v.as_str() {
                Some(s) => s,
                None => {
                    return ToolResponse::error(
                        "invalid_arguments",
                        &format!(
                            "pairs keys and values must be strings, got String -> {}",
                            json_type_name(v)
                        ),
                        None,
                        Some("validate_brackets"),
                    );
                }
            };
            if k.len() > MAX_PAIR_LEN || val_str.len() > MAX_PAIR_LEN {
                return ToolResponse::error(
                    "invalid_arguments",
                    &format!(
                        "pairs key/value length must be <= 16, got {}/{}",
                        k.len(),
                        val_str.len()
                    ),
                    None,
                    Some("validate_brackets"),
                );
            }
            if k.chars().count() > 1 || val_str.chars().count() > 1 {
                return ToolResponse::error(
                        "invalid_arguments",
                        &format!(
                            "pairs key/value must be single characters, got key '{}' ({} chars) and value '{}' ({} chars)",
                            k, k.chars().count(), val_str, val_str.chars().count()
                        ),
                        None,
                        Some("validate_brackets"),
                    );
            }
            if let (Some(key_ch), Some(val_ch)) = (k.chars().next(), val_str.chars().next()) {
                map.insert(key_ch, val_ch);
            }
        }
        map
    } else if let Some(v) = pairs_val {
        // pairs was provided but is not a dict — return type error like Python
        return ToolResponse::error(
            "invalid_arguments",
            &format!("pairs must be a dict or None, got {}", json_type_name(v)),
            None,
            Some("validate_brackets"),
        );
    } else {
        [('(', ')'), ('[', ']'), ('{', '}'), ('<', '>')]
            .iter()
            .cloned()
            .collect()
    };

    match crate::text::validate_brackets_with_pairs(text, &pairs) {
        Ok(result) => {
            let result: CheckBracketsResult = result;
            ToolResponse::success(
                serde_json::json!({
                    "balanced": result.balanced,
                    "unmatched_openers": result.unmatched_openers,
                    "unmatched_closers": result.unmatched_closers,
                }),
                Some("validate_brackets"),
            )
            .with_tool("validate_brackets")
        }
        Err(e) => ToolResponse::error("invalid_arguments", &e, None, Some("validate_brackets")),
    }
}

pub fn validate_json(args: &Value) -> ToolResponse {
    let text = match _require_str(args, "text", "validate_json") {
        Ok(s) => s,
        Err(e) => return *e,
    };

    match crate::text::validate_json(text) {
        Ok(result) => {
            let result: ValidateJsonResult = result;
            let findings = if !result.valid {
                let error_msg = result
                    .error
                    .clone()
                    .unwrap_or_else(|| "Invalid JSON".to_string());
                let span = if result.line.is_some() || result.column.is_some() {
                    let mut s = serde_json::json!({});
                    if let Some(line) = result.line {
                        s["line"] = serde_json::json!(line);
                    }
                    if let Some(col) = result.column {
                        s["column"] = serde_json::json!(col);
                    }
                    s
                } else {
                    serde_json::Value::Null
                };
                vec![serde_json::json!({
                    "code": "JSON_PARSE_ERROR",
                    "severity": "error",
                    "message": error_msg,
                    "span": span,
                    "details": {"position": result.position},
                })]
            } else {
                vec![]
            };
            let machine_code = if !result.valid {
                Some("JSON_INVALID".to_string())
            } else {
                None
            };
            let mut resp = ToolResponse::success(
                serde_json::json!({
                    "valid": result.valid,
                    "error": result.error,
                    "line": result.line,
                    "column": result.column,
                    "position": result.position,
                    "type": result.json_type,
                    "top_level_keys": result.top_level_keys,
                }),
                Some("validate_json"),
            )
            .with_tool("validate_json");
            if !findings.is_empty() {
                resp = resp.with_findings(findings);
            }
            if let Some(code) = machine_code {
                resp = resp.with_machine_code(&code);
            }
            resp
        }
        Err(e) => ToolResponse::error("invalid_arguments", &e, None, Some("validate_json")),
    }
}

pub fn validate_toml_tool(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'text' parameter",
                None,
                Some("validate_toml"),
            )
        }
    };
    let detail = args
        .get("detail")
        .and_then(|v| v.as_str())
        .unwrap_or("normal");

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!("Text exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("validate_toml"),
        );
    }

    let valid_details = ["summary", "normal", "full"];
    if !valid_details.contains(&detail) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported detail level: {}", detail),
            Some(vec![format!("Use one of: {}", valid_details.join(", "))]),
            Some("validate_toml"),
        );
    }

    match crate::text::toml::validate_toml(text) {
        Ok(result) => {
            if detail == "summary" {
                ToolResponse::success(
                    serde_json::json!({
                        "valid": result.valid,
                        "error": result.error,
                    }),
                    Some("validate_toml"),
                )
                .with_tool("validate_toml")
            } else {
                ToolResponse::success(
                    serde_json::json!({
                        "valid": result.valid,
                        "error": result.error,
                        "line": result.line,
                        "column": result.column,
                        "position": result.position,
                        "type": result.toml_type,
                        "top_level_keys": result.top_level_keys,
                        "tables": result.tables,
                    }),
                    Some("validate_toml"),
                )
                .with_tool("validate_toml")
            }
        }
        Err(e) => ToolResponse::error("invalid_arguments", &e, None, Some("validate_toml")),
    }
}

pub fn validate_schema_light_tool(args: &Value) -> ToolResponse {
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error(
                "invalid_arguments",
                "Missing 'text' parameter",
                None,
                Some("validate_schema_light"),
            )
        }
    };
    let detail = args
        .get("detail")
        .and_then(|v| v.as_str())
        .unwrap_or("normal");
    let valid_details = ["summary", "normal", "full"];
    if !valid_details.contains(&detail) {
        return ToolResponse::error(
            "invalid_arguments",
            &format!("Unsupported detail level: {}", detail),
            Some(vec![format!("Use one of: {}", valid_details.join(", "))]),
            Some("validate_schema_light"),
        );
    }

    let schema_val = args.get("schema");
    let schema = match schema_val.and_then(|v| v.as_object()) {
        Some(o) => o.clone(),
        None => {
            let type_name = match schema_val {
                Some(v) => json_type_name(v),
                None => "NoneType",
            };
            return ToolResponse::error(
                "invalid_arguments",
                &format!("schema must be a dict, got {}", type_name),
                None,
                Some("validate_schema_light"),
            );
        }
    };

    const MAX_SCHEMA_SIZE: usize = 100_000;
    let schema_json =
        serde_json::to_string(&serde_json::Value::Object(schema.clone())).unwrap_or_default();
    if schema_json.len() > MAX_SCHEMA_SIZE {
        return ToolResponse::error(
            "input_too_large",
            &format!(
                "Schema JSON size {} bytes exceeds limit of {} bytes",
                schema_json.len(),
                MAX_SCHEMA_SIZE
            ),
            None,
            Some("validate_schema_light"),
        );
    }

    if text.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error(
            "input_too_large",
            &format!(
                "text length {} exceeds {}",
                text.chars().count(),
                MAX_TEXT_LENGTH
            ),
            None,
            Some("validate_schema_light"),
        );
    }

    let data: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            return ToolResponse::error(
                "invalid_arguments",
                &format!("Invalid JSON: {}", e),
                Some(vec!["Provide valid JSON".to_string()]),
                Some("validate_schema_light"),
            )
        }
    };

    let schema_value = serde_json::Value::Object(schema);
    let mut violations: Vec<serde_json::Value> = Vec::new();
    const MAX_SCHEMA_VIOLATIONS: usize = 100;

    fn get_type_name(value: &serde_json::Value) -> &str {
        match value {
            serde_json::Value::Null => "null",
            serde_json::Value::Bool(_) => "boolean",
            serde_json::Value::Number(n) => {
                if n.is_i64() || n.is_u64() {
                    "integer"
                } else {
                    "number"
                }
            }
            serde_json::Value::String(_) => "string",
            serde_json::Value::Array(_) => "array",
            serde_json::Value::Object(_) => "object",
        }
    }

    fn add_violation(
        violations: &mut Vec<serde_json::Value>,
        path: &str,
        message: &str,
        value_type: Option<&str>,
        expected_type: Option<&str>,
    ) {
        if violations.len() < MAX_SCHEMA_VIOLATIONS {
            violations.push(serde_json::json!({
                "path": path,
                "message": message,
                "value_type": value_type,
                "expected_type": expected_type,
            }));
        }
    }

    fn validate(
        path: &str,
        value: &serde_json::Value,
        schema_def: &serde_json::Value,
        violations: &mut Vec<serde_json::Value>,
        depth: usize,
        elements: &mut usize,
    ) {
        if violations.len() >= MAX_SCHEMA_VIOLATIONS {
            return;
        }
        if depth > MAX_SCHEMA_DEPTH {
            add_violation(violations, path, "schema depth limit exceeded", None, None);
            return;
        }
        *elements += 1;
        if *elements > MAX_SCHEMA_ELEMENTS {
            add_violation(
                violations,
                path,
                "schema element limit exceeded",
                None,
                None,
            );
            return;
        }

        let schema_obj = match schema_def.as_object() {
            Some(o) => o,
            None => return,
        };

        let expected_type = schema_obj.get("type").and_then(|v| v.as_str());

        if let Some(exp_type) = expected_type {
            let actual_type = get_type_name(value);
            let type_matches = match exp_type {
                "object" => value.is_object(),
                "array" => value.is_array(),
                "string" => value.is_string(),
                "number" => value.is_number(),
                "integer" => value.is_i64() || value.is_u64(),
                "boolean" => value.is_boolean(),
                "null" => value.is_null(),
                _ => false,
            };

            if !type_matches {
                let msg = format!("expected {}, got {}", exp_type, actual_type);
                add_violation(violations, path, &msg, Some(actual_type), Some(exp_type));
                return;
            }
        }

        if let (Some(exp_type), serde_json::Value::Object(obj_val)) = (expected_type, value) {
            if exp_type == "object" {
                let required = schema_obj.get("required").and_then(|v| v.as_array());
                if let Some(req_arr) = required {
                    for req_item in req_arr {
                        if let Some(req_key) = req_item.as_str() {
                            if !obj_val.contains_key(req_key) {
                                let full_path = if path.is_empty() {
                                    format!("/{}", req_key)
                                } else {
                                    format!("{}/{}", path, req_key)
                                };
                                add_violation(
                                    violations,
                                    &full_path,
                                    &format!("missing required key '{}'", req_key),
                                    None,
                                    Some("object"),
                                );
                            }
                        }
                    }
                }

                if let Some(add_props) = schema_obj
                    .get("additional_properties")
                    .or_else(|| schema_obj.get("additionalProperties"))
                {
                    if add_props.as_bool() == Some(false) {
                        let props = schema_obj.get("properties").and_then(|v| v.as_object());
                        let allowed_keys: std::collections::HashSet<_> = props
                            .map(|p| p.keys().cloned().collect())
                            .unwrap_or_default();
                        for key in obj_val.keys() {
                            if !allowed_keys.contains(key) {
                                let full_path = if path.is_empty() {
                                    format!("/{}", key)
                                } else {
                                    format!("{}/{}", path, key)
                                };
                                add_violation(
                                    violations,
                                    &full_path,
                                    &format!("additional property '{}' not allowed", key),
                                    Some("string"),
                                    None,
                                );
                            }
                        }
                    }
                }

                if let Some(properties) = schema_obj.get("properties").and_then(|v| v.as_object()) {
                    for (prop_name, prop_schema) in properties {
                        if let Some(prop_value) = obj_val.get(prop_name) {
                            let full_path = if path.is_empty() {
                                format!("/{}", prop_name)
                            } else {
                                format!("{}/{}", path, prop_name)
                            };
                            validate(
                                &full_path,
                                prop_value,
                                prop_schema,
                                violations,
                                depth + 1,
                                elements,
                            );
                        }
                    }
                }
            }
        } else if let (Some(exp_type), serde_json::Value::Array(arr_val)) = (expected_type, value) {
            if exp_type == "array" {
                if let Some(min_items) = schema_obj
                    .get("min_items")
                    .or_else(|| schema_obj.get("minItems"))
                    .and_then(|v| v.as_u64())
                {
                    if (arr_val.len() as u64) < min_items {
                        add_violation(
                            violations,
                            path,
                            &format!(
                                "array has {} items, minimum is {}",
                                arr_val.len(),
                                min_items
                            ),
                            Some("array"),
                            None,
                        );
                    }
                }

                if let Some(max_items) = schema_obj
                    .get("max_items")
                    .or_else(|| schema_obj.get("maxItems"))
                    .and_then(|v| v.as_u64())
                {
                    if arr_val.len() as u64 > max_items {
                        add_violation(
                            violations,
                            path,
                            &format!(
                                "array has {} items, maximum is {}",
                                arr_val.len(),
                                max_items
                            ),
                            Some("array"),
                            None,
                        );
                    }
                }

                if let Some(items_schema) = schema_obj.get("items") {
                    for (i, item) in arr_val.iter().enumerate() {
                        let item_path = format!("{}/[{}]", path, i);
                        validate(
                            &item_path,
                            item,
                            items_schema,
                            violations,
                            depth + 1,
                            elements,
                        );
                    }
                }
            }
        } else if let (Some(exp_type), serde_json::Value::String(str_val)) = (expected_type, value)
        {
            if exp_type == "string" {
                if let Some(min_len) = schema_obj.get("min_length").and_then(|v| v.as_u64()) {
                    if (str_val.chars().count() as u64) < min_len {
                        add_violation(
                            violations,
                            path,
                            &format!(
                                "string has length {}, minimum is {}",
                                str_val.chars().count(),
                                min_len
                            ),
                            Some("string"),
                            None,
                        );
                    }
                }

                if let Some(max_len) = schema_obj.get("max_length").and_then(|v| v.as_u64()) {
                    if (str_val.chars().count() as u64) > max_len {
                        add_violation(
                            violations,
                            path,
                            &format!(
                                "string has length {}, maximum is {}",
                                str_val.chars().count(),
                                max_len
                            ),
                            Some("string"),
                            None,
                        );
                    }
                }

                if let Some(pattern) = schema_obj.get("pattern").and_then(|v| v.as_str()) {
                    if let Ok(re) = regex::Regex::new(pattern) {
                        // Use find with start offset 0 to match Python's re.match behavior
                        // (match at start of string only)
                        let matched = re.find(str_val).map(|m| m.start() == 0).unwrap_or(false);
                        if !matched {
                            let display_val = if str_val.chars().count() > 20 {
                                let truncated: String = str_val.chars().take(20).collect();
                                format!("{}...", truncated)
                            } else {
                                str_val.clone()
                            };
                            add_violation(
                                violations,
                                path,
                                &format!(
                                    "string '{}' does not match pattern '{}'",
                                    display_val, pattern
                                ),
                                Some("string"),
                                None,
                            );
                        }
                    }
                }
            }
        }

        if let Some(enum_values) = schema_obj.get("enum") {
            if let Some(arr) = enum_values.as_array() {
                if !arr.contains(value) {
                    fn fmt_enum_value(v: &serde_json::Value) -> String {
                        match v {
                            serde_json::Value::String(s) => format!("'{}'", s),
                            other => format!("{}", other),
                        }
                    }
                    let value_str = fmt_enum_value(value);
                    let enum_str: Vec<String> = arr.iter().map(fmt_enum_value).collect();
                    add_violation(
                        violations,
                        path,
                        &format!(
                            "value {} is not in enum [{}]",
                            value_str,
                            enum_str.join(", ")
                        ),
                        Some(get_type_name(value)),
                        None,
                    );
                }
            }
        }
    }

    // Pre-check schema depth (matching Python: return error immediately if too deep)
    fn check_schema_depth(o: &serde_json::Value, d: usize) -> Result<usize, String> {
        if d > MAX_SCHEMA_DEPTH {
            return Err("schema too deeply nested".to_string());
        }
        match o {
            serde_json::Value::Object(obj) => {
                if obj.is_empty() {
                    Ok(d)
                } else {
                    obj.values()
                        .map(|v| check_schema_depth(v, d + 1))
                        .max()
                        .unwrap_or(Ok(d))
                }
            }
            serde_json::Value::Array(arr) => {
                if arr.is_empty() {
                    Ok(d)
                } else {
                    arr.iter()
                        .map(|v| check_schema_depth(v, d + 1))
                        .max()
                        .unwrap_or(Ok(d))
                }
            }
            _ => Ok(d),
        }
    }
    if let Err(e) = check_schema_depth(&schema_value, 0) {
        return ToolResponse::error(
            "input_too_large",
            &format!("schema nesting too deep (max {}): {}", MAX_SCHEMA_DEPTH, e),
            None,
            Some("validate_schema_light"),
        );
    }

    validate("", &data, &schema_value, &mut violations, 0, &mut 0usize);

    let truncated = violations.len() >= MAX_SCHEMA_VIOLATIONS;
    let valid = violations.is_empty();

    let summary = if violations.is_empty() {
        "Data is valid".to_string()
    } else if truncated {
        format!(
            "Schema violations detected (truncated, {} shown)",
            violations.len()
        )
    } else {
        let issue = if violations.len() == 1 {
            "issue"
        } else {
            "issues"
        };
        format!("Schema violations detected: {} {}", violations.len(), issue)
    };

    let output = if detail == "summary" {
        serde_json::json!({
            "valid": valid,
            "summary": summary,
        })
    } else {
        serde_json::json!({
            "valid": valid,
            "violations": violations,
            "truncated": truncated,
            "summary": summary,
        })
    };

    ToolResponse::success(output, Some("validate_schema_light")).with_tool("validate_schema_light")
}
