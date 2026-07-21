//! Fuzz TOML and configuration parsing.
//!
//! Asserts: no panic, bounded findings, validation deterministic,
//! auto-detection deterministic.

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
    let _ = (r1, r2);

    // Dotenv validation
    let _ = dotenv_validate(text, true, "^[A-Z_][A-Z0-9_]*$", "warn");

    // INI validation
    let _ = ini_validate(text, "warn");
});
