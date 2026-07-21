use eggsact::text::{dotenv_validate, ini_validate, toml_shape};

#[test]
fn toml_shape_deterministic() {
    let inputs = ["[a]\nb=1\n[c]\nd=2", "", "[x]"];
    for input in &inputs {
        let s1 = toml_shape(input, 100);
        let s2 = toml_shape(input, 100);
        assert_eq!(s1, s2);
    }
}

#[test]
fn dotenv_validate_deterministic() {
    let input = "FOO=bar\nBAZ=qux\nEMPTY=";
    let r1 = dotenv_validate(input, true, "^[A-Z_][A-Z0-9_]*$", "warn");
    let r2 = dotenv_validate(input, true, "^[A-Z_][A-Z0-9_]*$", "warn");
    assert_eq!(r1, r2);
}

#[test]
fn ini_validate_deterministic() {
    let input = "[s]\nk=v\nk2=v2";
    let r1 = ini_validate(input, "warn");
    let r2 = ini_validate(input, "warn");
    assert_eq!(r1, r2);
}
