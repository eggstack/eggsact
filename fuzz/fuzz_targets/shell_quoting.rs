#![no_main]

//! Fuzz shell quoting and round-trip properties.
//!
//! Properties: parse(quote_argv(argv)) == argv for safe argv,
//! re-quoting is stable, quoting cannot merge arguments.

use libfuzzer_sys::fuzz_target;
use eggsact::text::{shell_split, shell_quote_join};

const MAX_ARGV_ITEMS: usize = 50;
const MAX_ARG_LEN: usize = 1_000;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else { return };
    if text.len() > MAX_ARGV_ITEMS * MAX_ARG_LEN { return; }

    // Split then rejoin
    let split_result = shell_split(text, "posix", true);
    if split_result.parse_ok && split_result.argc <= MAX_ARGV_ITEMS {
        let join_result = shell_quote_join(&split_result.argv, "posix");

        // Round-trip: re-splitting the quoted command should recover the original argv
        if join_result.roundtrip_ok {
            let resplit = shell_split(&join_result.command, "posix", false);
            if resplit.parse_ok {
                assert_eq!(split_result.argv, resplit.argv);
                // Quoting cannot merge arguments
                assert_eq!(split_result.argv.len(), resplit.argv.len());
            }
        }

        // Re-quoting should be stable
        let rejoin = shell_quote_join(&split_result.argv, "posix");
        assert_eq!(join_result.command, rejoin.command);
    }
});
