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

The first corrective pass (`ae2be1d`, closed in `2cd6b60`) fixed IPv6 CIDR
cardinality, mapped-IPv6 classification, and the original DOM/DOW full-range
cron bug without changing registry/profile/dependency shape. Review after that
closure found one remaining cron dialect mismatch for star-step expressions,
closed by the C6 star-syntax correction below. The utility line is now
**closed** pending release.

---

# Completed corrective closure: network + initial cron syntax fix

The prior corrective line remains valid except for its conclusion that only a
bare `*` should carry wildcard/star semantics in DOM/DOW matching.

Completed and retained:

- IPv6 CIDR counts are exact and prefix-only.
- IPv4-mapped IPv6 detection is limited to `::ffff:0:0/96`.
- Explicit full ranges/lists such as DOM `1-31` are not treated as equivalent
  to wildcard syntax.
- Regex safety envelopes map high/medium/low findings to error/warn/info,
  restoring Python-reference parity.
- Regression/property coverage, docs, changelog, AGENTS tree layout, generated
  docs, cargo-deny, packaging, publish dry-run, release-check, and cron fuzz
  smoke all passed.
- The release binary measured 8,229,484 bytes versus the 8,229,476-byte
  post-expansion reference (+8 bytes), with no locked normal dependency delta.

The final cron follow-up below supersedes only the prior `*/n is restricted`
rule and any documentation/tests encoding that rule.

---

# Completed corrective closure: Vixie/Cronie star-step DOM/DOW semantics (C6)

Closed. The prior conclusion that only a bare `*` carries star semantics is
superseded; `*/n` step forms carry Vixie/Cronie star syntax while still
constraining via their parsed value sets.

Retained:

- `CronField` carries `star_syntax: input.starts_with("*")` instead of
  `unrestricted`; the flag is parser/runtime state only, with no wire-shape
  change.
- `day_matches()` uses DOM AND DOW when either field has star syntax,
  otherwise DOM OR DOW.
- `*/1` and `*/2` on either DOM or DOW follow the star-flag/AND path;
  explicit full ranges/lists such as `1-31` remain non-star (OR).
- Direct unit tests cover bare-star, explicit-range, `*/1`, and nontrivial
  star steps on both DOM and DOW, plus Sunday `0`/`7` normalization; the
  independent property test encodes the AND/OR reference rule from case
  metadata and includes both `*/1` and `*/2`.
- Docs corrected everywhere the old rule was stated (`AGENTS.md`,
  `README.md`, `architecture/tools.md`, `docs/mcp-tools.md`, the
  mcp-tools skill, `CHANGELOG.md` `[Unreleased]`); step syntax is described
  as the Vixie/Cronie extension, not POSIX-defined.
- Invariants held: 86 tools / 23 categories, original 80-tool prefix,
  full-profile-only utilities, no dependency delta, no response-schema change.
- Gates passed: fmt, clippy, lib/bins/non-parity tests, doctests,
  generate-docs check, cargo-deny, packaging, and publish dry-run. The cron
  fuzz smoke was skipped (requires nightly cargo-fuzz; stable only locally).
- Release binary delta negligible; dependency graph unchanged.

---

# Parallel future track: MCP 2026-07-28 compatibility

This remains separate from the utility corrective work. Do not mix protocol
modernization into C6.

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
