# Tightening Phase 05: Generated Docs, Profile References, and Agent Tool Cards

## Objective

Eliminate documentation and tool-surface drift by generating documentation from the registry. eggsact already has a central `ToolSpec` direction. This phase makes the registry the source for README tool tables, architecture profile references, schema snapshots, machine-code references, and compact tool cards for codegg.

## Problem Statement

Manual tool tables drift as tools, profiles, schemas, and exposure metadata change. This is especially costly for agent-facing tools because stale docs cause model confusion and harness integration mistakes.

The current registry already contains most of the metadata needed for generated docs: name, description, category, tier, profiles, tags, exposure, cost, stability, handler, schemas, aliases, harness use, and composite status. The missing piece is a generator and CI enforcement.

## Desired Behavior

Adding or changing a tool in `ToolSpec` should update all derived docs through one generator command.

CI should fail when generated docs are stale.

codegg should be able to consume compact tool cards generated from the same registry used for MCP listing and dispatch.

Human docs and agent docs should differ in format, not source of truth.

## Implementation Steps

### 1. Choose generator shape

Use one of these approaches:

- `cargo run --bin generate-docs`
- `cargo xtask generate-docs`
- `cargo test generated_docs_are_current` with helper generation functions

For this repo size, a small generator binary is sufficient. If eggsact later adopts an xtask pattern, the generator can move there.

### 2. Add generated block markers

Use stable markers in docs:

```markdown
<!-- BEGIN GENERATED: eggsact tools -->
...
<!-- END GENERATED: eggsact tools -->
```

Generate only marked sections at first. This avoids rewriting prose-heavy docs and keeps review diffs small.

### 3. Generate README tool tables

Generate category tables with:

- Tool name
- Short description
- Tier
- Exposure
- Stability
- Primary profiles or profile summary

Keep README concise. Link to architecture docs for full metadata.

### 4. Generate architecture profile reference

Generate profile tables showing:

- Profile name
- Model-audience tools
- Harness-audience tools
- Tool count
- Intended consumer

The intended consumer can come from a small static profile metadata table if it is not already represented in `ToolSpec`.

### 5. Generate compact codegg tool cards

Generate one compact Markdown or JSON file per profile, or one combined file with profile sections. Each card should include:

- Tool name
- When to use
- When not to use
- Required arguments
- Output route fields
- Common machine codes
- Cost/tier hints

If `ToolSpec` does not yet contain enough fields for `when_to_use` and `when_not_to_use`, add optional metadata fields or maintain a small adjacent metadata table keyed by tool name. Long term, this should move into the registry.

### 6. Generate schema snapshots

Generate compact input/output schema snapshots for profile-critical tools. Start with codegg profiles rather than every full-profile tool.

Suggested initial snapshots:

- `codegg_core_min`
- `codegg_preflight`
- `codegg_patch`
- `codegg_config`
- `codegg_shell`
- `codegg_unicode_security`

### 7. Add CI check

Add a CI step that runs the generator in check mode. Check mode should exit nonzero if generated content differs from committed content.

Suggested command shape:

```bash
cargo run --bin generate-docs -- --check
```

or equivalent.

### 8. Update contributor docs

Document the workflow:

1. Add or edit `ToolSpec`.
2. Add/update schemas and handler.
3. Run tests.
4. Run doc generator.
5. Commit source and generated docs together.

## Test Plan

Add unit tests for generator functions using a small synthetic tool list.

Add snapshot tests for generated profile references.

Add CI check mode test if practical.

Add a regression test that every public tool appears in generated docs unless explicitly excluded.

Add a test that every generated tool card references a known registered tool.

## Acceptance Criteria

- README tool tables are generated or have generated sections.
- Architecture profile tables are generated or have generated sections.
- codegg compact tool cards are generated from registry metadata.
- CI fails when generated docs are stale.
- Adding a new tool requires no manual table edits outside generated metadata/prose.

## Non-Goals

Do not generate all prose docs in this phase.

Do not require perfect tool-card prose before the generator exists.

Do not block normal tool work on a large documentation framework. Keep the generator small and deterministic.

## Handoff Notes

Start with generated profile/tool tables because they use metadata that already exists. Then add tool cards. If the registry lacks enough metadata for high-quality cards, add minimal optional fields rather than hardcoding large text blocks inside the generator.
