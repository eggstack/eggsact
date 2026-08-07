# Phase 2 — Path and Wire-Boundary Corrections

## Status

- **Status:** complete
- **Repository:** `eggstack/eggsact`
- **Target branch:** `main`
- **Roadmap:** `plans/2026-08-04-bounded-correctness-simplification-roadmap.md`
- **Roadmap commit:** `2211ebb3adae4df6551023676047d018e113a4f7`
- **Depends on:** Phase 1 execution-context soundness
- **Priority:** high
- **Scope:** correct UNC/share lexical boundaries, enforce the declared MCP request limit before unbounded allocation, correct finding-truncation accounting, and classify capacity failures truthfully
- **Expected change size:** medium, localized to path utilities, MCP input loop/protocol helpers, response truncation, and focused tests

## Objective

Repair four bounded correctness contracts without redesigning path handling or MCP transport.

After this phase:

1. Windows UNC host/share prefixes are represented structurally and cannot be removed by `..` collapse;
2. path scope checks cannot treat a target that traverses above a UNC share as inside the root;
3. an oversized unterminated JSONL request is rejected without first allocating the complete line;
4. findings-truncation notices report the exact omitted count for all caps, including zero and one;
5. rate/capacity pressure is not mislabeled as a malformed JSON-RPC request;
6. the local stdio server retains bounded in-flight and worker concurrency without adding remote-service hardening machinery.

---

# Hard constraints

This phase must not:

- replace lexical path tools with filesystem canonicalization;
- access the host filesystem to decide path scope;
- add a third-party path or virtual-filesystem crate;
- implement every Windows namespace/device path variant unless required by an existing documented contract;
- broaden path tools into symlink or mount-boundary analysis;
- replace JSONL stdio MCP transport;
- add network framing, HTTP, sockets, or authentication;
- build a generic streaming codec framework;
- increase request/output limits;
- add a new rate-limiting library;
- add a new CI workflow or fuzz target solely for these corrections;
- change unrelated tool response schemas.

Use small private helpers and existing types.

---

# Files to inspect first

At minimum inspect:

```text
src/text/path.rs
src/tools/path.rs
src/tools/patch.rs
src/tools/repo.rs
src/mcp/server.rs
src/mcp/runtime.rs
src/mcp/protocol.rs
src/mcp/response.rs
src/mcp/machine_codes.rs
tests/text/test_path.rs
tests/mcp/
tests/property/
architecture/text-library.md
architecture/mcp-server.md
architecture/budget-concurrency.md
```

Search for:

```text
path_normalize
path_scope_check
path_batch_scope_check
patch_contract_check
is_unc_track
starts_with("\\\\")
MAX_REQUEST_BYTES
next_line
read_line
read_until
truncate_response
findings_truncated
RateLimiter
MAX_REQUESTS_PER_SECOND
CapacityExceeded
invalid_request
RESOURCE_EXHAUSTED
```

List all consumers of `path_scope_check()` before changing semantics.

---

# Workstream 1 — Represent Windows path prefixes by structure, not component text

## Current defect

UNC handling currently uses a mutable boolean and tests whether path components literally equal `server` or `share`. Host and share names are arbitrary. The semantic prefix is defined by position:

```text
\\host\share\remaining\components
```

Dot-segment collapse must never pop `host` or `share`.

## Required internal model

Introduce the smallest internal representation that separates prefix from collapsible components. Recommended conceptual shape:

```rust
enum WindowsPrefix<'a> {
    None,
    DriveRelative { drive: &'a str },
    DriveRooted { drive: &'a str },
    Unc { host: &'a str, share: &'a str },
}
```

An owned variant or smaller helper struct is acceptable. Do not publish a new public path type unless unavoidable.

Required distinctions:

- `C:foo` is drive-relative, not absolute;
- `C:\foo` and `C:/foo` are drive-rooted absolute paths;
- `\\host\share` and `//host/share` are UNC absolute paths;
- a UNC path missing host or share is malformed/incomplete and must not be normalized into a misleading valid share root;
- prefix components are stored separately from ordinary components;
- `..` may pop only ordinary components after the protected prefix;
- `..` at the share root remains visible as an attempted escape or causes an explicit boundary warning/result, rather than rewriting the share identity.

Do not use component names such as `server` or `share` as sentinels.

## Lexical behavior decision

For traversal above an absolute root/share, choose one consistent bounded behavior:

- clamp at the protected root while preserving `escapes_via_dotdot = true`; or
- retain unresolved `..` and make `inside_root = false`.

For security/scope checks, the final result must be outside/not allowed. Normalization output may clamp for display only if findings preserve the attempted escape unambiguously.

Document the selected lexical contract in the existing path documentation.

## Required tests

Add exact tests for:

```text
\\host\share\dir\..\file        -> within the same share
\\host\share\..\secret          -> escape attempt; never becomes \\host\secret
\\host\share\dir\..\..\secret  -> escape attempt at share boundary
//host/share/dir/../file           -> normalized UNC separators
\\server\documents\..\secret    -> proves names are arbitrary, not sentinel text
\\host\share                      -> valid share root
\\host                             -> incomplete UNC prefix
C:foo                               -> drive-relative
C:\foo                             -> drive-rooted absolute
C:/foo/../bar                       -> C:\bar
```

Add `path_scope_check()` tests with an UNC root and both inside/escape targets. Add one consumer-level regression for the highest-risk path preflight that uses scope checking; do not duplicate every consumer test.

## Acceptance criteria

- no UNC protection logic depends on literal component text;
- UNC normalization cannot change the share name by collapsing `..`;
- scope checks reject traversal above the share root;
- drive-relative paths remain non-absolute and are not silently resolved under an unrelated rooted path;
- existing POSIX and ordinary Windows tests remain green.

---

# Workstream 2 — Bound MCP line allocation before parsing

## Current defect

The server calls `BufRead::next_line()` and only afterward checks `MAX_REQUEST_BYTES`. The processing limit is one MiB, but allocation can exceed that limit before the check when a client sends an oversized unterminated line.

## Required implementation

Add one private bounded JSONL reader helper in or near `server.rs`. It must:

- read incrementally from `AsyncBufRead` using `fill_buf`/`consume`, a bounded chunk loop, or another Tokio API that does not allocate the full line first;
- retain at most `MAX_REQUEST_BYTES + a small detection byte/chunk` in the request buffer;
- detect newline and return one complete logical line without the newline terminator;
- handle LF and CRLF consistently with current behavior;
- distinguish clean EOF with no buffered data from EOF terminating a final non-newline request;
- drain or discard the remainder of an oversized line through the next newline before processing the next request;
- emit exactly one request-too-large error per oversized line;
- avoid parsing partial oversized JSON;
- leave subsequent valid requests usable.

Recommended result shape:

```rust
enum LimitedLine {
    Line(String),
    TooLarge { observed_at_least: usize },
    Eof,
}
```

Exact names are not important. Do not build a reusable codec crate or generic framed transport.

## Required tests

Use the nearest async unit/integration layer to cover:

1. normal short request with LF;
2. normal short request with CRLF;
3. exactly `MAX_REQUEST_BYTES` accepted according to the documented byte definition;
4. limit plus one rejected;
5. multi-megabyte unterminated input does not return a giant retained line;
6. oversized line followed by a valid line yields one size error and then processes the valid request;
7. EOF after a final non-newline valid request;
8. empty lines remain ignored.

The test does not need to prove allocator internals. It must exercise a helper whose buffer length is structurally capped and assert the cap/detection behavior.

## Error contract

Keep the existing human-readable maximum. Use parse error `-32700` only if that is the current documented contract for oversized wire input; otherwise prefer a server-defined resource/input-limit error. Do not label it `-32600` merely because it cannot be processed.

Document the chosen code once.

## Acceptance criteria

- `next_line()` or an equivalent unbounded whole-line allocation is absent from the production request loop;
- the helper's retained request buffer is structurally bounded;
- an oversized line is drained before the next request;
- the server remains usable after rejection.

---

# Workstream 3 — Correct findings truncation accounting and edge cases

## Current defect

When `findings.len() > max_findings`, the implementation reserves one slot for a synthetic notice and keeps `max_findings - 1` real findings, but calculates omitted count as `original_len - max_findings`. The notice is therefore off by one.

## Required implementation

Capture the original length before mutation. Calculate:

```text
real_cap = max_findings.saturating_sub(1)
omitted = original_len - real_cap
```

Then construct output according to an explicit edge contract.

Required edge behavior:

- `max_findings >= 1`: retain up to `max_findings - 1` highest-severity real findings and append one notice;
- `max_findings == 0`: return zero findings; record truncation in `limits_applied`; do not exceed the cap by inserting a notice;
- the notice's omitted count equals the number of removed real findings;
- stable severity ordering remains deterministic for ties according to existing behavior; do not add a broad sorting redesign.

## Required tests

Add table-driven cases for caps `0`, `1`, `2`, and a larger cap, asserting:

- total output length never exceeds cap;
- exact retained real finding count;
- exact omitted count;
- correct `limits_applied` value;
- no truncation when original length is within cap.

## Acceptance criteria

- omitted count is exact;
- cap zero does not emit a synthetic finding;
- existing route-critical verdict preservation remains unchanged;
- no response-schema redesign occurs.

---

# Workstream 4 — Truthful resource-pressure errors and local rate-limit decision

## Current defect

Duplicate/malformed request handling and capacity pressure share `invalid_request(-32600)` paths. In-flight capacity and request-rate exhaustion are not malformed JSON-RPC requests.

## Required classification

Add or reuse a server-defined JSON-RPC error code in the `-32000` through `-32099` range with structured data where useful. Recommended data codes:

```text
RESOURCE_EXHAUSTED
RATE_LIMITED
TOO_MANY_IN_FLIGHT
```

Reuse existing machine-code constants rather than multiplying constants when semantics already match.

Required behavior:

- malformed structure remains `-32600`;
- unknown methods remain `-32601`;
- invalid tool arguments remain `-32602`;
- internal failures remain `-32603` or the established server error;
- queue/in-flight exhaustion is a server/resource error;
- any retained request-rate rejection is a server/resource error.

## Decide whether the fixed request-rate limiter remains

Evaluate the actual local stdio threat model:

- input is one local process pipe;
- `MAX_IN_FLIGHT_REQUESTS` bounds active request tasks;
- tool semaphore/worker limits bound blocking execution;
- request bytes and output bytes are bounded;
- queue/resource errors are available.

Preferred outcome: remove the fixed 10 request/second limiter if these bounds are sufficient. Agent clients commonly issue legitimate bursts, and a wall-clock request rate is not needed for an internet-facing abuse boundary because eggsact is not one.

Retaining the limiter is acceptable only with a product reason independent of tests or generic production-hardening instinct. If retained:

- keep it simple;
- classify rejection truthfully;
- document why local callers need it;
- do not add token buckets, per-client identities, configuration files, or metrics.

## Required tests

- in-flight capacity produces the selected server/resource code and preserves request ID;
- malformed request still produces `-32600`;
- if limiter removed, a deterministic burst within in-flight capacity is accepted or bounded only by the existing capacity controls;
- if limiter retained, a deterministic unit test covers its boundary without sleeps.

## Acceptance criteria

- resource pressure is not returned as `-32600`;
- the rate limiter is removed or narrowly justified;
- no new rate-limit subsystem or configuration surface is added.

---

# Workstream 5 — Documentation reconciliation

Update only affected documents:

```text
CHANGELOG.md
architecture/text-library.md
architecture/mcp-server.md
architecture/budget-concurrency.md
docs/mcp-tools.md            # only if a public path/wire contract is described there
```

Required statements:

- path operations are lexical and UNC share roots are protected boundaries;
- filesystem symlinks/mounts remain outside the tool's claim;
- request byte limit is enforced during reading, not only after allocation;
- resource errors are distinguished from malformed requests;
- rate-limit policy reflects the final bounded decision.

Do not create a new security or transport architecture document.

---

# Rejection searches

Before completion, search for and disposition:

```text
part == "server"
part == "share"
is_unc_track
lines.next_line()
findings.len().saturating_sub(budget.max_findings)
invalid_request("Too many in-flight requests"
invalid_request("Rate limit exceeded"
```

Literal `server`/`share` text may remain in tests/examples, but not as UNC prefix logic.

---

# Execution order for a smaller implementation agent

1. Sync to latest `origin/main`; confirm Phase 1 completion.
2. Add UNC share-boundary regression tests.
3. Refactor Windows prefix parsing and dot-segment collapse only enough to pass those tests.
4. Add the highest-risk consumer-level scope regression.
5. Extract a bounded JSONL line-reader helper and unit-test it independently.
6. Replace the production read loop and add recovery-after-oversize coverage.
7. Fix findings truncation accounting with cap-zero/one tests.
8. Introduce truthful server/resource errors and decide the fixed rate limiter.
9. Run targeted path, response, protocol, and server tests.
10. Run the ordinary verification gate.
11. Update bounded documentation and this completion record once.

Do not combine timeout/test-isolation work from Phase 3 into this phase.

---

# Verification

Targeted commands should be adapted to discovered test names. At minimum:

```bash
cargo test --locked --all-features test_path
cargo test --locked --all-features path_scope
cargo test --locked --all-features truncate_findings
cargo test --locked --all-features request
cargo test --locked --all-features rate_limit
cargo test --locked --all-features capacity
```

Then run:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity
cargo test --locked --doc
cargo run --locked --features dev-tools --bin generate-docs -- --check
```

Run existing path fuzz targets manually only if the path parser changes substantially. Do not add them to ordinary CI.

---

# Acceptance checklist

- [ ] UNC host/share prefix is structural and independent of component names.
- [ ] Dot-segment collapse cannot pop a UNC host or share.
- [ ] UNC scope escapes are rejected.
- [ ] Drive-relative and drive-rooted paths remain distinct.
- [ ] MCP request buffering is capped before full-line allocation.
- [ ] Oversized lines are drained and the next valid request is processed.
- [ ] Findings truncation reports the exact omitted count.
- [ ] `max_findings = 0` and `1` obey the total cap.
- [ ] Capacity/rate failures use truthful server/resource errors.
- [ ] Fixed request-rate limiting is removed or narrowly justified without expansion.
- [ ] No filesystem access, new dependency, transport rewrite, or new subsystem was added.
- [ ] Focused tests and ordinary verification pass.
- [ ] Documentation matches the corrected contracts.

---

# Completion record

- **Implementation commit(s):** `76ec421` (path/wire corrections — UNC, bounded reader, truncation, rate-limit); `a3f78e3` (companion corrective — `read_bounded_line()` rewrite, Windows drive-relative `WindowsPrefix` classifier)
- **Windows prefix design:** private `WindowsPrefix` enum with `_classify_windows_prefix()` classifier; used in `path_analyze` (absolute check + warning) and `path_scope_check` (conservative rejection of drive-relative)
- **UNC escape disposition:** UNC host/share prefixes are structural position-only; dot-segment collapse cannot pop above share root
- **Bounded reader design:** `read_bounded_line()` uses only `fill_buf()`/`consume()`; line-length tracking via `payload_total` + `tentative` counters; CRLF detection across chunks via `last_byte` state
- **Truncation edge behavior:** `max_findings = 0` reserves no slot; `max_findings = 1` shows only the notice; omission count is exact
- **Rate limiter disposition:** fixed 10 req/s limiter removed; bounded in-flight (32) and worker (16) limits retained
- **Resource error code(s):** `RESOURCE_EXHAUSTED` (-32004) for rate/capacity, distinct from `-32600`
- **Targeted tests:** 9 bounded-reader tests (buffered_cursor helpers, exactly-cap, CRLF split, oversized-then-valid), 11 path tests (WS3-1 through WS3-11), UNC share-root tests — all pass
- **Ordinary verification:** fmt, clippy, 3560+ tests, doc tests, generate-docs check all pass
- **Documentation updated:** AGENTS.md, architecture/budget-concurrency.md, architecture/text-library.md
- **Deferred findings:** none
- **Final phase disposition:** complete
