# Eggsact Self-Update Migration to Eggfetch

Status: blocked on `eggfetch-01-transport-readiness.md`
Priority: P1
Scope: replace only the self-updater's external `curl` transport with a minimal `eggfetch-core` dependency, preserve update semantics/security, measure binary/dependency impact, and qualify all shipped targets; no installer rewrite, no updater-policy redesign, no MCP/network-tool expansion

Qualified eggfetch-core version: TBD after Plan 01
Plan 01 completion commit/release: TBD
Minimal features: expected `http1,tls-rustls,tls-native-roots,proxy`; confirm after Plan 01
Known dependency/binary considerations: expected binary/dependency growth because HTTP/TLS moves in-process; must be measured before closure

## Objective

Replace the bespoke `curl` subprocess transport in `src/update.rs` with the qualified `eggfetch-core` release produced by Plan 01 while keeping the rest of Eggsact's updater ownership and user-visible behavior intact.

The goal is maintenance consolidation and a self-contained post-install updater, not a claim that the release binary will become smaller. Today Eggsact avoids linking a network/TLS stack and instead depends on the host's `curl` executable when `eggsact update` runs. After this migration, Eggsact will carry the required HTTP/TLS code in-process. The implementation must therefore produce explicit before/after dependency and binary-size evidence and must not hide a material footprint regression behind the word "consolidation".

At closure, the updater should still do exactly what it does today at the application-policy level:

```text
crates.io metadata -> choose latest stable
qualified release target -> try GitHub binary
404 binary -> Cargo fallback
binary exists -> require checksum sidecar
checksum mismatch -> refuse
candidate --version mismatch/failure -> refuse
valid candidate -> platform-specific atomic/staged replacement
```

Only the network transport implementation changes.

## Preconditions

Do not start implementation until Plan 01 records a published crates.io version of `eggfetch-core` that provides:

- Rust 1.89 compatibility;
- HTTP/1 + rustls/native roots;
- response streaming;
- separate connect and total timeout controls;
- redirect following with an explicit policy that rejects HTTPS -> HTTP downgrade before the downgraded request is sent;
- explicit opt-in environment proxy resolution, with `NO_PROXY` support, while native default behavior remains environment-independent.

If those prerequisites are not available as a published crate, this plan remains blocked. Do not use a git dependency for the final implementation and do not recreate the missing policy locally in Eggsact.

## Standing constraints

- Keep Eggsact a single crate.
- Preserve Rust 1.89.0 MSRV.
- Preserve all existing updater target mappings, asset names, checksum format, Cargo fallback, candidate validation, and replacement behavior.
- `packaging/install.sh` and `packaging/install.ps1` remain bootstrap installers and are out of scope except for regression verification; they must not depend on eggfetch.
- Do not add an HTTP client surface to Eggsact's public library API.
- Do not expose HTTP through MCP tools.
- Do not add retries in this migration. The current updater does not perform an application-level retry sequence.
- Do not enable HTTP/2, HTTP/3, compression, cookies, multipart, JSON, tracing, or other eggfetch features without a measured requirement.
- Use existing `serde_json` for crates.io metadata parsing rather than enabling eggfetch's optional JSON feature.
- Stream release binaries to disk; do not buffer multi-megabyte executable bodies in memory.
- Preserve checksum verification as application-owned defense-in-depth.
- Preserve `404 -> Cargo fallback`; other non-success HTTP statuses and transport failures remain hard failures.
- No silent fallback from failed/invalid proxy configuration to direct networking if the qualified eggfetch policy treats configuration errors as failures.
- No runtime dependence on external `curl` after Eggsact itself is installed and `eggsact update` is invoked.

## Part A — Capture a trustworthy pre-migration baseline

Before changing `Cargo.toml` or `src/update.rs`, record the current baseline from a clean checkout at the implementation commit's parent.

### A1. Dependency baseline

Record:

```bash
cargo tree --locked
cargo tree --locked -d
cargo tree --locked -e features
```

Summarize at minimum:

- direct dependency count;
- total resolved package count from `Cargo.lock`;
- duplicate package/version families;
- current Tokio feature set;
- absence of Hyper/rustls/eggfetch in the runtime graph.

Do not use raw `Cargo.lock` line count as the primary dependency metric.

### A2. Binary baseline

Build the same optimized profile used for shipped binaries:

```bash
cargo build --locked --release
```

Record the exact byte size of the stripped release executable on the available native host. If the existing release workflow can cheaply provide additional target-size evidence without changing production behavior, record at least Linux x86-64 and Linux AArch64 as well. Do not require cross-target execution solely for this measurement.

Record build environment enough to make the comparison meaningful:

```text
commit
rustc version
target triple
profile.release settings
binary byte size
```

The before/after comparison must use the same host/target/toolchain/profile.

### A3. Behavioral baseline

Preserve or add focused unit tests for the policy helpers before replacing the transport:

- stable version parsing;
- candidate output parsing;
- checksum parsing;
- HTTP status classification;
- target/asset mapping;
- Windows replacement-script invariants.

Where current tests already cover them, retain them unchanged unless the function boundary legitimately moves.

## Part B — Add the minimal eggfetch dependency

After Plan 01 supplies the exact version, add `eggfetch-core` with default features disabled.

Expected shape:

```toml
eggfetch-core = {
    version = "<qualified-version>",
    default-features = false,
    features = ["http1", "tls-rustls", "tls-native-roots", "proxy"]
}
```

Use the smallest final feature set Plan 01 proved sufficient.

Run `cargo update -p eggfetch-core --precise <version>` or the appropriate locked dependency workflow so `Cargo.lock` is deterministic. Do not accept unrelated broad dependency upgrades in the same lockfile change.

Immediately inspect:

```bash
cargo tree --locked -p eggsact -e features
cargo tree --locked -d
```

Check specifically for:

- duplicated `base64` lines;
- multiple TLS providers/backends;
- accidental HTTP/2/HTTP/3 activation;
- compression libraries;
- cookie/multipart dependencies;
- unexpected Tokio features beyond what eggfetch requires.

If the lockfile resolves a materially broader surface than Plan 01 qualified, stop and fix the feature/dependency issue before touching updater logic.

## Part C — Introduce a narrow updater transport adapter

### C1. Keep transport private to `src/update.rs`

Do not make the eggfetch client a crate-global singleton or public library type. The only current network consumer is the self-updater. Keep the abstraction local and small.

A suitable internal shape is conceptually:

```text
UpdateHttpClient
  build() -> configured eggfetch Client
  download_to(url, destination) -> DownloadStatus
  get_small_text(url, max_bytes) -> text/status
```

A separate struct is optional; a pair of private helper functions is acceptable if that is simpler. The important constraint is one configuration point for TLS/redirect/timeout/proxy behavior rather than rebuilding policy for each call.

### C2. Configure policy once

The updater client must explicitly configure:

- user agent `eggsact-self-update`;
- secure certificate/hostname verification;
- native roots with packaged fallback according to the qualified eggfetch configuration;
- HTTP/1 only;
- redirects enabled;
- strict HTTPS downgrade rejection from Plan 01;
- environment proxy resolution explicitly enabled;
- 10-second connect timeout;
- 120-second total wall-clock timeout;
- no retry policy;
- no automatic feature expansion beyond the selected crate features.

Do not use `Timeout::from_secs(120)` as a substitute for the current semantics: the updater requires a distinct 10-second connect deadline and 120-second total cap.

### C3. Async boundary

Eggfetch is async-first. Eggsact's CLI `main()` is intentionally synchronous except when it enters MCP mode and constructs a current-thread Tokio runtime.

Keep the public command structure simple. Preferred shape:

```text
update::run()                 synchronous CLI entry
  -> create current-thread Tokio runtime
  -> block_on(update::run_async())

update::run_async()
  -> build one eggfetch client
  -> perform metadata/download operations
  -> run existing filesystem/process replacement logic
```

It is also acceptable for `main.rs` to build the updater runtime directly if that produces less duplication and clearer error ownership, but do not convert the entire CLI to `#[tokio::main]` solely for this command.

Do not create nested runtimes inside async code. Tests for async transport helpers should use Tokio test/runtime support directly.

## Part D — Replace each current network operation without changing application policy

### D1. crates.io metadata

Current behavior downloads `https://crates.io/api/v1/crates/eggsact` into a temporary file and then parses `crate.max_stable_version`.

With eggfetch, this response is small and may be buffered in memory. Keep the existing `serde_json` interpretation and `StableVersion` parser.

Add a conservative maximum metadata-body bound before/while collecting the response. This prevents a malformed server/proxy response from causing unbounded memory use. The exact bound should be comfortably above the normal crates.io payload but still small (for example, low single-digit MiB or smaller based on observed payload size). Document the selected bound.

Do not enable eggfetch's `json` feature just to parse this response.

### D2. Release binary

For GitHub release assets:

1. send GET;
2. inspect final status;
3. `404` -> return `DownloadStatus::NotFound` without treating it as a generic transport failure;
4. other non-2xx -> `HardFailure`;
5. 2xx -> create/truncate the staging destination safely;
6. consume `Response::bytes_stream()` incrementally;
7. write every chunk to disk;
8. flush/close successfully before returning `Success`.

On any body/read/write failure, remove the partial destination or leave it in a clearly unusable state that `prepare_candidate()` cannot accidentally consume. Prefer removal.

Do not decode/compress executable data. With compression features disabled, release bytes should remain ordinary transport payload bytes.

### D3. Checksum sidecar

The checksum sidecar is small and can be buffered or streamed. Preserve the existing parser: first whitespace-delimited token, exactly 64 hexadecimal characters.

A missing or failed checksum remains fatal when the release binary itself existed. Do not Cargo-fallback after a binary was successfully found but its integrity sidecar could not be obtained or verified.

### D4. Redirect behavior

GitHub release asset URLs normally redirect to asset storage. The migration must preserve working redirect downloads while explicitly preventing HTTPS -> HTTP downgrade.

Tests must exercise an actual local redirect chain; do not assume that because GitHub works today the policy is correct.

### D5. Proxy behavior

Use the explicit environment-proxy API qualified in Plan 01. Do not add a second parser in Eggsact.

Because production tests should not depend on the developer's proxy environment, configure test clients with supplied/synthetic proxy environment snapshots where the eggfetch API permits it. At least one integration-style test should prove an HTTPS/proxy selection path at the policy layer or via eggfetch's own already-proven tests without requiring public network access.

## Part E — Error mapping and user-visible behavior

Preserve the current updater's concise application errors. Eggfetch errors may contain richer transport classification, but `eggsact update` should not dump implementation-heavy debug structures by default.

Map errors into messages that retain useful phase/category information where available:

```text
cannot resolve/connect to update endpoint
TLS/certificate failure
update request timed out
failed while reading release asset
failed while writing staged release asset
HTTP <status> from <redacted URL/context>
proxy configuration/routing failure
```

Do not expose credentials, proxy authorization, URL userinfo, or raw environment values. Rely on eggfetch's redaction primitives where relevant and avoid reformatting errors in a way that defeats them.

Keep the existing updater's exit-code behavior unless there is a documented reason to change it.

## Part F — Add deterministic transport integration tests

The current `src/update.rs` tests are mostly pure-policy tests. Add a focused local HTTP/TLS test harness sufficient to verify Eggsact's integration contract without public network access.

Prefer small test-only local servers and the qualified eggfetch test utilities if available. Avoid bringing a second production HTTP server/client dependency into Eggsact.

Cover at least:

### F1. Status classification

- 200 body -> `Success` and exact file contents;
- 204 empty success where applicable -> `Success`;
- 404 -> `NotFound`;
- 500/503 -> `HardFailure`;
- body stream abort after headers -> `HardFailure` and no consumable partial candidate.

### F2. Redirects

- allowed redirect chain completes;
- redirect loop/max-hop error is bounded;
- HTTPS -> HTTP downgrade is rejected before downgraded body acquisition.

If a real local TLS fixture is disproportionately heavy, use eggfetch's own strict-downgrade regression as the lower-level proof and add an Eggsact configuration assertion showing that strict mode is selected. Do not duplicate a large TLS test framework just for one integration assertion.

### F3. Timeouts

Prove at the Eggsact configuration boundary that:

```text
connect = 10s
total = 120s
```

A test may use shorter injected durations through a test-only client constructor to exercise timeout mapping deterministically. Do not make the production constants tiny just to speed tests.

### F4. Streaming/file behavior

- a multi-chunk response is written in order;
- a read failure removes the partial file;
- a destination write failure is surfaced;
- metadata response respects its configured body bound;
- release binary path is not buffered into one `Vec<u8>`/`Bytes` allocation by Eggsact code.

### F5. No external curl requirement

Add a test or structural assertion proving `src/update.rs` no longer spawns `curl`. A source-level grep in a release-contract script is acceptable if kept narrow; do not ban the word `curl` repository-wide because bootstrap documentation/installers legitimately use it.

## Part G — Measure footprint and make the tradeoff explicit

After the migration compiles and tests pass, repeat the exact Part A measurements.

Record in the plan/roadmap closure evidence:

```text
metric                            before       after       delta
stripped release bytes            ...          ...         ...
direct dependencies               ...          ...         ...
resolved packages                 ...          ...         ...
duplicate package families        ...          ...         ...
Tokio feature set                 ...          ...         ...
TLS/HTTP implementation           external     in-process  intentional
post-install curl requirement     yes          no          improvement
```

The expected outcome is that binary/package footprint may increase while external runtime dependency and HTTP maintenance burden decrease.

### G1. Material-growth review gate

Do not silently merge a large binary regression. If the same-target stripped release binary grows by either:

- at least 10%, or
- at least 1 MiB,

mark the plan `requires maintainer size decision` and record which eggfetch features/dependencies account for the growth. This is a review trigger, not an automatic rejection threshold.

Before asking for that decision, verify that no optional eggfetch feature, duplicate `base64` line, second TLS backend, compression codec, or unused protocol support is accidentally linked.

If the growth is below the trigger, still record it; do not describe the migration as a size reduction unless the measurement actually shows one.

## Part H — Documentation and release contract

Update only documentation that becomes factually stale.

At minimum review:

- `README.md` updater/install wording;
- `docs/release.md`;
- `architecture/cli-binaries.md`;
- `docs/verification.md`;
- `CHANGELOG.md` for the eventual release;
- `plans/roadmap.md` closure evidence.

Document the distinction clearly:

- bootstrap installers may still require `curl`/PowerShell/web tooling because they run before Eggsact exists;
- `eggsact update` itself no longer requires external `curl` after this migration.

Do not advertise proxy semantics more broadly than the qualified eggfetch behavior.

## Part I — Full qualification

Run the repository merge gate in the required order:

```bash
cargo fmt --all -- --check
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
```

Also run the relevant release checks from the repository release skill/documentation:

```bash
python3 scripts/check-release-contract.py
bash -n packaging/install.sh
shellcheck packaging/install.sh  # when available
cargo build --locked --release
python3 scripts/smoke-mcp-binary.py target/release/eggsact
```

Run the scheduled-equivalent platform compilation checks or otherwise verify:

- Linux x86-64;
- Linux AArch64 release workflow compatibility;
- macOS x86-64/aarch64 compilation;
- Windows x86-64 compilation.

This dependency touches TLS/native root handling and therefore needs cross-platform compilation evidence even though the updater code path is small.

Before release closure, run a controlled end-to-end update smoke against a real published Eggsact release flow on at least one supported Unix host:

```text
installed older binary
-> `eggsact update`
-> crates.io version lookup
-> redirected GitHub asset download
-> checksum validation
-> candidate version validation
-> replacement
-> new `eggsact --version`
```

For Windows, preserve the existing deferred replacement mechanism and qualify it through the existing Windows release workflow/test strategy; do not redesign replacement merely because transport changed.

## Completion criteria

This plan is complete only when all are true:

- `src/update.rs` no longer launches `curl` for self-update networking;
- Eggsact consumes a published crates.io `eggfetch-core` version, not a git/path dependency;
- only the minimal qualified eggfetch features are enabled;
- HTTPS certificate/hostname verification remains enabled;
- redirects required by GitHub assets work;
- HTTPS -> HTTP redirect downgrade is rejected before downgraded I/O;
- environment proxy routing remains available through explicit eggfetch opt-in;
- connect timeout remains 10 seconds and total timeout remains 120 seconds;
- `404` release asset still triggers Cargo fallback;
- non-404 HTTP/transport failures remain hard failures;
- release binary downloads stream to disk and partial files are not consumed after failure;
- SHA-256 verification and candidate `--version` validation remain unchanged in authority;
- bootstrap installer behavior remains functional and separate;
- full merge/release gates pass;
- supported-platform compilation passes;
- exact dependency and binary-size deltas are recorded;
- any material size-growth trigger is explicitly reviewed rather than hidden;
- roadmap and changelog describe the actual tradeoff accurately.

## Closure / pruning rule

Once shipped and evidenced, update `plans/roadmap.md` with the qualified eggfetch version, implementation commit, dependency/binary deltas, test/workflow evidence, and the post-install `curl` removal result. Then prune this plan and Plan 01 according to the repository's normal planning convention; git history retains their execution detail.
