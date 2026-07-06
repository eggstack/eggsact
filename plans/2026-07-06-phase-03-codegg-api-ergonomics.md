# Phase 03: codegg API ergonomics and typed wrappers

## Purpose

Make the intended codegg integration path explicit, safe, and low-friction. The repository already has a strong in-process API through `ToolRegistry`, `Profile`, `ToolAudience`, and `ExecutionContext`; this phase tightens the public surface so codegg agents do not accidentally expose harness-only tools, misuse legacy listing APIs, or hand-build JSON for hot preflight calls.

## Current observations

`ToolRegistry::available_tools()` is documented as legacy and not model-safe because it filters hidden tools but does not exclude harness-only tools. The safer APIs are `available_tools_model_safe()`, `available_tools_for_audience(ToolAudience::Model)`, and harness-specific listing through `ToolAudience::Harness`. The method name `available_tools()` is generic enough that a codegg integration can choose it by mistake.

`ExecutionContext` already bundles request-scoped state: eval context, compatibility mode, profile, audience, budget, cancellation, request ID, and source. It should become the preferred in-process call path for codegg where stateful or budget-aware dispatch is needed.

Typed preflight wrappers already exist for some workflows. Expanding wrappers to the common route-critical and repo-audit paths will reduce JSON shape mistakes and improve codegg maintainability.

## Implementation plan

1. Deprecate or relabel unsafe legacy listing.

   If semver policy allows, add:

   ```rust
   #[deprecated(note = "use available_tools_model_safe, available_tools_for_audience, or available_tools_for_current_audience")]
   ```

   to `ToolRegistry::available_tools()`.

   If deprecation would create too much warning noise for existing users, add a stronger doc comment and introduce a clearer alias such as `available_tools_debug_legacy()` or `available_tools_non_hidden_legacy()`. Then update examples and docs to avoid `available_tools()` in model-facing contexts.

2. Update crate-level and agent-module examples.

   The docs should show three common patterns:

   - Model-facing codegg session:

     ```rust
     let registry = ToolRegistry::with_profile_and_audience(Profile::CodeggCore, ToolAudience::Model);
     let tools = registry.available_tools_model_safe();
     ```

   - Harness preflight:

     ```rust
     let registry = ToolRegistry::with_profile_and_audience(Profile::CodeggPreflight, ToolAudience::Harness);
     ```

   - Context-aware execution:

     ```rust
     let ctx = ExecutionContext::agent_default(Profile::CodeggCore, ToolAudience::Model)
         .with_request_id("...");
     let response = registry.call_json_with_execution_context("text_equal", args, &ctx)?;
     ```

3. Document profile selection for codegg.

   Add a compact table to README or architecture docs:

   - `codegg_core_min`: always-on cheap core safety and edit/config gates.
   - `codegg_core`: general coding-agent deterministic helpers.
   - `codegg_preflight`: harness-side pre-apply and pre-exec checks.
   - `codegg_patch`: patch/edit validation and summarization.
   - `codegg_config`: config and dependency-file validation.
   - `codegg_shell`: shell command parsing and preflight.
   - `codegg_unicode_security`: suspicious Unicode and identifier inspection.
   - `codegg_repo_audit`: repository-level structural inspection.

   Explicitly state which profiles are intended for model-visible listing and which are primarily harness-selected.

4. Add typed wrapper coverage.

   Add or complete typed wrappers for these workflows:

   - `EditPreflight`
   - `CommandPreflight`
   - `ConfigPreflight`
   - `DependencyEditPreflight`
   - `RepoManifestInspect`
   - `ConfigFileInspect`
   - `TextSecurityInspect`
   - `PatchSummary`
   - `PatchApplyCheck` for harness use only

   Each wrapper should have typed input and output structs, a `run` method, and a conversion path to/from the underlying JSON tool response. Outputs should expose at least `verdict`, `machine_code`, `findings`, and any core summary fields without requiring JSON indexing.

5. Normalize wrapper error behavior.

   Wrappers should return `Result<Output, PreflightError>` or a similarly consistent error type. Missing mandatory wrapper fields should produce contract errors before dispatch. Tool-level invalid input should be represented predictably rather than silently becoming `Value::Null`.

6. Add wrapper tests.

   For each wrapper, add:

   - Minimal valid input test.
   - Missing mandatory field test.
   - Review/block test if the tool has verdict semantics.
   - Machine code extraction test.
   - Profile/audience test where relevant.

7. Update generated docs/tool cards.

   If typed wrappers are documented manually, ensure they do not conflict with generated MCP tool docs. MCP schemas remain generated from `ToolSpec`; wrapper docs should point to the underlying tool names.

## Acceptance criteria

- Public examples use model-safe listing by default.
- Legacy `available_tools()` is deprecated or clearly marked as not model-safe in docs and examples.
- `ExecutionContext` has clear codegg examples for model-facing and harness calls.
- Typed wrappers exist for the common codegg route-critical and repo-audit workflows.
- Wrapper tests cover success, invalid contract, and verdict/machine-code extraction.
- Generated docs and clippy/tests pass.

## Risks and constraints

Do not remove `available_tools()` outright unless intentionally making a breaking release. Do not make wrappers diverge from underlying tool schemas; wrappers are ergonomic frontends, not separate semantics. Keep wrapper outputs stable and avoid overfitting to internal JSON shapes that are not part of the tool contract.

## Handoff notes

Begin by updating docs/examples before writing many wrappers. That will clarify the intended integration surface and reveal naming gaps. Then implement wrappers in small groups, starting with `EditPreflight`, `CommandPreflight`, and `TextSecurityInspect`, because those are the highest-leverage codegg workflows.
