use serde_json::Value;

pub fn math_eval_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "expression": {"type": "string", "description": "Math expression to evaluate (e.g., '5 + 3', '30m + 100ft', 'five plus three')", "maxLength": 10000}
        },
        "required": ["expression"]
    })
}

pub fn unit_convert_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "value": {"type": "number", "description": "Numeric value to convert (must be finite; NaN and infinity are rejected)"},
            "from_unit": {"type": "string", "description": "Source unit (e.g., 'km', 'ft', 'kg')"},
            "to_unit": {"type": "string", "description": "Target unit (e.g., 'm', 'in', 'lb')"}
        },
        "required": ["value", "from_unit", "to_unit"]
    })
}

pub fn unit_info_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {"unit": {"type": "string", "description": "Unit name or alias (e.g., 'km', 'kilogram', '℃')"}},
        "required": ["unit"]
    })
}

pub fn constant_lookup_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {"name": {"type": "string", "description": "Constant name (e.g., 'avogadro', 'planck', 'c', 'G')"}},
        "required": ["name"]
    })
}

pub fn math_eval_output() -> Value {
    serde_json::json!({"type":"object","properties":{"value":{"type":"string","description":"Evaluation result as string"},"type":{"type":"string","description":"Python type name of the result"},"unit":{"type":["string","null"],"description":"Unit name (only when result has units)"},"display":{"type":["string","null"],"description":"Human-readable result with units (only when result has units)"}}})
}

pub fn unit_convert_output() -> Value {
    serde_json::json!({"type":"object","properties":{"value":{"type":"number","description":"Converted value"},"from_unit":{"type":"string"},"to_unit":{"type":"string"},"factor":{"type":["number","null"],"description":"Conversion factor used (null for temperature conversions)"}}})
}

pub fn unit_info_output() -> Value {
    serde_json::json!({"type":"object","properties":{"unit":{"type":"string"},"canonical":{"type":"string","description":"Canonical unit name"},"category":{"type":"string","description":"Unit category (e.g., 'length', 'mass', 'temperature')"},"is_valid":{"type":"boolean"}}})
}

pub fn constant_lookup_output() -> Value {
    serde_json::json!({"type":"object","properties":{"name":{"type":"string"},"value":{"type":"number","description":"Constant value"},"symbol":{"type":"string","description":"Display symbol (e.g., 'N_A', 'h', 'c')"},"display_name":{"type":"string","description":"Human-readable name"}}})
}
