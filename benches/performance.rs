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
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
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

fn release_binary() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent()?.parent().map(|dir| dir.join("eggsact")))
}

fn mcp_request(
    stdin: &mut std::process::ChildStdin,
    stdout: &mut BufReader<std::process::ChildStdout>,
    id: u64,
) {
    let request = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "id": id,
        "params": {
            "name": "text_equal",
            "arguments": {"a": "hello", "b": "hello"},
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                "io.modelcontextprotocol/clientCapabilities": {}
            }
        }
    });
    writeln!(stdin, "{request}").expect("write MCP request");
    stdin.flush().expect("flush MCP request");

    let mut line = String::new();
    let bytes = stdout.read_line(&mut line).expect("read MCP response");
    assert!(bytes > 0, "MCP server exited before replying");
    let response: serde_json::Value =
        serde_json::from_str(&line).expect("MCP response must be JSON");
    assert_eq!(
        response.get("id").and_then(serde_json::Value::as_u64),
        Some(id)
    );
}

fn measure_mcp_stdio_warm(binary: PathBuf) {
    let mut child = Command::new(binary)
        .arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn MCP server");
    let mut stdin = child.stdin.take().expect("MCP stdin");
    let stdout = child.stdout.take().expect("MCP stdout");
    let mut stdout = BufReader::new(stdout);

    // Protocol selection and all warmup requests happen before timing. Each
    // measured operation is one sequential write/flush/read round trip.
    for id in 0..WARMUP as u64 {
        mcp_request(&mut stdin, &mut stdout, id);
    }
    let started = Instant::now();
    for id in WARMUP as u64..(WARMUP + REPETITIONS) as u64 {
        mcp_request(&mut stdin, &mut stdout, id);
    }
    let elapsed = started.elapsed();
    let ns = elapsed.as_nanos() / REPETITIONS as u128;
    println!("scenario=mcp_stdio_warm_cheap_call repetitions={REPETITIONS} ns_per_op={ns}");

    drop(stdin);
    let status = child.wait().expect("reap MCP server");
    assert!(
        status.success(),
        "MCP server exited unsuccessfully: {status}"
    );
}

fn representative_repo_paths(count: usize) -> Vec<String> {
    (0..count)
        .map(|index| match index % 10 {
            0 => format!("src/module_{index}.rs"),
            1 => format!("tests/test_{index}.py"),
            2 => format!("web/component_{index}.js"),
            3 => format!("config/service_{index}.yaml"),
            4 => format!(".github/workflows/check-{index}.yml"),
            5 => format!("docs/guide_{index}.md"),
            6 => format!("manifests/package_{index}.json"),
            7 => format!("locks/dependency_{index}.lock"),
            8 => format!("hidden/.env_{index}"),
            _ => format!("scripts/tool_{index}.sh"),
        })
        .collect()
}

fn representative_diff_inputs() -> (String, String) {
    let mut a = String::new();
    let mut b = String::new();
    for index in 0..320 {
        let marker = char::from_u32(0x1000 + index as u32).expect("valid benchmark marker");
        a.push(marker);
        a.push('A');
        b.push(marker);
        b.push('B');
    }
    (a, b)
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
    let small_input_budget = eggsact::mcp::budget::ToolBudget::CHEAP.with_max_input_bytes(1);
    measure("input_budget_path_small", || {
        let response = registry
            .call_json_with_budget("text_equal", cheap_args.clone(), Some(small_input_budget))
            .unwrap();
        assert_eq!(response.error_type.as_deref(), Some("input_too_large"));
        black_box(response);
    });
    let near_limit_args = json!({"a": &large_text, "b": &large_text});
    let near_limit_budget =
        eggsact::mcp::budget::ToolBudget::CHEAP.with_max_input_bytes(large_text.len());
    measure("input_budget_path_near_limit", || {
        let response = registry
            .call_json_with_budget(
                "text_equal",
                near_limit_args.clone(),
                Some(near_limit_budget),
            )
            .unwrap();
        assert_eq!(response.error_type.as_deref(), Some("input_too_large"));
        black_box(response);
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
    let repo_paths_100 = representative_repo_paths(100);
    let repo_paths_1000 = representative_repo_paths(1000);
    measure("repo_facts_100_paths", || {
        black_box(eggsact::services::repo::repo_facts(&repo_paths_100));
    });
    measure("repo_facts_1000_paths", || {
        black_box(eggsact::services::repo::repo_facts(&repo_paths_1000));
    });
    measure("diff_spans_unicode", || {
        black_box(eggsact::text::diff_spans("αβγ hello", "αβγ goodbye", 50));
    });
    let (diff_a_representative, diff_b_representative) = representative_diff_inputs();
    let representative_span_count =
        eggsact::text::diff_spans(&diff_a_representative, &diff_b_representative, 500).len();
    assert!(representative_span_count > 100);
    measure("diff_spans_representative_multispan", || {
        let spans = eggsact::text::diff_spans(&diff_a_representative, &diff_b_representative, 500);
        assert_eq!(spans.len(), representative_span_count);
        black_box(spans);
    });
    let named_capture =
        eggsact::text::compile_regex(r"(?P<word>[A-Za-z]+)", None, false, false, false).unwrap();
    measure("regex_named_captures", || {
        black_box(named_capture.captures("hello world").unwrap());
    });
    let named_capture_many_pattern = r"(?P<word>[A-Za-z]+)=(?P<number>[0-9]+)";
    let named_capture_many_text = (0..500)
        .map(|index| format!("word={index}"))
        .collect::<Vec<_>>()
        .join(" ");
    measure("regex_named_captures_500_matches", || {
        let result = eggsact::text::regex_finditer(
            named_capture_many_pattern,
            &named_capture_many_text,
            None,
            500,
            false,
            true,
        );
        assert_eq!(result.match_count, 500);
        black_box(result);
    });
    measure("codec_hex", || {
        let _ = black_box(registry.call_json(
            "codec_convert",
            json!({"value": "SGVsbG8gV29ybGQ=", "from": "base64", "to": "hex"}),
        ));
    });

    if let Some(binary) = release_binary() {
        measure_mcp_stdio_warm(binary);
    }

    // Keep Duration referenced in this std-only harness so future maintainers
    // can add an explicit cold-start wall-clock sample without new crates.
    black_box(Duration::from_secs(0));
}
