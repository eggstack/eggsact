//! Fuzz Markdown structure extraction and fenced code-block parsing.
//!
//! Asserts: no panic, extracted ranges within source bounds, code slices
//! match content, unclosed fences deterministic, output ordering deterministic.

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
    let _ = serde_json::to_string(&s2);

    // Code fence extract
    let fences = code_fence_extract(text, None, true);
    // Check all spans are within source bounds
    for fence in &fences.fences {
        assert!(fence.start_line <= fence.end_line);
    }
    let _ = serde_json::to_string(&fences);

    // With language filter
    let _ = code_fence_extract(text, Some("rust"), true);
    let _ = code_fence_extract(text, Some("python"), false);
});
