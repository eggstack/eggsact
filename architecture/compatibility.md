# Compatibility Mode

`CompatibilityMode` (defined in `src/mcp/compat.rs:21-36`) controls whether tool call validation and error messages use Python-parity behavior or strict JSON Schema behavior. It exists because the eggsact codebase serves two distinct audiences:

- **MCP server boundary** — must preserve Python `eggcalc` error message wording for backward compatibility with existing MCP clients and parity tests.
- **In-process Rust API** — should use standard JSON Schema conventions that Rust consumers expect.

The mode is a simple enum that threads through the validation pipeline, affecting only type name formatting in error messages. All other behavior (bool rejection, dispatch, serialization) is identical across modes.

## Mode Definitions

| Mode | Type Names | Default? | Use Case |
|------|-----------|----------|----------|
| `EggcalcPython` | `NoneType`, `bool`, `int`, `float`, `str`, `list`, `dict` | No | MCP server boundary — preserves Python `eggcalc` compatibility |
| `StrictNative` | `null`, `boolean`, `integer`, `number`, `string`, `array`, `object` | Yes | In-process agent API — standard JSON Schema conventions |

### Full Type Name Mapping

| JSON Value | `EggcalcPython` | `StrictNative` |
|------------|-----------------|----------------|
| `null` | `NoneType` | `null` |
| `true` / `false` | `bool` | `boolean` |
| `42` (integer) | `int` | `integer` |
| `2.5` (float) | `float` | `number` |
| `"hello"` | `str` | `string` |
| `[1, 2]` | `list` | `array` |
| `{"key": "val"}` | `dict` | `object` |

The mapping is implemented in `json_type_name()` (`src/mcp/schema_validation.rs:16-47`), which is the single source of truth for type name formatting.

## Where Each Mode Is Used

| Consumer | Mode | File | Reason |
|----------|------|------|--------|
| MCP `tools/call` handler | `EggcalcPython` | `src/mcp/server.rs:553` | Preserves Python-parity error messages for existing MCP clients |
| `ToolRegistry::new()` | `StrictNative` | `src/agent/mod.rs:330-336` | Rust-native consumers expect standard JSON Schema names |
| `ToolRegistry::with_profile()` | `StrictNative` | `src/agent/mod.rs:341-347` | Same |
| `ToolRegistry::with_profile_and_audience()` | `StrictNative` | `src/agent/mod.rs:352-358` | Same |
| `ToolRegistry::with_compat_mode()` | explicit | `src/agent/mod.rs:371-374` | Override the default per-registry |
| `ExecutionContext::mcp_default()` | `EggcalcPython` | `src/agent/mod.rs:1029-1040` | MCP dispatch contexts |
| `ExecutionContext::agent_default()` | `StrictNative` | `src/agent/mod.rs:1043-1054` | In-process agent contexts |
| `ExecutionContext::library_default()` | `StrictNative` | `src/agent/mod.rs:1015-1026` | Library API contexts |
| `ExecutionContext::cli_default()` | `StrictNative` (default) | `src/agent/mod.rs:1001-1012` | CLI contexts |
| Preflight wrappers | `StrictNative` | `src/preflight/mod.rs` — `EditPreflight` uses the shared `DEFAULT_REGISTRY` (`LazyLock<ToolRegistry>` of `ToolRegistry::default()`); `CommandPreflight` (:1156), `ConfigPreflight` (:1309) and `TextSecurityInspect` (:1559) each construct `ToolRegistry::default()`; `PatchApplyCheck` (:1416) constructs `with_profile_and_audience(Profile::Full, ToolAudience::Harness)` | Rust-native consumers with fail-closed contract enforcement |

## Behavioral Differences

### Type Names in Error Messages

When validation rejects a value, the error message includes expected and actual type names. The mode controls which vocabulary appears:

```
EggcalcPython:  Argument 'text' must be str, got int
StrictNative:   Argument 'text' must be string, got integer
```

Another example with nested objects:

```
EggcalcPython:  Argument 'config.inner' must be str, got NoneType
StrictNative:   Argument 'config.inner' must be string, got null
```

Bool rejection for numeric fields also uses mode-appropriate names:

```
EggcalcPython:  Argument 'count' must be int, got bool
StrictNative:   Argument 'count' must be integer, got boolean
```

### Bool Handling

JSON booleans are **always rejected** for numeric schema fields (`integer`, `number`) in both modes. This is intentional — MCP model-generated booleans for number fields are commonly mistakes. The rejection logic in `validate_property_inner()` (`src/mcp/schema_validation.rs:105-121`) fires after the initial type check, specifically handling the case where `value_matches_type()` would pass a boolean for a numeric schema (since `bool` matches `"number"` in the basic type check).

### What Validation the Mode Affects

The `compat` parameter threads through these functions:

| Function | File | Effect |
|----------|------|--------|
| `json_type_name(value, compat)` | `schema_validation.rs:16` | Returns the type name string for error messages |
| `validate_property(value, schema, path, compat)` | `schema_validation.rs:49` | Per-property validation, passes compat to inner |
| `validate_property_inner(value, schema, path, max_depth, compat)` | `schema_validation.rs:58` | Recursive validation with compat-aware error messages |
| `validate_arguments(name, arguments, compat)` | `schema_validation.rs:407` | Top-level argument validation, delegates to validate_property |

The mode propagates recursively through nested object/array validation — every level of `validate_property_inner` receives and forwards the compat parameter.

### What the Mode Does NOT Affect

| Feature | Why Independent |
|---------|----------------|
| `python_json_dumps()` | Always uses Python-style formatting for MCP responses regardless of compat mode — this is wire format, not validation |
| Calculator MCP mode (`set_mcp_mode`) | Controls evaluator behavior (random/side-effect functions), not validation. Deprecated for new code — use `EvalContext::mcp_mode()` through the thread-local bridge instead. |
| Schema compaction | Controlled by `EGGCALC_MCP_SCHEMA_DETAIL` env var |
| Error sanitization (`sanitize_error()`) | Always active — redacts paths, addresses, variable assignments |
| Tool dispatch logic | Profile/audience checks are mode-independent |
| Bool rejection for numeric fields | Always rejected in both modes |
| Budget/truncation logic | Mode-independent resource management |
| Machine codes and verdicts | Generated by tool handlers, not validation |

## Propagation Through the System

```
MCP Server (server.rs:553)
  └─ ToolRegistry::with_profile_and_audience(profile, audience)
       └─ .with_compat_mode(CompatibilityMode::EggcalcPython)
            └─ prepare_tool_call(name, args)
                 └─ schema_validation::validate_arguments(name, args, compat_mode)
                      └─ validate_property(value, schema, path, compat)
                           └─ validate_property_inner(..., compat)
                                └─ json_type_name(value, compat)
```

For the in-process API:

```
ToolRegistry::default()  →  compat_mode = StrictNative
  └─ call_json(name, args)
       └─ prepare_tool_call(name, args)
            └─ validate_arguments(name, args, StrictNative)
```

For `ExecutionContext`:

```
ExecutionContext::agent_default(profile, audience)
  └─ compatibility_mode = StrictNative
       └─ call_json_with_execution_context(name, args, ctx)
            └─ validate_arguments(name, args, ctx.compatibility_mode)
```

The MCP server overrides the mode at `server.rs:553`:

```rust
let registry = ToolRegistry::with_profile_and_audience(profile, get_active_audience())
    .with_compat_mode(CompatibilityMode::EggcalcPython);
```

While all `ExecutionContext` factory methods set the mode explicitly:

```rust
// MCP contexts use EggcalcPython
ExecutionContext::mcp_default(profile, audience)  →  EggcalcPython

// All others use StrictNative
ExecutionContext::agent_default(profile, audience) →  StrictNative
ExecutionContext::library_default()                →  StrictNative
ExecutionContext::cli_default()                    →  StrictNative (via Default)
ExecutionContext::test_default()                   →  StrictNative (via Default)
```

## Usage Examples

### ToolRegistry Constructors

```rust
use eggsact::agent::{ToolRegistry, CompatibilityMode, Profile, ToolAudience};

// StrictNative (default) — standard JSON Schema error messages
let strict = ToolRegistry::new();
assert_eq!(strict.compat_mode(), CompatibilityMode::StrictNative);

// EggcalcPython — Python-parity error messages for MCP compatibility
let compat = ToolRegistry::new()
    .with_compat_mode(CompatibilityMode::EggcalcPython);
assert_eq!(compat.compat_mode(), CompatibilityMode::EggcalcPython);

// Explicit override for a specific profile/audience
let custom = ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Harness)
    .with_compat_mode(CompatibilityMode::StrictNative);
```

### ExecutionContext

```rust
use eggsact::agent::{ExecutionContext, CompatibilityMode, Profile, ToolAudience};

// MCP contexts default to EggcalcPython
let mcp_ctx = ExecutionContext::mcp_default(Profile::Full, ToolAudience::Model);
assert_eq!(mcp_ctx.compatibility_mode, CompatibilityMode::EggcalcPython);

// Agent contexts default to StrictNative
let agent_ctx = ExecutionContext::agent_default(Profile::Full, ToolAudience::Model);
assert_eq!(agent_ctx.compatibility_mode, CompatibilityMode::StrictNative);

// Builder with explicit mode
let ctx = ExecutionContext::builder()
    .compatibility_mode(CompatibilityMode::EggcalcPython)
    .profile(Profile::Full)
    .audience(ToolAudience::Model)
    .build();
```

### Demonstrating the Difference

```rust
use eggsact::agent::{ToolRegistry, CompatibilityMode};

// Passing a string where an integer is expected
let args = serde_json::json!({"expression": "2 + 2", "precision": "high"});

// StrictNative error
let strict = ToolRegistry::new()
    .with_compat_mode(CompatibilityMode::StrictNative);
let err = strict.call_json("math_eval", args.clone()).unwrap_err();
// err message contains "string" and "integer"

// EggcalcPython error
let compat = ToolRegistry::new()
    .with_compat_mode(CompatibilityMode::EggcalcPython);
let err = compat.call_json("math_eval", args).unwrap_err();
// err message contains "str" and "int"
```

## Testing

### Unit Tests

The `schema_validation::tests` module (`src/mcp/schema_validation.rs:464-731`) contains comprehensive tests for both modes:

| Test | Mode | What It Verifies |
|------|------|-----------------|
| `json_type_name_eggcalc_python_uses_python_names` | EggcalcPython | All 7 JSON types map to Python names |
| `json_type_name_strict_native_uses_json_schema_names` | StrictNative | All 7 JSON types map to JSON Schema names |
| `strict_native_error_uses_json_schema_type_names` | StrictNative | Error message contains "string" and "integer" |
| `eggcalc_python_error_uses_python_type_names` | EggcalcPython | Error message contains "str" and "int" |
| `strict_native_rejects_bool_for_integer` | StrictNative | Bool→integer error uses "boolean"/"integer" |
| `eggcalc_python_rejects_bool_for_integer` | EggcalcPython | Bool→integer error uses "bool"/"int" |
| `strict_native_rejects_null_for_string` | StrictNative | Null→string error uses "null"/"string" |
| `eggcalc_python_rejects_null_for_string` | EggcalcPython | Null→string error uses "NoneType"/"str" |
| `strict_native_error_message_for_nested_object` | StrictNative | Nested path `config.inner` with JSON Schema names |
| `strict_native_rejects_array_for_object` | StrictNative | Array→object uses "array"/"object" |
| `strict_native_rejects_string_for_number` | StrictNative | String→number uses "string"/"number" |

### Integration Tests

The MCP server handler tests (the `#[cfg(test)] mod tests` in `src/mcp/server.rs`, starting line 1052) exercise the full `ToolRegistry::call_json()` path with `EggcalcPython` mode (matching the MCP server default):

| Test | What It Verifies |
|------|-----------------|
| `test_bug018_pattern_matches_anywhere_in_string` | Pattern validation works end-to-end |
| `test_bug018_pattern_anchored_accepts` | Anchored pattern matches correctly |
| `test_bug018_pattern_anchored_rejects` | Anchored pattern rejects correctly |
| `test_bug019_multipleof_relative_tolerance` | multipleOf with floating point tolerance |

### Running Tests

```bash
# Unit tests for type names and validation
cargo test --lib schema_validation::tests

# Integration tests for StrictNative
cargo test --test lib -- test_strict_native

# Integration tests for EggcalcPython
cargo test --test lib -- test_eggcalc_python

# All schema validation tests
cargo test --lib schema_validation

# Parity tests (requires Python eggcalc at ../eggcalc)
cargo test --test lib parity
```

## Migration Notes

### Switching from EggcalcPython to StrictNative

If migrating MCP clients to expect standard JSON Schema type names:

1. **Update error message assertions** — any test or client that pattern-matches on `NoneType`, `int`, `float`, `str`, `list`, `dict` must be updated to `null`, `integer`, `number`, `string`, `array`, `object`.

2. **No schema changes needed** — the tool input schemas use JSON Schema type names (`"type": "string"`, etc.) in both modes. Only the *error message* vocabulary changes.

3. **No bool behavior change** — booleans are rejected for numeric fields in both modes.

4. **Wire format unchanged** — `python_json_dumps()` always produces Python-style serialization regardless of compat mode.

### Switching from StrictNative to EggcalcPython

If adding Python-parity support to an in-process consumer:

```rust
let registry = ToolRegistry::new()
    .with_compat_mode(CompatibilityMode::EggcalcPython);
```

Or via `ExecutionContext`:

```rust
let ctx = ExecutionContext::builder()
    .compatibility_mode(CompatibilityMode::EggcalcPython)
    .build();
```

No other changes are required — the mode only affects validation error messages.

### Key Invariants

- **Both modes reject bools for numeric fields** — this is not configurable via compat mode.
- **The mode is per-registry or per-context** — there is no global static. Each `ToolRegistry` or `ExecutionContext` carries its own mode.
- **The MCP server always uses EggcalcPython** — this is hardcoded at `server.rs:553` and is not configurable via environment variable.
- **Preflight wrappers always use StrictNative** — they construct `ToolRegistry::default()` internally, which uses the default mode.
