# Contributing to eggsact

## Prerequisites

- **Rust toolchain** (stable) -- `rustup` is recommended for installation.
- **Python 3.x** -- required for parity tests only. Install `eggcalc` dependencies from
  the repo root.

Verify your setup:

```sh
rustc --version
python --version
cargo --version
```

## Building

```sh
cargo build              # debug build
cargo build --release    # optimized release build
```

The debug binary lands in `target/debug/eggsact`. The parity test suite expects this
path, so run `cargo build` before running parity tests.

## Testing

```sh
cargo fmt --check        # formatting gate
cargo clippy --all-targets --all-features
cargo test               # all tests (unit, integration, parity)
cargo package            # crates.io package verification
cargo test --lib         # unit tests within src/ only
cargo test --test lib parity   # parity tests against Python
cargo test --doc         # doc tests
```

Unit tests live inside `src/` files as `#[cfg(test)]` modules. Integration and parity
tests live in `tests/`. The parity test entry point is `tests/lib.rs`, which declares
the `parity` module.

## Project Structure

```
eggsact/
  src/
    main.rs              # CLI entry point, argument parsing
    lib.rs               # Library root, re-exports run()/evaluate()
    calc/                # Calculator core (3 modules)
      mod.rs
      evaluator.rs       # AST-based expression evaluation
      normalize.rs       # Natural language normalization
      units.rs           # Unit definitions and conversion
    mcp/                 # MCP server protocol, runtime, registry, validation
      server.rs          # Protocol orchestration, stdio loop, dispatch
      compat.rs          # CompatibilityMode enum
      registry/          # Tool registration (ToolSpec declarations, single source of truth)
        mod.rs           # Re-exports, tests
        types.rs         # ToolDefinition, ToolSpec, enums
        all_tools.rs     # ALL_TOOLS aggregation from specs/
        listing.rs       # Filtering, audience, schema compaction
      specs/             # ToolSpec declarations per tool category
        mod.rs           # Re-exports all category slices
        math.rs          # MATH_TOOLS
        text.rs          # TEXT_TOOLS
        ...              # One file per category
      protocol.rs        # JSON-RPC types
      response.rs        # ToolResponse, error sanitization, finding helpers
      runtime.rs         # Rate limiter, constants, profile management
      schema_validation.rs
      machine_codes.rs   # Machine-readable response codes
      budget.rs          # Per-tool budgets, composite sub-budgets
      schemas/           # JSON-schema builders per tool category
        mod.rs
        ...
    tools/               # MCP tool implementations (by category)
      helpers.rs         # Shared constants, utilities
      math.rs            # Math & unit tools
      text.rs            # Text processing tools (18)
      json.rs            # JSON tools (6)
      regex.rs           # Regex tools (3)
      validation.rs      # Validation tools (4)
      path.rs            # Path tools (5)
      shell.rs           # Shell tools (4)
      list.rs            # List tools (3)
      markdown.rs        # Markdown tools (2)
      patch.rs           # Patch tools (3)
      config.rs          # Config tools (3)
      identifier.rs      # Identifier tools (3)
      unicode.rs         # Unicode tools (2)
      version.rs         # Version tools (2)
      cargo.rs           # Cargo tool (1)
    agent/               # In-process agent API (ToolRegistry, Profile, call_json)
    preflight/           # Typed preflight wrappers
    text/                # Text processing library (24 modules)
  tests/
    lib.rs               # Test entry point
    parity/              # Python/Rust comparison tests
    mcp/                 # MCP protocol tests
    calc/                # Calculator tests
    text/                # Text processing tests
```

## Adding a New MCP Tool

1. **Implement the function** in `src/tools/<category>.rs`. Follow the existing pattern:
   take `&Value`, validate arguments at the boundary, call a library function when
   one exists, and return `ToolResponse`.

2. **Add a `ToolSpec` entry** in `src/mcp/specs/<category>.rs`. This is the single
   source of truth for tool registration — it defines the handler, category, tier,
   tags, profiles, input schema, and output schema all in one place. Each category
   exports a `pub const <CATEGORY>_TOOLS: &[ToolSpec]` slice, which `all_tools.rs`
   aggregates into the combined `ALL_TOOLS`.

3. **Run the invariant test** to verify sync:
   ```bash
   cargo test tool_registration_tables_are_in_sync -- --nocapture
   ```

4. **Regenerate docs** from the registry:
   ```bash
   cargo run --bin generate-docs
   ```
   This updates README tool tables, architecture profile references, and
   `generated/tool-cards.md`.

5. **Prefer reusable library code** under `src/text/` or `src/calc/` for business
   logic. Keep `src/tools/*.rs` wrappers thin so the same behavior is testable
   without going through JSON-RPC.

6. **Add tests** at the right layer:
   - Library behavior: `tests/text/` or `tests/calc/`.
   - MCP request/response behavior: `tests/mcp/`.
   - Python reference compatibility: `tests/parity/` using `compare_tool_parity()`.

## Adding a New Text Processing Function

1. **Add the implementation** in the appropriate module under `src/text/`. Match the
   existing code style for that module.

2. **Re-export from `src/text/mod.rs`** if the function is part of the public API.
   Add a `pub use` line and update the `pub mod` list if you created a new module.

3. **Add unit tests** in the same file as `#[cfg(test)]` module, or in a separate file
   under `tests/text/` for integration-level coverage.

4. **Add an MCP tool wrapper** if the function should be exposed to MCP clients. Create
   a thin wrapper in `src/tools/<category>.rs` that parses input, calls your function, and
   returns the result as a `ToolResponse`. Add a `ToolSpec` entry in `src/mcp/specs/<category>.rs`
   and run `cargo run --bin generate-docs` to regenerate docs.

## Code Style

- Use type annotations on all public functions and structs.
- Use `serde` for JSON serialization. Derive `Serialize` and `Deserialize` on data
  structures.
- Follow existing naming conventions: snake_case for functions and variables,
  PascalCase for types.
- Keep error types consistent with the existing set: `input_too_large`,
  `invalid_arguments`, `evaluation_error`, etc.
- Prefer `ahash` for hash maps (already a dependency).
- Keep the crate standard-library-plus-minimal-deps. Check `Cargo.toml` before
  adding a new dependency.

## Parity with Python

When changing behavior, always verify parity tests still pass:

```sh
cargo test --test lib parity
```

If your change introduces a valid behavioral difference from Python, update the parity
test to accept the new behavior and document the divergence in `docs/parity.md`.

## Release Checklist

Run the release script from the repository root before tagging or publishing:

```sh
./release.sh
```

The script regenerates confusable-character data, verifies formatting, runs clippy,
runs the full test suite, builds the optimized binary, and verifies the crates.io
package with `cargo package`.

Before publishing, make sure the worktree is clean after `./release.sh`, the version
in `Cargo.toml` matches the release tag, and `CHANGELOG.md` has an entry for the
release. Publish with:

```sh
cargo publish
```

GitHub Actions mirrors the release gates on pull requests and pushes to `main`: fmt,
clippy, build, tests, and `cargo package`.
