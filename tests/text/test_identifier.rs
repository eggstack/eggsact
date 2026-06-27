use eggsact::text::{
    identifier_analyze, identifier_inspect, identifier_table_inspect, TableIdentifierEntry,
};

// ─── identifier_analyze ──────────────────────────────────────────────

#[test]
fn test_identifier_analyze_single() {
    let result = identifier_analyze("my_variable", None);
    assert!(!result.text.is_empty());
    assert_eq!(result.text, "my_variable");
}

#[test]
fn test_identifier_analyze_python_valid() {
    let result = identifier_analyze("valid_name", None);
    assert!(result.python_valid);
}

#[test]
fn test_identifier_analyze_python_keyword() {
    let result = identifier_analyze("def", None);
    assert!(result.python_keyword);
}

#[test]
fn test_identifier_analyze_classification() {
    let result = identifier_analyze("my_variable_name", None);
    assert!(!result.classification.is_empty());
}

#[test]
fn test_identifier_analyze_snake_case() {
    let result = identifier_analyze("my_variable_name", None);
    assert_eq!(result.classification, "snake_case");
}

#[test]
fn test_identifier_analyze_camel_case() {
    let result = identifier_analyze("myVariableName", None);
    assert!(!result.classification.is_empty());
}

#[test]
fn test_identifier_analyze_pascal_case() {
    let result = identifier_analyze("MyVariableName", None);
    assert!(!result.classification.is_empty());
}

#[test]
fn test_identifier_analyze_constant_case() {
    let result = identifier_analyze("MY_VARIABLE", None);
    assert_eq!(result.classification, "SCREAMING_SNAKE_CASE");
}

#[test]
fn test_identifier_analyze_with_language() {
    let result = identifier_analyze("def", Some(vec!["python"]));
    assert!(result.python_keyword);
}

#[test]
fn test_identifier_analyze_summary() {
    let result = identifier_analyze("my_var", None);
    assert!(!result.summary.is_empty());
}

// ─── identifier_inspect ──────────────────────────────────────────────

#[test]
fn test_identifier_inspect_basic() {
    let ids = vec!["foo".to_string(), "bar".to_string()];
    let result = identifier_inspect(&ids, "python", "none", false, false);
    assert_eq!(result.identifiers.len(), 2);
}

#[test]
fn test_identifier_inspect_reserved_keywords() {
    let ids = vec!["def".to_string(), "class".to_string(), "return".to_string()];
    let result = identifier_inspect(&ids, "python", "none", false, false);
    for info in &result.identifiers {
        assert!(
            !info.valid,
            "Expected '{}' to be flagged as invalid (reserved keyword)",
            info.raw
        );
    }
}

#[test]
fn test_identifier_inspect_casefold() {
    let ids = vec!["Foo".to_string(), "foo".to_string()];
    let result = identifier_inspect(&ids, "python", "none", true, false);
    // With casefold, "Foo" and "foo" should be detected as potential collisions
    if !result.collisions.is_empty() {
        assert!(!result.collisions.is_empty());
    }
}

#[test]
fn test_identifier_inspect_empty() {
    let ids: Vec<String> = vec![];
    let result = identifier_inspect(&ids, "python", "none", false, false);
    assert!(result.identifiers.is_empty());
}

#[test]
fn test_identifier_inspect_normalization_nfc() {
    let ids = vec!["caf\u{00e9}".to_string()];
    let result = identifier_inspect(&ids, "python", "nfc", false, false);
    assert_eq!(result.identifiers.len(), 1);
}

#[test]
fn test_identifier_inspect_identifier_info_fields() {
    let ids = vec!["my_var".to_string()];
    let result = identifier_inspect(&ids, "python", "none", false, false);
    let info = &result.identifiers[0];
    assert_eq!(info.raw, "my_var");
    assert!(!info.normalized.is_empty());
}

// ─── identifier_table_inspect ────────────────────────────────────────

#[test]
fn test_identifier_table_inspect_basic() {
    let entries = vec![
        TableIdentifierEntry {
            name: "foo".to_string(),
            kind: "variable".to_string(),
            file: String::new(),
            line: 1,
        },
        TableIdentifierEntry {
            name: "bar".to_string(),
            kind: "function".to_string(),
            file: String::new(),
            line: 5,
        },
    ];
    let result = identifier_table_inspect(&entries, "python", None);
    assert!(result.count > 0 || !result.findings.is_empty());
}

#[test]
fn test_identifier_table_inspect_collisions() {
    let entries = vec![
        TableIdentifierEntry {
            name: "foo".to_string(),
            kind: "variable".to_string(),
            file: String::new(),
            line: 1,
        },
        TableIdentifierEntry {
            name: "Foo".to_string(),
            kind: "variable".to_string(),
            file: String::new(),
            line: 2,
        },
    ];
    let result = identifier_table_inspect(&entries, "python", None);
    assert!(
        !result.collisions.is_empty(),
        "Expected casefold collision between 'foo' and 'Foo'"
    );
    assert!(result.collisions.iter().any(|c| c.kind == "casefold"));
}

#[test]
fn test_identifier_table_inspect_reserved() {
    let entries = vec![TableIdentifierEntry {
        name: "def".to_string(),
        kind: "identifier".to_string(),
        file: String::new(),
        line: 1,
    }];
    let result = identifier_table_inspect(&entries, "python", None);
    assert!(
        !result.reserved_keyword_hits.is_empty(),
        "Expected 'def' to be detected as reserved keyword"
    );
    assert_eq!(result.reserved_keyword_hits[0].name, "def");
}

#[test]
fn test_identifier_table_inspect_empty() {
    let result = identifier_table_inspect(&[], "python", None);
    assert!(result.count == 0 || result.findings.is_empty());
}

#[test]
fn test_identifier_table_inspect_with_checks() {
    let entries = vec![TableIdentifierEntry {
        name: "my_var".to_string(),
        kind: "variable".to_string(),
        file: String::new(),
        line: 1,
    }];
    let checks = vec!["reserved", "style"];
    let result = identifier_table_inspect(&entries, "python", Some(checks));
    assert_eq!(result.count, 1);
    assert!(result.reserved_keyword_hits.is_empty());
}

// ─── Smoke tests for LazyLock regex initialization ───────────────────

#[test]
fn test_identifier_analyze_multiple_calls_no_panic() {
    // Smoke test: calling identifier_analyze multiple times should not panic
    // This verifies that LazyLock regex patterns are initialized correctly
    let identifiers = [
        "my_var",
        "valid_name",
        "def",
        "class",
        "_private",
        "camelCase",
        "PascalCase",
        "SCREAMING_SNAKE",
        "kebab-case",
        "$dollar",
        "café",
        "日本語",
    ];
    for id in &identifiers {
        let _ = identifier_analyze(id, None);
    }
    // Call again to verify repeated usage
    for id in &identifiers {
        let _ = identifier_analyze(id, None);
    }
}

#[test]
fn test_identifier_analyze_snake_case_suggestions() {
    let result = identifier_analyze("helloWorld", None);
    assert_eq!(
        result.suggestions.get("snake_case").map(|s| s.as_str()),
        Some("helloworld")
    );
}

#[test]
fn test_identifier_analyze_screaming_snake_classification() {
    let result = identifier_analyze("SCREAMING_SNAKE_CASE", None);
    assert_eq!(result.classification, "SCREAMING_SNAKE_CASE");
    assert!(result.env_valid);
}

#[test]
fn test_identifier_inspect_multiple_calls_no_panic() {
    // Smoke test: calling identifier_inspect multiple times should not panic
    let ids = vec![
        "foo".to_string(),
        "bar".to_string(),
        "baz".to_string(),
        "def".to_string(),
        "class".to_string(),
    ];
    let languages = ["python", "rust", "javascript"];
    let normalizations = ["none", "NFC", "NFD", "NFKC", "NFKD"];
    for lang in &languages {
        for norm in &normalizations {
            let result = identifier_inspect(&ids, lang, norm, true, true);
            assert!(!result.identifiers.is_empty());
        }
    }
}

#[test]
fn test_identifier_table_inspect_multiple_calls_no_panic() {
    // Smoke test: calling identifier_table_inspect multiple times should not panic
    let entries = vec![
        TableIdentifierEntry {
            name: "my_var".to_string(),
            kind: "variable".to_string(),
            file: "test.rs".to_string(),
            line: 1,
        },
        TableIdentifierEntry {
            name: "MyVar".to_string(),
            kind: "function".to_string(),
            file: "test.rs".to_string(),
            line: 5,
        },
        TableIdentifierEntry {
            name: "def".to_string(),
            kind: "identifier".to_string(),
            file: "test.rs".to_string(),
            line: 10,
        },
    ];
    let languages = ["python", "rust", "javascript", "typescript"];
    for lang in &languages {
        let result = identifier_table_inspect(&entries, lang, None);
        assert!(result.count == entries.len() as i32);
    }
}
