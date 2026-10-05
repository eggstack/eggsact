# Temporal Core

`src/temporal/` is a small `pub(crate)` leaf core for time arithmetic. It holds fixed-offset RFC 3339 datetime helpers ([`mod.rs`](../src/temporal/mod.rs), 75 lines) and a Vixie/Cronie-style five-field cron parser with a bounded next-match search ([`cron.rs`](../src/temporal/cron.rs), 408 lines). The core is reached from exactly two `temporal`-category tools implemented in [`src/tools/temporal.rs`](../src/tools/temporal.rs) (284 lines), declared in [`src/mcp/specs/temporal.rs`](../src/mcp/specs/temporal.rs) and schema'd in [`src/mcp/schemas/temporal.rs`](../src/mcp/schemas/temporal.rs).

The governing rule is **explicit inputs only**: no clock, no IANA timezone database, no environment variables, no network. Nothing in this core can answer "what time is it?" — a reference instant is always supplied by the caller, and that instant's own fixed offset is the only offset ever used. There is no DST, no `CRON_TZ`, no locale. See [overview.md](overview.md) for the layer diagram, [tools.md](tools.md) for the category-level tool contract, and [text-library.md](text-library.md) for the sibling `src/text/` core it must not be confused with.

## The Determinism Boundary

Four inputs are forbidden, and none of them appear anywhere in `src/temporal/`:

| Forbidden | Consequence in this core |
|-----------|---------------------------|
| Wall clock | No function accepts or reads "now". `cron_inspect` has a **mandatory** `after` argument (`schemas/temporal.rs:12` `required: ["expression","after"]`) — there is no default and no fallback. |
| TZ database | `parse_fixed_offset` accepts only a numeric offset or `Z`; IANA names are rejected by the schema `pattern` before the handler runs. No zone rules, no DST transitions, no leap seconds. |
| Environment | No `std::env` read anywhere in the core. The only env influence is the server-wide `EGGCALC_MCP_PROFILE` (see [mcp-server.md](mcp-server.md)), which selects tool visibility, not time behaviour. |
| Network | No transport, no resolver. |

Because the offset is fixed, a "local wall-clock time" derived from `after` is unambiguous: there is no instant at which it repeats or does not exist. This is why `search_next` can re-apply `after.offset()` to every generated result without any zone-transition handling.

Identical `(expression, after, count)` therefore produce byte-identical output within one eggsact version.

## Datetime Helpers (`mod.rs`)

| Function | Signature | Notes |
|----------|-----------|-------|
| `NANOS_PER_SECOND` | `pub const NANOS_PER_SECOND: i128 = 1_000_000_000` | The only shared constant. |
| `parse_rfc3339` | `fn(&str) -> Result<OffsetDateTime, String>` | Delegates to `time`'s well-known `Rfc3339` description. Error prefix: `Invalid RFC 3339 timestamp: {error}`. |
| `format_rfc3339` | `fn(OffsetDateTime) -> Result<String, String>` | Error prefix: `Unable to format RFC 3339 timestamp: {error}`. |
| `parse_fixed_offset` | `fn(&str) -> Result<UtcOffset, String>` | Fixed-offset parsing, below. |
| `unix_nanos` | `fn(OffsetDateTime) -> i128` | Thin wrapper over `OffsetDateTime::unix_timestamp_nanos()`. |
| `unix_unit` | `fn(i128, i128) -> i128` | `nanos.div_euclid(unit)` — **floor**, not truncation. |
| `date_components` | `fn(OffsetDateTime) -> serde_json::Value` | `year`, `month` (as `u8`), `day`, `hour`, `minute`, `second`, `nanosecond`, `weekday`. |
| `weekday_name` | `fn(Weekday) -> &'static str` | `SUN`, `MON`, `TUE`, `WED`, `THU`, `FRI`, `SAT` — Vixie abbreviations. |

### Accepted offset formats

`parse_fixed_offset` is a hand-rolled six-byte parser, not a `time` format:

1. `"Z"` → `UtcOffset::UTC`.
2. Otherwise the input must be exactly 6 bytes, start with `+` or `-`, have `:` at index 3, and have ASCII digits at indices 1–2 and 4–5. Failure → `offset must be exactly Z or +HH:MM/-HH:MM`.
3. `hours > 23 || minutes > 59` → `offset is outside the fixed-offset range: {value}`.
4. `UtcOffset::from_hms(sign * hours, sign * minutes, 0)`; the sign is applied to **both** components, so `-00:30` is UTC−00:30, not a sign error. Residual failure → `invalid fixed offset: {error}`.

There is deliberately no `+05`, `+0530`, `UTC`, `Etc/UTC`, or `+25:00` form. The schema `pattern` `^(Z|[+-][0-9]{2}:[0-9]{2})$` enforces the same shape at the MCP boundary, and rejects `Europe/Paris` with JSON-RPC `-32602` before the handler is entered.

### Round-trip behaviour

`datetime_convert` normalises every input to an instant, then projects it twice: `rfc3339` uses the caller-selected offset (or the input's own offset when `output_offset` is absent), and `utc_rfc3339` always uses UTC. The integer fields are emitted as **JSON strings**, not numbers, because `i128` has no lossless JSON number form. `unix_unit`'s `div_euclid` floor semantics mean a pre-epoch instant reports the lower whole unit: `-1` ns yields `unix_seconds: "-1"`, not `"0"`. `unix_nanoseconds` is the authoritative field — every other representation is derived from it, and the property test `datetime_nanosecond_round_trip_preserves_the_instant` pins that `rfc3339 → unix_nanoseconds → rfc3339` is identity on both `utc_rfc3339` and `unix_nanoseconds` for sub-second, non-UTC, and leap-day inputs.

## Cron Parser

### The five-field model

`parse(expression)` requires exactly five whitespace-separated fields, `minute hour day-of-month month day-of-week`, and returns a `CronSchedule` of five `CronField` values:

```rust
pub struct CronField {
    pub values: Vec<u32>,   // sorted ascending, deduplicated
    pub min: u32,
    pub max: u32,
    pub star_syntax: bool,  // input.starts_with('*')
}
pub struct CronSchedule {
    pub minute: CronField, pub hour: CronField, pub day_of_month: CronField,
    pub month: CronField,  pub day_of_week: CronField,
}
```

Membership (`CronField::allows`) is a range check plus `binary_search` over `values`, so it is `O(log n)` and the vector's ascending invariant is load-bearing.

| Field | Range | Names | Special |
|-------|-------|-------|---------|
| minute | `0..=59` | — | — |
| hour | `0..=23` | — | — |
| day-of-month | `1..=31` | — | — |
| month | `1..=12` | `JAN`…`DEC` | case-insensitive |
| day-of-week | `0..=7` | `SUN`…`SAT` | `7` aliases to `0` |

Accepted per-field syntax:

- **Single value** — `5`.
- **List** — `1,15`, comma-separated; empty items rejected.
- **Range** — `1-5`, `MON-FRI`. Non-wrapping; `5-1` is an error.
- **Step** — `*/2`, `1-31/2`, `5/2` (open-ended from `5` to the field max). Steps must be positive integers and contain only one `/`.
- **Names** — matched case-insensitively *before* numeric parsing, so `mon`, `Mon`, and `MON` are equivalent. Names are legal anywhere a value is (including range ends).

`star_syntax` is set from `input.starts_with('*')` on the field's whole text, so `*` and `*/2` and even `*,5` are star-syntax, while `1-31`, `0-7`, and `1-31/2` are not.

### Rejected forms and error vocabulary

`parse` returns `Err(String)`; the adapter maps every one of these to machine code `INVALID_ARGUMENTS`.

| Input | Message |
|-------|---------|
| `@daily` (any `@`-prefixed token) | `cron nicknames such as @daily are not supported` |
| Field count ≠ 5 | `cron expression must contain exactly five fields` |
| 5-field expression containing `CRON_TZ`, or starting with `TZ=` | `cron timezone prefixes are not supported` |
| Empty field | `cron fields cannot be empty` |
| `1,,2` or trailing comma | `cron lists cannot contain empty items` |
| `*/2/3` | `invalid cron step expression: {item}` |
| `*/x` | `invalid cron step: {step}` |
| `*/0` | `cron step must be positive` |
| `MON-FRI-FRI` | `invalid cron range: {range_part}` |
| `5-1` | `cron ranges do not wrap: {range_part}` |
| `XYZ`, `MON-FOO` | `invalid cron value: {token}` |
| Value > `u32::MAX` | `cron value is too large: {token}` |
| `0 0 * * 8` | `cron value 8 is outside 0..7` |
| Empty resolved set | `cron field must allow at least one value` |

**Ordering caveat worth knowing:** the five-field check runs *before* the timezone-prefix check. A conventional prefix form such as `CRON_TZ=UTC 0 0 * * *` has six fields and therefore reports `cron expression must contain exactly five fields`, not the timezone message. The prefix guard only fires for an already-five-field expression that contains `CRON_TZ` (e.g. `0 0 CRON_TZ * *`) or begins with `TZ=`. Both paths still reject the input, so this is a message-selection detail, not a hole.

## Cron Matching Semantics

This is the highest-value contract in the core. `day_matches(schedule, date)` (`cron.rs:193-209`) first requires `month` to allow the date's month, then computes two independent predicates and combines them:

```rust
let dom = schedule.day_of_month.allows(date.day());
let dow = schedule.day_of_week.allows(date.weekday().number_days_from_sunday());
if schedule.day_of_month.star_syntax || schedule.day_of_week.star_syntax {
    dom && dow          // at least one field is star-syntax → AND
} else {
    dom || dow          // neither is star-syntax       → OR
}
```

`number_days_from_sunday()` maps Sunday→0 … Saturday→6, which is why the DOW field's parsed `7` was already folded to `0` at parse time.

### DOM/DOW truth table

`S` = the field's text starts with `*`. All rows verified against `src/temporal/cron.rs` and reproduced through the running server.

| Expression | DOM text | DOW text | DOM `S` | DOW `S` | Rule | Dates that match |
|------------|----------|----------|---------|---------|------|------------------|
| `0 0 * * MON` | `*` | `MON` | yes | no | AND | Mondays only |
| `0 0 1 * *` | `1` | `*` | no | yes | AND | 1st of the month only |
| `0 0 * * *` | `*` | `*` | yes | yes | AND | every day (both are full sets) |
| `0 0 */1 * MON` | `*/1` | `MON` | yes | no | AND | Mondays only |
| `0 0 1 * */1` | `1` | `*/1` | no | yes | AND | 1st of the month only |
| `0 0 */2 * MON` | `*/2` | `MON` | yes | no | AND | Mondays whose DOM ∈ {1,3,…,31} |
| `0 0 1 * */2` | `1` | `*/2` | no | yes | AND | 1st **and** DOW ∈ {0,2,4,6} (Sun/Tue/Thu/Sat) |
| `0 0 * * */2` | `*` | `*/2` | yes | yes | AND | DOW ∈ {0,2,4,6} |
| `0 0 1 * MON` | `1` | `MON` | no | no | OR | 1st of the month **or** any Monday |
| `0 0 1-31 * MON` | `1-31` | `MON` | no | no | OR | every day (DOM already admits all) |
| `0 0 1 * 0-7` | `1` | `0-7` | no | no | OR | every day (DOW already admits all) |

**The rule in one sentence:** look at the *text* of the DOM and DOW fields, not their value sets — if either begins with `*` (a bare `*` or a `*/n` step) both predicates must hold, otherwise either one suffices.

**The trap this creates.** `*/1` and `1-31` have *identical* parsed value sets for day-of-month, yet they sit on opposite sides of the switch:

- `0 0 * * 1` → DOM is star-syntax → **AND** → Mondays only.
- `0 0 */1 * 1` → DOM is star-syntax → **AND** → Mondays only.
- `0 0 1-31 * 1` → neither is star-syntax → **OR** → **every day**.

Bare `*` behaves as the familiar wildcard not because of special-casing but because its value set already contains every value; an explicit full range is *not* an equivalent spelling and inverts the DOM/DOW combination. A reader implementing a compatible matcher must therefore compare the raw field text, never the expanded value list.

This matches the contract recorded in `AGENTS.md` and in [tools.md](tools.md) (temporal section). The only refinement worth recording: "starts with `*`" is evaluated on the entire field string, so a mixed list such as `*,5` is star-syntax, and a stepped *range* such as `1-31/2` is not, even when it happens to admit every value.

## Bounded Next-Match Search

`search_next(schedule, after, count) -> Result<Vec<String>, Box<ToolResponse>>` (`cron.rs:211-261`):

1. Takes a `ToolBudget::MODERATE` handler budget via `budget::for_handler` and polls it once per day.
2. Captures `offset = after.offset()` and starts at `date = after.date()`.
3. Walks **calendar days only**. For each day where `day_matches` holds, it iterates `hour.values` (outer) then `minute.values` (inner) in ascending order, builds `date.with_time(Time::from_hms(h, m, 0)).assume_offset(offset)`, and keeps the result only when `local > after` — the comparison is **strict**, so a match at the reference instant itself is excluded.
4. Returns as soon as `result.len() == count`.
5. Otherwise advances with `date.next_day()`.

**The bound is `const MAX_DAYS: usize = 146_097`** (one Gregorian 400-year cycle), and the loop is `for _ in 0..=MAX_DAYS`, so **146,098 distinct dates** are examined at most — the start day plus 146,097 further days. The loop can never run unbounded: it either fills `count` or exits.

Consequences of that bound:

- **Fewer than `count` is normal.** `Ok(result)` is returned with fewer items, possibly zero, when the schedule cannot fill the request inside the window. `count` in the response is `next_runs.len()`, i.e. the number actually returned, **not** the number requested.
- **An unsatisfiable schedule is a success, not an error.** `0 0 30 2 *` (February 30th) returns `ok: true` with `next_runs: []`, `count: 0`, and `satisfiable: false`. Use `satisfiable` to distinguish "no match inside the window" from "no match requested".
- **Calendar-range exhaustion is an error.** If `date.next_day()` returns `None` (the `Date` ceiling), the call returns `ToolResponse::error_with_code("invalid_arguments", INVALID_ARGUMENTS, "cron search exceeded the supported calendar range", None, Some("cron_inspect"))`.
- **Ordering is guaranteed.** Days ascend, and within a day hours then minutes ascend, so `next_runs` is strictly increasing. Combined with the strict `>` filter, no instant can repeat.

`satisfiable(schedule, after) -> bool` is a lighter day-level probe: it uses a `ToolBudget::CHEAP` budget, scans the same 146,098-date bound, and returns `true` on the first day-level match. It ignores time-of-day entirely, so it answers "can this *day* predicate ever be satisfied from `after` onward", not "will `count` results be returned".

Both functions take a handler budget, which is why `temporal` appears as a narrow exception to the leaf-core layering rule recorded in [overview.md](overview.md) and [text-library.md](text-library.md).

## The Two Tools

Both are `tier: 2`, `profiles: ["full"]`, `exposure: ToolExposure::Contextual`, `harness_use: ["none"]`, `aliases: []`, `stability: ToolStability::Stable`, `composite: false`, `category: "temporal"`. They are `full`-profile only, so they are absent from the `default` profile's model-facing list.

Two validation layers apply. **Schema-level** violations (pattern, `minimum`/`maximum`, missing required, wrong JSON type) are rejected by the MCP boundary as JSON-RPC `-32602` before the handler runs. **Handler-level** checks in `text_arg` / `count_arg` are the in-process library path and carry the vocabulary below.

### `datetime_convert` (`ToolCost::Cheap`)

Input:

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `value` | string | yes | `maxLength: 100000`; the timestamp **as text**, never a JSON number. |
| `format` | string | yes | `enum: ["rfc3339","unix_seconds","unix_milliseconds","unix_nanoseconds"]`. |
| `output_offset` | string | no | `pattern: ^(Z\|[+-][0-9]{2}:[0-9]{2})$`; no IANA names. Defaults to the parsed input's own offset. |

Output:

| Field | Type | Meaning |
|-------|------|---------|
| `rfc3339` | string | Canonical RFC 3339 in `output_offset` (or the input's offset). |
| `utc_rfc3339` | string | The same instant rendered in UTC. |
| `unix_seconds` | string | Floor whole seconds (`div_euclid`). |
| `unix_milliseconds` | string | Floor whole milliseconds. |
| `unix_nanoseconds` | string | Authoritative nanosecond value. |
| `offset_seconds` | integer | `selected_offset.whole_seconds()` (e.g. `19800` for `+05:30`). |
| `selected_offset` | string | `UtcOffset::to_string()` — note the `+05:30:00` seconds-suffixed form, which differs from the `+05:30` input spelling. |
| `components` | object | `year`, `month`, `day`, `hour`, `minute`, `second`, `nanosecond`, `weekday` in the **selected** offset. |

Handler-level error strings: `format must be rfc3339, unix_seconds, unix_milliseconds, or unix_nanoseconds`; `output_offset must be a string, got {type}`; `{format} must be a signed decimal integer string`; `{format} is outside the supported integer range`; `unix seconds overflow nanosecond conversion`; `unix milliseconds overflow nanosecond conversion`; `unix timestamp is outside the supported range: {error}`; `timestamp is outside the supported range: {error}`; `{field} must be a string, got {type}` / `got NoneType`; `{field} length {n} bytes exceeds 100000`.

### `cron_inspect` (`ToolCost::Moderate`)

Input:

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `expression` | string | yes | `maxLength: 100000`; bounded five-field Vixie/POSIX-style cron. |
| `after` | string | yes | `maxLength: 100000`; mandatory RFC 3339 reference instant. |
| `count` | integer | no | `minimum: 1`, `maximum: 32`, `default: 5`. Absent → `5`. |

Output:

| Field | Type | Meaning |
|-------|------|---------|
| `expression` | string | The expression exactly as supplied (not normalized). |
| `normalized_expression` | string | Five fully expanded decimal sets, e.g. `0 9 1,2,3,…,31 1,2,3,4,5 1,2,3,4,5`. Potentially very long. |
| `parsed_values` | object | `minute`, `hour`, `day_of_month`, `month`, `day_of_week` arrays (DOW `7` already folded to `0`). |
| `offset` | string | `after.offset().to_string()`, e.g. `-04:00:00`. |
| `offset_seconds` | integer | `after.offset().whole_seconds()`. |
| `satisfiable` | boolean | Day-level satisfiability probe from `after` onward. |
| `next_runs` | array of string | RFC 3339 instants, strictly increasing, all in `after`'s offset. |
| `count` | integer | `next_runs.len()` — the number returned, not the number requested. |

Handler-level error strings: `count must be between 1 and 32`; `count must be an integer between 1 and 32, got {type}`; plus every parser message from the table above, all wrapped as `invalid_arguments`.

### Machine codes emitted

| Code | Source | Condition |
|------|--------|-----------|
| `INVALID_ARGUMENTS` | `src/tools/temporal.rs:11-19` (`invalid()`) and `cron.rs:248-256` | Any schema-valid but semantically rejected input, including the calendar-range exhaustion error. |
| `INPUT_TOO_LARGE` | `text_arg` (`tools/temporal.rs:28-39`) | A string argument exceeding `MAX_TEXT_LENGTH` = 100,000 **bytes** (`src/tools/helpers.rs:15`). |
| `CANCELLED` | `budget::check_not_cancelled` via `cron.rs:222-226` | The MCP server's cooperative cancel flag was set during the `search_next` day walk. |
| `TIMEOUT` | `budget::check_deadline` via the same path | The `MODERATE` budget's 30 s elapsed deadline expired during the walk. |

Schema-level rejections surface as JSON-RPC `-32602` with the underlying schema message, not as a tool `machine_code`. `datetime_convert` declares `ToolCost::Cheap` and never touches a budget, so it cannot emit `CANCELLED`/`TIMEOUT`; `cron_inspect` declares `ToolCost::Moderate` and the core's `for_handler(ToolBudget::MODERATE)` matches it.

## Determinism and Test Coverage

Twelve tests pin this behaviour, all green at HEAD (`cargo test --locked --lib temporal` → 6 passed; `cargo test --locked --test lib test_utility_properties` → 6 passed).

**Parser and DOM/DOW matrix** — `src/temporal/cron.rs:290-407`:
- `dom_and_dow_use_star_syntax_flags` pins cases 1–4 and 9–11 of the truth table, including that `1-31` and `0-7` carry `star_syntax == false` and that the Sunday `0`/`7`/`SUN` spellings normalize alike.
- `star_step_fields_carry_star_syntax_and_use_and` pins `*/1`, `*/2` on both DOM and DOW, `* * */2`, and the Sunday `0`/`7`/`SUN` equivalence under AND.

**Adapter behaviour** — `src/tools/temporal.rs:236-283`:
- `datetime_handles_epoch_offsets_and_negative_fraction` pins epoch zero, an explicit `-04:00 → Z` conversion, and the floor rule (`-1` ns → `unix_seconds "-1"`).
- `cron_names_and_strict_after` pins `MON-FRI` names and the strict `>` filter against the reference instant.
- `cron_dom_dow_use_or_semantics` pins the OR branch end-to-end through `cron_inspect` (`0 0 1 * MON` → the two following Mondays).
- `cron_rejects_non_five_field_forms` pins that `@daily` and six-field expressions both fail.

**Properties** — `tests/property/test_utility_properties.rs`:
- `cron_results_are_ordered_and_strictly_after_the_reference` pins ascending lexicographic order and the strict-after rule for `*/17 * * * *` at the maximum `count: 32`.
- `cron_results_satisfy_independent_dom_dow_rules` re-implements the DOM/DOW rule as ten independent `fn(u8, u8) -> bool` predicates **derived from case metadata, not from `day_matches()`**, and asserts every returned instant satisfies them. The comment there is explicit that `*/1` is the case a value-coverage check would get wrong. This is the anti-oracle test for the truth table above.
- `datetime_nanosecond_round_trip_preserves_the_instant` pins the round-trip.

**Fuzzing** — `fuzz/fuzz_targets/cron_inspection.rs` drives `cron_inspect` with arbitrary `expression` bytes against the fixed reference `2026-09-03T11:00:00Z` and `count: 1`. The fixed reference is what keeps the fuzz target inside the bounded search window.

**Not pinned.** Two behaviours are verified by reading source and by manual server invocation, but have no dedicated test: (a) the `*/1` vs `1-31` OR/AND inversion in the truth table, and (b) the `count: 99` / `count: 0` schema rejections and the `output_offset` pattern rejection, which are exercised only through the MCP boundary. Adding a test for (a) is the single highest-value follow-up; `*/1` is exactly the input a value-coverage-based test would mis-classify.

## Invariants and Review Checklist

When touching `src/temporal/`, verify each of these:

1. **No clock, no TZ db, no env, no net.** Any new `Instant::now()`, `SystemTime`, `env::var`, or zone lookup is a contract break.
2. **`after` stays mandatory.** Do not add a default "now" fallback to `cron_inspect`.
3. **`star_syntax` is `input.starts_with('*')` on the raw field text.** Do not derive it from the expanded value set, and do not normalise `1-31` to `*` — the DOM/DOW AND/OR switch depends on the distinction.
4. **Preserve `values` sorted and deduplicated.** `allows()` uses `binary_search`.
5. **Keep `MAX_DAYS`.** The 400-year bound is what makes `search_next` terminate; never replace it with an unbounded scan or a `loop`.
6. **Keep the strict `local > after` filter.** Changing it to `>=` alters every pinned test.
7. **Keep results in `after`'s offset.** `assume_offset(offset)` is what keeps output free of DST ambiguity.
8. **Keep the hardcoded `"NoneType"` in `text_arg` missing-argument messages** unless a compat-mode sweep is done — the rest of the codebase uses mode-aware `json_type_name()`.
9. **Register in one place.** A `ToolSpec` in `src/mcp/specs/temporal.rs` is the single source of truth; `tool_registration_tables_are_in_sync` catches drift. After any registry/profile/exposure change run `cargo run --locked --features dev-tools --bin generate-docs`.
10. **Re-run the pinned tests** after any semantic change: `cargo test --locked --lib temporal` and `cargo test --locked --test lib test_utility_properties`.

## Related drift

Five documentation errors were found while verifying this document against HEAD
and have been fixed in place. Recorded because each is easy to regress:

- `tools.md` listed `temporal` among the "leaf utilities without a tier". That
  contradicted `search_next` (`budget::for_handler(ToolBudget::MODERATE)`) and
  `satisfiable` (`ToolBudget::CHEAP`) in `src/temporal/cron.rs`. Note the budget
  lives in the **core**, not in the `cron_inspect` handler, which is why a
  handler-scoped table can look correct at a glance.
- `tools.md` and `text-library.md` both said the search covers "146,097 days".
  The loop is `for _ in 0..=MAX_DAYS` with `MAX_DAYS = 146_097`, so 146,098 dates
  are examined. No test or behaviour depends on the off-by-one; the two numbers
  should simply not be used interchangeably.
- `text-library.md` said `CRON_TZ` / `TZ=` prefixes are rejected, without saying
  which error appears. `CRON_TZ=UTC 0 0 * * *` has six fields, so the five-field
  check at `cron.rs:157` fires first and the caller sees the field-count error,
  not the timezone message. The prefix guard at `cron.rs:160` only fires for an
  already-five-field expression. Both paths were verified against the running
  server.
- `testing.md` described the `cron_inspection` fuzz target as covering "DOM/DOW
  star-syntax rules". The target only feeds arbitrary `expression` bytes and
  discards the result; it asserts nothing. It catches panics and
  non-termination, nothing more. The DOM/DOW rules are pinned by the ordinary
  and property tests, not by fuzzing.
- The temporal deep-dive pointer in `overview.md` targeted `tools.md`. It now
  targets this file.

One wording refinement to `AGENTS.md`'s cron contract, which is otherwise
accurate: `star_syntax` is evaluated on the whole field string, so `*,5` **is**
star-syntax while `1-31/2` is not. The documented rule is right; the edge is just
worth stating.
