use eggsact::text::{json_shape, text_fingerprint, text_hash};
use std::collections::BTreeMap;

/// Assert that a struct with BTreeMap fields serializes to byte-identical JSON
/// across repeated calls.
fn assert_byte_stable(val: &impl serde::Serialize, label: &str) {
    let json1 = serde_json::to_string(val).unwrap();
    let json2 = serde_json::to_string(val).unwrap();
    assert_eq!(json1, json2, "{}: repeated serialization differs", label);
}

#[test]
fn regex_groupdict_serializes_in_lexical_order() {
    let mut groups = BTreeMap::new();
    groups.insert("zeta".to_string(), "z".to_string());
    groups.insert("alpha".to_string(), "a".to_string());
    groups.insert("middle".to_string(), "m".to_string());

    let json = serde_json::to_string(&groups).unwrap();
    let alpha_pos = json.find("\"alpha\"").unwrap();
    let middle_pos = json.find("\"middle\"").unwrap();
    let zeta_pos = json.find("\"zeta\"").unwrap();
    assert!(
        alpha_pos < middle_pos && middle_pos < zeta_pos,
        "groupdict keys not in lexical order: {}",
        json
    );
}

#[test]
fn regex_groupdict_deterministic_across_serializations() {
    let mut groups = BTreeMap::new();
    groups.insert("beta".to_string(), "2".to_string());
    groups.insert("alpha".to_string(), "1".to_string());

    assert_byte_stable(&groups, "regex_groupdict");
}

#[test]
fn json_shape_keys_serialize_lexically() {
    let input = r#"{"zebra": 1, "alpha": 2, "middle": {"delta": 3}}"#;
    let shape = json_shape(input, 10, 100, 100);
    let json = serde_json::to_string(&shape).unwrap();

    let alpha_pos = json.find("\"alpha\"").unwrap();
    let middle_pos = json.find("\"middle\"").unwrap();
    let zebra_pos = json.find("\"zebra\"").unwrap();
    assert!(
        alpha_pos < middle_pos && middle_pos < zebra_pos,
        "json_shape keys not in lexical order: {}",
        json
    );
}

#[test]
fn json_shape_deterministic_serialization() {
    let input = r#"{"b": 1, "a": 2, "c": {"x": 3, "y": 4}}"#;
    let shape = json_shape(input, 10, 100, 100);
    assert_byte_stable(&shape, "json_shape");
}

#[test]
fn text_hash_hashes_serialize_lexically() {
    let hashes = text_hash(
        "hello world",
        &["sha256".into(), "md5".into(), "sha1".into()],
        "utf-8",
    );
    let json = serde_json::to_string(&hashes.hashes).unwrap();

    let md5_pos = json.find("\"md5\"").unwrap();
    let sha1_pos = json.find("\"sha1\"").unwrap();
    let sha256_pos = json.find("\"sha256\"").unwrap();
    assert!(
        md5_pos < sha1_pos && sha1_pos < sha256_pos,
        "text_hash hashes not in lexical order: {}",
        json
    );
}

#[test]
fn text_hash_deterministic_serialization() {
    let hashes = text_hash("hello world", &["sha256".into(), "md5".into()], "utf-8");
    assert_byte_stable(&hashes, "text_hash");
}

#[test]
fn text_fingerprint_normalization_serialize_lexically() {
    let fp = text_fingerprint("hello", "NFC", "auto", false, false);
    let json = serde_json::to_string(&fp.normalization).unwrap();

    let applied_pos = json.find("\"applied\"").unwrap();
    let input_nfc_pos = json.find("\"input_is_nfc\"").unwrap();
    assert!(
        applied_pos < input_nfc_pos,
        "text_fingerprint normalization not in lexical order: {}",
        json
    );
}

#[test]
fn text_fingerprint_deterministic_serialization() {
    let fp = text_fingerprint("hello world", "NFC", "auto", true, true);
    assert_byte_stable(&fp, "text_fingerprint");
}
