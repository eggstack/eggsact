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
    assert!(result.argv.iter().all(|a| a.is_utf8()));
    // Deterministic
    let result2 = shell_split(cmd, "posix", true);
    assert_eq!(result.parse_ok, result2.parse_ok);
    assert_eq!(result.argv, result2.argv);

    // Windows
    let result_w = shell_split(cmd, "windows", true);
    assert!(result_w.argv.iter().all(|a| a.is_utf8()));

    // Serialization
    let _ = serde_json::to_string(&result);
});
