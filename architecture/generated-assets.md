# Generated Assets and Parity Workflow

Maintainer reference for generated files, doc generation, confusables data, parity testing, diagnostics, and the verification pipeline.

## Generated Files

| File | Source | Generator Command | Purpose |
|------|--------|-------------------|---------|
| `architecture/mcp-server.md` profile reference | `ToolSpec` registry + `available_profiles()` | `cargo run --features dev-tools --bin generate-docs` | Per-profile model/harness tool counts and harness-only listings |
| `generated/tool-cards.md` | `ToolSpec` registry | `cargo run --features dev-tools --bin generate-docs` | Per-codegg-profile tool cards with required args, aliases, composite flags |
| `src/text/confusables_generated.rs` | Unicode UTS #39 `confusables.txt` | `python3 scripts/generate_confusables.py` | Sorted static table of Unicode codepoints to confusable alternatives (binary-search key lookup) |

These files are **never hand-edited**. Edit the source of truth and re-run the generator.

## Doc Generation

`src/bin/generate_docs.rs` is a standalone binary that reads the `ToolSpec` registry at compile time and produces two outputs:

### What It Reads

- `all_tools_vec()` — the full `ToolSpec` registry from `src/mcp/registry/all_tools.rs`
- `tools_for_profile_audience(profile, audience)` — filtered tool lists per profile
- `available_profiles()` — all registered profile names
- Each `ToolSpec`'s `input_schema()` closure — for required-arg extraction in tool cards

### What It Produces

**1. Profile reference in `architecture/mcp-server.md`**

Inserted between markers under the `### Profile Reference` heading:

```
<!-- BEGIN GENERATED: profile reference -->
{profile comparison table}
<!-- END GENERATED: profile reference -->
```

The table lists each profile with Model tool count, Harness tool count, model tool names, and harness-only tool names.

**2. `generated/tool-cards.md`**

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
cargo run --features dev-tools --bin generate-docs -- --check
```

Compares current generated output against file contents without writing. Exit code 1 means files are stale. CI runs this as part of the verification pipeline.

### When to Regenerate

- Adding, removing, or renaming a tool in `src/mcp/specs/`
- Changing a tool's description, category, cost tier, exposure, stability, or profile membership
- Changing `CATEGORY_ORDER` or `CODEGG_PROFILES` constants in `src/bin/generate_docs.rs`

## Confusables Data

`src/text/confusables_generated.rs` is an auto-generated sorted static table mapping Unicode codepoints to their confusable alternatives per Unicode UTS #39.

### Format

The file contains a sorted array literal of `(u32, &str)` tuples:

```rust
// Unicode version: (version)
// Source checksum (SHA-256): (checksum)
(0x0022, "U+0027 U+0027"),  // " → ''
(0x0030, "U+004F"),          // 0 → O
(0x0049, "U+006C"),          // I → l
```

The file is included into a `&[(u32, &str)]` static via `include!()` in `confusables.rs`. Lookups use binary search by code point.

### Generation

```bash
python3 scripts/generate_confusables.py
```

The script:

1. Fetches `confusables.txt` from the version-specific Unicode 17.0.0 source
   `https://www.unicode.org/Public/17.0.0/security/confusables.txt`
2. Verifies downloaded bytes against a pinned SHA-256 checksum
3. Verifies the file header reports the pinned Unicode Security version
4. Parses hex code point mappings (source → substitution)
5. Writes two files:
   - `src/text/confusables_generated.rs` — sorted static table of `(u32, &str)` tuples (included at compile time)
   - `data/confusables.rs` — standalone reference with same static table

### Build Impact

- Checked into the repo and compiled as part of the crate
- No network access needed at build time (data is static)
- Listed in `Cargo.toml`'s `include` list for `cargo package`
- Regeneration needed only when a new Unicode version adds confusables mappings
- Regeneration is a maintainer action; ordinary CI and `scripts/release-check.sh`
  use the checked-in data and do not download Unicode sources

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
cargo test --locked --all-features -- --skip parity --test-threads=4
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

There are **37 accepted parity failures**. These are accepted behavioral differences, not regressions. They are tracked in:

- `docs/parity.md` — full decision table with category definitions (C1–C6)
- `tests/fixtures/accepted_parity_failures.txt` — 37 test names for regression detection

| Category | Count | Root Cause |
|----------|-------|------------|
| C1 | 9 | Shell tokenization drift (`shell_split` comment/quote/escape handling) |
| C2 | 4 | Prompt input inspect output shape differences |
| C3 | 3 | Unicode policy check finding structure differences |
| C4 | 11 | Miscellaneous tool output drift (metadata, error envelopes, cosmetic) |
| C5 | 8 | `tools/list` ordering and Rust superset (80 vs 67 tools) |
| C6 | 2 | Raw MCP response comparison — needs Harness audience in test |

These accumulated across phases 06–09. An earlier Category A (23 failures) was fixed by adding `EGGCALC_MCP_AUDIENCE` env var support.

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
- Runtime limits (in-flight, workers, request/output bytes)
- Live runtime metrics (active requests, timeouts, blocking concurrency)
- Environment variable names (no values)

Since v1.2.2, diagnostics report only stable runtime/package facts — no
source-tree-relative file existence checks and no development command strings,
so output is identical for installed binaries and source checkouts.

### MCP Output (JSON)

The `runtime_diagnostics` tool returns a JSON object:

```json
{
  "active_profile": "full",
  "active_audience": "Model",
  "tool_count": 80,
  "route_critical_tools": ["edit_preflight", "command_preflight", "config_preflight", "patch_apply_check", "text_security_inspect"],
  "profile_tool_count": 80,
  "model_visible_tool_count": 71,
  "harness_visible_tool_count": 80,
  "compatibility_mode": "eggcalc_python",
  "budget_tier_summary": { "cheap": 42, "moderate": 33, "heavy": 5 },
  "runtime": {
    "active_profile": "full",
    "active_audience": "Harness",
    "schema_detail": "full",
    "limits": {
      "max_in_flight_requests": 32,
      "max_tool_workers": 16,
      "max_request_bytes": 1000000,
      "max_output_bytes": 1000000
    },
    "live_metrics": {
      "active_requests": 1,
      "active_blocking_handlers": 1,
      "timed_out_handlers": 0,
      "total_timeouts": 0,
      "peak_blocking_concurrency": 1,
      "sync_pool_stuck_workers": 0
    }
  },
  "known_env_vars": ["EGGCALC_NO_CONFIG", "EGGCALC_MCP_PROFILE", "EGGCALC_MCP_AUDIENCE", "EGGCALC_MCP_SCHEMA_DETAIL"]
}
```

(The tool is harness-only; the envelope wraps this object as `{ok, tool, result, machine_code}`.)

Two companion tools provide deeper introspection:

- `profile_inspect` — per-profile tool counts, route-critical presence, harness-only presence, warnings
- `tool_availability_explain` — why a specific tool is or isn't callable (profile membership, exposure, audience)

## Verification Workflow

### When to Regenerate

| Change | Regenerate |
|--------|------------|
| Add/remove/rename tool in `src/mcp/specs/` | `cargo run --features dev-tools --bin generate-docs` |
| Change tool metadata (tier, cost, exposure, profiles) | `cargo run --features dev-tools --bin generate-docs` |
| New Unicode version with updated confusables | `python3 scripts/generate_confusables.py` |
| Change `CATEGORY_ORDER` or `CODEGG_PROFILES` | `cargo run --features dev-tools --bin generate-docs` |

### Verification Steps

```bash
# 1. Regenerate docs
cargo run --features dev-tools --bin generate-docs

# 2. Check for unexpected changes
git diff README.md architecture/mcp-server.md generated/tool-cards.md

# 3. Verify generated docs are current
cargo run --features dev-tools --bin generate-docs -- --check

# 4. Or run individual gates in order (see AGENTS.md for the canonical list)
cargo fmt --all -- --check
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features --lib
cargo test --locked --all-features --bins
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
cargo deny check advisories bans licenses sources
```

### CI Enforcement

A single **Linux correctness** job runs on push/PR to `main` (`.github/workflows/ci.yml`):

1. `cargo fmt --all -- --check`
2. `cargo run --locked --features dev-tools --bin generate-docs -- --check` (generated docs freshness)
3. `cargo clippy --locked --all-targets --all-features -- -D warnings`
4. `cargo test --locked --all-features -- --skip parity --test-threads=4`
5. `cargo test --locked --doc`

MSRV, cargo-deny, platform checks, latest-compatible deps, and parity run via scheduled/manual workflows (`maintenance.yml`, `latest-compatible.yml`, `parity.yml`). See `docs/verification.md`.

The `--check` gate in step 2 ensures that any `ToolSpec` change is accompanied by regenerated docs. A failing check means the registry changed but the generated output was not refreshed — the PR must re-run the generator before CI will pass.

CI does **not** publish to crates.io. The maintainer publishes manually per `docs/release.md`.
