use eggsact::mcp::response::ToolResponse;
use eggsact::tools::{cidr_inspect, codec_convert, cron_inspect, datetime_convert, radix_convert};
use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

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
fn ipv6_address_count_depends_only_on_prefix_length() {
    for prefix in 0..=128u8 {
        let expected = if prefix == 0 {
            "340282366920938463463374607431768211456".to_string()
        } else {
            (1u128 << u32::from(128 - prefix)).to_string()
        };
        let first = output(cidr_inspect(
            &serde_json::json!({"cidr":format!("2001:db8::1/{prefix}")}),
        ));
        let second = output(cidr_inspect(
            &serde_json::json!({"cidr":format!("ffff:ffff:ffff:ffff::1/{prefix}")}),
        ));
        assert_eq!(first["address_count"], expected, "prefix={prefix}");
        assert_eq!(
            second["address_count"], first["address_count"],
            "prefix={prefix}"
        );
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

#[test]
fn cron_results_satisfy_independent_dom_dow_rules() {
    type DayRule = fn(u8, u8) -> bool;
    let cases: [(&str, DayRule); 6] = [
        ("0 0 * * MON", |_day: u8, dow: u8| dow == 1),
        ("0 0 1 * *", |day: u8, _dow: u8| day == 1),
        ("0 0 1 * MON", |day: u8, dow: u8| day == 1 || dow == 1),
        ("0 0 1-31 * MON", |day: u8, dow: u8| {
            (1..=31).contains(&day) || dow == 1
        }),
        ("0 0 1 * 0-7", |day: u8, dow: u8| {
            day == 1 || (0..=6).contains(&dow)
        }),
        ("0 0 */1 * MON", |day: u8, dow: u8| {
            (1..=31).contains(&day) || dow == 1
        }),
    ];
    for (expression, matches) in cases {
        let value = output(cron_inspect(&serde_json::json!({
            "expression": expression,
            "after": "2026-09-03T00:00:00Z",
            "count": 16,
        })));
        for timestamp in value["next_runs"].as_array().unwrap() {
            let instant = OffsetDateTime::parse(timestamp.as_str().unwrap(), &Rfc3339).unwrap();
            assert!(
                matches(instant.day(), instant.weekday().number_days_from_sunday()),
                "expression={expression}, timestamp={timestamp}"
            );
        }
    }
}
