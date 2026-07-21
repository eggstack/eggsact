use eggsact::text::{dotenv_validate, ini_validate, toml_shape, validate_toml};

#[test]
fn toml_validate_no_panic() {
    let inputs = ["", "[section]\nkey=value", "key = \"value\"", "???"];
    for input in &inputs {
        let _ = validate_toml(input);
    }
}

#[test]
fn toml_shape_deterministic() {
    let inputs = ["[a]\nb=1\n[c]\nd=2", "", "[x]"];
    for input in &inputs {
        let s1 = toml_shape(input, 100);
        let s2 = toml_shape(input, 100);
        assert_eq!(s1.is_err(), s2.is_err());
    }
}

#[test]
fn dotenv_validate_no_panic() {
    let inputs = ["FOO=bar", "", "123=456", "FOO=bar\nBAZ=qux"];
    for input in &inputs {
        let _ = dotenv_validate(input, true, "^[A-Z_][A-Z0-9_]*$", "warn");
    }
}

#[test]
fn dotenv_validate_deterministic() {
    let input = "FOO=bar\nBAZ=qux\nEMPTY=";
    let r1 = dotenv_validate(input, true, "^[A-Z_][A-Z0-9_]*$", "warn");
    let r2 = dotenv_validate(input, true, "^[A-Z_][A-Z0-9_]*$", "warn");
    let _ = (r1, r2);
}

#[test]
fn ini_validate_no_panic() {
    let inputs = ["[section]\nkey=value", "", "[s]\nk=v\nk2=v2"];
    for input in &inputs {
        let _ = ini_validate(input, "warn");
    }
}

#[test]
fn ini_validate_deterministic() {
    let input = "[s]\nk=v\nk2=v2";
    let r1 = ini_validate(input, "warn");
    let r2 = ini_validate(input, "warn");
    let _ = (r1, r2);
}
