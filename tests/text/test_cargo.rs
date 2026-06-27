use eggsact::text::cargo_toml_inspect;

// ─── cargo_toml_inspect ──────────────────────────────────────────────

#[test]
fn test_cargo_inspect_minimal() {
    let text = "[package]\nname = \"myapp\"\nversion = \"0.1.0\"";
    let result = cargo_toml_inspect(text, false, false);
    assert!(result.parse_ok);
    assert_eq!(result.package.name.as_deref(), Some("myapp"));
    assert_eq!(result.package.version.as_deref(), Some("0.1.0"));
}

#[test]
fn test_cargo_inspect_empty() {
    let result = cargo_toml_inspect("", false, false);
    assert!(result.parse_ok);
}

#[test]
fn test_cargo_inspect_with_dependencies() {
    let text = "[package]\nname = \"myapp\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"1.0\"\ntokio = \"1\"";
    let result = cargo_toml_inspect(text, false, true);
    assert!(result.parse_ok);
    assert!(!result.dependencies.dependencies.is_empty());
    assert!(result.dependencies.dependencies.contains_key("serde"));
    assert!(result.dependencies.dependencies.contains_key("tokio"));
}

#[test]
fn test_cargo_inspect_workspace() {
    let text = "[workspace]\nmembers = [\"app\", \"lib\"]";
    let result = cargo_toml_inspect(text, true, false);
    assert!(result.workspace.present);
}

#[test]
fn test_cargo_inspect_no_workspace() {
    let text = "[package]\nname = \"myapp\"";
    let result = cargo_toml_inspect(text, true, false);
    assert!(!result.workspace.present);
}

#[test]
fn test_cargo_inspect_dev_dependencies() {
    let text = "[package]\nname = \"myapp\"\nversion = \"0.1.0\"\n\n[dev-dependencies]\nassert_cmd = \"2\"";
    let result = cargo_toml_inspect(text, false, true);
    assert!(result.parse_ok);
    assert!(!result.dependencies.dev_dependencies.is_empty());
    assert!(result
        .dependencies
        .dev_dependencies
        .contains_key("assert_cmd"));
}

#[test]
fn test_cargo_inspect_features() {
    let text = "[package]\nname = \"myapp\"\nversion = \"0.1.0\"\n\n[features]\ndefault = [\"std\"]\nstd = []";
    let result = cargo_toml_inspect(text, false, false);
    assert!(result.parse_ok);
    assert_eq!(result.package.name.as_deref(), Some("myapp"));
}

#[test]
fn test_cargo_inspect_full_package() {
    let text = "[package]\nname = \"myapp\"\nversion = \"0.1.0\"\nauthors = [\"Test\"]\nedition = \"2021\"\ndescription = \"A test app\"";
    let result = cargo_toml_inspect(text, false, false);
    assert_eq!(result.package.name.as_deref(), Some("myapp"));
    assert_eq!(result.package.version.as_deref(), Some("0.1.0"));
    assert_eq!(result.package.edition.as_deref(), Some("2021"));
}

#[test]
fn test_cargo_inspect_invalid_toml() {
    let result = cargo_toml_inspect("this is not valid TOML = = =", false, false);
    assert!(!result.parse_ok);
    assert!(!result.findings.is_empty());
}
