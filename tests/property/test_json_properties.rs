use eggsact::text::{json_canonicalize, json_compare, json_shape};

#[test]
fn json_canonicalize_deterministic() {
    let inputs = ["{\"b\":2,\"a\":1}", "[3,1,2]", "{\"x\":{\"b\":2,\"a\":1}}"];
    for input in &inputs {
        let r1 = json_canonicalize(input, true, Some(2), false, true, false);
        let r2 = json_canonicalize(input, true, Some(2), false, true, false);
        assert_eq!(r1.is_err(), r2.is_err());
        if let (Ok(v1), Ok(v2)) = (r1, r2) {
            assert_eq!(v1.canonical, v2.canonical);
            assert_eq!(v1.valid, v2.valid);
        }
    }
}

#[test]
fn json_canonicalize_idempotent() {
    let inputs = ["{\"b\":2,\"a\":1}", "[3,1,2]", "1", "\"hello\""];
    for input in &inputs {
        if let Ok(c1) = json_canonicalize(input, true, Some(2), false, true, false) {
            if let Some(ref canonical) = c1.canonical {
                if let Ok(c2) = json_canonicalize(canonical, true, Some(2), false, true, false) {
                    assert_eq!(
                        c1.canonical, c2.canonical,
                        "Idempotence violated for: {}",
                        input
                    );
                }
            }
        }
    }
}

#[test]
fn json_compare_symmetric() {
    let pairs = [
        ("{\"a\":1,\"b\":2}", "{\"b\":2,\"a\":1}"),
        ("[1,2,3]", "[1,2,3]"),
    ];
    for (a, b) in &pairs {
        let r1 = json_compare(a, b, true, false, false, false, false, 100);
        let r2 = json_compare(b, a, true, false, false, false, false, 100);
        assert_eq!(r1, r2);
    }
}

#[test]
fn json_compare_self_equal() {
    let inputs = ["{}", "[]", "null", "123", "\"hello\"", "{\"a\":1}"];
    for input in &inputs {
        let result = json_compare(input, input, false, false, false, false, false, 100);
        assert!(result.is_ok());
    }
}

#[test]
fn json_shape_deterministic() {
    let inputs = ["{\"a\":1,\"b\":{\"c\":2}}", "[1,2,3]", "null"];
    for input in &inputs {
        let s1 = json_shape(input, 10, 100, 100);
        let s2 = json_shape(input, 10, 100, 100);
        assert_eq!(s1, s2);
    }
}
