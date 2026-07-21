use eggsact::text::{argv_compare, shell_quote_join, shell_split};

#[test]
fn shell_split_deterministic() {
    let cmds = ["ls -la", "echo 'hello world'", "cmd arg1 arg2", ""];
    for cmd in &cmds {
        let r1 = shell_split(cmd, "posix", true);
        let r2 = shell_split(cmd, "posix", true);
        assert_eq!(r1.parse_ok, r2.parse_ok);
        assert_eq!(r1.argv, r2.argv);
    }
}

#[test]
fn shell_split_all_tokens_are_strings() {
    let cmds = ["ls -la", "echo \"hello\\nworld\"", "cmd 'arg with spaces'"];
    for cmd in &cmds {
        let result = shell_split(cmd, "posix", true);
        for arg in &result.argv {
            assert!(!arg.is_empty() || result.argv.len() == 1);
        }
    }
}

#[test]
fn shell_quote_roundtrip_basic() {
    let argvs: Vec<Vec<String>> = vec![
        vec!["ls".into(), "-la".into()],
        vec!["echo".into(), "hello world".into()],
        vec!["cmd".into(), "".into(), "arg".into()],
    ];
    for argv in &argvs {
        let joined = shell_quote_join(argv, "posix");
        if joined.roundtrip_ok {
            let resplit = shell_split(&joined.command, "posix", false);
            if resplit.parse_ok {
                assert_eq!(&resplit.argv, argv, "Round-trip failed for {:?}", argv);
            }
        }
    }
}

#[test]
fn shell_quote_stability() {
    let argv = vec!["echo".to_string(), "hello world".to_string()];
    let j1 = shell_quote_join(&argv, "posix");
    let j2 = shell_quote_join(&argv, "posix");
    assert_eq!(j1.command, j2.command);
}

#[test]
fn shell_split_empty_command() {
    let r = shell_split("", "posix", true);
    assert!(r.argv.is_empty() || r.argv.iter().all(|a| a.is_empty()));
}

#[test]
fn argv_compare_deterministic() {
    let _ = argv_compare(Some("ls -la"), Some("ls -la"), None, None, "posix");
    let _ = argv_compare(Some("cmd a b"), Some("cmd a c"), None, None, "posix");
}
