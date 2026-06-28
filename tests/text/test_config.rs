use eggsact::text::{dotenv_validate, ini_validate};

// ─── dotenv_validate ─────────────────────────────────────────────────

#[test]
fn test_dotenv_validate_valid() {
    let text = "KEY=value\nOTHER=123";
    let result = dotenv_validate(text, false, "^[A-Z_]+$", "error");
    assert!(result.parse_ok);
}

#[test]
fn test_dotenv_validate_empty() {
    let result = dotenv_validate("", false, ".*", "error");
    assert!(result.parse_ok);
}

#[test]
fn test_dotenv_validate_comments() {
    let text = "# comment\nKEY=value\n# another comment";
    let result = dotenv_validate(text, false, "^[A-Z_]+$", "error");
    assert!(result.parse_ok);
}

#[test]
fn test_dotenv_validate_export_prefix() {
    let text = "export KEY=value";
    let result = dotenv_validate(text, true, "^[A-Z_]+$", "error");
    assert!(!result.entries.is_empty());
}

#[test]
fn test_dotenv_validate_export_not_allowed() {
    let text = "export KEY=value";
    let result = dotenv_validate(text, false, "^[A-Z_]+$", "error");
    assert!(!result.parse_ok);
    assert!(!result.invalid_lines.is_empty());
    assert!(result
        .invalid_lines
        .iter()
        .any(|il| il.reason.contains("export")));
}

#[test]
fn test_dotenv_validate_quoted_values() {
    let text = r#"KEY="value with spaces""#;
    let result = dotenv_validate(text, false, "^[A-Z_]+$", "error");
    assert!(result.parse_ok);
    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].value, "value with spaces");
}

#[test]
fn test_dotenv_validate_empty_value() {
    let text = "KEY=";
    let result = dotenv_validate(text, false, "^[A-Z_]+$", "error");
    assert!(result.parse_ok);
    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].key, "KEY");
}

#[test]
fn test_dotenv_validate_duplicate_policy_error() {
    let text = "KEY=1\nKEY=2";
    let result = dotenv_validate(text, false, "^[A-Z_]+$", "error");
    assert!(!result.parse_ok);
    assert!(!result.duplicates.is_empty());
}

#[test]
fn test_dotenv_validate_duplicate_policy_warn() {
    let text = "KEY=1\nKEY=2";
    let result = dotenv_validate(text, false, "^[A-Z_]+$", "warn");
    assert!(result.parse_ok);
    assert!(!result.duplicates.is_empty());
    assert!(result.findings.iter().any(|f| f.contains("Duplicate key")));
}

#[test]
fn test_dotenv_validate_duplicate_policy_last() {
    let text = "KEY=1\nKEY=2";
    let result = dotenv_validate(text, false, "^[A-Z_]+$", "last");
    assert!(result.parse_ok);
    assert_eq!(result.entries.len(), 2);
}

// ─── ini_validate ────────────────────────────────────────────────────

#[test]
fn test_ini_validate_valid() {
    let text = "[section]\nkey = value\n";
    let result = ini_validate(text, "error");
    assert!(result.parse_ok);
}

#[test]
fn test_ini_validate_empty() {
    let result = ini_validate("", "error");
    assert!(result.parse_ok);
}

#[test]
fn test_ini_validate_multiple_sections() {
    let text = "[section1]\nkey1 = val1\n[section2]\nkey2 = val2\n";
    let result = ini_validate(text, "error");
    assert!(result.parse_ok);
}

#[test]
fn test_ini_validate_comments() {
    let text = "; comment\n[section]\nkey = value\n";
    let result = ini_validate(text, "error");
    assert!(result.parse_ok);
}

#[test]
fn test_ini_validate_hash_comments() {
    let text = "# comment\n[section]\nkey = value\n";
    let result = ini_validate(text, "error");
    assert!(result.parse_ok);
}

#[test]
fn test_ini_validate_duplicate_keys() {
    let text = "[section]\nkey = 1\nkey = 2\n";
    let result = ini_validate(text, "error");
    assert!(!result.parse_ok);
    assert!(!result.duplicates.is_empty());
}

#[test]
fn test_ini_validate_no_section() {
    let text = "key = value\n";
    let result = ini_validate(text, "error");
    assert!(result.parse_ok);
    assert!(result.keys_by_section.contains_key("(top-level)"));
}

#[test]
fn test_ini_validate_nested_brackets() {
    let text = "[section]\nkey = [1, 2, 3]\n";
    let result = ini_validate(text, "error");
    assert!(result.parse_ok);
    assert!(result.keys_by_section.contains_key("section"));
}
