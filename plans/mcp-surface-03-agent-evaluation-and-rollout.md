# Agent Tool-Selection Evaluation and Discovery Rollout

Status: planned
Priority: P1
Scope: deterministic retrieval evaluation, model-facing tool-selection evaluation, context-budget gates, discovery-surface rollout, documentation drift prevention

## Objective

Measure whether the new discovery surface actually improves agentic tool use before making it the recommended client integration path.

This plan is intentionally an evaluation and rollout gate, not another architecture pass. `mcp-surface-01` should provide current protocol behavior and `mcp-surface-02` should provide an explicit opt-in discovery surface. This plan decides whether that surface is good enough to recommend by default, tunes only the parts that fail measured scenarios, and records objective context/selection evidence.

Do not change existing canonical tool names or remove capabilities to improve benchmark scores. Do not add model-provider SDKs or API keys to ordinary CI. Do not turn the repository into an LLM benchmarking framework.

## Research basis — 2026-09-10

Primary sources reviewed for this plan:

- Anthropic advanced tool use / Tool Search: https://www.anthropic.com/engineering/advanced-tool-use
- MCP server instructions evaluation guidance: https://blog.modelcontextprotocol.io/posts/2025-11-03-using-server-instructions/
- MCP current roadmap / progressive-discovery motivation: https://blog.modelcontextprotocol.io/posts/mcp-roadmap/
- MCP 2026-07-28 release / deterministic cacheable lists: https://blog.modelcontextprotocol.io/posts/2026-07-28/

Relevant findings:

- Tool-definition context cost and tool-selection accuracy should be measured separately. A smaller schema catalog is not successful if it increases search/invocation errors enough to offset the savings.
- Anthropic's published Tool Search evaluation showed both large context savings and improved MCP tool-use accuracy; it also identifies wrong tool selection and wrong parameter generation as common failure modes in overlapping catalogs.
- MCP server instructions can improve workflow selection for some models and not others, so they should be evaluated rather than assumed effective.
- Deterministic, stable tool ordering helps both result caching and upstream prompt caching.

## Success dimensions

Treat the rollout as a multi-objective evaluation across:

1. **Initial context cost** — serialized bytes/definitions presented before any user task.
2. **Discovery recall** — whether a short user intent can retrieve the correct long-tail capability.
3. **Selection precision** — whether adjacent/overlapping tools are ranked in an order that reflects their intended semantics.
4. **Argument usability** — whether search schema detail gives enough information to construct a valid `tool_invoke` call.
5. **End-to-end call overhead** — extra search/invoke calls required for long-tail operations.
6. **Compatibility** — direct mode and existing named profiles remain unchanged.
7. **Safety/policy containment** — no hidden/harness-only capability leaks through discovery.

Do not collapse these into one opaque score.

## Part A — Establish serialized context baselines

### A1. Measure actual Tool payload bytes

Add a dev/test helper that serializes the exact Tool definitions emitted by `tools/list` for at least:

- `full` + Model + direct;
- `default` + Model + direct;
- `codegg_core_min` + Model + direct;
- `full` + Model + discovery.

Record:

- tool count;
- total serialized bytes;
- aggregate description bytes;
- aggregate input-schema bytes;
- aggregate output-schema bytes;
- aggregate metadata bytes where measurable.

Do not add a model-specific tokenizer dependency to CI. Serialized UTF-8 bytes are deterministic and provide a stable regression metric. A separate external evaluation may estimate tokens for representative providers/models.

### A2. Add a context-budget regression test

After the discovery implementation is stable, add a relative gate rather than an arbitrary absolute token number.

Recommended initial acceptance criterion:

- discovery/full-Model `tools/list` serialized Tool definitions are no more than 25% of direct/full-Model Tool-definition bytes;
- discovery mode advertises at most 10 Tool definitions, with a target of 7 from plan 02.

If the measured implementation is materially better, tighten the byte threshold before closure. Avoid a threshold so tight that harmless schema wording changes create maintenance noise.

### A3. Bound on-demand discovery results

Measure `tool_search` response size for:

- summary detail, default limit;
- schema detail, one result;
- schema detail, maximum allowed limit.

Set deterministic output bounds that prevent a single search call from recreating the original 77-tool context dump.

If schema-detail results are too large, reduce the maximum result count or require the caller to ask for schema detail with a lower bound; do not globally strip necessary argument constraints.

## Part B — Build a deterministic retrieval fixture corpus

### B1. Add intent fixtures covering every capability

Create a checked-in fixture such as:

`tests/fixtures/tool_discovery_intents.json`

Each fixture should contain at least:

```json
{
  "intent": "summarize the files and hunk counts in this unified diff",
  "primary": "patch_summary",
  "acceptable": [],
  "category": "patch",
  "kind": "selection"
}
```

Cover every stable tool that is visible to `full` + Model audience with at least two materially different intent phrasings where practical:

- one terminology-close query;
- one task/goal phrasing that does not simply repeat the canonical tool name.

HarnessOnly/Hidden tools should have negative fixtures proving they do not appear for Model audience rather than positive discovery fixtures.

Deprecated tools should have migration fixtures rather than normal selection fixtures.

### B2. Add hard negatives for semantic-overlap clusters

The most valuable fixtures are not exact-name searches. Add explicit competing-tool cases for:

- `edit_preflight` vs `patch_apply_check` vs `patch_summary` vs `patch_contract_check` vs `diff_risk_classify`;
- `command_preflight` vs `shell_split` vs `argv_compare` vs `shell_quote_join`;
- `config_preflight` vs `config_file_inspect` vs `validate_json` / `validate_toml` / dotenv / INI tools;
- `text_security_inspect` vs `text_inspect` vs Unicode/identifier/prompt inspection;
- `text_equal` vs `text_diff_explain` vs line-range comparison;
- `json_compare` vs `structured_data_compare`;
- `repo_manifest_inspect` vs `repo_language_detect` vs `repo_tree_summarize`;
- `text_hash` vs `text_fingerprint`;
- `json_extract` vs deprecated `json_query`.

Fixtures should encode the intended semantic distinction. If the repository cannot explain why one tool should win a scenario, fix the descriptions/contracts rather than gaming the search weights.

### B3. Keep fixtures task-oriented

Do not write a corpus where every intent contains the expected tool's exact canonical name. That tests string matching, not agent discovery.

Prefer realistic coding-agent requests such as:

- “check whether this generated edit can be applied exactly once”;
- “tell me which languages this repo appears to use”;
- “find suspicious invisible characters in this identifier list”;
- “compare these JSON configs while ignoring formatting/key order”.

## Part C — Deterministic retrieval gates

### C1. Add top-k retrieval metrics

Create a lightweight test/helper that runs every fixture through the same production search function and records top-1/top-3/top-5 ranks.

Recommended rollout gates after tuning:

- top-5 recall: 100% for stable model-visible capability fixtures;
- top-3 recall: >= 98%;
- top-1 primary-tool accuracy: >= 90%;
- zero Model-audience HarnessOnly/Hidden leaks;
- zero deprecated-tool wins when a nondeprecated replacement is the fixture target.

Because the search is deterministic, these are non-flaky CI assertions once the fixture corpus is frozen.

If 90% top-1 is unrealistic for legitimate multi-tool intents, mark explicit acceptable alternatives rather than weakening the whole threshold.

### C2. Tune metadata before algorithm complexity

When a fixture fails, apply fixes in this order:

1. correct or sharpen the target/competitor descriptions;
2. add a meaningful alias or tag if the vocabulary gap is real;
3. adjust simple deterministic field weights;
4. only then consider a more sophisticated lexical ranking model.

Do not add embeddings or a vector dependency for a catalog of this size unless the simple approach demonstrably cannot meet the fixture gates.

### C3. Add typo and abbreviation coverage

Add a small separate set for common abbreviation/typo behavior (`toml`, `argv`, `cidr`, `unicode`, etc.) to ensure close-match support helps without dominating normal semantic ranking.

Do not allow fuzzy matching to make unrelated short queries rank arbitrarily.

## Part D — Model-in-the-loop evaluation without provider coupling

### D1. Define portable evaluation scenarios

Create a human-readable or JSON fixture export containing representative end-to-end scenarios with:

- user request;
- available initial Tool definitions for direct and discovery modes;
- expected capability or acceptable capability set;
- success criteria for arguments/results;
- whether a search call is expected.

A target of 40–60 representative scenarios is sufficient for the rollout decision; this is separate from the larger deterministic retrieval corpus.

### D2. Do not put external model APIs in ordinary CI

The repository should provide deterministic fixture export and result-scoring helpers only. Model execution can be performed through external harnesses/clients already used by maintainers.

If a small provider-neutral scorer can consume recorded JSON traces, add it under `scripts/` or a `dev-tools` binary without runtime dependencies. It should score recorded traces, not call remote APIs.

### D3. Test at least two model families when practical

Before switching recommended integration rendering, run the same scenario set with at least:

- one current OpenAI coding/agent model;
- one current Anthropic coding/agent model.

A third smaller/cheaper model is useful because weak-tool-selection models benefit most from reduced choice, but it is not a blocking requirement if unavailable.

Record model version/date and client/harness because tool exposure behavior is host-dependent.

### D4. Compare direct and discovery modes

For each model/harness combination measure:

- task success;
- correct capability chosen;
- invalid-argument rate;
- search calls per successful task;
- total MCP calls per successful task;
- obvious wrong-tool retries;
- initial Tool-definition bytes/tokens;
- any host incompatibility with the generic invoke facade.

Do not require discovery mode to reduce total calls for long-tail operations; one extra search step is expected. The question is whether context/selection gains justify it.

## Part E — Evaluate server instructions separately

Run a small controlled subset with and without eggsact's server instructions.

Keep an instruction only if it improves or preserves behavior across tested models. Good candidate content is limited to relationships not obvious from Tool descriptions, for example:

- prefer workflow-level preflight tools for proposed edits/commands/configs;
- in discovery mode, search for long-tail utilities before generic invocation;
- eggsact tools inspect/compute locally and do not execute commands or touch external state.

Do not use instructions to compensate for a confusing `tool_search` schema or bad ranking.

Set an instruction-size regression budget (for example <= 500 UTF-8 bytes) so this mechanism does not become another context sink.

## Part F — Rollout policy

### F1. Preserve existing direct defaults in 1.x unless compatibility policy is intentionally revised

Do not silently change `Profile::default()` or remove tools from existing profiles. The compatibility policy treats profile visibility reductions as breaking for integrations that depend on them.

The safest 1.x rollout is:

- `eggsact --mcp` continues to mean current direct behavior unless explicitly configured otherwise;
- discovery mode is selectable via the new MCP surface option;
- generated `integrate` instructions may recommend discovery mode after this plan's gates pass;
- direct/full remains documented for clients that implement their own deferred loading or need canonical definitions.

A future 2.0 may make discovery the process default if real usage justifies it. That is not required for this line to succeed.

### F2. Gate `integrate` renderer default on measured results

Switch the six generated client integrations to discovery mode only if all of the following hold:

- deterministic retrieval gates pass;
- discovery Tool-definition bytes are <= 25% of direct/full or better;
- model-in-loop task success is noninferior to direct mode within a small practical margin (target no worse than 2 percentage points aggregated);
- wrong-tool/invalid-argument behavior is not worse and preferably improves;
- no tested client rejects or mishandles the generic discovery facades;
- every stable Model-visible capability remains reachable;
- direct mode remains a simple documented opt-out.

If one host has poor support, render direct mode for that host rather than weakening discovery for all clients.

### F3. Do not optimize for a single frontier model

If a ranking/description change improves one model but damages another, prefer model-agnostic semantics and deterministic retrieval quality. Eggsact is infrastructure, not a prompt package for one provider.

## Part G — Documentation generation and drift prevention

### G1. Generate tool-count/category tables from `ToolSpec`

The current architecture documentation has already shown drift risk around manually maintained category/tool tables as new network/encoding/temporal tools were added.

Extend `generate-docs` so canonical tool counts/category tables that can be derived from the registry are generated or checked from the registry rather than manually copied.

At minimum the following values should have one source of truth:

- total underlying tool count;
- per-category counts;
- per-profile/audience counts;
- discovery pinned/facade advertised count if represented in generated documentation.

Do not generate every prose section of `docs/mcp-tools.md`; only mechanize facts that are demonstrably drift-prone.

### G2. Add discovery metrics to diagnostics only if useful

Consider adding non-sensitive static diagnostics such as active MCP surface and advertised tool count to `--diagnostics`.

Do not add noisy runtime search telemetry or persistent usage tracking. The project remains local and deterministic.

### G3. Update the coding-agent skill

The `.opencode/skills/mcp-tools/SKILL.md` guidance should teach maintainers/agents:

- direct vs discovery surface semantics;
- profile remains capability policy;
- search/invoke may not bypass audience restrictions;
- new tools require at least discovery fixtures or an explicit reason they are HarnessOnly/Hidden;
- description quality is part of the agent API contract.

## Part H — Closure evidence

Before pruning these temporary plan files after implementation, record in `plans/roadmap.md`:

- final discovery advertised tool count;
- direct/full vs discovery serialized Tool-definition bytes and percentage reduction;
- deterministic top-1/top-3/top-5 retrieval scores;
- model/client smoke matrix summary;
- whether `integrate` renderers now recommend discovery;
- any host kept on direct mode and why;
- protocol version support after plan 01;
- confirmation that underlying capability count/names remain intact.

Git history will preserve detailed plan execution; the roadmap should retain only durable outcomes and measurements.

## Implementation order

1. Land plans 01 and 02 with discovery still opt-in.
2. Add exact serialized-context measurement helpers and baseline snapshots.
3. Build the complete deterministic intent corpus, starting with overlap clusters.
4. Tune descriptions/aliases/simple ranking until deterministic gates pass.
5. Add portable end-to-end model evaluation scenarios and trace scoring.
6. Run controlled direct-vs-discovery tests across at least two model families and the supported MCP clients practical for the maintainer.
7. Evaluate server instructions with/without treatment.
8. Decide per-client `integrate` default based on measured compatibility/results.
9. Generate/check drift-prone registry tables, update docs, run full verification.
10. Record closure evidence in the roadmap and prune completed plan files per repository convention.

## Verification

Run the full verification sequence from `AGENTS.md` plus the new deterministic discovery suite.

Required rollout evidence should include a concise table equivalent to:

```text
metric                              direct/full     discovery
advertised tools                    <measured>      <measured>
serialized Tool bytes               <measured>      <measured>
relative initial context            100%            <=25%
retrieval top-1                     n/a             >=90%
retrieval top-3                     n/a             >=98%
retrieval top-5                     n/a             100%
Model->HarnessOnly discovery leaks  n/a             0
stable capability reachability      100%            100%
```

Model-driven success/selection metrics should be recorded with model and client versions but should not become a networked merge-blocking CI job.

## Completion criteria

This plan is complete when discovery mode has deterministic fixture coverage for the full model-visible capability set, meets explicit retrieval and context-size gates, has been exercised end-to-end with more than one model family and relevant MCP clients, preserves every allowed underlying capability, cannot bypass profile/audience policy, and either becomes the recommended generated integration surface on evidence or is explicitly retained as opt-in with the blocking measurements documented.