# Eggfetch 0.1.7 Updater Dependency Bump and Downstream Qualification

Planning baseline: `40959b704431430668e9ca2bfe959a8ef32495d8` (`main`, 2026-09-18)
Upstream dependency target: `eggfetch-core 0.1.7`
Status: active implementation handoff

## Objective

Move Eggsact's self-update transport from `eggfetch-core 0.1.6` to the published `0.1.7` corrective release, consume eggfetch's existing authoritative decoded-body limit for small metadata/checksum responses, prove the corrected total deadline through response-body consumption at the Eggsact boundary, and requalify the updater without reopening its architecture.

This is a narrow downstream dependency correction. The updater remains:

- binary-only application functionality in `src/update.rs`;
- HTTP/1;
- strict redirect following with HTTPS -> HTTP downgrade rejection;
- explicit environment-proxy support;
- native roots + WebPKI fallback;
- 10-second connect and 120-second total deadlines;
- no configured retry policy;
- automatic decompression disabled;
- binary assets streamed to disk;
- SHA-256 verified before replacement;
- crates.io-authoritative for stable version selection;
- free of an external `curl` requirement after installation.

No MCP/library network API is added.

## Why this bump is required

Eggsact currently pins:

```toml
eggfetch-core = { version = "0.1.6", default-features = false, features = ["http1", "tls-rustls", "tls-native-roots", "proxy"] }
```

Upstream `eggfetch-core 0.1.7` closes the response-body total-deadline defect that matters directly to the updater:

- `Timeout.total` remains one absolute logical-request deadline through response-body EOF/trailers;
- delayed first body polling cannot restart the total budget;
- body trickle/progress cannot keep the request alive past total;
- read and total timeout phases remain distinct;
- pool permits are released on timeout terminalization;
- redirect/retry remaining-budget behavior is qualified;
- the public `ResponseBody` field shape remains compatible with 0.1.6.

Upstream final executable/test qualification is recorded on `82f3f38631b44a9a5c5ec5b40790e5015aeb40f8`, with coordinated 0.1.7 release commit `43c3b312f2def887d0f0b7ce539faa626adf2cc8` and tag `v0.1.7`.

Eggsact's current short-timeout regression only delays response headers. It therefore does not prove that Eggsact benefits from the corrected post-header/body lifecycle behavior.

Separately, `get_small_text()` currently treats `Content-Length` as an early bound, buffers via `response.bytes().await`, and only checks the received length afterward. Eggfetch already provides `RequestBuilder::max_decoded_body_size`, which is authoritative for identity/unencoded and decoded bodies even when `Content-Length` is absent or false. Eggsact should consume that existing API instead of relying on post-buffer inspection for safety.

## Important upstream graph change from 0.1.6

The 0.1.7 release also includes eggfetch's intervening feature/dependency refactor.

Notably:

- `proxy` now owns the optional `eggfetch-http-connect` crate;
- `http1` remains the compatibility high-level bundle and now expands through the explicit routing/policy feature split;
- `proxy` still depends on `http1`, `tls-rustls`, high-level URL support, and Basic-auth/Base64 support.

Therefore this bump may legitimately change Eggsact's `Cargo.lock` shape even though Eggsact's requested feature list is unchanged.

Do **not** switch Eggsact to `standard-http1` in this pass. Eggsact requires environment proxy routing and strict redirect following, while eggfetch's current `proxy` feature itself re-enables the compatibility `http1` bundle. Replacing the alias would not produce the intended lean graph and would mix footprint redesign into a correctness bump.

## Part A — Bump only the qualified dependency

Update `Cargo.toml`:

```toml
eggfetch-core = { version = "0.1.7", default-features = false, features = ["http1", "tls-rustls", "tls-native-roots", "proxy"] }
```

Keep the feature list exactly unchanged.

Update the lockfile with a targeted dependency operation, for example:

```sh
cargo update -p eggfetch-core --precise 0.1.7
```

Allow only resolver changes required by the new eggfetch release and its changed feature/dependency ownership.

Expected/acceptable examples include:

- `eggfetch-core 0.1.6 -> 0.1.7`;
- addition of `eggfetch-http-connect 0.1.7` because `proxy` now owns it;
- removal/addition/version movement of transitive packages that is directly explained by the 0.1.6 -> 0.1.7 eggfetch graph.

Do not run a broad unconstrained `cargo update` as part of this implementation.

### Lockfile acceptance

Record before/after:

```text
direct dependencies:
resolved packages:
added packages:
removed packages:
version-changed packages:
duplicate-version families:
```

Every lockfile delta must be attributable to the eggfetch bump. Unrelated dependency refresh is out of scope.

## Part B — Make eggfetch's body limit authoritative in get_small_text

Change `get_small_text()` so each metadata/checksum request sets the request-local decoded-body bound before `send()`:

```rust
client
    .get(url)?
    .max_decoded_body_size(max_bytes)
    .send()
    .await
```

The existing constants remain application policy:

- `METADATA_MAX_BYTES = 1 MiB`;
- `CHECKSUM_MAX_BYTES = 64 KiB`.

### Bound ownership

Eggfetch's stream-level decoded-body cap becomes the authoritative safety mechanism.

The existing `Content-Length` check may remain as an early/application-friendly rejection if useful, but it must be documented as advisory/early only. It must not be the safety boundary.

Remove the ordinary post-buffer `bytes.len() > max_bytes` check if it becomes unreachable/redundant under the authoritative eggfetch limit. Do not maintain two independent accumulation-limit implementations.

Preserve useful Eggsact-facing error text. When eggfetch returns `Error::DecodedBodyTooLarge` for this path, map it to a concise message identifying:

- the metadata/checksum object;
- the redacted URL;
- the configured byte bound.

Do not expose credentials or raw secret-bearing URLs.

### Acceptance

- [ ] Small valid metadata/checksum bodies still succeed.
- [ ] A declared oversized body is rejected.
- [ ] An oversized response with no useful `Content-Length` is rejected by eggfetch's decoded-body limit.
- [ ] The path does not first accumulate an unbounded response and then inspect its length.
- [ ] Binary release downloads are unaffected and remain streamed to disk.

## Part C — Add an absent-length/stream-bound regression

Extend updater tests with a response whose body exceeds a deliberately small test bound without a trustworthy declared length.

Preferred fixture:

- HTTP/1 chunked response with no `Content-Length`;
- multiple chunks whose aggregate decoded size exceeds the request-local cap.

Alternative:

- close-delimited identity body with no `Content-Length` if that is simpler in the current fixture.

Call `get_small_text()` with a small test bound rather than allocating >1 MiB merely to test the production constant.

Require:

- `get_small_text()` returns an error;
- error classification/message identifies the configured bound;
- no panic;
- no successful oversized string is returned.

Do not add a second custom bounded reader in Eggsact.

## Part D — Prove total timeout after headers for buffered small bodies

Add a deterministic local TCP regression for the metadata/checksum consumption path:

1. response headers arrive before a short injected total deadline;
2. headers describe or begin a successful 200 response;
3. the response body then stalls beyond the total deadline;
4. `get_small_text()` attempts to consume it;
5. Eggsact returns its existing concise `update request timed out` error.

This must specifically exercise post-header body consumption. The current slow-header test remains useful but is not a substitute.

The test should fail under the old headers-only total behavior and pass with eggfetch 0.1.7.

Do not assert fragile millisecond equality. Assert timeout classification/message within a generous outer test bound.

## Part E — Prove total timeout after headers for streamed binary downloads

Add a second downstream regression for `download_to()`:

1. server sends 200 headers promptly;
2. optionally sends one binary body chunk;
3. stalls before EOF beyond a short injected total deadline;
4. `download_to()` returns the mapped timeout error;
5. the partial destination file is removed.

This proves that the corrected total deadline is effective on Eggsact's actual streamed release-asset path and that timeout cleanup remains intact.

Keep the existing structural guard that `download_to()` uses `bytes_stream()` and does not call buffered `bytes()`.

## Part F — Preserve updater policy and API behavior

Re-run and retain the existing updater tests covering:

- explicit target mappings;
- strict stable-version parsing;
- checksum parsing;
- HTTP status classification;
- Windows staged replacement script behavior;
- distinct connect vs total deadlines;
- strict redirect policy;
- explicit environment proxy selection and fail-closed invalid proxy config;
- no external curl;
- redacted error mapping;
- 404 -> Cargo fallback behavior;
- redirects and redirect-loop bounds;
- streamed chunk ordering;
- destination write failures;
- existing slow-header timeout behavior;
- binary path streaming.

No retry policy should be configured.

Although eggfetch's `http1` compatibility bundle compiles retry support, Eggsact must continue dispatching without a `RetryPolicy`. Do not add retries merely because the capability exists in the dependency.

## Part G — Audit the 0.1.7 feature/footprint delta

Because 0.1.7 changes eggfetch's internal feature/dependency ownership, perform a bounded before/after review.

Use the same host, toolchain, target and release profile for both the planning baseline and the bumped tree.

Record:

```text
baseline SHA:
bump candidate SHA:
rustc:
target:
profile:

stripped release bytes before:
stripped release bytes after:
delta bytes / percent:

direct dependencies before/after:
resolved packages before/after:
eggfetch-related package delta:
```

This is not a new size-optimization program.

Interpretation:

- a small graph/size movement attributable to the 0.1.7 refactor is acceptable;
- if the bump adds >=1 MiB or >=10% stripped size relative to the current eggfetch-based baseline, stop and document the attribution before accepting it;
- do not hide the existing ~3.65 MiB / ~39% curl-to-in-process transport cost — that historical consolidation tradeoff remains accepted and is not re-litigated here;
- do not attempt a proxy/feature redesign inside this plan.

If a future footprint pass is desired, it should begin from the fact that current eggfetch `proxy` re-enables the full `http1` compatibility bundle.

## Part H — Update current-version documentation without rewriting history

Update current factual references from `eggfetch-core 0.1.6` to `0.1.7` where they describe the code that exists now:

- `src/update.rs` module-level transport documentation;
- `AGENTS.md` updater gotcha;
- `architecture/cli-binaries.md`;
- `architecture/overview.md`;
- relevant current `CHANGELOG.md` Unreleased entry.

For `plans/roadmap.md`, preserve the historical 0.1.6 migration/footprint evidence as history. Add a concise follow-up note stating that the updater dependency was later advanced to 0.1.7 for body-lifecycle total correctness and authoritative small-body limits. Do not rewrite the original 0.1.6 measurements as if they were taken against 0.1.7.

Review `README.md`, `docs/installation.md` and `docs/cli.md` for behavioral wording; update only if the implementation changes make a current statement inaccurate. They do not need a dependency-version number merely for consistency.

## Part I — Verification

### Focused updater tests

Run the updater tests first. Use the existing crate/test layout rather than creating a new network-test subsystem.

At minimum verify the new cases for:

- no-length oversized metadata body;
- small valid metadata body;
- post-header buffered-body total timeout;
- post-header streamed-binary total timeout + partial-file removal;
- slow-header timeout;
- redirect behavior;
- proxy configuration;
- no-curl structural guard.

### Tier 1 merge gate

Run exactly the repository merge gate:

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

### MSRV, policy, and supported platforms

Because this is a network/TLS dependency bump, manually dispatch `maintenance.yml` after the implementation lands and require:

- Rust 1.89.0 check + library tests green;
- cargo-deny green;
- Windows `cargo check --locked --all-targets --all-features` green;
- macOS `cargo check --locked --all-targets --all-features` green.

Ordinary Linux CI alone is not sufficient release evidence for this dependency change.

### Latest-compatible

Run or inspect `latest-compatible.yml` after the pinned bump so the repository still tolerates the newest semver-compatible graph. A failure here is triage evidence rather than an automatic merge blocker, per `docs/verification.md`.

## Part J — Optional controlled live updater smoke

A real-network smoke is useful but must not replace deterministic local regressions.

If run, use a disposable/staged binary and verify the crates.io metadata/TLS/proxy/redirect path without risking the maintainer's active executable.

Acceptable outcomes depend on the current Eggsact release state:

- reports the installed/staged version is already current; or
- detects a newer stable version and proceeds through the normal verified staging path in a disposable environment.

Record the exact binary/version and outcome if performed.

Do not make live GitHub/crates.io access an ordinary unit-test or CI requirement.

## Part K — Handoff and release boundary

This plan ends when `main` is merge-ready on eggfetch-core 0.1.7 with downstream behavior and platform qualification recorded.

Do not automatically publish a new Eggsact version from this implementation plan.

However, the corrected dependency only reaches installed users in the next Eggsact release. Before the next crate/binary publication:

- ensure the dependency bump is included in the release commit;
- run the normal Tier 4 `scripts/release-check.sh` from a clean worktree;
- follow `docs/release.md` for crates.io-first publication, tag creation, binary workflow and installer verification.

A future Eggsact release must not claim the 0.1.7 updater correction unless its source actually contains this bump.

## Files expected to change

Expected implementation scope:

- `Cargo.toml`;
- `Cargo.lock`;
- `src/update.rs`.

Expected documentation/status scope:

- `CHANGELOG.md`;
- `AGENTS.md`;
- `architecture/cli-binaries.md`;
- `architecture/overview.md`;
- `plans/roadmap.md`;
- this plan.

Only update `README.md` / installation / CLI docs if current user-facing behavior text becomes inaccurate.

Unexpected MCP/tool registry, calculator, text, preflight, release workflow, installer, public library API, feature graph, or unrelated dependency changes are scope-expansion signals.

## Explicit non-goals

Do not expand this dependency bump into:

- an Eggsact HTTP/MCP/library API;
- retry-policy addition;
- HTTP/2 or HTTP/3;
- compression/cookies/multipart/JSON/tracing features;
- custom TLS policy;
- proxy redesign;
- dropping environment-proxy support;
- switching to `standard-http1` while `proxy` still re-enables `http1`;
- a second body-limit implementation;
- a custom bounded HTTP reader;
- updater policy redesign;
- release workflow redesign;
- installer redesign;
- general dependency upgrades;
- broad binary-footprint optimization;
- automatic Eggsact publication.

## Completion criteria

The bump is ready for handoff only when:

- [ ] `Cargo.toml` requires `eggfetch-core 0.1.7` with the existing feature list unchanged.
- [ ] `Cargo.lock` resolves the intended 0.1.7 graph with no unrelated update churn.
- [ ] Any `eggfetch-http-connect` addition/removal of old eggfetch transitive packages is explicitly attributable.
- [ ] `get_small_text()` sets request-local `max_decoded_body_size(max_bytes)` before sending.
- [ ] `Content-Length` is not treated as the authoritative body-size safety boundary.
- [ ] No redundant caller-side accumulation-limit implementation remains.
- [ ] Oversized no-length/chunked metadata is rejected.
- [ ] Valid small metadata still succeeds.
- [ ] A response whose headers arrive before total but body stalls past total maps to Eggsact's timeout error for `get_small_text()`.
- [ ] The same post-header total behavior is proven for `download_to()`.
- [ ] Partial binary files are removed on body timeout.
- [ ] Binary assets remain streamed and are not buffered wholesale.
- [ ] Existing strict redirect / proxy / TLS / 10s connect / 120s total / no-retry policy is unchanged.
- [ ] Existing updater regression suite remains green.
- [ ] Before/after lockfile and stripped-binary deltas are recorded on comparable builds.
- [ ] Any >=1 MiB or >=10% new stripped-size growth from the 0.1.7 bump is explicitly reviewed before acceptance.
- [ ] Current source/architecture/agent docs identify eggfetch-core 0.1.7.
- [ ] Historical 0.1.6 migration evidence in the roadmap remains historically truthful.
- [ ] Tier 1 merge gate passes.
- [ ] Release-contract and release-binary smoke checks pass.
- [ ] Rust 1.89 MSRV maintenance job passes.
- [ ] cargo-deny maintenance job passes.
- [ ] Windows supported-platform compile check passes.
- [ ] macOS supported-platform compile check passes.
- [ ] Remote ordinary CI passes on the implementation commit.
- [ ] No MCP/tool surface, public Eggsact library API, updater policy, unrelated dependency, or release automation change is introduced.

## Closure record template

Append before marking complete:

```text
Planning baseline: 40959b704431430668e9ca2bfe959a8ef32495d8
Implementation commit:
Qualified eggfetch-core: 0.1.7
Upstream release/freeze reference: 82f3f386 / 43c3b312 / v0.1.7

Cargo.toml feature set:
Lockfile eggfetch-core:
Lockfile eggfetch-http-connect:
Other lockfile delta:

get_small_text authoritative body limit:
No-length oversize proof:
Small-body proof:
Post-header metadata total proof:
Post-header binary-stream total proof:
Partial-file cleanup:
Slow-header timeout:
Redirect/proxy/TLS policy regression:
No-curl / streamed-binary structural guards:

Baseline stripped bytes:
Candidate stripped bytes:
Size delta:
Resolved-package delta:
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

Eggfetch 0.1.7 is fully adopted by Eggsact when the updater is pinned to the published corrective release, small metadata bodies are bounded by eggfetch before unbounded accumulation, post-header total timeout behavior is proven on both buffered and streamed updater paths, the changed dependency graph is measured and explained, and Linux/MSRV/policy/Windows/macOS qualification is green without expanding Eggsact's network surface.
