use crate::mcp::machine_codes;
use crate::mcp::schemas::{disposition, finding, severity, verdict, ToolResponse};
use crate::tools::helpers::*;
use serde_json::Value;

pub fn shell_split(args: &Value) -> ToolResponse {
    let command = match args.get("command").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'command' parameter",
                None,
                Some("shell_split"),
            )
        }
    };
    let shell = args
        .get("shell")
        .and_then(|v| v.as_str())
        .unwrap_or("posix");
    let detect_risky_features = args
        .get("detect_risky_features")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    if command.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Command exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("shell_split"),
        );
    }

    let valid_shells = ["posix"];
    if !valid_shells.contains(&shell) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported shell: {}", shell),
            Some(vec!["Use one of: posix".to_string()]),
            Some("shell_split"),
        );
    }

    let result = crate::text::shell_split(command, shell, detect_risky_features);

    ToolResponse::success(
        serde_json::json!({
            "parse_ok": result.parse_ok,
            "argv": result.argv,
            "argc": result.argc,
            "features": {
                "has_pipe": result.features.has_pipe,
                "has_redirection": result.features.has_redirection,
                "has_command_substitution": result.features.has_command_substitution,
                "has_variable_expansion": result.features.has_variable_expansion,
                "has_glob_pattern": result.features.has_glob_pattern,
                "has_control_operator": result.features.has_control_operator,
                "has_background": result.features.has_background,
                "has_unbalanced_quotes": result.features.has_unbalanced_quotes,
            },
            "findings": result.findings,
        }),
        Some("shell_split"),
    )
    .with_tool("shell_split")
}

pub fn shell_quote_join(args: &Value) -> ToolResponse {
    let argv_raw = match args.get("argv").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'argv' parameter",
                None,
                Some("shell_quote_join"),
            )
        }
    };
    let shell = args
        .get("shell")
        .and_then(|v| v.as_str())
        .unwrap_or("posix");

    if argv_raw.len() > MAX_LIST_ITEMS {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INVALID_ARGUMENTS,
            &format!("argv length {} exceeds MAX_LIST_ITEMS", argv_raw.len()),
            None,
            Some("shell_quote_join"),
        );
    }

    let non_str_indices: Vec<usize> = argv_raw
        .iter()
        .enumerate()
        .filter(|(_, v)| !v.is_string())
        .map(|(i, _)| i)
        .collect();
    if !non_str_indices.is_empty() {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            "All argv elements must be strings",
            Some(vec![format!(
                "Non-string items at indices: {:?}",
                &non_str_indices[..5.min(non_str_indices.len())]
            )]),
            Some("shell_quote_join"),
        );
    }
    let oversized_indices: Vec<usize> = argv_raw
        .iter()
        .enumerate()
        .filter(|(_, v)| {
            v.as_str()
                .is_some_and(|s| s.chars().count() > MAX_TEXT_LENGTH)
        })
        .map(|(i, _)| i)
        .collect();
    if !oversized_indices.is_empty() {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INVALID_ARGUMENTS,
            &format!("argv items exceed max length {}", MAX_TEXT_LENGTH),
            Some(vec![format!(
                "Oversized items at indices: {:?}",
                &oversized_indices[..5.min(oversized_indices.len())]
            )]),
            Some("shell_quote_join"),
        );
    }
    let mut argv: Vec<String> = Vec::with_capacity(argv_raw.len());
    for v in argv_raw.iter() {
        if let Some(s) = v.as_str() {
            argv.push(s.to_string());
        }
    }

    let valid_shells = ["posix"];
    if !valid_shells.contains(&shell) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported shell: {}", shell),
            Some(vec![format!("Use one of: {}", valid_shells.join(", "))]),
            Some("shell_quote_join"),
        );
    }

    let result = crate::text::shell_quote_join(&argv, shell);

    ToolResponse::success(
        serde_json::json!({
            "command": result.command,
            "roundtrip_ok": result.roundtrip_ok,
            "findings": result.findings,
        }),
        Some("shell_quote_join"),
    )
    .with_tool("shell_quote_join")
}

pub fn argv_compare(args: &Value) -> ToolResponse {
    let left_command = args.get("left_command").and_then(|v| v.as_str());
    let right_command = args.get("right_command").and_then(|v| v.as_str());
    let left_argv = match args.get("left_argv").and_then(|v| v.as_array()) {
        Some(arr) => {
            if arr.len() > MAX_LIST_ITEMS {
                return ToolResponse::error_with_code(
                    "input_too_large",
                    machine_codes::INVALID_ARGUMENTS,
                    &format!("left_argv length {} exceeds {}", arr.len(), MAX_LIST_ITEMS),
                    None,
                    Some("argv_compare"),
                );
            }
            let non_str: Vec<usize> = arr
                .iter()
                .enumerate()
                .filter(|(_, v)| !v.is_string())
                .map(|(i, _)| i)
                .collect();
            if !non_str.is_empty() {
                return ToolResponse::error_with_code(
                    "invalid_arguments",
                    machine_codes::INVALID_ARGUMENTS,
                    "All left_argv elements must be strings",
                    Some(vec![format!(
                        "Non-string items at indices: {:?}",
                        &non_str[..5.min(non_str.len())]
                    )]),
                    Some("argv_compare"),
                );
            }
            let oversized: Vec<usize> = arr
                .iter()
                .enumerate()
                .filter(|(_, v)| {
                    v.as_str()
                        .is_some_and(|s| s.chars().count() > MAX_TEXT_LENGTH)
                })
                .map(|(i, _)| i)
                .collect();
            if !oversized.is_empty() {
                return ToolResponse::error_with_code(
                    "input_too_large",
                    machine_codes::INVALID_ARGUMENTS,
                    &format!("left_argv items exceed max length {}", MAX_TEXT_LENGTH),
                    Some(vec![format!(
                        "Oversized items at indices: {:?}",
                        &oversized[..5.min(oversized.len())]
                    )]),
                    Some("argv_compare"),
                );
            }
            Some(
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect::<Vec<_>>(),
            )
        }
        None => None,
    };
    let right_argv = match args.get("right_argv").and_then(|v| v.as_array()) {
        Some(arr) => {
            if arr.len() > MAX_LIST_ITEMS {
                return ToolResponse::error_with_code(
                    "input_too_large",
                    machine_codes::INVALID_ARGUMENTS,
                    &format!("right_argv length {} exceeds {}", arr.len(), MAX_LIST_ITEMS),
                    None,
                    Some("argv_compare"),
                );
            }
            let non_str: Vec<usize> = arr
                .iter()
                .enumerate()
                .filter(|(_, v)| !v.is_string())
                .map(|(i, _)| i)
                .collect();
            if !non_str.is_empty() {
                return ToolResponse::error_with_code(
                    "invalid_arguments",
                    machine_codes::INVALID_ARGUMENTS,
                    "All right_argv elements must be strings",
                    Some(vec![format!(
                        "Non-string items at indices: {:?}",
                        &non_str[..5.min(non_str.len())]
                    )]),
                    Some("argv_compare"),
                );
            }
            let oversized: Vec<usize> = arr
                .iter()
                .enumerate()
                .filter(|(_, v)| {
                    v.as_str()
                        .is_some_and(|s| s.chars().count() > MAX_TEXT_LENGTH)
                })
                .map(|(i, _)| i)
                .collect();
            if !oversized.is_empty() {
                return ToolResponse::error_with_code(
                    "input_too_large",
                    machine_codes::INVALID_ARGUMENTS,
                    &format!("right_argv items exceed max length {}", MAX_TEXT_LENGTH),
                    Some(vec![format!(
                        "Oversized items at indices: {:?}",
                        &oversized[..5.min(oversized.len())]
                    )]),
                    Some("argv_compare"),
                );
            }
            Some(
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect::<Vec<_>>(),
            )
        }
        None => None,
    };
    let shell = args
        .get("shell")
        .and_then(|v| v.as_str())
        .unwrap_or("posix");

    let valid_shells = ["posix"];
    if !valid_shells.contains(&shell) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported shell: {}", shell),
            Some(vec![format!("Use one of: {}", valid_shells.join(", "))]),
            Some("argv_compare"),
        );
    }

    // XOR validation: each side must be either a *_command OR an *_argv, not both (and not neither).
    let left_both = left_command.is_some() == left_argv.is_some();
    let right_both = right_command.is_some() == right_argv.is_some();
    if left_both {
        let msg = if left_command.is_some() && left_argv.is_some() {
            "Provide exactly one of left_command or left_argv, not both"
        } else {
            "Provide exactly one of left_command or left_argv"
        };
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            msg,
            None,
            Some("argv_compare"),
        );
    }
    if right_both {
        let msg = if right_command.is_some() && right_argv.is_some() {
            "Provide exactly one of right_command or right_argv, not both"
        } else {
            "Provide exactly one of right_command or right_argv"
        };
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            msg,
            None,
            Some("argv_compare"),
        );
    }

    if let Some(cmd) = left_command {
        if cmd.chars().count() > MAX_TEXT_LENGTH {
            return ToolResponse::error_with_code(
                "input_too_large",
                machine_codes::INVALID_ARGUMENTS,
                "Left command exceeds MAX_TEXT_LENGTH",
                None,
                Some("argv_compare"),
            );
        }
    }
    if let Some(cmd) = right_command {
        if cmd.chars().count() > MAX_TEXT_LENGTH {
            return ToolResponse::error_with_code(
                "input_too_large",
                machine_codes::INVALID_ARGUMENTS,
                "Right command exceeds MAX_TEXT_LENGTH",
                None,
                Some("argv_compare"),
            );
        }
    }

    let left_ref = left_command;
    let right_ref = right_command;
    let left_argv_ref = left_argv.as_deref();
    let right_argv_ref = right_argv.as_deref();

    let result =
        crate::text::argv_compare(left_ref, right_ref, left_argv_ref, right_argv_ref, shell);

    ToolResponse::success(
        serde_json::json!({
            "argv_equal": result.argv_equal,
            "left_argv": result.left_argv,
            "right_argv": result.right_argv,
            "first_difference": result.first_difference,
            "findings": result.findings,
        }),
        Some("argv_compare"),
    )
    .with_tool("argv_compare")
}

// ---------------------------------------------------------------------------
// Command Policy Engine
// ---------------------------------------------------------------------------

/// Command classification for the allow/review/block matrix.
/// `allow` = permitted under this policy; `review` = needs human review;
/// `block` = always blocked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CmdDisposition {
    Allow,
    Review,
    Block,
}

/// Default classification matrix for the "default" policy.
/// Returns `(program, subcommand) -> CmdDisposition`.
fn classify_default(program: &str, subcommand: &str) -> CmdDisposition {
    match program {
        // Read-only tools — always allowed
        "rg" | "grep" | "find" | "ls" | "cat" | "head" | "tail" | "wc" | "file" | "which"
        | "where" | "type" | "echo" | "printf" | "realpath" | "pwd" | "readlink" | "stat"
        | "du" | "df" | "id" | "whoami" | "uname" | "date" => CmdDisposition::Allow,

        // Rust build/test — `cargo build` and `cargo bench` are reviewed because
        // build scripts and benches execute arbitrary code; safe subcommands are
        // allowed.
        "cargo" | "rustc" | "rustup" => match subcommand {
            "check" | "test" | "clippy" | "doc" | "search" | "version" | "list" | "tree"
            | "loc" => CmdDisposition::Allow,
            "fmt" | "fix" | "clean" | "publish" | "build" | "bench" => CmdDisposition::Review,
            _ => CmdDisposition::Review,
        },

        // Git — read operations allowed, write operations reviewed
        "git" => match subcommand {
            "status" | "diff" | "log" | "show" | "describe" | "rev-parse" | "remote" | "tag"
            | "blame" | "shortlog" => CmdDisposition::Allow,
            "add" | "checkout" | "restore" | "merge" | "rebase" | "cherry-pick" | "commit"
            | "fetch" | "pull" | "push" | "reset" | "revert" | "clean" | "config" | "branch"
            | "stash" => CmdDisposition::Review,
            _ => CmdDisposition::Review,
        },

        // Package managers
        "npm" | "yarn" | "pnpm" | "bun" => match subcommand {
            "list" | "ls" | "outdated" | "version" => CmdDisposition::Allow,
            "install" | "ci" | "update" | "add" | "remove" | "run" => CmdDisposition::Review,
            _ => CmdDisposition::Review,
        },
        "pip" | "pip3" | "python" | "python3" => match subcommand {
            "-c" | "-m" => CmdDisposition::Allow,
            "install" | "uninstall" | "upgrade" => CmdDisposition::Review,
            _ => CmdDisposition::Review,
        },

        // Network tools — review
        "curl" | "wget" | "http" | "https" => CmdDisposition::Review,

        // Destructive tools — blocked
        "rm" | "rmdir" | "shred" | "wipefs" => CmdDisposition::Block,
        "chmod" | "chown" | "chgrp" => CmdDisposition::Block,
        "dd" | "mkfs" | "fdisk" | "parted" => CmdDisposition::Block,
        "sudo" | "su" | "doas" | "pkexec" => CmdDisposition::Block,

        // Process control — review
        "kill" | "pkill" | "killall" | "xkill" => CmdDisposition::Review,
        "nohup" | "screen" | "tmux" | "setsid" => CmdDisposition::Review,

        _ => CmdDisposition::Review,
    }
}

/// Strict policy: only explicitly safe commands are allowed.
fn classify_strict(program: &str, subcommand: &str) -> CmdDisposition {
    match program {
        "cargo" | "rustc" | "rustup" => match subcommand {
            "check" | "test" | "clippy" | "fmt" | "doc" | "search" | "version" | "list"
            | "tree" | "loc" => CmdDisposition::Allow,
            _ => CmdDisposition::Review,
        },
        "git" => match subcommand {
            "status" | "diff" | "log" | "show" | "describe" | "rev-parse" => CmdDisposition::Allow,
            _ => CmdDisposition::Review,
        },
        "rg" | "grep" | "find" | "ls" | "cat" | "head" | "tail" | "wc" | "which" | "echo"
        | "pwd" | "uname" | "date" => CmdDisposition::Allow,
        "rm" | "rmdir" | "shred" | "chmod" | "chown" | "chgrp" | "dd" | "mkfs" | "sudo" | "su"
        | "doas" => CmdDisposition::Block,
        _ => CmdDisposition::Review,
    }
}

/// Permissive policy: only block clearly destructive or exfiltration patterns.
fn classify_permissive(program: &str, _subcommand: &str) -> CmdDisposition {
    match program {
        "rm" | "rmdir" | "shred" | "wipefs" | "dd" | "mkfs" | "fdisk" | "parted" | "chmod"
        | "chown" | "chgrp" | "sudo" | "su" | "doas" | "pkexec" => CmdDisposition::Block,
        _ => CmdDisposition::Allow,
    }
}

/// Select the classification function based on policy mode.
fn classify(program: &str, subcommand: &str, policy: &str) -> CmdDisposition {
    match policy {
        "strict" => classify_strict(program, subcommand),
        "permissive" => classify_permissive(program, subcommand),
        _ => classify_default(program, subcommand),
    }
}

/// Detect behavioral features of a command that may trigger findings.
fn detect_behavioral_features(
    argv: &[String],
    features: &Value,
) -> Vec<(&'static str, &'static str, &'static str)> {
    let mut result: Vec<(&str, &str, &str)> = Vec::new();
    if argv.is_empty() {
        return result;
    }
    let program = argv[0].as_str();

    // Network access detection
    let network_programs = [
        "curl",
        "wget",
        "http",
        "https",
        "nc",
        "ncat",
        "socat",
        "ssh",
        "scp",
        "sftp",
        "rsync",
        "telnet",
        "ftp",
        "nmap",
        "ping",
        "traceroute",
        "dig",
        "nslookup",
        "host",
    ];
    if network_programs.contains(&program) {
        result.push((
            machine_codes::SHELL_NETWORK_ACCESS,
            "NetworkAccess",
            "command accesses the network",
        ));
    }

    // Filesystem write detection
    let fs_write_programs = [
        "rm", "rmdir", "shred", "wipefs", "dd", "mkfs", "fdisk", "parted",
    ];
    let fs_write_subcommands = ["write", "create", "delete", "remove", "unlink"];
    if fs_write_programs.contains(&program) {
        result.push((
            machine_codes::SHELL_FILESYSTEM_WRITE,
            "FilesystemWrite",
            "command performs filesystem write/deletion",
        ));
    }
    if argv.len() > 1 && fs_write_subcommands.contains(&argv[1].as_str()) {
        result.push((
            machine_codes::SHELL_FILESYSTEM_WRITE,
            "FilesystemWrite",
            "subcommand performs filesystem write/deletion",
        ));
    }

    // Privilege escalation detection
    let priv_esc_programs = ["sudo", "su", "doas", "pkexec", "runas"];
    if priv_esc_programs.contains(&program) {
        result.push((
            machine_codes::SHELL_PRIVILEGE_ESCALATION,
            "PrivilegeEscalation",
            "command attempts privilege escalation",
        ));
    }

    // Process control detection
    let proc_control_programs = [
        "kill", "pkill", "killall", "xkill", "nohup", "screen", "tmux", "setsid",
    ];
    if proc_control_programs.contains(&program) {
        result.push((
            machine_codes::SHELL_PROCESS_CONTROL,
            "ProcessControl",
            "command controls system processes",
        ));
    }

    // Environment mutation detection (FOO=bar cmd pattern)
    if let Some(first) = argv.first() {
        if first.contains('=') && !first.starts_with('-') {
            result.push((
                machine_codes::SHELL_ENV_MUTATION,
                "EnvMutation",
                "command mutates environment variables",
            ));
        }
    }

    // Shell feature-based detection
    if let Some(obj) = features.as_object() {
        if obj
            .get("has_command_substitution")
            .and_then(|v| v.as_bool())
            == Some(true)
        {
            result.push((
                machine_codes::SHELL_COMMAND_SUBSTITUTION,
                "CommandSubstitution",
                "command uses command substitution",
            ));
        }
        if obj.get("has_redirection").and_then(|v| v.as_bool()) == Some(true) {
            result.push((
                machine_codes::SHELL_REDIRECTION,
                "Redirection",
                "command uses I/O redirection",
            ));
        }
        if obj.get("has_pipe").and_then(|v| v.as_bool()) == Some(true) {
            result.push((
                machine_codes::SHELL_PIPELINE,
                "Pipeline",
                "command uses a pipeline",
            ));
        }
        if obj.get("has_background").and_then(|v| v.as_bool()) == Some(true) {
            result.push((
                machine_codes::SHELL_BACKGROUND_EXECUTION,
                "BackgroundExecution",
                "command uses background execution (&)",
            ));
        }
    }

    result
}

/// Priority-ordered primary code selection.
/// Returns the highest-priority machine code from the collected codes.
fn select_primary_code(codes: &[String]) -> String {
    let priority = [
        machine_codes::SHELL_PARSE_ERROR,
        machine_codes::SHELL_DESTRUCTIVE_COMMAND,
        machine_codes::SHELL_PRIVILEGE_ESCALATION,
        machine_codes::SHELL_NETWORK_ACCESS,
        machine_codes::SHELL_FILESYSTEM_WRITE,
        machine_codes::SHELL_PROCESS_CONTROL,
        machine_codes::SHELL_COMMAND_SUBSTITUTION,
        machine_codes::SHELL_REDIRECTION,
        machine_codes::SHELL_PIPELINE,
        machine_codes::SHELL_BACKGROUND_EXECUTION,
        machine_codes::SHELL_UNAPPROVED_COMMAND,
        machine_codes::SHELL_RISK,
        machine_codes::REGEX_RISK,
    ];
    for code in priority {
        if codes.iter().any(|c| c == code) {
            return code.to_string();
        }
    }
    machine_codes::COMMAND_OK.to_string()
}

/// Check for destructive shell patterns that should always block.
fn check_destructive_patterns(
    command: &str,
    argv: &[String],
) -> Vec<(&'static str, &'static str, &'static str)> {
    let mut result = Vec::new();
    let lower = command.to_lowercase();

    // Pipe-to-shell pattern: curl/wget ... | sh/bash/zsh
    if lower.contains("| sh")
        || lower.contains("| bash")
        || lower.contains("| zsh")
        || lower.contains("| python")
        || lower.contains("| perl")
        || lower.contains("| ruby")
    {
        result.push((
            machine_codes::SHELL_DESTRUCTIVE_COMMAND,
            "PipeToShell",
            "piping output to a shell interpreter is dangerous",
        ));
    }

    // rm -rf / or similar
    if argv.len() >= 2 && argv[0] == "rm" {
        let has_force_recursive = argv
            .iter()
            .any(|a| a == "-rf" || a == "-fr" || a == "-r" || a == "-f");
        let has_root = argv.iter().any(|a| a == "/" || a == "/*" || a == ".");
        if has_force_recursive && has_root {
            result.push((
                machine_codes::SHELL_DESTRUCTIVE_COMMAND,
                "DestructiveRemove",
                "rm -rf on root/parent paths is destructive",
            ));
        }
    }

    // git reset --hard, git clean -fdx, git push --force
    if argv.len() >= 3 && argv[0] == "git" {
        match argv[1].as_str() {
            "reset" if argv[2] == "--hard" => {
                result.push((
                    machine_codes::SHELL_DESTRUCTIVE_COMMAND,
                    "DestructiveGitReset",
                    "git reset --hard discards uncommitted changes",
                ));
            }
            "clean" => {
                let has_force = argv
                    .iter()
                    .any(|a| a == "-f" || a == "-fd" || a == "-fdx" || a == "-xdf");
                if has_force {
                    result.push((
                        machine_codes::SHELL_DESTRUCTIVE_COMMAND,
                        "DestructiveGitClean",
                        "git clean -f removes untracked files",
                    ));
                }
            }
            "push" => {
                let has_force = argv
                    .iter()
                    .any(|a| a == "--force" || a == "-f" || a == "--force-with-lease");
                if has_force {
                    result.push((
                        machine_codes::SHELL_DESTRUCTIVE_COMMAND,
                        "ForceGitPush",
                        "force-pushing rewrites remote history",
                    ));
                }
            }
            _ => {}
        }
    }

    // chmod -R 777 or chown -R
    if argv.len() >= 2 && argv[0] == "chmod" {
        let has_recursive = argv.iter().any(|a| a == "-R" || a == "--recursive");
        let has_permissive = argv
            .iter()
            .any(|a| a == "777" || a == "a+rwx" || a == "u+s");
        if has_recursive && has_permissive {
            result.push((
                machine_codes::SHELL_DESTRUCTIVE_COMMAND,
                "PermissiveChmod",
                "chmod -R 777 opens permissions globally",
            ));
        }
    }
    if argv.len() >= 2 && argv[0] == "chown" {
        let has_recursive = argv.iter().any(|a| a == "-R" || a == "--recursive");
        if has_recursive {
            result.push((
                machine_codes::SHELL_DESTRUCTIVE_COMMAND,
                "RecursiveChown",
                "recursive chown changes ownership broadly",
            ));
        }
    }

    result
}

pub fn command_preflight(args: &Value) -> ToolResponse {
    let budget_ctx = crate::mcp::budget::for_handler(crate::mcp::budget::ToolBudget::HEAVY);

    let command = match args.get("command").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing 'command' parameter",
                None,
                Some("command_preflight"),
            )
        }
    };
    let platform = args
        .get("platform")
        .and_then(|v| v.as_str())
        .unwrap_or("posix");
    let policy = args
        .get("policy")
        .and_then(|v| v.as_str())
        .unwrap_or("default");
    let _working_directory = args.get("working_directory").and_then(|v| v.as_str());

    if command.chars().count() > MAX_TEXT_LENGTH {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Command exceeds {} chars", MAX_TEXT_LENGTH),
            None,
            Some("command_preflight"),
        );
    }

    let valid_platforms = ["posix", "windows", "auto"];
    if !valid_platforms.contains(&platform) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported platform: {}", platform),
            Some(vec![format!("Use one of: {}", valid_platforms.join(", "))]),
            Some("command_preflight"),
        );
    }

    let valid_policies = ["default", "strict", "permissive"];
    if !valid_policies.contains(&policy) {
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Unsupported policy: {}", policy),
            Some(vec![format!("Use one of: {}", valid_policies.join(", "))]),
            Some("command_preflight"),
        );
    }

    // Parse optional policy_config (structured override on top of policy enum)
    let policy_config = args.get("policy_config").cloned();
    let max_command_length = policy_config
        .as_ref()
        .and_then(|pc| pc.get("max_command_length").and_then(|v| v.as_u64()))
        .unwrap_or(MAX_TEXT_LENGTH as u64);

    if command.chars().count() > max_command_length as usize {
        return ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INVALID_ARGUMENTS,
            &format!("Command exceeds {} chars", max_command_length),
            None,
            Some("command_preflight"),
        );
    }

    let mut subresults = serde_json::Map::new();
    let mut findings: Vec<serde_json::Value> = Vec::new();
    let mut code_list: Vec<String> = Vec::new();
    let mut matched_rules: Vec<String> = Vec::new();

    // 1. Parse command via shell_split
    if platform == "windows" {
        return ToolResponse::error_with_code(
            "unsupported_platform",
            machine_codes::UNSUPPORTED_FEATURE,
            "Windows shell splitting is not supported; only 'posix' is available",
            Some(vec!["Use platform='posix' or platform='auto'".to_string()]),
            Some("command_preflight"),
        );
    }

    if budget_ctx.should_stop() {
        return budget_ctx
            .check_should_stop("command_preflight")
            .unwrap_err();
    }

    let shell = "posix";
    let ss_args = serde_json::json!({"command": command, "shell": shell});
    let ss_result = shell_split(&ss_args);
    if let Some(ref r) = ss_result.result {
        subresults.insert(
            "shell_split".to_string(),
            serde_json::json!({
                "argv": r.get("argv").cloned().unwrap_or(serde_json::json!([])),
                "features": r.get("features").cloned().unwrap_or(serde_json::json!({})),
            }),
        );
    } else if let Some(ref e) = ss_result.error {
        code_list.push(machine_codes::SHELL_PARSE_ERROR.to_string());
        findings.push(finding(
            "SHELL_PARSE_ERROR",
            severity::HIGH,
            e,
            Some(disposition::BLOCKING),
            None,
        ));
        matched_rules.push("parse_error".to_string());
    }

    // Extract argv and features for downstream analysis
    let argv: Vec<String> = subresults
        .get("shell_split")
        .and_then(|r| r.get("argv"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let shell_features = subresults
        .get("shell_split")
        .and_then(|r| r.get("features"))
        .cloned()
        .unwrap_or(serde_json::json!({}));

    let program = argv.first().map(|s| s.as_str()).unwrap_or("");
    let subcommand = if argv.len() > 1 { argv[1].as_str() } else { "" };

    // 2. Apply policy_config custom allow/deny rules (deny beats allow)
    if let Some(ref config) = policy_config {
        let deny_commands: Vec<String> = config
            .get("deny_commands")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let allow_commands: Vec<String> = config
            .get("allow_commands")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        if deny_commands.iter().any(|d| d == program) {
            code_list.push(machine_codes::SHELL_UNAPPROVED_COMMAND.to_string());
            findings.push(finding(
                "SHELL_UNAPPROVED_COMMAND",
                severity::HIGH,
                &format!("'{}' is in the deny list", program),
                Some(disposition::BLOCKING),
                None,
            ));
            matched_rules.push("deny_command".to_string());
        } else if !allow_commands.is_empty() && !allow_commands.iter().any(|a| a == program) {
            code_list.push(machine_codes::SHELL_UNAPPROVED_COMMAND.to_string());
            findings.push(finding(
                "SHELL_UNAPPROVED_COMMAND",
                severity::MEDIUM,
                &format!("'{}' is not in the allow list", program),
                Some(disposition::CAUTION),
                None,
            ));
            matched_rules.push("not_in_allow_list".to_string());
        }

        // Subcommand-level allow/deny
        let deny_subcommands: std::collections::HashMap<String, Vec<String>> = config
            .get("deny_subcommands")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| {
                        v.as_array().map(|arr| {
                            (
                                k.clone(),
                                arr.iter()
                                    .filter_map(|v| v.as_str().map(String::from))
                                    .collect(),
                            )
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        if let Some(denied_subs) = deny_subcommands.get(program) {
            if denied_subs.iter().any(|s| s == subcommand) {
                code_list.push(machine_codes::SHELL_UNAPPROVED_COMMAND.to_string());
                findings.push(finding(
                    "SHELL_UNAPPROVED_COMMAND",
                    severity::HIGH,
                    &format!(
                        "'{} {}' is in the deny subcommands list",
                        program, subcommand
                    ),
                    Some(disposition::BLOCKING),
                    None,
                ));
                matched_rules.push("deny_subcommand".to_string());
            }
        }
    }

    // 3. Apply built-in policy classification (deny rules beat allow rules)
    let classify_disposition =
        if !code_list.contains(&machine_codes::SHELL_UNAPPROVED_COMMAND.to_string()) {
            classify(program, subcommand, policy)
        } else {
            CmdDisposition::Block
        };

    // If policy classifies as block and we don't already have a block finding, add it
    if classify_disposition == CmdDisposition::Block
        && !findings
            .iter()
            .any(|f| f.get("severity").and_then(|v| v.as_str()) == Some(severity::HIGH))
    {
        code_list.push(machine_codes::SHELL_UNAPPROVED_COMMAND.to_string());
        findings.push(finding(
            "SHELL_UNAPPROVED_COMMAND",
            severity::HIGH,
            &format!("'{}' is not permitted under {} policy", program, policy),
            Some(disposition::BLOCKING),
            None,
        ));
        matched_rules.push("policy_classify_block".to_string());
    } else if classify_disposition == CmdDisposition::Review
        && !findings
            .iter()
            .any(|f| f.get("severity").and_then(|v| v.as_str()) == Some(severity::HIGH))
        && !findings
            .iter()
            .any(|f| f.get("severity").and_then(|v| v.as_str()) == Some(severity::MEDIUM))
    {
        // Only add review finding if no higher-severity finding already present
        matched_rules.push("policy_classify_review".to_string());
        findings.push(finding(
            machine_codes::SHELL_POLICY_REVIEW,
            severity::MEDIUM,
            &format!("'{}' requires review under {} policy", program, policy),
            Some(disposition::CAUTION),
            None,
        ));
    } else if classify_disposition == CmdDisposition::Allow {
        matched_rules.push("policy_classify_allow".to_string());
    }

    // 4. Detect destructive patterns (always block regardless of policy)
    let destructive = check_destructive_patterns(command, &argv);
    for (code, kind, message) in &destructive {
        if !code_list.contains(&code.to_string()) {
            code_list.push(code.to_string());
        }
        findings.push(finding(
            kind,
            severity::HIGH,
            message,
            Some(disposition::BLOCKING),
            None,
        ));
        matched_rules.push(format!("destructive:{}", kind));
    }

    // 5. Detect behavioral features (network, filesystem, process, env, shell features)
    let behavioral = detect_behavioral_features(&argv, &shell_features);

    // 5a. Filter behavioral findings based on policy_config allow_* overrides
    let allow_network = policy_config
        .as_ref()
        .and_then(|pc| pc.get("allow_network").and_then(|v| v.as_bool()))
        .unwrap_or(false);
    let allow_filesystem_write = policy_config
        .as_ref()
        .and_then(|pc| pc.get("allow_filesystem_write").and_then(|v| v.as_bool()))
        .unwrap_or(false);
    let allow_process_control = policy_config
        .as_ref()
        .and_then(|pc| pc.get("allow_process_control").and_then(|v| v.as_bool()))
        .unwrap_or(false);
    let allow_env_mutation = policy_config
        .as_ref()
        .and_then(|pc| pc.get("allow_env_mutation").and_then(|v| v.as_bool()))
        .unwrap_or(false);

    let behavioral: Vec<_> = behavioral
        .into_iter()
        .filter(|(code, _, _)| {
            if allow_network && *code == machine_codes::SHELL_NETWORK_ACCESS {
                matched_rules.push("allow_network".to_string());
                return false;
            }
            if allow_filesystem_write && *code == machine_codes::SHELL_FILESYSTEM_WRITE {
                matched_rules.push("allow_filesystem_write".to_string());
                return false;
            }
            if allow_process_control && *code == machine_codes::SHELL_PROCESS_CONTROL {
                matched_rules.push("allow_process_control".to_string());
                return false;
            }
            if allow_env_mutation && *code == machine_codes::SHELL_ENV_MUTATION {
                matched_rules.push("allow_env_mutation".to_string());
                return false;
            }
            true
        })
        .collect();
    for (code, kind, message) in &behavioral {
        let (sev, disp) = match *code {
            c if c == machine_codes::SHELL_PRIVILEGE_ESCALATION
                || c == machine_codes::SHELL_NETWORK_ACCESS =>
            {
                if policy == "strict" {
                    (severity::HIGH, disposition::BLOCKING)
                } else {
                    (severity::MEDIUM, disposition::CAUTION)
                }
            }
            _ => {
                if policy == "strict" {
                    (severity::MEDIUM, disposition::CAUTION)
                } else {
                    (severity::INFO, disposition::INFORMATIONAL)
                }
            }
        };
        if !code_list.contains(&code.to_string()) {
            code_list.push(code.to_string());
        }
        findings.push(finding(kind, sev, message, Some(disp), None));
        matched_rules.push(format!("feature:{}", kind));
    }

    // 6. Emit shell feature findings (pipe, redirect, command substitution, etc.)
    if budget_ctx.should_stop() {
        return budget_ctx
            .check_should_stop("command_preflight")
            .unwrap_err();
    }

    if let Some(obj) = shell_features.as_object() {
        let risky: Vec<&String> = obj
            .iter()
            .filter(|(_, v)| v.as_bool() == Some(true))
            .map(|(k, _)| k)
            .collect();
        for rf in &risky {
            let (sev, disp) = if policy == "strict" {
                (severity::HIGH, disposition::BLOCKING)
            } else {
                (severity::MEDIUM, disposition::CAUTION)
            };
            findings.push(finding("RISKY_SHELL_FEATURE", sev, rf, Some(disp), None));
        }
        if !risky.is_empty() && !code_list.contains(&machine_codes::SHELL_RISK.to_string()) {
            code_list.push(machine_codes::SHELL_RISK.to_string());
        }
    }

    // 7. Check for regex-like args in the command
    if budget_ctx.should_stop() {
        return budget_ctx
            .check_should_stop("command_preflight")
            .unwrap_err();
    }

    let looks_like_regex = command.contains("grep")
        || command.contains("sed")
        || command.contains("awk")
        || command.to_lowercase().contains("regex");

    if looks_like_regex && !argv.is_empty() {
        let regex_metachars: std::collections::HashSet<char> = ".*+?[]|^$\\(){}".chars().collect();
        let regex_args: Vec<&String> = argv
            .iter()
            .filter(|arg| {
                !arg.starts_with('-')
                    && !arg.is_empty()
                    && arg.chars().any(|c| regex_metachars.contains(&c))
            })
            .collect();
        for pattern in &regex_args {
            let rs_args = serde_json::json!({"pattern": pattern.as_str()});
            let rs_result = crate::tools::regex_safety_check_tool(&rs_args);
            if let Some(ref r) = rs_result.result {
                let risk = r.get("risk").and_then(|v| v.as_str()).unwrap_or("none");
                let mut has_rs_findings = false;
                if let Some(findings_arr) = r.get("findings").and_then(|v| v.as_array()) {
                    has_rs_findings = !findings_arr.is_empty();
                    for f in findings_arr {
                        let (sev, disp) = if risk != "none" {
                            (severity::MEDIUM, disposition::CAUTION)
                        } else {
                            (severity::INFO, disposition::INFORMATIONAL)
                        };
                        let kind = f
                            .get("kind")
                            .and_then(|v| v.as_str())
                            .unwrap_or("REGEX_RISK");
                        findings.push(finding(
                            &kind.to_uppercase(),
                            sev,
                            f.get("message").and_then(|v| v.as_str()).unwrap_or(""),
                            Some(disp),
                            None,
                        ));
                    }
                }
                if has_rs_findings
                    && risk != "none"
                    && !code_list.contains(&machine_codes::REGEX_RISK.to_string())
                {
                    code_list.push(machine_codes::REGEX_RISK.to_string());
                }
                let regex_summary = serde_json::json!({
                    "pattern": pattern.as_str(),
                    "findings_count": r.get("findings").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0),
                    "risk": risk,
                });
                let regex_subresults = subresults
                    .entry("regex_safety_check".to_string())
                    .or_insert_with(|| serde_json::json!([]));
                if let Some(items) = regex_subresults.as_array_mut() {
                    items.push(regex_summary);
                } else {
                    *regex_subresults = serde_json::json!([regex_summary]);
                }
            }
        }
    }

    // 8. Determine verdict based on findings severity
    let has_critical = findings
        .iter()
        .any(|f| f.get("severity").and_then(|v| v.as_str()) == Some(severity::CRITICAL));
    let has_error = findings
        .iter()
        .any(|f| f.get("severity").and_then(|v| v.as_str()) == Some(severity::HIGH));
    let has_warn = findings
        .iter()
        .any(|f| f.get("severity").and_then(|v| v.as_str()) == Some(severity::MEDIUM));
    let response_verdict = if has_critical || has_error {
        verdict::BLOCK
    } else if has_warn {
        verdict::REVIEW
    } else {
        verdict::ALLOW
    };

    // 9. Select primary machine code by priority
    let has_parse_error = code_list.contains(&machine_codes::SHELL_PARSE_ERROR.to_string());
    let unique_codes: Vec<String> = code_list.into_iter().fold(Vec::new(), |mut acc, c| {
        if !acc.contains(&c) {
            acc.push(c);
        }
        acc
    });
    let primary_code = select_primary_code(&unique_codes);

    let feature_names: Vec<String> = shell_features
        .as_object()
        .map(|obj| {
            obj.iter()
                .filter(|(_, v)| v.as_bool() == Some(true))
                .map(|(k, _)| k.clone())
                .collect()
        })
        .unwrap_or_default();

    let summary = format!(
        "Command {} under {} policy ({} finding(s))",
        response_verdict,
        policy,
        findings.len()
    );

    let mut result = serde_json::json!({
        "verdict": response_verdict,
        "command": command,
        "platform": platform,
        "policy": policy,
        "program": program,
        "subcommand": subcommand,
        "features": feature_names,
        "findings": findings,
        "matched_rules": matched_rules,
        "machine_code": primary_code,
        "summary": summary,
    });
    if let Some(wd) = _working_directory {
        result["working_directory"] = serde_json::json!(wd);
    }
    if !subresults.is_empty() {
        result["subresults"] = serde_json::Value::Object(subresults);
    }

    let mut resp =
        ToolResponse::success(result, Some("command_preflight")).with_tool("command_preflight");
    resp = resp
        .with_machine_code(&primary_code)
        .with_verdict(response_verdict);
    if !findings.is_empty() {
        resp = resp.with_findings(findings);
    }

    // 10. Emit recommended_next_tool when appropriate
    let has_unbalanced = shell_features
        .as_object()
        .and_then(|o| o.get("has_unbalanced_quotes"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let has_non_ascii = !command.is_ascii();
    if has_unbalanced || has_parse_error {
        resp = resp.with_recommended_next_tool(ToolResponse::next_tool(
            "shell_split",
            "command has ambiguous parsing — verify with shell_split",
            Some(serde_json::json!({"command": command, "shell": shell})),
        ));
    } else if has_non_ascii {
        resp = resp.with_recommended_next_tool(ToolResponse::next_tool(
            "text_security_inspect",
            "command contains non-ASCII characters — run unicode security check",
            Some(serde_json::json!({"text": command})),
        ));
    }
    resp
}
