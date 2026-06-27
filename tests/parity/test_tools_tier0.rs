use crate::parity::compare_tool_parity;

#[test]
fn test_unit_convert_basic() {
    let args = serde_json::json!({"value": 1.0, "from_unit": "km", "to_unit": "m"});
    let result = compare_tool_parity("unit_convert", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_unit_convert_mixture() {
    let args = serde_json::json!({"value": 1.0, "from_unit": "mi", "to_unit": "km"});
    let result = compare_tool_parity("unit_convert", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_unit_convert_temperature() {
    let args = serde_json::json!({"value": 32.0, "from_unit": "F", "to_unit": "C"});
    let result = compare_tool_parity("unit_convert", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_unit_info_km() {
    let args = serde_json::json!({"unit": "km"});
    let result = compare_tool_parity("unit_info", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_unit_info_kg() {
    let args = serde_json::json!({"unit": "kg"});
    let result = compare_tool_parity("unit_info", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_unit_info_invalid() {
    let args = serde_json::json!({"unit": "not_a_unit"});
    let result = compare_tool_parity("unit_info", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_constant_lookup_speed_of_light() {
    let args = serde_json::json!({"name": "speed_of_light"});
    let result = compare_tool_parity("constant_lookup", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_constant_lookup_planck() {
    let args = serde_json::json!({"name": "planck"});
    let result = compare_tool_parity("constant_lookup", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_constant_lookup_boltzmann() {
    let args = serde_json::json!({"name": "boltzmann"});
    let result = compare_tool_parity("constant_lookup", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_path_normalize_basic() {
    let args = serde_json::json!({"path": "/foo/bar/../baz"});
    let result = compare_tool_parity("path_normalize", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_path_normalize_double_slash() {
    let args = serde_json::json!({"path": "//foo//bar"});
    let result = compare_tool_parity("path_normalize", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_path_normalize_trailing_slash() {
    let args = serde_json::json!({"path": "/foo/bar/"});
    let result = compare_tool_parity("path_normalize", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_fingerprint_basic() {
    let args = serde_json::json!({"text": "hello world"});
    let result = compare_tool_parity("text_fingerprint", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}

#[test]
fn test_text_fingerprint_unicode() {
    let args = serde_json::json!({"text": "héllo wörld"});
    let result = compare_tool_parity("text_fingerprint", args);
    assert!(result.passed, "Parity failed: {:?}", result.error);
}
