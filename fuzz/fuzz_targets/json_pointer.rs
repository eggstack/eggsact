#![no_main]

//! Fuzz JSON parsing and pointer extraction.
//!
//! Asserts: no panic on malformed JSON or pointers,
//! canonicalization idempotent, serialization succeeds.

use libfuzzer_sys::fuzz_target;
use eggsact::text::{validate_json, json_extract, json_canonicalize, json_shape, json_compare};

const MAX_TEXT_LEN: usize = 50_000;

fuzz_target!(|data: &[u8]| {
    // Split into JSON text and pointer
    if data.len() < 2 { return; }
    let split = data.len() / 3;
    let Ok(json_text) = std::str::from_utf8(&data[..split]) else { return };
    let Ok(pointer) = std::str::from_utf8(&data[split..split*2]) else { return };
    let Ok(other_json) = std::str::from_utf8(&data[split*2..]) else { return };
    if json_text.len() > MAX_TEXT_LEN || other_json.len() > MAX_TEXT_LEN { return; }

    // Validate should not panic, serialization succeeds
    if let Ok(result) = validate_json(json_text) {
        assert!(serde_json::to_string(&result).is_ok());
    }

    // Extract should not panic, serialization succeeds
    if let Ok(result) = json_extract(json_text, pointer, 10_000) {
        assert!(serde_json::to_value(&result).is_ok());
    }

    // Canonicalize and check idempotence
    if let Ok(canon1) = json_canonicalize(json_text, true, Some(2), false, true, false) {
        if let Some(ref canonical) = canon1.canonical {
            if let Ok(canon2) = json_canonicalize(canonical, true, Some(2), false, true, false) {
                assert_eq!(canon1.canonical, canon2.canonical);
            }
        }
    }

    // Shape should not panic
    let _ = json_shape(json_text, 10, 100, 100);

    // Compare should not panic
    let _ = json_compare(json_text, other_json, true, false, false, false, false, 100);
});
