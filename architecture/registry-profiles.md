# Registry, Profiles & Audience Filtering

How eggsact registers tools, organizes them into profiles, and controls visibility by audience.

See also: [MCP Server](mcp-server.md), [Agent API](agent-api.md), [Compatibility Mode](compatibility.md)

## Files

| File | Purpose |
|------|---------|
| `src/mcp/registry/types.rs` | `ToolSpec`, `ToolDefinition`, `ToolExposure`, `ToolCost`, `ToolStability`, `ToolHandler` |
| `src/mcp/registry/all_tools.rs` | `ALL_TOOLS_VEC` (LazyLock), `PROFILE_NAMES` (11 profiles) |
| `src/mcp/registry/listing.rs` | `get_tool()`, `tools_for_profile()`, `tools_for_profile_audience()`, `mcp_tool_definitions()`, `ROUTE_CRITICAL_TOOLS`, `find_close_match()` |
| `src/mcp/registry/mod.rs` | Re-exports, profile snapshot tests |
| `src/mcp/specs/*.rs` | 20 `ToolSpec` declaration files (one per category) |
| `src/mcp/schemas/*.rs` | 20 JSON-schema builder files (one per category) |

---

## ToolSpec (Single Source of Truth)

Every tool is declared once as a `ToolSpec` in `src/mcp/specs/<category>.rs`:

```rust
pub const MATH_TOOLS: &[ToolSpec] = &[
    ToolSpec {
        name: "math_eval",
        description: "Evaluate arithmetic...",
        handler: math_eval,              // fn from src/tools/math.rs
        input_schema: math_eval_input,    // fn() -> Value from src/mcp/schemas/math.rs
        output_schema: math_eval_output,
        category: "math",
        tier: 0,                          // 0=essential, 1=common, 2=advanced, 3=specialized
        profiles: &["full", "default", "human_math"],
        tags: &["math", "evaluation", "arithmetic", "units", "constants"],
        exposure: ToolExposure::Default,
        harness_use: &["none"],
        aliases: &[],
        cost: ToolCost::Moderate,
        stability: ToolStability::Stable,
        composite: false,
    },
];
```

### ToolSpec Fields

| Field | Type | Purpose |
|-------|------|---------|
| `name` | `&str` | Tool identifier (unique across all tools) |
| `description` | `&str` | Human-readable description for `tools/list` |
| `handler` | `ToolHandler` | Function pointer `fn(&Value) -> ToolResponse` |
| `input_schema` | `fn() -> Value` | JSON Schema builder (called lazily) |
| `output_schema` | `fn() -> Value` | JSON Schema for response shape |
| `category` | `&str` | Category name for grouping |
| `tier` | `u8` | 0=essential, 1=common, 2=advanced, 3=specialized |
| `profiles` | `&[&str]` | Which named profiles include this tool |
| `tags` | `&[&str]` | Search/filter tags |
| `exposure` | `ToolExposure` | Visibility level (see below) |
| `harness_use` | `&[&str]` | How harnesses use this tool |
| `aliases` | `&[&str]` | Alternative names for the tool |
| `cost` | `ToolCost` | `Cheap`, `Moderate`, `Heavy` — maps to `ToolBudget` |
| `stability` | `ToolStability` | `Stable`, `Deprecated`, `Experimental` |
| `composite` | `bool` | Whether tool orchestrates other tools internally |

### ToolExposure Levels

| Level | Meaning | Appears in Model? | Appears in Harness? | Appears in Debug? |
|-------|---------|-------------------|---------------------|-------------------|
| `Default` | Always visible | Yes | Yes | Yes |
| `Contextual` | Visible when relevant | Yes | Yes | Yes |
| `ExpertOnly` | Advanced tools | Yes | Yes | Yes |
| `HarnessOnly` | Harness-only tools (e.g. diagnostics) | No | Yes | Yes |
| `Hidden` | Not in any listing | No | No | No |

### ToolStability

| Level | Meaning |
|-------|---------|
| `Stable` | Guaranteed API stability within semver |
| `Deprecated` | Will be removed; emits deprecation warning |
| `Experimental` | May change without notice |

---

## ALL_TOOLS Aggregation

`ALL_TOOLS_VEC` in `src/mcp/registry/all_tools.rs` is a `LazyLock<Vec<ToolSpec>>` that collects all 20 category slices at first access:

```
specs/math.rs → MATH_TOOLS (4)
specs/text.rs → TEXT_TOOLS (18)
specs/json.rs → JSON_TOOLS (6)
specs/regex.rs → REGEX_TOOLS (3)
specs/validation.rs → VALIDATION_TOOLS (4)
specs/path.rs → PATH_TOOLS (6)
specs/shell.rs → SHELL_TOOLS (4)
specs/list.rs → LIST_TOOLS (3)
specs/markdown.rs → MARKDOWN_TOOLS (2)
specs/patch.rs → PATCH_TOOLS (5)
specs/config.rs → CONFIG_TOOLS (3)
specs/toml.rs → TOML_TOOLS (1)
specs/identifier.rs → IDENTIFIER_TOOLS (3)
specs/unicode.rs → UNICODE_TOOLS (2)
specs/version.rs → VERSION_TOOLS (2)
specs/cargo.rs → CARGO_TOOLS (1)
specs/dependency.rs → DEPENDENCY_TOOLS (1)
specs/repo.rs → REPO_TOOLS (5)
specs/diagnostics.rs → DIAGNOSTICS_TOOLS (3)
specs/analysis.rs → ANALYSIS_TOOLS (4)
```

A test (`tool_registration_tables_are_in_sync`) verifies that `ALL_TOOLS_VEC.len()` matches the sum of all category slice lengths. Adding a tool requires only one `ToolSpec` entry — no manual registration.

---

## Profile System

### Named Profiles

11 named profiles control which tools are exposed to which consumers. Counts are what `tools/list` returns per audience (measured on v1.2.3):

| Profile | Purpose | Model | Harness | Debug |
|---------|---------|-------|---------|-------|
| `full` | All non-hidden tools | 71 | 80 | 80 |
| `default` | Essential + common tools | 25 | 25 | 25 |
| `codegg_core_min` | Minimal coder-agent set | 6 | 6 | 6 |
| `codegg_core` | Standard coder-agent set | 19 | 19 | 19 |
| `codegg_preflight` | Preflight-focused set | 7 | 13 | 13 |
| `codegg_patch` | Patch editing set | 10 | 12 | 12 |
| `codegg_config` | Config inspection set | 14 | 14 | 14 |
| `codegg_unicode_security` | Unicode/security set | 6 | 8 | 8 |
| `codegg_shell` | Shell command set | 5 | 6 | 6 |
| `codegg_repo_audit` | Repository audit set | 18 | 18 | 18 |
| `human_math` | Human-readable math | 4 | 4 | 4 |

Profiles are not audience-bound; the audience filter applies on top of profile membership (`Model` excludes `HarnessOnly`, both exclude `Hidden`).

`Profile::from_str_opt()` is strict — returns `None` for unknown names. Use `Profile::custom(name)` for ad-hoc profiles.

### Profile Resolution

- **MCP server**: Profile set once at startup via `EGGCALC_MCP_PROFILE` env var. Applies to all `tools/call` requests.
- **In-process API**: Each `ToolRegistry` is bound to one profile at construction time.
- **`tools/list`** accepts a `profile` parameter for filtering the listing, but that does not change which profile `tools/call` enforces.

---

## Audience System

### ToolAudience

| Audience | Description | Excluded Exposures |
|----------|-------------|-------------------|
| `Model` | LLM-facing tools | HarnessOnly + Hidden |
| `Harness` | Codegg harness tools | Hidden |
| `Debug` | All non-hidden tools | None |

`ToolAudience::can_execute_exposure()` enforces audience at dispatch time — MCP `tools/call` rejects harness-only tools for model audience.

### Audience Resolution

- **MCP server**: Default audience is `Model`. Overridable via `EGGCALC_MCP_AUDIENCE` env var.
- **In-process API**: Default audience is `Model`. Pass `ToolAudience::Harness` to `ToolRegistry::with_profile_and_audience()` for harness workflows.

---

## Tool Lookup & Filtering

### Key Functions (listing.rs)

| Function | Purpose |
|----------|---------|
| `get_tool(name)` | Look up a tool by exact name (unfiltered) |
| `tool_handler_for(name)` | Get the handler function pointer |
| `tools_for_profile(profile)` | All tools in a profile (no audience filter) |
| `tools_for_profile_audience(profile, audience)` | Tools filtered by profile + audience exposure |
| `list_tool_definitions(...)` | Full filtering by profile/names/tier/tags/schema_detail |
| `compact_input_schema(schema)` | Truncate descriptions to 120 chars, strip defaults |
| `find_close_match(name)` | Levenshtein-based tool name suggestions |

Registry-check helpers live on `ToolRegistry` in `src/agent/mod.rs`: `has_tool(name)` (existence with profile/audience filtering), `get_tool_unfiltered(name)` (administrative lookup bypassing audience/exposure), and `has_registered_tool(name)` (existence without filtering).

### Schema Compaction

`EGGCALC_MCP_SCHEMA_DETAIL` controls schema verbosity in `tools/list` (`tools/list` also accepts a per-request `schema_detail` parameter):

| Value | Behavior |
|-------|----------|
| `full` (default) | Full JSON Schema with descriptions and defaults; deprecated field always emitted |
| `normal` | Accepted value, currently identical output to `full` |
| `compact` | Descriptions truncated to 120 chars, defaults stripped, schemas compacted, tier/tags dropped |

---

## Route-Critical Tools

A subset of tools are classified as **route-critical** — they produce structured verdicts and machine codes that downstream harnesses depend on for routing decisions:

| Tool | Category | Verdict Types |
|------|----------|---------------|
| `edit_preflight` | patch | allow / review / block |
| `command_preflight` | shell | allow / review / block |
| `config_preflight` | config | valid / valid_with_warnings / invalid |
| `patch_apply_check` | patch | allow / review / block |
| `text_security_inspect` | text | allow / review / block |

`ROUTE_CRITICAL_TOOLS` constant and `is_route_critical()` helper in `registry/listing.rs` identify these tools. Route-critical tools **must** always emit `machine_code` and `verdict` in their response envelope. Verified by fixture-backed route-contract tests.

---

## Tool Registration Pattern

Adding a new tool requires exactly one step:

1. Add a `ToolSpec` entry to `src/mcp/specs/<category>.rs`

No manual registration, no config file, no build step. The `tool_registration_tables_are_in_sync` test catches drift between the spec count and `ALL_TOOLS_VEC`.

### Adding a New Category

1. Create `src/mcp/specs/<category>.rs` with a `pub const CATEGORY_TOOLS: &[ToolSpec]` slice
2. Create `src/mcp/schemas/<category>.rs` with schema builder functions
3. Create `src/tools/<category>.rs` with handler functions
4. Add `mod <category>;` to `src/mcp/specs/mod.rs` and `src/mcp/schemas/mod.rs`
5. Add the category slice to `ALL_TOOLS_VEC` in `src/mcp/registry/all_tools.rs`
6. Add re-exports to `src/tools/mod.rs`

---

## Profile Snapshot Tests

Profile snapshot tests in `tests/mcp/test_hardening_and_gaps.rs` verify that:
- All 11 named profiles exist
- Each profile's tool list matches expected tool counts
- No unexpected tools appear in profiles
- Profile filtering is consistent between MCP and in-process paths
