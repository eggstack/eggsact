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

# Active line: deterministic utility corrective closure

## Scope

Fix exactly these correctness issues and their missing tests:

1. IPv6 CIDR address counts depend on prefix width, not network address.
2. IPv4-mapped IPv6 detection must distinguish mapped addresses from
   IPv4-compatible IPv6 forms such as `::1`.
3. Cron DOM/DOW wildcard semantics must preserve whether `*` was syntactically
   present rather than infer wildcard-ness from an equivalent full value set.

Also close two documentation/release-note drifts discovered in review:

- add the deterministic utility expansion under `CHANGELOG.md` `[Unreleased]`;
- fix the `AGENTS.md` source-tree indentation that currently makes text files
  appear beneath `temporal/`.

No new tools, profiles, machine codes, public APIs, dependencies, or MCP
protocol behavior belong in this line.

## Invariants that must remain unchanged

- 86 registered tools / 23 categories.
- The original 80-tool registration prefix stays byte-for-byte ordered.
- Only `full` contains the six new contextual tools; `default`, all `codegg_*`
  profiles, and `human_math` remain unchanged.
- Runtime dependency set remains unchanged; specifically no bigint, IP/CIDR,
  cron, timezone, or additional parsing crate is added.
- No system clock, locale, filesystem timezone database, environment, network,
  or randomness is introduced.
- MCP 2026-07-28 remains unadvertised and is not coupled to these fixes.
- Existing Python parity expectations for legacy tools remain unchanged.

---

## C1 — Correct IPv6 CIDR address counts

**Primary file:** `src/tools/network.rs`

### Defect

The IPv6 branch currently derives `address_count` from the network address:

```rust
u128::MAX - network + 1
```

That is incorrect. CIDR cardinality depends only on prefix length. It also
causes a host prefix such as `::/128` to overflow/wrap in optimized arithmetic
instead of reporting one address.

### Required implementation

Derive `host_bits = 128 - prefix` once and calculate count as:

- prefix `0`: exact decimal constant `2^128` because it is one larger than
  `u128::MAX`;
- prefix `1..=128`: `1u128 << host_bits` where `host_bits <= 127`.

Prefer a small helper with an explicit contract, for example:

```rust
fn ipv6_address_count(prefix: u8) -> String
```

Do not calculate cardinality from `network`, `last`, or subtraction between
addresses. Do not add a bigint dependency.

Keep the existing wire type as a decimal string.

### Regression tests

Add direct unit coverage for at least:

| CIDR | Expected count |
|------|----------------|
| `::/0` | `340282366920938463463374607431768211456` |
| `2001:db8::/1` | `170141183460469231731687303715884105728` |
| `2001:db8::/64` | `18446744073709551616` |
| `2001:db8::/127` | `2` |
| `2001:db8::/128` | `1` |
| `ffff:ffff:ffff:ffff::/64` | same count as any other `/64` |

Strengthen `tests/property/test_utility_properties.rs` so IPv6 count is a
function of prefix length only. At minimum compare two different networks with
the same generated/representative prefix and assert identical `address_count`.
For prefixes `1..=128`, independently check the expected power-of-two value.

### Acceptance

- `/0`, `/1`, `/64`, `/127`, `/128` return exact counts.
- Two networks with the same prefix return the same count.
- No overflow/panic/wrap is possible in debug or release builds.
- Existing network normalization, boundary, and containment tests still pass.

---

## C2 — Correct IPv4-mapped IPv6 classification

**Primary file:** `src/tools/network.rs`

### Defect

`ip_inspect` currently uses `Ipv6Addr::to_ipv4()` to decide whether an IPv6
address is IPv4-mapped. That conversion also recognizes IPv4-compatible IPv6
forms, so values such as `::1` can be incorrectly reported with the
`ipv4_mapped` tag/result.

### Required implementation

Use mapped-only semantics. Prefer `Ipv6Addr::to_ipv4_mapped()` on the declared
Rust 1.89 MSRV if available. If MSRV verification shows that API is not
available or suitable, implement the mapped prefix check directly:

- first 80 bits zero;
- next 16 bits all ones (`ffff`);
- final 32 bits are the embedded IPv4 address.

Centralize the detection in one helper and use it for both:

- the `ipv4_mapped` special-use tag;
- the structured `ipv4_mapped` result field.

Do not broaden this tool into general IPv4-compatible/NAT64/translation-prefix
classification during the corrective pass.

### Regression tests

Assert:

- `::ffff:192.0.2.1` is mapped and returns embedded `192.0.2.1`;
- `::ffff:c000:0201` canonical equivalent is mapped;
- `::1` is loopback but **not** mapped;
- `::192.0.2.1` is **not** mapped;
- `::` is unspecified but **not** mapped;
- ordinary IPv6 and IPv4 inputs remain unchanged.

Add a table-driven test that verifies `special_use` remains lexicographically
stable when multiple tags apply.

### Acceptance

- Only true `::ffff:0:0/96` mapped addresses receive mapped metadata.
- Loopback/unspecified IPv6 never gain an embedded IPv4 result by conversion
  side effect.
- No schema or response-shape change is required.

---

## C3 — Preserve cron wildcard syntax for DOM/DOW semantics

**Primary files:**

- `src/temporal/cron.rs`
- `src/tools/temporal.rs` only if response/documentation plumbing requires it

### Defect

`CronField::is_all()` infers wildcard status by checking whether the parsed
value set covers the full field domain. For day-of-month/day-of-week, ordinary
Vixie/POSIX semantics depend on whether a field was syntactically unrestricted,
not merely whether a restricted expression happens to enumerate every value.

For example:

```text
0 0 1-31 * MON
```

has both DOM and DOW syntactically restricted. Because `1-31` matches every
valid day, the OR rule makes this effectively daily. Treating `1-31` as if it
were `*` instead makes Monday control the schedule, which is incorrect.

### Required representation change

Retain explicit syntactic wildcard metadata while parsing. Keep the change
minimal, for example:

```rust
pub struct CronField {
    pub values: Vec<u32>,
    pub min: u32,
    pub max: u32,
    pub unrestricted: bool,
}
```

`unrestricted` should represent the syntax required by the documented DOM/DOW
rule, not value-set equivalence.

For this implementation line:

- bare `*` is unrestricted;
- `*/1` should be treated consistently with the intended cron dialect and
  covered by an explicit regression test; choose and document one rule rather
  than deriving it accidentally from values;
- explicit full ranges/lists such as `1-31`, `0-7`, or equivalent enumerations
  remain syntactically restricted even when they cover the full domain.

Then change `day_matches()` to use the retained unrestricted flags:

- both unrestricted -> day matches;
- DOM unrestricted only -> DOW controls;
- DOW unrestricted only -> DOM controls;
- neither unrestricted -> `dom || dow`.

Do not change month wildcard handling or add Quartz semantics.

### Normalization contract

`normalized_expression` may continue to emit expanded numeric values if that
is the existing contract. It must not be used to reconstruct syntactic
wildcard semantics internally. Execution semantics come from the parsed field
metadata.

If exposing an `unrestricted` flag in `parsed_values` would change the wire
contract unnecessarily, keep it internal. The corrective pass should prefer
no response-shape change.

### Regression tests

Add direct cron tests for:

1. `0 0 * * MON` -> Mondays only.
2. `0 0 1 * *` -> first day of month only.
3. `0 0 1 * MON` -> first of month **or** Monday.
4. `0 0 1-31 * MON` -> every valid day, because both fields are restricted
   and DOM always matches.
5. `0 0 1 * 0-7` -> every valid day under the same restricted/full-range
   reasoning because DOW always matches.
6. Explicit full-range/list expressions are not silently treated as bare `*`.
7. `*/1` behavior matches the chosen documented dialect rule.

Strengthen the utility property test so returned cron timestamps are not only
ordered/strictly-after but independently satisfy the parsed schedule's DOM/DOW
rule. Include at least one syntactically-full-range case so the prior defect
cannot recur.

### Acceptance

- DOM/DOW behavior is based on retained parse syntax, not normalized value-set
  equivalence.
- Existing names, steps, ranges, Sunday `0`/`7`, leap-year, and strict-after
  behavior remain unchanged.
- No new cron dependency is introduced.
- Search complexity and the existing 400-year bound remain unchanged.

---

## C4 — Documentation and release-note closure

### `CHANGELOG.md`

Populate `[Unreleased]` with the six deterministic utility additions and the
correctness fixes from C1-C3. Do not create a release/version bump unless the
maintainer explicitly chooses to publish.

At minimum document:

- six new contextual/full-profile tools;
- `base64` + `time` dependency additions with deterministic fixed-offset
  temporal behavior;
- corrected IPv6 CIDR counts;
- corrected mapped-IPv6 classification;
- corrected cron DOM/DOW unrestricted-field semantics.

### `AGENTS.md`

Fix the source tree so `src/text/regex_engine.rs` and
`src/text/confusables_generated.rs` are visually nested beneath `text/`, not
`temporal/`.

Do a narrow documentation search for claims directly affected by C1-C3.
Expected likely files:

- `architecture/tools.md`
- `docs/mcp-tools.md`
- `.opencode/skills/mcp-tools/SKILL.md` only if it repeats utility semantics

Do not rewrite unrelated documentation during this pass.

---

## C5 — Verification and final closure

### Targeted verification first

Run focused tests before the full suite so failures are attributable:

```bash
cargo test --locked network
cargo test --locked cron
cargo test --locked --test lib property
cargo test --locked tool_registration_tables_are_in_sync
cargo run --locked --features dev-tools --bin generate-docs -- --check
```

Use the repository's actual test filters/modules where names differ; do not add
new test binaries solely for this pass.

### Full gate

Then run the canonical verification sequence from `AGENTS.md`, including:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features --lib
cargo test --locked --all-features --bins
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo deny check advisories bans licenses sources
cargo package --locked --list
cargo package --locked --verbose
cargo publish --locked --dry-run
```

Run Python parity if the sibling `eggcalc` environment is available. The new
Rust-only utility tools should not create accepted parity failures for legacy
tools.

### Fuzzing

Because C3 modifies parser state, run the existing cron/utility fuzz target (or
the utility fuzz target that exercises cron) for a short targeted pass. Any
panic or reproducible semantic failure becomes a regression test before
closure. Do not create another fuzz target unless the existing one cannot
exercise the changed parser.

### Binary/dependency regression check

This pass should not materially alter binary size or the dependency graph.
Rebuild release and record final bytes in the PR/handoff description:

```bash
cargo build --release --locked
wc -c < target/release/eggsact
cargo tree --locked -e normal
```

Expected dependency delta from the pre-corrective state: **none**. A material
binary increase is a signal that the corrective implementation grew beyond
scope.

### Final acceptance checklist

This line is closed only when all are true:

- IPv6 address counts are exact for `/0` through `/128`.
- IPv4-mapped classification accepts only the mapped prefix form.
- Cron DOM/DOW semantics distinguish bare unrestricted fields from explicit
  full-range equivalents.
- Regression/property tests directly cover all three previously-missed cases.
- 86 tools / 23 categories and all profile counts remain unchanged.
- No dependency is added or broadened.
- Generated docs are current.
- `[Unreleased]` accurately describes the utility addition and fixes.
- `AGENTS.md` tree indentation is corrected.
- Tier 1/full local release gate passes.
- Release binary remains approximately at the post-expansion size with no
  unexplained material growth.

Once these gates pass, change this section to **Completed corrective closure**
and record only concise closure evidence (commits, test gate, dependency and
binary delta). Do not retain another long completed execution record in the
living roadmap; git history is the archive.

---

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
