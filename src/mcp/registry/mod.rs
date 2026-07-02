mod all_tools;
mod listing;
mod types;

pub use all_tools::{ALL_TOOLS, PROFILE_NAMES};
pub use listing::{
    all_tools as all_tools_list, available_profiles, compact_input_schema, compact_output_schema,
    find_close_match, get_tool, input_schema_for, list_tool_definitions, mcp_tool_definitions,
    output_schema_for, tool_count, tool_handler_for, tool_names, tool_names_for_profile_audience,
    tools_for_profile, tools_for_profile_audience, ToolListAudience, ToolListOptions,
};
pub use types::{ToolCost, ToolDefinition, ToolExposure, ToolHandler, ToolSpec, ToolStability};

#[cfg(test)]
mod tests {
    use super::*;

    // -- Enum serialization preserves current strings --

    #[test]
    fn exposure_enum_serializes_to_expected_strings() {
        assert_eq!(ToolExposure::Default.as_str(), "default");
        assert_eq!(ToolExposure::Contextual.as_str(), "contextual");
        assert_eq!(ToolExposure::ExpertOnly.as_str(), "expert_only");
        assert_eq!(ToolExposure::HarnessOnly.as_str(), "harness_only");
        assert_eq!(ToolExposure::Hidden.as_str(), "hidden");
    }

    #[test]
    fn cost_enum_serializes_to_expected_strings() {
        assert_eq!(ToolCost::Cheap.as_str(), "cheap");
        assert_eq!(ToolCost::Moderate.as_str(), "moderate");
        assert_eq!(ToolCost::Heavy.as_str(), "heavy");
    }

    #[test]
    fn stability_enum_serializes_to_expected_strings() {
        assert_eq!(ToolStability::Stable.as_str(), "stable");
        assert_eq!(ToolStability::Deprecated.as_str(), "deprecated");
        assert_eq!(ToolStability::Experimental.as_str(), "experimental");
    }

    // -- Every tool has a valid exposure enum (compile-time guarantee via type system) --

    #[test]
    fn all_tools_have_valid_exposure() {
        for spec in ALL_TOOLS {
            // exposure is typed as ToolExposure, so this always has a valid variant.
            // This test documents the invariant and ensures as_str() works.
            let _ = spec.exposure.as_str();
        }
    }

    // -- Hidden tools excluded from ordinary listing --

    #[test]
    fn hidden_tools_excluded_from_mcp_definitions() {
        let definitions = mcp_tool_definitions();
        for tool in &definitions {
            assert_ne!(tool.llm_exposure.as_deref(), Some("hidden"));
        }
    }

    #[test]
    fn full_profile_excludes_hidden_tools() {
        let tools = tools_for_profile("full");
        for spec in tools {
            assert_ne!(spec.exposure, ToolExposure::Hidden);
        }
    }

    // -- Harness-only tools excluded from model audience lists --

    #[test]
    fn harness_only_excluded_from_model_audience_default() {
        let model_tools = tool_names_for_profile_audience("default", ToolListAudience::Model);
        for spec in ALL_TOOLS {
            if spec.exposure == ToolExposure::HarnessOnly && spec.profiles.contains(&"default") {
                assert!(
                    !model_tools.contains(&spec.name),
                    "harness_only tool '{}' should not appear in default model audience",
                    spec.name
                );
            }
        }
    }

    #[test]
    fn harness_only_excluded_from_model_audience_codegg_core_min() {
        let model_tools =
            tool_names_for_profile_audience("codegg_core_min", ToolListAudience::Model);
        for spec in ALL_TOOLS {
            if spec.exposure == ToolExposure::HarnessOnly
                && spec.profiles.contains(&"codegg_core_min")
            {
                assert!(
                    !model_tools.contains(&spec.name),
                    "harness_only tool '{}' should not appear in codegg_core_min model audience",
                    spec.name
                );
            }
        }
    }

    #[test]
    fn harness_only_excluded_from_model_audience_codegg_core() {
        let model_tools = tool_names_for_profile_audience("codegg_core", ToolListAudience::Model);
        for spec in ALL_TOOLS {
            if spec.exposure == ToolExposure::HarnessOnly && spec.profiles.contains(&"codegg_core")
            {
                assert!(
                    !model_tools.contains(&spec.name),
                    "harness_only tool '{}' should not appear in codegg_core model audience",
                    spec.name
                );
            }
        }
    }

    #[test]
    fn harness_only_excluded_from_model_audience_full() {
        let model_tools = tool_names_for_profile_audience("full", ToolListAudience::Model);
        for spec in ALL_TOOLS {
            if spec.exposure == ToolExposure::HarnessOnly {
                assert!(
                    !model_tools.contains(&spec.name),
                    "harness_only tool '{}' should not appear in full model audience",
                    spec.name
                );
            }
        }
    }

    // -- Harness audience includes expected preflight tools --

    #[test]
    fn harness_audience_includes_harness_only_tools() {
        let harness_tools =
            tool_names_for_profile_audience("codegg_preflight", ToolListAudience::Harness);
        // codegg_preflight should include harness-only tools
        let harness_only_in_profile: Vec<&str> = ALL_TOOLS
            .iter()
            .filter(|t| {
                t.exposure == ToolExposure::HarnessOnly && t.profiles.contains(&"codegg_preflight")
            })
            .map(|t| t.name)
            .collect();
        for name in &harness_only_in_profile {
            assert!(
                harness_tools.contains(name),
                "harness audience for codegg_preflight should include '{}'",
                name
            );
        }
    }

    // -- Profile snapshots --
    //
    // These are exact snapshots of critical codegg-facing profile+audience
    // combinations. Update intentionally when profile contents change.
    // Keep sorted alphabetically before snapshotting (see snapshot_names helper).

    /// Helper: sorted tool names for a profile+audience.
    fn snapshot_names(profile: &str, audience: ToolListAudience) -> Vec<String> {
        let mut names: Vec<String> = tool_names_for_profile_audience(profile, audience)
            .into_iter()
            .map(String::from)
            .collect();
        names.sort();
        names
    }

    #[test]
    fn profile_snapshot_codegg_core_min_model() {
        let actual = snapshot_names("codegg_core_min", ToolListAudience::Model);
        let expected = vec![
            "command_preflight",
            "config_preflight",
            "edit_preflight",
            "text_replace_check",
            "text_security_inspect",
            "validate_json",
        ];
        assert_eq!(
            actual,
            expected.into_iter().map(String::from).collect::<Vec<_>>()
        );
    }

    #[test]
    fn profile_snapshot_codegg_core_model() {
        let actual = snapshot_names("codegg_core", ToolListAudience::Model);
        let expected = vec![
            "cargo_toml_inspect",
            "command_preflight",
            "config_preflight",
            "edit_preflight",
            "identifier_inspect",
            "path_normalize",
            "structured_data_compare",
            "text_diff_explain",
            "text_equal",
            "text_fingerprint",
            "text_inspect",
            "text_replace_check",
            "text_security_inspect",
            "validate_json",
            "validate_toml",
        ];
        assert_eq!(
            actual,
            expected.into_iter().map(String::from).collect::<Vec<_>>()
        );
    }

    #[test]
    fn profile_snapshot_codegg_preflight_harness() {
        let actual = snapshot_names("codegg_preflight", ToolListAudience::Harness);
        let expected = vec![
            "command_preflight",
            "config_preflight",
            "edit_preflight",
            "patch_apply_check",
            "path_scope_check",
            "prompt_input_inspect",
            "shell_split",
            "text_security_inspect",
            "unicode_policy_check",
        ];
        assert_eq!(
            actual,
            expected.into_iter().map(String::from).collect::<Vec<_>>()
        );
    }

    #[test]
    fn profile_snapshot_codegg_patch_model() {
        let actual = snapshot_names("codegg_patch", ToolListAudience::Model);
        let expected = vec![
            "edit_preflight",
            "line_range_compare",
            "line_range_extract",
            "patch_summary",
            "text_diff_explain",
            "text_replace_check",
        ];
        assert_eq!(
            actual,
            expected.into_iter().map(String::from).collect::<Vec<_>>()
        );
    }

    #[test]
    fn profile_snapshot_codegg_patch_harness() {
        let actual = snapshot_names("codegg_patch", ToolListAudience::Harness);
        let expected = vec![
            "edit_preflight",
            "line_range_compare",
            "line_range_extract",
            "patch_apply_check",
            "patch_summary",
            "text_diff_explain",
            "text_replace_check",
        ];
        assert_eq!(
            actual,
            expected.into_iter().map(String::from).collect::<Vec<_>>()
        );
    }

    #[test]
    fn profile_snapshot_codegg_config_model() {
        let actual = snapshot_names("codegg_config", ToolListAudience::Model);
        let expected = vec![
            "config_preflight",
            "dotenv_validate",
            "ini_validate",
            "json_canonicalize",
            "json_compare",
            "json_extract",
            "structured_data_compare",
            "toml_shape",
            "validate_json",
            "validate_schema_light",
            "validate_toml",
            "version_compare",
        ];
        assert_eq!(
            actual,
            expected.into_iter().map(String::from).collect::<Vec<_>>()
        );
    }

    #[test]
    fn profile_snapshot_codegg_config_harness() {
        let actual = snapshot_names("codegg_config", ToolListAudience::Harness);
        let expected = vec![
            "config_preflight",
            "dotenv_validate",
            "ini_validate",
            "json_canonicalize",
            "json_compare",
            "json_extract",
            "structured_data_compare",
            "toml_shape",
            "validate_json",
            "validate_schema_light",
            "validate_toml",
            "version_compare",
        ];
        assert_eq!(
            actual,
            expected.into_iter().map(String::from).collect::<Vec<_>>()
        );
    }

    #[test]
    fn profile_snapshot_codegg_shell_model() {
        let actual = snapshot_names("codegg_shell", ToolListAudience::Model);
        let expected = vec![
            "argv_compare",
            "command_preflight",
            "regex_safety_check",
            "shell_quote_join",
        ];
        assert_eq!(
            actual,
            expected.into_iter().map(String::from).collect::<Vec<_>>()
        );
    }

    #[test]
    fn profile_snapshot_codegg_shell_harness() {
        let actual = snapshot_names("codegg_shell", ToolListAudience::Harness);
        let expected = vec![
            "argv_compare",
            "command_preflight",
            "regex_safety_check",
            "shell_quote_join",
            "shell_split",
        ];
        assert_eq!(
            actual,
            expected.into_iter().map(String::from).collect::<Vec<_>>()
        );
    }

    #[test]
    fn profile_snapshot_codegg_unicode_security_model() {
        let actual = snapshot_names("codegg_unicode_security", ToolListAudience::Model);
        let expected = vec![
            "canonicalize_text",
            "identifier_inspect",
            "text_inspect",
            "text_position",
            "text_security_inspect",
            "text_transform",
        ];
        assert_eq!(
            actual,
            expected.into_iter().map(String::from).collect::<Vec<_>>()
        );
    }

    #[test]
    fn profile_snapshot_codegg_unicode_security_harness() {
        let actual = snapshot_names("codegg_unicode_security", ToolListAudience::Harness);
        let expected = vec![
            "canonicalize_text",
            "identifier_inspect",
            "prompt_input_inspect",
            "text_inspect",
            "text_position",
            "text_security_inspect",
            "text_transform",
            "unicode_policy_check",
        ];
        assert_eq!(
            actual,
            expected.into_iter().map(String::from).collect::<Vec<_>>()
        );
    }

    #[test]
    fn profile_snapshot_codegg_repo_audit_model() {
        let actual = snapshot_names("codegg_repo_audit", ToolListAudience::Model);
        let expected = vec![
            "cargo_toml_inspect",
            "code_fence_extract",
            "identifier_table_inspect",
            "json_shape",
            "markdown_structure",
            "text_fingerprint",
        ];
        assert_eq!(
            actual,
            expected.into_iter().map(String::from).collect::<Vec<_>>()
        );
    }

    #[test]
    fn profile_snapshot_human_math_model() {
        let names = snapshot_names("human_math", ToolListAudience::Model);
        assert!(!names.is_empty(), "human_math model should have tools");
    }

    #[test]
    fn profile_snapshot_default_model() {
        let names = snapshot_names("default", ToolListAudience::Model);
        assert!(!names.is_empty(), "default model should have tools");
        for name in &names {
            let spec = get_tool(name).expect("tool should exist");
            assert_ne!(spec.exposure, ToolExposure::HarnessOnly);
        }
    }

    #[test]
    fn preflight_exposure_policy_is_intentional() {
        // These tools are intentionally model-visible (Default exposure)
        // because they provide advisory feedback that helps models make
        // better decisions. The harness calls them automatically anyway.
        let model_preflight_tools = ["edit_preflight", "command_preflight", "config_preflight"];
        for name in &model_preflight_tools {
            let spec = get_tool(name).expect("tool should exist");
            assert_eq!(
                spec.exposure,
                ToolExposure::Default,
                "{} should be Default exposure (model-visible advisory)",
                name
            );
        }

        // These tools are intentionally harness-only because they enforce
        // safety policies that the model should not bypass or see results of.
        let harness_only_tools = [
            "patch_apply_check",
            "path_scope_check",
            "prompt_input_inspect",
            "unicode_policy_check",
            "shell_split",
        ];
        for name in &harness_only_tools {
            let spec = get_tool(name).expect("tool should exist");
            assert_eq!(
                spec.exposure,
                ToolExposure::HarnessOnly,
                "{} should be HarnessOnly exposure (enforcement tool)",
                name
            );
        }

        // text_security_inspect is model-visible because it's a composite tool
        // that models use for security hygiene on text input.
        let spec = get_tool("text_security_inspect").expect("tool should exist");
        assert_eq!(
            spec.exposure,
            ToolExposure::Default,
            "text_security_inspect should be Default (model-visible composite)"
        );
    }
}
