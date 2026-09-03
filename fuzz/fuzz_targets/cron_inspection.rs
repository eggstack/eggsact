#![no_main]

use eggsact::tools::cron_inspect;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: &[u8]| {
    let expression = String::from_utf8_lossy(input);
    let _ = cron_inspect(&serde_json::json!({
        "expression": expression,
        "after": "2026-09-03T11:00:00Z",
        "count": 1,
    }));
});
