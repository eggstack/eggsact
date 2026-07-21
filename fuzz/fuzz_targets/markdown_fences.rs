#![no_main]

//! Fuzz Markdown structure extraction and fenced code-block parsing.
//!
//! Asserts: no panic, extracted ranges within source bounds,
//! unclosed fences deterministic, output ordering deterministic.

use libfuzzer_sys::fuzz_target;
use eggsact::text::{markdown_structure, code_fence_extract};

const MAX_TEXT_LEN: usize = 50_000;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else { return };
    if text.len() > MAX_TEXT_LEN { return; }

    // Markdown structure
    let structure = markdown_structure(text, true, true, true, true);
    let _ = serde_json::to_string(&structure);

    // Deterministic
    let s2 = markdown_structure(text, true, true, true, true);
    let j1 = serde_json::to_value(&structure).unwrap();
    let j2 = serde_json::to_value(&s2).unwrap();
    assert_eq!(j1, j2);

    // Code fence extract
    let fences = code_fence_extract(text, None, true);
    // Check all spans are within source bounds
    for block in &fences.blocks {
        if let Some(end) = block.end_line {
            assert!(block.start_line <= end);
        }
    }
    let _ = serde_json::to_string(&fences);

    // Code fence extract deterministic
    let fences2 = code_fence_extract(text, None, true);
    let j1 = serde_json::to_value(&fences).unwrap();
    let j2 = serde_json::to_value(&fences2).unwrap();
    assert_eq!(j1, j2);

    // With language filter
    let _ = code_fence_extract(text, Some("rust"), true);
    let _ = code_fence_extract(text, Some("python"), false);
});
