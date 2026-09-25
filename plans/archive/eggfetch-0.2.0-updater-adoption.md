# Eggfetch 0.2.0 Updater Adoption and Requalification

Planning baseline: `074f71cbbf99290b3c2bc8e022976642d9128a64` (`main`, 2026-09-20)
Upstream dependency target: published `eggfetch-core 0.2.0`
Upstream release commit: `8959ca890ee34f4cf456aed648315322f1e83ef7`
Upstream issue #24 fix: `37ab02b3873a4f0ce7018bd716e326bcf0595230`
Status: closed

## Objective

Advance Eggsact's private self-update transport from `eggfetch-core 0.1.7` to the
published `0.2.0` release, prove that the updater-facing API and behavior remain
unchanged across the pre-1.0 minor-version boundary, measure the dependency and
binary-footprint delta, and requalify the supported updater paths without
expanding Eggsact's network surface.

This is a dependency-adoption and requalification pass. It is not a transport
redesign.

The updater must remain:

- binary-only application functionality in `src/update.rs`;
- HTTP/1 only;
- strict redirect following with HTTPS -> HTTP downgrade rejection;
- explicit opt-in environment-proxy support with fail-closed invalid config;
- native roots + packaged WebPKI fallback;
- 10-second connect and 120-second total wall-clock deadlines;
- request-local authoritative bounds for small metadata/checksum bodies;
- no configured retry policy;
- automatic decompression disabled;
- free of compression/cookie/multipart/JSON/tracing feature expansion;
- release binaries streamed to disk rather than buffered wholesale;
- SHA-256 verified before replacement;
- crates.io-authoritative for stable version selection;
- free of an external `curl` requirement after installation.

No MCP-facing or public library networking API is added.

## Current Eggsact state

At the planning baseline, `Cargo.toml` contains:

```toml
eggfetch-core = { version = "0.1.7", default-features = false, features = ["http1", "tls-rustls", "tls-native-roots", "proxy"] }
```

The lockfile resolves:

```text
eggfetch-core         0.1.7
eggfetch-http-connect 0.1.7
```

The updater imports and uses the following upstream surface directly:

- `Client`;
- `HttpVersionPolicy`;
- `ProxyEnvironment`;
- `RedirectPolicy`;
- `Timeout`;
- `redact_url_string`;
- `ClientBuilder::{user_agent,http_version_policy,redirect_policy,timeout,automatic_decompression,proxy_environment,build}`;
- `RequestBuilder::{max_decoded_body_size,send}`;
- `Response::{status,content_length,bytes,bytes_stream}`;
- the typed `Error` variants used by `map_transport_error` and
  `map_small_body_error`.

The existing 0.1.7 qualification already proves:

- post-header total timeout for buffered metadata;
- post-header total timeout for streamed binary bodies;
- partial-file cleanup after streamed failures;
- authoritative no-length/chunked small-body limits;
- strict redirect and proxy behavior;
- no-curl updater operation;
- streamed binary downloads;
- Linux merge-gate correctness;
- Rust 1.89 MSRV;
- cargo-deny;
- Windows/macOS supported-platform compile checks;
- a controlled live crates.io updater smoke.

This pass should reuse those tests as regression contracts rather than rebuild
the updater architecture.

## Upstream 0.2.0 assessment

Upstream `CHANGELOG.md` for 0.2.0 explicitly records:

- no intentional breaking Rust/Python/C/CLI/HTTPX API changes from 0.1.7;
- no feature-graph/default changes;
- no MSRV change (still Rust 1.89);
- no dependency-policy change;
- the minor bump primarily resynchronizes registries and contains
  API-preserving private decomposition work;
- issue #24's streaming decompression chunk-boundary fix is included.

At `v0.2.0`, the Eggsact-required features still exist with the same intended
composition:

```text
http1
tls-rustls
tls-native-roots
proxy
```

The upstream stable compile-contract tests still exercise the root
`Client`/`Timeout`/`HttpVersionPolicy` surface and proxy/TLS feature
imports used by Eggsact.

Because this is still a pre-1.0 `0.1.x -> 0.2.x` transition, upstream's
compatibility statement is evidence, not a substitute for downstream
compilation and behavior tests.

## Issue #24 boundary

Issue #24 is fixed in upstream 0.2.0, but Eggsact is not currently exposed to
that failing path.

Eggsact:

- does not enable `compression-gzip`, `compression-brotli`,
  `compression-deflate`, or `compression-zstd`;
- configures `.automatic_decompression(false)` on the updater client;
- therefore does not negotiate or invoke eggfetch's automatic compression
  decoder stack in normal updater operation.

Do **not** enable compression merely because issue #24 is fixed.

The value of 0.2.0 to Eggsact is that it is the current synchronized,
API-preserving upstream release containing all 0.1.7 correctness plus later
qualified maintenance. The issue #24 fix is relevant as upstream release
history, not as a reason to change Eggsact's transport policy.

## Part A — Update only the eggfetch line

Change `Cargo.toml` to:

```toml
eggfetch-core = { version = "0.2.0", default-features = false, features = ["http1", "tls-rustls", "tls-native-roots", "proxy"] }
```

Keep the feature list exactly unchanged.

Do not add any `compression-*` feature.

Use a targeted lockfile update, preferably:

```sh
cargo update -p eggfetch-core --precise 0.2.0
```

If Cargo requires explicit coordination for the private proxy helper, update it
to the matching release without broadening the resolver operation:

```sh
cargo update -p eggfetch-http-connect --precise 0.2.0
```

Expected direct eggfetch movement:

```text
eggfetch-core         0.1.7 -> 0.2.0
eggfetch-http-connect 0.1.7 -> 0.2.0
```

Do not run an unconstrained repository-wide dependency refresh as part of this
plan.

### Lockfile acceptance

Record:

```text
baseline SHA:
implementation SHA:
eggfetch-core before/after:
eggfetch-http-connect before/after:
resolved package count before/after:
added packages:
removed packages:
other version changes:
duplicate-version families:
```

Ideally the lockfile delta is limited to the two eggfetch packages and their
checksums. Any additional package movement must be explained by the 0.2.0
resolver graph. Unrelated opportunistic upgrades are out of scope.

## Part B — Compile-qualify the exact updater-facing API

Before changing updater behavior, compile the current `src/update.rs` against
0.2.0.

The existing code should continue to compile without compatibility shims.

Explicitly verify that the following remain source-compatible:

1. `Timeout::builder().connect(...).total(...).build()`;
2. `RedirectPolicy::strict(MAX_REDIRECTS)`;
3. `ProxyEnvironment::from_env()`;
4. `Client::builder()`;
5. `.user_agent(...)`;
6. `.http_version_policy(HttpVersionPolicy::Http1Only)`;
7. `.redirect_policy(...)`;
8. `.timeout(...)`;
9. `.automatic_decompression(false)`;
10. `.proxy_environment(...)`;
11. `.get(url)`;
12. `.max_decoded_body_size(max_bytes)`;
13. `.send().await`;
14. `Response::content_length()`;
15. `Response::bytes().await`;
16. `Response::bytes_stream()`;
17. the error variants matched by Eggsact.

If upstream 0.2.0 requires a source edit for any item above, stop treating this
as a version-only bump. Record the exact incompatible symbol and make only the
smallest behavior-preserving adaptation required.

Do not paper over a changed semantic contract with wildcard error handling or
by deleting typed error branches merely to compile.

## Part C — Preserve no-compression policy

Keep this client construction invariant:

```rust
.automatic_decompression(false)
```

and retain the Cargo feature invariant that no `compression-*` feature is
enabled.

Verify the resolved eggfetch feature graph with Cargo metadata/tree tooling,
for example:

```sh
cargo tree -e features -p eggfetch-core
```

Record that none of these are active:

```text
compression-gzip
compression-brotli
compression-deflate
compression-zstd
```

The issue #24 regression does not need to be copied into Eggsact because the
updater deliberately does not use that decoder surface.

However, retain the existing chunked-transfer updater regressions. In
particular, the no-`Content-Length` chunked small-body limit test must remain
green because it proves chunked transfer framing itself still works through the
0.2.0 transport with decompression disabled.

## Part D — Re-run updater correctness contracts

Run the focused updater suite first.

At minimum preserve and requalify:

- valid crates.io metadata fetch;
- valid checksum fetch;
- no-length/chunked oversized metadata rejection through
  `max_decoded_body_size`;
- ordinary declared oversized metadata rejection;
- post-header metadata body stall -> total timeout;
- post-header streamed binary stall -> total timeout;
- partial binary cleanup after body failure/timeout;
- slow-header timeout;
- strict HTTPS -> HTTP downgrade rejection;
- redirect-loop bound;
- explicit environment proxy selection;
- invalid proxy configuration fails closed;
- credential-safe URL/error redaction;
- binary body chunk ordering;
- destination write-failure cleanup;
- 404 -> Cargo fallback behavior;
- no external `curl` invocation;
- release binary path remains `bytes_stream()` based.

Do not alter expected user-facing updater semantics simply because upstream
private internals were decomposed.

## Part E — Error-contract review

The updater currently maps concrete `eggfetch_core::Error` variants into
stable Eggsact-facing messages.

After the dependency bump:

1. compile every existing match arm;
2. run error-path tests that cover timeout, proxy, redirect, body-size,
   connect, TLS, and body read failures;
3. confirm `DecodedBodyTooLarge` still reaches
   `map_small_body_error` and produces the configured bound in the message;
4. confirm `Timeout { phase, elapsed }` still maps to the existing concise
   updater timeout message;
5. confirm errors continue to use redacted URLs.

If 0.2.0 introduces additional variants that are not relevant to the updater,
the existing final fallback may handle them. Do not add speculative mappings
for upstream surfaces Eggsact never enables.

## Part F — Dependency and footprint audit

Upstream 0.2.0 includes substantial API-preserving private decomposition since
0.1.7. Measure the downstream result rather than assuming zero footprint
movement.

Use the same host, target, toolchain and release profile for baseline and
candidate.

Record:

```text
baseline SHA:
candidate SHA:
rustc:
target:
profile:

stripped release bytes before:
stripped release bytes after:
delta bytes:
delta percent:

resolved packages before:
resolved packages after:
eggfetch-related packages before/after:
unexpected transitive delta:
```

Use the current accepted 0.1.7 updater state as the baseline.

Interpretation:

- small movement attributable to upstream private refactoring is acceptable;
- if the candidate adds >=1 MiB **or** >=10% stripped binary size, stop and
  attribute the growth before accepting;
- do not re-litigate the historical curl -> in-process transport size cost;
- do not redesign the proxy feature graph in this plan;
- do not switch to `standard-http1`: `proxy` still owns/re-enables the
  compatibility `http1` path in upstream 0.2.0.

## Part G — Documentation truth pass

Update current factual version references from 0.1.7 to 0.2.0 where they
describe the implementation that exists now.

Expected current-state locations include:

- `src/update.rs` module-level transport documentation;
- `AGENTS.md` updater notes/gotchas;
- `architecture/cli-binaries.md`;
- `architecture/overview.md`;
- `CHANGELOG.md` Unreleased entry;
- `plans/roadmap.md`;
- this plan's closure record.

Preserve the existing 0.1.6 and 0.1.7 migration records as historical facts.
Do not rewrite their measurements, SHAs, qualification results, or rationale as
if they were performed against 0.2.0.

Only touch `README.md`, `docs/installation.md`, or `docs/cli.md` if a
current behavioral statement becomes inaccurate. They do not need dependency
version numbers for cosmetic consistency.

## Part H — Repository verification

### Focused qualification

Run the updater-focused tests first and fix only regressions attributable to
0.2.0 adoption.

### Tier 1 merge gate

Run the repository merge gate:

```sh
cargo fmt --all -- --check
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
```

### Release contract

Run:

```sh
python3 scripts/check-release-contract.py
bash -n packaging/install.sh
cargo build --locked --release
./target/release/eggsact --version
python3 scripts/smoke-mcp-binary.py ./target/release/eggsact
```

Run `shellcheck packaging/install.sh` when available.

The release-contract checks must continue to prove the installed updater does
not shell out to `curl`.

## Part I — MSRV, dependency policy, and supported platforms

Because this changes the TLS/network dependency line across a pre-1.0 minor
boundary, require the existing maintenance coverage after implementation:

- Rust 1.89.0 check and library tests;
- cargo-deny;
- Windows `cargo check --locked --all-targets --all-features`;
- macOS `cargo check --locked --all-targets --all-features`.

Upstream 0.2.0 states the MSRV remains 1.89, but Eggsact must independently
prove its own resolved graph still honors that contract.

Ordinary Linux CI is necessary but not sufficient platform evidence.

## Part J — Latest-compatible policy check

After pinning/resolving 0.2.0, inspect or run the repository's
`latest-compatible.yml` lane.

Because `version = "0.2.0"` allows later compatible 0.2.x releases under
Cargo's normal caret semantics, ensure the latest-compatible graph does not
silently invalidate the updater API contract.

Per existing repository policy, classify failures according to
`docs/verification.md`; do not broaden this plan into unrelated dependency
maintenance.

## Part K — Optional controlled live smoke

After deterministic local and CI qualification, a controlled live updater smoke
may be run from a disposable/staged release binary:

```sh
./target/release/eggsact update
```

Acceptable results depend on the current published Eggsact version:

- reports that the staged version is already current; or
- detects a newer stable version and follows the ordinary verified staging
  path in a disposable environment.

Record:

```text
binary version:
platform:
crates.io metadata result:
redirect/TLS/proxy result:
update outcome:
replacement performed?:
```

Do not make public-network access a unit-test or routine CI requirement.

## Part L — Release boundary

This plan ends when `main` is merge-ready on `eggfetch-core 0.2.0` with
updater behavior, footprint, MSRV, dependency-policy, and supported-platform
qualification recorded.

Do not automatically publish a new Eggsact version from this implementation
plan.

Before the next normal Eggsact release:

- ensure the 0.2.0 dependency bump is present in the release commit;
- run the normal Tier 4 release check from a clean worktree;
- follow `docs/release.md` for crates.io-first publication, tag creation,
  binary workflow, and installer verification.

## Files expected to change

Expected implementation scope:

- `Cargo.toml`;
- `Cargo.lock`;
- `src/update.rs` only if a source-compatible version/documentation adjustment
  is required.

Expected documentation/status scope:

- `CHANGELOG.md`;
- `AGENTS.md`;
- `architecture/cli-binaries.md`;
- `architecture/overview.md`;
- `plans/roadmap.md`;
- this plan.

Existing updater tests may change only when necessary to encode a discovered
0.2.0 compatibility regression or to update version-specific assertions.

Unexpected MCP/tool-registry, calculator, text/preflight tool, installer,
release workflow, public library API, network policy, or unrelated dependency
changes are scope-expansion signals.

## Explicit non-goals

Do not expand this adoption into:

- enabling response compression/decompression;
- adding gzip/Brotli/deflate/zstd features;
- copying eggfetch issue #24's decoder regression suite into Eggsact;
- adding retries;
- HTTP/2 or HTTP/3;
- proxy redesign;
- TLS policy redesign;
- cookie/multipart/JSON/tracing support;
- public Eggsact HTTP APIs;
- a custom bounded HTTP reader;
- a second body-size limit implementation;
- switching from `http1` to `standard-http1`;
- general dependency upgrades;
- broad binary-size optimization;
- release automation redesign;
- installer redesign;
- automatic Eggsact publication.

## Completion criteria

The adoption is complete only when:

- [x] `Cargo.toml` requires `eggfetch-core 0.2.0`.
- [x] The feature list remains exactly
  `http1,tls-rustls,tls-native-roots,proxy`.
- [x] No `compression-*` feature is enabled.
- [x] `Cargo.lock` resolves `eggfetch-core 0.2.0`.
- [x] `Cargo.lock` resolves matching `eggfetch-http-connect 0.2.0`.
- [x] All other lockfile movement is explicitly attributable or reverted.
- [x] The existing updater-facing eggfetch API compiles without semantic
  degradation.
- [x] No compatibility shim broadens or weakens typed error handling.
- [x] `.automatic_decompression(false)` remains configured.
- [x] Chunked no-length small-body bound regression remains green.
- [x] Small metadata/checksum fetches remain bounded by
  `max_decoded_body_size`.
- [x] Post-header metadata total-timeout regression remains green.
- [x] Post-header streamed-binary total-timeout regression remains green.
- [x] Partial-file cleanup remains green.
- [x] Strict redirect downgrade rejection remains green.
- [x] Environment proxy behavior remains green.
- [x] Invalid proxy configuration still fails closed.
- [x] No-curl structural/release-contract guard remains green.
- [x] Release binaries remain streamed via `bytes_stream()`.
- [x] Error messages remain credential-safe/redacted.
- [x] Before/after resolved-package and stripped-binary deltas are recorded.
- [x] Any >=1 MiB or >=10% size growth is investigated before acceptance.
- [x] Current implementation docs identify eggfetch-core 0.2.0.
- [x] Historical 0.1.6/0.1.7 records remain truthful.
- [x] Tier 1 merge gate passes.
- [x] Release-contract and release-binary smoke pass.
- [x] Rust 1.89 MSRV check passes.
- [x] cargo-deny passes.
- [x] Windows supported-platform compile check passes.
- [x] macOS supported-platform compile check passes.
- [x] Latest-compatible policy lane is inspected/passes per repository policy.
- [x] Ordinary remote CI passes on the implementation commit.
- [x] No MCP/tool surface, public library API, updater policy, unrelated
  dependency, or release automation change is introduced.

## Closure record template

Append before marking this plan complete:

```text
Planning baseline: 074f71cbbf99290b3c2bc8e022976642d9128a64
Implementation commit:
Qualified eggfetch-core: 0.2.0
Upstream release commit: 8959ca890ee34f4cf456aed648315322f1e83ef7
Upstream v0.2.0 tag:
Issue #24 fix reference: 37ab02b3873a4f0ce7018bd716e326bcf0595230

Cargo.toml feature set:
Compression features:
automatic_decompression:
Lockfile eggfetch-core:
Lockfile eggfetch-http-connect:
Other lockfile delta:
Resolved packages before/after:

Updater API source changes:
Timeout contract:
Redirect contract:
Proxy contract:
Small-body limit contract:
Error mapping contract:
Chunked no-length regression:
Metadata post-header total regression:
Binary post-header total regression:
Partial-file cleanup:
No-curl guard:
Streaming guard:

Baseline stripped bytes:
Candidate stripped bytes:
Size delta:
Size-review disposition:

Tier 1:
Release contract:
Release build/MCP smoke:
MSRV 1.89:
cargo-deny:
Windows compile:
macOS compile:
Latest-compatible:
Remote CI:
Optional live updater smoke:

Current-doc version references:
Historical roadmap disposition:
Known limitations:
Next-release handoff:
```

## Exit criterion

Eggfetch 0.2.0 is fully adopted by Eggsact when the existing self-update
transport compiles and behaves identically on the synchronized upstream
release, the feature set remains deliberately compression-free, the dependency
and binary-footprint deltas are measured and explained, and Linux/MSRV/policy/
Windows/macOS qualification is green without expanding Eggsact's public or
agent-facing surface.

## Closure record

```text
Planning baseline: 074f71cbbf99290b3c2bc8e022976642d9128a64
Implementation commit: bfe12d760554f053dcc6028f32fb3e41dff58a5f
Qualified eggfetch-core: 0.2.0
Upstream release commit: 8959ca890ee34f4cf456aed648315322f1e83ef7
Upstream v0.2.0 tag: v0.2.0 (published 2026-09-22; only 0.2.x on crates.io at adoption)
Issue #24 fix reference: 37ab02b3873a4f0ce7018bd716e326bcf0595230

Cargo.toml feature set: http1,tls-rustls,tls-native-roots,proxy (unchanged)
Compression features: none enabled (cargo tree confirms no compression-*
  in the resolved graph; enabled eggfetch features are
  advanced-routing,basic-auth,high-level-url,http1,hyper-rustls,
  logical-retry,native-http1,proxy,redirects,standard-route,
  tls-native-roots,tls-rustls,transport-http1)
automatic_decompression: false (unchanged, src/update.rs)
Lockfile eggfetch-core: 0.1.7 -> 0.2.0
Lockfile eggfetch-http-connect: 0.1.7 -> 0.2.0
Other lockfile delta: none (only 2 version + 2 checksum lines changed)
Resolved packages before/after: 166 -> 166
Duplicate-version families: unchanged (getrandom x2, rand_core/phf,
  syn 2/3, webpki-roots, windows-sys same as baseline)

Updater API source changes: none (src/update.rs compiles unchanged
  except the 0.1.7 -> 0.2.0 module-doc version reference; all 17
  plan-listed API items source-compatible, no shims, typed error
  match arms intact)
Timeout contract: green (distinct 10s connect / 120s total asserted;
  buffered + streamed post-header total-timeout regressions pass)
Redirect contract: green (strict, downgrade-denied, loop bounded)
Proxy contract: green (explicit env selection, invalid config fails closed)
Small-body limit contract: green (declared-oversize + chunked no-length
  bound via max_decoded_body_size)
Error mapping contract: green (timeout/TLS/proxy/connect variants mapped,
  DecodedBodyTooLarge bound message, URLs redacted)
Chunked no-length regression: green
Metadata post-header total regression: green
Binary post-header total regression: green (incl. partial-file cleanup)
Partial-file cleanup: green
No-curl guard: green (structural + release-contract)
Streaming guard: green (bytes_stream chunk order, write-failure cleanup)

Baseline stripped bytes: 11_101_504 (aarch64-apple-darwin, rustc 1.98.1,
  release strip/lto-thin/cgu-1, HEAD 146a2f2 before bump)
Candidate stripped bytes: 11_101_552 (same host/toolchain/profile)
Size delta: +48 bytes (~0.0004%)
Size-review disposition: accepted, far below the >=1 MiB / >=10% trigger;
  no curl->in-process relitigation, no proxy-graph redesign.

Tier 1: green locally in order (fmt, generate-docs --check, clippy
  -D warnings, full non-parity suite 643+34+14+3011+51 lib/bin/integration
  + 11 doc tests with 0 failures, then doc tests)
Release contract: green (check-release-contract.py, bash -n, shellcheck,
  release build, eggsact --version 1.2.6, MCP smoke 77 tools)
Release build/MCP smoke: green
MSRV 1.89: green locally (cargo +1.89.0 check --locked --all-targets
  --all-features and --all-features --lib: 643 passed; 2 pre-existing
  unused-variable warnings in src/tools/list.rs, unrelated to this bump)
  and green remotely in Maintenance run 35740940881 (MSRV job success)
cargo-deny: green locally (advisories/bans/licenses/sources ok) and green
  remotely in Maintenance run 35740940881 (cargo-deny job success)
Windows compile: green natively via Maintenance run 35740940881
  (https://github.com/eggstack/eggsact/actions/runs/35740940881,
  workflow_dispatch on main, head SHA 0c30b1d552f067f8eb5661124d4833fa52fd2611,
  Check (windows-latest) success running
  cargo check --locked --all-targets --all-features).
  Historical context only: local cross-check from macOS was blocked by ring
  0.17.14 needing an MSVC C compiler (identical on the 0.1.7 baseline; ring
  version untouched by this bump). That limitation is no longer the
  qualification boundary; native Windows CI above is the qualification evidence.
macOS compile: green locally (native cargo check --locked --all-targets
  --all-features) and green natively via Maintenance run 35740940881
  (Check (macos-latest) success)
Latest-compatible: green in scratch worktree at b38dccf (2026-09-22):
  `cargo update` keeps eggfetch-core/http-connect at 0.2.0 (no newer 0.2.x
  published) with 29 unrelated patch bumps; `cargo check --all-targets
  --all-features`, `--all-features --lib` (643 passed), `--bins`
  (34+14 passed), and `--tests -- --skip parity --test-threads=4`
  (3011 integration passed) all green. Caret range does not invalidate
  the updater API contract.
Remote CI: green — CI run 35734609288 (workflow_dispatch on bfe12d7
  after a ref-lock race swallowed the push event; head SHA verified
  bfe12d7) completed success 2026-09-22; Maintenance run 35740940881
  (workflow_dispatch on main, head SHA 0c30b1d552f067f8eb5661124d4833fa52fd2611)
  completed success 2026-09-22 with MSRV, cargo-deny,
  Check (windows-latest), and Check (macos-latest) all success
Optional live updater smoke: not run (no disposable staged newer version;
  not a unit/CI requirement)

Current-doc version references: src/update.rs, AGENTS.md,
  architecture/cli-binaries.md, architecture/overview.md, CHANGELOG
  Unreleased all identify eggfetch-core 0.2.0; README/docs/installation.md/
  docs/cli.md untouched (no version numbers, no behavioral change);
  skills reviewed (no eggfetch claims); check-release-contract.py
  version-agnostic (no change)
Historical roadmap disposition: 0.1.6 footprint record and 0.1.7 correction
  record preserved verbatim; 0.2.0 section flipped active -> landed
Known limitations: local Windows cross-check from macOS remains impossible
  without an MSVC toolchain (pre-existing, baseline-identical) but is
  historical context only after native Maintenance run 35740940881; live
  crates.io smoke optional and not run
Corrective closeout: eggfetch-0.2.0-adoption-closeout-corrective.md records
  Maintenance run 35740940881 as the native Windows/macOS/MSRV/cargo-deny
  qualification evidence for the accepted implementation (no updater/
  dependency implementation change required)
Next-release handoff: 0.2.0 bump rides the next normal Eggsact release;
  run Tier 4 release check from a clean worktree per docs/release.md;
  do not publish from this plan
```
