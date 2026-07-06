# Phase 01: generated docs, metadata, and release-blocking drift

## Purpose

Repair generated documentation and public metadata drift before deeper feature work. The current repository appears to have a malformed or stale README generated block and hand-maintained architecture text that no longer matches the registry and MCP server. This phase makes the registry the visible source of truth again and prevents future release drift.

## Current observations

The README describes eggsact as a natural-language math calculator with MCP support and says it ships with 68 MCP tools. The generated MCP tools block begins with `<!-- BEGIN GENERATED: eggsact tools -->`, but the fetched README content ends without a matching end marker and stops before newer categories such as TOML, patch, cargo, dependency, repo, and diagnostics. The docs generator already has logic to detect orphaned begin markers and strip/rebuild generated blocks, so this is likely exactly the kind of corruption the generator is intended to repair.

The architecture document also has stale prose. It says `tools/list` returns all 68 tool definitions, while the server implements additional methods such as `ping` and `profiles/list`. The architecture tool category section includes some newer categories but is still partly hand-maintained and vulnerable to future drift.

The package metadata in `Cargo.toml` and README still lead with calculator/NLP positioning. That undersells the current role of eggsact as a deterministic coding-agent utility layer.

## Implementation plan

1. Run the generated documentation workflow locally:

   ```bash
   cargo run --bin generate-docs
   cargo run --bin generate-docs -- --check
   ```

   If the first command modifies files, inspect the generated diff carefully. Verify that `README.md`, `architecture/mcp-server.md`, and `generated/tool-cards.md` are the only expected generated outputs unless the generator intentionally writes more files.

2. Repair generated markers.

   Confirm that `README.md` has exactly one `<!-- BEGIN GENERATED: eggsact tools -->` and one corresponding `<!-- END GENERATED: eggsact tools -->`. Confirm that `architecture/mcp-server.md` has exactly one `<!-- BEGIN GENERATED: profile reference -->` and one corresponding `<!-- END GENERATED: profile reference -->`. Add a small test to `src/bin/generate_docs.rs` or a docs-focused test module that checks marker balance for generated files.

3. Replace fixed tool counts in hand-maintained prose.

   Avoid hardcoded `68 tools` style text outside generated blocks. Prefer phrases such as `registered tools`, `the ToolSpec registry`, or generated counts. Where a fixed count is useful in generated docs, it should come from `all_tools_vec()` and non-hidden filtering.

4. Update `architecture/mcp-server.md` supported methods.

   Ensure the supported methods table includes at least:

   - `initialize`
   - `notifications/initialized`
   - `notifications/cancelled`
   - `tools/list`
   - `tools/call`
   - `profiles/list`
   - `ping`

   If any are intentionally undocumented internal methods, state that explicitly.

5. Update architecture category/module tables.

   Ensure the implementation table lists all current tool modules under `src/tools/` and all spec modules under `src/mcp/specs/`, including `dependency`, `repo`, and `diagnostics`. If this table remains hand-maintained, add a note that generated tool cards are canonical for current tool inventory.

6. Update public positioning.

   Change the README first paragraph and `Cargo.toml` description from calculator-first framing to coding-agent utility-first framing. Suggested direction:

   ```text
   Deterministic MCP and in-process utility tools for coding agents, including math, text, JSON, regex, path, shell, config, Unicode, patch, dependency, and repository preflight helpers.
   ```

   Keep natural-language math in the feature list, not the primary identity.

7. Document diagnostics discoverability.

   The CLI supports `--diagnostics` and `--diagnostics --format json`; add this to README quick start and architecture docs. Mention that diagnostics expose tool counts, profiles, budget tiers, compatibility mode, known env var names, generated-data presence, and parity reference status.

8. Verify generated tool cards.

   Confirm `generated/tool-cards.md` includes all model-visible codegg profile tools and excludes hidden tools. Confirm that the profile sections match `CODEGG_PROFILES` in `generate_docs.rs`.

## Tests to add or update

Add a generated-marker integrity test that reads `README.md` and `architecture/mcp-server.md` and checks balanced begin/end markers. This can live in `src/bin/generate_docs.rs` tests if the test environment runs from repository root, or in an integration test guarded with clear path assumptions.

Add a generated README coverage test if not already sufficient: every non-hidden tool from `all_tools_vec()` should appear in the generated README content. Existing generator tests already cover generated content; the missing piece is that committed files match generated content and marker balance is sane.

## Acceptance criteria

- `cargo run --bin generate-docs -- --check` passes.
- `cargo fmt --all -- --check` passes.
- `cargo test --all-features` passes.
- README has balanced generated markers and lists all non-hidden current categories.
- Architecture docs include implemented MCP methods and current tool categories.
- Package and README descriptions reflect coding-agent utility scope rather than calculator-only scope.
- No hand-maintained fixed tool count remains where it can drift from the registry.

## Risks and constraints

Do not manually edit generated blocks except through the generator unless diagnosing a generator bug. Do not remove natural-language math positioning entirely; it remains a supported differentiating feature. Avoid large rewrites of architecture docs beyond drift correction in this phase; deeper runtime semantics are phase 04.

## Handoff notes

Start by running the generator and inspecting the diff. If CI currently fails generated-docs, this phase should restore a green baseline before any feature work. If the generator itself produces duplicate or malformed blocks, fix `find_all_generated_spans`, `strip_all_generated_blocks`, or insertion logic before updating docs by hand.
