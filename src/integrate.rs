//! Read-only renderers for registering eggsact as a client-owned MCP server.

use std::env;
use std::path::Path;

const CLIENTS: &[(&str, &str)] = &[
    ("zed", "Zed settings JSON"),
    ("codex", "Codex config.toml or `codex mcp add`"),
    ("claude", "Claude Code `claude mcp add`"),
    ("cursor", "Cursor mcp.json"),
    ("vscode", "VS Code mcp.json or `code --add-mcp`"),
    ("opencode", "OpenCode opencode.jsonc"),
];

fn resolved_path() -> Result<String, String> {
    let path = env::current_exe()
        .map_err(|error| format!("cannot resolve eggsact executable: {error}"))?;
    Ok(path.to_string_lossy().into_owned())
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).expect("serializing a string cannot fail")
}

pub fn render(client: &str, executable: &str) -> Result<String, String> {
    let path = Path::new(executable);
    if path.as_os_str().is_empty() {
        return Err("eggsact executable path cannot be empty".into());
    }
    match client {
        "zed" => Ok(format!(
            r#"{{
  "context_servers": {{
    "eggsact": {{
      "command": {},
      "args": ["--mcp"],
      "env": {{}}
    }}
  }}
}}"#,
            json_string(executable)
        )),
        "codex" => Ok(format!(
            "[mcp_servers.eggsact]\ncommand = {}\nargs = [\"--mcp\"]\n",
            json_string(executable)
        )),
        "claude" => Ok(format!(
            "claude mcp add eggsact -- {} --mcp",
            shell_quote(executable)
        )),
        "cursor" => Ok(format!(
            r#"{{
  "mcpServers": {{
    "eggsact": {{
      "command": {},
      "args": ["--mcp"]
    }}
  }}
}}"#,
            json_string(executable)
        )),
        "vscode" => Ok(format!(
            "code --add-mcp {}",
            shell_quote(&format!(
                r#"{{"name":"eggsact","command":{},"args":["--mcp"]}}"#,
                json_string(executable)
            ))
        )),
        "opencode" => Ok(format!(
            r#"{{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {{
    "servers": {{
      "eggsact": {{
        "type": "local",
        "command": [{}, "--mcp"]
      }}
    }}
  }}
}}"#,
            json_string(executable)
        )),
        _ => Err(format!(
            "unknown client '{client}'; choose one of: {}",
            CLIENTS
                .iter()
                .map(|(name, _)| *name)
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

fn shell_quote(value: &str) -> String {
    if cfg!(windows) {
        format!("\"{}\"", value.replace('"', "\\\""))
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn command_on_path(command: &str) -> bool {
    let Some(paths) = env::var_os("PATH") else {
        return false;
    };
    for directory in env::split_paths(&paths) {
        let candidate = directory.join(command);
        if candidate.is_file() {
            return true;
        }
        if cfg!(windows)
            && [".exe", ".cmd", ".bat"]
                .iter()
                .any(|suffix| directory.join(format!("{command}{suffix}")).is_file())
        {
            return true;
        }
    }
    false
}

pub fn run(client: Option<&str>) -> Result<(), String> {
    let path = resolved_path()?;
    match client {
        None | Some("list") => {
            if client.is_none() {
                println!("usage: eggsact integrate list | detect | <client>");
            }
            println!("Supported MCP clients:");
            for (name, description) in CLIENTS {
                println!("  {name:<8} {description}");
            }
        }
        Some("detect") => {
            println!("MCP client detection (PATH only):");
            for (name, _) in CLIENTS {
                let command = match *name {
                    "vscode" => "code",
                    "cursor" => "cursor-agent",
                    other => other,
                };
                println!(
                    "  {name:<8} {}",
                    if command_on_path(command) {
                        "found"
                    } else {
                        "not found"
                    }
                );
            }
        }
        Some(name) => {
            println!("Register eggsact as the client-owned stdio MCP server named 'eggsact':\n");
            println!("{}", render(name, &path)?);
            println!("\nThis command/config is an instruction only; eggsact does not edit client configuration.");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderers_keep_server_name_path_and_stdio_argument() {
        for client in ["zed", "codex", "claude", "cursor", "vscode", "opencode"] {
            let rendered = render(client, "/path with spaces/eggsact").unwrap();
            assert!(rendered.contains("eggsact"));
            assert!(rendered.contains("--mcp"));
            assert!(rendered.contains("path with spaces"));
        }
    }

    #[test]
    fn strict_json_renderers_parse() {
        let zed: serde_json::Value =
            serde_json::from_str(&render("zed", "/opt/eggsact").unwrap()).unwrap();
        assert_eq!(zed["context_servers"]["eggsact"]["args"][0], "--mcp");
        let cursor: serde_json::Value =
            serde_json::from_str(&render("cursor", "/opt/eggsact").unwrap()).unwrap();
        assert_eq!(cursor["mcpServers"]["eggsact"]["command"], "/opt/eggsact");
        let opencode: serde_json::Value =
            serde_json::from_str(&render("opencode", "/opt/eggsact").unwrap()).unwrap();
        assert_eq!(opencode["mcp"]["servers"]["eggsact"]["command"][1], "--mcp");
    }

    #[test]
    fn unknown_clients_are_rejected() {
        assert!(render("unknown", "eggsact").is_err());
    }
}
