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

## Active consolidation line

The next implementation line is a bounded maintenance/consolidation pass based
on the September 2026 architecture review. The repository does **not** need a
broad utility expansion. The goal is to reduce duplicate sources of truth,
restore typed internal composition, tighten the Rust API boundary, and deepen
only the highest-value harness integrations while preserving the existing MCP
surface and lightweight single-crate design.

Execute the plans in this order:

1. [`consolidation-01-typed-composition.md`](consolidation-01-typed-composition.md)
   - make typed deterministic operations the canonical internal composition
     path;
   - eliminate tool-handler-to-tool-handler JSON composition where full
     dispatch policy is not required;
   - split responsibility-dense modules only along real architectural seams;
   - preserve tool names, response contracts, profiles, audiences, machine
     codes, compatibility behavior, and CLI behavior.
2. [`consolidation-02-shared-analysis.md`](consolidation-02-shared-analysis.md)
   - introduce canonical repository facts and patch-analysis representations;
   - remove duplicated ecosystem/path detection across repo tools;
   - parse and classify each unified diff once, then apply separate summary,
     contract, and review policies;
   - add cross-tool differential tests so shared facts cannot drift.
3. [`consolidation-03-api-config-build-surface.md`](consolidation-03-api-config-build-surface.md)
   - document and stage a cleaner supported Rust API hierarchy without a
     breaking 1.x visibility change;
   - add typed dependency-preflight integration and only other typed workflow
     facades justified by concrete consumers;
   - correct YAML inspection so heuristic analysis is not represented as
     parser-backed validation;
   - measure coarse feature-gating value and implement it only if the benefit
     exceeds conditional-compilation/CI complexity;
   - retire stale docs and maintain the existing `json_query` deprecation path.

### Consolidation acceptance criteria

This line is complete when all of the following hold:

- internal composites operate on typed/core results rather than parsing sibling
  `ToolResponse` JSON unless a deliberate registry-policy hop is required;
- repository ecosystem/path facts have one canonical classifier;
- patch parsing and neutral patch facts have one canonical implementation used
  by `patch_summary`, `patch_contract_check`, and `diff_risk_classify`;
- shared-fact differential tests protect against cross-tool semantic drift;
- the recommended Rust library API favors `calc`, typed `text`, `ToolRegistry`,
  and typed workflow APIs rather than raw `tools::*` handlers;
- YAML/config guarantees accurately describe heuristic versus parser-backed
  behavior, without adding a YAML dependency absent a concrete workflow;
- feature gating is either implemented as a small measured set of coarse
  features or explicitly rejected with evidence; per-tool feature matrices are
  not introduced;
- architecture documentation matches the resulting module tree;
- the full verification order in `AGENTS.md` passes, including generated-doc
  checks and relevant packaging/API compile checks;
- no gratuitous new MCP tools, workspace split, parser framework, daemon, or
  general-purpose agent/sandbox functionality is introduced.

Once all three plans ship and verification evidence is recorded, prune their
execution detail from `plans/` and reduce this section to concise shipped-state
evidence per the repository planning convention.

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

## Future opportunities

1. Evaluate MCP Bundle/official MCP Registry distribution after the raw-binary
   release proves the deployment path; keep it non-blocking.
2. Measure high-frequency tool latency only if profiling shows a real need.
3. Revisit schema-detail defaults as model context economics change.
4. Consider first-class YAML only when a concrete workflow justifies its
   dependency and semantic surface.
5. Consider an explicit stateful `ToolRegistry` calculator session only if a
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
