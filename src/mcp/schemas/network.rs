use serde_json::Value;

pub fn ip_inspect_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {"address": {"type": "string", "maxLength": 100000, "description": "IPv4 or IPv6 address"}},
        "required": ["address"]
    })
}

pub fn ip_inspect_output() -> Value {
    serde_json::json!({"type":"object","properties":{"address":{"type":"string"},"family":{"type":"string","enum":["ipv4","ipv6"]},"bytes_hex":{"type":"string"},"numeric":{"type":"string"},"special_use":{"type":"array","items":{"type":"string"}},"ipv4_mapped":{"type":["object","null"]}}})
}

pub fn cidr_inspect_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "cidr": {"type": "string", "maxLength": 100000, "description": "IPv4 or IPv6 CIDR"},
            "contains": {"type": "string", "maxLength": 100000, "description": "Optional address to test for containment"}
        },
        "required": ["cidr"]
    })
}

pub fn cidr_inspect_output() -> Value {
    serde_json::json!({"type":"object","properties":{"family":{"type":"string"},"cidr":{"type":"string"},"prefix_length":{"type":"integer"},"host_bits":{"type":"integer"},"network_address":{"type":"string"},"netmask":{"type":"string"},"first_address":{"type":"string"},"last_address":{"type":"string"},"broadcast_address":{"type":["string","null"]},"address_count":{"type":"string"},"contains":{"type":["boolean","null"]},"contains_address":{"type":["string","null"]}}})
}
