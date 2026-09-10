# eggsact Roadmap

This is the single living planning document. Completed execution detail is
pruned once a line ships; git history retains the prior plan and evidence.
For current facts, read `AGENTS.md`, `architecture/overview.md`, and
`docs/verification.md` first.

## Purpose

eggsact is a deterministic local utility layer for coding agents: a CLI
calculator/utility binary, an MCP stdio server exposing curated tools, and an
in-process Rust library for harnesses. Keep it lightweight, bounded, local,
and exact-input/exact-output. MCP is a transport adapter over the deterministic
tool substrate, not a reason to accumulate unrelated agent features.

## Shipped foundations

- Single-crate Rust implementation with a single-source `ToolSpec` registry.
- 86 tools across 23 categories, with profile/audience/exposure filtering.
- Deterministic math, text, JSON, regex, path, shell, config, patch, repo,
  dependency, network, encoding, and fixed-offset temporal utilities.
- In-process `ToolRegistry` / `ExecutionContext` APIs and typed preflight
  wrappers for coding-agent harness integration.
- Stable machine codes, structured findings/verdicts, bounded execution,
  cooperative cancellation, truncation, and concurrent MCP stdio dispatch.
- Generated documentation, property tests, fuzz targets, MSRV/cargo-deny
  policy, and a manual release gate.

## Current release state

Latest published version: **1.2.4**. The deterministic utility, cron, and
binary-distribution corrective lines are closed. The original 80-tool
registration order remains an exact prefix, with the six later utilities in
the full profile only.

The first binary-bearing GitHub Release is published at
[`v1.2.4`](https://github.com/eggstack/eggsact/releases/tag/v1.2.4). Its five
qualified target binaries, SHA-256 sidecars, `install.sh`, and `install.ps1`
were produced by successful workflow
[`33944943782`](https://github.com/eggstack/eggsact/actions/runs/33944943782).

## Shipped consolidation line

The September 2026 maintenance/consolidation pass is complete. The repository
remains a single-crate, 86-tool design with no broad utility expansion. The
three consolidation plans have been pruned per the planning convention; git
history retains their execution detail and evidence.

- **Typed-first composition** (`consolidation-01`): `src/services/`
  (fingerprint, newline, security, repo, patch analysis) over `text/`/`calc`
  cores; `tools/*` adapters parse input, call typed cores/services, and build
  the wire shape once at the boundary. No adapter-to-adapter JSON composition
  except three intentional same-module reuses documented in
  `architecture/tools.md`.
- **Shared analysis** (`consolidation-02`): `RepoFacts` is the canonical
  ecosystem/path/language classifier; `PatchAnalysis` parses each unified
  diff once into neutral facts; differential tests guard against cross-tool
  semantic drift.
- **API/config/build surface** (`consolidation-03`):
  - recommended Rust hierarchy (`calc`/root → typed `text` →
    `ToolRegistry`/execution contexts → typed `preflight` → MCP server) is
    documented in `docs/library-api.md` and `architecture/overview.md`; raw
    `tools::*` handlers stay `pub` for 1.x compatibility but are documented
    as adapter internals with no visibility reduction in 1.x;
  - typed `DependencyPreflight` added (ecosystem, added/removed/version/
    source changes, hook changes, findings, verdict, machine code);
    patch-review and repo-audit facades were evaluated and declined (their
    projections already answer those workflows through `ToolRegistry`);
  - `config_file_inspect` reports an additive `analysis_mode`
    (`"parser"` vs `"heuristic"`); YAML stays heuristic-only with no parser
    dependency and `config_preflight` intentionally excludes YAML;
  - coarse feature gating was measured and explicitly declined: 20 direct /
    88 total dependency crates, 7.4 MiB stripped release binary, ~58s
    warm-cache release build; tokio/serde_json/toml/regex/unicode span all
    layers and the shipped binary stays full-featured, so cfg-gating would
    add CI-matrix and conditional-compilation cost for no binary win;
  - architecture drift corrected (stale `mcp::tools` wording, wrapper
    counts); `sync_pool.rs` references verified current; no lightweight
    path-reference check was added (the real drift was module-path
    vocabulary, which a file-path checker would not catch);
  - `json_query` stays deprecated-but-compatible in 1.x with no promotion in
    default docs; calculator context isolation semantics are unchanged.

## Binary distribution closure

The C8 Zig bootstrap correction is implemented in `6658702`. The release
workflow now extracts each pinned Zig 0.14.1 archive into a fixed directory,
strips the archive wrapper directory, and uses that path consistently for
`GITHUB_PATH` and `zig version`. `scripts/check-release-contract.py` guards
the invariant. Follow-up fixes discovered only by real release execution
added a crates.io user agent, accepted macOS's `arm64` architecture spelling,
and use `shasum -a 256` when `sha256sum` is unavailable.

The published matrix is:

| Host | Target | Qualification |
|---|---|---|
| Linux x86-64 | `x86_64-unknown-linux-gnu` | staged version/help/MCP smoke; glibc 2.17 floor |
| Linux AArch64 | `aarch64-unknown-linux-gnu` | native `ubuntu-24.04-arm` build and executable smoke; glibc 2.17 floor |
| macOS Intel | `x86_64-apple-darwin` | native staged smoke |
| macOS Apple Silicon | `aarch64-apple-darwin` | native staged smoke |
| Windows x86-64 | `x86_64-pc-windows-msvc` | native staged smoke |

Zig 0.14.1 and cargo-zigbuild 0.23.3 remain release-only tooling. ARMv7 is
recognized by the Unix installer but remains Cargo fallback/source-only until
its own executable, QEMU, or native qualification gate exists. Windows
installer parsing passed; Windows deferred self-update behavior was not
executed on this Linux host and remains documented as staged replacement.

The exact-tag Unix installer was run against v1.2.4 and installed a candidate
reporting `eggsact 1.2.4`. The exact-tag and
`releases/latest/download/install.sh` payloads were both fetched after
publication and matched. No runtime dependency or release-only tool was
added; the release assets are stripped standalone binaries (approximately
7.2–10.9 MiB).

The local release gate passed for 1.2.4 before publication. Ordinary CI passed
on the release-preparation and corrective commits, including runs
`33941151181`, `33941872259`, `33942657807`, `33943491822`, and
`33944382758`. The final binary workflow completed all target,
installer, checksum, smoke, and draft-assembly jobs in `33944943782`.

## Active MCP surface modernization line

Research on 2026-09-10 found that eggsact's internal capability architecture is
already consolidated, but its ordinary MCP presentation remains much broader
than current agent-tool guidance recommends: the `full` Model audience exposes
77 canonical tools and schema detail defaults to `full`. Current MCP and model
platform work increasingly treats large always-loaded tool catalogs as a
selection/context-scaling problem.

The implementation goal is **not** to remove or merge the 86 deterministic
capabilities. It is to keep the capability registry intact while making the
ordinary agent-facing surface much smaller, searchable, and protocol-current.

The first two implementation plans have now landed on `main`:

- **Protocol modernization** (`mcp-surface-01`, commit `35f7dc2e`): added the
  `2026-07-28` modern stateless request envelope alongside the legacy
  initialize-era revisions, `server/discover`, modern cache/result metadata,
  standard Tool annotations, and schema-conforming `structuredContent` while
  reusing existing registry/validation/execution paths.
- **Progressive discovery** (`mcp-surface-02`, commit `a5c00b8d`): added
  `McpSurface::{Direct, Discovery}`, five profile-filtered pinned front doors,
  and MCP-only deterministic `tool_search` / `tool_invoke` facades. Direct
  remains the default. Discovery remains presentation-only: profile/audience
  policy is still enforced by `ToolRegistry` and the normal bounded execution
  path. Ordinary CI for `a5c00b8d` passed.

Post-implementation audit found two corrective items before agent evaluation
should be treated as authoritative rollout evidence:

1. **`mcp-surface-02c-protocol-contract-corrective.md` — P1.** Pin one MCP
   protocol era per stdio connection rather than permitting the same long-lived
   process to select legacy vs modern independently per request. Keep modern
   request metadata request-scoped, but add a race-safe connection-era state
   (`Undecided` → `Legacy` or `Modern20260728`) and reject cross-era switching
   after the opening exchange. The same corrective pass removes or neutralizes
   the dormant `tool_invoke_output_schema()` contract: a generic router returns
   target-specific structured results and should not advertise a narrow
   facade-level schema that those results cannot satisfy. This pass lands
   **before** plan 03.
2. **`mcp-surface-03-agent-evaluation-and-rollout.md` — P1.** After corrective
   closure, measure exact serialized Tool-definition cost, build deterministic
   full-capability retrieval fixtures and hard-negative overlap cases, run
   portable direct-vs-discovery agent evaluations, and gate any generated
   client integration default on measured context reduction, selection quality,
   reachability, and host compatibility.

The corrective pass must not broaden the line into a second MCP rewrite. It
should preserve stdio-only transport, existing tool names/profile membership,
the seven-or-fewer `full`/Model discovery definitions, direct-mode default,
existing budget/cancellation behavior, and zero new runtime dependencies. Its
focused acceptance evidence includes mixed-era rejection, deterministic
opening-era selection under races, no poisoned connection state after invalid
opening attempts, target-diverse `tool_invoke` modern results, and confirmation
that no generic invoke output schema is advertised.

Plan 03 remains the rollout gate. The present discovery tests already prove
exact-name reachability, profile/audience containment, deterministic ordering,
and a substantial context reduction, but they do **not** yet establish the
planned natural-language top-1/top-3/top-5 retrieval targets or cross-model
agent success. Do not change generated client integrations to prefer discovery
until those measurements pass. The rollout target remains discovery
Tool-definition bytes <=25% of direct/full where practical, top-5 retrieval
coverage for all stable Model-visible capability fixtures, and noninferior
end-to-end agent success across more than one model family/client path.

Research sources for the line include the MCP 2026-07-28 release and current
roadmap, the current MCP Tools rule forbidding per-connection/side-effect-driven
tool-list mutation, official MCP SDK migration/stdio version guidance, and
Anthropic's Tool Search measurements showing large context and tool-selection
gains from on-demand discovery. Detailed URLs and constraints remain in the
plan files so the handoff does not depend on this roadmap summary remaining
current.

Compatibility posture for 1.x: do not remove tools from existing profiles, do
not rename canonical tools, and do not silently make `Profile::default()` a
narrow profile. Discovery remains explicit/opt-in during corrective and
evaluation work. Generated `integrate` instructions may recommend it only after
the evaluation plan's rollout gates pass. Direct/full remains the escape hatch
for hosts that already perform their own deferred tool loading or require
canonical first-class tool definitions.

## Future opportunities

1. Evaluate MCP Bundle/official MCP Registry distribution after the raw-binary
   release proves the deployment path; keep it non-blocking.
2. Measure high-frequency tool latency only if profiling shows a real need.
3. Consider first-class YAML only when a concrete workflow justifies its
   dependency and semantic surface.
4. Consider an explicit stateful `ToolRegistry` calculator session only if a
   real consumer needs persistent PRNG/memory/variable state; do not change
   isolated `ExecutionContext` semantics by default.

## Standing non-goals

- Not a general sandbox: classify risk, do not enforce it.
- Not every utility belongs in MCP: admit specification-heavy exact operations,
  not generic DevUtils feature parity.
- No systemd, launchd, Windows SCM, cron, PID files, restart command, or
  background daemon for the client-owned stdio server.
- No automatic crates.io publishing or tag creation in GitHub Actions.
- No apt/deb/rpm, Homebrew, winget, Chocolatey, MSI, container distribution,
  code-signing/notarization infrastructure, or Windows ARM64 release without a
  separate qualification decision.