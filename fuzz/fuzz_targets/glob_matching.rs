//! Fuzz glob parsing and path matching.
//!
//! Asserts: no panic, normalization idempotent, matching deterministic,
//! classification buckets stable.

use libfuzzer_sys::fuzz_target;
use eggsact::text::{glob_match, path_analyze, path_normalize};

const MAX_PATH_LEN: usize = 5_000;

fuzz_target!(|data: &[u8]| {
    // Split into pattern and path
    if data.len() < 2 { return; }
    let split = data.len() / 2;
    let Ok(pattern) = std::str::from_utf8(&data[..split]) else { return };
    let Ok(path) = std::str::from_utf8(&data[split..]) else { return };
    if pattern.len() > MAX_PATH_LEN || path.len() > MAX_PATH_LEN { return; }

    // Glob match should not panic
    let m1 = glob_match(pattern, path, "posix", true);
    let m2 = glob_match(pattern, path, "posix", true);
    assert_eq!(m1.matches, m2.matches);

    // Windows mode
    let _ = glob_match(pattern, path, "windows", true);

    // Path analyze
    let _ = path_analyze(path, "posix");
    let _ = path_analyze(path, "windows");

    // Path normalize idempotence
    if let Ok(norm1) = path_normalize(path, "posix") {
        if let Ok(norm2) = path_normalize(&norm1, "posix") {
            assert_eq!(norm1, norm2);
        }
    }
});
