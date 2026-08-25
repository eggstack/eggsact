# eggsact Roadmap

This is the single living planning document. Historical per-phase execution
records were pruned after v1.2.3; they remain retrievable from git history.
For current architecture and workflow facts, read `AGENTS.md`,
`architecture/overview.md`, and `docs/verification.md` first.

## Purpose

eggsact is a deterministic local utility layer for coding agents: a CLI
calculator/utility binary, an MCP stdio server exposing curated tools to
models, and an in-process Rust library that harnesses call directly for
preflight and safety checks. The design goals are unchanged: low-entropy,
machine-checkable operations that run before, during, and after model
reasoning; MCP as one transport adapter over a general deterministic tool
substrate.

## Shipped foundations (through v1.2.3)

| Area | Status |
|------|--------|
| Single-source `ToolSpec` registry (`src/mcp/specs/`), generated listings/schemas/docs | Done |
| Module split: protocol / runtime / registry / schemas / tools by category | Done |
| Stable machine codes + verdicts + structured findings on route-critical tools | Done |
| In-process API: `ToolRegistry`, `ExecutionContext`, budget-aware dispatch | Done |
| Typed preflight wrappers: `EditPreflight`, `CommandPreflight`, `ConfigPreflight`, `PatchApplyCheck`, `TextSecurityInspect` | Done |
| Profiles + exposure/audience model (`full`, `default`, 8 codegg profiles, `human_math`) | Done |
| Edit/command/config preflight workflows, shell policy engine | Done |
| Repo/config/dependency inspectors (Cargo, TOML, dotenv, INI, JSON) | Done |
| Unicode security: confusables, mixed-script, invisible chars, prompt inspection | Done |
| Concurrent MCP stdio with out-of-order responses by id, cooperative cancellation | Done |
| Sync execution pool, budgets, truncation envelopes | Done |
| Generated docs (`architecture/mcp-server.md` profile block, `generated/tool-cards.md`) | Done |
| Golden fixtures, 55 property tests, 12 fuzz targets, MSRV/cargo-deny gates | Done |

## Current release state

Latest published version: **1.2.3** (see `CHANGELOG.md`). Release process is
manual per `docs/release.md`; CI verifies merge correctness only.

## Open opportunities

No committed phases. Candidate directions, in rough value order:

1. **codegg integration depth** — wire more of the typed preflight wrappers
   into actual harness flows rather than exposing them only as MCP tools.
2. **YAML support** — the one deferred inspector format; requires accepting a
   dependency (see Non-Goals history).
3. **Benchmarks** — lightweight latency/counters for high-frequency tools
   (fingerprinting, path scope checks, validate_json) if profiling shows need.
4. **Schema detail ergonomics** — revisit `EGGCALC_MCP_SCHEMA_DETAIL=compact`
   defaults as model contexts evolve.

## Non-goals (standing)

- Not a general sandbox: classify risk, do not enforce it.
- Not every tool model-visible: most value is harness-only.
- No external services: local, deterministic, bounded.
