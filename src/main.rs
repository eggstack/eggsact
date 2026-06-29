use std::env;

#[derive(Debug, PartialEq, Eq)]
enum CliCommand {
    Help,
    Version,
    Mcp,
    Evaluate(String),
}

fn parse_args(args: impl IntoIterator<Item = String>) -> CliCommand {
    let args: Vec<String> = args.into_iter().collect();

    match args.as_slice() {
        [] => CliCommand::Help,
        [flag] if flag == "-h" || flag == "--help" => CliCommand::Help,
        [flag] if flag == "-V" || flag == "--version" => CliCommand::Version,
        [flag] if flag == "--mcp" => CliCommand::Mcp,
        _ => CliCommand::Evaluate(args.join(" ")),
    }
}

fn print_usage() {
    println!("Usage: eggsact [--mcp | expression]");
    println!("  --mcp          Start MCP server mode");
    println!("  -h, --help     Print this help message");
    println!("  -V, --version  Print version information");
    println!("  expression     Evaluate math expression");
}

#[tokio::main]
async fn main() {
    env::set_var("EGGCALC_NO_CONFIG", "1");

    match parse_args(env::args().skip(1)) {
        CliCommand::Help => print_usage(),
        CliCommand::Version => println!("eggsact {}", env!("CARGO_PKG_VERSION")),
        CliCommand::Mcp => eggsact::mcp::server::main().await,
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
}
