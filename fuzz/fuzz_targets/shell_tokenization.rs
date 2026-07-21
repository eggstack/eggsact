#![no_main]

//! Fuzz shell parsing and tokenization.
//!
//! Properties: deterministic, dangerous metacharacters never silently dropped.

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
    // Windows determinism
    let result_w2 = shell_split(cmd, "windows", true);
    assert_eq!(result_w.parse_ok, result_w2.parse_ok);
    assert_eq!(result_w.argv, result_w2.argv);

    // Dangerous metacharacters never silently dropped:
    // if a metacharacter is present and parse succeeds, the feature flags
    // or findings should reflect it
    let has_semicolon = cmd.contains(';');
    let has_pipe = cmd.contains('|');
    let has_ampersand = cmd.contains('&');
    let has_dollar = cmd.contains('$');
    let has_backtick = cmd.contains('`');
    let has_redirect = cmd.contains('>') || cmd.contains('<');
    if result.parse_ok {
        if has_semicolon || has_pipe || has_ampersand || has_dollar || has_backtick || has_redirect {
            assert!(
                result.features.has_control_operator
                    || result.features.has_pipe
                    || result.features.has_background
                    || result.features.has_variable_expansion
                    || result.features.has_command_substitution
                    || result.features.has_redirection
                    || !result.findings.is_empty()
            );
        }
    }

    // Serialization via JSON values (ShellSplitResult doesn't derive Serialize)
    let _ = serde_json::json!({
        "parse_ok": result.parse_ok,
        "argc": result.argc,
        "argv": result.argv,
    });
});
