use std::env;

use eggsact::mcp::runtime;

#[derive(Debug, PartialEq, Eq)]
enum CliCommand {
    Help,
    Version,
    Mcp,
    Diagnostics { format: String },
    Error(String),
    Evaluate(String),
}

fn parse_args(args: impl IntoIterator<Item = String>) -> CliCommand {
    let args: Vec<String> = args.into_iter().collect();

    match args.as_slice() {
        [] => CliCommand::Help,
        [flag] if flag == "-h" || flag == "--help" => CliCommand::Help,
        [flag] if flag == "-V" || flag == "--version" => CliCommand::Version,
        [flag] if flag == "--mcp" => CliCommand::Mcp,
        _ => {
            if !args.iter().any(|arg| arg == "--diagnostics") {
                if args.iter().any(|arg| arg == "--format") {
                    return CliCommand::Error(
                        "--format is only valid with --diagnostics".to_string(),
                    );
                }
                return CliCommand::Evaluate(args.join(" "));
            }

            let mut format = "text";
            let mut format_seen = false;
            let mut i = 0;
            while i < args.len() {
                match args[i].as_str() {
                    "--diagnostics" => {}
                    "--format" => {
                        if format_seen || i + 1 >= args.len() {
                            return CliCommand::Error(
                                "--format requires exactly one value: json or text".to_string(),
                            );
                        }
                        format_seen = true;
                        format = args[i + 1].as_str();
                        i += 1;
                        if format != "json" && format != "text" {
                            return CliCommand::Error(format!(
                                "unknown diagnostics format '{}'; expected json or text",
                                format
                            ));
                        }
                    }
                    other => {
                        return CliCommand::Error(format!(
                            "unexpected argument '{}' with --diagnostics",
                            other
                        ));
                    }
                }
                i += 1;
            }
            CliCommand::Diagnostics {
                format: format.to_string(),
            }
        }
    }
}

fn print_usage() {
    println!("Usage: eggsact [--mcp | --diagnostics [--format json|text] | expression]");
    println!("  --mcp              Start MCP server mode");
    println!("  --diagnostics      Print diagnostic information");
    println!("  --format json|text Output format for --diagnostics (default: text)");
    println!("  -h, --help         Print this help message");
    println!("  -V, --version      Print version information");
    println!("  expression         Evaluate math expression");
}

fn print_diagnostics(format: &str) {
    let version = env!("CARGO_PKG_VERSION");
    let tool_count = eggsact::mcp::registry::tool_count();
    let profiles = eggsact::mcp::registry::available_profiles();
    let compat_mcp = "EggcalcPython";
    let compat_inprocess = "StrictNative";
    let env_var_names = [
        "EGGCALC_NO_CONFIG",
        "EGGCALC_MCP_PROFILE",
        "EGGCALC_MCP_AUDIENCE",
        "EGGCALC_MCP_SCHEMA_DETAIL",
    ];
    let route_critical = eggsact::mcp::registry::ROUTE_CRITICAL_TOOLS;

    let budget_tiers = [
        ("cheap", "1 MB in/out, 10s, 100 findings"),
        ("moderate", "1 MB in/out, 30s, 100 findings"),
        ("heavy", "1 MB in / 2 MB out, 30s, 100 findings"),
    ];

    if format == "json" {
        let profiles_obj: serde_json::Map<String, serde_json::Value> = profiles
            .iter()
            .map(|p| {
                let count = eggsact::mcp::registry::tools_for_profile(p).len();
                (p.to_string(), serde_json::Value::Number(count.into()))
            })
            .collect();

        let tiers_obj: serde_json::Map<String, serde_json::Value> = budget_tiers
            .iter()
            .map(|(name, desc)| {
                (
                    name.to_string(),
                    serde_json::Value::String(desc.to_string()),
                )
            })
            .collect();

        let env_vars: Vec<serde_json::Value> = env_var_names
            .iter()
            .map(|v| serde_json::Value::String(v.to_string()))
            .collect();

        let route_critical_vec: Vec<serde_json::Value> = route_critical
            .iter()
            .map(|name| serde_json::Value::String(name.to_string()))
            .collect();

        let diag = serde_json::json!({
            "version": version,
            "tool_count": tool_count,
            "profiles": profiles_obj,
            "compatibility_mode": {
                "mcp_server": compat_mcp,
                "in_process_api": compat_inprocess,
            },
            "route_critical_tools": route_critical_vec,
            "budget_tiers": tiers_obj,
            "runtime": {
                "active_profile": runtime::get_active_profile(),
                "active_audience": runtime::get_active_audience().to_string(),
                "schema_detail": runtime::get_schema_detail(),
                "limits": {
                    "max_in_flight_requests": runtime::MAX_IN_FLIGHT_REQUESTS,
                    "max_tool_workers": runtime::MAX_TOOL_WORKERS,
                    "max_request_bytes": runtime::MAX_REQUEST_BYTES,
                    "max_output_bytes": runtime::MAX_OUTPUT_BYTES,
                },
            },
            "env_var_names": env_vars,
        });
        println!("{}", serde_json::to_string_pretty(&diag).unwrap());
    } else {
        println!("eggsact diagnostics (v{})", version);
        println!();
        println!("Tools: {} total", tool_count);
        println!();
        println!("Profiles:");
        for p in profiles {
            let count = eggsact::mcp::registry::tools_for_profile(p).len();
            println!("  {}: {} tools", p, count);
        }
        println!();
        println!("Route-critical tools:");
        for name in route_critical {
            println!("  {}", name);
        }
        println!();
        println!("Compatibility mode (default by surface):");
        println!("  MCP server:       {}", compat_mcp);
        println!("  In-process API:   {}", compat_inprocess);
        println!();
        println!("Runtime:");
        println!("  Active profile: {}", runtime::get_active_profile());
        println!("  Active audience: {}", runtime::get_active_audience());
        println!("  Schema detail: {}", runtime::get_schema_detail());
        println!(
            "  Limits: {} in-flight, {} workers, {} bytes request, {} bytes output",
            runtime::MAX_IN_FLIGHT_REQUESTS,
            runtime::MAX_TOOL_WORKERS,
            runtime::MAX_REQUEST_BYTES,
            runtime::MAX_OUTPUT_BYTES,
        );
        println!();
        println!("Budget tiers:");
        for (name, desc) in &budget_tiers {
            println!("  {}: {}", name, desc);
        }
        println!();
        println!("Known env vars (names only, no values):");
        for v in &env_var_names {
            println!("  {}", v);
        }
    }
}

fn main() {
    // EGGCALC_NO_CONFIG is no longer set here: Rust no longer performs
    // Python-style config loading, so the env var is vestigial. Callers that
    // invoke the Python `eggcalc` sibling can set EGGCALC_NO_CONFIG=1
    // externally if needed. Removing the mutation avoids the `set_var` UB
    // window on Rust ≥1.89 when other threads may exist (LazyLock statics).

    match parse_args(env::args().skip(1)) {
        CliCommand::Help => print_usage(),
        CliCommand::Version => println!("eggsact {}", env!("CARGO_PKG_VERSION")),
        CliCommand::Mcp => {
            if let Err(error) = runtime::init_active_profile() {
                eprintln!("Error: {error}");
                std::process::exit(1);
            }
            // Release builds only: in debug builds this compilation takes
            // multiple wall-clock seconds per process, which multiplied across
            // every short-lived MCP subprocess in the integration suite blew
            // the CI job budget (>60 min). Debug builds keep plain lazy
            // initialization; release builds precompile here so no single
            // request is charged for it.
            #[cfg(not(debug_assertions))]
            eggsact::calc::normalize::warm_calculator_regex_cache();
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .thread_name("eggsact-mcp")
                .build()
                .expect("failed to create Tokio runtime");
            rt.block_on(eggsact::mcp::server::main());
        }
        CliCommand::Diagnostics { format } => print_diagnostics(&format),
        CliCommand::Error(error) => {
            eprintln!("Error: {error}");
            print_usage();
            std::process::exit(2);
        }
        CliCommand::Evaluate(expression) => match eggsact::calc::run(&expression) {
            Ok((result, _type)) => println!("{}", result),
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_args, CliCommand};

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parse_no_args_as_help() {
        assert_eq!(parse_args(args(&[])), CliCommand::Help);
    }

    #[test]
    fn parse_help_flags() {
        assert_eq!(parse_args(args(&["--help"])), CliCommand::Help);
        assert_eq!(parse_args(args(&["-h"])), CliCommand::Help);
    }

    #[test]
    fn parse_version_flags() {
        assert_eq!(parse_args(args(&["--version"])), CliCommand::Version);
        assert_eq!(parse_args(args(&["-V"])), CliCommand::Version);
    }

    #[test]
    fn parse_mcp_flag() {
        assert_eq!(parse_args(args(&["--mcp"])), CliCommand::Mcp);
    }

    #[test]
    fn parse_expression_joins_all_remaining_args() {
        assert_eq!(
            parse_args(args(&["thirty", "plus", "five"])),
            CliCommand::Evaluate("thirty plus five".to_string())
        );
    }

    #[test]
    fn parse_diagnostics_flag() {
        assert_eq!(
            parse_args(args(&["--diagnostics"])),
            CliCommand::Diagnostics {
                format: "text".to_string()
            }
        );
    }

    #[test]
    fn parse_diagnostics_format_json() {
        assert_eq!(
            parse_args(args(&["--diagnostics", "--format", "json"])),
            CliCommand::Diagnostics {
                format: "json".to_string()
            }
        );
    }

    #[test]
    fn parse_diagnostics_format_is_order_independent() {
        assert_eq!(
            parse_args(args(&["--format", "json", "--diagnostics"])),
            CliCommand::Diagnostics {
                format: "json".to_string()
            }
        );
    }

    #[test]
    fn parse_diagnostics_rejects_unknown_format() {
        assert!(matches!(
            parse_args(args(&["--diagnostics", "--format", "xml"])),
            CliCommand::Error(_)
        ));
    }

    #[test]
    fn parse_format_without_diagnostics_is_an_error() {
        assert!(matches!(
            parse_args(args(&["--format", "json"])),
            CliCommand::Error(_)
        ));
    }
}
