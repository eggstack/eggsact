use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::process;

use eggsact::mcp::registry::all_tools_vec;
use eggsact::mcp::registry::tools_for_profile_audience;
use eggsact::mcp::registry::{
    available_profiles, ToolCost, ToolExposure, ToolListAudience, ToolSpec, ToolStability,
};

const BEGIN_TOOLS: &str = "<!-- BEGIN GENERATED: eggsact tools -->";
const END_TOOLS: &str = "<!-- END GENERATED: eggsact tools -->";

const BEGIN_PROFILES: &str = "<!-- BEGIN GENERATED: profile reference -->";
const END_PROFILES: &str = "<!-- END GENERATED: profile reference -->";

const REGENERATE_COMMAND: &str = "cargo run --bin generate-docs";

const CODEGG_PROFILES: &[&str] = &[
    "codegg_core_min",
    "codegg_core",
    "codegg_preflight",
    "codegg_patch",
    "codegg_config",
    "codegg_unicode_security",
    "codegg_shell",
    "codegg_repo_audit",
];

const CATEGORY_ORDER: &[&str] = &[
    "math",
    "text",
    "json",
    "regex",
    "validation",
    "list",
    "path",
    "shell",
    "markdown",
    "config",
    "identifier",
    "unicode",
    "version",
    "toml",
    "patch",
    "cargo",
    "dependency",
    "repo",
];

fn exposure_short(e: &ToolExposure) -> &'static str {
    match e {
        ToolExposure::Default => "default",
        ToolExposure::Contextual => "contextual",
        ToolExposure::ExpertOnly => "expert",
        ToolExposure::HarnessOnly => "harness",
        ToolExposure::Hidden => "hidden",
    }
}

fn cost_short(c: &ToolCost) -> &'static str {
    match c {
        ToolCost::Cheap => "cheap",
        ToolCost::Moderate => "mod",
        ToolCost::Heavy => "heavy",
    }
}

fn stability_short(s: &ToolStability) -> &'static str {
    match s {
        ToolStability::Stable => "stable",
        ToolStability::Deprecated => "deprecated",
        ToolStability::Experimental => "exp",
    }
}

fn profile_display(spec: &ToolSpec) -> String {
    let mut profiles: Vec<&str> = spec.profiles.to_vec();
    profiles.sort();
    profiles.join(", ")
}

fn generate_readme_tools() -> String {
    let visible_tools: Vec<&ToolSpec> = all_tools_vec()
        .iter()
        .filter(|spec| spec.exposure != ToolExposure::Hidden)
        .collect();
    let mut by_category: BTreeMap<&str, Vec<&ToolSpec>> = BTreeMap::new();
    for spec in &visible_tools {
        by_category.entry(spec.category).or_default().push(spec);
    }

    let mut out = String::new();
    out.push_str(&format!("{} tools across {} categories. See `architecture/mcp-server.md` for the full reference.\n\n", visible_tools.len(), by_category.len()));

    for &cat in CATEGORY_ORDER {
        let tools = match by_category.get(cat) {
            Some(t) => t,
            None => continue,
        };
        out.push_str(&format!("### {} ({})\n\n", capitalize(cat), tools.len()));
        out.push_str("| Tool | Tier | Exposure | Stability | Cost | Profiles |\n");
        out.push_str("|------|------|----------|-----------|------|----------|\n");
        for spec in tools {
            out.push_str(&format!(
                "| `{}` | {} | {} | {} | {} | {} |\n",
                spec.name,
                spec.tier,
                exposure_short(&spec.exposure),
                stability_short(&spec.stability),
                cost_short(&spec.cost),
                profile_display(spec),
            ));
        }
        out.push('\n');
    }

    out
}

fn generate_profile_reference() -> String {
    let profiles = available_profiles();
    let mut out = String::new();
    out.push_str(
        "| Profile | Model Tools | Harness Tools | Model Tool Names | Harness-Only Tools |\n",
    );
    out.push_str(
        "|---------|-------------|---------------|------------------|--------------------|\n",
    );

    for &profile in profiles {
        let model_tools = tools_for_profile_audience(profile, ToolListAudience::Model);
        let harness_tools = tools_for_profile_audience(profile, ToolListAudience::Harness);
        let model_count = model_tools.len();
        let harness_count = harness_tools.len();

        let mut model_names: Vec<&str> = model_tools.iter().map(|s| s.name).collect();
        model_names.sort();
        let model_names_str = model_names
            .iter()
            .map(|n| format!("`{}`", n))
            .collect::<Vec<_>>()
            .join(", ");

        let model_set: std::collections::HashSet<&str> =
            model_tools.iter().map(|s| s.name).collect();
        let mut harness_only: Vec<&str> = harness_tools
            .iter()
            .filter(|s| !model_set.contains(s.name))
            .map(|s| s.name)
            .collect();
        harness_only.sort();
        let harness_only_str = harness_only
            .iter()
            .map(|n| format!("`{}`", n))
            .collect::<Vec<_>>()
            .join(", ");

        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} |\n",
            profile, model_count, harness_count, model_names_str, harness_only_str
        ));
    }

    out
}

fn required_args(schema: &serde_json::Value) -> Vec<(String, String)> {
    let mut result = Vec::new();
    let required = match schema.get("required").and_then(|r| r.as_array()) {
        Some(arr) => arr,
        None => return result,
    };
    let props = match schema.get("properties").and_then(|p| p.as_object()) {
        Some(obj) => obj,
        None => return result,
    };
    for req in required {
        if let Some(name) = req.as_str() {
            let typ = props
                .get(name)
                .and_then(|p| p.get("type"))
                .and_then(|t| t.as_str())
                .unwrap_or("any");
            result.push((name.to_string(), typ.to_string()));
        }
    }
    result
}

fn generate_tool_card(spec: &ToolSpec, profile: &str) -> String {
    let schema = (spec.input_schema)();
    let req = required_args(&schema);

    let mut card = format!("### `{}`\n\n", spec.name);
    card.push_str(&format!("{}\n\n", spec.description));
    card.push_str(&format!(
        "- **Tier**: {} | **Cost**: {} | **Stability**: {}\n",
        spec.tier,
        cost_short(&spec.cost),
        stability_short(&spec.stability)
    ));
    card.push_str(&format!(
        "- **Exposure**: {}\n",
        exposure_short(&spec.exposure)
    ));
    card.push_str(&format!("- **Profile**: `{}`\n", profile));

    if spec.composite {
        card.push_str("- **Composite**: yes\n");
    }

    if !req.is_empty() {
        card.push_str("- **Required args**:\n");
        for (name, typ) in &req {
            card.push_str(&format!("  - `{}` ({})\n", name, typ));
        }
    } else {
        card.push_str("- **Required args**: none\n");
    }

    if !spec.aliases.is_empty() {
        card.push_str(&format!(
            "- **Aliases**: {}\n",
            spec.aliases
                .iter()
                .map(|a| format!("`{}`", a))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    card.push('\n');
    card
}

fn generate_tool_cards() -> String {
    let mut out = String::new();
    out.push_str("# Tool Cards\n\n");
    out.push_str(
        "Generated from the ToolSpec registry. Each section corresponds to a codegg profile.\n\n",
    );

    for &profile in CODEGG_PROFILES {
        let tools: Vec<&ToolSpec> = tools_for_profile_audience(profile, ToolListAudience::Model)
            .into_iter()
            .filter(|spec| spec.exposure != ToolExposure::Hidden)
            .collect();
        if tools.is_empty() {
            continue;
        }
        out.push_str(&format!("## `{}`\n\n", profile));

        let mut sorted = tools;
        sorted.sort_by_key(|s| s.name);

        for spec in sorted {
            out.push_str(&generate_tool_card(spec, profile));
        }
    }

    out
}

fn extract_between<'a>(content: &'a str, begin: &str, end: &str) -> Option<&'a str> {
    let start = content.find(begin)? + begin.len();
    let rest = &content[start..];
    let end_pos = rest.find(end)?;
    Some(&rest[..end_pos])
}

fn replace_between(content: &str, begin: &str, end: &str, replacement: &str) -> String {
    let start = content.find(begin).expect("marker not found");
    let after_begin = start + begin.len();
    let rest = &content[after_begin..];
    let end_pos = rest.find(end).expect("end marker not found");
    let after_end = after_begin + end_pos + end.len();

    let mut result = String::with_capacity(content.len());
    result.push_str(&content[..after_begin]);
    result.push('\n');
    result.push_str(replacement);
    result.push('\n');
    result.push_str(&content[after_end..]);
    result
}

fn read_file(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("error: cannot read {}: {}", path, e);
        process::exit(1);
    })
}

fn write_file(path: &str, content: &str) {
    fs::write(path, content).unwrap_or_else(|e| {
        eprintln!("error: cannot write {}: {}", path, e);
        process::exit(1);
    });
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().to_string() + c.as_str(),
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let check_mode = args.iter().any(|a| a == "--check");
    let output_dir = args
        .windows(2)
        .find(|w| w[0] == "--output-dir")
        .map(|w| w[1].clone())
        .unwrap_or_else(|| ".".to_string());

    let readme_content = generate_readme_tools();
    let profile_content = generate_profile_reference();
    let cards_content = generate_tool_cards();

    let mut stale_files = Vec::new();

    // Check/update README.md
    let readme_path = format!("{}/README.md", output_dir);
    let readme_file = read_file(&readme_path);
    if readme_file.contains(BEGIN_TOOLS) && readme_file.contains(END_TOOLS) {
        let existing = extract_between(&readme_file, BEGIN_TOOLS, END_TOOLS).unwrap_or("");
        if existing.trim() != readme_content.trim() {
            stale_files.push(readme_path.clone());
            if !check_mode {
                let updated =
                    replace_between(&readme_file, BEGIN_TOOLS, END_TOOLS, &readme_content);
                write_file(&readme_path, &updated);
            }
        }
    } else {
        // Markers not present — insert after "## MCP Tools" heading
        stale_files.push(readme_path.clone());
        if !check_mode {
            let marker_section = format!("\n{}\n{}\n{}\n", BEGIN_TOOLS, readme_content, END_TOOLS);
            if let Some(pos) = readme_file.find("## MCP Tools") {
                let heading_end = pos + "## MCP Tools".len();
                let rest = &readme_file[heading_end..];
                // Find the next line after the heading that isn't blank
                let insert_at = heading_end + rest.find('\n').unwrap_or(0) + 1;
                let mut updated = String::with_capacity(readme_file.len() + marker_section.len());
                updated.push_str(&readme_file[..insert_at]);
                updated.push_str(&marker_section);
                updated.push_str(&readme_file[insert_at..]);
                write_file(&readme_path, &updated);
            }
        }
    }

    // Check/update architecture/mcp-server.md
    let arch_path = format!("{}/architecture/mcp-server.md", output_dir);
    let arch_file = read_file(&arch_path);
    if arch_file.contains(BEGIN_PROFILES) && arch_file.contains(END_PROFILES) {
        let existing = extract_between(&arch_file, BEGIN_PROFILES, END_PROFILES).unwrap_or("");
        if existing.trim() != profile_content.trim() {
            stale_files.push(arch_path.clone());
            if !check_mode {
                let updated =
                    replace_between(&arch_file, BEGIN_PROFILES, END_PROFILES, &profile_content);
                write_file(&arch_path, &updated);
            }
        }
    } else {
        stale_files.push(arch_path.clone());
        if !check_mode {
            let marker_section = format!(
                "\n{}\n{}\n{}\n",
                BEGIN_PROFILES, profile_content, END_PROFILES
            );
            // Insert after "### Profile Reference" heading
            if let Some(pos) = arch_file.find("### Profile Reference") {
                let heading_end = pos + "### Profile Reference".len();
                let rest = &arch_file[heading_end..];
                let insert_at = heading_end + rest.find('\n').unwrap_or(0) + 1;
                let mut updated = String::with_capacity(arch_file.len() + marker_section.len());
                updated.push_str(&arch_file[..insert_at]);
                updated.push_str(&marker_section);
                updated.push_str(&arch_file[insert_at..]);
                write_file(&arch_path, &updated);
            }
        }
    }

    // Check/update generated/tool-cards.md
    let cards_path = format!("{}/generated/tool-cards.md", output_dir);
    let cards_file = fs::read_to_string(&cards_path).unwrap_or_default();
    if cards_file.trim() != cards_content.trim() {
        stale_files.push(cards_path.clone());
        if !check_mode {
            let dir = format!("{}/generated", output_dir);
            fs::create_dir_all(&dir).unwrap_or_else(|e| {
                eprintln!("error: cannot create {}: {}", dir, e);
                process::exit(1);
            });
            write_file(&cards_path, &cards_content);
        }
    }

    if check_mode && !stale_files.is_empty() {
        eprintln!("Stale generated docs:");
        for f in &stale_files {
            eprintln!("  {}", f);
        }
        eprintln!("Run `{REGENERATE_COMMAND}` to regenerate.");
        process::exit(1);
    }

    if !check_mode {
        if stale_files.is_empty() {
            eprintln!("All generated docs are up to date.");
        } else {
            eprintln!("Generated docs updated:");
            for f in &stale_files {
                eprintln!("  {}", f);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eggsact::mcp::registry::{
        all_tools_vec, tools_for_profile_audience, ToolExposure, ToolListAudience,
    };

    #[test]
    fn tool_table_contains_all_non_hidden_tools() {
        let table = generate_readme_tools();
        let all = all_tools_vec();
        let non_hidden: Vec<&str> = all
            .iter()
            .filter(|s| s.exposure != ToolExposure::Hidden)
            .map(|s| s.name)
            .collect();
        for name in &non_hidden {
            let backtick_name = format!("`{}`", name);
            assert!(
                table.contains(&backtick_name),
                "tool table missing {}",
                name
            );
        }
    }

    #[test]
    fn generated_readme_excludes_hidden_tools() {
        let table = generate_readme_tools();
        let all = all_tools_vec();
        let hidden: Vec<&str> = all
            .iter()
            .filter(|s| s.exposure == ToolExposure::Hidden)
            .map(|s| s.name)
            .collect();
        for name in &hidden {
            let backtick_name = format!("`{}`", name);
            assert!(
                !table.contains(&backtick_name),
                "tool table should not include hidden tool {}",
                name
            );
        }
        // If no hidden tools exist, the test still passes — it guards future additions.
    }

    #[test]
    fn generated_tool_cards_exclude_hidden_tools() {
        let cards = generate_tool_cards();
        let all = all_tools_vec();
        let hidden: Vec<&str> = all
            .iter()
            .filter(|s| s.exposure == ToolExposure::Hidden)
            .map(|s| s.name)
            .collect();
        for name in &hidden {
            let header = format!("### `{}`", name);
            assert!(
                !cards.contains(&header),
                "tool cards should not include hidden tool {}",
                name
            );
        }
        // If no hidden tools exist, the test still passes — it guards future additions.
    }

    #[test]
    fn profile_counts_match_registry() {
        let profile_ref = generate_profile_reference();
        for &profile in available_profiles() {
            let model_tools = tools_for_profile_audience(profile, ToolListAudience::Model);
            let harness_tools = tools_for_profile_audience(profile, ToolListAudience::Harness);
            let model_count = model_tools.len();
            let harness_count = harness_tools.len();
            let profile_line = format!("| `{}` | {} | {} |", profile, model_count, harness_count);
            assert!(
                profile_ref.contains(&profile_line),
                "profile reference missing or wrong count for {}: expected model={} harness={}",
                profile,
                model_count,
                harness_count
            );
        }
    }

    #[test]
    fn profile_reference_includes_harness_only_tools() {
        let profile_ref = generate_profile_reference();
        // For profiles that have harness-only tools, verify at least one appears in the output
        for &profile in available_profiles() {
            let model_tools = tools_for_profile_audience(profile, ToolListAudience::Model);
            let harness_tools = tools_for_profile_audience(profile, ToolListAudience::Harness);
            let model_set: std::collections::HashSet<&str> =
                model_tools.iter().map(|s| s.name).collect();
            let harness_only: Vec<&str> = harness_tools
                .iter()
                .filter(|s| !model_set.contains(s.name))
                .map(|s| s.name)
                .collect();
            if !harness_only.is_empty() {
                // At least one harness-only tool should appear in the profile reference line
                let profile_line = profile_ref
                    .lines()
                    .find(|l| l.contains(&format!("`{}`", profile)))
                    .unwrap_or("");
                for name in &harness_only {
                    assert!(
                        profile_line.contains(&format!("`{}`", name)),
                        "profile '{}' has harness-only tool '{}' but it's not in the profile reference",
                        profile,
                        name
                    );
                }
            }
        }
    }

    #[test]
    fn tool_cards_reference_only_known_tools() {
        let cards = generate_tool_cards();
        let all = all_tools_vec();
        let known: std::collections::HashSet<&str> = all.iter().map(|s| s.name).collect();
        for line in cards.lines() {
            if let Some(name) = line.strip_prefix("### `").and_then(|s| s.strip_suffix('`')) {
                assert!(
                    known.contains(name),
                    "tool card references unknown tool: {}",
                    name
                );
            }
        }
    }

    #[test]
    fn tool_card_required_args_match_schema() {
        let all = all_tools_vec();
        for spec in all {
            let schema = (spec.input_schema)();
            let req = required_args(&schema);
            let card = generate_tool_card(spec, "test_profile");
            if req.is_empty() {
                assert!(
                    card.contains("**Required args**: none"),
                    "{}: expected 'none' in card",
                    spec.name
                );
            } else {
                for (arg_name, _) in &req {
                    assert!(
                        card.contains(&format!("`{}`", arg_name)),
                        "{}: card missing required arg `{}`",
                        spec.name,
                        arg_name
                    );
                }
            }
        }
    }

    /// The stale-doc message must reference the actual cargo binary name
    /// (`generate-docs`, with a dash) so users can copy/paste it without
    /// hitting "no such binary". Cargo uses the `name` field from
    /// `[[bin]]` in `Cargo.toml`, not the source filename.
    #[test]
    fn stale_docs_message_uses_cargo_bin_name() {
        assert_eq!(REGENERATE_COMMAND, "cargo run --bin generate-docs");
        assert!(
            REGENERATE_COMMAND.contains("generate-docs"),
            "REGENERATE_COMMAND must use the dash form, got: {REGENERATE_COMMAND}"
        );
        assert!(
            !REGENERATE_COMMAND.contains("generate_docs"),
            "REGENERATE_COMMAND must not use the underscore form, got: {REGENERATE_COMMAND}"
        );
    }
}
