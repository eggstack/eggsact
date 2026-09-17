# Eggfetch Self-Update Transport Readiness

Status: complete — `eggfetch-core` 0.1.6 published 2026-09-17, ready for Plan 02
Priority: P1
Scope: qualify and, where necessary, minimally extend `eggfetch-core` so Eggsact can replace its bespoke `curl` subprocess transport without losing updater security or deployment behavior; no Eggsact updater migration in this plan

## Objective

Prepare a stable, generic `eggfetch-core` contract that can own the HTTP/TLS portion of `eggsact update` without moving Eggsact-specific release/update policy into eggfetch.

This is intentionally an upstream-readiness plan. Eggsact currently has no `reqwest` dependency. Its self-updater in `src/update.rs` shells out to `curl` for the small amount of network I/O it needs. Moving that transport in-process can reduce duplicated networking maintenance and remove `curl` as a post-install runtime dependency, but it is not automatically a binary-size optimization because `eggfetch-core` links an HTTP/TLS stack into the Eggsact binary.

`eggfetch-core` 0.1.5 is already close to the required shape: it uses the same Rust 1.89 MSRV as Eggsact, is async-first on Tokio, supports HTTP/1, rustls/native roots, redirects, streaming response bodies, phase-aware timeouts, and optional proxy support. Two behavioral boundaries must be resolved before Eggsact should depend on it for executable updates:

1. Eggsact's current `curl --proto '=https'` contract does not permit redirect downgrade to plain HTTP, while eggfetch's current generic redirect validator accepts both `http` and `https` destinations.
2. Native `eggfetch-core` intentionally does not read proxy environment variables, while `curl` normally participates in environment proxy configuration. A direct replacement would therefore regress updater reachability in proxied environments unless Eggsact reimplements policy that belongs in the HTTP layer.

This plan closes those gaps generically in eggfetch, verifies the exact feature/dependency surface Eggsact will consume, and produces a published eggfetch version suitable for `eggfetch-02-eggsact-migration.md`.

## Ownership boundary

Keep this boundary explicit throughout implementation:

```text
Eggsact owns:
  release/version selection
  crates.io response interpretation
  GitHub asset naming and target mapping
  Cargo fallback
  SHA-256 sidecar parsing and verification
  temporary staging
  candidate --version validation
  Unix/Windows executable replacement
  update UX and exit semantics

Eggfetch owns:
  URL parsing
  DNS/TCP
  TLS and trust roots
  HTTP request/response framing
  redirect transport policy
  HTTP status exposure
  proxy routing when explicitly enabled
  phase-aware timeout enforcement
  streaming response-body delivery
```

Do not add any Eggsact-specific release URL, checksum, executable-update, GitHub-release, or Cargo-install behavior to eggfetch.

## Current Eggsact transport contract to preserve

Before changing eggfetch, treat the existing `src/update.rs::download()` behavior as the compatibility reference. It currently asks `curl` for:

- HTTPS-only protocol allowance via `--proto '=https'`;
- TLS 1.2 minimum via `--tlsv1.2`;
- redirects via `--location`;
- 10-second connect timeout;
- 120-second wall-clock maximum;
- `eggsact-self-update` user agent;
- response body written directly to a destination file;
- final HTTP status extraction;
- `2xx -> Success`, `404 -> NotFound`, all other outcomes -> `HardFailure`;
- normal `curl` environment-proxy behavior;
- no application-level retry loop.

The higher-level updater deliberately interprets `404` for a release binary as "binary unavailable, fall back to Cargo". Other transport/status failures are hard failures. Preserve this distinction.

The SHA-256 sidecar and candidate execution checks are defense-in-depth but do not justify weakening the transport contract. In particular, an HTTPS-to-HTTP redirect downgrade must not become newly acceptable merely because the downloaded file is later hashed.

## Standing constraints

- Keep eggfetch generic. No `eggsact` feature, updater helper, release-download helper, or repository-specific API.
- Preserve native eggfetch's deterministic default: native clients must not begin reading process proxy environment implicitly just because this work lands.
- Preserve current secure TLS defaults and certificate/hostname verification.
- Do not enable HTTP/2, HTTP/3, compression, cookies, multipart, JSON, tracing, or retries merely for Eggsact.
- Do not broaden the Eggsact migration to installer bootstrap scripts; `packaging/install.sh` must continue to use an external bootstrap downloader because Eggsact does not yet exist at that point.
- No new unsafe code.
- Maintain Rust 1.89 compatibility.
- Keep all new proxy-environment parsing testable from supplied values/maps rather than mutating process environment in parallel tests.
- Treat any public eggfetch API addition as general-purpose pre-1.0 API: document semantics and test failure boundaries.

## Part A — Confirm the exact upstream baseline

### A1. Verify 0.1.5 and current main

In `eggstack/eggfetch`, verify the published/current `eggfetch-core` surface used by this plan rather than relying on assumptions from older revisions:

- workspace MSRV is Rust 1.89;
- `Client` / `ClientBuilder` support HTTP/1 + rustls;
- `Timeout` can independently express 10-second connect and 120-second total deadlines;
- `Response::status()` exposes status before body collection;
- `Response::bytes_stream()` can stream the release binary without whole-body buffering;
- redirects are disabled by default and configurable when enabled;
- native proxy behavior remains explicit rather than implicit;
- default TLS behavior verifies hostname/certificate and supports TLS 1.2/1.3;
- system trust roots are available with packaged WebPKI fallback under `tls-native-roots`.

If current main already contains an equivalent solution to Parts B or C, do not add a second API. Document the existing call path and add only missing regression coverage.

### A2. Pin the minimal prospective Eggsact feature set

The intended dependency should remain approximately:

```toml
eggfetch-core = {
    version = "<qualified-version>",
    default-features = false,
    features = ["http1", "tls-rustls", "tls-native-roots", "proxy"]
}
```

`proxy` is present only to preserve the updater's current environment-proxy reachability. If the final generic environment-proxy API can be implemented without the broader proxy feature, record that and use the smaller feature set instead.

Explicitly verify that none of these are activated transitively for Eggsact's use case unless required by the selected feature set:

- `http2`
- `http3`
- compression codecs
- cookies
- multipart
- `json`
- tracing

Produce `cargo tree -e features` evidence in eggfetch or a temporary consumer fixture so the eventual Eggsact plan has an authoritative expected feature graph.

## Part B — Add a strict redirect transport policy if still missing

### B1. General-purpose API

Add the smallest generic policy that can express "follow redirects, but never downgrade HTTPS to HTTP".

Acceptable designs include either:

- an explicit redirect downgrade policy (preferred if it keeps call sites simple), or
- an allowed-schemes/redirect-target policy that can express the same rule without forcing caller callbacks into the hot path.

Conceptually, the caller needs to configure:

```text
follow redirects: yes
maximum redirects: bounded
HTTPS -> HTTPS: allowed
HTTPS -> HTTP: rejected before sending the next hop
HTTP -> HTTP/HTTPS: whatever the selected generic policy defines
```

Do not hard-code "updater mode" or GitHub-specific hostname rules.

### B2. Security behavior

The downgrade decision must happen before any request is dispatched to the disallowed redirect target. Returning a response and checking `Response::history()` after the body has already crossed plain HTTP is not sufficient.

Preserve existing redirect safety behavior:

- credential stripping rules;
- body replayability checks;
- method rewrite rules;
- redirect loop/max-hop bounds;
- relative and scheme-relative location handling;
- unsupported-scheme rejection.

### B3. Tests

Add focused tests covering at least:

- HTTPS -> HTTPS redirect succeeds under strict mode;
- HTTPS -> relative HTTPS-equivalent redirect succeeds;
- HTTPS -> HTTP redirect is rejected before second-hop I/O;
- multi-hop HTTPS chain followed by an HTTP downgrade is rejected on the downgrade hop;
- current default/compat redirect behavior is unchanged when strict mode is not selected;
- sensitive-header stripping remains unchanged across allowed cross-origin redirects.

The tests should prove the transport policy, not merely string-match an error message.

## Part C — Add explicit opt-in environment proxy resolution if still missing

### C1. Preserve native deterministic defaults

Native `eggfetch-core` must continue to perform no process-environment proxy discovery unless the caller asks for it. The new API should make environment participation visible at the call site.

A reasonable shape is a small resolver/value object such as:

```text
ProxyEnvironment / EnvironmentProxyConfig
  parse supplied environment snapshot
  resolve explicit proxy route for a URL
  apply NO_PROXY bypass

ClientBuilder
  opt into/use resolved environment proxy configuration
```

The exact naming may follow existing eggfetch conventions.

Avoid an API whose only entry point calls `std::env` internally and is difficult to test. Prefer a pure parser/resolver from supplied `(key, value)` data plus a convenience constructor that snapshots the actual process environment once.

### C2. Semantics required for Eggsact

At minimum, HTTPS downloads must support conventional environment routing used by command-line clients:

- `HTTPS_PROXY` / `https_proxy`;
- `ALL_PROXY` / `all_proxy` fallback where supported by the existing proxy subsystem;
- `NO_PROXY` / `no_proxy` bypass;
- HTTP proxy URLs used as CONNECT proxies for HTTPS targets;
- existing SOCKS support only if it naturally falls out of the current proxy parser; do not add a new SOCKS implementation for this plan.

If eggfetch already has HTTPX-compatible environment parsing utilities in another boundary, reuse shared pure parsing/normalization logic rather than maintaining a second parser with subtly different semantics. Do not make the native API inherit all HTTPX behavior accidentally; document the selected native environment precedence rules.

### C3. Security and error behavior

- Proxy credentials must continue to follow eggfetch's existing redaction rules.
- Invalid configured proxy values should produce a bounded, redacted configuration/request error rather than silently falling back to direct transport.
- `NO_PROXY` must be applied before proxy dispatch.
- Never log raw proxy environment values.
- Do not add broad environment capture to debug output.

### C4. Tests

Use supplied environment snapshots/maps. Do not rely on global `set_var`/`remove_var` mutation in parallel test processes.

Cover at least:

- HTTPS proxy selected;
- lowercase variant selected according to documented precedence;
- `ALL_PROXY` fallback;
- `NO_PROXY` bypass for exact host and representative domain form;
- invalid proxy URL fails closed and redacts credentials;
- no environment resolver configured -> direct/native behavior unchanged;
- resolver snapshot remains stable even if an unrelated caller changes process environment later.

## Part D — Minimize avoidable dependency duplication

### D1. Base64 version alignment

Current Eggsact and eggfetch-core use different `base64` 0.x minor lines. Because 0.x semver minor versions are distinct Cargo compatibility lines, a direct dependency can otherwise leave two copies in the final graph.

In eggfetch, determine whether moving its unconditional `base64` dependency to the same compatible line used by Eggsact is source-compatible and passes the full eggfetch suite. If yes, align it in the upstream patch. If no, document the concrete incompatibility and leave it for the Eggsact footprint measurement rather than forcing a risky refactor.

Do not make dependency-version churn outside this directly duplicated path part of the plan.

### D2. Verify the minimal graph

Record before/after:

```text
direct eggfetch-core dependencies
resolved package count for minimal Eggsact feature set
cargo tree -d duplicate packages
TLS backend/provider packages
Tokio features enabled
```

The purpose is to eliminate avoidable duplication, not to redesign eggfetch's internal transport stack.

## Part E — Upstream qualification and release

Before Eggsact consumes the changes:

1. run eggfetch's ordinary formatting/lint/test gates;
2. run Rust 1.89/MSRV verification;
3. run cross-platform checks required by eggfetch's own release policy;
4. verify the strict redirect and environment-proxy tests on supported hosts;
5. publish a normal eggfetch patch release according to that repository's release conventions;
6. verify `eggfetch-core` with the qualified version is visible on crates.io;
7. record the exact qualified version in this plan and in `eggfetch-02-eggsact-migration.md` before implementation begins.

Do not point Eggsact at an unreleased git revision for the final migration. The goal is a normal crates.io dependency with a reproducible `Cargo.lock`.

## Verification commands / evidence

Use eggfetch's repository-specific gates, plus targeted evidence equivalent to:

```bash
cargo test -p eggfetch-core redirect
cargo test -p eggfetch-core proxy
cargo check -p eggfetch-core --no-default-features \
  --features http1,tls-rustls,tls-native-roots,proxy
cargo tree -p eggfetch-core --no-default-features \
  --features http1,tls-rustls,tls-native-roots,proxy -e features
cargo tree -p eggfetch-core --no-default-features \
  --features http1,tls-rustls,tls-native-roots,proxy -d
```

Adjust command syntax to the actual eggfetch workspace if necessary, but retain equivalent evidence.

## Completion criteria

This plan is complete only when all of the following are true:

- a published crates.io `eggfetch-core` version at Rust 1.89 provides the required generic API;
- Eggsact can request redirects while rejecting HTTPS -> HTTP downgrade before second-hop I/O;
- Eggsact can explicitly opt into environment proxy resolution without changing eggfetch native defaults;
- HTTPS proxy + `NO_PROXY` behavior is regression-tested without global environment mutation;
- the intended minimal feature set is documented and verified;
- avoidable `base64` duplication is removed or explicitly justified;
- no Eggsact-specific behavior has been added to eggfetch;
- no updater/release policy has moved out of Eggsact;
- exact upstream version and evidence are recorded for Plan 02.

If strict redirect policy or opt-in environment proxy support is intentionally declined upstream, stop this line before Plan 02 and record the reason. Do not compensate by growing a second redirect/proxy implementation inside Eggsact merely to force the consolidation.

## Handoff to Plan 02

Once complete, update the header of `plans/eggfetch-02-eggsact-migration.md` with:

```text
Qualified eggfetch-core version: X.Y.Z
Plan 01 completion commit/release: <reference>
Minimal features: <exact list>
Known dependency/binary considerations: <summary>
```

Then proceed with the Eggsact-side migration and measurement. Do not combine the upstream API change and Eggsact dependency migration into one opaque cross-repository change set.

## Completion record (2026-09-17)

Qualified `eggfetch-core` version: **0.1.6** ([crates.io](https://crates.io/crates/eggfetch-core/0.1.6)).
Upstream commits: `181786de` (`feat(core)`) + `2e988b5e` (release 0.1.6) on
`eggstack/eggfetch` main, rebased over `8176d577`; CI run `35184372773`
passed before publication.

- Strict redirect policy: `RedirectDowngradePolicy::{Allow, Deny}` on
  `RedirectPolicy` (default `Allow`), `RedirectPolicy::strict()`,
  `ClientBuilder::redirect_downgrade_policy()`, and
  `build_redirect_request_with_redirect_policy()`. `Deny` rejects HTTPS ->
  HTTP before second-hop I/O (`InvalidRedirectLocation`); relative and
  scheme-relative locations resolve first. Unit + local HTTPS-origin
  integration tests (downgrade target sees zero requests); compat behavior
  unchanged when strict is not selected.
- Opt-in environment proxy: `ProxyEnvironment::from_map()` (pure, for tests)
  / `from_env()` (one-shot snapshot) with `resolve()` per URL and
  `client_proxies()`, plus fallible `ClientBuilder::proxy_environment()`.
  Native default stays environment-independent. Lowercase wins, `ALL_PROXY`
  fallback per scheme, `NO_PROXY` via native parsing applied before dispatch,
  invalid values fail closed with redacted errors. `Proxy::http_compat()` /
  `https_compat()` added for credential-bearing environment URLs.
- Minimal feature set verified: `http1,tls-rustls,tls-native-roots,proxy`
  (see `docs/architecture/feature-flags.md` "H1 updater transport" in
  eggfetch). Runtime graph: 24 direct deps, 112 resolved packages, single
  `base64` 0.23.1 line; only duplicate is `webpki-roots` 0.26.11/1.0.8
  (0.26 wraps 1.x data — inherent). No http2/http3/compression/cookies/
  multipart/json; `tracing` present only via `hyper-rustls/logging` inside
  `tls-rustls`. Tokio features: rt/net/time/sync/macros/io-util.
- `base64` aligned 0.22 -> 0.23 (source-compatible `Engine` API; full
  workspace suite passes). The remaining `base64` 0.21 line is dev-only
  (`rcgen` -> `pem` test fixtures) and never enters the shipped graph.
- Baseline confirmations (Part A): MSRV Rust 1.89 (verified with 1.89.0
  toolchain), HTTP/1 + rustls, independent connect/total `Timeout` phases,
  `Response::status()` before body, single-consumption `bytes_stream()`,
  redirects disabled by default, no implicit env proxy, TLS 1.2 minimum with
  native-roots + WebPKI fallback and hostname verification.
- No Eggsact-specific release/update policy added to eggfetch; no updater
  migration in this plan (that is Plan 02).

## Handoff to Plan 02 (recorded)

```text
Qualified eggfetch-core version: 0.1.6
Plan 01 completion commit/release: eggstack/eggfetch 2e988b5e / crates.io eggfetch-core 0.1.6
Minimal features: http1,tls-rustls,tls-native-roots,proxy
Known dependency/binary considerations: HTTP/TLS moves in-process; expect binary/package growth vs external curl. Runtime graph for the feature set: 24 direct / 112 resolved packages, single base64 0.23, rustls 0.23.45 + ring via hyper-rustls 0.27. Measure before/after stripped release bytes; >=10% or >=1 MiB growth triggers maintainer review.
```
