use serde_json::Value;

pub fn list_compare_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "a": {"type": "array", "items": {"type": "string"}, "description": "First list", "maxItems": 10000},
            "b": {"type": "array", "items": {"type": "string"}, "description": "Second list", "maxItems": 10000},
            "mode": {"type": "string", "enum": ["ordered", "set", "multiset"], "default": "set", "description": "Comparison mode: ordered (first diff, aligned ops), set (presence only), multiset (count deltas)"},
            "casefold": {"type": "boolean", "default": false, "description": "Casefold elements before comparison"},
            "normalization": {"type": "string", "enum": ["raw", "NFC", "NFD", "NFKC", "NFKD"], "default": "NFC", "description": "Unicode normalization form"},
            "trim": {"type": "boolean", "default": false, "description": "Trim whitespace from each element"},
            "include_near_matches": {"type": "boolean", "default": false, "description": "Include near matches (fuzzy matching)"},
            "near_match_threshold": {"type": "integer", "default": 2, "description": "Maximum edit distance for near matches"},
            "ignore_order": {"type": "boolean", "description": "Legacy: use mode=set or mode=multiset instead"},
            "treat_as_multiset": {"type": "boolean", "description": "Legacy: use mode=multiset instead"},
        },
        "required": ["a", "b"]
    })
}

pub fn list_dedupe_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "items": {"type": "array", "items": {"type": "string"}, "description": "List of strings to dedupe", "maxItems": 10000},
            "normalization": {"type": "string", "enum": ["raw", "NFC", "NFD", "NFKC", "NFKD"], "default": "NFC"},
            "casefold": {"type": "boolean", "default": false, "description": "Apply casefolding before comparison"},
            "stable": {"type": "boolean", "default": true, "description": "Accepted for compatibility; deduplication keeps first occurrence order"}
        },
        "required": ["items"]
    })
}

pub fn list_sort_input() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "items": {"type": "array", "items": {"type": "string"}, "description": "List of strings to sort", "maxItems": 10000},
            "normalization": {"type": "string", "enum": ["raw", "NFC", "NFD", "NFKC", "NFKD"], "default": "NFC"},
            "casefold": {"type": "boolean", "default": false, "description": "Apply casefolding for sorting"},
            "reverse": {"type": "boolean", "default": false, "description": "Sort in descending order"},
            "stable": {"type": "boolean", "default": true, "description": "Accepted for compatibility; Python sorting is always stable"}
        },
        "required": ["items"]
    })
}

pub fn list_compare_output() -> Value {
    serde_json::json!({"type":"object","properties":{"equal":{"type":"boolean"},"first_diff_index":{"type":["integer","null"],"description":"Index of first difference (ordered mode)"},"equal_prefix_length":{"type":"integer","description":"Length of equal prefix (ordered mode)"},"aligned":{"type":"array","description":"Aligned operations (ordered mode)"},"count_deltas":{"type":"object","description":"Count differences (multiset mode)"},"only_in_a":{"type":"array"},"only_in_b":{"type":"array"},"missing_in_a":{"type":"array","description":"Alias for only_in_b"},"missing_in_b":{"type":"array","description":"Alias for only_in_a"},"duplicates_in_a":{"type":"array"},"duplicates_in_b":{"type":"array"},"near_matches":{"type":"array","description":"Items that differ only by edit distance"}}})
}

pub fn list_dedupe_output() -> Value {
    serde_json::json!({"type":"object","properties":{"items":{"type":"array","items":{"type":"string"}},"original_count":{"type":"integer"},"deduped_count":{"type":"integer"},"duplicates_removed":{"type":"integer"}}})
}

pub fn list_sort_output() -> Value {
    serde_json::json!({"type":"object","properties":{"items":{"type":"array","items":{"type":"string"}},"original_count":{"type":"integer"},"sorted_count":{"type":"integer"}}})
}
