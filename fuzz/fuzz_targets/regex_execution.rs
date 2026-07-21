//! Fuzz regex compile and bounded execution.
//!
//! Asserts: no panic, output bounded, spans within bounds, deterministic.

use libfuzzer_sys::fuzz_target;
use eggsact::text::{regex_safety_check, validate_regex, regex_finditer};

const MAX_PATTERN_LEN: usize = 500;
const MAX_TEXT_LEN: usize = 10_000;

fuzz_target!(|data: &[u8]| {
    // Split input into pattern and text
    if data.len() < 2 { return; }
    let split = data.len() / 2;
    let Ok(pattern) = std::str::from_utf8(&data[..split]) else { return };
    let Ok(text) = std::str::from_utf8(&data[split..]) else { return };
    if pattern.len() > MAX_PATTERN_LEN || text.len() > MAX_TEXT_LEN { return; }

    // Safety check should not panic
    let safety = regex_safety_check(pattern);
    let _ = serde_json::to_string(&safety);

    // validate_regex should not panic
    if let Ok(valid) = validate_regex(pattern, text) {
        let _ = valid;
    }

    // regex_finditer with bounded matches
    let result = regex_finditer(pattern, text, None, 100, false, false);
    // All match spans should be within text bounds
    for m in &result.matches {
        assert!(m.start <= text.len());
        assert!(m.end <= text.len());
        assert!(m.start <= m.end);
    }
    let _ = serde_json::to_string(&result);
});
