# eggsact Roadmap

This is the single living planning document. Completed execution detail is
pruned once a line ships; git history retains the prior plan and evidence.
For current facts, read `AGENTS.md`, `architecture/overview.md`, and
`docs/verification.md` first.

## Purpose

eggsact is a deterministic local utility layer for coding agents: a CLI
calculator/utility binary, an MCP stdio server exposing curated tools, and an
in-process Rust library for harnesses. Keep it lightweight, bounded, local,
and exact-input/exact-output. MCP is a transport adapter over the deterministic
tool substrate, not a reason to accumulate unrelated agent features.

## Shipped foundations

- Single-crate Rust implementation with a single-source `ToolSpec` registry.
- 86 tools across 23 categories, with profile/audience/exposure filtering.
- Deterministic math, text, JSON, regex, path, shell, config, patch, repo,
  dependency, network, encoding, and fixed-offset temporal utilities.
- In-process `ToolRegistry` / `ExecutionContext` APIs and typed preflight
  wrappers for coding-agent harness integration.
- Stable machine codes, structured findings/verdicts, bounded execution,
  cooperative cancellation, truncation, and concurrent MCP stdio dispatch.
- Generated documentation, property tests, fuzz targets, MSRV/cargo-deny
  policy, and a manual release gate.

## Current release state

Latest published version: **1.2.6** (binaries for all five qualified targets;
see the [v1.2.6 release](https://github.com/eggstack/eggsact/releases/tag/v1.2.6)).
The deterministic utility, cron, and
binary-distribution corrective lines are closed. The original 80-tool
registration order remains an exact prefix, with the six later utilities in
the full profile only.

The first binary-bearing GitHub Release is published at
[`v1.2.4`](https://github.com/eggstack/eggsact/releases/tag/v1.2.4). Its five
qualified target binaries, SHA-256 sidecars, `install.sh`, and `install.ps1`
were produced by successful workflow
[`33944943782`](https://github.com/eggstack/eggsact/actions/runs/33944943782).

## Shipped consolidation line

The September 2026 maintenance/consolidation pass is complete. The repository
remains a single-crate, 86-tool design with no broad utility expansion. The
three consolidation plans have been pruned per the planning convention; git
history retains their execution detail and evidence.

- **Typed-first composition** (`consolidation-01`): `src/services/`
  (fingerprint, newline, security, repo, patch analysis) over `text/`/`calc`
  cores; `tools/*` adapters parse input, call typed cores/services, and build
  the wire shape once at the boundary. No adapter-to-adapter JSON composition
  except three intentional same-module reuses documented in
  `architecture/tools.md`.
- **Shared analysis** (`consolidation-02`): `RepoFacts` is the canonical
  ecosystem/path/language classifier; `PatchAnalysis` parses each unified
  diff once into neutral facts; differential tests guard against cross-tool
  semantic drift.
- **API/config/build surface** (`consolidation-03`):
  - recommended Rust hierarchy (`calc`/root → typed `text` →
    `ToolRegistry`/execution contexts → typed `preflight` → MCP server) is
    documented in `docs/library-api.md` and `architecture/overview.md`; raw
    `tools::*` handlers stay `pub` for 1.x compatibility but are documented
    as adapter internals with no visibility reduction in 1.x;
  - typed `DependencyPreflight` added (ecosystem, added/removed/version/
    source changes, hook changes, findings, verdict, machine code);
    patch-review and repo-audit facades were evaluated and declined (their
    projections already answer those workflows through `ToolRegistry`);
  - `config_file_inspect` reports an additive `analysis_mode`
    (`"parser"` vs `"heuristic"`); YAML stays heuristic-only with no parser
    dependency and `config_preflight` intentionally excludes YAML;
  - coarse feature gating was measured and explicitly declined: 19 direct /
    73 total dependency crates (`Cargo.lock`), 7.4 MiB stripped release binary, ~58s
    warm-cache release build; tokio/serde_json/toml/regex/unicode span all
    layers and the shipped binary stays full-featured, so cfg-gating would
    add CI-matrix and conditional-compilation cost for no binary win;
  - architecture drift corrected (stale `mcp::tools` wording, wrapper
    counts); `sync_pool.rs` references verified current; no lightweight
    path-reference check was added (the real drift was module-path
    vocabulary, which a file-path checker would not catch);
  - `json_query` stays deprecated-but-compatible in 1.x with no promotion in
    default docs; calculator context isolation semantics are unchanged.

## Binary distribution closure

The C8 Zig bootstrap correction is implemented in `6658702`. The release
workflow now extracts each pinned Zig 0.14.1 archive into a fixed directory,
strips the archive wrapper directory, and uses that path consistently for
`GITHUB_PATH` and `zig version`. `scripts/check-release-contract.py` guards
the invariant. Follow-up fixes discovered only by real release execution
added a crates.io user agent, accepted macOS's `arm64` architecture spelling,
and use `shasum -a 256` when `sha256sum` is unavailable.

The published matrix is:

| Host | Target | Qualification |
|---|---|---|
| Linux x86-64 | `x86_64-unknown-linux-gnu` | staged version/help/MCP smoke; glibc 2.17 floor |
| Linux AArch64 | `aarch64-unknown-linux-gnu` | native `ubuntu-24.04-arm` build and executable smoke; glibc 2.17 floor |
| macOS Intel | `x86_64-apple-darwin` | native staged smoke |
| macOS Apple Silicon | `aarch64-apple-darwin` | native staged smoke |
| Windows x86-64 | `x86_64-pc-windows-msvc` | native staged smoke |

Zig 0.14.1 and cargo-zigbuild 0.23.3 remain release-only tooling. ARMv7 is
recognized by the Unix installer but remains Cargo fallback/source-only until
its own executable, QEMU, or native qualification gate exists. Windows
installer parsing passed; Windows deferred self-update behavior was not
executed on this Linux host and remains documented as staged replacement.

The exact-tag Unix installer was run against v1.2.4 and installed a candidate
reporting `eggsact 1.2.4`. The exact-tag and
`releases/latest/download/install.sh` payloads were both fetched after
publication and matched. No runtime dependency or release-only tool was
added; the release assets are stripped standalone binaries (approximately
7.2–10.9 MiB).

The local release gate passed for 1.2.4 before publication. Ordinary CI passed
on the release-preparation and corrective commits, including runs
`33941151181`, `33941872259`, `33942657807`, `33943491822`, and
`33944382758`. The final binary workflow completed all target,
installer, checksum, smoke, and draft-assembly jobs in `33944943782`.

## Eggfetch self-update transport consolidation — shipped

A 2026-09-16 audit found that Eggsact did **not** carry `reqwest` or
another in-process HTTP client. `eggsact update` owned a small transport
wrapper around an external `curl` process. That transport is now consolidated
behind `eggfetch-core` to remove duplicated HTTP/TLS/subprocess maintenance
and make the post-install updater self-contained. This was always a
maintenance/runtime-dependency consolidation, not a size optimization: HTTP/TLS
moved inside the executable, so the binary/dependency footprint grew as
expected.

- **Qualified eggfetch**: `eggfetch-core` **0.1.6** (crates.io, Rust 1.89;
  upstream `eggstack/eggfetch` `181786de` + `2e988b5e`, CI `35184372773`).
  Minimal features `http1,tls-rustls,tls-native-roots,proxy`. No HTTP/2/3,
  compression, cookies, multipart, JSON, tracing (beyond
  `hyper-rustls/logging`), or retries. Single `base64` 0.23 line; single TLS
  backend (rustls 0.23.45 + ring via hyper-rustls 0.27).
- **Implementation**: `src/update.rs` only (private adapter, single
  configuration point). Policy preserved: crates.io authority, exact GitHub
  asset, SHA-256 sidecar, candidate `--version`, 404 -> Cargo fallback, staged
  Windows replacement. Transport: `eggsact-self-update` UA, HTTP/1 only,
  `RedirectPolicy::strict` (HTTPS -> HTTP rejected before second-hop I/O),
  native roots + WebPKI fallback with full verification, explicit
  `ProxyEnvironment::from_env` (invalid proxy fails closed), 10s connect /
  120s total via `Timeout::builder`, `automatic_decompression(false)`,
  streamed binary downloads with partial-file removal, 1 MiB metadata /
  64 KiB checksum bounds. `run()` builds a current-thread Tokio runtime and
  blocks on `run_async()`; the CLI is otherwise still synchronous.
- **Footprint (same host/toolchain/profile: x86_64-unknown-linux-gnu,
  rustc 1.98.1, release stripsymbols/lto-thin/codegen-units-1)**:

```text
metric                            before       after       delta
stripped release bytes            9_860_864    13_692_696  +3_831_832 (+38.9%, +3.65 MiB)
direct dependencies               19           21          +2 (eggfetch-core, futures-util)
resolved packages (Cargo.lock)    74           172         +98 (HTTP/TLS graph + dev url)
duplicate families                none         hashbrown x2, syn x2, webpki-roots x2*, windows-sys x2
Tokio features                    rt,macros,io-std,io-util,sync,time  +fs,+net (fs: async staging writes; net: eggfetch/hyper)
TLS/HTTP implementation           external curl  in-process hyper/rustls  intentional
post-install curl requirement     yes          no          improvement
```

`*` `webpki-roots` 0.26 wraps 1.x data (inherent per Plan 01); `hashbrown`
0.14 (dashmap) vs 0.17 (indexmap), `syn` 2 vs 3 (icu/displaydoc), and
`windows-sys` 0.52 vs 0.61 are disjoint transitive requirements, not version
drift. No unrelated lockfile upgrades (only additions).
- **Size-review gate**: +38.9% / +3.65 MiB exceeds the >=10% / >=1 MiB
  maintainer-review trigger. Reviewed: growth is the expected linked
  hyper/rustls/ring/webpki/url/icu stack, not an accidental feature
  (verified: no http2/http3/compression/cookies/multipart/json, no second TLS
  backend, no duplicate base64). Accepted as the documented consolidation
  tradeoff; do not describe as a size reduction.
- **Tests**: 16 updater unit/integration tests in `src/update.rs`
  (policy helpers unchanged + local-TCP status/redirect/streaming/timeout/
  proxy-config/no-curl guards; TLS-downgrade transport proof reused from
  eggfetch with an Eggsact strict-mode configuration assertion). Merge gate
  passes locally (fmt, generate-docs --check, clippy -D warnings, full tests
  --skip parity, doc tests). Release contract passes
  (`check-release-contract.py` now also guards `src/update.rs` against
  `Command::new("curl")` + requires `eggfetch-core` in `Cargo.toml`;
  `bash -n`, release build, MCP smoke 77 tools pass). `cargo-deny` passes
  after allowing the standard rustls trust-root licenses
  (ISC, BSD-3-Clause, CDLA-Permissive-2.0). Controlled live smoke on this
  Linux host: `target/release/eggsact update` against real crates.io reports
  `1.2.5 already current`, proving metadata/TLS/redirect/proxy/timeout path
  without a replacement. Cross-platform compilation needs
  native runners (Windows/macOS `cargo check` cannot link ring/macos SDK from
  this Linux host); the dependency's own cross-platform CI plus unchanged
  platform replacement paths are the evidence, with scheduled
  `maintenance.yml` as the gate.
- **Docs**: `README`, `docs/installation.md`, `docs/cli.md`,
  `architecture/cli-binaries.md`, `architecture/overview.md`, `AGENTS.md`,
  `CHANGELOG (Unreleased)`, and `scripts/check-release-contract.py` updated
  to state the self-contained updater vs bootstrap-installer distinction.
  `docs/verification.md` / `docs/release.md` required no transport change.
  Skills reviewed; no stale updater claims to prune.
- **Result**: `eggsact update` no longer requires external `curl`;
  bootstrap `packaging/install.*` still does. Detailed execution plans
  pruned per convention; git history retains them.

## Eggfetch 0.1.7 updater dependency correction — landed

Plan: `eggfetch-0.1.7-updater-dependency-bump.md` (closure record appended to
the plan file)

The original eggfetch self-update migration remains shipped and its 0.1.6
footprint/qualification record above stays historical. The narrow follow-up
advanced the updater to upstream `eggfetch-core 0.1.7`, which corrects
`Timeout.total` through response-body EOF/trailers while preserving the public
`ResponseBody` shape.

The follow-up kept the existing
`http1,tls-rustls,tls-native-roots,proxy` feature set and updater policy
unchanged. Small crates.io/checksum responses moved onto eggfetch's existing
request-local `max_decoded_body_size` enforcement (authoritative; the
`Content-Length` check and the removed caller-side post-buffer length check are
not the safety boundary), so absent/false `Content-Length` cannot cause
unbounded caller-side buffering. New local regressions prove post-header total
timeout on both the buffered metadata path (`get_small_text`) and the streamed
binary path (`download_to`), including partial-file cleanup.

Because upstream 0.1.7 also contains the intervening feature/dependency split
(`proxy` now owns `eggfetch-http-connect 0.1.7`), the handoff records a
targeted lockfile and comparable stripped-binary delta rather than assuming a
version-only graph change. Lockfile: `eggfetch-core 0.1.6 -> 0.1.7`,
`+eggfetch-http-connect 0.1.7`, `-dashmap 6.2.1` subgraph (`crossbeam-utils`,
`hashbrown 0.14.5`, `lock_api`, `parking_lot_core`, `redox_syscall`,
`scopeguard`); resolved packages 172 -> 166; `hashbrown` deduped to one family
(`syn`/`webpki-roots`/`windows-sys` duplicates unchanged). Comparable stripped
release builds (aarch64-apple-darwin, rustc 1.98.1, strip/lto-thin/cgu-1):
11_101_984 -> 11_101_760 bytes (-224 bytes, ~0%), below the >=1 MiB / >=10%
review trigger. This is not a new footprint program:
`proxy` currently re-enables the compatibility `http1` bundle, so a
`standard-http1` alias switch stayed explicitly out of scope.

Qualification was the ordinary merge gate, release-contract smoke, Rust 1.89
MSRV, cargo-deny, and Windows/macOS supported-platform compile checks. No new
Eggsact version was published from the bump; the corrected dependency reaches
users with the next normal Eggsact release after this line closed.

## Eggfetch 0.2.0 updater adoption — landed

Plan: `eggfetch-0.2.0-updater-adoption.md` (closure record appended to
the plan file)

The self-update transport was qualified on `eggfetch-core 0.1.7` and is now
adopted on synchronized upstream `0.2.0`, whose changelog records no
intentional breaking Rust API, feature-graph/default, MSRV, or dependency-
policy changes from 0.1.7. The release also contains the issue #24 streaming
decompression chunk-boundary correction on a decoder path Eggsact never
enables: the updater enables no `compression-*` features and keeps
`.automatic_decompression(false)`.

The adoption kept the existing
`http1,tls-rustls,tls-native-roots,proxy` feature set and updater policy
unchanged (strict redirect policy, environment proxy behavior, 10s connect /
120s total deadlines, authoritative small-body bounds, no-retry policy,
streamed binary downloads). `src/update.rs` compiled against 0.2.0 with no
source changes beyond the version reference; all 19 updater tests pass,
including the chunked no-length small-body bound and both post-header total-
timeout regressions. Lockfile movement is limited to `eggfetch-core` /
`eggfetch-http-connect 0.1.7 -> 0.2.0` (resolved packages 166 -> 166).
Comparable stripped release builds (aarch64-apple-darwin, rustc 1.98.1,
strip/lto-thin/cgu-1): 11_101_504 -> 11_101_552 bytes (+48 bytes, ~0%),
below the >=1 MiB / >=10% review trigger. Qualification was the ordinary
merge gate, release-contract smoke, Rust 1.89 MSRV, cargo-deny, Windows/macOS
supported-platform compile checks, and the latest-compatible lane. No Eggsact
release was published by the plan; the dependency reaches users in the next
normal Eggsact release after closure. The 0.1.6/0.1.7 migration records above
remain historical.

## MCP surface modernization — evaluation closure pending

Research on 2026-09-10 found that eggsact's internal capability architecture is
already consolidated, but its ordinary MCP presentation remains much broader
than current agent-tool guidance recommends: the `full` Model audience exposes
77 canonical tools and schema detail defaults to `full`. The implementation
keeps all underlying deterministic capabilities while adding a smaller,
searchable, protocol-current presentation surface.

The protocol/runtime/discovery implementation is complete. The deterministic
evaluation harness is also useful and passing. The remaining work is a narrow
evidence-closure pass: the prior `mcp-surface-03` roadmap entry was marked
complete before the model/client evidence required by its own completion
criteria existed. `mcp-surface-03c-evaluation-closure-corrective.md` is now the
only active MCP plan.

- **Protocol modernization** (`mcp-surface-01`, commit `35f7dc2e`) — complete:
  `2026-07-28` modern request envelopes coexist with legacy revisions;
  `server/discover`, modern cache/result metadata, standard Tool annotations,
  and schema-conforming `structuredContent` reuse the existing registry and
  bounded execution paths.
- **Progressive discovery** (`mcp-surface-02`, commit `a5c00b8d`) — complete:
  `McpSurface::{Direct, Discovery}` provides five profile-filtered pinned front
  doors plus MCP-only deterministic `tool_search` / `tool_invoke` facades.
  Profile/audience policy remains authoritative and direct remains the 1.x
  default.
- **Protocol/contract corrective architecture** (`mcp-surface-02c`, commit
  `77ff57a5`) — complete: race-safe one-era-per-stdio-connection state is
  separate from legacy `SessionState`, and the misleading generic
  `tool_invoke_output_schema()` was removed.
- **Stdio era-classification corrective** (`mcp-surface-02d`, commit
  `121babde`) — complete: `initialize` or other claim-less openings select the
  legacy path; modern claims select modern; cross-era requests use `-32022`;
  mismatched notifications are dropped before side effects. Local verification
  passed, and the official `@modelcontextprotocol/client@2.0.0` smoke selected
  modern with `versionNegotiation=auto` and legacy under default negotiation.
- **Deterministic discovery evaluation infrastructure** (commit `40222999`,
  extended under `03c` deterministic preparation) — landed and retained:
  `src/mcp/discovery_eval.rs` measures serialized Tool definitions; registry
  facts are generated; full/Model direct is 77 tools / 111,911 bytes versus
  discovery 7 / 6,088 bytes (5.44%, gate <=25%); semantic coverage is now
  registry-derived at 100% (76 stable full/Model targets, 88 positive intents
  with second phrasings for overlap-prone tools, 9 HarnessOnly/Hidden
  containment, no bare-name fixtures, deprecated stays migration/negative);
  retrieval holds top-1 ~96.6% / top-3 100% / top-5 100% with zero leaks;
  48 Model task scenarios plus 4 Model `must_not_expose` containment cases
  (`audience`/`kind`, plural `expected_tools`) replace the prior mixed 40;
  `scripts/score-discovery-traces.py` now enforces the plural contract with
  strict validation and `--pair` direct/discovery deltas plus 2pp
  noninferiority gates. CI run `34546867650` passed on `40222999`; local
  `test_discovery` (15 tests) passes on the expanded corpus.
- **`mcp-surface-03c-evaluation-closure-corrective.md` — P1, active
  (deterministic preparation complete; external evidence blocked).**
  Deterministic/scorer preparation above is done without changing protocol or
  capability architecture. Still required before pruning: matched
  direct-vs-discovery runs through real relevant clients for at least one
  current OpenAI coding/agent model and one current Anthropic coding/agent
  model; a controlled server-instructions with/without subset; and the
  evidence-backed rollout/default decision. Attempted 2026-09-11 in this
  environment: `codex`/`claude` CLIs exist but no provider credentials,
  subscriptions, or evaluation budget were available for ~200 model calls, so
  no traces were fabricated. Sanitized evidence belongs in
  `tests/fixtures/discovery_traces/` per its README; the scorer is ready but
  is not a substitute for recorded traces.

Deterministic semantic coverage is now complete: every stable Model-visible
capability has task-oriented discovery coverage, and the four prior
HarnessOnly scenario targets (`path_scope_check`, `shell_split`,
`patch_apply_check`, `unicode_policy_check`) are separated as Model
`must_not_expose` containment rather than successful discovery tasks.

Generated integrations intentionally remain direct while `03c` is active. This
is the conservative 1.x behavior and prevents an evidence gap from becoming a
user-facing default change. Discovery remains fully reachable and explicitly
selectable. If external model/client execution is unavailable, the corrective
must remain active or explicitly blocked; the existence of a trace scorer is
not a substitute for recorded traces.

The durable rollout gates remain:

```text
semantic stable-Model coverage        100%
retrieval top-1                       >=90%
retrieval top-3                       >=98%
retrieval top-5                       100%
Model -> HarnessOnly leaks            0
discovery/full direct byte ratio      <=25%
Model task scenarios                  >=40 after containment split
OpenAI direct/discovery pair          recorded + scored
Anthropic direct/discovery pair       recorded + scored
server-instructions A/B               recorded
```

After these gates are satisfied, the roadmap should record exact model/client
versions, success/selection/invalid-argument/retry/call-count results, the
instructions A/B result, and the final integration/default decision. Only then
is the MCP modernization/evaluation line closed and the `03c` plan eligible for
pruning.

## Performance optimization campaign — closure corrective complete

The sequential campaign planned at `23af42219ef1088ddeff080a2e07a3361e8468c3`
is complete in implementation commit `85e10bf`. The changes preserve the
public Rust/MCP shapes, schemas, profile and audience policy, legacy/modern
protocol behavior, discovery ranking, deterministic outputs, and bounded
execution semantics. Generated documentation remained unchanged because no
ToolSpec metadata changed.

The implementation itself remains landed, and the post-closure audit was
resolved narrowly under **`performance-04-closure-corrective.md` — P1** in
implementation commit `e8067ae`. The corrective did not revisit the broad
optimization design. It closed the five evidence/compatibility gaps: newline
position differential testing, persistent MCP warm measurement, production
input-budget measurement, representative diff/repository workloads, and an
explicit regex capture-name reuse decision. The patch correctness fix,
response/list/search/JSON/text-replace hot-path changes, flat dependency count,
and original qualification remain valid.

### Correctness evidence

The baseline nonzero-hunk fixture was reproduced in a detached baseline
worktree. The old implementation returned `B\nb\nc` for a hunk targeting line
2, while the expected result was `a\nB\nc`. The candidate adds focused
regressions for nonzero hunks, line-count-changing separated hunks, CRLF,
lenient EOF truncation, and fingerprints requested without result text. The
linear patch engine now uses original-source coordinates and emits the result
once; overlapping or out-of-order hunks fail deterministically.

### Historical pre-corrective benchmark evidence

The original dependency-free harness was run with 10 warmup and 50 measured iterations
on both the planning baseline and candidate using Rust 1.98.1,
`aarch64-apple-darwin`, macOS on Apple M4 Pro, release optimizations. Values
are arithmetic mean nanoseconds per operation from the stable `key=value`
records; they are evidence, not CI thresholds. The input and MCP rows below
are retained as historical evidence only: the former timed a JSON
serialization proxy and the latter spawned a fresh process per iteration, so
they are superseded by the corrective measurements that follow.

| Scenario | Baseline | Candidate | Change |
|---|---:|---:|---:|
| registry prepare/call | 3,014 | 2,598 | -13.8% |
| schema validation simple/nested | 532 / 597 | 244 / 215 | -54.1% / -64.0% |
| tools/list legacy/modern/compact | 301,845 / 388,136 / 775,545 | 306,870 / 379,924 / 650,073 | +1.7% / -2.1% / -16.2% |
| tools/list narrow name | 301,733 | 5,623 | -98.1% |
| discovery listing | 32,414 | 33,077 | +2.0% |
| response small/large/non-ASCII | 255 / 55,893 / 455 | 196 / 22,453 / 399 | -23.1% / -59.8% / -12.3% |
| response structured modern | 1,250 | 1,268 | +1.4% |
| input small/near-limit | 36 / 18,670 | 40 / 20,045 | +11.1% / +7.4% |
| text replace many matches | 291,154,474 | 1,375,555 | -99.5% |
| unordered JSON compare | 3,218 | 2,273 | -29.4% |
| JSON extract summary | 2,624 | 1,329 | -49.4% |
| patch late / 10 hunks / 100 hunks | 1,074,535 / 4,565,072 / 34,545,644 | 806,359 / 621,884 / 621,550 | -25.0% / -86.4% / -98.2% |
| tool search | 151,422 | 43,487 | -71.3% |
| repo facts | 3,465 | 3,664 | +5.7% |
| Unicode diff spans | 630 | 753 | +19.5% |
| named regex captures | 165 | 174 | +5.5% |
| codec hex | 1,801 | 1,078 | -40.1% |
| MCP stdio cheap call | 341,221,781 | 381,181,519 | +11.7% |

The neutral or slower rows are retained as honest evidence; no speculative
runtime-lock, calculator, near-match, transport, or dependency redesign was
accepted. Stripped release binary size was 11,101,696 bytes at baseline and
11,101,504 bytes for the candidate (-192 bytes). `Cargo.lock` contained 166
packages at both points.

### Corrective compatibility and representative benchmark evidence

The closure corrective used planning baseline `23af42219ef1088ddeff080a2e07a3361e8468c3`,
candidate `e8067ae`, Rust 1.98.1, `aarch64-apple-darwin`, macOS on Apple M4
Pro, and the same 10-warmup/50-repetition release-mode policy. Three
independent batches were run for each corrected scenario; the table reports
the median arithmetic mean in nanoseconds per operation.

The detached-baseline differential matrix matched the candidate for every
newline boundary. In particular, a match beginning on LF in `a\nb`, CR in
`a\rb`, CR and LF in `a\r\nb`, and the codepoint after each newline reports
the historical following-line position; `é\n😀` reports codepoint 2,
bytes 3..7, line 2, column 1. Empty matches at every boundary in LF, CR, and
CRLF inputs also match the baseline. The indexed implementation was corrected
to retain those semantics and the matrix is now a regression test.

| Corrected scenario | Baseline | Candidate | Change | Decision |
|---|---:|---:|---:|---|
| actual input-budget path, small | 1,519 | 1,105 | -27.3% | keep; production rejection path |
| actual input-budget path, near-limit | 40,315 | 40,931 | +1.5% | keep; production rejection path |
| `diff_spans` small Unicode | 560 | 783 | +39.8% | keep; +0.223 µs micro-case |
| `diff_spans` 640-codepoint multi-span | 736,713 | 471,760 | -36.0% | keep indexed offsets |
| `RepoFacts`, 100 mixed paths | 130,247 | 120,562 | -7.4% | keep one-pass analysis |
| `RepoFacts`, 1,000 mixed paths | 1,247,034 | 1,220,030 | -2.2% | keep one-pass analysis |
| named captures, 500 matches / 2 groups | 215,042 | 218,440 | +1.6% | decline metadata cache |
| persistent MCP warm call | 68,521 | 68,038 | -0.7% | keep; one child, correlated ids |

The warm MCP measurement now spawns one server outside timing, performs
warmup outside timing, then times sequential request serialization/flush,
server dispatch, response read, and JSON-RPC id correlation for each request.
The input rows call `call_json_with_budget` and assert `input_too_large`; they
no longer measure a duplicate `serde_json::to_vec` proxy. The multi-span and
100/1,000-path workloads justify retaining the diff and repository changes.
The repeated named-capture workload showed no material benefit from caching
capture names, so per-pattern metadata reuse is explicitly declined; no
public enum shape or regex behavior changes were needed.

### Qualification evidence

The corrective passed the focused text, diff, repository, and regex suites
(18, 144, 25, and 136 tests), then the required merge gate in order: fmt,
generated docs `--check`, all-features clippy with `-D warnings`, the full
non-parity suite (`3,764 passed, 1 ignored` with four test threads), and 11
doc tests. Release-contract validation, the release build, and the release
MCP smoke (77 tools) also passed. The dependency graph did not move, so the
previous cargo-deny advisories/bans/licenses/sources qualification remains
applicable. The corrected release binary is 11,101,504 bytes versus the
11,101,696-byte planning baseline; `Cargo.lock` remains at 166 packages. The
performance harness remains non-gating and is documented in
`architecture/performance.md`. Remote push CI run `35542158872` passed on
head `5fce6f345311ba194c915d811bcafd0426d46bfd` (10m1s; Linux correctness,
including the full merge gate).

## Future opportunities

1. Evaluate MCP Bundle/official MCP Registry distribution after the raw-binary
   release proves the deployment path; keep it non-blocking.
2. Consider first-class YAML only when a concrete workflow justifies its
   dependency and semantic surface.
3. Consider an explicit stateful `ToolRegistry` calculator session only if a
   real consumer needs persistent PRNG/memory/variable state; do not change
   isolated `ExecutionContext` semantics by default.

## Standing non-goals

- Not a general sandbox: classify risk, do not enforce it.
- Not every utility belongs in MCP: admit specification-heavy exact operations,
  not generic DevUtils feature parity.
- No systemd, launchd, Windows SCM, cron, PID files, restart command, or
  background daemon for the client-owned stdio server.
- No automatic crates.io publishing or tag creation in GitHub Actions.
- No apt/deb/rpm, Homebrew, winget, Chocolatey, MSI, container distribution,
  code-signing/notarization infrastructure, or Windows ARM64 release without a
  separate qualification decision.
