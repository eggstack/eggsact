# Typed Service Layer

`src/services/` is the typed deterministic service layer for composite tools. It exists
to hold composite logic that would otherwise be duplicated across JSON tool adapters, or
worse, implemented by one adapter calling another. Each service owns a *fact model* —
canonical classifications, single-parse aggregates, composite style derivations — and
each `tools/*` adapter that needs those facts is a *projection* over the same service
call. Repo analysis, patch analysis, and text-security inspection are each reachable
from more than one tool; without this layer the same classifier would exist two or
three times and the wire output could drift per caller.

The layer is a pure Rust library surface. It composes `crate::text::*` cores, returns
`serde`-serializable structs, and knows nothing about the JSON-RPC boundary. See also:
[tools.md](tools.md), [text-library.md](text-library.md), [overview.md](overview.md),
[budget-concurrency.md](budget-concurrency.md).

Verified against HEAD `65c916b`.

## Layering contract

The canonical flow is declared in [`src/services/mod.rs`](../src/services/mod.rs) and
repeated in [tools.md](tools.md):

```text
Typed deterministic core (`crate::text::*`)
        |
        +--> typed composite/service (this module)
        |
        +--> `tools/*` JSON adapter -> `ToolResponse`
                              |
                    MCP / `ToolRegistry`
```

A service is a *composition* of typed cores. It is not a *wire format*. The
prohibitions are hard:

| Prohibited in `src/services/` | Where that responsibility actually lives |
|-------------------------------|------------------------------------------|
| `ToolResponse` / MCP machine codes | `src/mcp/response.rs`, built at the adapter boundary |
| MCP registry, `ToolSpec`, profile/audience policy | `src/mcp/registry/`, `src/agent/` |
| JSON-schema validation / argument parsing | Tool schemas + adapter `args` parsing |
| Calling a sibling `tools::*` handler | Nothing — the handler-to-handler graph is closed (see [tools.md](tools.md)) |
| Reading the clock, env, network, or the filesystem | Nothing — explicit inputs only |

The rationale is that **the JSON boundary is the only place wire shape is built, exactly
once**. If a service emitted JSON, or returned a `ToolResponse`, the same facts would be
serialized by two callers and the envelope would have two places to drift. Keeping the
boundary intact also means a composite service can be unit-tested without an MCP server
or a `serde_json::Value` argument object — every service in this module has a
`#[cfg(test)] mod tests` that constructs plain `&str`/`&[String]` inputs.

Sibling-service calls *are* allowed and are the point of the design:
`src/services/patch_analysis.rs` calls `src/services/repo.rs` for path-role facts, and
`src/services/newline.rs` calls `src/services/fingerprint.rs`. The rule is "call a typed
function, never a handler."

### Module inventory

| File | Lines | Public surface re-exported from `services/mod.rs` |
|------|-------|----------------------------------------------|
| [`mod.rs`](../src/services/mod.rs) | 34 | module docs, layering contract, re-exports |
| [`fingerprint.rs`](../src/services/fingerprint.rs) | 57 | `FingerprintFacts`, `fingerprint_facts` |
| [`newline.rs`](../src/services/newline.rs) | 96 | `NewlineFacts`, `newline_facts` |
| [`patch_analysis.rs`](../src/services/patch_analysis.rs) | 335 | `analyze_patch`, `PatchAnalysis`, `PatchFileFacts` |
| [`repo.rs`](../src/services/repo.rs) | 943 | `repo_facts`, `RepoFacts` |
| [`security.rs`](../src/services/security.rs) | 533 | `inspect_text_security`, `SecurityFinding`, `SecurityInspection`, `SecurityInspectionCancelled` |

`mod.rs` re-exports selectively. `PatchFileFacts` is re-exported, but
`SimpleLineRange` and `RenamePair` (same module), `LanguageEvidence` and the
`RUST_MANIFESTS` / `RUST_SOURCE_HINTS` / `PYTHON_MANIFESTS` / `NODE_MANIFESTS` /
`GO_MANIFESTS` constants (`repo.rs`) are `pub` only within their submodule and are
reached as `crate::services::<module>::<item>`. All five modules are `pub mod` in
`mod.rs` and `src/lib.rs` exposes `pub mod services`, so the full item surface is
stable public API even though it is not the recommended import surface.

## `services::fingerprint` — fingerprint facts

Purpose: a thin typed wrapper over the `text_fingerprint` core with the `raw`/`raw`
defaults the edit-preflight fingerprint stage depends on.

```rust
pub struct FingerprintFacts {
    pub sha256: String,
    pub newline_style: String,
}

pub fn fingerprint_facts(text: &str) -> FingerprintFacts;
```

- Composes [`crate::text::text_fingerprint`](../src/text/transform.rs), called as
  `text_fingerprint(text, "raw", "raw", false, false)` — no Unicode normalization, no
  newline normalization, no final-newline trimming, no casefolding. The doc comment
  pins this to the same input tuple `edit_preflight` used when it called the
  `text_fingerprint` adapter with `{"unicode": "raw", "newline": "raw"}`.
- Owns: the hex SHA-256 of the canonical form, plus the detected newline style
  (`"LF"`, `"CRLF"`, `"CR"`, `"mixed"`, `"none"`). It deliberately keeps *only* those
  two fields out of the core's larger result.
- Cancellation: none. The call is linear over the input and takes no stop view.
- Derives `Clone, Debug, PartialEq, Eq, Serialize, Deserialize`.

## `services::newline` — composite newline style

Purpose: derive the *composite* newline style that `edit_preflight` reports, without
constructing a JSON argument object for `text_fingerprint` and without parsing a
`ToolResponse`.

```rust
pub struct NewlineFacts {
    pub style: String,
    pub original_style: String,
    pub replacement_style: Option<String>,
    pub mixed: bool,
    pub recommended_normalization: Option<String>,
    pub policy: String,
}

pub fn newline_facts(original: &str, replacement: Option<&str>, policy: &str) -> NewlineFacts;
```

- Composes `super::fingerprint::fingerprint_facts` (not the `text` core directly) for
  both the original and, when supplied, the replacement text.
- Owns: the composite `style` derivation, which is the part that is genuinely composite
  rather than a single measurement. It is `"mixed"` when the original is `"mixed"`, when
  the replacement is `"mixed"`, or when both are concrete non-`none` styles that
  differ; otherwise it is the original style. `recommended_normalization` is
  `Some("lf")` for `normalize_lf`, `Some("crlf")` for `normalize_crlf`, and `None` for
  any other policy (including `check`).
- `policy` is echoed back verbatim as given; the function does not validate it.
- Cancellation: none.

## `services::patch_analysis` — single-parse neutral diff facts

Purpose: parse a unified diff **once** and compute neutral facts that three patch tools
project differently. This is the clearest example of the layer's value: without it,
`patch_summary`, `patch_contract_check`, and `diff_risk_classify` would each re-parse and
each re-classify paths, and could disagree about path identity or add/delete totals.

```rust
pub struct PatchAnalysis {
    pub ok: bool,
    pub error: Option<String>,
    pub files: Vec<PatchFileFacts>,
    pub files_changed: usize,
    pub hunks_total: usize,
    pub additions: usize,
    pub deletions: usize,
    pub renames_detected: Vec<RenamePair>,
    pub binary_patch_detected: bool,
    pub line_ranges_by_file: BTreeMap<String, Vec<SimpleLineRange>>,
}

pub fn analyze_patch(patch_text: &str) -> PatchAnalysis;
```

Supporting types: `PatchFileFacts` (per-file `effective_path`, `old_path`,
`new_path`, `bucket`, the `is_manifest` / `is_lockfile` / `is_ci` / `is_config` /
`is_generated` / `is_vendor` / `is_security_sensitive` / `is_docs` / `is_tests`
booleans, `additions`, `deletions`, `hunks`, `line_ranges`), `SimpleLineRange`
(inclusive destination `start`/`end`, mirroring `text::patch::LineRange`), and
`RenamePair` (`from` / `to`). `PatchAnalysis` also exposes `total_changes()` and
`deletions_by_file()`.

- Composes [`crate::text::patch::parse_unified_diff`](../src/text/patch.rs) (one call
  per invocation) and `super::repo` for path roles: `repo_facts::path_bucket(&eff)`
  and `repo_facts::is_security_sensitive_path(&eff)`. There is **no** local classifier —
  this is what stops a manifest/lockfile/CI/generated/vendor path from getting different
  bucket facts depending on which patch tool was called.
- Owns: the `200_000`-byte `MAX_PATCH_LENGTH` guard (returns `ok: false` with an error
  string before parsing), effective-path selection (`new` unless empty or `/dev/null`,
  else `old`), rename-pair detection, per-hunk add/delete counting after stripping a
  trailing `\r` from each diff line, `binary_patch_detected` (presence of
  `"GIT binary patch"` or a NUL byte), and `line_ranges_by_file`. That last map is keyed
  by `new_file` (falling back to `old_file` when empty) — deliberately a different key
  from `effective_path`, which is the key for path identity. Records with both an empty
  effective path and an empty file key (e.g. `/dev/null`-only entries) are skipped so
  `files`/`files_changed` stay aligned with the summary.
- A unit test compares the analysis against `crate::text::patch_summary` for
  `files_changed`, `hunks_total`, `additions`, `deletions`, `binary_patch_detected`, and
  rename count, explicitly as a drift guard against the underlying core.
- Cancellation: none. The parse is bounded by `MAX_PATCH_LENGTH`.
- Verdicts, categories, large-deletion thresholds, and risk routing deliberately do
  **not** live here — those are per-tool policy, and the module doc says so.

## `services::security` — text-security inspection pipeline

Purpose: implement the whole `text_security_inspect` pipeline over typed `text::*`
cores, so the adapter and `edit_preflight` can both share it without either calling the
other.

```rust
pub struct SecurityFinding {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub disposition: Option<String>,
}

impl SecurityFinding {
    pub fn new(
        code: &str,
        severity: &str,
        message: impl Into<String>,
        disposition: Option<&str>,
    ) -> Self;
}

pub struct SecurityInspection {
    pub verdict: String,
    pub machine_code: String,
    pub machine_codes: Vec<String>,
    pub findings: Vec<SecurityFinding>,
    pub normalized_changed: bool,
    pub summary: String,
    pub recommended_action: String,
    pub subresults: BTreeMap<String, serde_json::Value>,
}

pub fn inspect_text_security(
    text: &str,
    policy: &str,
    normalize: &str,
    detail: &str,
    should_stop: &dyn Fn() -> bool,
) -> Result<SecurityInspection, SecurityInspectionCancelled>;
```

Pipeline stages, in order:

1. **`text_inspect` essentials** — `text::unicode_tools::{find_invisibles,
   is_bidi_control, detect_mixed_scripts}` and `text::confusables::lookup`. Bidi
   controls are split from other invisibles with the typed `is_bidi_control` predicate,
   never a display-string test. Warning text and severity vocabulary mirror
   `text_inspect` exactly so `TEXT_INSPECT_WARNING` counts stay identical.
   **Cancellation check.**
2. **`unicode_policy_check`** — `text::unicode_policy_check(text, uc_policy, None)`,
   where `uc_policy` is `"source_code"` when `policy == "source_code"` and
   `"human_text"` otherwise. Core severities (`error`/`critical`/`danger` →
   `high`/blocking, `warn`/`warning` → `medium`/caution, `info` → `info`/informational)
   are mapped to the security vocabulary. **Cancellation check.**
3. **Normalization check** — when `normalize != "none"`, applies `NFC`/`NFD`/`NFKC`/
   `NFKD` via `unicode_normalization`, records `canonicalize_text` in `subresults`, and
   emits `NORMALIZATION_DIFF` when the text changed. No cancellation check (single pass).
4. **`prompt_input_inspect`** — `text::inspect_prompt::prompt_input_inspect(text, None,
   None)`, for `policy` in `prompt` / `markdown` / `default`. Emits
   `PROMPT_INJECTION_RISK` when any finding is `warn` or `error`. **Cancellation check.**
5. **`identifier_inspect`** — for `policy` in `identifier` / `default`. The service
   extracts identifier-shaped whitespace tokens (first char `_` or alphabetic, remaining
   chars alphanumeric or `_`) and calls
   `text::identifier_inspect(&words, "generic", "NFC", false, true)`. Emits
   `IDENT_WARNING` / `IDENT_COLLISION` (both `medium`/caution) and
   `IDENTIFIER_COLLISION_RISK` as the envelope code. **Cancellation check.**

Verdict derivation: any `high` finding → `block`; else any `medium` → `review`; else
`allow`. `machine_code` is `TEXT_SECURITY_OK` when the code list is empty, otherwise
the first code in first-seen order; `machine_codes` preserves that first-seen order and
de-duplicates.

- `detail` is documented as affecting diagnostic detail. In the implementation it
  governs exactly one thing: `max_items`, which is `10` for `summary` and `100`
  otherwise, truncating the invisible and confusable lists. The `subresults` map is
  populated at every stage regardless of `detail`.
- `normalized_changed` is read back out of the `canonicalize_text` subresult, so it is
  `false` whenever `normalize == "none"`.
- `subresults` is a `BTreeMap<String, serde_json::Value>` — a deliberate seam, not a
  layering violation: it carries diagnostic *core* results (`text_inspect`,
  `unicode_policy_check`, `canonicalize_text`, `prompt_input_inspect`,
  `identifier_inspect`) for the adapter to splice into the existing wire shape. It is
  not a `ToolResponse` and carries no envelope, verdict, or machine code.
- The severity/disposition/verdict constants are local `const`s in the function body so
  the wire output stays byte-identical to the adapter vocabulary it replaced.
- Derives: `SecurityFinding` is `Clone, Debug, PartialEq, Eq, Serialize, Deserialize`;
  `SecurityInspection` is `Clone, Debug` (not serializable — the adapter serializes it).

## `services::repo` — canonical repository facts

Purpose: one source of truth for ecosystem, path, and language classification. It is the
largest service (943 lines) because it carries the classification tables themselves, and
it is the one other services depend on.

```rust
pub struct RepoFacts {
    pub normalized_paths: Vec<String>,
    pub buckets: BTreeMap<String, Vec<String>>,
    pub project_types: Vec<String>,
    pub ecosystems: Vec<String>,
    pub manifests_by_ecosystem: BTreeMap<String, Vec<String>>,
    pub other_manifests: Vec<String>,
    pub config_paths: Vec<String>,
    pub lockfile_paths: Vec<String>,
    pub entrypoint_candidates: Vec<String>,
    pub high_leverage_paths: Vec<String>,
    pub tool_hints: Vec<String>,
    pub languages: Vec<LanguageEvidence>,
    pub is_unknown: bool,
    pub is_mixed: bool,
}

pub fn repo_facts(paths: &[String]) -> RepoFacts;
```

Supporting public surface:

| Item | Signature / value |
|------|------------------|
| `LanguageEvidence` | `name`, `file_count`, `extensions`, `confidence: f64` |
| `language_for_path` | `fn(path: &str) -> Option<String>` |
| `language_evidence` | `fn(paths: &[String]) -> Vec<LanguageEvidence>` |
| `detect_project_types` | `fn(paths: &[String]) -> Vec<String>` |
| `detect_ecosystems` | `fn(paths: &[String]) -> Vec<String>` |
| `classify_path` | `fn(path: &str) -> (String, bool, bool)` — `(bucket, is_hidden, is_dotfile)` |
| `path_bucket` | `fn(path: &str) -> String` |
| `is_manifest_bucket` / `is_lockfile_bucket` / `is_ci_path` / `is_config_bucket` / `is_generated_path` / `is_vendor_path` | `fn(path: &str) -> bool` |
| `is_security_sensitive_path` | `fn(path: &str) -> bool` |
| `classify_manifests` | `fn(paths: &[String]) -> (Vec<String> × 7)` — rust, python, node, go, other, config, lockfile |
| `RUST_MANIFESTS`, `RUST_SOURCE_HINTS`, `PYTHON_MANIFESTS`, `NODE_MANIFESTS`, `GO_MANIFESTS` | `pub const &[&str]` |

Private tables (do not duplicate these elsewhere): `CONFIG_PATTERNS`,
`LOCKFILE_PATTERNS`, `SECURITY_SENSITIVE_SUBSTRINGS`, `LANGUAGE_TABLE` (26 language
entries), `LanguageGuess`, `PathFact`.

- Composes no `text::*` core. It is a pure function of the input path list, and it is
  the *source* rather than a *consumer* — except that `patch_analysis.rs` consumes
  `path_bucket` and `is_security_sensitive_path` from it.
- Bucket priority in `classify_path`, first match wins: `manifests` > `lockfiles` >
  `ci` > `configs` > `tests` > `generated` > `vendor` > `assets` > `scripts` > `docs` >
  `source`. Manifest and lockfile basenames are a single `matches!` arm that re-splits
  on a trailing `lock` / `go.sum` / `-lock.`.
- Ecosystem detection: `detect_project_types` returns `["unknown"]` when nothing
  matches, appends `"mixed"` when more than one of rust/python/node/go matches, and
  otherwise returns the single match. Rust `src/main.rs`, `src/lib.rs`, and `src/bin/`
  are counted as source hints, so `src/main.rs` alone is `rust` in both the manifest
  projection and the language projection. `ecosystems` on `RepoFacts` is the same list
  with `unknown`/`mixed` filtered out.
- `SECURITY_SENSITIVE_SUBSTRINGS` is the shared security-sensitivity fact (`auth`,
  `token`, `secret`, `crypto`, `tls`, `ssl`, `permission`, `policy`, `sandbox`, `exec`,
  `shell`, matched case-insensitively as substrings). Whether that fact blocks, reviews,
  or is merely informational is consumer policy and is deliberately *not* in this module.
- `confidence` in `LanguageEvidence` preserves the legacy adapter scale: `0.9` for more
  than 5 files, `0.7` for more than 1, else `0.5`. `dockerfile` and `makefile` are
  matched by basename and carry no extension list.
- Ordering is load-bearing: bucket values, entrypoint candidates, and high-leverage
  paths stay in input order with no sorting or dedup, so existing wire output is
  byte-stable. Only `tool_hints` is sorted and deduped. Extension lists preserve
  first-seen order.
- No filesystem access, no clock, no env. Callers enforce `max_paths` and length bounds
  before calling. Cancellation: none — the functions are linear scans over a slice.

### Public-but-uncalled items

`detect_ecosystems` and the six `is_*_bucket` / `is_*_path` predicates are `pub` with no
in-tree caller at HEAD: `repo_facts` computes `ecosystems` inline with the same filter
rather than delegating, and `patch_analysis.rs` reads `PatchFileFacts.bucket` from
`path_bucket` rather than calling the six predicates. They are retained as part of the
public surface; a reviewer changing them should not assume a consumer exists.

## Cancellation model

`BudgetContext` ([`src/mcp/budget.rs`](../src/mcp/budget.rs)) carries a deadline, a
`ToolBudget`, and an optional `Arc<AtomicBool>` cancel flag, and its check helpers
(`check_not_cancelled`, `check_deadline`, `check_should_stop`, `check_text_bytes`,
`check_list_len`) return `Err(ToolResponse)` — i.e. they *build wire output*. That makes
`BudgetContext` unusable as a service dependency without inverting the layering.
`inspect_text_security` is the only service that can be cancelled, and it takes the
minimal view instead:

```rust
should_stop: &dyn Fn() -> bool
```

- The adapter supplies the bridge. Both call sites build
  `let should_stop = || budget_ctx.should_stop();`
  ([`src/tools/text.rs:2537`](../src/tools/text.rs) and
  [`src/tools/patch.rs:1032`](../src/tools/patch.rs)), so the service sees exactly the
  deadline-or-cancelled union that `BudgetContext::should_stop()` computes, with no
  handle to the budget itself.
- `should_stop()` is called at four points: after stages 1, 2, 4, and 5. Stage 3
  (normalization) is a single pass and is not checked. Adding a stage means deciding
  whether it also gets a check.
- Cancellation is an **`Err`, not a partial result**. The function returns
  `Err(SecurityInspectionCancelled)`, a unit struct carrying no verdict, no partial
  findings, and no machine code. There is deliberately no `SecurityInspection` variant
  meaning "stopped early": a half-computed pipeline has an untrustworthy verdict, so the
  service refuses to produce one.
- Propagation back to the wire is entirely the adapter's job. `text_security_inspect`
  handles the `Err` by calling `budget_ctx.check_should_stop("text_security_inspect")`
  and unwrapping it ([`src/tools/text.rs:2541-2547`](../src/tools/text.rs)). That is
  what re-derives which of the two reasons applies: `is_cancelled()` first (machine code
  `CANCELLED`), then deadline expiry (machine code `TIMEOUT`). The service cannot
  distinguish the two, and does not try to.
- The other four services take no stop view. `repo_facts` and `analyze_patch` are bounded
  by caller-enforced input caps and the `MAX_PATCH_LENGTH` guard; `fingerprint_facts` and
  `newline_facts` are single linear passes over already length-capped text.

Cooperative, not forceful: the MCP server sets the flag on timeout, but blocking work
already inside a core continues. See [budget-concurrency.md](budget-concurrency.md).

## Consumer map

| Service / item | Consumers (`tools/*.rs` handlers) | Note |
|----------------|------------------------------------|------|
| `repo::repo_facts` | `repo_manifest_inspect` ([`repo.rs:129`](../src/tools/repo.rs)), `repo_tree_summarize` ([`repo.rs:919`](../src/tools/repo.rs)), `repo_language_detect` ([`repo.rs:1328`](../src/tools/repo.rs)), `helpers::classify_paths` shim ([`helpers.rs:1307`](../src/tools/helpers.rs)) | Three projections + one compatibility shim. No `BudgetContext` for `repo_manifest_inspect`; the other three are budgeted. |
| `repo::detect_project_types` | `test_command_suggest` ([`repo.rs:1222`](../src/tools/repo.rs)) | Command templates stay in the adapter; only the canonical project types are requested. |
| `repo::classify_path` | `helpers::classify_path` shim ([`helpers.rs:1300`](../src/tools/helpers.rs)) | Shim is `#[allow(dead_code)]`; canonical location for new code. |
| `repo::path_bucket` | `patch_analysis` ([`patch_analysis.rs:160`](../src/services/patch_analysis.rs)), `helpers::classify_diff_path` shim | Backbone of shared path-role facts. |
| `repo::is_security_sensitive_path` | `patch_analysis` ([`patch_analysis.rs:214`](../src/services/patch_analysis.rs)) | Shared fact; contract vs risk verdicts differ downstream by intent. |
| `patch_analysis::analyze_patch` | `patch_summary` ([`patch.rs:224`](../src/tools/patch.rs)), `patch_contract_check` ([`patch.rs:1207`](../src/tools/patch.rs)), `diff_risk_classify` ([`patch.rs:1457`](../src/tools/patch.rs)) | One parse each; neutral presentation vs contract policy vs review-routing policy. `patch_apply_check` does **not** consume it. |
| `fingerprint::fingerprint_facts` | `edit_preflight` ([`patch.rs:856`](../src/tools/patch.rs)) | Literal replacement mode only; patch and line_range modes take their fingerprint from their own cores. |
| `newline::newline_facts` | `edit_preflight` ([`patch.rs:943`](../src/tools/patch.rs)) | Runs when `newline_policy != "skip"`. |
| `security::inspect_text_security` | `text_security_inspect` ([`text.rs:2539`](../src/tools/text.rs)), `edit_preflight` ([`patch.rs:1033`](../src/tools/patch.rs)) | The only cancellable service. `edit_preflight` passes `normalize = "none"` and `detail = "full"`. |

The `helpers.rs` entries (`classify_path`, `classify_paths`, `classify_diff_path`) are
explicit compatibility shims over `services::repo`, retained so external callers of
`tools::helpers` keep compiling. New code must call `crate::services::repo` directly.

## Invariants and review checklist

When this layer changes, check:

1. **No boundary types.** No `ToolResponse`, no `mcp::machine_codes`, no registry,
   `ToolSpec`, profile, or audience references, and no JSON-schema validation in
   `src/services/*.rs`. `serde_json::Value` appears only in
   `SecurityInspection::subresults` as a diagnostic seam. A service composes `text::*`
   cores and other services; it never reaches into `tools::*` to borrow an internal
   result.
2. **JSON built once.** The adapter owns argument parsing, validation, verdict and
   machine-code derivation, and envelope construction. If you find yourself needing a
   `serde_json::json!` envelope inside a service, the logic probably belongs at the
   boundary instead.
3. **Explicit inputs only.** No clock, timezone database, environment variable, network,
   or filesystem access. `repo_facts` is a pure function of the path slice it is given.
4. **Single parse / single classification.** If you add a diff-derived fact, add it to
   `PatchAnalysis` rather than re-parsing in an adapter. If you add a path predicate, add
   it to `services::repo` so repo tools and patch tools cannot disagree.
5. **Order stability.** Bucket values, entrypoint candidates, high-leverage paths, and
   extension lists are input-ordered and un-deduped; only `tool_hints` is sorted and
   deduped. Changing that changes wire output.
6. **Vocabulary parity.** `security.rs` constants, warning strings, and severity
   mappings are deliberate byte-level mirrors of the adapter they replaced. Changing a
   severity, disposition, verdict, or summary string here is a wire change.
7. **Cancellation shape stays a view.** Services take `&dyn Fn() -> bool`, never
   `BudgetContext`, and cancellation is always an `Err` with no partial result. Do not
   widen the stop view to carry the budget.
8. **Facts vs policy.** Suggested commands, verdicts, risk categories, and thresholds are
   adapter policy. If a fact starts carrying a verdict, it has crossed the line.
9. **Doc/tests follow.** Every service module has unit tests exercising the typed API
   without JSON. Extend them rather than moving coverage to the adapter layer. The
   `patch_analysis` test that compares against `text::patch_summary` is the drift guard
   for the shared counts — do not weaken it.

## Related drift

Three documentation errors were found while writing this file and have been fixed
in place. They are recorded here because the corrections are easy to regress:

- `tools.md` previously described the `services/*` dependency set as "only
  `serde`/`BTreeMap`/typed cores + `super::repo`". `src/services/security.rs` also
  imports `serde_json` (the `json!` macro) and `unicode_normalization`. The
  prohibition itself — no `ToolResponse`, registry, profile/audience, or schema
  validation — was and remains accurate.
- `tools.md` stated that `edit_preflight` calls `inspect_text_security` "on the
  replacement text". It actually selects the inspected text per
  `replacement_mode`: `new` for `literal` and `line_range`, and the raw `patch`
  text for `patch` mode (`src/tools/patch.rs:1006-1028`).
- The `Typed services` deep-dive pointer in `overview.md` targeted `tools.md`. It
  now targets this file.

Re-verify the first two claims against source before changing them again; the
`security.rs` import set grows over time.
