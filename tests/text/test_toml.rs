use eggsact::text::{toml_shape, validate_toml};

// ─── validate_toml ───────────────────────────────────────────────────

#[test]
fn test_validate_toml_valid() {
    let text = "title = \"hello\"\nversion = \"1.0\"";
    let result = validate_toml(text).unwrap();
    assert!(result.valid);
}

#[test]
fn test_validate_toml_empty() {
    let result = validate_toml("").unwrap();
    assert!(result.valid);
}

#[test]
fn test_validate_toml_table() {
    let text = "[package]\nname = \"myapp\"\nversion = \"0.1.0\"";
    let result = validate_toml(text).unwrap();
    assert!(result.valid);
}

#[test]
fn test_validate_toml_invalid_syntax() {
    let result = validate_toml("this is not = = = toml");
    // Should detect invalid TOML
    assert!(result.is_err() || !result.unwrap().valid);
}

#[test]
fn test_validate_toml_array() {
    let text = "items = [1, 2, 3]";
    let result = validate_toml(text).unwrap();
    assert!(result.valid);
}

#[test]
fn test_validate_toml_nested_table() {
    let text = "[a]\nb = 1\n[a.c]\nd = 2";
    let result = validate_toml(text).unwrap();
    assert!(result.valid);
}

#[test]
fn test_validate_toml_boolean() {
    let text = "flag = true\nother = false";
    let result = validate_toml(text).unwrap();
    assert!(result.valid);
}

#[test]
fn test_validate_toml_comment() {
    let text = "# This is a comment\nkey = \"value\"";
    let result = validate_toml(text).unwrap();
    assert!(result.valid);
}

// ─── toml_shape ──────────────────────────────────────────────────────

#[test]
fn test_toml_shape_basic() {
    let text = "title = \"hello\"\nversion = \"1.0\"";
    let result = toml_shape(text, 100).unwrap();
    assert!(result.valid);
}

#[test]
fn test_toml_shape_empty() {
    let result = toml_shape("", 100).unwrap();
    assert!(result.valid);
}

#[test]
fn test_toml_shape_tables() {
    let text = "[package]\nname = \"myapp\"\n\n[dependencies]\nserde = \"1.0\"";
    let result = toml_shape(text, 100).unwrap();
    // tables is Option<Vec<String>>
    if let Some(ref tables) = result.tables {
        assert!(!tables.is_empty());
    }
}

#[test]
fn test_toml_shape_max_tables() {
    let text = "[a]\n[b]\n[c]\n[d]\n[e]";
    let result = toml_shape(text, 2).unwrap();
    assert!(
        result.truncated,
        "Expected truncated=true when tables exceed max_tables"
    );
}

#[test]
fn test_toml_shape_nested() {
    let text = "[a]\nb.c = 1";
    let result = toml_shape(text, 100).unwrap();
    assert!(!result.tables.as_ref().unwrap().is_empty());
}

#[test]
fn test_toml_shape_invalid() {
    let result = toml_shape("not = = valid", 100);
    assert!(result.is_ok());
    assert!(!result.unwrap().valid);
}
