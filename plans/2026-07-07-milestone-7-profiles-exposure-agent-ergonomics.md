# Milestone 7: Profiles, Exposure, and Agent Ergonomics

Date: 2026-07-07

Parent roadmap: `plans/2026-07-07-coding-agent-hardening-roadmap.md`

## Objective

Audit and refine the tool profile, exposure, and ergonomics model after runtime hardening and deterministic coding-agent tool additions. The outcome should be a coherent tool surface for model-facing agents, harness-driven preflight, debug workflows, and codegg integration.

The key principle is separation of concerns: model-facing sessions should see a useful but safe subset of deterministic tools, while harnesses should retain access to stricter preflight and route-critical tools used for automatic checks.

## Rationale

`eggsact` already has the right primitives: named profiles, `ToolExposure`, `ToolAudience`, cost classes, stability flags, tags, aliases, composite markers, and route-critical classification. Once milestone 6 tools are added, the registry should be audited as a complete product surface rather than a collection of individually correct tools.

Without this pass, new coding-agent tools may be technically implemented but poorly placed: too many tools in model-facing profiles, harness-only checks exposed directly to models, unclear profile selection guidance, inconsistent costs, or weak generated docs for agent integrators.

## Scope

In scope:

- Audit every tool’s registry metadata.
- Audit all profile memberships.
- Audit model/harness/debug exposure.
- Add profile/exposure invariant tests.
- Improve generated profile docs.
- Add or update integration examples for model-facing and harness-facing registries.
- Clarify route-critical and composite tool placement.
- Ensure new milestone 6 tools are placed coherently.

Out of scope:

- Major registry redesign.
- Removing existing profiles unless they are clearly obsolete.
- New tool implementation beyond minor metadata/doc fixes.
- Changing MCP protocol behavior.

## Files likely to change

- `src/mcp/registry/types.rs`
- `src/mcp/registry/listing.rs`
- `src/mcp/registry/all_tools.rs`
- `src/mcp/specs/*.rs`
- `src/agent/mod.rs`
- `src/bin/generate_docs.rs`
- `tests/mcp/test_tool_coverage.rs`
- `tests/mcp/test_mcp_tools.rs`
- `tests/mcp/test_route_contracts.rs`
- `architecture/coding-agent-integration.md`
- `architecture/tools.md`
- `generated/tool-cards.md`
- `README.md`

## Registry audit checklist

For every `ToolSpec`, verify:

- `name` is stable, lowercase snake_case, and unambiguous.
- `handler` points to the correct function.
- `input_schema` matches handler behavior.
- `output_schema` exists for tools where downstream callers need structured guarantees.
- `category` is correct.
- `tier` is appropriate for complexity and intended placement.
- `profiles` include all intended use cases and no irrelevant profiles.
- `tags` describe routing-relevant capabilities.
- `exposure` is correct: model-safe, harness-only, or hidden.
- `harness_use` is set where the tool is primarily intended for automatic harness decisions.
- `aliases` do not conflict and do not encourage stale names.
- `cost` matches actual runtime behavior and budget needs.
- `stability` reflects release readiness.
- `composite` is true when the tool orchestrates other tools.

## Profile audit targets

### `full`

Should contain every non-hidden tool. Verify hidden/internal tools remain excluded where intended.

### `default`

Should be safe for ordinary MCP clients but not necessarily minimal. It should not expose harness-only checks unless those checks are intentionally model-callable.

### `codegg_core_min`

Should be small and highly reliable. Include only tools that are cheap, deterministic, and broadly useful for coding sessions.

Recommended contents:

- Basic text equivalence/comparison tools.
- JSON lightweight validation/canonicalization.
- Path normalization/analysis.
- Regex validation/safety where cheap.
- Possibly `repo_language_detect` once implemented.

Avoid:

- Heavy composite tools.
- Harness-only route-critical tools.
- Broad repo inspection.

### `codegg_core`

Should be the normal model-facing coding profile. Include text, JSON, path, regex, Markdown, identifier, Unicode review, and lightweight code inspection tools.

Potential additions after milestone 6:

- `repo_language_detect`
- `test_command_suggest`
- `import_export_inspect`
- `code_block_map`

### `codegg_preflight`

Should be harness-oriented and include automatic checks for edits, commands, configs, patches, dependency edits, and repo safety.

Potential additions after milestone 6:

- `patch_contract_check`
- `lockfile_inspect`

### `codegg_patch`

Should focus on patch, edit, symbol-diff, and patch-risk classification.

Potential additions after milestone 6:

- `patch_contract_check`
- `symbol_name_diff`
- `lockfile_inspect`

### `codegg_config`

Should focus on JSON/TOML/dotenv/INI/config preflight and structured config shape. Avoid unrelated repo-audit tools.

### `codegg_unicode_security`

Should focus on Unicode, confusables, invisible characters, bidi, identifier inspection, and text-security checks.

### `codegg_shell`

Should focus on shell parsing, argument comparison, command preflight, and test-command suggestion. Ensure command execution remains out of scope.

Potential additions after milestone 6:

- `test_command_suggest`

### `codegg_repo_audit`

Should include repository manifest inspection, repo-tree summarization, dependency inspection, language detection, lockfile inspection, patch contract review, and source-structure heuristics.

Potential additions after milestone 6:

- `repo_language_detect`
- `patch_contract_check`
- `test_command_suggest`
- `import_export_inspect`
- `code_block_map`
- `symbol_name_diff`
- `lockfile_inspect`

### `human_math`

Should remain math/calculator focused. Do not add coding-agent tools.

## Exposure rules

Use these defaults unless there is a clear reason to deviate:

- Model-safe: cheap deterministic utilities, lightweight inspectors, non-mutating review helpers.
- Harness-only: tools meant to gate execution, edit application, shell execution, patch application, dependency changes, or route-critical decisions.
- Hidden: internal helpers, debug-only tools, deprecated unsafe surfaces, or tools not ready for public exposure.

Route-critical tools should generally be callable by harnesses. Some may also be model-visible if they are safe review helpers, but harness-only behavior should be explicit and tested.

## Invariant tests

Add or extend tests to assert:

- Every `ToolSpec` belongs to at least one meaningful profile unless intentionally hidden.
- No hidden tool appears in model or harness listings.
- No harness-only tool appears in `ToolAudience::Model` listings.
- Route-critical tools have expected exposure and profile placement.
- Every model-facing profile has at least one test that lists tools and calls a representative cheap tool.
- Every profile listed in docs exists in registry constants.
- Every `CODEGG_PROFILES` entry in generated docs maps to a registry profile.
- No alias conflicts with canonical tool names.
- New milestone 6 tools appear in intended profiles.

## Generated docs improvements

Update generated docs to make profile selection easier for agents and integrators.

Recommended additions:

- Profile purpose text.
- Intended audience per profile.
- Typical use cases.
- Tool count per profile.
- Route-critical tools section.
- Harness-only tools section.
- Warning that `full` is not the recommended model-facing profile.

If the generator becomes too complex, add a manually maintained profile guidance section outside generated markers and keep generated sections limited to tool lists.

## Integration examples

Add compile-tested or clearly marked examples for:

### Model-facing registry

```rust
use eggsact::agent::{Profile, ToolAudience, ToolRegistry};

let registry = ToolRegistry::with_profile_and_audience(
    Profile::from_str_opt("codegg_core").unwrap(),
    ToolAudience::Model,
);
let tools = registry.available_tools_model_safe();
```

### Harness-facing preflight registry

```rust
use eggsact::agent::{Profile, ToolAudience, ToolRegistry};

let registry = ToolRegistry::with_profile_and_audience(
    Profile::from_str_opt("codegg_preflight").unwrap(),
    ToolAudience::Harness,
);
let response = registry.call_json("command_preflight", serde_json::json!({
    "command": "cargo test --all-features",
})).unwrap();
```

### MCP environment configuration

```bash
EGGCALC_MCP_PROFILE=codegg_core EGGCALC_MCP_AUDIENCE=Model eggsact --mcp
EGGCALC_MCP_PROFILE=codegg_preflight EGGCALC_MCP_AUDIENCE=Harness eggsact --mcp
```

## Review process

Use a table for manual review. For every tool, capture:

- Tool name.
- Category.
- Current profiles.
- Proposed profiles.
- Current exposure.
- Proposed exposure.
- Cost.
- Stability.
- Route-critical/composite status.
- Notes.

Keep the table in a temporary issue/PR comment or a short committed review note if it is useful for future maintainers.

## Testing requirements

Run targeted tests:

```bash
cargo test --all-features profile -- --nocapture
cargo test --all-features audience -- --nocapture
cargo test --all-features tool_coverage -- --nocapture
```

Run full verification:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --lib
cargo test --all-features --bins
cargo test --all-features --tests -- --skip parity
cargo test --all-features --doc
cargo run --bin generate-docs -- --check
cargo package --verbose
```

## Acceptance criteria

- Every tool has reviewed registry metadata.
- Model-facing profiles exclude harness-only tools.
- Harness profiles include required preflight tools.
- New milestone 6 tools are placed in coherent profiles.
- Generated or manual docs clearly explain profile choice by workflow.
- Integration examples compile or are explicitly marked illustrative.
- Profile/exposure invariant tests pass.
- Generated docs are current.

## Handoff notes

Run this milestone after milestone 6 has at least the first three new tools implemented. A partial audit before tool additions is acceptable, but final profile and exposure closure should wait until the new tool surface exists.
