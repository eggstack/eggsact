use crate::mcp::machine_codes;
use crate::mcp::response::ToolResponse;
use crate::tools::helpers::{json_type_name, MAX_TEXT_LENGTH};
use serde_json::Value;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

const IPV6_ADDRESS_COUNT: &str = "340282366920938463463374607431768211456";

fn ipv6_address_count(prefix: u8) -> String {
    match prefix {
        0 => IPV6_ADDRESS_COUNT.to_string(),
        1..=128 => (1u128 << u32::from(128 - prefix)).to_string(),
        _ => unreachable!("IPv6 prefix length must be at most 128"),
    }
}

fn invalid(message: impl Into<String>, tool: &'static str) -> ToolResponse {
    ToolResponse::error_with_code(
        "invalid_arguments",
        machine_codes::INVALID_ARGUMENTS,
        &message.into(),
        None,
        Some(tool),
    )
}

fn require_text<'a>(
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

fn ip_bytes_hex(ip: IpAddr) -> String {
    let bytes = match ip {
        IpAddr::V4(value) => value.octets().to_vec(),
        IpAddr::V6(value) => value.octets().to_vec(),
    };
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn ipv4_mapped(address: Ipv6Addr) -> Option<Ipv4Addr> {
    let value = u128::from(address);
    (value >> 32 == 0xffff).then(|| Ipv4Addr::from(value as u32))
}

fn special_use_tags(ip: IpAddr) -> Vec<&'static str> {
    let mut tags = match ip {
        IpAddr::V4(address) => {
            let value = u32::from(address);
            let mut tags = Vec::new();
            if address.is_unspecified() {
                tags.push("unspecified");
            }
            if address.is_loopback() {
                tags.push("loopback");
            }
            if (value & 0xff00_0000) == 0x0a00_0000
                || (value & 0xfff0_0000) == 0xac10_0000
                || (value & 0xffff_0000) == 0xc0a8_0000
            {
                tags.push("private");
            }
            if (value & 0xffff_0000) == 0xa9fe_0000 {
                tags.push("link_local");
            }
            if (value & 0xf000_0000) == 0xe000_0000 {
                tags.push("multicast");
            }
            if (value & 0xffffff00) == 0xc000_0200
                || (value & 0xffffff00) == 0xc633_6400
                || (value & 0xffffff00) == 0xcb00_7100
            {
                tags.push("documentation");
            }
            if (value & 0xffff_c000) == 0x6440_0000 {
                tags.push("shared");
            }
            tags
        }
        IpAddr::V6(address) => {
            let value = u128::from(address);
            let mut tags = Vec::new();
            if address.is_unspecified() {
                tags.push("unspecified");
            }
            if address.is_loopback() {
                tags.push("loopback");
            }
            if (value >> 118) == 0b11_1111_1010u128 {
                tags.push("link_local");
            }
            if (value >> 121) == 0b111_1110u128 {
                tags.push("unique_local");
            }
            if (value >> 120) == 0xff {
                tags.push("multicast");
            }
            if (value >> 96) == (u128::from(Ipv6Addr::new(0x2001, 0x0db8, 0, 0, 0, 0, 0, 0)) >> 96)
            {
                tags.push("documentation");
            }
            if ipv4_mapped(address).is_some() {
                tags.push("ipv4_mapped");
            }
            tags
        }
    };
    tags.sort_unstable();
    tags
}

pub fn ip_inspect(args: &Value) -> ToolResponse {
    let address = match require_text(args, "address", "ip_inspect") {
        Ok(value) => value,
        Err(error) => return *error,
    };
    let ip = match address.parse::<IpAddr>() {
        Ok(ip) => ip,
        Err(_) => return invalid(format!("Invalid IP address: {address}"), "ip_inspect"),
    };
    let mapped = match ip {
        IpAddr::V6(value) => ipv4_mapped(value).map(|mapped| {
            serde_json::json!({
                "address": mapped.to_string(),
                "numeric": u32::from(mapped).to_string(),
            })
        }),
        IpAddr::V4(_) => None,
    };
    let (family, numeric) = match ip {
        IpAddr::V4(value) => ("ipv4", u32::from(value).to_string()),
        IpAddr::V6(value) => ("ipv6", u128::from(value).to_string()),
    };
    ToolResponse::success(
        serde_json::json!({
            "address": ip.to_string(),
            "family": family,
            "bytes_hex": ip_bytes_hex(ip),
            "numeric": numeric,
            "special_use": special_use_tags(ip),
            "ipv4_mapped": mapped,
        }),
        Some("ip_inspect"),
    )
    .with_tool("ip_inspect")
}

struct ParsedCidr {
    ip: IpAddr,
    prefix: u8,
}

fn parse_cidr(value: &str) -> Result<ParsedCidr, String> {
    let (address, prefix) = value
        .split_once('/')
        .ok_or_else(|| "CIDR must contain exactly one '/' prefix separator".to_string())?;
    if address.is_empty() || prefix.is_empty() || prefix.contains('/') {
        return Err("CIDR address and prefix are both required".to_string());
    }
    if !prefix.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!(
            "CIDR prefix must be a non-negative integer, got {prefix}"
        ));
    }
    let ip = address
        .parse::<IpAddr>()
        .map_err(|_| format!("Invalid IP address: {address}"))?;
    let prefix_value = prefix
        .parse::<u16>()
        .map_err(|_| format!("CIDR prefix is too large: {prefix}"))?;
    let width = match ip {
        IpAddr::V4(_) => 32,
        IpAddr::V6(_) => 128,
    };
    if prefix_value > width {
        return Err(format!(
            "CIDR prefix {prefix_value} exceeds address width {width}"
        ));
    }
    Ok(ParsedCidr {
        ip,
        prefix: prefix_value as u8,
    })
}

fn ipv4_mask(prefix: u8) -> u32 {
    if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    }
}

fn ipv6_mask(prefix: u8) -> u128 {
    if prefix == 0 {
        0
    } else {
        u128::MAX << (128 - prefix)
    }
}

pub fn cidr_inspect(args: &Value) -> ToolResponse {
    let cidr = match require_text(args, "cidr", "cidr_inspect") {
        Ok(value) => value,
        Err(error) => return *error,
    };
    let parsed = match parse_cidr(cidr) {
        Ok(value) => value,
        Err(error) => return invalid(error, "cidr_inspect"),
    };
    let (family, canonical, network, mask, first, last, broadcast, count, contains_value) =
        match parsed.ip {
            IpAddr::V4(address) => {
                let value = u32::from(address);
                let mask = ipv4_mask(parsed.prefix);
                let network = value & mask;
                let host_mask = !mask;
                let last = network | host_mask;
                let contains = match args.get("contains") {
                    None => None,
                    Some(Value::String(candidate)) => match candidate.parse::<IpAddr>() {
                        Ok(IpAddr::V4(candidate)) => Some((
                            candidate.to_string(),
                            (u32::from(candidate) & mask) == network,
                        )),
                        Ok(IpAddr::V6(_)) => {
                            return invalid(
                                "contains must use the same address family as cidr",
                                "cidr_inspect",
                            )
                        }
                        Err(_) => {
                            return invalid(
                                format!("Invalid contains IP address: {candidate}"),
                                "cidr_inspect",
                            )
                        }
                    },
                    Some(value) => {
                        return invalid(
                            format!("contains must be a string, got {}", json_type_name(value)),
                            "cidr_inspect",
                        )
                    }
                };
                (
                    "ipv4",
                    format!("{}/{}", Ipv4Addr::from(network), parsed.prefix),
                    Ipv4Addr::from(network).to_string(),
                    Ipv4Addr::from(mask).to_string(),
                    Ipv4Addr::from(network).to_string(),
                    Ipv4Addr::from(last).to_string(),
                    Some(Ipv4Addr::from(last).to_string()),
                    (u64::from(host_mask) + 1).to_string(),
                    contains,
                )
            }
            IpAddr::V6(address) => {
                let value = u128::from(address);
                let mask = ipv6_mask(parsed.prefix);
                let network = value & mask;
                let last = network | !mask;
                let contains = match args.get("contains") {
                    None => None,
                    Some(Value::String(candidate)) => match candidate.parse::<IpAddr>() {
                        Ok(IpAddr::V6(candidate)) => Some((
                            candidate.to_string(),
                            (u128::from(candidate) & mask) == network,
                        )),
                        Ok(IpAddr::V4(_)) => {
                            return invalid(
                                "contains must use the same address family as cidr",
                                "cidr_inspect",
                            )
                        }
                        Err(_) => {
                            return invalid(
                                format!("Invalid contains IP address: {candidate}"),
                                "cidr_inspect",
                            )
                        }
                    },
                    Some(value) => {
                        return invalid(
                            format!("contains must be a string, got {}", json_type_name(value)),
                            "cidr_inspect",
                        )
                    }
                };
                (
                    "ipv6",
                    format!("{}/{}", Ipv6Addr::from(network), parsed.prefix),
                    Ipv6Addr::from(network).to_string(),
                    Ipv6Addr::from(mask).to_string(),
                    Ipv6Addr::from(network).to_string(),
                    Ipv6Addr::from(last).to_string(),
                    None,
                    ipv6_address_count(parsed.prefix),
                    contains,
                )
            }
        };
    let contains_json = contains_value.as_ref().map(|(_, result)| *result);
    let contains_address = contains_value.map(|(address, _)| address);
    ToolResponse::success(
        serde_json::json!({
            "family": family,
            "cidr": canonical,
            "prefix_length": parsed.prefix,
            "host_bits": match parsed.ip { IpAddr::V4(_) => 32 - parsed.prefix, IpAddr::V6(_) => 128 - parsed.prefix },
            "network_address": network,
            "netmask": mask,
            "first_address": first,
            "last_address": last,
            "broadcast_address": broadcast,
            "address_count": count,
            "contains": contains_json,
            "contains_address": contains_address,
        }),
        Some("cidr_inspect"),
    )
    .with_tool("cidr_inspect")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(args: Value) -> Value {
        ip_inspect(&args).result.unwrap()
    }

    #[test]
    fn classifies_ipv4_boundaries() {
        assert_eq!(
            result(serde_json::json!({"address":"10.0.0.1"}))["special_use"],
            serde_json::json!(["private"])
        );
        assert_eq!(
            result(serde_json::json!({"address":"192.0.2.255"}))["special_use"],
            serde_json::json!(["documentation"])
        );
        assert_eq!(
            result(serde_json::json!({"address":"100.64.0.0"}))["special_use"],
            serde_json::json!(["shared"])
        );
    }

    #[test]
    fn classifies_only_true_ipv4_mapped_ipv6_addresses() {
        let mapped = result(serde_json::json!({"address":"::ffff:192.0.2.1"}));
        assert_eq!(mapped["ipv4_mapped"]["address"], "192.0.2.1");
        assert_eq!(mapped["special_use"], serde_json::json!(["ipv4_mapped"]));

        let canonical_mapped = result(serde_json::json!({"address":"::ffff:c000:0201"}));
        assert_eq!(canonical_mapped["ipv4_mapped"]["address"], "192.0.2.1");

        for address in ["::1", "::192.0.2.1", "::", "2001:db8::1"] {
            let value = result(serde_json::json!({"address":address}));
            assert!(value["ipv4_mapped"].is_null(), "address={address}");
            assert!(
                !value["special_use"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|tag| tag == "ipv4_mapped"),
                "address={address}"
            );
        }
    }

    #[test]
    fn special_use_tags_are_lexicographically_stable() {
        for address in [
            "10.0.0.1",
            "192.0.2.1",
            "::1",
            "::ffff:192.0.2.1",
            "ff02::1",
        ] {
            let value = result(serde_json::json!({"address":address}));
            let tags = value["special_use"].as_array().unwrap();
            let mut sorted = tags.clone();
            sorted.sort_by_key(|tag| tag.to_string());
            assert_eq!(*tags, sorted, "address={address}");
        }
    }

    #[test]
    fn canonicalizes_and_contains_ipv4_cidr() {
        let response =
            cidr_inspect(&serde_json::json!({"cidr":"10.1.2.3/24","contains":"10.1.2.200"}));
        let value = response.result.unwrap();
        assert_eq!(value["cidr"], "10.1.2.0/24");
        assert_eq!(value["contains"], true);
        assert_eq!(value["broadcast_address"], "10.1.2.255");
    }

    #[test]
    fn handles_ipv6_zero_prefix_exact_count() {
        let response = cidr_inspect(&serde_json::json!({"cidr":"2001:db8::1/0"}));
        let value = response.result.unwrap();
        assert_eq!(value["address_count"], IPV6_ADDRESS_COUNT);
        assert_eq!(value["cidr"], "::/0");
    }

    #[test]
    fn calculates_ipv6_address_count_from_prefix_only() {
        for (cidr, expected) in [
            ("::/0", IPV6_ADDRESS_COUNT),
            ("2001:db8::/1", "170141183460469231731687303715884105728"),
            ("2001:db8::/64", "18446744073709551616"),
            ("2001:db8::/127", "2"),
            ("2001:db8::/128", "1"),
            ("ffff:ffff:ffff:ffff::/64", "18446744073709551616"),
        ] {
            let value = cidr_inspect(&serde_json::json!({"cidr":cidr}))
                .result
                .unwrap();
            assert_eq!(value["address_count"], expected, "cidr={cidr}");
        }
    }

    #[test]
    fn rejects_cross_family_and_bad_prefix() {
        assert!(!cidr_inspect(&serde_json::json!({"cidr":"10.0.0.0/33"})).ok);
        assert!(!cidr_inspect(&serde_json::json!({"cidr":"10.0.0.0/24","contains":"::1"})).ok);
    }
}
