use std::env;

#[derive(Debug, PartialEq, Eq)]
enum CliCommand {
    Help,
    Version,
    Mcp,
    Diagnostics { format: String },
    Evaluate(String),
}

fn parse_args(args: impl IntoIterator<Item = String>) -> CliCommand {
    let args: Vec<String> = args.into_iter().collect();

    match args.as_slice() {
        [] => CliCommand::Help,
        [flag] if flag == "-h" || flag == "--help" => CliCommand::Help,
        [flag] if flag == "-V" || flag == "--version" => CliCommand::Version,
        [flag] if flag == "--mcp" => CliCommand::Mcp,
        [flag] if flag == "--diagnostics" => CliCommand::Diagnostics {
            format: "text".to_string(),
        },
        [s1, s2, fmt] if s1 == "--diagnostics" && s2 == "--format" => CliCommand::Diagnostics {
            format: fmt.clone(),
        },
        _ => CliCommand::Evaluate(args.join(" ")),
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
    let generated_doc_cmd = "cargo run --bin generate-docs";
    let compat_mcp = "EggcalcPython";
    let compat_inprocess = "StrictNative";
    let env_var_names = [
        "EGGCALC_NO_CONFIG",
        "EGGCALC_MCP_PROFILE",
        "EGGCALC_MCP_AUDIENCE",
        "EGGCALC_MCP_SCHEMA_DETAIL",
    ];
    let confusables_exists = std::path::Path::new("src/text/confusables_generated.rs").exists();
    let parity_ref_exists = std::path::Path::new("../eggcalc").exists();

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

        let diag = serde_json::json!({
            "version": version,
            "tool_count": tool_count,
            "profiles": profiles_obj,
            "generated_doc_command": generated_doc_cmd,
            "compatibility_mode": {
                "mcp_server": compat_mcp,
                "in_process_api": compat_inprocess,
            },
            "budget_tiers": tiers_obj,
            "env_var_names": env_vars,
            "generated_data": {
                "confusables_generated_rs": confusables_exists,
            },
            "parity_reference": {
                "path": "../eggcalc",
                "exists": parity_ref_exists,
            },
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
        println!("Generated-doc command: {}", generated_doc_cmd);
        println!();
        println!("Compatibility mode (default by surface):");
        println!("  MCP server:       {}", compat_mcp);
        println!("  In-process API:   {}", compat_inprocess);
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
        println!();
        println!(
            "confusables_generated.rs exists: {}",
            if confusables_exists { "yes" } else { "no" }
        );
        println!(
            "../eggcalc parity ref exists:    {}",
            if parity_ref_exists { "yes" } else { "no" }
        );
    }
}

#[tokio::main]
async fn main() {
    env::set_var("EGGCALC_NO_CONFIG", "1");

    match parse_args(env::args().skip(1)) {
        CliCommand::Help => print_usage(),
        CliCommand::Version => println!("eggsact {}", env!("CARGO_PKG_VERSION")),
        CliCommand::Mcp => eggsact::mcp::server::main().await,
        CliCommand::Diagnostics { format } => print_diagnostics(&format),
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
}
