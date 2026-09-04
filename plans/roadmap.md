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
closure found one remaining cron dialect mismatch for star-step expressions.
The utility line is therefore **reopened for one final, cron-only compatibility
correction** below.

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

# Active corrective line: Vixie/Cronie star-step DOM/DOW semantics

## Why this is needed

The current parser stores:

```rust
unrestricted: input == "*"
```

and `day_matches()` uses that flag to choose between DOM/DOW OR behavior and a
one-field-controls-the-day shortcut. This fixes explicit full ranges such as
`1-31`, but it does not faithfully model the Vixie/Cronie dialect for fields
that **start with `*` and then apply a step**, such as `*/1` or `*/2`.

Cronie's Vixie-derived parser records `DOM_STAR` / `DOW_STAR` when the first
character of the corresponding field is `*`, before parsing the rest of the
field. Its runtime day matching then uses:

- if either DOM/DOW star flag is set: `dom_match && dow_match`;
- otherwise: `dom_match || dow_match`.

This distinction is important:

- `0 0 */1 * MON` -> Mondays only (`*/1` DOM matches every day, then AND Monday);
- `0 0 */2 * MON` -> only Mondays whose day-of-month also satisfies the step;
- `0 0 1-31 * MON` -> every day, because neither field has star syntax and the
  explicit full DOM range ORs with Monday.

POSIX specifies the five-field DOM/DOW element/list rule but does not define
`*/n` steps. For step syntax, eggsact should explicitly follow the Vixie/Cronie
extension it already claims to approximate rather than invent a project-specific
shortcut.

Primary references for implementation review:

- <https://github.com/cronie-crond/cronie/blob/master/src/entry.c>
- <https://github.com/openbsd/src/blob/master/usr.sbin/cron/cron.c>
- <https://man7.org/linux/man-pages/man5/crontab.5.html>
- <https://man7.org/linux/man-pages/man1/crontab.1p.html>

## Scope freeze

This is a semantic correction to the existing five-field parser only.

Do not:

- add tools, categories, profiles, schemas, machine codes, or response fields;
- add or broaden dependencies;
- add seconds/year fields, Quartz syntax, nicknames, `CRON_TZ`, DST/IANA time
  zones, random/hash scheduling, or locale behavior;
- change the 400-year search bound or search architecture;
- couple this work to MCP 2026-07-28;
- revise unrelated regex/network/encoding/datetime behavior.

All existing registry/profile/dependency invariants remain unchanged:
86 tools / 23 categories, original 80-tool prefix preserved, and only the
`full` profile contains the six contextual utility additions.

---

## C6.1 — Replace `unrestricted` with explicit star-syntax state

**Primary file:** `src/temporal/cron.rs`

The current name `unrestricted` is misleading for `*/2`: the field carries
Vixie star semantics but its parsed value set still constrains dates. Rename the
internal field to something precise such as:

```rust
pub struct CronField {
    pub values: Vec<u32>,
    pub min: u32,
    pub max: u32,
    pub star_syntax: bool,
}
```

Set `star_syntax` from the raw, successfully parsed field using the same rule as
the Vixie/Cronie parser for the supported grammar:

```rust
star_syntax: input.starts_with('*')
```

Do not infer this flag from the expanded value set. Therefore:

- `*` -> star syntax;
- `*/1`, `*/2`, etc. -> star syntax;
- explicit `1-31`, `0-7`, or equivalent explicit lists -> no star syntax;
- malformed star-prefixed input still fails normal parsing and never reaches a
  successful schedule.

This flag is parser/runtime state only. Do not add it to the MCP response unless
it is already exposed through an existing parsed-field serialization path; the
preferred correction has **no wire-shape change**.

### Acceptance

- Existing value parsing/normalization remains unchanged.
- No value-set equivalence is used to derive star semantics.
- Star-step syntax is represented without pretending the field is logically
  unconstrained.

---

## C6.2 — Match DOM/DOW with the Vixie star-flag rule

**Primary file:** `src/temporal/cron.rs`

Replace the current four-way `unrestricted` shortcut in `day_matches()` with a
rule that evaluates both parsed value sets first:

```rust
let dom = schedule.day_of_month.allows(day);
let dow = schedule.day_of_week.allows(weekday);

if schedule.day_of_month.star_syntax || schedule.day_of_week.star_syntax {
    dom && dow
} else {
    dom || dow
}
```

This single rule covers the ordinary cases without special casing:

| DOM | DOW | Expected rule |
|-----|-----|---------------|
| `*` | `MON` | all DOM values AND Monday -> Mondays |
| `1` | `*` | day 1 AND all DOW values -> first of month |
| `*` | `*` | all DOM AND all DOW -> every day |
| `1` | `MON` | day 1 OR Monday |
| `1-31` | `MON` | full explicit DOM OR Monday -> every day |
| `*/1` | `MON` | all stepped DOM values AND Monday -> Mondays |
| `*/2` | `MON` | stepped DOM value AND Monday |

Keep Sunday `0`/`7` normalization and all month/day-name behavior unchanged.

### Acceptance

- Bare-star behavior remains unchanged from correct existing cases.
- Explicit full ranges continue to use OR semantics when neither DOM nor DOW
  begins with `*`.
- Star-step forms constrain using their parsed values and select AND semantics.
- Search complexity and satisfiability behavior do not change.

---

## C6.3 — Regression and independent semantic tests

**Primary files:**

- `src/temporal/cron.rs`
- `tests/property/test_utility_properties.rs`

Replace tests that currently assert `*/1` is restricted/project-specific.

Required direct cases:

1. `0 0 * * MON` -> Mondays only.
2. `0 0 1 * *` -> first of month only.
3. `0 0 1 * MON` -> first of month OR Monday.
4. `0 0 1-31 * MON` -> every valid day.
5. `0 0 */1 * MON` -> Mondays only.
6. `0 0 */2 * MON` -> only Mondays satisfying the DOM step.
7. `0 0 1 * */1` -> first of month only.
8. At least one DOW star-step narrower than `*/1` to prove both parsed sets are
   evaluated under the star-flag/AND path.
9. Existing Sunday `0`/`7`, names, ranges, steps, leap-year, impossible-date,
   fixed-offset, strictly-after, and 400-year-bound tests remain green.

The independent property/semantic test must not duplicate `day_matches()`'s
implementation mechanically. Encode the expected reference rule from raw test
case metadata: star-flag case uses DOM AND DOW; non-star case uses DOM OR DOW.
Include both `*/1` and `*/2` so a future simplification cannot regress by merely
checking whether the expanded set covers the full domain.

If the existing cron fuzz target exercises field parsing/search, run it after
the parser-state change. Do not add a new fuzz target solely for this fix.

---

## C6.4 — Correct affected documentation and release notes

The previous pass intentionally documented `*/1` as restricted; those statements
must now be corrected everywhere they occur.

Search at minimum:

- `AGENTS.md`
- `README.md`
- `architecture/tools.md`
- `docs/mcp-tools.md`
- `.opencode/skills/mcp-tools/SKILL.md`
- `CHANGELOG.md` `[Unreleased]`

Preferred wording:

- ordinary explicit DOM/DOW fields use OR when neither field has Vixie star
  syntax;
- if either DOM or DOW **starts with `*`**, including supported `*/n` step
  forms, both parsed field predicates must match;
- bare `*` therefore behaves as the familiar wildcard because its value set
  already contains every value;
- explicit full ranges/lists are not equivalent to star syntax.

Do not call `*/n` behavior POSIX-defined. Describe it as the Vixie/Cronie step
extension layered over the five-field cron model.

Update `[Unreleased]` rather than creating a version bump. No release is part of
this implementation handoff.

---

## C6.5 — Verification and final closure

Run targeted checks first using the repository's actual test filters:

```bash
cargo test --locked cron
cargo test --locked --all-features property
cargo run --locked --features dev-tools --bin generate-docs -- --check
```

Then run the canonical gate from `AGENTS.md` / `scripts/release-check.sh`,
including fmt, clippy, non-parity tests, doctests, cargo-deny, package checks,
and publish dry-run.

Run the existing cron fuzz target for a short smoke pass if available in the
current fuzz harness. Any reproducible failure becomes a regression test.

Recheck the lightweight invariant:

```bash
cargo build --release --locked
wc -c < target/release/eggsact
cargo tree --locked -e normal
```

Expected dependency delta: **none**. Expected binary delta: negligible. Stop
and review if this small state/matching correction causes material growth.

### Final acceptance checklist

Close this line only when all are true:

- `CronField` retains star syntax independently from parsed value coverage.
- `*/1` and `*/2` follow the Vixie/Cronie star-flag path.
- `day_matches()` uses DOM AND DOW when either relevant field has star syntax,
  otherwise DOM OR DOW.
- Explicit full ranges/lists remain non-star expressions.
- Direct and independent tests cover bare-star, explicit-range, `*/1`, and
  nontrivial star-step behavior on both DOM and DOW.
- Documentation no longer claims `*/1` is restricted in the prior sense.
- 86 tools / 23 categories, profile counts, response schemas, and the first
  80-tool order remain unchanged.
- No dependency or nondeterministic source is added.
- Generated docs and `[Unreleased]` are current.
- Full local release gate and CI pass.
- Release binary/dependency graph show no unexplained material growth.

After these gates pass, replace this active section with a concise completed
closure record. Keep the detailed implementation plan in git history rather
than accumulating completed phase text in the living roadmap.

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
