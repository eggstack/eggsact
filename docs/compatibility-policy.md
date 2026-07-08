# Compatibility Policy

This document defines versioning expectations, stability guarantees, and
deprecation rules for the eggsact crate. It is the single source of truth for
what constitutes a breaking change and how transitions are managed.

## Semantic Versioning

eggsact follows [Semantic Versioning 2.0.0](https://semver.org/):

- **MAJOR** — incompatible API changes (breaking Rust API, tool removal, machine
  code removal, schema-removing input change).
- **MINOR** — backwards-compatible new functionality (new tools, new optional
  fields, new machine codes, new profiles).
- **PATCH** — backwards-compatible bug fixes (incorrect calculation, schema
  tightening that rejects previously-invalid-but-undocumented inputs, doc fixes).

The crate is currently pre-1.0 in spirit (rapid iteration through phases 01–12)
but uses a `1.x.y` version scheme. A future `2.0.0` bump will signal that the
compatibility policy below is in full effect.

## Public Rust API Stability

The public Rust API consists of items re-exported from `src/lib.rs`:

| Symbol | Module | Description |
|--------|--------|-------------|
| `evaluate` | `calc` | Direct math expression evaluation |
| `evaluate_with_context` | `calc` | Context-aware math evaluation |
| `run` | `calc` | Natural language math evaluation |
| `run_with_context` | `calc` | Context-aware natural language evaluation |
| `split_at_operators` | `calc` | Tokenizer helper |
| `EvalContext` | `calc` | Evaluation context (PRNG, variables, memory) |
| `agent::ToolRegistry` | `agent` | In-process tool dispatch |
| `agent::Profile` | `agent` | Profile enum for tool filtering |
| `preflight::*` | `preflight` | Typed preflight wrappers |

**Rules:**

- Renaming or removing a public item is a MAJOR change.
- Adding a new public item is a MINOR change.
- Changing a function signature is a MAJOR change.
- Adding a new parameter with a default is a MINOR change (Rust doesn't have
  default params, so this typically means a new function + deprecation of old).
- Changing the return type is a MAJOR change.
- Adding a variant to a public enum with a catch-all `Other(String)` is MINOR;
  without a catch-all is MAJOR.

## MCP Tool Name Stability

Tool names are the primary contract between eggsact and MCP clients (codegg,
other harnesses). Once a tool ships in a release:

- **Renaming** a tool is a MAJOR change.
- **Removing** a tool is a MAJOR change.
- **Adding** a tool is a MINOR change.
- **Marking a tool `deprecated: true`** in its `ToolSpec` is a MINOR change
  (tool still exists and functions).
- **Actually removing a deprecated tool** is a MAJOR change (requires one full
  minor release as deprecated before removal).

Tool aliases (`ToolSpec.aliases`) provide alternate names. Adding an alias is
MINOR. Removing an alias is MAJOR if any known client depends on it.

## MCP Input Schema Compatibility

Input schemas define the JSON arguments a tool accepts.

- **Adding an optional property** (one not in `required`) is non-breaking.
- **Adding a new value to an enum** is non-breaking (clients send strings they
  already know).
- **Removing a property** is breaking.
- **Renaming a property** is breaking.
- **Moving a property from optional to required** is breaking.
- **Changing a property's type** is breaking.
- **Tightening validation** (rejecting inputs that were previously accepted) is
  breaking UNLESS the previously accepted input was a documented bug or was
  already rejected by the Python reference.

Schema changes that tighten validation should be documented in `CHANGELOG.md`
under the "Changed" section with a clear explanation of what inputs are now
rejected.

## MCP Output Schema Compatibility

Output schemas define the JSON structure returned by a tool.

- **Adding an optional field** to the output is non-breaking.
- **Adding a new machine code** is non-breaking (it's a new string value in an
  existing field).
- **Removing a field** is breaking.
- **Renaming a field** is breaking.
- **Changing a field's type** is breaking.
- **Changing a machine code's meaning** for an existing response shape is
  breaking.

The `machine_code` and `verdict` fields on route-critical tools
(`edit_preflight`, `command_preflight`, `config_preflight`,
`patch_apply_check`, `text_security_inspect`) are particularly sensitive.
Their values drive downstream action selection in codegg.

## Machine-Code Stability

Machine codes are defined in `src/mcp/machine_codes.rs` and emitted in tool
response envelopes. They are the machine-readable contract for harnesses.

- **Adding a new code** is MINOR.
- **Removing a code** is MAJOR.
- **Changing a code's string value** is MAJOR.
- **Adding a category-prefixed alias** (e.g., `COMMON_INVALID_ARGUMENTS` for
  `INVALID_ARGUMENTS`) is MINOR — the underlying value is identical.
- **Deprecating a code** (keeping it but adding a preferred alternative) is
  MINOR. The old code must continue to work.

The `ALL` array in `machine_codes.rs` is the canonical registry. A test
(`machine_code_table_is_synchronized`) verifies that the `ALL` set matches
the declared constants.

## Profile and Audience Compatibility

Profiles (`full`, `default`, `codegg_core_min`, `codegg_core`,
`codegg_preflight`, `codegg_shell`, `codegg_patch`, `codegg_config`,
`codegg_unicode_security`, `codegg_repo_audit`, `human_math`) control which
tools are visible to a given integration.

- **Adding a tool to a profile** is MINOR (more tools available).
- **Removing a tool from a profile** is MAJOR if the profile is used by a
  named integration (e.g., `codegg_core_min`).
- **Adding a new profile** is MINOR.
- **Removing a profile** is MAJOR.
- **Changing `ToolExposure`** on a tool is MINOR if it makes the tool MORE
  visible, MAJOR if it makes it LESS visible to a profile that depends on it.

The `ToolAudience` enum (`Model`, `Harness`, `Debug`) controls execution-time
access. Changing which audience can execute a tool is a behavioral change that
should be documented.

## CompatibilityMode Behavior

`CompatibilityMode` (in `src/mcp/compat.rs`) controls validation and error
message formatting:

- **`EggcalcPython`** — MCP server default. Python-style type names in errors
  (`NoneType`, `int`, `float`, `str`, `list`, `dict`). Used at the MCP
  boundary for client compatibility.
- **`StrictNative`** — In-process API default. Standard JSON Schema type names
  (`null`, `integer`, `number`, `string`, `array`, `object`). Used by
  `ToolRegistry::default()` and preflight wrappers.

Both modes reject booleans for numeric schema fields.

- **Changing which inputs are rejected** in either mode is a behavioral change
  that should be documented as PATCH or MINOR depending on severity.
- **Switching the default mode** for an API surface is a MAJOR change.
- **Adding a new mode** is MINOR.
- **Removing a mode** is MAJOR.

## Deprecation Policy

### Tools

1. A tool is **deprecated** by setting `stability: ToolStability::Deprecated` in
   its `ToolSpec`. The tool continues to function normally.
2. A deprecated tool must remain functional for at least **two minor releases**
   (or 6 months, whichever is longer) before removal.
3. Deprecation is announced in `CHANGELOG.md` under "Deprecated" with a
   migration path (recommended replacement tool, if any).
4. The tool's description should note it is deprecated.

### Machine Codes

1. Deprecated codes continue to emit their original string value.
2. A preferred alternative code is introduced in the same release.
3. The deprecated code must remain for at least **two minor releases**.

### Profiles

1. Deprecated profiles continue to function.
2. Removal requires **two minor releases** of deprecation notice.
3. Named integrations (codegg) must be updated before the profile is removed.

### CompatibilityMode

1. Changing validation behavior in a mode is documented in CHANGELOG.
2. Switching defaults requires a MAJOR version bump.

## What Constitutes a Breaking Change

**Breaking (MAJOR):**
- Removing or renaming a public Rust API item
- Removing or renaming an MCP tool
- Removing a machine code
- Removing a profile
- Removing a required input schema property
- Changing an output field's type or name
- Switching a CompatibilityMode default
- Tightening validation that rejects previously-accepted inputs (unless fixing
  a documented bug)

**Non-breaking (MINOR or PATCH):**
- Adding a new tool, profile, or machine code
- Adding optional input/output fields
- Adding tool aliases
- Adding enum variants (with catch-all)
- Deprecating (but not removing) tools, codes, or profiles
- Tightening validation that matches the Python reference (bug fix)
- Documentation-only changes

### Regex Backend Contract Extension

The `validate_regex` and `regex_finditer` tools gained new optional output
fields (`engine_used`, `dialect`, `unsupported_features`) and a new machine
code (`REGEX_UNSUPPORTED_FEATURE`) in a minor release. This is a backward-
compatible behavioral extension:

- Callers that ignore unknown fields in the response envelope continue to
  work unchanged (standard JSON ignore-unknown behavior).
- The `REGEX_UNSUPPORTED_FEATURE` machine code is new (MINOR) and can be
  handled by callers that support it or ignored by callers that do not.
- The `dialect` field always reports `"eggsact-regex"` and is not expected
  to change. The `engine_used` field reflects backend selection between
  `"rust-regex"` and `"fancy-regex"` based on pattern features — callers
  should not assume a specific engine, only that the engine is chosen
  automatically to support the pattern's features.

## Versioning Workflow

1. Before release, run the canonical verification gate (see `release.sh`).
2. Review `CHANGELOG.md` for any breaking changes since last release.
3. If breaking changes exist, bump MAJOR. If only additions, bump MINOR. If
   only fixes, bump PATCH.
4. Update `Cargo.toml` version.
5. Run `cargo run --bin generate-docs` to regenerate docs.
6. Tag the release.

## References

- `src/mcp/machine_codes.rs` — machine code registry
- `src/mcp/registry/types.rs` — `ToolSpec`, `ToolExposure`, `ToolStability`
- `src/mcp/compat.rs` — `CompatibilityMode`
- `architecture/machine-codes.md` — machine code taxonomy
- `architecture/compatibility.md` — CompatibilityMode usage details
- `AGENTS.md` — tool counts, profiles, architecture overview
