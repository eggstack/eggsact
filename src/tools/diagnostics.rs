use crate::mcp::machine_codes;
use crate::mcp::registry;
use crate::mcp::response::ToolResponse;
use crate::mcp::runtime;
use serde_json::{json, Value};

pub fn runtime_diagnostics(_args: &Value) -> ToolResponse {
    let active_profile = runtime::get_active_profile();
    let tool_count = registry::tool_count();
    let route_critical_tools: Vec<&str> = registry::ROUTE_CRITICAL_TOOLS.to_vec();
    let profile_tool_count = registry::tools_for_profile(&active_profile).len();

    let all_tools_list = registry::all_tools_vec();
    let mut cheap = 0u32;
    let mut moderate = 0u32;
    let mut heavy = 0u32;
    for spec in all_tools_list {
        match spec.cost {
            registry::ToolCost::Cheap => cheap += 1,
            registry::ToolCost::Moderate => moderate += 1,
            registry::ToolCost::Heavy => heavy += 1,
        }
    }

    let active_audience = format!("{:?}", runtime::get_active_audience());
    let schema_detail = runtime::get_schema_detail();

    let model_visible_tool_count =
        registry::tools_for_profile_audience(&active_profile, registry::ToolListAudience::Model)
            .len();
    let harness_visible_tool_count =
        registry::tools_for_profile_audience(&active_profile, registry::ToolListAudience::Harness)
            .len();

    let confusables_exists = std::path::Path::new("src/text/confusables_generated.rs").exists();
    let tool_cards_exists = std::path::Path::new("generated/tool-cards.md").exists();

    let metrics = runtime::snapshot_metrics();

    let result = json!({
        "active_profile": active_profile,
        "active_audience": active_audience,
        "tool_count": tool_count,
        "route_critical_tools": route_critical_tools,
        "profile_tool_count": profile_tool_count,
        "model_visible_tool_count": model_visible_tool_count,
        "harness_visible_tool_count": harness_visible_tool_count,
        "compatibility_mode": "eggcalc_python",
        "budget_tier_summary": {
            "cheap": cheap,
            "moderate": moderate,
            "heavy": heavy,
        },
        "runtime": {
            "active_profile": active_profile,
            "active_audience": active_audience,
            "schema_detail": schema_detail,
            "limits": {
                "max_requests_per_second": runtime::MAX_REQUESTS_PER_SECOND,
                "max_in_flight_requests": runtime::MAX_IN_FLIGHT_REQUESTS,
                "max_tool_workers": runtime::MAX_TOOL_WORKERS,
                "max_request_bytes": runtime::MAX_REQUEST_BYTES,
                "max_output_bytes": runtime::MAX_OUTPUT_BYTES,
            },
            "live_metrics": {
                "active_requests": metrics.active_requests,
                "active_blocking_handlers": metrics.active_blocking_handlers,
                "timed_out_handlers": metrics.timed_out_handlers,
                "total_timeouts": metrics.total_timeouts,
                "peak_blocking_concurrency": metrics.peak_blocking_concurrency,
            },
        },
        "known_env_vars": [
            "EGGCALC_NO_CONFIG",
            "EGGCALC_MCP_PROFILE",
            "EGGCALC_MCP_AUDIENCE",
            "EGGCALC_MCP_SCHEMA_DETAIL",
        ],
        "generated_doc_command": "cargo run --bin generate-docs",
        "verification_command": "cargo run --bin verify-eggsact",
        "generated_data": {
            "confusables_generated_rs": confusables_exists,
            "tool_cards_md": tool_cards_exists,
        },
        "parity_available": std::path::Path::new("../eggcalc").exists(),
    });

    ToolResponse::success(result, Some("runtime_diagnostics"))
        .with_tool("runtime_diagnostics")
        .with_machine_code(machine_codes::OK)
}

/// Profile purposes for self-documentation.
fn profile_purpose(name: &str) -> &'static str {
    match name {
        "full" => "All non-hidden tools. Default for MCP server.",
        "default" => "Default tool set for general use.",
        "codegg_core_min" => "Minimal codegg tool set: preflight + validate_json.",
        "codegg_core" => "Core codegg tools: preflight, text inspection, path, cargo, analysis.",
        "codegg_preflight" => "Preflight and safety enforcement tools for codegg harness.",
        "codegg_patch" => "Patch and diff tools for codegg edit workflows.",
        "codegg_config" => "Configuration file validation and inspection tools.",
        "codegg_unicode_security" => "Unicode security and text inspection tools.",
        "codegg_shell" => "Shell command analysis and safety tools.",
        "codegg_repo_audit" => "Repository audit and manifest inspection tools.",
        "human_math" => "Human-friendly math evaluation tools.",
        _ => "Unknown profile.",
    }
}

/// Returns true if the profile is primarily intended for harness/debug use.
fn profile_intended_audience(name: &str) -> &'static str {
    match name {
        "full" | "default" | "human_math" => "mixed",
        "codegg_core_min"
        | "codegg_core"
        | "codegg_patch"
        | "codegg_config"
        | "codegg_unicode_security"
        | "codegg_shell"
        | "codegg_repo_audit" => "model",
        "codegg_preflight" => "harness",
        _ => "unknown",
    }
}

pub fn profile_inspect(args: &Value) -> ToolResponse {
    let profile_name = match args.get("profile").and_then(|v| v.as_str()) {
        Some(name) => name,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing required argument: profile",
                None,
                Some("profile_inspect"),
            )
        }
    };

    if !registry::PROFILE_NAMES.contains(&profile_name) {
        let available: Vec<&str> = registry::PROFILE_NAMES.to_vec();
        return ToolResponse::error_with_code(
            "invalid_arguments",
            machine_codes::INVALID_ARGUMENTS,
            &format!(
                "Unknown profile: {:?}. Available profiles: {}",
                profile_name,
                available.join(", ")
            ),
            None,
            Some("profile_inspect"),
        );
    }

    let profile_tools = registry::tools_for_profile(profile_name);
    let tool_count = profile_tools.len();

    let has_route_critical = profile_tools
        .iter()
        .any(|t| registry::is_route_critical(t.name));
    let has_harness_only = profile_tools
        .iter()
        .any(|t| t.exposure == registry::ToolExposure::HarnessOnly);

    let model_count =
        registry::tools_for_profile_audience(profile_name, registry::ToolListAudience::Model).len();
    let harness_count =
        registry::tools_for_profile_audience(profile_name, registry::ToolListAudience::Harness)
            .len();

    let representative_tools: Vec<&str> = profile_tools.iter().take(5).map(|t| t.name).collect();

    let mut warnings = Vec::new();
    if tool_count == 0 {
        warnings.push("Profile has no tools in the active configuration.");
    }
    if has_harness_only && model_count == 0 {
        warnings.push("Profile only has harness-only tools; no model-visible tools.");
    }

    let purpose = profile_purpose(profile_name);
    let intended = profile_intended_audience(profile_name);

    let result = json!({
        "name": profile_name,
        "tool_count": tool_count,
        "model_visible_tool_count": model_count,
        "harness_visible_tool_count": harness_count,
        "intended_audience": intended,
        "purpose": purpose,
        "contains_route_critical_tools": has_route_critical,
        "contains_harness_only_tools": has_harness_only,
        "representative_tools": representative_tools,
        "warnings": warnings,
    });

    ToolResponse::success(result, Some("profile_inspect"))
        .with_tool("profile_inspect")
        .with_machine_code(machine_codes::OK)
}

pub fn tool_availability_explain(args: &Value) -> ToolResponse {
    let tool_name = match args.get("tool").and_then(|v| v.as_str()) {
        Some(name) => name,
        None => {
            return ToolResponse::error_with_code(
                "invalid_arguments",
                machine_codes::INVALID_ARGUMENTS,
                "Missing required argument: tool",
                None,
                Some("tool_availability_explain"),
            )
        }
    };

    let active_profile = runtime::get_active_profile();
    let profile = args
        .get("profile")
        .and_then(|v| v.as_str())
        .unwrap_or(&active_profile);
    let active_audience = format!("{:?}", runtime::get_active_audience());
    let audience_str = args
        .get("audience")
        .and_then(|v| v.as_str())
        .unwrap_or(&active_audience);

    // Check if tool exists at all
    let spec = match registry::get_tool(tool_name) {
        Some(s) => s,
        None => {
            let all_names = registry::tool_names();
            let close_match = registry::find_close_match(tool_name, &all_names);
            let result = json!({
                "exists": false,
                "available_in_profile": false,
                "callable_by_audience": false,
                "exposure": null,
                "profiles": [],
                "reason": format!("Tool '{}' does not exist.", tool_name),
                "suggested_tool": close_match,
                "suggested_profile": null,
                "suggested_audience": null,
            });
            return ToolResponse::success(result, Some("tool_availability_explain"))
                .with_tool("tool_availability_explain")
                .with_machine_code(machine_codes::OK);
        }
    };

    let exists = true;
    let exposure = spec.exposure.as_str().to_string();
    let profiles: Vec<&str> = spec.profiles.to_vec();
    let in_profile = spec.profiles.contains(&profile);

    // Determine audience callability
    let audience_matches = match audience_str {
        "Model" | "model" => {
            spec.exposure != registry::ToolExposure::HarnessOnly
                && spec.exposure != registry::ToolExposure::Hidden
        }
        "Harness" | "harness" => spec.exposure != registry::ToolExposure::Hidden,
        "Debug" | "debug" => spec.exposure != registry::ToolExposure::Hidden,
        _ => spec.exposure != registry::ToolExposure::Hidden,
    };

    let callable = in_profile && audience_matches;

    // Build reason
    let reason = if !in_profile {
        format!(
            "Tool '{}' is not included in profile '{}'. It is available in profiles: {}.",
            tool_name,
            profile,
            profiles.join(", ")
        )
    } else if !audience_matches {
        format!(
            "Tool '{}' has exposure '{}' which is not callable by the '{}' audience.",
            tool_name, exposure, audience_str
        )
    } else {
        format!(
            "Tool '{}' is available and callable in profile '{}' with '{}' audience.",
            tool_name, profile, audience_str
        )
    };

    // Suggest a profile if tool is not in the active profile
    let suggested_profile = if !in_profile {
        profiles.first().copied()
    } else {
        None
    };

    // Suggest audience if tool is harness-only and caller is model
    let suggested_audience = if in_profile && !audience_matches {
        if spec.exposure == registry::ToolExposure::HarnessOnly {
            Some("Harness")
        } else {
            None
        }
    } else {
        None
    };

    let result = json!({
        "exists": exists,
        "available_in_profile": in_profile,
        "callable_by_audience": callable,
        "exposure": exposure,
        "profiles": profiles,
        "reason": reason,
        "suggested_tool": null,
        "suggested_profile": suggested_profile,
        "suggested_audience": suggested_audience,
    });

    ToolResponse::success(result, Some("tool_availability_explain"))
        .with_tool("tool_availability_explain")
        .with_machine_code(machine_codes::OK)
}
