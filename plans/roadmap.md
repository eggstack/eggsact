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
| Golden fixtures, 59 property tests, 13 fuzz targets, MSRV/cargo-deny gates | Done |

## Current release state

Latest published version: **1.2.3** (see `CHANGELOG.md`). Release process is
manual per `docs/release.md`; CI verifies merge correctness only.

---

# Completed line: deterministic utility expansion

Status: **complete**. Implemented in `879570e`, documented/generated in
`59e9150`, and test-harness retry hardening landed in `915c684`.

Delivered six `full`-profile-only, `Contextual` tools: `ip_inspect`,
`cidr_inspect`, `datetime_convert`, `cron_inspect`, `codec_convert`, and
`radix_convert`. The final catalog is 86 tools across 23 categories; the
original 80-tool registration order remains an exact prefix, and all other
named profiles remain unchanged.

Closure evidence: the release gate passed locally, including formatting,
generated-doc checks, clippy, the non-parity suite, doc tests, cargo-deny,
package verification, and publish dry run. The release binary grew from
8,051,496 to 8,229,476 bytes (+177,980; +2.21%), below the roadmap threshold.
The only new runtime dependencies are `base64` (std only, no SIMD feature)
and `time` (std/parsing/formatting only, no host timezone features). The
MCP 2026-07-28 protocol track below remains separately gated and unadvertised.

## Goal

Add only the deterministic developer primitives that fill clear correctness
gaps for coding/operations agents without turning eggsact into a general
"devutils" collection. The selected surface is deliberately small:

1. `ip_inspect`
2. `cidr_inspect`
3. `datetime_convert`
4. `cron_inspect`
5. `codec_convert`
6. `radix_convert`

The intended post-line catalog is **86 tools across 23 categories**. All six
new tools are initially `full`-profile only and model-visible as
`ToolExposure::Contextual`. No `default`, `codegg_*`, or `human_math` profile
expands in this line. This keeps ordinary model context unchanged while
making the primitives available when a caller explicitly chooses `full`.

### Admission rule

A new primitive belongs in this line only when all of the following are true:

- identical inputs produce identical semantic outputs;
- no filesystem, network, environment, locale, system clock, system timezone,
  random source, or hidden process state is required;
- the operation is easy for software to compute exactly but sufficiently
  specification-heavy or error-prone that model-only execution is unreliable;
- the implementation is bounded by explicit input/output/search limits;
- dependency and stripped-binary growth are proportionate to the value;
- the capability does not duplicate an existing eggsact tool at a different
  spelling.

## Research decisions / scope freeze

| Area | Decision | Rationale |
|------|----------|-----------|
| IPv4/IPv6 + CIDR | **Implement, std-only** | `std::net` supplies address parsing; CIDR masks/arithmetic are small integer operations. |
| Datetime conversion | **Implement with `time`** | Exact RFC 3339 / Unix conversion is high-value; `time` 0.3.55 supports the crate's Rust 1.89 MSRV. |
| Time zones | **Fixed offsets only** | IANA/system timezone lookup introduces host/tzdb state and a much larger semantic/dependency surface. |
| Cron | **Implement a bounded 5-field parser over `time`** | Avoid the `chrono` + parser/builder dependency trees of general cron crates; eggsact needs a narrow Vixie-style subset, not a scheduler. |
| Base64 | **Use `base64` with default features disabled** | Current default enables `simd-unsafe`; eggsact does not need SIMD for bounded utility payloads. |
| Hex | **Implement locally** | Encoding/decoding is trivial and does not justify another crate. |
| Radix | **Implement std-only with `u128` magnitude** | Bases 2–36 need no bigint dependency for the intended coding-agent use. |
| General JSON Schema validator tool | **Do not add** | A standards-complete validator has disproportionate dependency/binary surface; the existing light validator must not be relabeled as full JSON Schema. |
| YAML | **Defer** | Anchors, aliases, tags, duplicate-key semantics and round-tripping expand scope beyond this line. |
| `now` / current time | **Do not add** | Violates identical-input determinism. Callers must provide the reference instant. |
| UUID/random/password/key generation | **Do not add** | Nondeterministic and outside eggsact's exact-input/exact-output role. |
| HTTP/network utilities | **Do not add** | External I/O belongs in dedicated tools such as eggsearch, not eggsact. |
| JWT/X.509/PGP/crypto toolbox | **Do not add** | Large correctness/dependency domain with no demonstrated Codegg requirement. |

Dependency candidates are therefore limited to:

```toml
# exact compatible version is resolved/locked during implementation
# Do not enable local-offset, macros, serde, rand, or timezone features.
time = { version = "0.3", default-features = false, features = ["std", "parsing", "formatting"] }

# Disable the default `simd-unsafe` feature; std implies alloc.
base64 = { version = "0.23", default-features = false, features = ["std"] }
```

Before merging either dependency, verify the selected locked release still
satisfies the declared Rust 1.89 MSRV and `cargo deny` policy. Do not add
`chrono`, `jiff`, `croner`, `cron`, a hex crate, a bigint crate, or a network
dependency for this work.

### Binary-size gate

The lightweight binary is part of the product contract. Record a baseline
before the first implementation commit:

```bash
cargo build --release --locked
wc -c < target/release/eggsact
cargo tree --locked -e normal
```

Repeat after (a) the std-only tools, (b) `base64`, and (c) `time`/temporal
work. If any dependency phase increases the stripped release binary by more
than **1 MiB or 10% of the baseline, whichever is smaller**, stop and review
before continuing. This is a design-review threshold, not a reason to hide
size with feature removal. Also inspect `cargo tree -e normal` for unexpected
network, TLS, OS-timezone, random, or proc-macro/runtime dependencies.

---

## Phase U0 — Baseline and registry invariants

**Objective:** establish the compatibility and size baselines before adding a
new category or dependency.

### Work

1. Run the repository verification commands from `AGENTS.md` on current main.
2. Record the current release binary byte size and normal dependency tree in
   the implementation handoff/PR description; do not add evidence files to
   the repository.
3. Snapshot current profile counts and confirm the current 80-tool order.
4. Search hand-maintained docs for hard-coded `80 tools` / `20 categories`
   references so the closure pass has an explicit update list.
5. Confirm the current Python parity suite does not require a one-for-one tool
   list. New Rust-only tools must not be added to
   `accepted_parity_failures.txt` merely to suppress catalog differences.

### Acceptance

- Current main passes Tier 1 verification before feature work begins.
- Baseline binary size and dependency tree are known.
- Existing tool order is preserved as a compatibility prefix in all later
  phases.

---

## Phase U1 — Network literals: `ip_inspect` + `cidr_inspect`

**Objective:** add exact IPv4/IPv6 parsing, classification and CIDR arithmetic
without adding a dependency or performing network I/O.

### Files / registration

Create:

- `src/tools/network.rs`
- `src/mcp/specs/network.rs`
- `src/mcp/schemas/network.rs`

Wire the category through:

- `src/tools/mod.rs`
- `src/mcp/specs/mod.rs`
- `src/mcp/schemas/mod.rs`
- `src/mcp/registry/all_tools.rs`

Append `NETWORK_TOOLS` **after the existing category slices**. Do not insert it
before an existing category: `all_tools.rs` deliberately preserves the legacy
registration order for parity. The pre-existing 80 tools must remain the
first 80 in the same order.

Both tools:

- category: `network`
- tier: `2`
- profiles: `&["full"]`
- exposure: `Contextual`
- harness use: `none`
- cost: `Cheap`
- composite: `false`

No new route-critical classification or typed preflight wrapper is needed.
Use existing common machine codes (`INVALID_ARGUMENTS`, `INPUT_TOO_LARGE`,
`UNSUPPORTED_FEATURE`) for errors; do not create category-specific codes
unless implementation discovers a stable caller-routing need.

### `ip_inspect` contract

Input:

```json
{"address":"2001:db8::1"}
```

Required behavior:

- parse with `std::net::IpAddr` and reject surrounding junk;
- return canonical textual form and family (`ipv4` / `ipv6`);
- return address bytes as deterministic lowercase hex;
- return numeric representation as a **decimal string** (`u32`/`u128`
  internally) so JSON/JS integer precision is never a wire concern;
- return a lexicographically stable array of applicable special-use tags;
- report IPv4-mapped IPv6 information when applicable.

Keep classification explicit and standards-based rather than exposing a
single ambiguous `is_global` boolean. Initial tags should cover at least:

- IPv4: unspecified, loopback, RFC1918 private, link-local, multicast,
  documentation ranges, shared/CGNAT (`100.64.0.0/10`);
- IPv6: unspecified, loopback, link-local (`fe80::/10`), unique-local
  (`fc00::/7`), multicast (`ff00::/8`), documentation (`2001:db8::/32`),
  IPv4-mapped.

Do not infer routability from DNS, interfaces, route tables, or host state.
Keep the range table centralized and unit-tested so future standards updates
are auditable.

### `cidr_inspect` contract

Input:

```json
{"cidr":"10.1.2.3/24","contains":"10.1.2.200"}
```

`contains` is optional and keeps containment in the same primitive rather than
adding another tiny tool.

Required result fields:

- address family;
- canonical network CIDR;
- prefix length and host-bit count;
- network address;
- netmask in canonical address form;
- first and last address in the block;
- IPv4 broadcast address (IPv4 only; label it as arithmetic broadcast, not
  "usable host");
- address count as a decimal string;
- optional containment result and canonicalized candidate address.

Implementation rules:

- split address/prefix manually and parse address with `std::net`;
- use `u32` for IPv4 and `u128` for IPv6 masks;
- special-case prefix zero before shifting to avoid width-sized shifts;
- do not use a bigint crate: IPv6 `/0` is the only count that exceeds `u128`;
  emit the exact constant `340282366920938463463374607431768211456`
  (`2^128`) for that case and use `u128` otherwise;
- reject cross-family `contains` checks rather than coercing them;
- do not report "usable host count" because `/31`, `/32`, `/127`, `/128`
  semantics depend on deployment context.

### Tests

Unit/fixture coverage must include:

- IPv4 and compressed/expanded IPv6 canonicalization;
- all special-use classification boundaries (just below, first, last, just
  above each range where representable);
- `/0`, host-prefix (`/32`, `/128`), and ordinary masks;
- non-network input normalization (`10.1.2.3/24` -> `10.1.2.0/24`);
- first/last/broadcast arithmetic;
- IPv6 `/0` exact address count;
- candidate containment at first/last/outside boundaries;
- malformed address, missing prefix, too-large prefix, negative/junk prefix,
  cross-family containment;
- deterministic output ordering.

Add property tests for network normalization idempotence and containment:
normalizing an already-normalized CIDR is stable; network/last addresses are
contained; immediate representable neighbors outside a non-`/0` block are
not.

### Acceptance

- No new dependency is introduced in U1.
- Existing tool order remains unchanged for the first 80 entries.
- Only the `full` profile count changes (+2); all other profile snapshots are
  unchanged.
- Tier 1 verification passes.
- Release binary delta is recorded and expected to be negligible.

---

## Phase U2 — Exact encodings: `codec_convert` + `radix_convert`

**Objective:** provide exact byte/text and integer-base conversions without
creating a collection of one-operation tools.

### Files / registration

Create:

- `src/tools/encoding.rs`
- `src/mcp/specs/encoding.rs`
- `src/mcp/schemas/encoding.rs`

Append `ENCODING_TOOLS` after the prior registration prefix/new categories.
Both tools are tier 2, `full` only, `Contextual`, non-composite and `Cheap`.

### Dependency

Add `base64` with defaults disabled and only `std` enabled. Verify the locked
crate does not enable its `simd-unsafe` feature. Hex and radix logic stay
local/std-only.

### `codec_convert` contract

Input:

```json
{"value":"48656c6c6f","from":"hex","to":"utf8"}
```

Supported formats are exactly:

- `utf8`
- `hex`
- `base64`
- `base64url`

Semantics:

- all transformations operate on bytes internally;
- `utf8` source means the UTF-8 bytes of the JSON string;
- conversion *to* `utf8` fails on invalid UTF-8; never use lossy replacement;
- hex input accepts ASCII upper/lowercase digits only, requires even length,
  and rejects `0x`, whitespace and separators; output is lowercase;
- standard Base64 decoding accepts valid padded or unpadded standard-alphabet
  input but rejects whitespace/mixed alphabets; output is canonical padded
  standard Base64;
- Base64URL decoding accepts valid padded or unpadded URL-safe input but
  rejects whitespace/mixed alphabets; output is canonical **unpadded**
  Base64URL;
- identical `from`/`to` still parses/validates then returns canonical output
  rather than blindly echoing malformed input.

Bound the decoded/encoded byte count before allocation where possible and
respect existing request/output budgets. The base64 crate documents potential
`usize` overflow in length calculations; perform eggsact's own input/output
bounds before calling allocating APIs.

### `radix_convert` contract

Input:

```json
{"value":"-ff","from_base":16,"to_base":2,"uppercase":false}
```

Semantics:

- bases 2 through 36 inclusive;
- signed-magnitude string input with optional leading `+`/`-`;
- ASCII digits `0-9a-zA-Z`; reject any digit outside `from_base`;
- parse magnitude into `u128` with checked multiply/add;
- no `0x`, `0o`, `0b`, separators, decimals, exponents or whitespace;
- canonical output has no leading zeros, except zero itself;
- negative zero canonicalizes to `0`;
- output alphabet is lowercase unless `uppercase:true`;
- no two's-complement interpretation and no arbitrary precision in the first
  implementation.

If fixed-width/two's-complement conversion later proves useful, extend this
contract deliberately rather than guessing width from the input.

### Tests

`codec_convert`:

- RFC 4648-known standard and URL-safe vectors;
- empty input in every format;
- padded/unpadded decode behavior and canonical re-encode;
- invalid alphabet, invalid padding, odd hex, invalid UTF-8;
- NUL and non-ASCII UTF-8 round trips;
- input/output limit boundaries.

`radix_convert`:

- zero and signed zero;
- bases 2, 8, 10, 16, 36 and mixed conversions;
- `u128::MAX` in representative bases;
- checked overflow one digit beyond range;
- invalid digit/base/prefix/whitespace;
- uppercase/lowercase canonicalization;
- round-trip property: parse(convert(x, a->b), b) equals original magnitude/sign
  for generated values and bases.

### Acceptance

- `base64` is the only dependency added in U2 and `simd-unsafe` is disabled.
- No unsafe code is added to eggsact for the feature.
- Only the `full` profile count changes (+2 relative to U1).
- Binary-size review passes the stated threshold.
- Tier 1 verification passes.

---

## Phase U3 — Explicit time: `datetime_convert` + `cron_inspect`

**Objective:** eliminate model arithmetic around timestamps and cron schedules
while retaining eggsact's identical-input determinism.

### Files / registration

Create:

- `src/temporal/mod.rs` — reusable parsing/conversion helpers
- `src/temporal/cron.rs` — cron parser, bitsets and next-run search
- `src/tools/temporal.rs`
- `src/mcp/specs/temporal.rs`
- `src/mcp/schemas/temporal.rs`

Export the pure temporal module from `src/lib.rs` only if its typed API is
small and intentionally supportable; otherwise keep it crate-internal and
expose it through `ToolRegistry`, consistent with most non-text utility
categories.

Append `TEMPORAL_TOOLS` after the current registry prefix/new categories.
`datetime_convert` is `Cheap`; `cron_inspect` is `Moderate` because it performs
bounded schedule search. Both are tier 2, `full` only, `Contextual`,
non-composite.

### Dependency

Add `time` with only `std`, `parsing`, and `formatting`. Explicitly do **not**
enable `local-offset`, macros, rand, serde, or platform timezone features.
The implementation must never call a wall-clock `now()` or discover a host
UTC offset.

### `datetime_convert` contract

Input shape:

```json
{
  "value":"2026-09-03T11:00:00-04:00",
  "format":"rfc3339",
  "output_offset":"Z"
}
```

`format` is one of:

- `rfc3339`
- `unix_seconds`
- `unix_milliseconds`
- `unix_nanoseconds`

`value` is always a string, including Unix integer formats. This avoids JSON
safe-integer ambiguity and lets nanosecond timestamps use the full supported
range. `output_offset` is optional; when absent, preserve the RFC 3339 input
offset or use UTC for Unix inputs. When present, it is exactly `Z` or a numeric
`+HH:MM`/`-HH:MM` fixed offset.

Return a normalized record containing:

- canonical RFC 3339 at the selected fixed offset;
- UTC RFC 3339;
- Unix seconds, milliseconds and nanoseconds as decimal strings;
- selected offset seconds;
- calendar components useful to agents (year/month/day/hour/minute/second and
  weekday) if they can be produced without inflating the schema excessively.

Use `time::format_description::well_known::Rfc3339` for RFC 3339 parsing and
formatting. Document and test whole-unit behavior for negative fractional
instants; exact nanoseconds are the authoritative representation. Reject
leap-second inputs unless the selected `time` API explicitly and correctly
supports them—do not normalize an unsupported `:60` silently.

### `cron_inspect` contract

Input:

```json
{
  "expression":"0 9 * * MON-FRI",
  "after":"2026-09-03T11:00:00-04:00",
  "count":5
}
```

The reference instant is mandatory and RFC 3339. The schedule is interpreted
in the **fixed UTC offset carried by `after`**. No current time, locale,
`TZ`, `/etc/localtime`, IANA name, or DST rule is consulted.

Initial grammar is deliberately the conventional five-field form:

`minute hour day-of-month month day-of-week`

Support:

- `*`
- comma lists
- inclusive numeric/name ranges (`a-b`)
- steps (`*/n`, `a-b/n`)
- month names `JAN`..`DEC` (case-insensitive)
- weekday names `SUN`..`SAT` (case-insensitive)
- weekday `0` and `7` both meaning Sunday

Ranges do not wrap across the endpoint (`FRI-MON` is rejected rather than
assigned a project-specific meaning).

Explicitly reject in v1:

- seconds or year fields;
- Quartz `?`, `L`, `W`, `#`;
- nicknames such as `@daily`;
- timezone prefixes/suffixes (`CRON_TZ`, `TZ`);
- random/hash scheduling extensions;
- locale-dependent names.

Day-of-month/day-of-week semantics must follow ordinary Vixie/POSIX cron:
when both fields are restricted, a day matches when **either** field matches;
when one is wildcard, the restricted field controls. Put this rule in both
code comments and user documentation because it is a frequent source of
cross-implementation errors.

Result contains:

- original and normalized five-field expression;
- parsed allowed values for each field in stable numeric order;
- fixed offset used;
- `satisfiable` boolean;
- `next_runs` as canonical RFC 3339 strings, strictly after `after`;
- count actually returned.

`count` defaults to 5 and is bounded to 1..32 in the schema/handler.

### Cron search algorithm / bound

Do not scan minute-by-minute. Parse each field once into compact bitsets or
small boolean sets, then advance by calendar day and select allowed
hour/minute values within matching days. This keeps sparse schedules cheap.

A five-field schedule has no year component and Gregorian weekday/leap-year
alignment repeats over 400 years. Use a maximum 400-year search horizon from
the supplied reference instant. If no day matches within a complete
representable cycle, return a successful `satisfiable:false` result with an
empty `next_runs` list. If the requested reference lies too close to the
`time` crate's representable year boundary to inspect the required horizon,
return a bounded-range error rather than overflow or loop indefinitely.

Check the cooperative budget/cancellation context at natural search
boundaries even though the search is bounded; `cron_inspect` is declared
`Moderate`.

### Tests

`datetime_convert`:

- Unix epoch and pre-epoch instants;
- positive/negative fixed offsets including date rollover;
- leap-day valid/invalid cases;
- fractional second precision through nanoseconds;
- seconds/milliseconds/nanoseconds conversions;
- negative sub-second semantics;
- minimum/maximum supported practical dates;
- malformed RFC 3339, offset, integer and overflow inputs;
- proof that no host timezone/system clock affects output.

`cron_inspect`:

- wildcards, lists, ranges and steps in every field;
- month/day names and case normalization;
- Sunday `0`/`7` equivalence;
- DOM/DOW OR behavior and wildcard behavior;
- month-end boundaries and leap years;
- Gregorian century case around 2100;
- impossible schedule such as February 31 -> `satisfiable:false`;
- strictly-after behavior when `after` is itself a scheduled instant;
- fixed-offset preservation;
- rejected six/seven-field and Quartz syntax;
- `count` bounds;
- search-bound behavior near representable date limits.

Add property tests for RFC3339 -> Unix nanos -> RFC3339 instant preservation,
and for cron invariants (all returned timestamps are ordered, strictly after
the reference, and independently satisfy every relevant parsed field under
the documented DOM/DOW rule).

Because cron is a parser with adversarial text input, add a fuzz target if the
existing fuzz harness can do so without material maintenance cost. At minimum,
all parser panics/crashes discovered during implementation become regression
tests.

### Acceptance

- `time` is the only dependency added in U3.
- `cargo tree` shows no local-time/IANA timezone dependency path.
- No tool reads current time, environment timezone, filesystem timezone data,
  network data or locale.
- Only the `full` profile count changes (+2 relative to U2).
- Binary-size review passes the threshold.
- Tier 1 verification passes plus targeted parser fuzzing/property tests.

---

## Phase U4 — Catalog, documentation and release closure

**Objective:** land the six primitives as one coherent capability line without
leaving stale counts, schemas or profile assumptions.

### Registry/profile closure

1. Confirm final count is 86 registered tools / 23 categories.
2. Preserve the old 80-tool registration prefix exactly.
3. Expected profile behavior:
   - `full`: +6 model-visible/contextual tools;
   - every other named profile: unchanged.
4. Do not add a new profile merely for these utilities.
5. Run `tool_registration_tables_are_in_sync` and all profile snapshot tests.
6. Run `cargo run --features dev-tools --bin generate-docs` and commit generated
   `architecture/mcp-server.md` profile block + `generated/tool-cards.md`.

### Hand-maintained docs

Update, where applicable:

- `AGENTS.md` counts/category list and relevant gotchas;
- `architecture/tools.md` category/file table and behavioral contracts;
- `architecture/registry-profiles.md` counts/category aggregation;
- `architecture/overview.md` if its module/category map is hard-coded;
- `docs/mcp-tools.md` complete wire examples and explicit cron/timezone/codec
  semantics;
- `docs/library-api.md` only if temporal helpers become a public typed API;
- README tool/category counts or capability summary if present;
- `CHANGELOG.md` only when preparing an actual release, not during planning.

Search globally for stale hard-coded counts before closure rather than relying
on the list above.

### Catalog/context-cost check

Compare serialized `tools/list` output for `default`, `codegg_core`, and
`full` before/after. `default` and Codegg profile listings must be byte-for-byte
unchanged apart from any unrelated generated/version metadata; the new schema
cost is paid only by `full` callers.

Do not add `mcpgrade`, Node, or another runtime dependency solely for this
check. An external MCP catalog grader may be run manually as a review aid. If
a recurring catalog-quality problem appears, prefer a small native Rust test
for the precise invariant.

### Full verification

Run the canonical verification order from `AGENTS.md`, then Tier 3 fuzzing for
new parser surfaces and `scripts/release-check.sh` before publication.

Final closure report/PR description should include:

- six tools added and their exposure/profile placement;
- dependency delta (`base64`, `time` only) and resolved feature sets;
- baseline/final stripped binary bytes and percentage delta;
- relevant property/fuzz coverage;
- confirmation that no nondeterministic source was introduced;
- confirmation that non-`full` profile catalogs did not grow.

---

# Parallel protocol track: MCP 2026-07-28 compatibility

This is related to the utility expansion because the expanded catalog should
not be released while eggsact's MCP documentation calls an obsolete revision
"preferred" indefinitely, but it is a **separately mergeable protocol
workstream**. Do not entangle the six tool implementations with a speculative
MCP rewrite.

The current server is legacy-era: it prefers `2025-11-25`, accepts
`2024-11-05`, and requires `initialize` -> `notifications/initialized` before
ordinary methods. MCP `2026-07-28` replaces this with a stateless per-request
model and `server/discover`.

Primary references:

- <https://modelcontextprotocol.io/specification/2026-07-28/basic/versioning>
- <https://modelcontextprotocol.io/specification/2026-07-28/basic/index>
- <https://modelcontextprotocol.io/specification/2026-07-28/server/discover>
- <https://modelcontextprotocol.io/specification/2026-07-28/server/tools>
- <https://github.com/modelcontextprotocol/conformance>

## P0 — Conformance/dependency gate before advertising 2026-07-28

The new revision also states that clients and servers MUST support JSON Schema
2020-12 and validate schemas according to their dialect. Eggsact's current
runtime validator intentionally implements a strict bounded subset. Therefore:

1. Do **not** merely add `2026-07-28` to the supported-version constant.
2. Inventory every normative server requirement relevant to a stdio,
   tools-only implementation: per-request `_meta`, stateless dispatch,
   `server/discover`, unsupported-version error `-32022`, modern result shapes,
   deterministic/cacheable `tools/list`, removed/deprecated methods, schema
   dialect handling, cancellation and error semantics.
3. Run/inspect the official conformance suite scenarios applicable to a stdio
   tools server. If the framework's server runner still requires HTTP, do not
   add production HTTP solely for the suite; use its fixtures/schema tests or
   a test-only adapter/client to exercise eggsact stdio behavior.
4. Resolve JSON Schema 2020-12 conformance before claiming support.

### JSON Schema decision gate

Evaluate `jsonschema` with **default features disabled** as an internal
protocol/schema validator candidate. Its defaults currently include HTTP/file
resolution and TLS; these are forbidden for eggsact. Even with defaults off,
it has a materially larger dependency set than the six utility tools.

Prototype on a branch/temporary commit and measure:

```toml
jsonschema = { version = "0.52", default-features = false }
```

Required properties if retained:

- no `resolve-http`, `resolve-file`, TLS, async retrieval, or automatic network
  `$ref` resolution;
- unresolved external refs fail closed;
- bounded schema depth/subschema/resource use per MCP guidance;
- current tool schemas and argument validation continue to produce stable,
  actionable errors (compatibility mode may need an adapter layer);
- MSRV remains 1.89;
- binary/dependency growth passes an explicit maintainer review.

If a standards-complete validator is judged too expensive, **do not implement
a home-grown full JSON Schema engine** and do not advertise `2026-07-28` as
supported. Keep the legacy protocol honest and document the blocker until a
smaller conformance path exists. This gate is independent of whether the six
new tools ship.

A standards-complete internal validator, if accepted for protocol conformance,
does **not** automatically justify a public `json_schema_validate` utility
tool. Public surface expansion remains a separate product decision.

## P1 — Dual-era protocol architecture

Only after P0 is resolved, implement 2026 modern semantics while preserving
legacy compatibility for existing clients.

### Protocol model

Add typed modern request metadata in `src/mcp/protocol.rs` for:

- `io.modelcontextprotocol/protocolVersion` (required);
- `io.modelcontextprotocol/clientCapabilities` (required);
- `io.modelcontextprotocol/clientInfo` (optional);
- unknown `_meta` entries retained/tolerated where the spec permits.

Introduce an explicit request-era/context type rather than scattering date
string checks through `server.rs`, e.g. `ProtocolEra::{Modern, Legacy}` plus a
`RequestProtocolContext` holding the requested revision and declared client
capabilities.

A dual-era stdio process selects behavior as specified by MCP:

- a modern request carrying required per-request metadata is handled
  statelessly and must not read client identity/capabilities/version from a
  prior request;
- an `initialize` opener selects legacy semantics for the legacy client;
- keep the existing legacy `SessionState` only for legacy dispatch;
- modern requests must bypass legacy `Ready` gating;
- server-configured profile/audience remain valid process configuration
  because they are server policy, not inferred client/session state.

Do not migrate to the Rust MCP SDK solely for this change; eggsact's custom
stdio transport is small and already contains project-specific profile,
budget, response and compatibility behavior.

### Version errors / discovery

Implement `server/discover` before ordinary modern dispatch. It must report:

- `resultType: "complete"`;
- supported modern/legacy revisions intended for discovery;
- server capabilities;
- `_meta.io.modelcontextprotocol/serverInfo`;
- conservative cache fields initially (`ttlMs: 0`, `cacheScope: "private"`)
  until cache semantics are verified with profile/audience filtering.

For unsupported modern versions, return JSON-RPC `-32022` with structured
`data.supported` and `data.requested` as required by the spec.

Modern requests missing required `_meta` protocol version or client
capabilities return `-32602`; never fall back to connection-global legacy
metadata for them.

### Modern result shapes

For modern requests only:

- `tools/list` adds `resultType: "complete"` and spec-compliant cache fields;
- `tools/call` complete results add `resultType: "complete"` while preserving
  the eggsact tool envelope inside MCP content/structured result fields;
- add server-info `_meta` to results where required/recommended;
- keep tool order deterministic;
- do not change legacy response bytes/shapes merely to share code.

Confirm against the final spec/conformance tests which legacy methods (for
example `ping`) are absent in modern MCP and return `-32601` for them rather
than accidentally carrying legacy behavior forward.

### Testing

Split MCP test helpers into explicit legacy and modern paths. Preserve all
existing `2024-11-05` / `2025-11-25` initialization tests, then add modern
coverage for:

- discovery before initialization;
- direct modern `tools/list` / `tools/call` with no handshake;
- missing required `_meta`;
- unsupported version `-32022` and supported-version data;
- proof that one modern request's client metadata does not affect the next;
- deterministic list ordering/resultType/cache fields;
- legacy opener still gets legacy lifecycle/response shape;
- modern and legacy behavior can coexist without shared client state;
- cancellation/concurrency remains correlated by JSON-RPC id;
- all relevant official conformance scenarios.

Do not make protocol modernization a reason to introduce HTTP transport,
Tasks, subscriptions, sampling, elicitation, resources, prompts, or other MCP
features eggsact does not need.

## P2 — Protocol documentation/closure

After conformance tests pass:

- make `2026-07-28` the preferred revision only if all claimed normative
  requirements are satisfied;
- retain supported legacy revisions for compatibility;
- update `architecture/mcp-server.md`, `docs/mcp-tools.md`, `AGENTS.md` lifecycle
  gotchas and any examples that assume initialization is universally required;
- document the dual-era selection rule clearly;
- document the supported JSON Schema dialect(s) and external `$ref` policy;
- add protocol-conformance checks to Tier 3/manual verification unless they
  are cheap/stable enough for Tier 1.

---

# Implementation order / handoff

Recommended execution order:

1. **U0** baseline.
2. **U1** network tools (std-only) and verify architecture/registration pattern.
3. **U2** encoding/radix; size-gate `base64`.
4. **U3** datetime/cron; size-gate `time` and fuzz the parser.
5. **U4** catalog/docs/closure.
6. Run **P0** protocol conformance research in parallel or immediately after
   U0. Land **P1/P2** separately once the JSON Schema/conformance gate is
   resolved.

Do not combine all phases into one unreviewable commit. Each U phase should be
independently green and leave the repository releasable. Dependency commits
should make their feature selection and binary delta easy to inspect.

## Line completion criteria

The deterministic utility line is complete when:

- all six planned tools are implemented with the contracts above;
- only `base64` and `time` were added for the tool work, with the constrained
  feature sets above;
- no system clock/timezone/network/filesystem/randomness is reachable from the
  new handlers;
- existing 80-tool order remains the registration prefix;
- non-`full` profiles have not expanded;
- generated and hand-maintained docs are current;
- Tier 1, targeted property/fuzz checks and release verification pass;
- final binary/dependency delta has been reviewed and accepted.

MCP 2026 compatibility is tracked alongside this work but has its own
completion condition: do not call it complete or preferred until the full
normative/conformance gate, especially JSON Schema 2020-12 handling, is
resolved.

---

## Later opportunities (not committed by this line)

1. **codegg integration depth** — wire more typed preflight wrappers into
   actual harness flows when there is a concrete call site.
2. **YAML support** — revisit only when a real configuration workflow justifies
   the dependency and YAML semantic surface.
3. **Benchmarks** — add targeted latency/counter benchmarks when profiling
   identifies a hot tool; do not build a benchmark framework preemptively.
4. **Schema detail ergonomics** — revisit `EGGCALC_MCP_SCHEMA_DETAIL=compact`
   defaults based on observed model/tool-catalog cost.
5. **Two's-complement radix mode / IANA timezones** — only after demonstrated
   caller demand; neither is part of the first utility expansion.

## Non-goals (standing)

- Not a general sandbox: classify risk, do not enforce it.
- Not every tool model-visible by default: context cost is a product constraint.
- No external services: local, deterministic, bounded.
- No utility-count race: prefer one exact, specification-heavy primitive over
  many trivial wrappers.
- No hidden host-state semantics: callers provide every value that can affect a
  result.
