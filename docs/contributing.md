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
cargo test               # all tests (unit, integration, parity)
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
    main.rs              # CLI entry point
    lib.rs               # Library root, re-exports public API
    calc/
      mod.rs             # Calculator module root
      evaluator.rs       # AST-based expression evaluation
      normalize.rs       # Natural language normalization
      units.rs           # Unit definitions and conversion
    mcp/
      mod.rs             # MCP module root
      server.rs          # JSON-RPC 2.0 stdio server, tool dispatch
      tools.rs           # Tool implementation functions
      schemas.rs         # Tool definitions with input/output schemas
    text/
      mod.rs             # Text module root, re-exports
      primitives.rs      # UTF-8, codepoint, grapheme utilities
      confusables.rs     # Homoglyph and confusable detection
      diff.rs            # String diffing
      validate.rs        # JSON, regex, bracket validation
      ...                # 20+ additional text processing modules
  tests/
    lib.rs               # Test entry point
    parity/              # Python/Rust comparison tests
    mcp/                 # MCP protocol tests
    calc/                # Calculator tests
    text/                # Text processing tests
```

## Adding a New MCP Tool

1. **Implement the function** in `src/mcp/tools.rs`. Follow the existing pattern:
   take `&Value`, validate arguments at the boundary, call a library function when
   one exists, and return `ToolResponse`.

2. **Register the dispatch entry** in `TOOL_HANDLERS` in `src/mcp/server.rs`. This
   static array routes tool names to implementation functions.

3. **Add schema and metadata entries** in `src/mcp/server.rs` and `src/mcp/schemas.rs`.
   The tool list, schema builder, and `TOOL_METADATA` entry must stay in sync. Run
   the `tool_registration_tables_are_in_sync` test after editing these tables.

4. **Prefer reusable library code** under `src/text/` or `src/calc/` for business
   logic. Keep `src/mcp/tools.rs` wrappers thin so the same behavior is testable
   without going through JSON-RPC.

5. **Add tests** at the right layer:
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
   a thin wrapper in `src/mcp/tools.rs` that parses input, calls your function, and
   returns the result as a `ToolResponse`.

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
