# Phase 6: First-Class codegg Edit Preflight

## Goal

Promote `edit_preflight` into a single, first-class harness API for model-authored edits. The current handler already composes sub-tools (text_replace_check, patch_apply_check, line_range_extract, text_fingerprint) but lacks path scope validation, Unicode/newline policy, edit metadata, and a complete verdict vocabulary. This phase closes those gaps so codegg can call a single typed API before applying any model-authored edit.

## Current State

### What exists today

| Component | Location | Notes |
|-----------|----------|-------|
| `edit_preflight` handler | `src/tools/patch.rs:240-604` | 364 lines, composes 3 sub-tools + fingerprint |
| `EditPreflightInput` | `src/preflight/mod.rs:527-563` | 9 fields: original, mode, old/new/patch/start_line/end_line, fingerprint, strict |
| `EditPreflightOutput` | `src/preflight/mod.rs:566-584` | 8 fields: ok_to_apply, mode, verdict, machine_code, summary, findings, recommended_next_tool, raw |
| `EditVerdict` enum | `src/preflight/mod.rs:114-143` | Allow, Review, Block, SafeToApply, SafeWithWarnings, Other |
| ToolSpec | `src/mcp/specs/patch.rs:40-56` | 5 profiles, Default exposure, composite, Heavy cost |
| Schema (input) | `src/mcp/schemas/patch.rs:25-53` | Missing `verdict` in output schema |
| Schema (output) | `src/mcp/schemas/patch.rs:55-67` | No `verdict`, no `newline_policy`, no `unicode_policy` |

### What's missing

1. **Path scope**: No `file_path`/`workspace_root` input — can't validate the edit target is inside the repo.
2. **Newline policy**: No newline consistency check on replacement text (mixed CRLF/LF risk).
3. **Unicode policy**: No security inspection of the replacement text (invisible chars, confusables, bidi).
4. **Edit metadata**: No way to pass context (e.g., "model wants to add a function") for audit trails.
5. **Verdict vocabulary**: Current verdicts (`allow`/`review`/`block`/`safe_to_apply`/`safe_with_warnings`) don't distinguish between stale context, old_text_not_found, multiple_matches, patch_failed, etc. The plan calls for richer machine-code-driven verdicts.
6. **Output schema gaps**: `verdict` field missing from output schema definition.

## Plan

### Step 1: Expand `EditPreflightInput` and output schema

**File: `src/preflight/mod.rs`**

Add new fields to `EditPreflightInput`:

```rust
pub struct EditPreflightInput {
    // Existing fields (unchanged)
    pub original: String,
    pub mode: ReplacementMode,
    pub old: Option<String>,
    pub new: Option<String>,
    pub patch: Option<String>,
    pub start_line: Option<u64>,
    pub end_line: Option<u64>,
    pub expected_fingerprint: Option<String>,
    pub strict: bool,

    // NEW: path scope
    pub file_path: Option<String>,         // target file path
    pub workspace_root: Option<String>,    // repo/workspace root for scope check

    // NEW: newline policy
    pub newline_policy: NewlinePolicy,     // require_lf | require_crlf | mixed_ok | match_original

    // NEW: unicode policy
    pub unicode_policy: UnicodePolicy,     // allow | review_if_invisible | block_if_risky

    // NEW: edit metadata (audit trail)
    pub edit_metadata: Option<EditMetadata>, // reason, model_id, etc.
}
```

New enums:

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum NewlinePolicy {
    #[default]
    MatchOriginal,  // accept whatever the original has
    RequireLf,      // enforce LF-only in replacement
    RequireCrlf,    // enforce CRLF in replacement
    MixedOk,        // no newline validation
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum UnicodePolicy {
    #[default]
    Allow,             // no unicode security check
    ReviewIfInvisible, // warn on invisible/confusable chars
    BlockIfRisky,      // block on high-severity unicode findings
}

#[derive(Clone, Debug, Default)]
pub struct EditMetadata {
    pub reason: Option<String>,      // e.g. "add error handling"
    pub model_id: Option<String>,    // which model generated this edit
    pub source_tool: Option<String>, // e.g. "apply_patch", "edit_file"
}
```

Update `EditPreflightOutput` to include:

```rust
pub struct EditPreflightOutput {
    // Existing fields
    pub ok_to_apply: bool,
    pub mode: String,
    pub verdict: EditVerdict,
    pub machine_code: String,
    pub summary: String,
    pub findings: Vec<Finding>,
    pub recommended_next_tool: Option<String>,
    pub raw: Value,

    // NEW
    pub path_scope: Option<PathScopeResult>,  // from path_scope_check composition
    pub newline_check: Option<NewlineCheckResult>,
    pub unicode_check: Option<UnicodeCheckResult>,
    pub fingerprint: Option<FingerprintResult>,
}

#[derive(Clone, Debug)]
pub struct PathScopeResult {
    pub inside_root: bool,
    pub relative_path: String,
    pub escapes_via_dotdot: bool,
}

#[derive(Clone, Debug)]
pub struct NewlineCheckResult {
    pub original_style: String,  // "LF" / "CRLF" / "mixed" / "none"
    pub replacement_style: String,
    pub consistent: bool,
    pub policy_satisfied: bool,
}

#[derive(Clone, Debug)]
pub struct UnicodeCheckResult {
    pub verdict: String,  // "allow" / "review" / "block"
    pub findings_count: usize,
    pub has_invisible: bool,
    pub has_confusables: bool,
}

#[derive(Clone, Debug)]
pub struct FingerprintResult {
    pub matched: bool,
    pub expected: String,
    pub actual: String,
}
```

### Step 2: Update `edit_preflight` handler

**File: `src/tools/patch.rs`** (handler at lines 240-604)

Add new composition steps after existing sub-tool calls:

1. **Path scope check** (if `file_path` and `workspace_root` provided):
   - Call `path_scope_check_tool` in-process with `{root: workspace_root, target: file_path}`
   - If `inside_root == false`: add PATH_SCOPE_ESCAPE finding (HIGH severity), set machine_code to `PATH_SCOPE_ESCAPE`, set verdict to BLOCK
   - Store result in `subresults["path_scope"]`

2. **Newline policy check** (on replacement text):
   - If `mode == "literal"` and `new` is provided: detect newline style of `new` vs `original`
   - If `mode == "patch"`: detect newline style of the unified diff content
   - If `mode == "line_range"`: N/A (no replacement text provided)
   - Apply policy: `RequireLf` → block if CRLF found; `RequireCrlf` → block if LF-only; `MatchOriginal` → warn if styles differ
   - Add NEWLINE_INCONSISTENCY finding (MEDIUM) if policy violated

3. **Unicode security check** (on replacement text):
   - If `mode == "literal"` and `new` is provided: call `text_security_inspect` on `new` with `policy: "source_code"`
   - If `mode == "patch"`: extract added lines from patch, call `text_security_inspect` on those
   - Apply `unicode_policy`: `Allow` → skip; `ReviewIfInvisible` → convert HIGH findings to MEDIUM; `BlockIfRisky` → keep HIGH as BLOCK
   - Add UNICODE_RISK finding if issues found
   - Store in `subresults["unicode_check"]`

4. **Fingerprint for patch/line_range modes** (if `expected_fingerprint` provided):
   - Already done for literal mode. Extend to patch mode (compute fingerprint of the whole patched result) and line_range mode (compute fingerprint of the modified line range)
   - Add FINGERPRINT_MISMATCH finding if mismatch

### Step 3: Update output schema

**File: `src/mcp/schemas/patch.rs`**

Add to `edit_preflight_output`:

```rust
pub fn edit_preflight_output() -> Value {
    json!({
        "type": "object",
        "properties": {
            "ok_to_apply": { "type": "boolean" },
            "mode": { "type": "string", "enum": ["literal", "patch", "line_range"] },
            "verdict": { "type": "string", "enum": ["allow", "review", "block", "safe_to_apply", "safe_with_warnings"] },
            "machine_code": { "type": "string" },
            "summary": { "type": "string" },
            "findings": { "type": "array" },
            "recommended_next_tool": { "type": ["string", "null"] },
            "subresults": { "type": "object" },
            // NEW
            "path_scope": { "type": ["object", "null"] },
            "newline_check": { "type": ["object", "null"] },
            "unicode_check": { "type": ["object", "null"] },
            "fingerprint": { "type": ["object", "null"] }
        },
        "required": ["ok_to_apply", "mode", "verdict", "machine_code", "summary", "findings"]
    })
}
```

### Step 4: Update ToolSpec input schema

**File: `src/mcp/schemas/patch.rs`**

Add new input fields to `edit_preflight_input`:

```rust
// Add to the properties:
"file_path": { "type": "string", "description": "Target file path for scope validation" },
"workspace_root": { "type": "string", "description": "Workspace root for scope check" },
"newline_policy": { "type": "string", "enum": ["match_original", "require_lf", "require_crlf", "mixed_ok"], "default": "match_original" },
"unicode_policy": { "type": "string", "enum": ["allow", "review_if_invisible", "block_if_risky"], "default": "allow" },
"edit_reason": { "type": "string", "description": "Audit trail: why this edit is being made" },
"model_id": { "type": "string", "description": "Audit trail: which model generated this edit" },
"source_tool": { "type": "string", "description": "Audit trail: tool that originated this edit" }
```

### Step 5: Update `EditPreflight::parse_response`

**File: `src/preflight/mod.rs`** (parse_response at lines 627-670)

Add parsing for new optional fields:

```rust
// Path scope (optional — only present when file_path was provided)
let path_scope = result.get("path_scope").and_then(|v| {
    if v.is_null() { return None; }
    Some(PathScopeResult {
        inside_root: v.get("inside_root")?.as_bool()?,
        relative_path: v.get("relative_path")?.as_str()?.to_string(),
        escapes_via_dotdot: v.get("escapes_via_dotdot")?.as_bool()?,
    })
});

// Newline check (optional)
let newline_check = result.get("newline_check").and_then(|v| { /* ... */ });

// Unicode check (optional)
let unicode_check = result.get("unicode_check").and_then(|v| { /* ... */ });

// Fingerprint (optional)
let fingerprint = result.get("fingerprint").and_then(|v| { /* ... */ });
```

### Step 6: Expand `EditVerdict` vocabulary

**File: `src/preflight/mod.rs`**

The plan calls for unified verdicts like `stale_context`, `old_text_not_found`, `multiple_matches`, `patch_failed`, `line_range_invalid`, `path_scope_escape`, `unicode_risk`, `blocked`. These are already communicated via machine_codes today. The verdict enum should add:

```rust
pub enum EditVerdict {
    Allow,
    Review,
    Block,
    SafeToApply,
    SafeWithWarnings,
    // NEW: machine-code-driven verdicts
    StaleContext,        // fingerprint mismatch
    OldTextNotFound,     // literal mode, no match
    MultipleMatches,     // literal mode, >1 match
    PatchFailed,         // patch mode, doesn't apply
    LineRangeInvalid,    // line_range mode, out of bounds
    PathScopeEscape,     // target outside workspace root
    UnicodeRisk,         // unicode security check found risks
    Blocked,             // explicit block from any check
    Other(String),
}
```

However, this is a **breaking change** to the typed wrapper. Since the `EditVerdict` has `Other(String)` for forward compat, and the handler already returns machine_code separately, the pragmatic approach is:

- **Keep the existing 5 verdicts** + `Other(String)` for forward compat
- The handler already maps machine_codes → verdict (HIGH → BLOCK, MEDIUM → REVIEW, else → ALLOW)
- The new checks (path_scope, unicode, newline) feed into findings → machine_code → verdict chain naturally
- Document that `machine_code` is the primary routing signal, `verdict` is the high-level classification

**Decision**: Don't expand the enum variants. The `Other(String)` variant handles forward compat, and the machine_code field carries the precise routing signal. This keeps the typed wrapper backward-compatible.

### Step 7: Add tests

**File: `tests/mcp/test_edit_preflight_enhanced.rs`** (new)

Test cases:
1. **Path scope escape**: `file_path: "../etc/passwd"`, `workspace_root: "/repo"` → BLOCK with PATH_SCOPE_ESCAPE
2. **Path scope safe**: `file_path: "src/main.rs"`, `workspace_root: "/repo"` → no path_scope finding
3. **Newline policy violation**: original has LF, replacement has CRLF, policy=require_lf → NEWLINE_INCONSISTENCY finding
4. **Newline policy satisfied**: original has LF, replacement has LF, policy=require_lf → no finding
5. **Unicode risk (block)**: replacement contains bidi override chars, policy=block_if_risky → BLOCK with UNICODE_RISK
6. **Unicode risk (review)**: replacement contains confusable chars, policy=review_if_invisible → REVIEW
7. **Unicode allow**: replacement contains unicode, policy=allow → no unicode finding
8. **Fingerprint match (patch mode)**: provide expected_fingerprint, patch applies → fingerprint matched
9. **Fingerprint mismatch (patch mode)**: provide wrong fingerprint → FINGERPRINT_MISMATCH finding
10. **Edit metadata passthrough**: verify metadata appears in subresults for audit trail
11. **Full composition**: path_scope + newline + unicode + fingerprint all checked together
12. **Backward compat**: existing calls without new fields still work identically

**File: `src/preflight/mod.rs`** (inline tests)

Add contract tests:
13. Parse response with new optional fields present
14. Parse response without new optional fields (backward compat)
15. `NewlinePolicy` and `UnicodePolicy` serialization

### Step 8: Update documentation

**Files:**
- `architecture/mcp-server.md` — add edit_preflight composition diagram showing all sub-tools
- `.skills/mcp-tools.md` — note edit_preflight as the canonical example of composite tool composition
- `README.md` — update edit_preflight description in the generated tools table (description text in ToolSpec)
- `AGENTS.md` — note the enhanced edit_preflight capabilities

## Verification Order

```
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo run --bin generate-docs -- --check
cargo test --lib
cargo test --doc
cargo test --test lib mcp::test_edit_preflight_enhanced
cargo test --test lib mcp::test_composite_tools
cargo test --test lib mcp::test_hardening_and_gaps
cargo test --test lib mcp::test_response_structure
cargo test --test lib mcp::test_edge_cases
cargo test --test lib mcp::test_machine_codes
cargo test --test lib mcp::test_golden_fixtures
cargo test --test lib mcp::test_tool_coverage
cargo test --test lib mcp::test_deterministic_real_use
cargo test --test lib mcp::test_lifecycle_and_gaps
cargo test --test lib mcp::test_protocol
cargo test --test lib mcp::test_mcp_tools
cargo test --test lib mcp::test_comprehensive_parity
cargo test --test lib mcp::test_tool_gaps
cargo test --test lib mcp::test_parity
cargo test --test lib mcp::test_profile_audience
cargo test --test lib mcp::test_preflight
cargo test --test lib mcp::test_determinism
cargo package --verbose
```

## Risk Assessment

| Risk | Mitigation |
|------|-----------|
| Breaking existing edit_preflight calls | All new fields are Optional with defaults; backward-compatible |
| Performance regression from composing 3 more sub-tools | Sub-tools are already in-process (no registry overhead); path_scope_check and text_security_inspect are cheap |
| Schema drift between handler output and schema definition | Generated docs check catches drift; add new fields to schema |
| EditVerdict expansion breaks typed wrapper | Keep existing 5 variants + Other(String); machine_code carries precise signal |
| test_determinism timeout | Already known slow test; not a regression |
