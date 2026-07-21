#![no_main]

//! Fuzz unified diff and patch parsing.
//!
//! Asserts: no panic, bounded findings, hunk ranges valid, malformed input
//! returns deterministic structured failure.

use libfuzzer_sys::fuzz_target;
use eggsact::text::{patch_summary, patch_apply_check};

const MAX_TEXT_LEN: usize = 50_000;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else { return };
    if text.len() > MAX_TEXT_LEN { return; }

    // Patch summary should never panic
    let summary = patch_summary(text);
    // Summary output should be serializable
    let _ = serde_json::to_string(&summary);

    // Patch apply check with empty original
    let result = patch_apply_check("", text, false, false, false);
    let _ = serde_json::to_string(&result);

    // Bounded findings
    assert!(result.findings.len() <= 100);

    // Hunk ranges valid: affected line ranges are ordered
    for range in &result.affected_line_ranges {
        assert!(range.start <= range.end);
    }

    // Deterministic
    let result2 = patch_apply_check("", text, false, false, false);
    let j1 = serde_json::to_value(&result).unwrap();
    let j2 = serde_json::to_value(&result2).unwrap();
    assert_eq!(j1, j2);
});
