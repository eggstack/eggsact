# Phase 2 — Deterministic Output and TOML Corrections

## Status

- **Status:** complete
- **Repository:** `eggstack/eggsact`
- **Target branch:** `main`
- **Roadmap:** `plans/2026-07-31-lightweight-correctness-simplification-roadmap.md`
- **Depends on:** phase 1 regex/MCP contract repairs
- **Scope:** make public serialized maps byte-stable and correct localized TOML table/position defects
- **Expected change size:** medium but mechanical; no registry, transport, or parser redesign

## Objective

Provide deterministic public structured output where eggsact currently serializes ordinary `HashMap` values, and repair two TOML result defects:

1. scalar fields must not be reported as tables;
2. error columns must be based on Unicode characters rather than UTF-8 bytes.

This phase must distinguish public serialization concerns from internal lookup concerns. It is not a mandate to replace every `HashMap` in the crate.

---

# Hard constraints

This phase must not:

- add a general canonical-JSON subsystem;
- sort arrays whose order has semantic meaning;
- reorder findings, diffs, matches, or source-order lists without a documented reason;
- replace all `HashMap` uses indiscriminately;
- add `indexmap` or another ordered-map dependency unless the standard library cannot preserve a required input order;
- rewrite TOML parsing;
- change from `toml_edit` in this phase;
- redesign Cargo inspection;
- alter Python-compatible error wording except where position values are corrected;
- create snapshot-test infrastructure;
- add cross-process stress loops to ordinary CI;
- broaden into binary-size work from phase 4.

Prefer `BTreeMap` for public maps where lexical key order is acceptable and stable.

---

# Determinism contract to establish

Document one concise contract in the existing architecture documentation:

> For identical inputs and execution options within one eggsact version, public tool responses are semantically deterministic. Public JSON object fields generated from unordered internal collections are serialized in stable key order unless the tool explicitly documents source-order preservation.

The contract applies to tool response payloads, not to:

- internal cache layout;
- thread scheduling;
- MCP response arrival order across concurrent requests;
- arrays that intentionally preserve source or discovery order;
- error text originating from external parser versions unless normalized by eggsact.

MCP clients must still correlate concurrent responses by JSON-RPC ID. This phase does not restore request-order response serialization.

---

# Files to inspect first

At minimum inspect:

```text
src/text/validate.rs
src/text/cargo.rs
src/text/config.rs
src/text/toml.rs
src/text/identifier.rs
src/text/markdown.rs
src/text/synthesis.rs
src/tools/*.rs
src/mcp/response.rs
src/mcp/compat.rs
src/mcp/schemas/*.rs
tests/
architecture/text-library.md
architecture/tools.md
architecture/mcp-server.md
```

Search for public serialization boundaries:

```text
#[derive(... Serialize
HashMap<
serde_json::to_value
serde_json::json!
ToolResponse::success
groupdict
group_dict
keys:
dependencies:
target_specific
```

Create a temporary inventory while implementing:

| Type/field | Serialized publicly? | Ordering contract | Action |
|---|---|---|---|
| regex `groupdict` | yes | lexical capture name | `BTreeMap` |
| JSON shape `keys` | yes | lexical object key | `BTreeMap` or sorted insertion |
| Cargo dependency maps | yes | lexical dependency name | `BTreeMap` |
| internal duplicate groups | no | none | retain `HashMap` |
| internal lookup/cache map | no | none | retain `HashMap` |

The inventory need not become a permanent file.

---

# Workstream 1 — Identify public unordered maps

## Selection rule

Change a map only when all of the following are true:

1. it is serialized directly or converted to a public `serde_json::Value`;
2. callers do not depend on original source insertion order;
3. lexical key order is a valid deterministic representation;
4. changing the type does not require a new dependency.

Retain an internal `HashMap` when it is used only for lookup or aggregation and is converted into a sorted vector before output.

## Known high-priority candidates

Inspect and correct at least:

- `RegexMatch.groupdict`;
- `RegexFindIterMatch.group_dict`;
- `JsonShapeKey.keys`;
- Cargo `DependencySection.dependencies`;
- Cargo `DependencySection.dev_dependencies`;
- Cargo `DependencySection.build_dependencies`;
- Cargo `DependencySection.target_specific` and its nested dependency maps.

Also inspect other serialized structs for ordinary hash maps. Do not assume this list is complete.

## Preferred implementation

Use:

```rust
use std::collections::BTreeMap;
```

for public map fields.

Where a parser already preserves meaningful source order and the public contract requires that order, emit a `Vec<KeyValueEntry>` rather than adding a new map dependency. Do not make that change unless source order is explicitly useful and already documented.

## Compatibility considerations

JSON object ordering is not semantically significant, so changing key order should not alter consumers that correctly parse JSON. It can affect exact string snapshots. Update repository snapshots/golden outputs intentionally.

Do not rename fields or alter values while changing map types.

## Acceptance criteria

- Every public ordinary `HashMap` is either converted to deterministic output or documented as intentionally order-insensitive before serialization.
- Internal-only hash maps remain untouched unless a local cleanup is needed for compilation.
- No new ordered-map dependency is introduced.

---

# Workstream 2 — Deterministic map construction

Changing field types is insufficient if data is later copied into `serde_json::Map` through arbitrary iteration. Inspect each tool boundary.

Required rules:

- populate `BTreeMap` directly where possible;
- when producing `serde_json::Map`, iterate a sorted key sequence or a `BTreeMap`;
- do not sort user-facing arrays merely because their source used a hash collection;
- if a set is returned as an array and has no source-order contract, sort it explicitly before output;
- document case-sensitive lexical ordering where relevant.

Representative expected behavior:

```json
{
  "dependencies": {
    "ahash": {},
    "regex": {},
    "serde": {}
  }
}
```

not a process-dependent ordering.

## Findings order

Findings arrays should retain their existing deterministic pipeline order. If a findings list is built by iterating a hash map, convert that specific output construction to sorted keys before appending. Do not globally sort findings by message because pipeline order may carry meaning.

## Acceptance criteria

- Repeated serialization of representative outputs is byte-identical.
- Values and array order remain unchanged except where a previously unordered set/map is now explicitly ordered.

---

# Workstream 3 — Focused determinism regression tests

## Required unit tests

Add tests near the affected modules that assert lexical map-key order after serialization.

Representative patterns:

```rust
let json1 = serde_json::to_string(&result).unwrap();
let json2 = serde_json::to_string(&result).unwrap();
assert_eq!(json1, json2);
assert!(json1.find("\"alpha\"").unwrap() < json1.find("\"zeta\"").unwrap());
```

Prefer parsing and inspecting object key sequences when the serializer configuration permits it. Avoid brittle assertions over unrelated formatting.

Cover at minimum:

1. regex named capture groups inserted in non-lexical pattern order;
2. JSON shape keys supplied in non-lexical input order;
3. Cargo dependencies supplied in non-lexical TOML order;
4. target-specific Cargo dependencies;
5. one complete `ToolResponse` serialization path.

## Fresh-process verification

Because hash randomization is process-scoped, add at most one lightweight fresh-process regression if the repository already has binary/subprocess test helpers. The test should invoke an existing deterministic tool multiple times in separate child processes or MCP sessions and compare compact JSON responses.

Do not create new subprocess infrastructure solely for this check if no helper exists. In that case, type-level `BTreeMap` conversion plus serialization-order tests are sufficient.

Do not run hundreds of repetitions. Two or three fresh processes establish the contract.

## Acceptance criteria

- Tests fail against the prior unordered implementation.
- Tests do not depend on wall-clock timing.
- Tests do not snapshot entire large responses.

---

# Workstream 4 — Correct TOML table extraction

## Current defect

The recursive table extractor records every key before checking whether the associated `toml_edit::Item` is actually a table. Scalar fields under a table can therefore be emitted as table paths.

Example input:

```toml
[package]
name = "eggsact"
version = "1.2.1"

[dependencies]
serde = "1"
```

The table result must include actual tables such as:

```text
package
dependencies
```

It must not include:

```text
package.name
package.version
dependencies.serde
```

## Required contract

Define `tables` as paths for actual TOML tables and arrays of tables.

Handle these item kinds explicitly:

- `Item::Table`: include path and recurse into nested tables;
- `Item::ArrayOfTables`: include the path once; recurse into table contents only if the existing result contract expects nested table paths;
- scalar values: do not include as tables;
- `Item::None`: ignore.

Review how `toml_edit` represents dotted keys and implicit parent tables. Preserve logical table paths, deduplicated and deterministic.

## Source ordering versus lexical ordering

Choose one documented order:

- source order, if `toml_edit` reliably exposes it and existing callers benefit from it; or
- lexical order, if stability and simple testing are more important.

Do not use a hash-set iteration order. If deduplication is needed, track seen values separately while retaining the chosen vector order.

## Required tests

Add table-driven cases for:

1. one table with scalar keys;
2. sibling tables;
3. nested tables;
4. dotted table names;
5. arrays of tables;
6. inline tables used as scalar values;
7. empty input;
8. truncation by `max_tables` after correct table collection.

Each test should assert the exact table list for a small input.

## Summary correction

`toml_shape()` summary and table count must use the actual total table count, not merely the truncated vector length. If existing behavior intentionally reports returned-count rather than total-count, preserve it but make the summary explicit. Prefer:

```text
Valid TOML with N top-level keys and M tables
```

where `M` is the total discovered table count and `truncated` indicates whether the returned list is partial.

## Acceptance criteria

- Scalar key paths are absent from `tables`.
- Actual nested tables and arrays of tables are represented according to the documented contract.
- `max_tables` truncates the correct table list.
- Summary counts are truthful.

---

# Workstream 5 — Correct TOML Unicode positions

## Current defect

The current helper walks bytes and increments the column for each byte. A multibyte UTF-8 character before an error therefore inflates the reported column.

## Required position contract

Inspect the tool schema and existing JSON validation behavior before editing.

Use these definitions unless existing public documentation explicitly requires otherwise:

- `line`: one-based line number;
- `column`: one-based Unicode scalar-value column;
- `position`: zero-based Unicode scalar-value offset from the start of the input.

If `position` is explicitly documented as a byte offset, retain it and correct only line/column. If it is undocumented or intended to match the JSON tool/Python-style character offset, convert it consistently and update schema text.

Do not use grapheme clusters. Parser positions conventionally use Unicode scalar values/code points.

## Required helper behavior

Create or reuse one byte-offset conversion helper that:

- accepts a parser byte offset;
- clamps invalid/out-of-range offsets safely;
- computes line and character column without slicing at a non-boundary;
- treats `\n`, `\r\n`, and lone `\r` as line endings according to existing behavior;
- does not rescan the string more than necessary for one error position.

Prefer reusing an existing text primitive if it already provides tested byte-to-character conversion. Avoid duplicating JSON validation's position logic if it can be shared without coupling unrelated parser error wording.

## Required tests

Add exact assertions for errors after:

1. one two-byte character such as `é`;
2. one three-byte character such as `β` or `中`;
3. one four-byte character such as an emoji;
4. LF line ending;
5. CRLF line ending;
6. lone CR line ending;
7. an offset at end of input;
8. an empty input/parser error if applicable.

Use parser inputs that reliably place the `toml_edit` error span after the multibyte text. Do not assert the complete parser error message unless parity requires it; assert line, column, and position separately.

## Acceptance criteria

- Columns count characters, not bytes.
- No invalid UTF-8 slicing can occur.
- Line-ending behavior is covered.
- Existing normalized error wording remains unchanged except numeric positions.

---

# Workstream 6 — Documentation and schemas

Update existing documents only:

```text
architecture/text-library.md
architecture/tools.md
docs/mcp-tools.md or generated equivalent
architecture/machine-codes.md only if a code changes
```

Document:

- stable ordering of public map output;
- source-order exceptions, if any;
- TOML `tables` semantics;
- TOML position base and unit.

Update input/output schemas where descriptions are ambiguous. Regenerate existing generated docs. Do not create a separate determinism architecture document.

---

# Execution sequence for a smaller implementation agent

1. update local `main` and inspect phase-1 completion;
2. inventory serialized `HashMap` fields;
3. classify each as public or internal-only;
4. change public lexical maps to `BTreeMap` one module at a time;
5. run that module's focused tests after each conversion;
6. inspect tool-boundary JSON construction for arbitrary iteration;
7. add focused determinism tests;
8. correct TOML table extraction;
9. add exact small TOML structure tests;
10. correct TOML Unicode position conversion;
11. add LF/CRLF/Unicode position tests;
12. update schemas and affected documentation;
13. run generated-doc check;
14. run full verification;
15. commit once and fill the completion record.

Do not mix phase-3 dispatch refactoring into map type changes.

---

# Required verification

## Focused

Run the existing test targets covering:

```text
regex validation and finditer serialization
JSON shape
Cargo TOML inspection
TOML validation and shape
MCP/tool response serialization
schema/registry consistency
```

## Full

```bash
cargo fmt --all -- --check
cargo run --locked --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity
cargo test --locked --doc
cargo package --locked
```

Run focused Python parity if TOML/JSON positional behavior is parity-covered. Do not broaden parity scope.

---

# Acceptance checklist

- [x] Public serialized maps have an explicit stable ordering.
- [x] Internal-only hash maps were not mechanically replaced.
- [x] Regex named groups serialize stably.
- [x] JSON shape keys serialize stably.
- [x] Cargo dependency maps serialize stably.
- [x] Representative full tool responses are byte-stable.
- [x] TOML scalar fields are not listed as tables.
- [x] Nested tables are correctly listed.
- [x] Arrays of tables follow the documented contract.
- [x] TOML table truncation and summary counts are truthful.
- [x] TOML Unicode columns count characters rather than bytes.
- [x] LF, CRLF, and lone CR behavior is tested.
- [x] No ordered-map dependency was added.
- [x] Existing values and meaningful array order remain unchanged.
- [x] Generated docs and schemas are current.
- [x] Full verification passes.

---

# Completion record

- **Status:** complete
- **Implementation commit:** 0a3ace9
- **Gap fix commit:** `25c4893455719027cdc889a853039a918611ec65`
- **Public maps converted:** 14 fields across 8 structs (RegexMatch.groupdict, JsonShapeKey.keys, RegexFindIterMatch.group_dict, IdentifierAnalyzeResult.suggestions, DependencySection.dependencies/dev_dependencies/build_dependencies/target_specific, IniValidateResult.keys_by_section, TextHashResult.hashes, TextFingerprintResult.normalization, CommandPolicyConfig.allow_subcommands/deny_subcommands, PatchSummaryResult.line_ranges_by_file) + 3 local HashMaps (char_frequency, files_by_category x2)
- **Internal maps intentionally retained:** bracket validation pairs, JSON comparison key sets, cargo.rs seen_keys, config.rs seen_keys/seen_sections, identifier.rs casefold/norm maps, transform.rs mode_names, shell.rs deny_subcommands lookup, list.rs count_deltas (already serde_json::Map/BTreeMap), patch.rs internal lookups
- **TOML table contract:** only Item::Table and Item::ArrayOfTables included; scalar keys excluded; inline tables excluded; summary reports total table count
- **TOML position contract:** byte_offset_to_line_col counts Unicode characters via char iteration; break check uses `byte_pos + char_len > offset` to avoid overshooting CRLF pairs; bare CR handled per toml_edit convention
- **Focused tests:** 534 unit tests, 55 property tests, 8 new serialization determinism tests, 8 new TOML position tests (including direct byte_offset_to_line_col unit tests), 7 TOML structure tests — all pass
- **Full verification:** fmt ✓, clippy ✓, tests ✓ (skip parity), doc ✓, generate-docs --check ✓
- **Documentation updated:** architecture/text-library.md (determinism contract, TOML contracts, 4 stale HashMap refs), architecture/preflight.md (4 stale HashMap refs), architecture/tools.md (determinism/TOML contract references), architecture/mcp-server.md (determinism contract reference), docs/library-api.md (1 stale HashMap ref)
- **Deferred findings:** none

Do not create a separate evidence-only plan. Record concise closure here.