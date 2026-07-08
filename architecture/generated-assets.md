# Generated Assets and Parity Workflow

Maintainer reference for generated files, doc generation, confusables data, parity testing, diagnostics, and the verification pipeline.

## Generated Files

| File | Source | Generator Command | Purpose |
|------|--------|-------------------|---------|
| `README.md` tool tables | `ToolSpec` registry in `src/mcp/specs/` | `cargo run --bin generate-docs` | Category-organized tool catalog with tier, exposure, stability, cost, profiles |
| `architecture/mcp-server.md` profile reference | `ToolSpec` registry + `available_profiles()` | `cargo run --bin generate-docs` | Per-profile model/harness tool counts and harness-only listings |
| `generated/tool-cards.md` | `ToolSpec` registry | `cargo run --bin generate-docs` | Per-codegg-profile tool cards with required args, aliases, composite flags |
| `src/text/confusables_generated.rs` | Unicode UTS #39 `confusables.txt` | `python3 scripts/generate_confusables.py` | HashMap of Unicode codepoints to confusable alternatives |

These files are **never hand-edited**. Edit the source of truth and re-run the generator.

## Doc Generation

`src/bin/generate_docs.rs` is a standalone binary that reads the `ToolSpec` registry at compile time and produces three outputs:

### What It Reads

- `all_tools_vec()` — the full `ToolSpec` registry from `src/mcp/registry/all_tools.rs`
- `tools_for_profile_audience(profile, audience)` — filtered tool lists per profile
- `available_profiles()` — all registered profile names
- Each `ToolSpec`'s `input_schema()` closure — for required-arg extraction in tool cards

### What It Produces

**1. Tool tables in `README.md`**

Inserted between HTML comment markers under the `## MCP Tools` heading:

```
<!-- BEGIN GENERATED: eggsact tools -->
{category-organized tool tables}
<!-- END GENERATED: eggsact tools -->
```

Each category gets a `### Category (N)` heading with a markdown table:

| Column | Source |
|--------|--------|
| Tool | `spec.name` |
| Tier | `spec.tier` |
| Exposure | `spec.exposure` (short: default/contextual/expert/harness/hidden) |
| Stability | `spec.stability` (stable/deprecated/exp) |
| Cost | `spec.cost` (cheap/mod/heavy) |
| Profiles | `spec.profiles` sorted and comma-joined |

Hidden tools are excluded from the output.

**2. Profile reference in `architecture/mcp-server.md`**

Inserted between markers under the `### Profile Reference` heading:

```
<!-- BEGIN GENERATED: profile reference -->
{profile comparison table}
<!-- END GENERATED: profile reference -->
```

The table lists each profile with Model tool count, Harness tool count, model tool names, and harness-only tool names.

**3. `generated/tool-cards.md`**

A standalone file (no markers) organized by codegg profile. Each tool gets a card with:

- Description, tier, cost, stability, exposure
- Composite flag (if applicable)
- Required args with types (extracted from `inputSchema`)
- Aliases (if any)

Eight codegg profiles are generated: `codegg_core_min`, `codegg_core`, `codegg_preflight`, `codegg_patch`, `codegg_config`, `codegg_unicode_security`, `codegg_shell`, `codegg_repo_audit`.

### Marker-Based Insertion

The generator uses HTML comment markers for targeted insertion into existing files:

- Finds existing content between `BEGIN`/`END` markers
- **Strips all generated blocks first** (including orphaned BEGIN markers from prior failed runs) to guarantee clean output
- Inserts the new block after the target heading (`## MCP Tools` or `### Profile Reference`)
- Handles edge cases: missing markers (first run), orphaned markers (triplication bug), heading-absent files

### Check Mode

```bash
cargo run --bin generate-docs -- --check
```

Compares current generated output against file contents without writing. Exit code 1 means files are stale. CI runs this as part of the verification pipeline.

### When to Regenerate

- Adding, removing, or renaming a tool in `src/mcp/specs/`
- Changing a tool's description, category, cost tier, exposure, stability, or profile membership
- Changing `CATEGORY_ORDER` or `CODEGG_PROFILES` constants in `src/bin/generate_docs.rs`

## Confusables Data

`src/text/confusables_generated.rs` is a 6500+ line auto-generated Rust file mapping Unicode codepoints to their confusable alternatives per Unicode UTS #39.

### Format

Each line maps a single codepoint to its confusable replacement(s):

```rust
// Auto-generated from confusables.txt (Unicode UTS #39).
// DO NOT EDIT - regenerate with scripts/generate_confusables.py
m.insert("U+0022", "U+0027 U+0027");  // " → ''
m.insert("U+0030", "U+004F");          // 0 → O
m.insert("U+0049", "U+006C");          // I → l
```

The file is included into a `Lazy<HashMap<&'static str, &'static str>>` via `include!()` in the `unicode_policy_check` and `text_security_inspect` tool handlers.

### Generation

```bash
python3 scripts/generate_confusables.py
```

The script:

1. Fetches `confusables.txt` from `https://www.unicode.org/Public/security/latest/confusables.txt`
2. Parses hex code point mappings (source → substitution)
3. Writes two files:
   - `src/text/confusables_generated.rs` — the raw `m.insert()` lines (included at compile time)
   - `data/confusables.rs` — a self-contained module with `Lazy<HashMap>` wrapper (standalone reference)

### Build Impact

- Checked into the repo and compiled as part of the crate
- No network access needed at build time (data is static)
- Listed in `Cargo.toml`'s `include` list for `cargo package`
- Regeneration needed only when a new Unicode version adds confusables mappings

## Parity Tests

The parity suite in `tests/parity/` validates Rust tool output against the Python `eggcalc` reference implementation.

### How They Work

1. **Spawn both MCP servers** as subprocesses:
   - Python: `python3 -m eggcalc.mcp.server` (from `../eggcalc/`)
   - Rust: `eggsact --mcp` (built binary)
2. **Send identical JSON-RPC `tools/call` requests** to both servers via stdin
3. **Parse JSON-RPC responses** from each server's stdout
4. **Compare parsed output values** for strict JSON equality (`r_val == p_val`)

Three comparison modes exist in `tests/parity/mod.rs`:

| Function | Comparison | Use Case |
|----------|------------|----------|
| `compare_tool_parity()` | Strict JSON equality | Most tools |
| `compare_tool_parity_superset()` | Python output ⊆ Rust output | Tools where Rust adds fields |
| `compare_tool_text_parity()` | Raw text equality + parsed equality | Tools returning text content |

### Test Organization

| File | Tier | Test Count (approx) |
|------|------|---------------------|
| `test_tools_core.rs` | Core | 27 |
| `test_tools_tier0.rs` | Tier 0 | 14 |
| `test_tools_tier1.rs` | Tier 1 | 27 |
| `test_tools_tier2.rs` | Tier 2 | 25 |
| `test_tools_tier3.rs` | Tier 3 | 25 |
| `test_semantic_parity.rs` | Semantic | edge cases |
| `test_tools_phase4.rs` | Phase 4 | regex, shell, unicode, path, version |
| `test_tools_phase5.rs` | Phase 5 | text serialization |
| `test_tools_list.rs` | Tool List | catalog order parity |
| `test_error_handling.rs` | Errors | 33 |
| `test_bug_fixes.rs` | Bug Fixes | regression tests |

### Why Skipped in CI

The Python `eggcalc` package is not available in GitHub Actions. Parity tests require:

1. Python 3.x in the test environment
2. `eggcalc` at `../eggcalc` relative to the repo root
3. The Rust binary built at `target/debug/eggsact`

CI excludes parity with `--skip parity`:

```bash
cargo test --all-features --tests -- --skip parity
```

### Running Locally

```bash
# Verify Python eggcalc is available
ls ../eggcalc/mcp/server.py

# Build the Rust binary
cargo build

# Run parity tests only
cargo test --test lib parity

# Run all tests including parity
cargo test --all-features
```

### Known Failures

As of 2026-07-08: **383 passed, 33 failed** (out of 418 parity tests).

The 33 failures are accepted behavioral differences, not regressions. They are tracked in:

- `docs/parity.md` — full decision table with category definitions (C1–C6)
- `tests/fixtures/accepted_parity_failures.txt` — 33 test names for regression detection

| Category | Count | Root Cause |
|----------|-------|------------|
| C1 | 9 | Shell tokenization drift (`shell_split` comment/quote/escape handling) |
| C2 | 4 | Prompt input inspect output shape differences |
| C3 | 3 | Unicode policy check finding structure differences |
| C4 | 7 | Miscellaneous tool output drift (metadata, error envelopes, cosmetic) |
| C5 | 8 | `tools/list` ordering and Rust superset (80 vs 67 tools) |
| C6 | 2 | Raw MCP response comparison — needs Harness audience in test |

These accumulated across phases 06–09. Category A (23 failures) was fixed by adding `EGGCALC_MCP_AUDIENCE` env var support.

## Diagnostics

The `runtime_diagnostics` tool (MCP) and `--diagnostics` CLI flag expose generated-data and runtime state for introspection.

### CLI Usage

```bash
# Text summary
eggsact --diagnostics

# JSON output
eggsact --diagnostics --format json
```

### What It Prints

- Version, tool count, profile summary
- Budget tier distribution (cheap/moderate/heavy)
- Active profile, audience, schema detail
- Runtime limits (rate limit, in-flight, workers, request/output bytes)
- Environment variable names (no values)
- Generated data existence checks (`confusables_generated.rs`, `tool-cards.md`)
- Parity availability (`../eggcalc` exists)

### MCP Output (JSON)

The `runtime_diagnostics` tool returns a JSON object:

```json
{
  "active_profile": "full",
  "active_audience": "Model",
  "tool_count": 80,
  "route_critical_tools": ["edit_preflight", "command_preflight", ...],
  "profile_tool_count": 80,
  "model_visible_tool_count": 74,
  "harness_visible_tool_count": 80,
  "compatibility_mode": "eggcalc_python",
  "budget_tier_summary": { "cheap": 42, "moderate": 28, "heavy": 10 },
  "runtime": {
    "active_profile": "full",
    "active_audience": "Model",
    "schema_detail": "full",
    "limits": {
      "max_requests_per_second": 10,
      "max_in_flight_requests": 16,
      "max_tool_workers": 16,
      "max_request_bytes": 1048576,
      "max_output_bytes": 1048576
    }
  },
  "known_env_vars": ["EGGCALC_NO_CONFIG", "EGGCALC_MCP_PROFILE", ...],
  "generated_doc_command": "cargo run --bin generate-docs",
  "verification_command": "cargo run --bin verify-eggsact",
  "generated_data": {
    "confusables_generated_rs": true,
    "tool_cards_md": true
  },
  "parity_available": true
}
```

Two companion tools provide deeper introspection:

- `profile_inspect` — per-profile tool counts, route-critical presence, harness-only presence, warnings
- `tool_availability_explain` — why a specific tool is or isn't callable (profile membership, exposure, audience)

## Verification Workflow

### When to Regenerate

| Change | Regenerate |
|--------|------------|
| Add/remove/rename tool in `src/mcp/specs/` | `cargo run --bin generate-docs` |
| Change tool metadata (tier, cost, exposure, profiles) | `cargo run --bin generate-docs` |
| New Unicode version with updated confusables | `python3 scripts/generate_confusables.py` |
| Change `CATEGORY_ORDER` or `CODEGG_PROFILES` | `cargo run --bin generate-docs` |

### Verification Steps

```bash
# 1. Regenerate docs
cargo run --bin generate-docs

# 2. Check for unexpected changes
git diff README.md architecture/mcp-server.md generated/tool-cards.md

# 3. Verify generated docs are current
cargo run --bin generate-docs -- --check

# 4. Run full verification pipeline
cargo run --bin verify-eggsact

# 5. Or run individual gates in order
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --lib
cargo test --all-features --bins
cargo test --all-features --tests -- --skip parity
cargo test --doc
cargo run --bin generate-docs -- --check
cargo package --verbose
```

### CI Enforcement

GitHub Actions CI runs on push/PR to `main`:

1. `cargo fmt --all -- --check`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `cargo test --all-features --lib` (unit tests)
4. `cargo test --all-features --bins` (binary tests)
5. `cargo test --all-features --tests -- --skip parity` (integration, parity excluded)
6. `cargo test --doc` (doc tests)
7. `cargo run --bin generate-docs -- --check` (generated docs freshness)
8. `cargo package --verbose` (after all checks pass)

The `--check` gate in step 7 ensures that any `ToolSpec` change is accompanied by regenerated docs. A failing check means the registry changed but the generated output was not refreshed — the PR must re-run the generator before CI will pass.

CI does **not** publish to crates.io. The maintainer publishes manually per `docs/release.md`.
