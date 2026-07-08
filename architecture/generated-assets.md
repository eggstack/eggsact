# Generated Assets and Parity Workflow

Maintainer reference for generated files, doc generation, and Python parity testing.

## Generated Files

| File | Source | Regeneration |
|------|--------|--------------|
| `README.md` (tool tables, profile reference) | `ToolSpec` registry in `src/mcp/specs/` | `cargo run --bin generate-docs` |
| `generated/tool-cards.md` | `ToolSpec` registry in `src/mcp/specs/` | `cargo run --bin generate-docs` |
| `src/text/confusables_generated.rs` | Unicode confusables data from unicode.org | `python3 scripts/generate_confusables.py` |

These files are **never hand-edited**. Edit the source of truth and re-run the generator.

## Doc Generation

`cargo run --bin generate-docs` reads the `ToolSpec` registry and produces:

1. **Tool tables** embedded in `README.md` between `<!-- BEGIN GENERATED: eggsact tools -->` and `<!-- END GENERATED: eggsact tools -->` markers.
2. **Profile reference** embedded in `README.md` between `<!-- BEGIN GENERATED: profile reference -->` and `<!-- END GENERATED: profile reference -->` markers.
3. **`generated/tool-cards.md`** — full tool cards organized by codegg profile.

### Verification

```bash
cargo run --bin generate-docs -- --check
```

This compares current generated output against what is in the files. If the check fails, regenerate with the command without `--check`, inspect the diff, and commit only expected changes.

CI runs `--check` as part of the verification pipeline. A failing check means the `ToolSpec` registry changed but the generated output was not refreshed.

### When to Regenerate

- After adding, removing, or renaming a tool in `src/mcp/specs/`.
- After changing a tool's description, category, cost tier, exposure, stability, or profile membership.
- After changing the `CATEGORY_ORDER` or `CODEGG_PROFILES` constants in `src/bin/generate_docs.rs`.

## Confusables Data

`src/text/confusables_generated.rs` is auto-generated from Unicode UTS #39 confusables data. It maps Unicode codepoints to their confusable alternatives and is used by the `unicode_policy_check` tool to detect mixed-script and confusable-character attacks.

### Regeneration

```bash
python3 scripts/generate_confusables.py
```

The script fetches `confusables.txt` from the Unicode Consortium and writes the Rust source file. Regeneration is needed when a new Unicode version adds confusables mappings.

### Build Impact

The file is checked into the repo and compiled as part of the crate. It is listed in `Cargo.toml`'s `include` list for `cargo package`. No network access is needed at build time.

## Parity Tests

The parity suite validates Rust tool output against the Python `eggcalc` reference implementation. It spawns both MCP servers, sends identical JSON-RPC requests, and compares responses for strict JSON equality.

### Why Skipped in CI

The Python `eggcalc` package is not available in the CI environment. Parity tests require:

1. Python 3.x installed in the test environment.
2. `eggcalc` installed at `../eggcalc` relative to the repo root.
3. The Rust binary built at `target/debug/eggsact`.

These requirements are not met in GitHub Actions, so parity tests are excluded from CI with `--skip parity`.

### Running Locally

```bash
# Ensure Python eggcalc is available
ls ../eggcalc/mcp/server.py

# Build the Rust binary
cargo build

# Run parity tests only
cargo test --test lib parity

# Run all tests including parity
cargo test --all-features
```

### Known Failures

As of 2026-07-08, the parity suite has 33 known failures out of 418 tests. These are documented in `docs/parity.md` under "Known parity gaps" and are categorized as accepted behavioral differences, not regressions. The fixture file `tests/fixtures/accepted_parity_failures.txt` lists all 33 names.

## Diagnostics

```bash
# Text summary
eggsact --diagnostics

# JSON output
eggsact --diagnostics --format json
```

Diagnostics print version, tool count, profile summary, budget tiers, and environment variable names (no values). This helps verify that generated data and runtime configuration are consistent.
