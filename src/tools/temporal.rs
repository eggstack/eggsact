use crate::mcp::machine_codes;
use crate::mcp::response::ToolResponse;
use crate::temporal::{
    date_components, format_rfc3339, parse_fixed_offset, parse_rfc3339, unix_nanos, unix_unit,
    NANOS_PER_SECOND,
};
use crate::tools::helpers::{json_type_name, MAX_TEXT_LENGTH};
use serde_json::Value;
use time::OffsetDateTime;

fn invalid(message: impl Into<String>, tool: &'static str) -> ToolResponse {
    ToolResponse::error_with_code(
        "invalid_arguments",
        machine_codes::INVALID_ARGUMENTS,
        &message.into(),
        None,
        Some(tool),
    )
}

fn text_arg<'a>(
    args: &'a Value,
    field: &str,
    tool: &'static str,
) -> Result<&'a str, Box<ToolResponse>> {
    match args.get(field) {
        Some(Value::String(value)) if value.len() <= MAX_TEXT_LENGTH => Ok(value),
        Some(Value::String(value)) => Err(Box::new(ToolResponse::error_with_code(
            "input_too_large",
            machine_codes::INPUT_TOO_LARGE,
            &format!(
                "{} length {} bytes exceeds {}",
                field,
                value.len(),
                MAX_TEXT_LENGTH
            ),
            None,
            Some(tool),
        ))),
        Some(value) => Err(Box::new(invalid(
            format!("{} must be a string, got {}", field, json_type_name(value)),
            tool,
        ))),
        None => Err(Box::new(invalid(
            format!("{} must be a string, got NoneType", field),
            tool,
        ))),
    }
}

fn parse_integer(value: &str, format: &str) -> Result<i128, String> {
    if value.is_empty()
        || !value
            .bytes()
            .enumerate()
            .all(|(index, byte)| byte.is_ascii_digit() || (index == 0 && byte == b'-'))
    {
        return Err(format!("{format} must be a signed decimal integer string"));
    }
    value
        .parse::<i128>()
        .map_err(|_| format!("{format} is outside the supported integer range"))
}

fn parse_datetime(value: &str, format: &str) -> Result<OffsetDateTime, String> {
    match format {
        "rfc3339" => parse_rfc3339(value),
        "unix_seconds" => {
            let seconds = parse_integer(value, format)?;
            let nanos = seconds
                .checked_mul(NANOS_PER_SECOND)
                .ok_or_else(|| "unix seconds overflow nanosecond conversion".to_string())?;
            OffsetDateTime::from_unix_timestamp_nanos(nanos)
                .map_err(|error| format!("unix timestamp is outside the supported range: {error}"))
        }
        "unix_milliseconds" => {
            let milliseconds = parse_integer(value, format)?;
            let nanos = milliseconds
                .checked_mul(1_000_000)
                .ok_or_else(|| "unix milliseconds overflow nanosecond conversion".to_string())?;
            OffsetDateTime::from_unix_timestamp_nanos(nanos)
                .map_err(|error| format!("unix timestamp is outside the supported range: {error}"))
        }
        "unix_nanoseconds" => {
            let nanos = parse_integer(value, format)?;
            OffsetDateTime::from_unix_timestamp_nanos(nanos)
                .map_err(|error| format!("unix timestamp is outside the supported range: {error}"))
        }
        _ => Err(format!("unsupported datetime format: {format}")),
    }
}

pub fn datetime_convert(args: &Value) -> ToolResponse {
    let value = match text_arg(args, "value", "datetime_convert") {
        Ok(value) => value,
        Err(error) => return *error,
    };
    let format = match text_arg(args, "format", "datetime_convert") {
        Ok(value) => value,
        Err(error) => return *error,
    };
    if ![
        "rfc3339",
        "unix_seconds",
        "unix_milliseconds",
        "unix_nanoseconds",
    ]
    .contains(&format)
    {
        return invalid(
            "format must be rfc3339, unix_seconds, unix_milliseconds, or unix_nanoseconds",
            "datetime_convert",
        );
    }
    let parsed = match parse_datetime(value, format) {
        Ok(value) => value,
        Err(error) => return invalid(error, "datetime_convert"),
    };
    let selected_offset = match args.get("output_offset") {
        None => parsed.offset(),
        Some(Value::String(offset)) => match parse_fixed_offset(offset) {
            Ok(offset) => offset,
            Err(error) => return invalid(error, "datetime_convert"),
        },
        Some(value) => {
            return invalid(
                format!(
                    "output_offset must be a string, got {}",
                    json_type_name(value)
                ),
                "datetime_convert",
            )
        }
    };
    let instant = match OffsetDateTime::from_unix_timestamp_nanos(unix_nanos(parsed)) {
        Ok(value) => value,
        Err(error) => {
            return invalid(
                format!("timestamp is outside the supported range: {error}"),
                "datetime_convert",
            )
        }
    };
    let selected = instant.to_offset(selected_offset);
    let utc = instant.to_offset(time::UtcOffset::UTC);
    let canonical = match format_rfc3339(selected) {
        Ok(value) => value,
        Err(error) => return invalid(error, "datetime_convert"),
    };
    let utc_text = match format_rfc3339(utc) {
        Ok(value) => value,
        Err(error) => return invalid(error, "datetime_convert"),
    };
    let nanos = unix_nanos(instant);
    ToolResponse::success(
        serde_json::json!({
            "rfc3339": canonical,
            "utc_rfc3339": utc_text,
            "unix_seconds": unix_unit(nanos, NANOS_PER_SECOND).to_string(),
            "unix_milliseconds": unix_unit(nanos, 1_000_000).to_string(),
            "unix_nanoseconds": nanos.to_string(),
            "offset_seconds": selected_offset.whole_seconds(),
            "selected_offset": selected_offset.to_string(),
            "components": date_components(selected),
        }),
        Some("datetime_convert"),
    )
    .with_tool("datetime_convert")
}

fn count_arg(args: &Value) -> Result<usize, Box<ToolResponse>> {
    match args.get("count") {
        None => Ok(5),
        Some(Value::Number(value)) if value.is_u64() => match value.as_u64() {
            Some(value) if (1..=32).contains(&value) => Ok(value as usize),
            _ => Err(Box::new(invalid(
                "count must be between 1 and 32",
                "cron_inspect",
            ))),
        },
        Some(value) => Err(Box::new(invalid(
            format!(
                "count must be an integer between 1 and 32, got {}",
                json_type_name(value)
            ),
            "cron_inspect",
        ))),
    }
}

pub fn cron_inspect(args: &Value) -> ToolResponse {
    let expression = match text_arg(args, "expression", "cron_inspect") {
        Ok(value) => value,
        Err(error) => return *error,
    };
    let after_text = match text_arg(args, "after", "cron_inspect") {
        Ok(value) => value,
        Err(error) => return *error,
    };
    let count = match count_arg(args) {
        Ok(value) => value,
        Err(error) => return *error,
    };
    let schedule = match crate::temporal::cron::parse(expression) {
        Ok(value) => value,
        Err(error) => return invalid(error, "cron_inspect"),
    };
    let after = match parse_rfc3339(after_text) {
        Ok(value) => value,
        Err(error) => return invalid(error, "cron_inspect"),
    };
    let next_runs = match crate::temporal::cron::search_next(&schedule, after, count) {
        Ok(value) => value,
        Err(error) => return *error,
    };
    ToolResponse::success(
        serde_json::json!({
            "expression": expression,
            "normalized_expression": crate::temporal::cron::normalized_expression(&schedule),
            "parsed_values": crate::temporal::cron::parsed_values(&schedule),
            "offset": after.offset().to_string(),
            "offset_seconds": after.offset().whole_seconds(),
            "satisfiable": crate::temporal::cron::satisfiable(&schedule, after),
            "next_runs": next_runs,
            "count": next_runs.len(),
        }),
        Some("cron_inspect"),
    )
    .with_tool("cron_inspect")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datetime_handles_epoch_offsets_and_negative_fraction() {
        let result = datetime_convert(&serde_json::json!({"value":"0","format":"unix_seconds"}));
        assert_eq!(result.result.unwrap()["rfc3339"], "1970-01-01T00:00:00Z");
        let result = datetime_convert(
            &serde_json::json!({"value":"2026-09-03T11:00:00-04:00","format":"rfc3339","output_offset":"Z"}),
        );
        assert_eq!(result.result.unwrap()["rfc3339"], "2026-09-03T15:00:00Z");
        let result =
            datetime_convert(&serde_json::json!({"value":"-1","format":"unix_nanoseconds"}));
        assert_eq!(result.result.unwrap()["unix_seconds"], "-1");
    }

    #[test]
    fn cron_names_and_strict_after() {
        let result = cron_inspect(
            &serde_json::json!({"expression":"0 9 * * MON-FRI","after":"2026-09-03T11:00:00-04:00","count":2}),
        );
        let value = result.result.unwrap();
        assert_eq!(value["next_runs"][0], "2026-09-04T09:00:00-04:00");
        assert_eq!(value["count"], 2);
    }

    #[test]
    fn cron_dom_dow_use_or_semantics() {
        let result = cron_inspect(
            &serde_json::json!({"expression":"0 0 1 * MON","after":"2026-06-02T00:00:00Z","count":2}),
        );
        let value = result.result.unwrap();
        assert_eq!(value["next_runs"][0], "2026-06-08T00:00:00Z");
        assert_eq!(value["next_runs"][1], "2026-06-15T00:00:00Z");
    }

    #[test]
    fn cron_rejects_non_five_field_forms() {
        assert!(
            !cron_inspect(
                &serde_json::json!({"expression":"@daily","after":"2026-01-01T00:00:00Z"})
            )
            .ok
        );
        assert!(
            !cron_inspect(
                &serde_json::json!({"expression":"0 0 * * * *","after":"2026-01-01T00:00:00Z"})
            )
            .ok
        );
    }
}
