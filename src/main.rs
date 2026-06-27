use std::env;

#[tokio::main]
async fn main() {
    env::set_var("EGGCALC_NO_CONFIG", "1");
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 && args[1] == "--mcp" {
        eggsact::mcp::server::main().await;
    } else if args.len() > 1 {
        let expr = args[1..].join(" ");
        match eggsact::calc::run(&expr) {
            Ok((result, _type)) => println!("{}", result),
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        println!("Usage: eggsact [--mcp | expression]");
        println!("  --mcp       Start MCP server mode");
        println!("  expression  Evaluate math expression");
    }
}
