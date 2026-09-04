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

Latest published version: **1.2.3**. The deterministic utility and cron
corrective lines are closed. The original 80-tool registration order remains an
exact prefix, with the six later utilities in the full profile only.

## Completed line: binary distribution, self-update, and MCP bootstrap

Implementation is complete in the current source line. The closure commit SHA
is recorded after the implementation commit is created.

- Added `release-binaries.yml`, a tag-only workflow with pinned Actions. It
  requires an existing `vX.Y.Z` tag, exact tagged commit, clean checkout, and
  matching crates.io `max_stable_version`; it creates or updates only a draft
  GitHub Release and never publishes, tags, or pushes source.
- Qualified matrix: Linux x86-64 and AArch64 GNU builds with a documented
  glibc 2.17 build floor, macOS Intel and Apple Silicon, and Windows x86-64.
  ARMv7 is installer-recognized but remains Cargo fallback-only pending an
  executable/QEMU or native qualification result.
- Every staged executable is checked with `--version`, `--help`, and the real
  MCP initialize/initialized/tools-list/EOF smoke before SHA-256 generation.
- Added `packaging/install.sh` and `packaging/install.ps1`. They use verified
  binaries first, exact pinned versions when requested, deterministic user or
  system destinations, and Cargo fallback only for unsupported/404 targets.
  Network, checksum, and candidate-version failures are hard errors.
- Added `eggsact update`: crates.io is the stable-version authority, GitHub is
  the exact binary source, checksums and candidate identity are required, and
  replacement is staged. Existing client-owned MCP sessions are not killed or
  restarted.
- Added read-only `eggsact integrate list|detect|zed|codex|claude|cursor|vscode|opencode`
  renderers using resolved executable paths and the current official client
  registration shapes. No generic JSONC/TOML editor or stdio daemon was added.
- Updated README, installation/CLI/release/verification docs, architecture
  references, AGENTS.md, and release/testing skills. Packaging excludes the
  installer directory. The existing 86-tool/profile/schema contracts are
  unchanged.

The first raw-binary release must still supply truthful ARMv7 qualification and
real AArch64 SBC evidence before those claims are made. macOS and Windows
artifacts are intentionally unsigned/unnotarized in this line.

## Future opportunities

1. Evaluate MCP Bundle/official MCP Registry distribution after a raw-binary
   release proves the deployment path; keep it non-blocking.
2. Deepen actual coding-agent use of the existing typed preflight wrappers.
3. Measure high-frequency tool latency only if profiling shows a real need.
4. Revisit schema-detail defaults as model context economics change.
5. Consider YAML only when a concrete workflow justifies its dependency and
   semantic surface.

## Standing non-goals

- Not a general sandbox: classify risk, do not enforce it.
- Not every utility belongs in MCP: admit specification-heavy exact operations,
  not generic DevUtils feature parity.
- No external services or hidden host-state dependencies in deterministic tools.
- No systemd/cron/launchd/SCM lifecycle for the current client-owned stdio
  transport, and no production HTTP transport in this line.
