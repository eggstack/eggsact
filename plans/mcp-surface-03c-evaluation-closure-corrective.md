# MCP Discovery Evaluation Closure Corrective

Status: planned
Priority: P1
Scope: semantic discovery coverage, provider/client direct-vs-discovery evidence, server-instructions A/B, trace/scorer contract, closure evidence; no protocol, registry, transport, or capability redesign

## Objective

Close the remaining evidence gap in the MCP progressive-discovery line without reopening the protocol or discovery architecture.

The runtime implementation is in good shape after `mcp-surface-02d`: stdio era classification follows the current MCP SDK v2 reference behavior, the discovery surface remains a small presentation layer over the existing capability registry, profile/audience policy still authorizes every search/invoke target, and direct mode remains the 1.x compatibility default.

Commit `40222999` also added useful deterministic evaluation infrastructure:

- `src/mcp/discovery_eval.rs` for exact serialized Tool-definition measurements;
- `tests/fixtures/tool_discovery_intents.json` for lexical retrieval fixtures;
- `tests/fixtures/tool_discovery_scenarios.json` for portable end-to-end scenarios;
- `scripts/score-discovery-traces.py` for offline scoring of recorded model/client traces;
- generated registry facts and deterministic retrieval/context gates.

Those are valuable and should remain. The corrective issue is that the roadmap marked `mcp-surface-03` complete before the original completion criteria were actually satisfied. The missing evidence is specifically:

1. semantic retrieval coverage does not yet cover every stable `full` + Model-visible capability;
2. the current portable scenario corpus mixes Model-facing evaluation with HarnessOnly targets, so it cannot directly serve as a valid Model-audience direct-vs-discovery benchmark;
3. no recorded direct-vs-discovery traces from at least one current OpenAI coding/agent model and one current Anthropic coding/agent model are checked in or summarized;
4. no controlled server-instructions with/without evidence is recorded;
5. the offline scorer does not yet enforce the original paired noninferiority/selection gates, and its documented trace field names do not directly match the checked-in scenario fixture (`expected_tool` vs `expected_tools`).

This pass is complete only when the evidence exists. Creating more scaffolding without running the external evaluations is not closure.

## Standing constraints

Preserve all of the following:

- one Rust crate; no workspace split;
- 86 underlying deterministic capabilities and current canonical names;
- profile/audience policy as the authorization boundary;
- `McpSurface::Direct` as the 1.x process/integration default unless a separate compatibility decision explicitly changes it;
- discovery as presentation only, with the five pinned front doors plus `tool_search` / `tool_invoke` where allowed;
- no model-provider SDK in runtime dependencies or ordinary CI;
- no API keys, credentials, provider network calls, or nondeterministic model execution in GitHub merge CI;
- no embeddings/vector store/search service;
- no new telemetry or persistent user tracking;
- current MCP protocol behavior from `02d` untouched unless an independently verified protocol bug is found.

Do not change ranking weights merely to improve a benchmark unless a failing fixture exposes a real vocabulary/semantic problem. Prefer description/alias corrections that make the intended capability distinction clearer to both models and humans.

## Part A — Make semantic fixture coverage honest and complete

### A1. Derive the required positive-coverage set from the registry

In the discovery retrieval regression test, derive the set of stable tools visible under:

```text
profile = full
audience = Model
stability = Stable
```

Do not hard-code the expected count. Registry membership is the source of truth.

Require every tool in that set to be represented by at least one **positive task-oriented semantic fixture** in `tests/fixtures/tool_discovery_intents.json` where it is either the primary target or an explicitly acceptable equivalent.

Exact-name reachability remains a separate useful test, but it does not satisfy semantic coverage.

Add a coverage assertion that fails with the missing canonical names if a future Model-visible stable capability is added without a semantic intent fixture.

### A2. Expand the current 49 positive targets to full stable Model coverage

The current fixture file has strong overlap cases but only 49 positive target tools. Add realistic user-goal phrasing for the remaining stable Model-visible tools.

At minimum:

- one terminology-independent task phrasing for every stable Model-visible tool;
- a second materially different phrasing for overlap-prone or historically ambiguous tools;
- no fixture whose only meaningful cue is the canonical tool name;
- no HarnessOnly/Hidden target counted as positive Model coverage;
- deprecated tools remain migration/negative cases rather than ordinary positive targets when a replacement exists.

Retain explicit hard-negative clusters for patch/edit, shell/command, config/validation, text security/Unicode, text comparison, JSON/structured comparison, repository classification, hash/fingerprint, and deprecated `json_query` vs `json_extract`.

### A3. Keep the retrieval gates, but report coverage separately

Continue enforcing:

- top-1 primary/acceptable accuracy >= 90%;
- top-3 >= 98%;
- top-5 = 100%;
- zero HarnessOnly/Hidden leaks to Model audience;
- zero deprecated-tool wins when a nondeprecated replacement is the intended target.

Also report:

```text
stable Model-visible tools       <derived>
positive semantic targets        <derived>
coverage                         100%
positive intent count            <derived>
negative/containment count       <derived>
```

Do not hide missing capability coverage behind a perfect score on a smaller corpus.

## Part B — Repair the portable end-to-end scenario contract

### B1. Separate Model-facing success scenarios from containment scenarios

`tests/fixtures/tool_discovery_scenarios.json` currently includes targets that are intentionally HarnessOnly for Model audience, including at least:

- `path_scope_check`;
- `shell_split`;
- `patch_apply_check`;
- `unicode_policy_check`.

These are valuable policy tests, but they cannot be counted as expected successful Model-audience discovery tasks.

Refactor the scenario schema so every scenario declares its intended audience/purpose, for example:

```json
{
  "id": "...",
  "audience": "model",
  "kind": "task",
  "user_request": "...",
  "expected_tools": ["..."],
  "success_criteria": "...",
  "search_expected": true
}
```

and containment cases use a distinct kind/audience, for example:

```json
{
  "id": "...",
  "audience": "model",
  "kind": "must_not_expose",
  "forbidden_tools": ["shell_split"]
}
```

A Harness-audience scenario may target HarnessOnly tools, but it must not be mixed into the Model direct-vs-discovery success denominator.

### B2. Preserve a portable Model benchmark of roughly 40–60 tasks

After removing/reclassifying HarnessOnly success cases, keep at least 40 representative Model-audience task scenarios by replacing the removed cases with Model-visible capabilities from under-covered categories.

The Model benchmark should include:

- pinned front-door tasks that should not require search;
- long-tail tasks expected to use `tool_search` then `tool_invoke` in discovery mode;
- overlapping-capability tasks where wrong-tool selection is plausible;
- argument-sensitive tasks where schema detail matters;
- representative math, text, JSON, regex, path, shell-safe, config, patch, repo, dependency, network, encoding, temporal, Markdown, identifier, version, and validation operations.

The same scenario IDs and success criteria must be used for direct and discovery runs.

### B3. Unify fixture and trace field names

The current scenario fixture uses `expected_tools`, while `score-discovery-traces.py` documents/reads `expected_tool` plus `acceptable_tools`.

Choose one canonical trace contract and use it consistently. Prefer plural expected/acceptable sets because several valid scenarios have more than one acceptable capability.

Add strict validation in the scorer for:

- required model/client/mode metadata;
- recognized `direct|discovery` mode;
- scenario id;
- non-empty expected/acceptable tool set for task scenarios;
- selected tool shape;
- integer call/retry/error counters >= 0;
- duplicate/missing scenario ids;
- audience/kind compatibility where the trace carries those fields.

Malformed traces should fail loudly rather than quietly scoring `None` as an expected tool.

## Part C — Make the offline scorer answer the rollout question

### C1. Record the metrics plan 03 originally required

For each task scenario record, at minimum:

```text
id
success
selected_tool
search_calls
tool_invoke_calls
mcp_calls
invalid_arguments
wrong_tool_retries
```

At trace level record:

```text
model exact id/version
provider family
client/harness exact version or commit
mode = direct|discovery
run date
initial_tool_definition_bytes
server_instructions = on|off
```

Do not record credentials, account identifiers, unrelated prompts, local paths, or sensitive environment data.

### C2. Pair direct and discovery traces

Extend `scripts/score-discovery-traces.py` (or add one equally small offline helper) so it can compare a matched direct/discovery pair for the same:

- model;
- client/harness;
- scenario corpus revision;
- server-instructions treatment.

Report at least:

- task success rate;
- correct/acceptable capability selection rate;
- invalid-argument count/rate;
- wrong-tool retry count/rate;
- average search calls;
- average total MCP calls;
- initial Tool-definition bytes;
- discovery/direct initial-context ratio;
- direct → discovery success delta in percentage points.

The tool should identify missing scenario pairs instead of comparing unequal corpora.

### C3. Enforce the intended noninferiority gate in maintainer evaluation

For a model/client pair to support recommending discovery:

- discovery task success must be no worse than direct by more than 2 percentage points aggregated across the Model task corpus;
- correct/acceptable capability selection must not materially regress;
- invalid-argument behavior must not be worse in a way that changes task success or creates repeated retries;
- every Model-visible stable capability must remain reachable through the deterministic coverage gate;
- no supported tested client may reject or mishandle `tool_search` / `tool_invoke` in the evaluated workflow.

The script may return a clear PASS/FAIL summary for these gates, but model-backed traces remain maintainer-run evidence rather than merge-blocking CI.

## Part D — Run actual model/client evaluations

### D1. Required model families

Before this plan can be marked complete, run the same Model task corpus in both direct and discovery modes with at least:

1. one current OpenAI coding/agent model;
2. one current Anthropic coding/agent model.

Use models actually available to the maintainer at execution time. Record exact model identifiers, date, reasoning/thinking configuration where applicable, and client/harness version.

A third smaller/workhorse model is useful but optional.

If credentials/subscriptions/client support are unavailable, do **not** invent results and do **not** mark the plan complete. Leave the plan active and record the blocking evidence requirement in `plans/roadmap.md`.

### D2. Use real supported MCP clients/harnesses

Prefer clients the repository actually expects users to employ, such as current Codex/OpenAI agent tooling and Claude Code/Anthropic agent tooling where practical.

At minimum, each model family must exercise a real MCP client/harness path capable of:

- receiving the direct or discovery `tools/list` surface;
- calling the pinned tools;
- using `tool_search` and `tool_invoke` for long-tail discovery tasks;
- producing a trace that can be normalized into the scorer contract.

Do not build a new provider abstraction or benchmarking framework inside eggsact merely to run this evaluation.

### D3. Commit compact, auditable evidence

Store sanitized trace evidence in a dedicated, obvious location such as:

```text
tests/fixtures/discovery_traces/<date>-<model>-<client>-direct.json
tests/fixtures/discovery_traces/<date>-<model>-<client>-discovery.json
```

or an equivalent small evidence directory if the maintainer prefers not to place nondeterministic evidence under test fixtures.

The files must contain only the scorer fields and scenario ids/metrics needed for auditability; do not commit raw hidden reasoning, credentials, unrelated conversation content, or verbose provider responses.

Document the exact invocation procedure in `docs/verification.md` or a short adjacent README so another maintainer can reproduce the evaluation with their own credentials.

Ordinary CI may syntax-validate committed trace JSON and run the offline scorer, but it must never call provider APIs.

## Part E — Evaluate server instructions separately

### E1. Run a controlled A/B subset

Use a representative subset (roughly 10–15 scenarios, including front-door, long-tail, overlap, and argument-sensitive cases) with:

- current `SERVER_INSTRUCTIONS` enabled;
- server instructions absent/empty.

Run the A/B on both required model families where practical. The purpose is to learn whether the instructions improve tool choice/workflow behavior rather than assume they do.

### E2. Do not add a permanent production flag solely for this experiment

Prefer one of:

- a temporary local patch/build used only for the recorded evaluation;
- a dev-only/test harness path that does not enlarge the normal runtime surface.

Do not add a user-facing `--no-server-instructions` option unless there is an independent product need.

Record:

- task success delta;
- selection/retry observations;
- whether discovery workflow compliance improves;
- exact instruction byte size.

Keep instructions only if they are neutral or beneficial across the tested models. If the evidence is mixed, favor concise model-agnostic instructions and document the decision.

Maintain a <=500 UTF-8 byte budget unless a measured reason justifies otherwise.

## Part F — Rollout decision

### F1. Keep direct mode as the safe default during this pass

Do not change:

- `eggsact --mcp` default surface;
- `Profile::default()`;
- canonical profile membership;
- existing generated client integrations

until the model/client evidence is recorded and reviewed.

### F2. Decide explicitly after evidence

After the required traces are scored, choose one of two valid closure outcomes:

**Outcome A — discovery recommended for selected integrations**

Use only if the noninferiority and client-compatibility gates pass. Any integration switched to discovery must retain an obvious direct-mode escape hatch.

**Outcome B — discovery remains explicit/opt-in**

This is also a valid final product decision if direct remains preferable for 1.x compatibility, a host performs its own deferred loading, or model/client results are mixed. Closure still requires the actual model/client measurements and a concise explanation of why direct remains recommended.

Do not call “no model evidence, therefore keep direct” a completed evaluation. That is the current gap this corrective exists to close.

## Part G — Documentation and roadmap correction

### G1. While this plan is active

Update `plans/roadmap.md` so it no longer states that the entire MCP discovery evaluation line is complete.

Durable wording should distinguish:

- protocol/runtime/discovery implementation: complete;
- deterministic context/retrieval evaluation infrastructure: complete;
- external model/client evidence and instructions A/B: pending under this corrective.

### G2. At closure

Record in `plans/roadmap.md`:

- derived stable Model-visible tool count;
- semantic positive-target coverage count and percentage;
- final positive/negative fixture counts;
- direct/full vs discovery Tool-definition bytes and ratio;
- top-1/top-3/top-5 retrieval results;
- exact OpenAI model/client direct-vs-discovery summary;
- exact Anthropic model/client direct-vs-discovery summary;
- invalid-argument/retry/call-count observations;
- server-instructions A/B result;
- final integration/default decision and rationale;
- confirmation that direct mode remains available;
- confirmation that capability names/count/profile policy were not changed for benchmark convenience.

Only then prune this corrective plan per the repository convention.

## Part H — Focused verification

Before closure, run the full repository verification sequence from `AGENTS.md`:

```text
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features --lib
cargo test --locked --all-features --bins
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo deny check advisories bans licenses sources
cargo package --locked --list
cargo package --locked --verbose
cargo publish --locked --dry-run
```

Focused evidence must also include:

```text
semantic stable-Model coverage        100%
retrieval top-1                       >=90%
retrieval top-3                       >=98%
retrieval top-5                       100%
Model -> HarnessOnly leaks            0
discovery/full direct byte ratio      <=25% (current baseline ~5.44%)
Model task scenarios                  >=40 after containment split
OpenAI direct/discovery pair          recorded + scored
Anthropic direct/discovery pair       recorded + scored
instructions A/B                      recorded
ordinary CI                           green
```

If the current byte baseline changes because descriptions or fixture-driven metadata are improved, record the new measured values rather than preserving `111,911` / `6,088` as magic constants.

## Implementation order

1. Re-read current registry/discovery tests and derive the stable full/Model coverage set.
2. Add the programmatic semantic-coverage assertion.
3. Expand semantic fixtures until every stable Model-visible capability is covered.
4. Reclassify HarnessOnly end-to-end scenarios as containment/Harness cases and restore >=40 Model task scenarios.
5. Unify scenario/trace field names and harden scorer validation.
6. Extend the scorer for paired direct-vs-discovery deltas and gate summaries.
7. Run focused deterministic tests; tune descriptions/aliases only where failures reveal real semantic ambiguity.
8. Run OpenAI direct/discovery evaluation and save sanitized paired traces.
9. Run Anthropic direct/discovery evaluation and save sanitized paired traces.
10. Run server-instructions A/B subset and record the result.
11. Decide whether any generated integrations should recommend discovery or remain direct.
12. Run the full `AGENTS.md` verification gate and ordinary CI.
13. Record measured closure evidence in `plans/roadmap.md`.
14. Prune this corrective plan only after all evidence above exists.

## Completion criteria

This corrective is complete when all of the following are true:

- every stable `full` + Model-visible capability has semantic task-oriented discovery coverage;
- deterministic retrieval/context/policy gates pass on the complete corpus;
- Model task scenarios are cleanly separated from HarnessOnly/containment cases;
- the trace schema and scorer agree and reject malformed/incomparable traces;
- paired direct/discovery traces exist for at least one current OpenAI coding/agent model and one current Anthropic coding/agent model through real relevant clients/harnesses;
- the paired scorer demonstrates and records task-success/selection/argument/retry/call/context results;
- server instructions have been evaluated with a controlled with/without subset;
- the integration/default decision is evidence-backed and documented;
- direct mode remains available and no capability/profile/audience policy was weakened to improve scores;
- the full local verification sequence and ordinary CI pass;
- the roadmap contains the actual measured evidence rather than a statement that evidence can be collected later.

If model/client execution is blocked, stop after the deterministic/scorer preparation, leave this plan `Status: planned` or change it to an explicit blocked status, and keep the roadmap honest. Partial implementation is useful; fabricated closure is not.