use serde_json::Value;

pub fn datetime_convert_input() -> Value {
    serde_json::json!({"type":"object","properties":{"value":{"type":"string","maxLength":100000,"description":"RFC 3339 or signed decimal Unix timestamp, always supplied as text"},"format":{"type":"string","enum":["rfc3339","unix_seconds","unix_milliseconds","unix_nanoseconds"]},"output_offset":{"type":"string","pattern":"^(Z|[+-][0-9]{2}:[0-9]{2})$","description":"Optional fixed offset; no IANA timezone names"}},"required":["value","format"]})
}

pub fn datetime_convert_output() -> Value {
    serde_json::json!({"type":"object","properties":{"rfc3339":{"type":"string"},"utc_rfc3339":{"type":"string"},"unix_seconds":{"type":"string"},"unix_milliseconds":{"type":"string"},"unix_nanoseconds":{"type":"string"},"offset_seconds":{"type":"integer"},"selected_offset":{"type":"string"},"components":{"type":"object"}}})
}

pub fn cron_inspect_input() -> Value {
    serde_json::json!({"type":"object","properties":{"expression":{"type":"string","maxLength":100000,"description":"Bounded five-field Vixie/POSIX-style cron expression"},"after":{"type":"string","maxLength":100000,"description":"Mandatory RFC 3339 reference instant"},"count":{"type":"integer","minimum":1,"maximum":32,"default":5}},"required":["expression","after"]})
}

pub fn cron_inspect_output() -> Value {
    serde_json::json!({"type":"object","properties":{"expression":{"type":"string"},"normalized_expression":{"type":"string"},"parsed_values":{"type":"object"},"offset":{"type":"string"},"offset_seconds":{"type":"integer"},"satisfiable":{"type":"boolean"},"next_runs":{"type":"array","items":{"type":"string"}},"count":{"type":"integer"}}})
}
