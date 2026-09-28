# Deterministic Tool Substrate Milestone 008 — Prompt Wire Compatibility and Layering-Guard Corrective

Status: ready

Repository baseline: 5d57b89d30fdd226da9e99db23d943eaecfd0150

Source roadmap:

- plans/subsystems/deterministic-tool-substrate-roadmap.md#7

Corrects:

- plans/implementation/deterministic-tool-substrate/007-typed-core-ownership-and-adapter-deduplication.md
- plans/closure/deterministic-tool-substrate/007-status.md

Historical dependencies:

- Typed-first composition guard — closed/continuous
- Milestone 007 typed-core ownership and adapter deduplication — closed historical evidence
- Milestones 003–006 Unicode correctness/conformance line — closed historical evidence

Long-term requirements:

- plans/000-long-term-specification.md#2
- plans/000-long-term-specification.md#4.2
- plans/000-long-term-specification.md#5
- plans/000-long-term-specification.md#7
- plans/001-terminology-and-domain-model.md#4
- plans/002-long-term-roadmap.md#phase-0
- plans/003-planning-process.md#7

Applicable ADRs:

- plans/adrs/ADR-0001-planning-conventions-adoption.md

Primary class: invariant

## 1. Objective

Correct two post-closure defects in Milestone 007 without reopening its successful typed-core deduplication:

1. restore the pre-007 MCP/tool wire compatibility of
   prompt_input_inspect.recommended_next_tool, including its historical
   null/string/array cardinality and deterministic recommendation ordering,
   while preserving the public typed PromptInspectResult API and the
   Milestone-007 single semantic implementation; and
2. strengthen the source-architecture guard so the repository-wide claim
   "no tools/* handler calls another tools/* handler" is actually checked
   across every Rust adapter module, rather than only checking the four
   handler edges known when 007 was written.

Milestone 007 remains closed historical evidence. Its closure record is not
rewritten. Milestone 008 supersedes only the closure conclusions that:
- the prompt recommendation projection had no observable compatibility delta;
  and
- the four-edge guard was sufficient evidence for a repository-wide
  zero-handler-composition claim.

No new tool, ToolSpec, profile, audience, schema capability, protocol behavior,
or dependency is introduced.

## 2. Why this corrective is required

A post-007 review found that the implementation is structurally sound but the
closure overstates compatibility and guard coverage.

### 2.1 Prompt recommendation wire behavior narrowed

At the pre-007 baseline
38aa6da3e8745ed4599362e2ecb0837bc20fca90,
src/tools/text.rs::_pi_recommend_next_tool returned serde_json::Value with
three possible shapes:

- no recommendation -> null / absent envelope recommendation;
- one recommendation -> string;
- more than one recommendation -> array of strings.

The adapter built recommendation candidates in this fixed order:

1. HIDDEN_CHAR or BIDI_CONTROL -> text_inspect
2. ANSI_ESCAPE or TERMINAL_CONTROL -> text_transform
3. BASE64_BLOB -> text_inspect
4. HTML_COMMENT or MARKDOWN_LINK -> markdown_structure
5. INSTRUCTION_PHRASE -> text_inspect

Those pushes were not deduplicated. Therefore multi-category inputs could
produce arrays, including repeated text_inspect entries when multiple
conditions independently recommended it.

The same serde_json::Value was used both in the tool result's
recommended_next_tool field and in ToolResponse::with_recommended_next_tool.

After 007, prompt_input_inspect_tool delegates to the typed
PromptInspectResult.recommended_next_tool: Option<String>. The adapter now
projects only null/string and cannot reproduce the historical multi-tool array
shape. It also inherits the typed API's narrower recommendation policy, which
does not recommend text_inspect for BASE64_BLOB or INSTRUCTION_PHRASE.

This matters because the public output schema still explicitly declares:

type: ["string", "array"]

and describes "Recommended follow-up tool(s)." The 007 plan required no
observable tool-contract change, while its closure later documented the loss of
the multi-recommendation array as an intended refinement. That is a closure
mismatch, not a reason to reopen the completed deduplication architecture.

### 2.2 The layering guard is narrower than its invariant name

tests/mcp/test_substrate_007.rs contains
adapter_layering_has_no_handler_to_handler_composition, but the implementation
checks only:

- structured_data_compare -> json_compare
- structured_data_compare -> json_shape_tool
- config_preflight -> toml_shape_tool
- command_preflight -> shell_split
- the 15 deleted adapter-local _pi_* prompt helper names.

That correctly guards the known 007 edges. It does not mechanically prove that
another src/tools/*.rs handler cannot call a different public ToolResponse
handler in the future.

The architecture and roadmap now claim zero handler-to-handler composition
across src/tools. The automated guard should test that invariant generically.

### 2.3 007's incidental JSON correctness fix is not reopened

Milestone 007 also corrected typed json_compare behavior for max_diffs = 0
(BUG-002), with typed and MCP regressions. That was a scope expansion relative
to pure deduplication, but the behavior is independently correct and qualified.
008 must retain it and record it as an accepted historical scope delta; no
revert or redesign is required.

## 3. Why 007 verification did not catch these defects

The 007 verification was strong for code correctness and the four known
composition edges, but it did not compare all adapter wire fields against the
pre-007 implementation.

Specifically:

- the new prompt differential tests asserted the post-refactor behavior rather
  than recording the old adapter's multi-category recommendation outputs;
- no fixture exercised an input producing two or more recommendation classes;
- no fixture asserted recommendation behavior for BASE64_BLOB-only or
  INSTRUCTION_PHRASE-only findings;
- the output schema's string-or-array union was checked indirectly through
  generated-doc stability, but there was no behavioral fixture proving the
  array branch remained reachable;
- the layering test encoded an explicit list of the four known edges, so it
  could not detect a novel handler-to-handler edge with different names.

The corrective must add regressions for both failure modes before changing
production code.

## 4. Invariants that must not regress

- Keep all Milestone-007 typed-core ownership changes. Do not restore the
  deleted adapter-local prompt detector/risk-scoring implementation.
- src/text/inspect_prompt.rs remains the sole semantic owner of prompt
  inspection findings and recommendation classification.
- Preserve PromptInspectResult's public fields and types, including
  recommended_next_tool: Option<String>. Do not add a public struct field or
  make consumers migrate.
- Preserve the 86-tool / 23-category registry, registration order, ToolSpecs,
  input/output schema identities, profiles, audiences, exposure, aliases,
  machine codes, route-critical classification, direct/discovery behavior, and
  protocol eras.
- Preserve the prompt finding-code/severity corrections landed in 007:
  BIDI_CONTROL ownership, TERMINAL_CONTROL info severity, LONG_LINE,
  BASE64_BLOB, cumulative long-line spans, and C0/C1 terminal-control
  classification.
- Preserve the json_compare BUG-002 max_diffs = 0 fix and its regressions.
- Preserve all 003–006 Unicode/security semantics.
- No new runtime dependency is expected or justified.
- No adapter-to-adapter ToolResponse composition may be introduced.
- Deterministic output ordering and exact-input/exact-output behavior remain
  contractual.
- Existing cancellation, limits, truncation, and concurrency semantics remain
  unchanged.
- Do not edit the 007 closure record; 008 provides the corrective trace.

## 5. Scope

### In scope

- empirical/source-backed characterization of the pre-007 prompt recommendation
  result and envelope shapes;
- compatibility fixtures for zero, single, and multi recommendation cases;
- restoration of pre-007 MCP/tool recommendation projection while keeping the
  current typed PromptInspectResult contract intact;
- centralization of recommendation classification under the typed prompt module
  without reintroducing adapter semantic duplication;
- a generic source-architecture guard covering all src/tools/*.rs public
  ToolResponse handlers and all same-file/cross-file handler-call edges;
- self-tests for the guard scanner so its detection capability is itself
  regression-tested;
- documentation/schema wording reconciliation only where current prose
  incorrectly states recommendation cardinality or guard coverage;
- closure evidence, registry, and roadmap reconciliation.

### Explicit non-goals

- no revert of 007 typed-core ownership;
- no revert of 007 prompt finding-code/severity correctness fixes;
- no redesign of PromptInspectResult;
- no new public recommendation DTO;
- no ToolSpec or MCP protocol redesign;
- no schema narrowing to string-only;
- no change to discovery/profile/audience behavior;
- no ConfigFacts/dependency-facts work;
- no transport-neutral catalog/runtime extraction;
- no broad tools/helpers.rs cleanup;
- no general Rust parser dependency solely for the guard;
- no further json_compare behavior change beyond proving the existing BUG-002
  fix remains green.

## 6. Target compatibility behavior

The controlling compatibility target for prompt_input_inspect is the pre-007
MCP/tool adapter at commit
38aa6da3e8745ed4599362e2ecb0837bc20fca90.

The implementation must preserve the current typed API behavior separately.

### 6.1 MCP/tool result and envelope

For the wire surface, recommendation candidates are evaluated in the historical
fixed condition order:

1. HIDDEN_CHAR or BIDI_CONTROL -> text_inspect
2. ANSI_ESCAPE or TERMINAL_CONTROL -> text_transform
3. BASE64_BLOB -> text_inspect
4. HTML_COMMENT or MARKDOWN_LINK -> markdown_structure
5. INSTRUCTION_PHRASE -> text_inspect

Projection:

- 0 candidates -> result recommended_next_tool = null and no envelope
  recommended_next_tool;
- 1 candidate -> JSON string in both result and envelope;
- >1 candidates -> JSON array in both result and envelope.

Candidate order is stable and historical duplicate entries are preserved if
the baseline produces them. Do not silently deduplicate during this corrective.

### 6.2 Typed Rust result

PromptInspectResult.recommended_next_tool remains Option<String> and retains the
behavior present at the 008 repository baseline. 008 must not broaden typed
recommendations for BASE64_BLOB or INSTRUCTION_PHRASE solely to satisfy MCP
compatibility.

The preferred typed recommendation and legacy wire recommendation list are two
compatibility projections over the same finding set. Their distinction must be
explicit in code comments/tests so future consolidation does not accidentally
collapse one contract into the other again.

## 7. Recommended implementation shape

Keep semantic ownership in src/text/inspect_prompt.rs.

Use one small crate-private recommendation classifier/projection facility, for
example an internal PromptRecommendationProjection or equivalent, that can
derive:

- preferred: Option<String> for PromptInspectResult's existing typed contract;
- wire_candidates: Vec<String> for the historical MCP/tool projection.

The exact internal type/name is implementation-local. Requirements:

- it must not be public API;
- it must operate on the already-computed typed findings;
- it must not depend on ToolResponse, MCP registry types, profiles, schemas, or
  serde wire envelopes;
- typed prompt inspection uses preferred;
- prompt_input_inspect_tool obtains wire_candidates through a crate-private
  typed helper and only performs null/string/array serialization;
- the adapter must not duplicate the finding-code-to-recommendation mapping.

Do not add a public field to PromptInspectResult: public-struct field addition
would create unnecessary Rust compatibility risk.

## 8. Ordered work packages

### Work package A — Pin pre-007 recommendation compatibility

Intent:

Make the lost wire contract executable before modifying current code.

Required work:

1. Use the pre-007 baseline
   38aa6da3e8745ed4599362e2ecb0837bc20fca90 as the reference. A temporary
   git worktree or equivalent isolated checkout MAY be used to execute the old
   adapter; do not permanently copy the old semantic implementation into
   current tests.
2. Record exact result and ToolResponse-envelope recommendation values for at
   least:
   - clean text -> no recommendation;
   - hidden/bidi only -> text_inspect string;
   - ANSI/terminal only -> text_transform string;
   - Markdown/HTML only -> markdown_structure string;
   - base64 only -> text_inspect string;
   - instruction phrase only -> text_inspect string;
   - hidden + terminal -> multi-recommendation array in historical order;
   - terminal + markdown -> multi-recommendation array in historical order;
   - hidden + base64 (or another pair targeting the same tool) -> baseline
     duplicate-entry behavior;
   - three recommendation classes -> stable three-element ordering.
3. Add current-tree regression fixtures that assert those exact compatibility
   outputs after the production fix.
4. Add a test that result["recommended_next_tool"] and the ToolResponse envelope
   recommended_next_tool are identical whenever a recommendation exists.
5. Pin the output schema as accepting string or array; generated docs must stay
   stable.

Acceptance evidence:

- closure includes the exact baseline outputs/commands used;
- at least one new test fails on repository baseline 5d57b89 for the intended
  lost array/legacy recommendation reason before the fix;
- tests do not treat the 007 closure prose as a substitute for baseline
  behavior.

### Work package B — Restore wire recommendation projection without typed-API drift

Intent:

Restore MCP compatibility while preserving the successful 007 ownership model.

Required work:

- centralize recommendation classification/projection in
  src/text/inspect_prompt.rs or an equally typed/core-local module;
- keep PromptInspectResult.recommended_next_tool: Option<String> unchanged in
  type and baseline behavior;
- expose only the minimum crate-private helper required by the adapter to obtain
  the historical wire recommendation candidates;
- make prompt_input_inspect_tool serialize candidates as null/string/array per
  Section 6;
- use the exact same serialized Value for the result field and response
  envelope;
- preserve historical fixed ordering and duplicates;
- keep all other prompt fields, findings, risk_score, checks_run, summary,
  machine code, limits, and error handling unchanged.

Acceptance evidence:

- pre-007 recommendation matrix passes;
- current typed prompt tests prove typed preferred recommendation behavior did
  not change;
- src/tools/text.rs contains no finding-code-to-recommendation policy table;
- no _pi_* semantic detector family is restored.

### Work package C — Replace the four-edge guard with a repository-wide handler graph guard

Intent:

Make the automated evidence match the architecture's zero-handler-composition
claim.

Required work:

1. Discover every Rust file directly under src/tools/*.rs used by production.
2. Discover public adapter handler functions by source structure/signature,
   preferably functions returning ToolResponse. The implementation must not
   rely on a manually maintained four-handler list.
3. Build the discovered handler-symbol set.
4. For every discovered handler body, reject unqualified calls to any other
   discovered handler symbol, regardless of whether the callee is in the same
   or a different tools module.
5. Allow:
   - crate::text::*, crate::services::*, calc/core calls;
   - private projection/helper calls;
   - handler definitions themselves;
   - mentions in comments, normal/raw strings, byte strings, and char literals;
   - registry/spec references outside handler bodies.
6. Add scanner self-tests with synthetic source proving it:
   - detects a same-file handler call;
   - detects a cross-file handler call;
   - allows a module-qualified typed-core call with the same final identifier;
   - ignores comments and ordinary strings;
   - ignores raw strings and char literals;
   - does not mistake a longer identifier for a handler call.
7. Keep explicit diagnostics for the four historical 007 edges if useful, but
   the generic discovered graph is the controlling guard.

Prefer a small test-only lexical scanner over a new parser dependency. If a
robust scanner cannot be implemented without unacceptable false
positives/negatives, stop and report rather than adding a parsing dependency
silently.

Acceptance evidence:

- guard enumerates all current src/tools Rust modules and handler definitions;
- zero production handler-to-handler edges are reported;
- synthetic negative tests prove detection works rather than merely passing on
  the current tree;
- reintroducing one historical edge locally makes the generic guard fail.

### Work package D — Documentation and schema reconciliation

Intent:

Make the compatibility contract explicit without rewriting history.

Required work:

- do not modify plans/closure/deterministic-tool-substrate/007-status.md;
- update architecture/tools.md prompt_input_inspect documentation to state:
  - typed PromptInspectResult exposes one preferred recommendation;
  - MCP/tool compatibility projection may be null, string, or ordered array;
  - result and response envelope use the same compatibility value;
- document that zero handler-to-handler composition is enforced by a
  repository-wide discovered handler graph guard, not only four named edges;
- update architecture/text-library.md only if needed to distinguish typed
  preferred recommendation from MCP compatibility projection;
- keep src/mcp/schemas/text.rs prompt_input_inspect_output string-or-array
  contract unchanged unless only a description clarification is required;
- run generate-docs --check; no ToolSpec/profile/exposure diff is expected;
- add a CHANGELOG corrective note if repo convention records internal
  compatibility corrections.

Acceptance evidence:

- code, schema, architecture, and tests agree on cardinality;
- no historical closure file is rewritten;
- generated registration/profile material is unchanged.

### Work package E — Qualify 007 scope deltas and close 008

Intent:

Finish the corrective without reopening unrelated 007 behavior.

Required evidence:

- run the json_compare max_diffs = 0 typed and MCP regressions to prove the
  incidental 007 BUG-002 fix remains intact;
- record it in 008 closure as an accepted historical scope delta requiring no
  further corrective work;
- run focused prompt and layering suites;
- run registry sync and generated-doc checks;
- run the ordered merge gate;
- run parity when ../eggcalc is available;
- record remote ordinary CI;
- create plans/closure/deterministic-tool-substrate/008-status.md;
- return the substrate roadmap/registry to guard-only only after closure.

## 9. Failure, cancellation, truncation, and contention semantics

No execution-resource semantics change is intended.

- Prompt inspection input/list bounds remain adapter-enforced as in 007.
- Recommendation projection is O(number of retained findings) with fixed
  category checks and bounded by the existing MAX_FINDINGS limit.
- No new network, clock, locale, environment, or filesystem dependency enters
  production code.
- Test-only source scanning may read repository source files; production code
  must not.
- No locks or mutable global state are introduced.
- Findings truncation occurs before recommendation projection exactly as at the
  typed-core boundary today unless baseline evidence proves otherwise.
- Result/envelope recommendation serialization must be deterministic.

## 10. Compatibility and migration

No consumer migration is expected.

The intended compatibility result is:

- MCP/tool clients regain the pre-007 null/string/array behavior already
  permitted by the schema;
- Rust users of PromptInspectResult keep the current Option<String> API and
  behavior;
- no tool name, field name, schema type, machine code, profile, audience, or
  protocol changes;
- no recommendation array is introduced into the typed Rust DTO.

This is a restoration of the pre-007 wire behavior, not a new feature.

## 11. Required focused tests

Run focused checks first:

~~~bash
cargo test --locked --test lib substrate_008
cargo test --locked --test lib prompt_input_inspect
cargo test --locked --test lib adapter_layering
cargo test --locked --test lib test_inspect_prompt
cargo test --locked --test lib test_tool_coverage
cargo test --locked --test lib tool_registration_tables_are_in_sync
cargo test --locked --test lib json_compare
cargo test --locked --test lib structured_data_compare
~~~

If test module/filter names differ after implementation, use the narrowest
equivalent filters and record the actual commands in closure.

The focused suite must include:
- zero/single/multi prompt recommendation compatibility;
- result/envelope recommendation equality;
- typed preferred-recommendation non-regression;
- generic guard synthetic positives/negatives;
- a live-tree zero-edge scan;
- BUG-002 max_diffs = 0 regression.

## 12. Required broad verification

Run the repository merge gate in order:

~~~bash
cargo fmt --all -- --check
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
~~~

When ../eggcalc is available:

~~~bash
cargo test --locked --test lib parity -- --nocapture
~~~

Do not modify tests/fixtures/accepted_parity_failures.txt merely to close 008.

No dependency change is expected. If Cargo.toml or Cargo.lock changes, stop and
justify it before continuing; cargo-deny and full dependency qualification then
become required.

## 13. Static guard contract

At closure, the controlling architecture guard must establish:

For every production public ToolResponse handler H in src/tools/*.rs,
the body of H contains no unqualified call to another production public
ToolResponse handler G.

This covers same-module and cross-module composition.

A manually maintained denylist of known pairs is insufficient as the sole
guard. Explicit known-edge assertions may remain only as supplemental
diagnostics.

The guard implementation must itself have negative tests demonstrating that it
would fail on representative forbidden source.

## 14. Acceptance criteria

Milestone 008 may close only when:

- pre-007 prompt recommendation wire outputs are recorded from the historical
  baseline or equivalently proven from executable/source evidence;
- current prompt_input_inspect restores the historical null/string/array result
  cardinality;
- the ToolResponse envelope uses the same recommendation Value as the result;
- historical recommendation ordering is preserved;
- historical duplicate candidate behavior is preserved where baseline evidence
  shows it;
- BASE64_BLOB-only and INSTRUCTION_PHRASE-only recommendation behavior matches
  pre-007 MCP behavior;
- PromptInspectResult remains API-compatible and its current typed
  recommended_next_tool behavior is unchanged;
- prompt recommendation mapping is not duplicated in src/tools/text.rs;
- the generic architecture guard discovers all current public ToolResponse
  handlers across src/tools/*.rs and reports zero handler-to-handler call
  edges;
- guard self-tests prove same-file/cross-file detection and lexical exclusions;
- the four historical 007 edges remain absent;
- 007's BUG-002 max_diffs = 0 fix remains green and is not reverted;
- no ToolSpec, registry order, schema capability, profile, audience, exposure,
  machine code, protocol, dependency, or MSRV change occurs;
- focused tests, full merge gate, parity when available, and remote CI pass;
- plans/closure/deterministic-tool-substrate/008-status.md records all evidence.

## 15. Stop conditions

Stop and report rather than expanding scope when:

- empirical baseline execution contradicts the pre-007 source-derived
  recommendation behavior described in Section 6;
- restoring recommendation arrays requires changing public
  PromptInspectResult or ToolSpec/schema capability;
- a generic guard discovers a real handler-to-handler edge outside the four
  historical sites whose removal requires a new typed service or broader
  architecture;
- reliable source scanning would require adding a production/parser dependency;
- restoring wire behavior changes prompt finding codes, severities, risk
  scoring, machine codes, or other 007 correctness fixes;
- generated ToolSpec/profile material changes;
- parity shows a non-listed regression outside the recommendation field;
- repository changes after baseline 5d57b89 materially invalidate these
  findings.

A newly discovered handler edge that has an obvious existing typed-core
replacement MAY be corrected inside 008 if the change is local and
behavior-preserving. Otherwise register a separate successor rather than
broadening this corrective.

## 16. Closure evidence required

The 008 closure record must contain:

- implementation commit(s);
- repository baseline 5d57b89;
- pre-007 compatibility reference commit 38aa6da;
- exact baseline prompt recommendation matrix, including result and envelope;
- before/after failing fixture evidence for multi-recommendation cardinality;
- proof PromptInspectResult public shape and typed behavior did not change;
- proof output schema still accepts string or array;
- handler inventory count and zero-edge graph result;
- generic guard synthetic self-test results;
- explicit four historical edge absence;
- BUG-002 max_diffs = 0 regression outcome;
- generated-doc/registry-sync evidence;
- focused suite results;
- full merge-gate results;
- parity outcome or explicit unavailability;
- remote CI run ID;
- dependency/MSRV status;
- residual findings by severity;
- recommendation: closed, conditionally closed, corrective pass required, or
  blocked.

## 17. Handoff notes

Start with Work Package A. Do not change recommendation code until the
pre-007 wire matrix is recorded.

Keep the successful 007 architecture: typed findings are authoritative and the
adapter is a projection boundary. The corrective is specifically about
preserving two legacy projections from the same findings:
- the typed Rust API's one preferred recommendation; and
- the MCP/tool wire's historical zero/one/many recommendation value.

For the architecture guard, make the test prove its own detection behavior.
A green scan of current source without synthetic failing examples is
insufficient evidence.

Do not touch MCP discovery 03c, distribution M005, Unicode 003–006, or the
ADR-gated transport-neutral catalog/runtime line.
