# MCP Presentation Surface Milestone 03c — Closure Status

Status: blocked

Source implementation plan:

- `plans/implementation/mcp-presentation-surface/003c-evaluation-closure-corrective.md`

Source subsystem roadmap:

- `plans/subsystems/mcp-presentation-surface-roadmap.md#7` (Milestone 03c)

Repository baseline reviewed: `19a400eb9aacab68204b5b18152f93d505d69cba`
(`main`; deterministic `src/`/`tests/`/`scripts/` tree unchanged since the
`40222999` evaluation-preparation commit; closure pass adds docs + planning
records only)

Implementation commits or pull requests:

- `40222999` — deterministic evaluation infrastructure (retained):
  `src/mcp/discovery_eval.rs`, `tests/fixtures/tool_discovery_intents.json`,
  `tests/fixtures/tool_discovery_scenarios.json`,
  `scripts/score-discovery-traces.py`, registry facts, retrieval/context gates.
- Migration: plan moved from
  `plans/mcp-surface-03c-evaluation-closure-corrective.md` to
  `plans/implementation/mcp-presentation-surface/003c-evaluation-closure-corrective.md`
  without rewriting evidence (header maps the legacy path).
- This closure pass (uncommitted at review; committed with this record):
  stale pre-migration plan-path references corrected to the migrated path in
  `docs/verification.md`, `architecture/mcp-server.md`,
  `tests/fixtures/discovery_traces/README.md`; plan header set to
  `blocked` with a 2026-09-30 re-verification note; this closure record.

## 1. Executive finding

The deterministic half of 03c is complete and re-verified on the current
`main`: honest registry-derived semantic coverage (Parts A–B), the strict
plural offline scorer with paired noninferiority gates (Part C), and roadmap
honesty (Part G1) all hold with unchanged measured values. The external-evidence
half (Parts D–F: OpenAI + Anthropic direct/discovery pairs, instructions A/B,
evidence-backed rollout decision) was re-attempted 2026-09-30 and remains
blocked: `codex` 0.159.2 and `claude` 2.1.280 CLIs are present but there is no
evaluation budget or approval for the ~200 model calls the plan requires
(48 Model tasks × 2 modes × 2 families plus the 10–15-scenario instructions
A/B). No traces were fabricated, per the plan's explicit stop condition.

Per `plans/003-planning-process.md` §12 closure rules, a milestone MUST NOT be
marked closed when only internal infrastructure exists (a scorer without
recorded traces) or when closure would depend on fabricated external evidence.
The compliant disposition is therefore **blocked**, not closed or conditionally
closed. Generated integrations stay on direct; discovery stays explicitly
selectable. No future plan is unblocked by this disposition (see §11).

## 2. Requirement-to-evidence matrix

| Requirement (plan part) | Evidence | Result | Notes |
|---|---|---|---|
| A1: derive stable full/Model coverage set from registry, no hard-coded count | `discovery_fixture_retrieval_meets_rollout_gates` derives `stable_model_names` from `tools_for_profile_audience("full", Model)` filtered `Stable`; fails with missing canonical names | pass | Re-verified 2026-09-30: 76 stable tools |
| A2: every stable Model tool has task-oriented fixture; no bare-name, no HarnessOnly-positive, deprecated stays migration/negative | Same test: `positive_targets` from primary+acceptable; asserts `missing.is_empty()`; asserts intent phrasing ≠ bare name (+10 chars); asserts primary Model-visible + Stable; hard-negative clusters retained | pass | 76/76 targets, 88 positive intents, 9 containment, no bare-name fixtures |
| A3: retrieval gates top-1 ≥90%, top-3 ≥98%, top-5 100%, 0 leaks, 0 deprecated wins; coverage reported separately | Same test + `harness_only_targets_rejected_for_model_audience` + `deprecated_hidden_from_ordinary_search` | pass | Measured 2026-09-30: top-1 85/88 (96.59%), top-3 88/88 (100%), top-5 88/88 (100%); 3 top-1 misses all retrieve primary in top-5 (`validation-01`, `config-01`, `config-02`); 0 leaks |
| B1: Model task vs containment split (`audience`/`kind`, plural `expected_tools`, `must_not_expose` with `forbidden_tools`) | `portable_model_evaluation_scenarios_are_well_formed` enforces schema, uniqueness, audience/kind vocab, `expected_tools` non-empty for tasks, `forbidden_tools` + no `expected_tools` for containment, Model tasks must target Model-visible tools | pass | 48 `model`/`task` + 4 `model`/`must_not_expose`; Harness tasks never join Model denominator |
| B2: portable Model benchmark 40–60 tasks after split | Same test asserts `(40..=60).contains(&model_tasks)` and `containment >= 4` | pass | 48 tasks, 4 containment |
| B3: unify fixture/trace field names; strict scorer validation | Scorer `--help` documents canonical plural contract; `tests/fixtures/tool_discovery_scenarios.json` uses `expected_tools`; scorer accepts legacy singular with warning and fails loudly on malformed traces | pass | Verified: `/tmp/bad.json` (`{"bad":1}`) fails with `missing or empty model`; duplicate/missing ids, bad counters, cross-mode mismatches rejected |
| C1: metrics plan (per-task + trace-level fields) | Scorer contract: per-task `id/success/selected_tool/search_calls/tool_invoke_calls/mcp_calls/invalid_arguments/wrong_tool_retries`; header `model/provider/client/mode/run_date/initial_tool_definition_bytes/server_instructions` | pass | No credentials/paths recorded by contract |
| C2: paired direct/discovery deltas, missing-pair detection | `scripts/score-discovery-traces.py --pair` requires same model/provider/client/corpus/instructions, opposite modes, identical scenario ids; reports success/selection/invalid/retry/call/context deltas + byte ratio | pass | Synthetic 48-task pair scores PASS with correct deltas; mismatched corpora refused |
| C3: 2pp noninferiority gates, maintainer-run (not merge CI) | Scorer `gates`: `success_noninferiority_2pp`, `selection_noninferiority_2pp`, `invalid_retry_no_harm`, `discovery_workflow_exercised`; ordinary CI never calls providers | pass | Gates verified on synthetic pair; no model SDK in runtime deps |
| D1–D3: OpenAI + Anthropic direct/discovery pairs, real clients, sanitized committed traces + reproduce procedure | `tests/fixtures/discovery_traces/` contains only `README.md`; no `*-direct.json`/`*-discovery.json` pairs | blocked | Re-attempted 2026-09-30: CLIs present (`codex` 0.159.2, `claude` 2.1.280), no evaluation budget/approval for ~200 calls; nothing fabricated |
| E1–E2: instructions A/B subset (10–15 scenarios, on/off, no permanent flag, ≤500-byte budget) | No A/B recorded; `SERVER_INSTRUCTIONS` measured 264 UTF-8 bytes (within budget) but with/without delta unknown | blocked | Blocked on same evidence access; no `--no-server-instructions` flag added (correct per plan) |
| F1–F2: keep direct default during pass; evidence-backed rollout decision (Outcome A or B with rationale) | Defaults unchanged (`McpSurface::Direct` 1.x default, generated integrations direct); decision explicitly deferred | partial | F1 holds (safe default preserved); F2 cannot be taken without D/E evidence — calling "no evidence, therefore keep direct" a completed evaluation is prohibited by the plan |
| G1: roadmap honesty while active | `docs/verification.md` (Discovery evaluation), `architecture/mcp-server.md` (rollout baseline), `tests/fixtures/discovery_traces/README.md`, registry + subsystem roadmap all distinguish implementation, deterministic infra, and pending external evidence | pass | This pass corrected three stale pre-migration plan-path references to the migrated path |
| G2: record measured closure evidence at closure | This record + plan Progress (2026-09-30) hold the measured numbers; roadmap `G2` rollup awaits D–F evidence | partial | Deterministic numbers recorded; external rollup explicitly outstanding |
| H: full verification gate + focused evidence thresholds | Focused gates green (see §4); full merge gate runs before commit of this closure pass | pass (focused) / pending full-gate confirmation at commit | Thresholds: coverage 100%, top-1 96.59% ≥90%, top-3/5 100%, 0 leaks, byte ratio 5.44% ≤25%, 48 Model tasks ≥40, OpenAI/Anthropic pairs + A/B outstanding |

## 3. Production implementation evidence

No production-code change was required or made in this closure pass — the
correct outcome for a blocked evaluation closure. Landed ownership from the
retained `40222999` preparation (unchanged, re-verified):

- `src/mcp/discovery_eval.rs`: exact serialized Tool-definition measurements
  (`direct_metrics`, `discovery_metrics`, `discovery_ratio`); unit tests pin
  `discovery_ratio("full", Model) <= 0.25`.
- `tests/mcp/test_discovery.rs` (15 tests): advertised counts, byte budget,
  containment, retrieval, scenario well-formedness — all green.
- `tests/fixtures/tool_discovery_intents.json`: 97 fixtures (88 positive
  selection intents incl. second phrasings for overlap-prone tools,
  9 HarnessOnly/Hidden containment), 76/76 stable Model targets.
- `tests/fixtures/tool_discovery_scenarios.json`: 52 scenarios (48 Model
  tasks + 4 Model containment), `audience`/`kind`, plural `expected_tools`.
- `scripts/score-discovery-traces.py`: strict plural contract, `--pair`
  deltas, 2pp gates; no network/model calls.
- `tests/fixtures/discovery_traces/README.md`: maintainer procedure (same 48
  tasks both modes, sanitized fields, 10–15-scenario A/B via temporary local
  build, ≤500-byte budget); still contains no traces (correct — none exist).
- `SERVER_INSTRUCTIONS` (`src/mcp/runtime.rs:143`): 264 UTF-8 bytes, within
  the ≤500-byte budget; no production flag added for the experiment.
- Defaults preserved: `McpSurface::Direct` 1.x default, `Profile::default()`,
  canonical profile membership, generated integrations on direct.

This pass changed only docs/planning prose (three stale path references) plus
the plan header/Progress note and this record. `generate-docs --check` is
clean, confirming no ToolSpec/profile/exposure drift.

## 4. Verification executed

### Commands run

```bash
cargo test --locked --test lib test_discovery
cargo test --locked --test lib discovery_fixture_retrieval_meets_rollout_gates -- --nocapture
cargo test --locked --test lib discovery_metrics_enforce_context_budget -- --nocapture
cargo test --locked --lib discovery -- --nocapture
python3 scripts/score-discovery-traces.py --help
python3 scripts/score-discovery-traces.py /tmp/bad.json        # expect loud failure
python3 scripts/score-discovery-traces.py --pair /tmp/direct.json /tmp/disc.json  # synthetic 48-task pair
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo fmt --all -- --check
# Full merge gate runs before commit/push of this closure pass (see commit message trailer).
```

### Results

- `test_discovery`: 15/15 pass, 0 failed.
- Coverage report (test stderr): `stable Model-visible tools 76`,
  `positive semantic targets 76`, `coverage 100%`,
  `positive intent count 88`, `negative/containment count 9`.
- Temporary measurement harness (removed after run):
  `STABLE_MODEL_COUNT=76`, `TOTAL_MODEL_COUNT=77`,
  `DIRECT_TOOLS=77 DIRECT_BYTES=111911`,
  `DISC_TOOLS=7 DISC_BYTES=6088`, `RATIO=0.0544 (5.44%)`,
  `EVAL=88 TOP1=85 (96.59%) TOP3=88 (100.00%) TOP5=88 (100.00%)`;
  top-1 misses: `validation-01 → validate_json`,
  `config-01 → config_preflight`, `config-02 → config_file_inspect`
  (all primary within top-5; preference for description/alias clarification
  over weight tuning stands — no ranking change made).
- Scorer: `--help` prints canonical plural contract; malformed trace fails
  loudly (`missing or empty model`); synthetic matched 48-task pair reports
  success/selection deltas 0.0pp, byte ratio 0.0544, `verdict PASS` with all
  four gates true; mismatched corpora refused by contract.
- `generate-docs --check`: pass (no registry/profile/exposure drift).
- `cargo fmt --check`: pass.
- Provider probe 2026-09-30: `codex` 0.159.2 and `claude` 2.1.280 present;
  no `OPENAI_*`/`ANTHROPIC_*`/budget env; full ~200-call evaluation not
  started (requires maintainer budget approval); no traces fabricated.
- Full merge gate (`cargo fmt`, `generate-docs --check`, `clippy -D warnings`,
  `cargo test --skip parity --test-threads=4`, `cargo test --doc`) is run as
  part of committing this closure pass; the commit message records the outcome.
  Parity/bench are non-gating per `AGENTS.md` (parity excluded from CI;
  bench maintainer-run).

## 5. Invariant review

- One Rust crate, no workspace split; `Cargo.lock` tracked, `--locked`
  throughout: holds (no dependency change in this pass).
- 86 underlying deterministic capabilities and canonical names: holds
  (`generate-docs --check` clean; discovery baseline still 77 Model-visible
  direct / 7 discovery).
- Profile/audience policy as authorization boundary; search/invoke enforce the
  same rules; facades are not `ToolSpec`s: holds
  (`harness_only_targets_rejected_for_model_audience`,
  `narrow_profile_cannot_escape_through_router` green).
- `McpSurface::Direct` 1.x default unless a separate compatibility decision
  changes it: holds (no default/profile/integration change).
- Discovery presentation-only (five pinned front doors + `tool_search` /
  `tool_invoke` where allowed): holds (`advertises_at_most_seven` green).
- No model-provider SDK in runtime deps or ordinary CI; no keys/credentials/
  provider calls/nondeterministic execution in merge CI: holds (scorer is
  offline; traces absent rather than fabricated).
- No embeddings/vector store/search service; no telemetry/persistent tracking:
  holds (lexical retrieval only; no new data collection).
- Protocol behavior from `02d` untouched: holds (no `src/mcp/protocol.rs` /
  era-classification change in this pass).
- Ranking weights not tuned for the benchmark: holds (3 top-1 misses left
  standing; plan prefers description/alias corrections on real vocabulary
  evidence only).

## 6. Failure and recovery review

Applicable to the deterministic surface re-verified here; no new failure modes
introduced (docs/planning-only diff plus closure record):

- Oversize input/truncation (`limits_applied`): unchanged; search-result
  boundedness pinned by `search_result_size_bounded_for_max_schema_detail`
  (10-schema search <200KB, well under 1MB envelope).
- Duplicate delivery/idempotency: N/A to this pass (no protocol change);
  scenario-id uniqueness enforced (`duplicate scenario id` assertion green).
- Cancellation races / correlated JSON-RPC `id`s: unchanged; concurrent MCP
  responses still correlate by `id` (no runtime change).
- Process restart / stale generation: N/A (no new state).
- Contention/resource release: N/A (no new shared state; measurement harness
  was a removed test file).
- Malformed regex / heuristic YAML / cross-era input: untouched paths; scorer
  malformed-trace loud failure verified (fail-closed, no silent `None` scoring).
- Bounded event/artifact behavior: discovery stays ≤7 advertised tools and
  5.44% bytes; no artifact written (no traces fabricated).

## 7. Migration and compatibility review

- Registration-order prefix, ToolSpec names, schemas, profiles, audiences,
  exposure, machine codes, protocol negotiation: unchanged
  (`generate-docs --check` clean).
- Direct/discovery wire compatibility: unchanged; generated integrations stay
  on direct with discovery explicitly selectable — the safe 1.x posture.
- Rollback: reverting this closure-pass commit restores the prior planning
  prose exactly; deterministic behavior is identical either way (no `src/`
  change).
- Legacy planning path: `plans/archive/roadmap.md` retains the pre-migration
  era; the migrated plan header maps the old flat filename; this pass fixed
  three prose references that still pointed at the old flat path.

## 8. Determinism and bounded-execution review

- Evaluation uses exact serialized Tool definitions (UTF-8 bytes, not model
  tokens); no tokenizer/provider dependency added.
- Retrieval corpus is deterministic lexical search (`search_order_byte_stable`
  green); no clock, TZ db, env, or network in the measured path.
- Fixture counts are registry-derived (76 stable targets), not hard-coded;
  future Model-visible stable additions fail the coverage gate until a semantic
  fixture is added.
- Limits enforced: text 100k / expr 10k / list 10k / pattern 1k / 1M
  request-output envelopes; truncation automatic with `limits_applied`.
- Scorer performs no network or model calls; synthetic-pair verification used
  only local JSON.

## 9. Documentation and operations

- `docs/verification.md` (Discovery evaluation): stale flat plan path →
  migrated path; status wording `remains active` → `remains blocked`
  (matches this disposition).
- `architecture/mcp-server.md` (rollout baseline): stale `mcp-surface-03c`
  shorthand → migrated path.
- `tests/fixtures/discovery_traces/README.md`: stale flat plan path →
  migrated path; procedure (48 tasks both modes, sanitized fields, A/B via
  temporary local build, ≤500-byte budget) unchanged and still correct.
- `plans/implementation/mcp-presentation-surface/003c-evaluation-closure-corrective.md`:
  header `active` → `blocked` with closure link; added Progress (2026-09-30)
  with re-verified numbers and blocker re-confirmation; body retained as the
  authoritative work specification per migration.
- Static guards: `test_discovery` (15 tests) + `discovery_eval` unit tests
  remain the deterministic gates; scorer remains the offline pair gate.
- Operator impact: none — defaults, integrations, and surfaces unchanged.

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| High | No OpenAI direct/discovery pair recorded | Rollout decision cannot be evidence-backed; plan cannot close | Maintainer run: same 48 Model tasks, one current OpenAI coding/agent model via a real supported client, sanitized pair in `tests/fixtures/discovery_traces/`, scored with `--pair` |
| High | No Anthropic direct/discovery pair recorded | Same as above; both families required by plan | Maintainer run: same corpus, one current Anthropic coding/agent model via a real supported client, sanitized pair + `--pair` score |
| High | No server-instructions A/B recorded | Unknown whether instructions help/hurt; ≤500-byte budget unjustified by measurement | Maintainer run: 10–15-scenario subset (front-door, long-tail, overlap, argument-sensitive) with instructions on/off via temporary local build; record deltas + byte size |
| Medium | Rollout decision (Outcome A/B) not taken | Generated integrations correctly stay on direct, but the decision the corrective exists to produce is outstanding | Decide after D/E evidence; retain direct escape hatch for any integration switched to discovery |
| Medium | Roadmap G2 rollup (measured external evidence in roadmap) not written | Roadmap stays honestly "pending" but without the final numbers | Record model/client versions, success/selection/invalid/retry/call/context results, A/B outcome, and rationale once D–F land |
| Low | 3 top-1 retrieval misses (`validation-01`, `config-01`, `config-02`) | None on gates (96.59% ≥90%, top-3/5 100%); indicates overlap-prone phrasing, not a defect | Prefer description/alias clarification on real vocabulary evidence; do not tune ranking weights for the benchmark |
| Info | Full merge-gate wall time (~500s integration corpus) | None on this milestone | Non-gating bench stays maintainer-run per `architecture/performance.md` |

No critical findings (nothing unsafe to merge or operate — the blocked posture
is itself the safe state). No findings require reopening protocol, registry,
transport, or capability scope.

## 11. Roadmap disposition

**Milestone blocked and reason stated.** 03c stays `blocked` on the operational
dependency named in the plan and subsystem roadmap: provider credentials /
evaluation budget for OpenAI + Anthropic direct/discovery pairs and the
instructions A/B (~200 model calls; re-attempted 2026-09-30, CLIs present, no
budget/approval, nothing fabricated).

**No future plan is unblocked by this disposition.** Checked 2026-09-30:

- MCP presentation surface workstream: 01, 02, 02c, 02d are closed hard
  predecessors; 03c is the terminal milestone with no downstream hard
  dependent. The workstream stays `active` (guard + evidence wait), not
  guard-only.
- Deterministic tool substrate: 003–008 closed; workstream is guard-only with
  no open milestone. Nothing depends on 03c external evidence.
- Harness integration and docs: guard-only (generate-docs check, parity
  baseline, context isolation). Independent of 03c.
- Distribution, update, and release: M001–M005 closed (M005 conditionally on
  M005a); M005a (deterministic Windows artifacts) is `planned`/`blocked` on
  release sequencing, not on 03c. It remains implementable alongside the next
  release and is **not** unblocked or blocked by this record — explicitly
  independent.
- No proposed/ready plan lists 03c Parts D–F as a hard or interface
  dependency; the only sequencing constraint 03c imposes is the one it states
  itself: generated integrations stay on direct until the evidence gates pass.

When D–F evidence lands, the required follow-up is a short evidence-review
pass against this record (score pairs, record A/B, take the F2 decision, write
the G2 rollup), not a new corrective — the deterministic gates already hold.
Only then is 03c eligible for `closed` and pruning per the plan.

## 12. Registry updates

- `plans/registry.md`: 03c implementation-plan row `active` → `blocked` with
  closure link `plans/closure/mcp-presentation-surface/003c-status.md`;
  subsystem current-milestone cell notes deterministic prep re-verified
  2026-09-30, external evidence blocked; blocked-work table retains Parts D–F
  with 2026-09-30 re-attempt note; control-points bullet references this
  closure record.
- `plans/subsystems/mcp-presentation-surface-roadmap.md`: Status header stays
  `active; 03c active/blocked`; §4 current state notes 2026-09-30
  re-verification values; §12 milestone-status 03c row links this closure
  record and retains the provider/budget blocker.
- `plans/implementation/mcp-presentation-surface/003c-evaluation-closure-corrective.md`:
  header `active` → `blocked` with closure link; Progress (2026-09-30) added.
  Body retained unwritten per migration (authoritative work specification).
- No other registry/roadmap rows change: substrate stays guard-only,
  harness stays guard-only, distribution M005 (conditionally closed) + M005a
  (planned) untouched. Original closure records (003–008, M005) untouched as
  immutable history.
