#![no_main]

//! Fuzz regex feature classification.
//!
//! Asserts: no panic on arbitrary UTF-8, classification deterministic,
//! output is structured.

use libfuzzer_sys::fuzz_target;
use eggsact::text::classify_pattern;

const MAX_PATTERN_LEN: usize = 1_000;

fuzz_target!(|data: &[u8]| {
    let Ok(pattern) = std::str::from_utf8(data) else { return };
    if pattern.len() > MAX_PATTERN_LEN { return; }

    let class1 = classify_pattern(pattern);
    let class2 = classify_pattern(pattern);
    assert_eq!(format!("{:?}", class1.preferred_engine), format!("{:?}", class2.preferred_engine));
    assert_eq!(class1.features.len(), class2.features.len());
    assert_eq!(class1.unsupported_features, class2.unsupported_features);

    // Serializable
    let _ = serde_json::to_string(&serde_json::json!({
        "engine": format!("{:?}", class1.preferred_engine),
        "features": class1.features.len(),
        "unsupported": class1.unsupported_features,
    }));
});
