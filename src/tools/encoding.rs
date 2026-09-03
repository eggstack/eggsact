use crate::mcp::machine_codes;
use crate::mcp::response::ToolResponse;
use crate::tools::helpers::{json_type_name, MAX_TEXT_LENGTH};
use base64::engine::general_purpose::{STANDARD, URL_SAFE};
use base64::Engine;
use serde_json::Value;

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

fn format_arg<'a>(
    args: &'a Value,
    field: &str,
    tool: &'static str,
) -> Result<&'a str, Box<ToolResponse>> {
    let value = text_arg(args, field, tool)?;
    match value {
        "utf8" | "hex" | "base64" | "base64url" => Ok(value),
        _ => Err(Box::new(invalid(
            format!("{} must be one of utf8, hex, base64, base64url", field),
            tool,
        ))),
    }
}

fn decode_hex(value: &str) -> Result<Vec<u8>, String> {
    if !value.len().is_multiple_of(2) {
        return Err("hex input must contain an even number of digits".to_string());
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks(2) {
        let high = parse_digit(pair[0]).filter(|digit| *digit < 16);
        let low = parse_digit(pair[1]).filter(|digit| *digit < 16);
        match (high, low) {
            (Some(high), Some(low)) => bytes.push(((high << 4) | low) as u8),
            _ => return Err("hex input must contain only ASCII hexadecimal digits".to_string()),
        }
    }
    Ok(bytes)
}

fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn padded_base64(value: &str, url_safe: bool) -> Result<String, String> {
    let valid = |byte: u8| {
        byte.is_ascii_alphanumeric()
            || byte == b'='
            || (!url_safe && byte == b'+')
            || (!url_safe && byte == b'/')
            || (url_safe && byte == b'-')
            || (url_safe && byte == b'_')
    };
    if !value.bytes().all(valid) {
        return Err("base64 input contains an invalid or mixed alphabet character".to_string());
    }
    let padding = value.bytes().rev().take_while(|byte| *byte == b'=').count();
    if padding > 2 || value[..value.len().saturating_sub(padding)].contains('=') {
        return Err(
            "base64 padding must appear only at the end and contain at most two '=' characters"
                .to_string(),
        );
    }
    let unpadded_len = value.len() - padding;
    if unpadded_len % 4 == 1 {
        return Err("base64 input has an invalid length".to_string());
    }
    if padding > 0 && !value.len().is_multiple_of(4) {
        return Err("padded base64 input length must be a multiple of four".to_string());
    }
    let expected_padding = (4 - unpadded_len % 4) % 4;
    if padding > 0 && padding != expected_padding {
        return Err("base64 input has invalid padding".to_string());
    }
    let mut result = value.to_string();
    if padding == 0 {
        result.extend(std::iter::repeat_n('=', expected_padding));
    }
    Ok(result)
}

fn decode_base64(value: &str, url_safe: bool) -> Result<Vec<u8>, String> {
    let padded = padded_base64(value, url_safe)?;
    let engine = if url_safe { &URL_SAFE } else { &STANDARD };
    engine
        .decode(padded)
        .map_err(|_| "base64 input is not valid for the selected alphabet".to_string())
}

fn decode_codec(value: &str, format: &str) -> Result<Vec<u8>, String> {
    match format {
        "utf8" => Ok(value.as_bytes().to_vec()),
        "hex" => decode_hex(value),
        "base64" => decode_base64(value, false),
        "base64url" => decode_base64(value, true),
        _ => unreachable!("format validated at the handler boundary"),
    }
}

fn encode_codec(bytes: &[u8], format: &str) -> Result<String, String> {
    let result = match format {
        "utf8" => String::from_utf8(bytes.to_vec())
            .map_err(|_| "decoded bytes are not valid UTF-8".to_string())?,
        "hex" => encode_hex(bytes),
        "base64" => STANDARD.encode(bytes),
        "base64url" => URL_SAFE.encode(bytes).trim_end_matches('=').to_string(),
        _ => unreachable!("format validated at the handler boundary"),
    };
    if result.len() > MAX_TEXT_LENGTH {
        return Err(format!("encoded output exceeds {} bytes", MAX_TEXT_LENGTH));
    }
    Ok(result)
}

pub fn codec_convert(args: &Value) -> ToolResponse {
    let value = match text_arg(args, "value", "codec_convert") {
        Ok(value) => value,
        Err(error) => return *error,
    };
    let from = match format_arg(args, "from", "codec_convert") {
        Ok(value) => value,
        Err(error) => return *error,
    };
    let to = match format_arg(args, "to", "codec_convert") {
        Ok(value) => value,
        Err(error) => return *error,
    };
    let bytes = match decode_codec(value, from) {
        Ok(bytes) if bytes.len() <= MAX_TEXT_LENGTH => bytes,
        Ok(_) => {
            return ToolResponse::error_with_code(
                "input_too_large",
                machine_codes::INPUT_TOO_LARGE,
                &format!("decoded input exceeds {} bytes", MAX_TEXT_LENGTH),
                None,
                Some("codec_convert"),
            )
        }
        Err(error) => return invalid(error, "codec_convert"),
    };
    let converted = match encode_codec(&bytes, to) {
        Ok(value) => value,
        Err(error) if error.starts_with("encoded output exceeds") => {
            return ToolResponse::error_with_code(
                "input_too_large",
                machine_codes::INPUT_TOO_LARGE,
                &error,
                None,
                Some("codec_convert"),
            )
        }
        Err(error) => return invalid(error, "codec_convert"),
    };
    ToolResponse::success(
        serde_json::json!({"value": converted, "from": from, "to": to, "byte_length": bytes.len()}),
        Some("codec_convert"),
    )
    .with_tool("codec_convert")
}

fn parse_digit(byte: u8) -> Option<u32> {
    match byte {
        b'0'..=b'9' => Some(u32::from(byte - b'0')),
        b'a'..=b'z' => Some(u32::from(byte - b'a') + 10),
        b'A'..=b'Z' => Some(u32::from(byte - b'A') + 10),
        _ => None,
    }
}

fn parse_radix(value: &str, base: u32) -> Result<(bool, u128), String> {
    let (negative, digits) = match value.as_bytes().first() {
        Some(b'-') => (true, &value[1..]),
        Some(b'+') => (false, &value[1..]),
        _ => (false, value),
    };
    if digits.is_empty() {
        return Err("radix value must contain at least one digit".to_string());
    }
    let mut magnitude = 0u128;
    for byte in digits.bytes() {
        let digit = parse_digit(byte).ok_or_else(|| {
            format!(
                "invalid radix digit '{}'; use ASCII 0-9 or a-z",
                byte as char
            )
        })?;
        if digit >= base {
            return Err(format!(
                "digit '{}' is invalid for base {}",
                byte as char, base
            ));
        }
        magnitude = magnitude
            .checked_mul(u128::from(base))
            .and_then(|value| value.checked_add(u128::from(digit)))
            .ok_or_else(|| "radix magnitude exceeds u128".to_string())?;
    }
    Ok((negative && magnitude != 0, magnitude))
}

fn encode_radix(negative: bool, mut magnitude: u128, base: u32, uppercase: bool) -> String {
    if magnitude == 0 {
        return "0".to_string();
    }
    let alphabet = if uppercase {
        b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ"
    } else {
        b"0123456789abcdefghijklmnopqrstuvwxyz"
    };
    let mut digits = Vec::new();
    while magnitude > 0 {
        digits.push(alphabet[(magnitude % u128::from(base)) as usize] as char);
        magnitude /= u128::from(base);
    }
    if negative {
        digits.push('-');
    }
    digits.iter().rev().collect()
}

fn radix_base(args: &Value, field: &str) -> Result<u32, Box<ToolResponse>> {
    let value = match args.get(field) {
        Some(Value::Number(value)) if value.is_u64() => value.as_u64().unwrap_or(0),
        Some(value) => {
            return Err(Box::new(invalid(
                format!(
                    "{} must be an integer, got {}",
                    field,
                    json_type_name(value)
                ),
                "radix_convert",
            )))
        }
        None => {
            return Err(Box::new(invalid(
                format!("{} must be an integer, got NoneType", field),
                "radix_convert",
            )))
        }
    };
    if !(2..=36).contains(&value) {
        return Err(Box::new(invalid(
            format!("{} must be between 2 and 36", field),
            "radix_convert",
        )));
    }
    Ok(value as u32)
}

pub fn radix_convert(args: &Value) -> ToolResponse {
    let value = match text_arg(args, "value", "radix_convert") {
        Ok(value) => value,
        Err(error) => return *error,
    };
    let from = match radix_base(args, "from_base") {
        Ok(value) => value,
        Err(error) => return *error,
    };
    let to = match radix_base(args, "to_base") {
        Ok(value) => value,
        Err(error) => return *error,
    };
    let uppercase = match args.get("uppercase") {
        None => false,
        Some(Value::Bool(value)) => *value,
        Some(value) => {
            return invalid(
                format!("uppercase must be a boolean, got {}", json_type_name(value)),
                "radix_convert",
            )
        }
    };
    let (negative, magnitude) = match parse_radix(value, from) {
        Ok(value) => value,
        Err(error) => return invalid(error, "radix_convert"),
    };
    let converted = encode_radix(negative, magnitude, to, uppercase);
    ToolResponse::success(
        serde_json::json!({"value": converted, "from_base": from, "to_base": to, "uppercase": uppercase, "negative": negative, "magnitude_decimal": magnitude.to_string()}),
        Some("radix_convert"),
    )
    .with_tool("radix_convert")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codec_vectors_and_canonicalization() {
        let result =
            codec_convert(&serde_json::json!({"value":"SGVsbG8","from":"base64","to":"hex"}));
        assert_eq!(result.result.unwrap()["value"], "48656c6c6f");
        let result =
            codec_convert(&serde_json::json!({"value":"Zm8=","from":"base64","to":"base64url"}));
        assert_eq!(result.result.unwrap()["value"], "Zm8");
    }

    #[test]
    fn codec_rejects_invalid_inputs() {
        assert!(!codec_convert(&serde_json::json!({"value":"0x12","from":"hex","to":"utf8"})).ok);
        assert!(!codec_convert(&serde_json::json!({"value":"ff","from":"hex","to":"utf8"})).ok);
        assert!(!codec_convert(&serde_json::json!({"value":"a b","from":"base64","to":"hex"})).ok);
    }

    #[test]
    fn radix_supports_signed_magnitude_and_u128_max() {
        let result = radix_convert(&serde_json::json!({"value":"-ff","from_base":16,"to_base":2}));
        assert_eq!(result.result.unwrap()["value"], "-11111111");
        let result = radix_convert(
            &serde_json::json!({"value":u128::MAX.to_string(),"from_base":10,"to_base":16,"uppercase":true}),
        );
        assert_eq!(
            result.result.unwrap()["value"],
            "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"
        );
    }
}
