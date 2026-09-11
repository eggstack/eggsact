//! Progressive-discovery regression tests (plan 02 Part G).
//!
//! These tests serialize actual `tools/list` shapes and measure bytes
//! rather than relying on README counts. They enforce:
//! - discovery advertises at most pinned+facade count;
//! - discovery bytes materially below direct/full;
//! - search/invoke respect profile + audience (no bypass);
//! - deprecated hidden unless exact/included;
//! - recursive facade invocation rejected;
//! - search order byte-stable;
//! - max-limit schema detail bounded.

use eggsact::agent::{Profile, ToolAudience, ToolCallError, ToolRegistry};
use eggsact::mcp::discovery;
use eggsact::mcp::registry::{self, ToolListAudience, ToolStability};
use serde_json::Value;

#[derive(Debug, serde::Deserialize)]
struct IntentFixture {
    id: String,
    intent: String,
    primary: String,
    acceptable: Vec<String>,
    category: String,
    kind: String,
}

fn full_model_audience() -> ToolListAudience {
    ToolListAudience::Model
}

#[test]
fn discovery_full_model_advertises_at_most_seven() {
    let defs = discovery::discovery_legacy_definitions("full", full_model_audience(), None);
    assert!(
        defs.len() <= 7,
        "discovery legacy listing must be <=7, got {}",
        defs.len()
    );
    // Pinned 5 allowed in full/Model + 2 facades = 7.
    assert_eq!(defs.len(), 7);
    let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
    for pinned in discovery::PINNED_DISCOVERY_TOOLS {
        assert!(
            names.contains(pinned),
            "pinned tool '{}' must be advertised in full/Model discovery",
            pinned
        );
    }
    assert!(names.contains(&discovery::TOOL_SEARCH));
    assert!(names.contains(&discovery::TOOL_INVOKE));

    let modern = discovery::discovery_modern_values("full", full_model_audience(), None);
    assert_eq!(modern.len(), 7);
}

#[test]
fn discovery_narrow_profile_stays_constrained() {
    // codegg_patch/Model contains only edit_preflight among the pinned set.
    let defs =
        discovery::discovery_legacy_definitions("codegg_patch", ToolListAudience::Model, None);
    let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
    assert!(names.contains(&"edit_preflight"));
    assert!(names.contains(&discovery::TOOL_SEARCH));
    assert!(names.contains(&discovery::TOOL_INVOKE));
    assert!(
        !names.contains(&"math_eval"),
        "narrow profile must not leak pinned tools outside its capability set"
    );
    assert_eq!(defs.len(), 3);

    // human_math/Model contains only math_eval among the pinned set.
    let human =
        discovery::discovery_legacy_definitions("human_math", ToolListAudience::Model, None);
    let hnames: Vec<&str> = human.iter().map(|d| d.name.as_str()).collect();
    assert!(hnames.contains(&"math_eval"));
    assert!(!hnames.contains(&"edit_preflight"));
}

#[test]
fn discovery_bytes_materially_below_direct_full() {
    // Serialize the same way server.rs does for legacy + modern.
    let direct_legacy = registry::list_tool_definitions(registry::ToolListOptions {
        profile: "full",
        names: None,
        tier: None,
        tags: None,
        schema_detail: "full",
        audience: Some(ToolListAudience::Model),
    });
    let direct_bytes = serde_json::to_string(&serde_json::json!({"tools": direct_legacy}))
        .unwrap()
        .len();
    let disc_legacy =
        discovery::discovery_legacy_definitions("full", ToolListAudience::Model, None);
    let disc_bytes = serde_json::to_string(&serde_json::json!({"tools": disc_legacy}))
        .unwrap()
        .len();
    assert!(
        disc_bytes < direct_bytes / 2,
        "discovery legacy bytes ({}) must be < half of direct/full ({}))",
        disc_bytes,
        direct_bytes
    );

    let direct_modern = registry::list_modern_tool_values(registry::ToolListOptions {
        profile: "full",
        names: None,
        tier: None,
        tags: None,
        schema_detail: "full",
        audience: Some(ToolListAudience::Model),
    });
    let direct_modern_bytes = serde_json::to_string(&serde_json::json!({
        "resultType": "complete",
        "tools": direct_modern,
    }))
    .unwrap()
    .len();
    let disc_modern = discovery::discovery_modern_values("full", ToolListAudience::Model, None);
    let disc_modern_bytes = serde_json::to_string(&serde_json::json!({
        "resultType": "complete",
        "tools": disc_modern,
    }))
    .unwrap()
    .len();
    assert!(
        disc_modern_bytes < direct_modern_bytes / 2,
        "discovery modern bytes ({}) must be < half of direct/full ({}))",
        disc_modern_bytes,
        direct_modern_bytes
    );
}

#[test]
fn discovery_omits_output_schemas_and_bookkeeping() {
    let defs = discovery::discovery_legacy_definitions("full", ToolListAudience::Model, None);
    for d in &defs {
        assert!(
            d.output_schema.is_none(),
            "discovery legacy '{}' must omit outputSchema",
            d.name
        );
        assert!(d.tier.is_none(), "discovery '{}' must omit tier", d.name);
        assert!(d.tags.is_none(), "discovery '{}' must omit tags", d.name);
    }
    let modern = discovery::discovery_modern_values("full", ToolListAudience::Model, None);
    for v in &modern {
        let obj = v.as_object().unwrap();
        assert!(
            obj.get("outputSchema").is_none(),
            "discovery modern must omit outputSchema"
        );
        assert!(
            obj.get("_meta").is_none(),
            "discovery modern must omit namespaced bookkeeping _meta"
        );
        assert!(obj.get("annotations").is_some());
        assert!(obj.get("inputSchema").is_some());
    }
}

#[test]
fn all_stable_full_model_tools_searchable_by_exact_name() {
    let specs = registry::tools_for_profile_audience("full", ToolListAudience::Model);
    let stable: Vec<&&registry::ToolSpec> = specs
        .iter()
        .filter(|s| s.stability == ToolStability::Stable)
        .collect();
    assert!(!stable.is_empty());
    for spec in stable {
        let params =
            discovery::parse_search_params(&serde_json::json!({"query": spec.name})).unwrap();
        let hits = discovery::search_filtered(&specs, &params);
        assert!(
            hits.first().is_some_and(|(s, _)| s.name == spec.name),
            "stable tool '{}' must be top hit for exact-name search",
            spec.name
        );
    }
}

#[test]
fn all_stable_full_model_tools_invokable_through_router_policy() {
    // The facade reuses ToolRegistry::prepare_tool_call for the target, so
    // every stable Model tool must pass policy checks for Full/Model.
    let registry = ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Model);
    let specs = registry::tools_for_profile_audience("full", ToolListAudience::Model);
    for spec in specs {
        if spec.stability != ToolStability::Stable {
            continue;
        }
        // Minimal args per tool vary; policy check happens before schema
        // validation, so use empty object and accept either Ready or
        // InvalidArguments as "reachable" (both prove no profile/audience
        // bypass rejection). UnknownTool/ToolUnavailable/NotAllowed are failures.
        match registry.prepare_tool_call(spec.name, &Value::Object(Default::default())) {
            eggsact::agent::ToolCallOutcome::Ready { .. } => {}
            eggsact::agent::ToolCallOutcome::PreExecutionError(
                ToolCallError::InvalidArguments(_),
            ) => {}
            eggsact::agent::ToolCallOutcome::PreExecutionError(e) => {
                panic!(
                    "stable full/Model tool '{}' must be invokable through router, got: {}",
                    spec.name, e
                );
            }
        }
    }
}

#[test]
fn harness_only_targets_rejected_for_model_audience() {
    let registry = ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Model);
    // Known HarnessOnly tools in full profile.
    for target in [
        "patch_apply_check",
        "path_scope_check",
        "prompt_input_inspect",
        "unicode_policy_check",
        "shell_split",
    ] {
        match registry.prepare_tool_call(target, &Value::Object(Default::default())) {
            eggsact::agent::ToolCallOutcome::PreExecutionError(
                ToolCallError::ToolNotAllowedForAudience { .. },
            ) => {}
            other => panic!(
                "HarnessOnly '{}' must be rejected for Model audience, got: {}",
                target,
                match other {
                    eggsact::agent::ToolCallOutcome::Ready { .. } => "Ready".to_string(),
                    eggsact::agent::ToolCallOutcome::PreExecutionError(e) => e.to_string(),
                }
            ),
        }
    }
    // Search over Model-filtered specs must not return HarnessOnly tools.
    let specs = registry::tools_for_profile_audience("full", ToolListAudience::Model);
    assert!(specs
        .iter()
        .all(|s| s.exposure != registry::ToolExposure::HarnessOnly));
}

#[test]
fn narrow_profile_cannot_escape_through_router() {
    // codegg_patch must not reach math_eval even via generic invoke policy.
    let registry =
        ToolRegistry::with_profile_and_audience(Profile::CodeggPatch, ToolAudience::Model);
    match registry.prepare_tool_call("math_eval", &serde_json::json!({"expression": "1+1"})) {
        eggsact::agent::ToolCallOutcome::PreExecutionError(ToolCallError::ToolUnavailable {
            ..
        }) => {}
        other => panic!(
            "codegg_patch must not reach math_eval, got: {}",
            match other {
                eggsact::agent::ToolCallOutcome::Ready { .. } => "Ready".to_string(),
                eggsact::agent::ToolCallOutcome::PreExecutionError(e) => e.to_string(),
            }
        ),
    }
    // And search over the narrow profile must not surface it either.
    let specs = registry::tools_for_profile_audience("codegg_patch", ToolListAudience::Model);
    let params =
        discovery::parse_search_params(&serde_json::json!({"query": "math_eval", "limit": 10}))
            .unwrap();
    let hits = discovery::search_filtered(&specs, &params);
    // math_eval is not in codegg_patch; exact-name search over the filtered
    // slice finds nothing (presentation never widens capability).
    assert!(
        hits.iter().all(|(s, _)| s.name != "math_eval"),
        "narrow-profile search must not return tools outside the profile"
    );
}

#[test]
fn deprecated_hidden_from_ordinary_search() {
    let specs = registry::tools_for_profile_audience("full", ToolListAudience::Model);
    let params =
        discovery::parse_search_params(&serde_json::json!({"query": "json pointer extract"}))
            .unwrap();
    let hits = discovery::search_filtered(&specs, &params);
    assert!(hits.iter().all(|(s, _)| s.name != "json_query"));
    // Exact name still resolves with replacement hint.
    let exact =
        discovery::parse_search_params(&serde_json::json!({"query": "json_query"})).unwrap();
    let hits = discovery::search_filtered(&specs, &exact);
    let found = hits.iter().find(|(s, _)| s.name == "json_query");
    assert!(found.is_some(), "exact deprecated name must resolve");
    let val = discovery::matches_value(&hits, false);
    let entry = val
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == "json_query")
        .unwrap();
    assert_eq!(entry["deprecated"], true);
    assert_eq!(entry["replacement"], "json_extract");
}

#[test]
fn recursive_facade_invocation_rejected() {
    for facade in [discovery::TOOL_SEARCH, discovery::TOOL_INVOKE] {
        let err = discovery::parse_invoke_params(&serde_json::json!({"name": facade})).unwrap_err();
        assert!(
            err.contains("recursive"),
            "facade '{}' must be rejected as recursive, got: {}",
            facade,
            err
        );
    }
}

#[test]
fn search_order_byte_stable() {
    let specs = registry::tools_for_profile_audience("full", ToolListAudience::Model);
    let params =
        discovery::parse_search_params(&serde_json::json!({"query": "patch diff", "limit": 5}))
            .unwrap();
    let a = discovery::search_filtered(&specs, &params);
    let b = discovery::search_filtered(&specs, &params);
    let sa = serde_json::to_string(&discovery::matches_value(&a, false)).unwrap();
    let sb = serde_json::to_string(&discovery::matches_value(&b, false)).unwrap();
    assert_eq!(sa, sb);
}

#[test]
fn search_result_size_bounded_for_max_schema_detail() {
    let specs = registry::tools_for_profile_audience("full", ToolListAudience::Model);
    let params = discovery::parse_search_params(
        &serde_json::json!({"query": "text", "limit": 10, "detail": "schema"}),
    )
    .unwrap();
    let hits = discovery::search_filtered(&specs, &params);
    assert!(hits.len() <= 10);
    let val = discovery::matches_value(&hits, true);
    let bytes = serde_json::to_string(&val).unwrap().len();
    // 10 full input schemas must still fit comfortably under the 1MB
    // request/response envelope (generous 200KB bound for the matches array).
    assert!(
        bytes < 200_000,
        "max-limit schema search ({} bytes) must stay bounded",
        bytes
    );
    // Summary detail is smaller than schema detail for the same hits.
    let summary = discovery::matches_value(&hits, false);
    let summary_bytes = serde_json::to_string(&summary).unwrap().len();
    assert!(summary_bytes <= bytes);
}

#[test]
fn discovery_fixture_retrieval_meets_rollout_gates() {
    let fixtures: Vec<IntentFixture> =
        serde_json::from_str(include_str!("../fixtures/tool_discovery_intents.json"))
            .expect("discovery intent fixture JSON must parse");
    assert!(fixtures.len() >= 40, "fixture corpus must stay substantial");

    let specs = registry::tools_for_profile_audience("full", ToolListAudience::Model);
    let model_names: std::collections::HashSet<&str> = specs.iter().map(|s| s.name).collect();
    // A1: derive the required stable full/Model coverage set from the registry.
    // No hard-coded count: registry membership is the source of truth.
    let stable_model_names: std::collections::HashSet<&str> = specs
        .iter()
        .filter(|s| s.stability == ToolStability::Stable)
        .map(|s| s.name)
        .collect();
    // Positive semantic targets: primary or explicitly acceptable equivalent.
    // Exact-name reachability is covered separately and does not satisfy this.
    let mut positive_targets: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut positive_intents = 0usize;
    let mut negative_intents = 0usize;
    for fixture in &fixtures {
        if fixture.kind == "negative_selection" {
            negative_intents += 1;
        } else {
            positive_intents += 1;
            positive_targets.insert(fixture.primary.as_str());
            for acceptable in &fixture.acceptable {
                positive_targets.insert(acceptable.as_str());
            }
        }
    }
    let missing: Vec<&&str> = stable_model_names
        .iter()
        .filter(|name| !positive_targets.contains(**name))
        .collect();
    assert!(
        missing.is_empty(),
        "semantic coverage gap: every stable full/Model tool needs a task-oriented fixture; missing: {:?}",
        missing
    );
    // A3: report coverage separately so a perfect score on a smaller corpus
    // cannot hide missing capability coverage.
    eprintln!(
        "stable Model-visible tools       {}\npositive semantic targets        {}\ncoverage                         100%\npositive intent count            {}\nnegative/containment count       {}",
        stable_model_names.len(),
        positive_targets.len(),
        positive_intents,
        negative_intents
    );
    let mut evaluated = 0usize;
    let mut top1 = 0usize;
    let mut top3 = 0usize;
    let mut top5 = 0usize;
    let mut top1_failures = Vec::new();
    let mut top3_failures = Vec::new();

    for fixture in fixtures {
        assert!(!fixture.id.is_empty());
        assert!(!fixture.category.is_empty());
        if fixture.kind == "negative_selection" {
            assert!(
                !model_names.contains(fixture.primary.as_str()),
                "negative fixture '{}' targets a Model-visible tool; HarnessOnly/Hidden must not be counted as positive Model coverage",
                fixture.id
            );
            let params = discovery::parse_search_params(&serde_json::json!({
                "query": fixture.intent,
                "limit": 10
            }))
            .unwrap();
            let hits = discovery::search_filtered(&specs, &params);
            assert!(
                hits.iter().all(|(spec, _)| spec.name != fixture.primary),
                "Model search leaked negative target '{}' for fixture '{}'",
                fixture.primary,
                fixture.id
            );
            continue;
        }
        // Positive fixtures must be task-oriented, not bare canonical names,
        // must target Model-visible stable tools, and must not promote deprecated
        // tools as ordinary positive targets.
        assert!(
            model_names.contains(fixture.primary.as_str()),
            "fixture '{}' primary '{}' must be Model-visible",
            fixture.id,
            fixture.primary
        );
        let primary_spec = specs
            .iter()
            .find(|s| s.name == fixture.primary)
            .expect("primary must resolve in full/Model");
        assert!(
            primary_spec.stability == ToolStability::Stable,
            "fixture '{}' primary '{}' must be Stable; deprecated tools stay migration/negative cases",
            fixture.id,
            fixture.primary
        );
        let normalized_intent = fixture
            .intent
            .trim()
            .to_lowercase()
            .replace(['_', '-'], " ");
        let normalized_primary = fixture.primary.to_lowercase().replace('_', " ");
        assert!(
            normalized_intent != normalized_primary
                && normalized_intent.len() > normalized_primary.len() + 10,
            "fixture '{}' must be task-oriented phrasing, not a bare canonical name",
            fixture.id
        );
        evaluated += 1;
        let params = discovery::parse_search_params(&serde_json::json!({
            "query": fixture.intent,
            "limit": 5
        }))
        .unwrap();
        let hits = discovery::search_filtered(&specs, &params);
        let names: Vec<&str> = hits.iter().map(|(spec, _)| spec.name).collect();
        let accepted =
            |name: &str| name == fixture.primary || fixture.acceptable.iter().any(|a| a == name);
        if names.first().is_some_and(|name| accepted(name)) {
            top1 += 1;
        } else {
            top1_failures.push((fixture.id.clone(), fixture.primary.clone(), names.clone()));
        }
        if names.iter().take(3).any(|name| accepted(name)) {
            top3 += 1;
        } else {
            top3_failures.push((fixture.id.clone(), fixture.primary.clone(), names.clone()));
        }
        if names.iter().take(5).any(|name| accepted(name)) {
            top5 += 1;
        }
        assert!(
            names.iter().take(5).any(|name| accepted(name)),
            "fixture '{}' did not retrieve '{}' in top five: {:?}",
            fixture.id,
            fixture.primary,
            names
        );
    }

    assert!(evaluated >= 40);
    assert!(
        evaluated >= stable_model_names.len(),
        "positive intents ({evaluated}) must cover every stable Model tool ({})",
        stable_model_names.len()
    );
    assert!(
        top1 * 100 >= evaluated * 90,
        "top-1 {top1}/{evaluated}; failures: {top1_failures:?}"
    );
    assert!(
        top3 * 100 >= evaluated * 98,
        "top-3 {top3}/{evaluated}; failures: {top3_failures:?}"
    );
    assert_eq!(top5, evaluated, "top-5 must recall every positive fixture");
}

#[test]
fn discovery_metrics_enforce_context_budget() {
    let direct = eggsact::mcp::discovery_eval::direct_metrics("full", ToolListAudience::Model);
    let discovery =
        eggsact::mcp::discovery_eval::discovery_metrics("full", ToolListAudience::Model);
    assert_eq!(direct.advertised_tools, 77);
    assert_eq!(discovery.advertised_tools, 7);
    assert!(
        discovery.serialized_bytes * 100 <= direct.serialized_bytes * 25,
        "discovery {} bytes must be <=25% of direct {} bytes",
        discovery.serialized_bytes,
        direct.serialized_bytes
    );
}

#[test]
fn portable_model_evaluation_scenarios_are_well_formed() {
    let scenarios: Vec<Value> =
        serde_json::from_str(include_str!("../fixtures/tool_discovery_scenarios.json"))
            .expect("portable discovery scenario JSON must parse");
    assert!(scenarios.len() >= 40);
    let specs = registry::tools_for_profile_audience("full", ToolListAudience::Model);
    let model_names: std::collections::HashSet<&str> = specs.iter().map(|s| s.name).collect();
    let mut model_tasks = 0usize;
    let mut containment = 0usize;
    let mut seen_ids = std::collections::HashSet::new();
    for scenario in &scenarios {
        let id = scenario
            .get("id")
            .and_then(|v| v.as_str())
            .expect("scenario missing id");
        assert!(
            seen_ids.insert(id.to_string()),
            "duplicate scenario id '{id}'"
        );
        let audience = scenario
            .get("audience")
            .and_then(|v| v.as_str())
            .expect("scenario missing audience");
        let kind = scenario
            .get("kind")
            .and_then(|v| v.as_str())
            .expect("scenario missing kind");
        assert!(
            ["model", "harness"].contains(&audience),
            "scenario '{id}' has unknown audience '{audience}'"
        );
        assert!(
            ["task", "must_not_expose"].contains(&kind),
            "scenario '{id}' has unknown kind '{kind}'"
        );
        for field in ["user_request", "success_criteria", "search_expected"] {
            assert!(
                scenario.get(field).is_some(),
                "scenario '{id}' missing {field}"
            );
        }
        assert!(
            scenario["search_expected"].is_boolean(),
            "scenario '{id}' search_expected must be boolean"
        );
        if kind == "task" {
            let expected = scenario
                .get("expected_tools")
                .expect("task scenario missing expected_tools");
            assert!(
                expected.is_array() && !expected.as_array().unwrap().is_empty(),
                "task scenario '{id}' needs a non-empty expected_tools set"
            );
            // Plural expected/acceptable sets are canonical: several tasks admit
            // more than one valid capability. The offline scorer accepts the same
            // names so fixtures and traces agree.
            if let Some(acceptable) = scenario.get("acceptable_tools") {
                assert!(
                    acceptable.is_array(),
                    "scenario '{id}' acceptable_tools must be array"
                );
            }
            if audience == "model" {
                model_tasks += 1;
                for tool in expected.as_array().unwrap() {
                    let name = tool.as_str().expect("expected_tools must be strings");
                    assert!(
                        model_names.contains(name),
                        "Model task '{id}' targets HarnessOnly '{name}'; use must_not_expose containment or harness audience"
                    );
                }
            }
        } else {
            // must_not_expose containment: forbidden_tools, no expected_tools.
            let forbidden = scenario
                .get("forbidden_tools")
                .expect("containment scenario missing forbidden_tools");
            assert!(
                forbidden.is_array() && !forbidden.as_array().unwrap().is_empty(),
                "containment scenario '{id}' needs non-empty forbidden_tools"
            );
            assert!(
                scenario.get("expected_tools").is_none(),
                "containment scenario '{id}' must not carry expected_tools"
            );
            if audience == "model" {
                containment += 1;
            }
        }
    }
    // B2: preserve a portable Model benchmark of roughly 40-60 tasks after the
    // containment split. Harness tasks never join the Model denominator.
    assert!(
        (40..=60).contains(&model_tasks),
        "Model task scenarios must stay 40-60 after containment split, got {model_tasks}"
    );
    assert!(
        containment >= 4,
        "expected at least the four reclassified HarnessOnly containment cases, got {containment}"
    );
}
