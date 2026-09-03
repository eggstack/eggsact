# eggsact Roadmap

This is the single living planning document. Completed execution detail is
pruned once a line ships; git history retains the prior plans and evidence.
For current architecture and workflow facts, read `AGENTS.md`,
`architecture/overview.md`, and `docs/verification.md` first.

## Purpose

eggsact is a deterministic local utility layer for coding agents: a CLI
calculator/utility binary, an MCP stdio server exposing curated tools to
models, and an in-process Rust library that harnesses call directly for
preflight and safety checks. Keep the project lightweight, bounded, local,
and exact-input/exact-output. MCP is a transport adapter over the deterministic
tool substrate, not a reason to accumulate unrelated agent features.

## Shipped foundations

- Single-crate Rust implementation with a single-source `ToolSpec` registry.
- 86 tools across 23 categories, with profile/audience/exposure filtering.
- Deterministic math, text, JSON, regex, path, shell, config, patch, repo,
  dependency, network, encoding, and fixed-offset temporal utilities.
- In-process `ToolRegistry` / `ExecutionContext` APIs and typed preflight
  wrappers for codegg-style harness integration.
- Stable machine codes, structured findings/verdicts, bounded execution,
  cooperative cancellation, truncation, and concurrent MCP stdio dispatch.
- Generated tool/profile documentation, property tests, fuzz targets,
  MSRV/cargo-deny policy, and a manual release gate.

## Current release state

Latest published version: **1.2.3**. Current main contains the six deterministic
utility additions introduced after 1.2.3:

- `ip_inspect`
- `cidr_inspect`
- `codec_convert`
- `radix_convert`
- `datetime_convert`
- `cron_inspect`

The expansion kept the original 80-tool registration order as an exact prefix,
added only `base64` and `time` as runtime dependencies, and increased the
stripped release binary from 8,051,496 to 8,229,476 bytes (+177,980; +2.21%).
The final pre-corrective closure CI run passed.

The utility line is **reopened for one corrective closure pass** because review
found three correctness defects in newly-added behavior. Do not add features or
new dependencies during this pass.

---

# Completed corrective closure

The deterministic utility correctness line is closed in `ae2be1d`, with a
follow-up parity-envelope correction:

- IPv6 CIDR counts are exact and prefix-only; mapped IPv6 detection is limited
  to `::ffff:0:0/96`; cron DOM/DOW matching preserves syntactic wildcard state.
- Regex safety envelopes now map high/medium/low findings to error/warn/info,
  restoring parity with the Python reference.
- Regression and property coverage was added without changing the 86-tool,
  23-category registry, profiles, response shapes, or locked dependencies.
- README, changelog, AGENTS tree layout, architecture/docs, and the MCP-tools
  skill now document the corrected semantics; generated docs are current.
- Targeted tests, fmt/clippy, full non-parity tests, doctests, cargo-deny,
  packaging, publish dry-run, and `scripts/release-check.sh` pass. A nightly
  cron fuzz smoke run completed without a crash.
- The release binary is 8,229,484 bytes versus the 8,229,476-byte
  post-expansion reference (+8 bytes); locked normal dependency delta: none.
# Parallel future track: MCP 2026-07-28 compatibility

This remains separate from the corrective utility pass. Do not mix protocol
modernization into C1-C5.

## P0 — Conformance/dependency gate

Before advertising MCP `2026-07-28`:

1. Inventory normative requirements relevant to a stdio tools-only server,
   including stateless per-request metadata, `server/discover`, version errors,
   modern result/cache fields, schema dialect requirements, cancellation, and
   deterministic tool listing.
2. Resolve the specification's JSON Schema 2020-12 requirement. Eggsact's
   current runtime validator intentionally implements only a bounded subset.
3. Evaluate a standards-complete validator only with network/file resolution
   disabled and measure dependency/binary cost. If it is too expensive, keep
   the older advertised protocol support honest rather than writing a custom
   full JSON Schema implementation.
4. Exercise the official MCP conformance scenarios applicable to a stdio
   tools server without adding production HTTP solely for testing.

## P1 — Dual-era protocol implementation

Only after P0 passes:

- add explicit modern request metadata/context types;
- keep legacy initialize/session behavior for existing protocol revisions;
- route modern requests statelessly without reading prior client metadata;
- implement `server/discover` and required unsupported-version responses;
- add modern result/cache fields without changing legacy response shapes;
- preserve server-configured profile/audience behavior;
- do not adopt the Rust MCP SDK unless it demonstrably reduces code and
  maintenance burden;
- do not add Tasks, prompts, resources, sampling, elicitation, HTTP transport,
  or other unrelated MCP capabilities.

## P2 — Protocol closure

Advertise `2026-07-28` as preferred only after conformance passes. Update MCP
architecture/user docs, preserve supported legacy revisions, and add stable
conformance checks at the cheapest appropriate verification tier.

---

## Open opportunities after corrective closure

These remain candidates, not commitments:

1. Deepen actual codegg use of existing typed preflight wrappers before adding
   more MCP-visible tools.
2. Measure high-frequency tool latency only if profiling shows a real need.
3. Revisit schema-detail defaults as model context economics change.
4. Consider YAML only when a concrete Codegg workflow justifies its dependency
   and semantic surface.

## Standing non-goals

- Not a general sandbox: classify risk, do not enforce it.
- Not every utility belongs in MCP: admit specification-heavy exact operations,
  not generic DevUtils feature parity.
- No external services or hidden host-state dependencies.
- No feature growth during corrective/closure passes.
