use time::format_description::well_known::Rfc3339;
use time::{OffsetDateTime, UtcOffset, Weekday};

pub const NANOS_PER_SECOND: i128 = 1_000_000_000;

pub fn parse_rfc3339(value: &str) -> Result<OffsetDateTime, String> {
    OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|error| format!("Invalid RFC 3339 timestamp: {error}"))
}

pub fn format_rfc3339(value: OffsetDateTime) -> Result<String, String> {
    value
        .format(&Rfc3339)
        .map_err(|error| format!("Unable to format RFC 3339 timestamp: {error}"))
}

pub fn parse_fixed_offset(value: &str) -> Result<UtcOffset, String> {
    if value == "Z" {
        return Ok(UtcOffset::UTC);
    }
    let bytes = value.as_bytes();
    if bytes.len() != 6 || (bytes[0] != b'+' && bytes[0] != b'-') || bytes[3] != b':' {
        return Err("offset must be exactly Z or +HH:MM/-HH:MM".to_string());
    }
    if !bytes[1..3]
        .iter()
        .chain(bytes[4..6].iter())
        .all(u8::is_ascii_digit)
    {
        return Err("offset must be exactly Z or +HH:MM/-HH:MM".to_string());
    }
    let hours = value[1..3].parse::<u8>().unwrap_or(99);
    let minutes = value[4..6].parse::<u8>().unwrap_or(99);
    if hours > 23 || minutes > 59 {
        return Err(format!("offset is outside the fixed-offset range: {value}"));
    }
    let sign = if bytes[0] == b'-' { -1 } else { 1 };
    UtcOffset::from_hms(sign * hours as i8, sign * minutes as i8, 0)
        .map_err(|error| format!("invalid fixed offset: {error}"))
}

pub fn unix_nanos(value: OffsetDateTime) -> i128 {
    value.unix_timestamp_nanos()
}

pub fn unix_unit(nanos: i128, unit: i128) -> i128 {
    nanos.div_euclid(unit)
}

pub fn date_components(value: OffsetDateTime) -> serde_json::Value {
    serde_json::json!({
        "year": value.year(),
        "month": value.month() as u8,
        "day": value.day(),
        "hour": value.hour(),
        "minute": value.minute(),
        "second": value.second(),
        "nanosecond": value.nanosecond(),
        "weekday": weekday_name(value.weekday()),
    })
}

pub fn weekday_name(weekday: Weekday) -> &'static str {
    match weekday {
        Weekday::Sunday => "SUN",
        Weekday::Monday => "MON",
        Weekday::Tuesday => "TUE",
        Weekday::Wednesday => "WED",
        Weekday::Thursday => "THU",
        Weekday::Friday => "FRI",
        Weekday::Saturday => "SAT",
    }
}

pub mod cron;
