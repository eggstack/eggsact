use serde_json::Value;

pub fn codec_convert_input() -> Value {
    serde_json::json!({"type":"object","properties":{"value":{"type":"string","maxLength":100000},"from":{"type":"string","enum":["utf8","hex","base64","base64url"]},"to":{"type":"string","enum":["utf8","hex","base64","base64url"]}},"required":["value","from","to"]})
}

pub fn codec_convert_output() -> Value {
    serde_json::json!({"type":"object","properties":{"value":{"type":"string"},"from":{"type":"string"},"to":{"type":"string"},"byte_length":{"type":"integer"}}})
}

pub fn radix_convert_input() -> Value {
    serde_json::json!({"type":"object","properties":{"value":{"type":"string","maxLength":100000},"from_base":{"type":"integer","minimum":2,"maximum":36},"to_base":{"type":"integer","minimum":2,"maximum":36},"uppercase":{"type":"boolean","default":false}},"required":["value","from_base","to_base"]})
}

pub fn radix_convert_output() -> Value {
    serde_json::json!({"type":"object","properties":{"value":{"type":"string"},"from_base":{"type":"integer"},"to_base":{"type":"integer"},"uppercase":{"type":"boolean"},"negative":{"type":"boolean"},"magnitude_decimal":{"type":"string"}}})
}
