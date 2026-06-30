use crate::mcp::registry;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

static SCHEMA_CACHE: LazyLock<HashMap<String, Value>> = LazyLock::new(|| {
    let tools = registry::mcp_tool_definitions();
    let mut map = HashMap::new();
    for tool in tools {
        map.insert(tool.name, tool.input_schema);
    }
    map
});

pub(crate) fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "NoneType",
        Value::Bool(_) => "bool",
        Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                "int"
            } else {
                "float"
            }
        }
        Value::String(_) => "str",
        Value::Array(_) => "list",
        Value::Object(_) => "dict",
    }
}

pub(crate) fn validate_property(value: &Value, schema: &Value, path: &str) -> Option<String> {
    validate_property_inner(value, schema, path, 10)
}

pub(crate) fn validate_property_inner(
    value: &Value,
    schema: &Value,
    path: &str,
    max_depth: usize,
) -> Option<String> {
    if max_depth == 0 {
        return Some(format!("Schema nesting too deep at '{}'", path));
    }

    let obj = match schema.as_object() {
        Some(o) => o,
        None => return Some(format!("Schema for '{}' must be an object", path)),
    };

    let expected_type = obj.get("type")?;

    let type_options: Vec<&str> = match expected_type {
        Value::String(s) => vec![s.as_str()],
        Value::Array(arr) => arr.iter().filter_map(|v| v.as_str()).collect(),
        _ => {
            return Some(format!(
                "Argument '{}' has unsupported 'type' (must be a string or list of strings)",
                path
            ))
        }
    };

    let valid_type = type_options.iter().any(|t| value_matches_type(value, t));
    if !valid_type {
        if type_options.len() == 1 {
            return Some(format!(
                "Argument '{}' must be {}, got {}",
                path,
                type_options[0],
                json_type_name(value)
            ));
        }
        return Some(format!(
            "Argument '{}' must be one of [{}], got {}",
            path,
            type_options.join(", "),
            json_type_name(value)
        ));
    }

    if type_options
        .iter()
        .all(|t| *t == "integer" || *t == "number")
        && matches!(value, Value::Bool(_))
    {
        if type_options.len() == 1 {
            return Some(format!(
                "Argument '{}' must be {}, got bool",
                path, type_options[0]
            ));
        }
        return Some(format!(
            "Argument '{}' must be one of [{}], got bool",
            path,
            type_options.join(", ")
        ));
    }

    if let Some(const_val) = obj.get("const") {
        if value != const_val {
            return Some(format!(
                "Argument '{}' must equal {}, got {}",
                path, const_val, value
            ));
        }
    }

    if let Some(enums) = obj.get("enum").and_then(|v| v.as_array()) {
        if !enums.iter().any(|e| e == value) {
            return Some(format!(
                "Argument '{}' must be one of: {}",
                path,
                enums
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }

    if type_options.contains(&"string") && value.is_string() {
        if let Some(s) = value.as_str() {
            if let Some(min) = obj.get("minLength").and_then(|v| v.as_u64()) {
                if (s.chars().count() as u64) < min {
                    return Some(format!(
                        "Argument '{}' length {} is less than minLength {}",
                        path,
                        s.chars().count(),
                        min
                    ));
                }
            }
            if let Some(max) = obj.get("maxLength").and_then(|v| v.as_u64()) {
                if (s.chars().count() as u64) > max {
                    return Some(format!(
                        "Argument '{}' length {} exceeds maxLength {}",
                        path,
                        s.chars().count(),
                        max
                    ));
                }
            }
            if let Some(pattern) = obj.get("pattern").and_then(|v| v.as_str()) {
                match regex::Regex::new(pattern) {
                    Ok(re) => {
                        if re.find(s).is_none() {
                            return Some(format!(
                                "Argument '{}' does not match pattern '{}'",
                                path, pattern
                            ));
                        }
                    }
                    Err(e) => {
                        return Some(format!("Argument '{}' has invalid pattern: {}", path, e));
                    }
                }
            }
        }
    }

    let is_numeric = type_options
        .iter()
        .any(|t| *t == "number" || *t == "integer");
    let is_not_bool = !matches!(value, Value::Bool(_));
    if is_numeric && is_not_bool {
        if let Some(n) = value.as_f64() {
            if n.is_nan() {
                return Some(format!(
                    "Argument '{}' must be a finite number, got NaN",
                    path
                ));
            }
            if n.is_infinite() {
                let sign = if n > 0.0 { "+inf" } else { "-inf" };
                return Some(format!(
                    "Argument '{}' must be a finite number, got {}",
                    path, sign
                ));
            }
            if let Some(min) = obj.get("minimum").and_then(|v| v.as_f64()) {
                if n < min {
                    return Some(format!(
                        "Argument '{}' value {} is less than minimum {}",
                        path, n, min
                    ));
                }
            }
            if let Some(max) = obj.get("maximum").and_then(|v| v.as_f64()) {
                if n > max {
                    return Some(format!(
                        "Argument '{}' value {} exceeds maximum {}",
                        path, n, max
                    ));
                }
            }
            if let Some(excl_min) = obj.get("exclusiveMinimum").and_then(|v| v.as_f64()) {
                if n <= excl_min {
                    return Some(format!(
                        "Argument '{}' value {} must be > exclusiveMinimum {}",
                        path, n, excl_min
                    ));
                }
            }
            if let Some(excl_max) = obj.get("exclusiveMaximum").and_then(|v| v.as_f64()) {
                if n >= excl_max {
                    return Some(format!(
                        "Argument '{}' value {} must be < exclusiveMaximum {}",
                        path, n, excl_max
                    ));
                }
            }
            if let Some(multiple_of) = obj.get("multipleOf").and_then(|v| v.as_f64()) {
                if multiple_of > 0.0 {
                    let remainder = n % multiple_of;
                    let abs_check = remainder.abs() < 1e-12;
                    let rel_check = (remainder / multiple_of).abs() < 1e-9;
                    if !abs_check && !rel_check {
                        return Some(format!(
                            "Argument '{}' value {} is not a multiple of {}",
                            path, n, multiple_of
                        ));
                    }
                }
            }
        }
    }

    if type_options.contains(&"object") && value.is_object() {
        let value_obj = match value.as_object() {
            Some(obj) => obj,
            None => return Some(format!("Expected object at '{}'", path)),
        };
        let sub_props = obj.get("properties").and_then(|v| v.as_object());
        let sub_required = obj.get("required").and_then(|v| v.as_array());
        let sub_additional = obj
            .get("additionalProperties")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let has_sub_schema =
            sub_props.is_some_and(|p| !p.is_empty()) || sub_required.is_some_and(|r| !r.is_empty());

        if has_sub_schema {
            if let Some(req) = sub_required {
                for field in req {
                    if let Some(field_name) = field.as_str() {
                        if !value_obj.contains_key(field_name) {
                            return Some(format!(
                                "Missing required field '{}' in '{}'",
                                field_name, path
                            ));
                        }
                    }
                }
            }

            if !sub_additional {
                if let (Some(props), Some(val_obj)) = (sub_props, value.as_object()) {
                    let unknown: Vec<&String> = val_obj
                        .keys()
                        .filter(|k| !props.contains_key(k.as_str()))
                        .collect();
                    if !unknown.is_empty() {
                        return Some(format!(
                            "Unexpected field(s) in '{}': {}",
                            path,
                            unknown
                                .iter()
                                .map(|s| s.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                    }
                }
            }

            if let Some(val_obj) = value.as_object() {
                for (sub_key, sub_val) in val_obj {
                    if let Some(props) = sub_props {
                        if let Some(sub_schema) = props.get(sub_key.as_str()) {
                            let sub_path = format!("{}.{}", path, sub_key);
                            if let Some(err) = validate_property_inner(
                                sub_val,
                                sub_schema,
                                &sub_path,
                                max_depth - 1,
                            ) {
                                return Some(err);
                            }
                        }
                    }
                }
            }
        }
    }

    if type_options.contains(&"array") && value.is_array() {
        if let Some(arr) = value.as_array() {
            if let Some(min) = obj.get("minItems").and_then(|v| v.as_u64()) {
                if (arr.len() as u64) < min {
                    return Some(format!(
                        "Argument '{}' has {} items, less than minItems {}",
                        path,
                        arr.len(),
                        min
                    ));
                }
            }
            if let Some(max) = obj.get("maxItems").and_then(|v| v.as_u64()) {
                if (arr.len() as u64) > max {
                    return Some(format!(
                        "Argument '{}' has {} items, exceeds maxItems {}",
                        path,
                        arr.len(),
                        max
                    ));
                }
            }

            if obj.get("uniqueItems").and_then(|v| v.as_bool()) == Some(true) {
                let mut seen = HashSet::new();
                for item in arr {
                    let s = item.to_string();
                    if !seen.insert(s) {
                        return Some(format!(
                            "Argument '{}' has duplicate items but uniqueItems is True",
                            path
                        ));
                    }
                }
            }

            if let Some(items_schema) = obj.get("items") {
                for (i, item) in arr.iter().enumerate() {
                    let item_path = format!("{}[{}]", path, i);
                    if let Some(err) =
                        validate_property_inner(item, items_schema, &item_path, max_depth - 1)
                    {
                        return Some(err);
                    }
                }
            }
        }
    }

    None
}

fn value_matches_type(value: &Value, t: &str) -> bool {
    match t {
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value.is_i64() || value.is_u64(),
        "boolean" => value.is_boolean(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        "null" => value.is_null(),
        _ => false,
    }
}

pub(crate) fn validate_arguments(name: &str, arguments: &Value) -> Option<String> {
    let schema = SCHEMA_CACHE.get(name)?;

    let obj = arguments.as_object()?;

    let props = schema.get("properties").and_then(|v| v.as_object());
    let required = schema.get("required").and_then(|v| v.as_array());
    let additional = schema
        .get("additionalProperties")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if !additional {
        if let Some(p) = props {
            let mut unknown: Vec<&String> =
                obj.keys().filter(|k| !p.contains_key(k.as_str())).collect();
            unknown.sort();
            if !unknown.is_empty() {
                return Some(format!(
                    "Unexpected argument(s): {}",
                    unknown
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }
    }

    if let Some(req) = required {
        for field in req {
            if let Some(field_name) = field.as_str() {
                if !obj.contains_key(field_name) {
                    return Some(format!("Missing required argument: {}", field_name));
                }
            }
        }
    }

    if let Some(p) = props {
        for (key, value) in obj {
            if let Some(prop_schema) = p.get(key.as_str()) {
                if let Some(err) = validate_property(value, prop_schema, key) {
                    return Some(err);
                }
            }
        }
    }

    None
}
