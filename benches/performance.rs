//! Maintainer-run release-mode performance evidence.
//!
//! This is intentionally a dependency-free, non-gating harness.  Run it with
//! `cargo bench --locked --bench performance` and compare the stable text
//! output on the same host/toolchain before and after a change.

use eggsact::agent::ToolRegistry;
use eggsact::mcp::registry::{self, ToolListAudience, ToolListOptions};
use eggsact::mcp::response::{python_json_dumps, ToolResponse};
use serde_json::json;
use std::hint::black_box;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const WARMUP: usize = 10;
const REPETITIONS: usize = 50;

fn measure(label: &str, mut f: impl FnMut()) {
    for _ in 0..WARMUP {
        f();
    }
    let started = Instant::now();
    for _ in 0..REPETITIONS {
        f();
    }
    let elapsed = started.elapsed();
    let ns = elapsed.as_nanos() / REPETITIONS as u128;
    println!("scenario={label} repetitions={REPETITIONS} ns_per_op={ns}");
}

fn environment() {
    let rustc = Command::new("rustc")
        .arg("-Vv")
        .output()
        .ok()
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .trim()
                .replace('\n', ";")
        })
        .unwrap_or_else(|| "unavailable".to_string());
    let target = rustc
        .split(';')
        .find_map(|field| field.strip_prefix("host: "))
        .unwrap_or("unknown");
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let cpu = if cfg!(target_os = "macos") {
        Command::new("sysctl")
            .args(["-n", "machdep.cpu.brand_string"])
            .output()
            .ok()
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
            .unwrap_or_else(|| "unavailable".to_string())
    } else {
        std::fs::read_to_string("/proc/cpuinfo")
            .ok()
            .and_then(|text| {
                text.lines()
                    .find(|line| line.starts_with("model name"))
                    .and_then(|line| line.split_once(':'))
                    .map(|(_, model)| model.trim().to_string())
            })
            .unwrap_or_else(|| "unavailable".to_string())
    };
    println!(
        "baseline_or_candidate_sha={}",
        std::env::var("EGGSACT_BENCH_SHA").unwrap_or_else(|_| "unknown".to_string())
    );
    println!("rustc={rustc}");
    println!("target={target}");
    println!("os={os} arch={arch}");
    println!("release_profile=bench (release optimizations)");
    println!("cpu={cpu}");
    println!("warmup={WARMUP} repetitions={REPETITIONS}");
}

fn list_options(detail: &'static str) -> ToolListOptions<'static> {
    ToolListOptions {
        profile: "full",
        names: None,
        tier: None,
        tags: None,
        schema_detail: detail,
        audience: Some(ToolListAudience::Model),
    }
}

fn main() {
    environment();
    let registry = ToolRegistry::default();
    let cheap_args = json!({"a": "hello", "b": "hello"});
    let large_text = "x".repeat(64 * 1024);
    let patch = "--- a/file.txt\n+++ b/file.txt\n@@ -2,1 +2,1 @@\n-b\n+B\n";
    let narrow_names = vec!["text_equal".to_string()];

    measure("registry_prepare_call_cheap", || {
        black_box(
            registry
                .call_json("text_equal", cheap_args.clone())
                .unwrap(),
        );
    });
    let nested_args = json!({
        "a": {"items": [1, 2, 3]},
        "b": {"items": [3, 2, 1]},
        "ignore_array_order": true
    });
    measure("schema_validation_simple", || {
        black_box(registry.prepare_tool_call("text_equal", &cheap_args));
    });
    measure("schema_validation_nested", || {
        black_box(registry.prepare_tool_call("json_compare", &nested_args));
    });
    measure("tools_list_legacy_full", || {
        black_box(registry::list_tool_definitions(list_options("full")));
    });
    measure("tools_list_modern_full", || {
        black_box(registry::list_modern_tool_values(list_options("full")));
    });
    measure("tools_list_compact", || {
        black_box(registry::list_tool_definitions(list_options("compact")));
    });
    measure("tools_list_narrow_name_filter", || {
        black_box(registry::list_tool_definitions(ToolListOptions {
            names: Some(&narrow_names),
            ..list_options("normal")
        }));
    });
    measure("discovery_surface_listing", || {
        black_box(eggsact::mcp::discovery::discovery_legacy_definitions(
            "full",
            ToolListAudience::Model,
            None,
        ));
    });

    let small_response = ToolResponse::success(json!({"ok": true, "text": "hello"}), Some("bench"));
    let large_response = ToolResponse::success(json!({"text": large_text}), Some("bench"));
    let unicode_response = ToolResponse::success(json!({"text": "é🎉"}), Some("bench"));
    measure("response_small_ascii", || {
        black_box(python_json_dumps(&small_response));
    });
    measure("response_large_ascii", || {
        black_box(python_json_dumps(&large_response));
    });
    measure("response_non_ascii", || {
        black_box(python_json_dumps(&unicode_response));
    });
    measure("response_structured_modern", || {
        black_box(eggsact::mcp::response::wrap_tool_response_modern(
            &small_response,
        ));
    });
    measure("input_accounting_small", || {
        black_box(serde_json::to_vec(&cheap_args).unwrap().len());
    });
    measure("input_accounting_near_limit", || {
        black_box(
            serde_json::to_vec(&json!({"text": &large_text}))
                .unwrap()
                .len(),
        );
    });

    let many_matches = "needle ".repeat(5000);
    measure("text_replace_many_matches", || {
        let _ = black_box(eggsact::text::text_replace_check(
            &many_matches,
            "needle",
            "replacement",
            "exact",
            None,
            true,
            "preserve",
            false,
            2000,
        ));
    });
    measure("json_compare_unordered_arrays", || {
        let _ = black_box(registry.call_json(
            "json_compare",
            json!({"a": "[1,{\"x\":2},3]", "b": "[3,{\"x\":2},1]", "ignore_array_order": true}),
        ));
    });
    measure("json_extract_summary", || {
        let _ = black_box(registry.call_json(
            "json_extract",
            json!({"text": format!("{{\"items\":{}}}", "[1,2,3]"), "pointer": "/items", "detail": "summary"}),
        ));
    });
    measure("patch_apply_check_late_hunk", || {
        black_box(eggsact::text::patch_apply_check(
            &format!(
                "{}\n",
                (0..10000)
                    .map(|n| format!("line{n}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
            patch,
            true,
            true,
            false,
        ));
    });
    let patch_source_100k = (0..20_000)
        .map(|n| format!("line{n}"))
        .collect::<Vec<_>>()
        .join("\n");
    let patch_10_hunks = (0..10)
        .map(|n| {
            let line = 2 + n * 2;
            format!(
                "@@ -{line},1 +{line},1 @@\n-line{}\n+changed{}\n",
                line - 1,
                line - 1
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let patch_100_hunks = (0..100)
        .map(|n| {
            let line = 2 + n * 2;
            format!(
                "@@ -{line},1 +{line},1 @@\n-line{}\n+changed{}\n",
                line - 1,
                line - 1
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let patch_10 = format!("--- a/file.txt\n+++ b/file.txt\n{patch_10_hunks}");
    let patch_100 = format!("--- a/file.txt\n+++ b/file.txt\n{patch_100_hunks}");
    measure("patch_apply_check_100k_10_hunks", || {
        black_box(eggsact::text::patch_apply_check(
            &patch_source_100k,
            &patch_10,
            false,
            false,
            false,
        ));
    });
    measure("patch_apply_check_100k_100_hunks", || {
        black_box(eggsact::text::patch_apply_check(
            &patch_source_100k,
            &patch_100,
            false,
            false,
            false,
        ));
    });
    measure("tool_search", || {
        let params =
            eggsact::mcp::discovery::parse_search_params(&json!({"query": "compare json arrays"}))
                .unwrap();
        let specs = registry::tools_for_profile_audience("full", ToolListAudience::Model);
        black_box(eggsact::mcp::discovery::search_filtered(&specs, &params));
    });
    measure("repo_facts", || {
        black_box(eggsact::services::repo::repo_facts(&[
            "Cargo.toml".to_string(),
            "src/main.rs".to_string(),
            ".github/workflows/ci.yml".to_string(),
            "tests/test_main.rs".to_string(),
        ]));
    });
    measure("diff_spans_unicode", || {
        black_box(eggsact::text::diff_spans("αβγ hello", "αβγ goodbye", 50));
    });
    let named_capture =
        eggsact::text::compile_regex(r"(?P<word>[A-Za-z]+)", None, false, false, false).unwrap();
    measure("regex_named_captures", || {
        black_box(named_capture.captures("hello world").unwrap());
    });
    measure("codec_hex", || {
        let _ = black_box(registry.call_json(
            "codec_convert",
            json!({"value": "SGVsbG8gV29ybGQ=", "from": "base64", "to": "hex"}),
        ));
    });

    measure("mcp_stdio_warm_cheap_call", || {
        let binary = std::env::current_exe()
            .ok()
            .and_then(|path| path.parent()?.parent().map(|dir| dir.join("eggsact")));
        let Some(binary) = binary else { return };
        let request = json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "id": 1,
            "params": {
                "name": "text_equal",
                "arguments": {"a": "hello", "b": "hello"},
                "_meta": {
                    "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                    "io.modelcontextprotocol/clientCapabilities": {}
                }
            }
        });
        let mut child = Command::new(binary)
            .arg("--mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(format!("{}\n", request).as_bytes())
            .unwrap();
        let _ = child.wait_with_output().unwrap();
    });

    // Keep Duration referenced in this std-only harness so future maintainers
    // can add an explicit cold-start wall-clock sample without new crates.
    black_box(Duration::from_secs(0));
}
