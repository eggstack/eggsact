# Discovery Trace Evidence

Sanitized direct-vs-discovery traces from real model/client runs live here.
No traces are checked in yet; the deterministic retrieval/context gates pass,
but the external model/client evidence required by
`plans/mcp-surface-03c-evaluation-closure-corrective.md` Parts D/E is still
pending.

## File naming

```text
<YYYY-MM-DD>-<provider>-<model>-<client>-direct.json
<YYYY-MM-DD>-<provider>-<model>-<client>-discovery.json
```

Example: `2026-09-15-openai-gpt-5.2-codex-codex-cli-direct.json`.

## Trace contract

Use the canonical plural form (`expected_tools`, `acceptable_tools`) matching
`../tool_discovery_scenarios.json`. Required header: `model`, `provider`,
`client`, `mode`, `run_date`, `initial_tool_definition_bytes`,
`server_instructions`. Required per task: `id`, `expected_tools`,
`success`, `selected_tool`, `search_calls`, `tool_invoke_calls`,
`mcp_calls`, `invalid_arguments`, `wrong_tool_retries`. Legacy singular
`expected_tool` loads with a warning; prefer plural. See
`scripts/score-discovery-traces.py --help` for the full contract.

Record only scorer fields and scenario ids/metrics. Do not commit raw hidden
reasoning, credentials, account identifiers, unrelated conversation content,
verbose provider responses, local paths, or sensitive environment data.

## Procedure (maintainer-run, never CI)

1. Build the binary: `cargo build --locked --release`.
2. For each required family (at least one current OpenAI coding/agent model
   and one current Anthropic coding/agent model) through a real supported
   MCP client (e.g. Codex/OpenAI tooling, Claude Code/Anthropic tooling):
   - Run the same Model task corpus from
     `../tool_discovery_scenarios.json` (`audience=model`, `kind=task`,
     currently 48 tasks) once with `eggsact --mcp` (direct) and once with
     `eggsact --mcp --mcp-surface discovery`.
   - Use identical scenario IDs and success criteria for both modes.
   - Capture per scenario: success, selected tool, search/invoke/MCP call
     counts, invalid-argument and wrong-tool retry counts.
   - Record exact model id/version, provider family, client/harness exact
     version or commit, run date, initial Tool-definition bytes for each
     mode, and `server_instructions=on|off`.
3. Save sanitized paired traces here following the naming above.
4. Score offline with no network access:

```bash
python3 scripts/score-discovery-traces.py --pair \
  tests/fixtures/discovery_traces/<date>-<model>-<client>-direct.json \
  tests/fixtures/discovery_traces/<date>-<model>-<client>-discovery.json
```

The pair must share model, provider, client, corpus revision, and
instructions treatment with opposite modes and identical scenario IDs;
otherwise the scorer refuses to compare unequal corpora. Gates: discovery
success within 2pp of direct, selection within 2pp, no harmful
invalid/retry regression, discovery workflow actually exercised
(search + invoke observed).

5. For server instructions A/B (roughly 10–15 scenarios: front-door,
   long-tail, overlap, argument-sensitive), repeat a subset with current
   `SERVER_INSTRUCTIONS` enabled vs absent/empty via a temporary local
   build only. Do not add a production `--no-server-instructions` flag.
   Record success/selection/retry deltas, workflow-compliance notes, and
   exact instruction byte size (budget <=500 UTF-8 bytes unless measured
   reason justifies otherwise).

Ordinary CI never calls provider APIs; it may syntax-validate committed
trace JSON and run the offline scorer. See `docs/verification.md` for the
full evaluation procedure.
