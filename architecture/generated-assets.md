# Generated Assets and Parity Workflow

Maintainer reference for generated files, doc generation, confusables data, parity testing, diagnostics, and the verification pipeline.

## Generated Files

| File | Source | Generator Command | Purpose |
|------|--------|-------------------|---------|
| `architecture/mcp-server.md` profile reference | `ToolSpec` registry + `available_profiles()` | `cargo run --locked --features dev-tools --bin generate-docs` | Per-profile model/harness tool counts and harness-only listings |
| `generated/tool-cards.md` | `ToolSpec` registry | `cargo run --locked --features dev-tools --bin generate-docs` | Per-codegg-profile tool cards with required args, aliases, composite flags |
| `architecture/overview.md` registry facts | `ToolSpec` registry + profiles + discovery policy | `cargo run --locked --features dev-tools --bin generate-docs` | Underlying/category/profile counts and discovery advertised count |
| `src/text/confusables_generated.rs` | Unicode UTS #39 `confusables.txt` | `python3 scripts/generate_confusables.py` | Sorted static table of Unicode codepoints to confusable alternatives (binary-search key lookup) |
| `src/text/unicode_properties_generated.rs` | Unicode 18.0.0 UCD (`DerivedCoreProperties.txt`, `Scripts.txt`, `ScriptExtensions.txt`, `extracted/DerivedBidiClass.txt`, `BidiMirroring.txt`, `BidiBrackets.txt`, `PropertyValueAliases.txt`) | `python3 scripts/generate_unicode_security_properties.py` | Sorted range tables for Default_Ignorable, Script/Script_Extensions, Bidi_Class, mirroring, brackets, plus short→long script names (binary-search lookup; never hand-edit) |

These files are **never hand-edited**. Edit the source of truth and re-run the generator.

Generated tool-card/profile assets continue to be produced from `ToolSpec`
only. The two MCP-only discovery facades (`tool_search`, `tool_invoke` in
`src/mcp/discovery.rs`) are intentionally excluded — they are presentation
facades, not ordinary utility categories.

## Doc Generation

`src/bin/generate_docs.rs` is a standalone binary that reads the `ToolSpec` registry at compile time and produces three outputs:

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

**3. Registry facts in `architecture/overview.md`**

The generated registry-facts block contains the underlying tool count,
category counts, per-profile Model/Harness counts, and full/Model discovery
advertised count. This keeps drift-prone numbers out of hand-maintained
architecture prose while leaving explanatory text editable.

### Marker-Based Insertion

The generator uses HTML comment markers for targeted insertion into existing files:

- Finds existing content between `BEGIN`/`END` markers
- **Strips all generated blocks first** (including orphaned BEGIN markers from prior failed runs) to guarantee clean output
- Inserts the new block after the target heading (`## MCP Tools` or `### Profile Reference`)
- Handles edge cases: missing markers (first run), orphaned markers (triplication bug), heading-absent files

### Check Mode

```bash
cargo run --locked --features dev-tools --bin generate-docs -- --check
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

1. Fetches `confusables.txt` from the version-specific Unicode 18.0.0 source
   `https://www.unicode.org/Public/18.0.0/security/confusables.txt`
2. Verifies downloaded bytes against a pinned SHA-256 checksum
3. Verifies the file header reports the pinned Unicode Security version
4. Strict-parses hex code point mappings (source → substitution), failing
   closed — with no output written — on malformed rows, invalid Unicode
   scalar values (including surrogates), empty substitutions, missing type
   columns, or duplicate source mappings
5. Writes two files:
   - `src/text/confusables_generated.rs` — sorted static table of `(u32, &str)` tuples (included at compile time)
   - `data/confusables.rs` — standalone reference with same static table

### Validation and freshness checks

```bash
python3 scripts/generate_confusables.py --self-test  # offline strict-parser fixture suite (no network)
python3 scripts/generate_confusables.py --check      # maintainer check: fetch pinned source,
                                                     # regenerate in memory, fail if checked-in
                                                     # outputs differ (writes nothing)
```

`--self-test` is offline and deterministic: it exercises duplicate,
malformed, invalid-scalar, surrogate, empty-substitution, and known-good
miniature fixtures. `--check` requires network access to the pinned
unicode.org URL, so it is a maintainer/release check — ordinary merge CI
must not depend on unicode.org availability and does not run it.

Provenance single source of truth in code: `CONFUSABLES_UNICODE_VERSION`
(`"18.0.0"`), `CONFUSABLES_SOURCE_SHA256`, and `CONFUSABLES_ENTRY_COUNT`
(6712) in `src/text/confusables.rs` (checked against the generated header
and exact table length by unit tests). The semantic layer above the data
distinguishes the UTS #39 internal skeleton (`internal_skeleton`: NFD →
remove Default_Ignorable → mapping → NFD), the public operation
(`confusable_skeleton = bidiSkeleton(LTR, X)` via version-correct UAX #9
tables), per-character source mappings (`lookup`, `has_confusables`,
`find_confusables`), and the resolved-script verdict (`is_mixed_script`);
collision detection must use the public skeleton relation.

### Unicode security property tables (UTS #39 / UAX #24 / UAX #9)

`src/text/unicode_properties_generated.rs` is generated from seven
checksum-pinned Unicode 18.0.0 UCD inputs (see the file header for URLs,
SHA-256 per source, and entry counts: 27 Default_Ignorable ranges, 2321
Script ranges, 210 Script_Extensions overrides, 2356 Bidi_Class ranges, 438
mirroring entries, 130 bracket entries).

```bash
python3 scripts/generate_unicode_security_properties.py --self-test  # offline strict-parser fixtures (no network)
python3 scripts/generate_unicode_security_properties.py --check      # maintainer check: fetch pinned sources,
                                                                     # regenerate in memory, fail if checked-in
                                                                     # output differs (writes nothing)
```

`--self-test` is offline and deterministic (miniature fixtures for every
parser plus range-overlap/duplicate/surrogate guards). `--check` requires
network to the pinned unicode.org URLs and is a maintainer/release check —
ordinary merge CI must not depend on unicode.org availability and does not
run it. Provenance constants (`UNICODE_SECURITY_PROPERTIES_VERSION`,
per-source URLs/SHA-256, entry counts) live in
`src/text/unicode_properties.rs` and are checked against the generated
header and table lengths by unit tests.

### Dependency data-epoch qualification (Milestone 005)

Security-semantic providers were qualified from authoritative crate metadata
before adoption; older epochs are never silently mixed:

| Crate | `UNICODE_VERSION` | Verdict |
|---|---|---|
| `unicode-security` 0.1.2 | (16, 0, 0) | Rejected as authoritative (older than the Unicode 18 security baseline); not a dependency |
| `unicode-script` 0.5.8 | (17, 0, 0) | Rejected as authoritative; Script/Script_Extensions generated from pinned UCD 18.0.0 instead |
| `unicode-bidi-mirroring` 0.4.0 | Unicode 16 (per release notes) | Rejected as authoritative; mirroring generated from pinned UCD 18.0.0 instead |
| `unicode-bidi` 0.3.18 | (16, 0, 0) hardcoded | Algorithm adopted, data rejected: built with `default-features = false` (no `hardcoded-data`) and always driven through the custom Unicode 18 `Unicode18BidiData` source (Bidi_Class + brackets from generated tables) |
| `unicode-ident` 1.0.26 | (18, 0, 0) | Accepted (already current; Rust XID validity) |

### Provider epoch inventory (Unicode 18 qualification)

Only the confusables asset advances per milestone; independent providers
keep their own epochs (verified from authoritative crate/project metadata
2026-09-25):

| Provider | Crate / source | Unicode-data epoch | Role in security results |
|---|---|---|---|
| Confusables table | generated (`confusables.txt` 18.0.0, SHA-256 pinned) | 18.0.0 | security-semantic (skeleton mapping) |
| Normalization (NFD/NFC/NFKC/NFKD) | `unicode-normalization` 0.1.25 | 17.0 (upstream 18 update unreleased) | security-semantic (skeleton NFD); canonical mappings are stability-guaranteed, so epoch skew cannot alter existing skeletons |
| Case folding | `caseless` 0.2.2 (`UNICODE_VERSION = (16, 0, 0)`) | 16.0 (no newer release) | security-semantic (canonicalize, casefold collisions) |
| General category | `unicode-general-category` 1.1.0 (Unicode 16.0 badge) | 16.0 (no newer release) | support (combining-mark / control classification) |
| Character names | `unicode_names2` 3.1.0 | 17.0 | diagnostic-only (display names) |
| Segmentation | `unicode-segmentation` 1.13.3 | 17.0 | diagnostic (grapheme counts) |
| Rust XID | `unicode-ident` 1.0.26 (`UNICODE_VERSION = (18, 0, 0)`) | 18.0 | security-semantic (Rust validity) |
| UAX #9 algorithm | `unicode-bidi` 0.3.18 WITHOUT bundled data (`default-features = false`, custom `Unicode18BidiData`) | 18.0 (generated tables drive the algorithm) | security-semantic (bidiSkeleton L1/L2; L3/L4 in `confusables.rs`) |
| Script / Script_Extensions | generated (`Scripts.txt` + `ScriptExtensions.txt` 18.0.0, SHA-256 pinned) | 18.0.0 | security-semantic (resolved sets with Jpan/Kore/Hanb/Hntl) |
| Bidi_Class / mirroring / brackets | generated (`DerivedBidiClass.txt` + `BidiMirroring.txt` + `BidiBrackets.txt` 18.0.0, SHA-256 pinned) | 18.0.0 | security-semantic (bidiSkeleton) |
| Default_Ignorable | generated (`DerivedCoreProperties.txt` 18.0.0, SHA-256 pinned) | 18.0.0 | security-semantic (internal-skeleton removal) |

The accurate shipped claim is "confusables data: Unicode 18.0.0" plus
"security skeleton/script/bidi properties: Unicode 18.0.0" — never
"all Unicode processing: 18.0.0" (normalization remains 17.0, casefold and
general-category remain 16.0 per the table above; canonical mappings are
stability-guaranteed so the skew cannot alter existing skeletons).

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
cargo build --locked

# Run parity tests only
cargo test --locked --test lib parity

# Run all tests including parity
cargo test --locked --all-features
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
| C5 | 8 | `tools/list` ordering and Rust superset (86 vs 67 tools) |
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
  "active_audience": "model",
  "tool_count": 86,
  "route_critical_tools": ["edit_preflight", "command_preflight", "config_preflight", "patch_apply_check", "text_security_inspect"],
  "profile_tool_count": 86,
  "model_visible_tool_count": 77,
  "harness_visible_tool_count": 86,
  "compatibility_mode": "eggcalc_python",
  "budget_tier_summary": { "cheap": 47, "moderate": 34, "heavy": 5 },
  "runtime": {
    "active_profile": "full",
    "active_audience": "harness",
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
  "known_env_vars": ["EGGCALC_NO_CONFIG", "EGGCALC_MCP_PROFILE", "EGGCALC_MCP_AUDIENCE", "EGGCALC_MCP_SCHEMA_DETAIL", "EGGSACT_MCP_SURFACE"]
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
| Add/remove/rename tool in `src/mcp/specs/` | `cargo run --locked --features dev-tools --bin generate-docs` |
| Change tool metadata (tier, cost, exposure, profiles) | `cargo run --locked --features dev-tools --bin generate-docs` |
| New Unicode version with updated confusables | `python3 scripts/generate_confusables.py` |
| New Unicode version with updated security properties (Default_Ignorable, Script, Bidi) | `python3 scripts/generate_unicode_security_properties.py` |
| Change `CATEGORY_ORDER` or `CODEGG_PROFILES` | `cargo run --locked --features dev-tools --bin generate-docs` |

### Verification Steps

```bash
# 1. Regenerate docs
cargo run --locked --features dev-tools --bin generate-docs

# 2. Check for unexpected changes
git diff README.md architecture/mcp-server.md generated/tool-cards.md

# 3. Verify generated docs are current
cargo run --locked --features dev-tools --bin generate-docs -- --check

# 4. Or run the merge gate in order (see AGENTS.md for the canonical list)
cargo fmt --all -- --check
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
```

`--test-threads=4` is required for the integration suites (Tokio
blocking-pool starvation); `--lib`/doc tests do not need it. `cargo-deny`
and the full local gate (clean tree required) live in
`scripts/release-check.sh`, which never publishes or tags.

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
