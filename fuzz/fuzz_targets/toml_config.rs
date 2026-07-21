#![no_main]

//! Fuzz TOML and configuration parsing.
//!
//! Asserts: no panic, bounded findings, validation deterministic.

use libfuzzer_sys::fuzz_target;
use eggsact::text::{validate_toml, toml_shape, dotenv_validate, ini_validate};

const MAX_TEXT_LEN: usize = 50_000;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else { return };
    if text.len() > MAX_TEXT_LEN { return; }

    // TOML validation
    let _ = validate_toml(text);
    let _ = toml_shape(text, 100);

    // Deterministic
    let r1 = validate_toml(text);
    let r2 = validate_toml(text);
    match (&r1, &r2) {
        (Ok(a), Ok(b)) => {
            let j1 = serde_json::to_value(a).unwrap();
            let j2 = serde_json::to_value(b).unwrap();
            assert_eq!(j1, j2);
        }
        (Err(_), Err(_)) => {}
        _ => panic!("validate_toml determinism violated"),
    }

    // Bounded findings
    if let Ok(ref r) = r1 {
        assert!(r.error.is_none() || !r.error.as_ref().unwrap().is_empty());
    }

    // Dotenv validation
    let dr1 = dotenv_validate(text, true, "^[A-Z_][A-Z0-9_]*$", "warn");
    let dr2 = dotenv_validate(text, true, "^[A-Z_][A-Z0-9_]*$", "warn");
    let j1 = serde_json::to_value(&dr1).unwrap();
    let j2 = serde_json::to_value(&dr2).unwrap();
    assert_eq!(j1, j2);
    assert!(dr1.findings.len() <= 100);

    // INI validation
    let ir1 = ini_validate(text, "warn");
    let ir2 = ini_validate(text, "warn");
    let j1 = serde_json::to_value(&ir1).unwrap();
    let j2 = serde_json::to_value(&ir2).unwrap();
    assert_eq!(j1, j2);
    assert!(ir1.findings.len() <= 100);
});
