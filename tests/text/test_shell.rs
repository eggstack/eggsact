use eggsact::text::{argv_compare, shell_quote_join, shell_split};

// ─── shell_split ─────────────────────────────────────────────────────

#[test]
fn test_shell_split_simple() {
    let result = shell_split("ls -la /tmp", "posix", false);
    assert!(result.parse_ok);
    assert_eq!(result.argc, 3);
    assert_eq!(result.argv, vec!["ls", "-la", "/tmp"]);
}

#[test]
fn test_shell_split_single_command() {
    let result = shell_split("ls", "posix", false);
    assert!(result.parse_ok);
    assert_eq!(result.argc, 1);
    assert_eq!(result.argv, vec!["ls"]);
}

#[test]
fn test_shell_split_empty() {
    let result = shell_split("", "posix", false);
    assert!(result.parse_ok);
    assert_eq!(result.argc, 0);
    assert!(result.argv.is_empty());
}

#[test]
fn test_shell_split_single_quotes() {
    let result = shell_split("echo 'hello world'", "posix", false);
    assert!(result.parse_ok);
    assert_eq!(result.argv, vec!["echo", "hello world"]);
}

#[test]
fn test_shell_split_double_quotes() {
    let result = shell_split(r#"echo "hello world""#, "posix", false);
    assert!(result.parse_ok);
    assert_eq!(result.argv, vec!["echo", "hello world"]);
}

#[test]
fn test_shell_split_mixed_quotes() {
    let result = shell_split(r#"echo "it's a test""#, "posix", false);
    assert!(result.parse_ok);
    assert_eq!(result.argv, vec!["echo", "it's a test"]);
}

#[test]
fn test_shell_split_empty_args() {
    let result = shell_split("echo ''", "posix", false);
    assert!(result.parse_ok);
    assert_eq!(result.argv, vec!["echo", ""]);
}

#[test]
fn test_shell_split_features_pipe() {
    let result = shell_split("ls | grep foo", "posix", true);
    assert!(result.features.has_pipe);
}

#[test]
fn test_shell_split_features_redirection() {
    let result = shell_split("echo hello > out.txt", "posix", true);
    assert!(result.features.has_redirection);
}

#[test]
fn test_shell_split_features_variable_expansion() {
    let result = shell_split("echo $HOME", "posix", true);
    assert!(result.features.has_variable_expansion);
}

#[test]
fn test_shell_split_features_variable_in_single_quotes() {
    // $HOME inside single quotes should NOT be flagged as variable expansion
    let result = shell_split("echo '$HOME'", "posix", true);
    assert!(!result.features.has_variable_expansion);
}

#[test]
fn test_shell_split_features_glob() {
    let result = shell_split("ls *.txt", "posix", true);
    assert!(result.features.has_glob_pattern);
}

#[test]
fn test_shell_split_features_command_substitution() {
    let result = shell_split("echo $(date)", "posix", true);
    assert!(result.features.has_command_substitution);
}

#[test]
fn test_shell_split_backticks() {
    let result = shell_split("echo `date`", "posix", true);
    assert!(result.features.has_command_substitution);
}

#[test]
fn test_shell_split_unsupported_shell() {
    let result = shell_split("ls -la", "bash", false);
    assert!(!result.parse_ok);
}

#[test]
fn test_shell_split_unbalanced_quotes() {
    let result = shell_split("echo 'hello", "posix", true);
    assert!(result.features.has_unbalanced_quotes);
}

// ─── BUG-209: shell_split must not treat `#` as comment inside a word ───
// POSIX §2.3: a word beginning with `#` introduces a comment. A `#` that
// appears mid-word is a literal character and belongs to the current token.

#[test]
fn test_bug209_shell_split_hash_inside_word_kept() {
    let result = shell_split("echo foo#bar", "posix", false);
    assert!(result.parse_ok);
    assert_eq!(result.argv, vec!["echo", "foo#bar"]);
}

#[test]
fn test_bug209_shell_split_hash_starts_comment_after_whitespace() {
    let result = shell_split("echo hi # comment", "posix", false);
    assert!(result.parse_ok);
    assert_eq!(result.argv, vec!["echo", "hi"]);
}

#[test]
fn test_bug209_shell_split_hash_at_start_is_comment() {
    let result = shell_split("# only a comment", "posix", false);
    assert!(result.parse_ok);
    assert!(result.argv.is_empty());
}

// ─── shell_quote_join ────────────────────────────────────────────────

#[test]
fn test_shell_quote_join_simple() {
    let argv = vec!["ls".to_string(), "-la".to_string(), "/tmp".to_string()];
    let result = shell_quote_join(&argv, "posix");
    assert!(result.command.contains("ls"));
    assert!(result.command.contains("-la"));
}

#[test]
fn test_shell_quote_join_needs_quoting() {
    let argv = vec!["echo".to_string(), "hello world".to_string()];
    let result = shell_quote_join(&argv, "posix");
    assert!(result.command.contains("hello"));
}

#[test]
fn test_shell_quote_join_empty() {
    let argv: Vec<String> = vec![];
    let result = shell_quote_join(&argv, "posix");
    assert!(result.command.is_empty() || result.command.trim().is_empty());
}

#[test]
fn test_shell_quote_join_single_arg() {
    let argv = vec!["ls".to_string()];
    let result = shell_quote_join(&argv, "posix");
    assert!(result.command.contains("ls"));
}

// ─── argv_compare ────────────────────────────────────────────────────

#[test]
fn test_argv_compare_equal() {
    let left = vec!["ls".to_string(), "-la".to_string()];
    let right = vec!["ls".to_string(), "-la".to_string()];
    let result = argv_compare(None, None, Some(&left), Some(&right), "posix");
    assert!(result.argv_equal);
    assert!(result.first_difference.is_none());
}

#[test]
fn test_argv_compare_different() {
    let left = vec!["ls".to_string(), "-la".to_string()];
    let right = vec!["ls".to_string(), "-l".to_string()];
    let result = argv_compare(None, None, Some(&left), Some(&right), "posix");
    assert!(!result.argv_equal);
    assert_eq!(result.first_difference, Some(1));
}

#[test]
fn test_argv_compare_from_commands() {
    let result = argv_compare(Some("ls -la"), Some("ls -la"), None, None, "posix");
    assert!(result.argv_equal);
}

#[test]
fn test_argv_compare_different_commands() {
    let result = argv_compare(Some("ls -la"), Some("ls -l"), None, None, "posix");
    assert!(!result.argv_equal);
}

#[test]
fn test_argv_compare_length_mismatch() {
    let left = vec!["ls".to_string()];
    let right = vec!["ls".to_string(), "-la".to_string()];
    let result = argv_compare(None, None, Some(&left), Some(&right), "posix");
    assert!(!result.argv_equal);
    assert!(result.first_difference.is_some());
}

#[test]
fn test_argv_compare_empty_both() {
    let result = argv_compare(None, None, Some(&[]), Some(&[]), "posix");
    assert!(result.argv_equal);
}

#[test]
fn test_argv_compare_case_sensitive() {
    let left = vec!["LS".to_string()];
    let right = vec!["ls".to_string()];
    let result = argv_compare(None, None, Some(&left), Some(&right), "posix");
    assert!(!result.argv_equal);
}
