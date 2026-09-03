use eggsact::mcp::response::ToolResponse;
use eggsact::tools::{cidr_inspect, codec_convert, cron_inspect, datetime_convert, radix_convert};
use serde_json::Value;

fn output(response: ToolResponse) -> Value {
    assert!(response.ok, "unexpected tool failure: {:?}", response.error);
    response
        .result
        .expect("successful tool response has a result")
}

#[test]
fn cidr_normalization_is_idempotent() {
    for cidr in [
        "0.0.0.1/0",
        "10.1.2.3/24",
        "192.0.2.7/32",
        "2001:db8::1234/64",
        "::1/128",
    ] {
        let first = output(cidr_inspect(&serde_json::json!({"cidr": cidr})));
        let second = output(cidr_inspect(&serde_json::json!({"cidr": first["cidr"]})));
        assert_eq!(first["cidr"], second["cidr"]);
        assert_eq!(first["network_address"], second["network_address"]);
        assert_eq!(first["last_address"], second["last_address"]);
    }
}

#[test]
fn codec_and_radix_round_trips_preserve_values() {
    for (value, from, to) in [
        ("", "utf8", "base64"),
        ("Hello, world!", "utf8", "hex"),
        ("deadBEEF", "hex", "base64url"),
        ("SGVsbG8", "base64", "utf8"),
    ] {
        let converted = output(codec_convert(
            &serde_json::json!({"value": value, "from": from, "to": to}),
        ));
        let round_trip = output(codec_convert(
            &serde_json::json!({"value": converted["value"], "from": to, "to": from}),
        ));
        let canonical = output(codec_convert(
            &serde_json::json!({"value": round_trip["value"], "from": from, "to": from}),
        ));
        assert_eq!(round_trip["value"], canonical["value"]);
    }
    for (value, input, from, to) in [
        (0u128, "0", 2, 36),
        (1, "1", 10, 2),
        (255, "ff", 16, 8),
        (u128::MAX, "340282366920938463463374607431768211455", 10, 16),
    ] {
        let converted = output(radix_convert(
            &serde_json::json!({"value": input, "from_base": from, "to_base": to}),
        ));
        let round_trip = output(radix_convert(
            &serde_json::json!({"value": converted["value"], "from_base": to, "to_base": from}),
        ));
        assert_eq!(
            round_trip["magnitude_decimal"],
            value.to_string(),
            "value={value}, from={from}, to={to}, converted={converted}, round_trip={round_trip}"
        );
        assert_eq!(round_trip["negative"], false);
    }
}

#[test]
fn datetime_nanosecond_round_trip_preserves_the_instant() {
    for value in [
        "1969-12-31T23:59:59.999999999-04:00",
        "2026-09-03T11:00:00.123456789+05:30",
        "2000-02-29T00:00:00Z",
    ] {
        let first = output(datetime_convert(
            &serde_json::json!({"value": value, "format": "rfc3339"}),
        ));
        let second = output(datetime_convert(
            &serde_json::json!({"value": first["unix_nanoseconds"], "format": "unix_nanoseconds"}),
        ));
        assert_eq!(first["utc_rfc3339"], second["utc_rfc3339"]);
        assert_eq!(first["unix_nanoseconds"], second["unix_nanoseconds"]);
    }
}

#[test]
fn cron_results_are_ordered_and_strictly_after_the_reference() {
    let result = output(cron_inspect(
        &serde_json::json!({"expression": "*/17 * * * *", "after": "2026-09-03T11:00:00-04:00", "count": 32}),
    ));
    let runs = result["next_runs"].as_array().unwrap();
    assert_eq!(runs.len(), 32);
    for pair in runs.windows(2) {
        assert!(pair[0].as_str().unwrap() < pair[1].as_str().unwrap());
    }
    assert!(runs[0].as_str().unwrap() > "2026-09-03T11:00:00-04:00");
}
