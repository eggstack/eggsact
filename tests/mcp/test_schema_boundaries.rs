//! Schema boundary enforcement tests.
//!
//! Verifies that all registered tool schemas only use keywords supported
//! by the schema validator in `src/mcp/schema_validation.rs`.

use eggsact::mcp::registry::all_tools_vec;
use serde_json::Value;

/// Validation keywords that the schema validator supports.
const SUPPORTED_KEYWORDS: &[&str] = &[
    "type",
    "properties",
    "required",
    "additionalProperties",
    "items",
    "minItems",
    "maxItems",
    "uniqueItems",
    "minLength",
    "maxLength",
    "pattern",
    "minimum",
    "maximum",
    "exclusiveMinimum",
    "exclusiveMaximum",
    "multipleOf",
    "enum",
    "const",
];

/// Annotation keywords that are allowed but not enforced.
const ANNOTATION_KEYWORDS: &[&str] = &["description", "title", "default", "examples", "$schema"];

/// Validation keywords that should never appear in registered schemas.
const UNSUPPORTED_KEYWORDS: &[&str] = &[
    "$ref",
    "$defs",
    "definitions",
    "oneOf",
    "anyOf",
    "allOf",
    "not",
    "if",
    "then",
    "else",
    "patternProperties",
    "propertyNames",
    "dependentRequired",
    "dependentSchemas",
    "contains",
    "prefixItems",
    "format",
    "contentEncoding",
    "contentMediaType",
    "minProperties",
    "maxProperties",
];

/// Recursively walk a JSON Schema object and collect unsupported keyword violations.
fn collect_violations(schema: &Value, path: &str, violations: &mut Vec<String>) {
    let obj = match schema.as_object() {
        Some(o) => o,
        None => return,
    };

    for key in obj.keys() {
        if SUPPORTED_KEYWORDS.contains(&key.as_str()) || ANNOTATION_KEYWORDS.contains(&key.as_str())
        {
            continue;
        }
        if UNSUPPORTED_KEYWORDS.contains(&key.as_str()) {
            let full_path = if path.is_empty() {
                key.clone()
            } else {
                format!("{}.{}", path, key)
            };
            violations.push(full_path);
        }
    }

    // Recurse into properties (values only, not keys)
    if let Some(properties) = obj.get("properties").and_then(|p| p.as_object()) {
        for (prop_name, prop_schema) in properties {
            let prop_path = if path.is_empty() {
                format!("properties.{}", prop_name)
            } else {
                format!("{}.properties.{}", path, prop_name)
            };
            collect_violations(prop_schema, &prop_path, violations);
        }
    }

    // Recurse into items
    if let Some(items) = obj.get("items") {
        let items_path = if path.is_empty() {
            "items".to_string()
        } else {
            format!("{}.items", path)
        };
        collect_violations(items, &items_path, violations);
    }

    // Recurse into additionalProperties (if it's an object schema)
    if let Some(additional) = obj.get("additionalProperties") {
        if additional.is_object() {
            let add_path = if path.is_empty() {
                "additionalProperties".to_string()
            } else {
                format!("{}.additionalProperties", path)
            };
            collect_violations(additional, &add_path, violations);
        }
    }
}

#[test]
fn test_all_tool_schemas_use_only_supported_keywords() {
    let tools = all_tools_vec();
    let mut all_violations: Vec<String> = Vec::new();

    for spec in tools {
        let schema = (spec.input_schema)();
        let mut violations = Vec::new();
        collect_violations(&schema, "inputSchema", &mut violations);

        for violation in violations {
            all_violations.push(format!("tool={}, path={}", spec.name, violation));
        }
    }

    assert!(
        all_violations.is_empty(),
        "Found unsupported JSON Schema keywords in tool schemas:\n{}",
        all_violations.join("\n")
    );
}
