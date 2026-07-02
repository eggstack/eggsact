# Compatibility Mode

`CompatibilityMode` (defined in `src/mcp/compat.rs`) controls whether tool call validation and error messages use Python-parity behavior or strict JSON Schema behavior.

## Modes

| Mode | Type names | Use case |
|------|-----------|----------|
| `EggcalcPython` | `NoneType`, `bool`, `int`, `float`, `str`, `list`, `dict` | MCP server boundary — preserves Python `eggcalc` compatibility |
| `StrictNative` | `null`, `boolean`, `integer`, `number`, `string`, `array`, `object` | In-process agent API — standard JSON Schema conventions |

`StrictNative` is the default.

## Where Each Mode Is Used

| Consumer | Mode | Reason |
|----------|------|--------|
| MCP `tools/call` handler (`server.rs`) | `EggcalcPython` | Preserves Python-parity error messages for existing MCP clients |
| `ToolRegistry::new()` | `StrictNative` | Rust-native consumers expect standard JSON Schema names |
| `ToolRegistry::with_profile()` | `StrictNative` | Same |
| `ToolRegistry::with_profile_and_audience()` | `StrictNative` | Same |
| `ToolRegistry::with_compat_mode()` | explicit | Override the default per-registry |
| Preflight wrappers (`ConfigPreflight`, `CommandPreflight`, `EditPreflight`) | `StrictNative` (via `ToolRegistry::default()`) | Rust-native consumers |

## Behavioral Differences

### Type Names in Error Messages

When validation rejects a value, the error message includes expected and actual type names:

```
# EggcalcPython
"Expected str but got int at 'text'"

# StrictNative
"Expected string but got integer at 'text'"
```

### Affected Validation

The `compat` parameter threads through:
- `json_type_name()` — type name formatting
- `validate_property()` / `validate_property_inner()` — per-property validation
- `validate_arguments()` — full argument validation (via validate_property)

The mode does **not** affect:
- `python_json_dumps()` in `response.rs` — always uses Python-style formatting for MCP responses
- Calculator MCP mode (`set_mcp_mode`) — independent concern
- Schema compaction — controlled by `EGGCALC_MCP_SCHEMA_DETAIL` env var
- Error sanitization — always active
- Tool dispatch logic — profile/audience checks are mode-independent

## Usage

```rust
use eggsact::agent::{ToolRegistry, CompatibilityMode, Profile, ToolAudience};

// StrictNative (default) — standard JSON Schema error messages
let strict = ToolRegistry::new();

// EggcalcPython — Python-parity error messages for MCP compatibility
let compat = ToolRegistry::new()
    .with_compat_mode(CompatibilityMode::EggcalcPython);

// Explicit override for a specific profile/audience
let custom = ToolRegistry::with_profile_and_audience(Profile::Full, ToolAudience::Harness)
    .with_compat_mode(CompatibilityMode::StrictNative);
```

## Testing

Unit tests in `src/mcp/schema_validation.rs` verify type name output in both modes. Integration tests in `tests/mcp/test_hardening_and_gaps.rs` exercise the full `ToolRegistry::call_json()` path with each mode.

Run compatibility-specific tests:
```bash
cargo test --lib schema_validation::tests    # unit tests for type names
cargo test --test lib -- test_strict_native  # integration tests for StrictNative
cargo test --test lib -- test_eggcalc_python  # integration tests for EggcalcPython
```
