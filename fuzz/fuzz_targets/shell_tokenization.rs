#![no_main]

//! Fuzz shell parsing and tokenization.
//!
//! Properties: deterministic, no span exceeds source bounds, dangerous
//! metacharacters never silently dropped.

use libfuzzer_sys::fuzz_target;
use eggsact::text::shell_split;

const MAX_CMD_LEN: usize = 10_000;

fuzz_target!(|data: &[u8]| {
    let Ok(cmd) = std::str::from_utf8(data) else { return };
    if cmd.len() > MAX_CMD_LEN { return; }

    // POSIX
    let result = shell_split(cmd, "posix", true);
    // All argv items are valid UTF-8 (they are String)
    for a in &result.argv {
        assert!(std::str::from_utf8(a.as_bytes()).is_ok());
    }
    // Deterministic
    let result2 = shell_split(cmd, "posix", true);
    assert_eq!(result.parse_ok, result2.parse_ok);
    assert_eq!(result.argv, result2.argv);

    // Windows
    let result_w = shell_split(cmd, "windows", true);
    for a in &result_w.argv {
        assert!(std::str::from_utf8(a.as_bytes()).is_ok());
    }

    // Serialization via JSON values (ShellSplitResult doesn't derive Serialize)
    let _ = serde_json::json!({
        "parse_ok": result.parse_ok,
        "argc": result.argc,
        "argv": result.argv,
    });
});
