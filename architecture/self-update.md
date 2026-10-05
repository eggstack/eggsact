# Self-Update and Client Integration

`eggsact update` replaces the running binary with the latest stable release, and
`eggsact integrate` renders MCP setup text for a client. They are the only two
commands that touch anything outside the process, and both are deliberately
narrow. The trust model has four properties, and the rest of this document is an
account of how the code maintains them:

1. **Verified download.** A candidate is never committed because a download
   succeeded. It must satisfy SHA-256 size/digest evidence, a bounded
   `--version` check printing the exact authorized identity, and proof that the
   destination really is the running `eggsact` executable.
2. **No daemon.** No `restart` subcommand, PID file, service-manager unit,
   watchdog, or HTTP listener. Success is a file replacement plus an explanation
   that new MCP launches use the new image.
3. **No silent config edits.** `integrate` only prints text. It never writes a
   client config file and never installs a background process.
4. **Bounded, non-adaptive transport.** One configuration point, explicit
   timeouts, no retry policy, no compression or cookies. Retries and new fetch
   features require measurement, not taste (see
   [performance.md](performance.md)).

Source ownership: [`src/update.rs`](../src/update.rs) (policy and CLI
presentation), [`src/integrate.rs`](../src/integrate.rs) (renderers),
[`src/main.rs`](../src/main.rs) (parsing and dispatch). The release process that
*produces* what the updater consumes is in [../docs/release.md](../docs/release.md)
and is not duplicated here.

## Architecture overview

The updater is manifest-first. Eggsact selects and authorizes a release *before*
any manifest or artifact URL is constructed; the manifest then describes the
release that was already chosen and can never move that choice.

```text
eggsact update
  run()  →  current-thread Tokio runtime "eggsact-update"          (update.rs:1198)
  run_async()                                                      (update.rs:1213)
    │
    ├─ env::current_exe() ; parse_stable_version(CARGO_PKG_VERSION)
    ├─ crates_latest_version_async()   ← metadata, bounded 1 MiB   (update.rs:912)
    │     GET https://crates.io/api/v1/crates/eggsact
    │     json → crate.max_stable_version → parse_stable_version
    ├─ is_already_current(latest <= current)? → print + exit 0     (update.rs:1209)
    │     (no manifest or artifact download happens at all)
    ├─ unique_temp_dir("eggsact-update") → 0700 staging            (update.rs:550)
    │
    └─ target_for_host(OS, ARCH)?                                    (update.rs:150)
         │
         ├─ None ─→ cargo_candidate(staging, latest) + eggup_hash  (update.rs:924)
         │          (unsupported host → exact-version cargo install)
         │
         └─ Some ─→ prepare_candidate_async(staging, RELEASE_ORIGIN, …)
                                                                          (update.rs:974)
                │
                ├─ fetch_release_manifest(origin, latest)           (update.rs:706)
                │     GET {origin}/download/v{version}/release-manifest.json
                │     bounded by eggup_eggpack::MAX_MANIFEST_BYTES
                │     → MetadataFetch::Present | Absent | Err
                │
                ├─ Present ─→ prepare_manifest_candidate_async       (update.rs:788)
                │     resolve_manifest_projection (product/release/target binding)
                │     require exactly one Installable artifact
                │     projection.bind_requests → PlannedAcquisition per artifact
                │     acquire_manifest_artifact → fetch_artifact (streamed)
                │
                └─ Absent ──→ prepare_legacy_candidate_async        (update.rs:995)
                      exact 404 ONLY; asset + .sha256 sidecar, or Cargo
                │
                └─ commit: shared tail commit_artifact_set           (update.rs:1091)
                      root = current_exe.parent()
                      InstallPlan::new(product, release, root, set)
                        → prepare()          (private staging)
                        → verify_integrity() (SHA-256)
                        → validate()         (ExactIdentityValidator, 10s)
                      CommitOwnership(CurrentExeVerifier, AbsentPolicy::DenyCreate)
                        │
                        ├─ unix ────→ commit → TransactionDisposition
                        │              Committed         → "updated …"
                        │              RolledBack        → hard error, previous kept
                        │              RecoveryRequired  → hard error + evidence path
                        │
                        └─ windows ─→ staged_path → replace_current
                                       PowerShell helper, reports "update staged"
```
The manifest never selects a newer or different release. `resolve_manifest_projection`
compares the manifest's product and release against the release Eggsact already
authorized and treats a mismatch as a hard error, so a manifest cannot be used to
redirect an install to another product or version
([update.rs:718](../src/update.rs)).

## The Eggup transport stack

Four `eggup-*` crates plus `eggfetch-core` are pinned to a single git revision
so the stack can never resolve to mixed sources:

| Crate | Version (`Cargo.lock`) | Role in the update path |
|---|---|---|
| `eggup-acquisition` | 0.1.2 | `AcquisitionRequest`, `FetchLimits`, `FetchOutcome`, `CancelFlag`, `AcquisitionError`, `redact_url` |
| `eggup-eggfetch` | 0.1.2 | `EggfetchConfig::strict` → `EggfetchTransport`; the only runtime edge to `eggfetch-core` |
| `eggup-core` | 0.1.2 | `InstallPlan`, `ArtifactSet`/`ArtifactMember`, `ExactIdentityValidator`, `OwnershipVerifier`, `hash_file`, `TransactionDisposition` |
| `eggup-eggpack` | 0.1.2 | manifest parse/project/bind/materialize: `project_json`, `install_ids`, `bind_requests`, `MAX_MANIFEST_BYTES` |
| `eggfetch-core` | 0.2.0 | in-process HTTP/1 + TLS, underneath `eggup-eggfetch` |

All four are declared with `git = "https://github.com/eggstack/eggup.git"` at the
same `rev` ([Cargo.toml:50-53](../Cargo.toml)); the `eggup_dependency_identities_are_single_git_source`
and `eggup_tree_has_single_source_per_package_and_no_direct_producer_edge` tests
([update.rs:2942](../src/update.rs), [update.rs:2971](../src/update.rs)) enforce
that as a 40-character commit SHA, one identity per package, and no direct
`eggpack-*` producer edge. The manifest schema is reached only through
`eggup-eggpack`, so the consumer never links the producer's schema crate.

`eggfetch-core` is a **dev**-dependency of eggsact (`Cargo.toml:57`) with
features `http1`, `tls-rustls`, `tls-native-roots`, `proxy`. Every direct
reference in `src/update.rs` is `#[cfg(test)]`. At runtime the binary reaches
`eggfetch-core` transitively through `eggup-eggfetch`; the production code never
constructs a `Client`.

### Transport policy

`eggup_transport()` ([update.rs:578](../src/update.rs)) is the single
configuration point. It starts from `EggfetchConfig::strict()` and sets the
eggsact user agent (`USER_AGENT = "eggsact-self-update"`), the two timeouts,
`MAX_REDIRECTS`, and `ProxyDecision::FromEnvironment`, then pipes through
`EggfetchTransport::strict`:

- **HTTPS-downgrade rejection.** `strict()` denies an explicit `https` → `http`
  redirect before the second-hop I/O. The harness pins the same contract:
  `updater_selects_strict_redirect_policy` asserts `downgrade ==
  RedirectDowngradePolicy::Deny`, that an https → https hop is accepted, and an
  https → http hop rejected. `MAX_REDIRECTS` covers the normal 1–2 hop GitHub asset
  chain.
- **TLS.** `tls-rustls` with `tls-native-roots`, full certificate and hostname
  verification; TLS, certificate, and hostname failures get a distinct message
  class and never fall back to Cargo.
- **Proxy.** Explicit opt-in environment routing; an invalid proxy URL fails closed
  rather than silently connecting direct.
- **Distinct timeouts.** The 10s connect deadline and 120s total wall-clock cap are
  enforced together, not collapsed into one value — preserved from the historical
  `curl --connect-timeout 10` / `curl --max-time 120` contract.
- **Bounded bodies.** `METADATA_MAX_BYTES` 1 MiB, `CHECKSUM_MAX_BYTES` 64 KiB,
  `ARTIFACT_MAX_BYTES` 128 MiB (Eggsact-owned; observed release binaries are
  ~11–17 MiB). Small bodies are bounded in memory; the manifest path tightens the
  artifact ceiling to the manifest's exact size per artifact.
- **No retries, no compression, no cookies.** There is no retry policy anywhere in
  the path; adding one is a measured change, not a robustness tweak.
- **No external `curl`.** `updater_transport_never_spawns_curl` reads
  `src/update.rs` back through `include_str!` and fails if `curl` appears at all.

Every blocking sync acquisition runs inside `tokio::task::spawn_blocking`
(`eggup_fetch_metadata`, `eggup_download`, `acquire_manifest_artifact`), so the
current-thread runtime is never blocked. Errors are mapped by `map_eggup_error`
against the redacted URL only; `redact_url_for_error` delegates to
`eggup_acquisition::redact_url`.

## The eggpack manifest path (M003)

Added in `65c916b`, this is the release-manifest consumer path recorded in the
source as "Eggpack Interop M003".

### URL and shape

`MANIFEST_FILE_NAME = "release-manifest.json"`, published at the root of every
release since the Eggpack cutover, fetched as bounded metadata under
`eggup_eggpack::MAX_MANIFEST_BYTES`. The URL is built from the already-authorized
origin as `{origin}/download/v{version}/release-manifest.json`, so with the
production `RELEASE_ORIGIN` it resolves to
`https://github.com/eggstack/eggsact/releases/download/v1.2.7/release-manifest.json`
— asserted in `manifest_url_uses_producer_filename_under_authorized_origin`. The
producer emits the file during release staging
(`.github/workflows/release-binaries.yml`, `eggpack ci _prepare-stage
--release-manifest ./eggpack-finalized/release-manifest.json`).

The schema is the one the adapter qualifies against, pinned by the
producer-shaped fixture in [update.rs:2259](../src/update.rs):

```json
{"schema_version": 1, "product_id": "eggsact", "release_id": "9.9.9",
 "source_revision": "<40 hex>",
 "targets": [{"target": "x86_64-unknown-linux-gnu",
              "form": {"kind": "direct",
                       "artifact": {"name": "eggsact-x86_64-unknown-linux-gnu",
                                    "size": 42, "sha256": "<64 hex>"},
                       "install": "eggsact"}}]}
```

### Entry → platform asset mapping

1. `eggup_eggpack::project_json(bytes, target.rust_target)` selects the entry for
   the host triple; a manifest that does not serve it is a hard error, not a
   fallback signal.
2. `install_ids(&projection)` yields `(product, release)`. The product must equal
   `eggsact` and the release must equal the version Eggsact selected
   ([update.rs:730](../src/update.rs)).
3. The projection must be `ManifestProjection::Installable` and select exactly one
   artifact — eggsact is a direct single-binary release, so a multi-artifact
   selection or an `Archive` form fails closed, the latter noting that it
   "requires separately qualified extraction" ([update.rs:809](../src/update.rs)).
4. `bind_requests` pairs the manifest's `artifact.name` with an
   `AcquisitionRequest`, returning `PlannedAcquisition` values whose `limits` are
   tightened to the manifest's exact `size`. Tightening only ever narrows: a
   caller ceiling below the manifest size is rejected ("caller byte limit is
   below…"), and `manifest_artifact_ceiling_is_tightened_never_widened` asserts
   the planned limit equals the body length and stays under the baseline.
5. Each artifact is streamed to `staging/<artifact.name>` through
   `fetch_artifact`.

Because size and digest come from the manifest, the manifest branch does **no**
sidecar parsing. The `.sha256` sidecar is used only by the legacy branch below.

### The legacy-404 compatibility branch

`prepare_candidate_async` is a two-arm dispatch on a *structural* outcome, not
on parsed error text:

```rust
match fetch_release_manifest(origin, &latest).await? {
    MetadataFetch::Present(bytes) => { … manifest path … }
    MetadataFetch::Absent        => { … legacy sidecar path … }
}
```

`MetadataFetch::Absent` is produced only by `FetchOutcome::NotFound` from
`fetch_metadata` — an exact transport 404. Everything else propagates as a hard
error through `?`. Concretely:

- **Exact manifest 404 → legacy path.** The updater downloads the versioned asset,
  then its `{binary_url}.sha256` sidecar, parses the first whitespace token as a
  64-hex SHA-256 digest, hashes the download, and compares. That sidecar digest
  becomes the `IntegrityRequirement`, so this path keeps the same independent
  integrity evidence. `manifest_absence_uses_legacy_sidecar_path` drives the whole
  branch through a local fixture and commits successfully.
- **Manifest present but unusable → hard.** Malformed JSON, an unsupported
  `schema_version`, a non-UTF-8 body, or a body above `MAX_MANIFEST_BYTES` all fail
  inside `resolve_manifest_projection`; the tests assert each and that the error
  never mentions legacy.
- **Product / release / target mismatch → hard.** Asserted by
  `manifest_binding_rejects_identity_mismatch`.
- **Manifest-backed artifact 404 → hard, and explicitly refuses fallback.**
  `acquire_manifest_artifact` returns "release manifest artifact '<name>' is absent
  from the authorized release; refusing fallback to legacy evidence". This is the
  key asymmetry: a 404 on the *manifest* buys legacy compatibility, while a 404 on
  an artifact the manifest promised is an incomplete release. The test also asserts
  the message contains no `cargo`.
- **Single call site, structurally enforced.**
  `legacy_construction_lives_only_behind_manifest_absence` slices the source of
  `prepare_candidate_async` and asserts `MetadataFetch::Absent` appears and
  `prepare_legacy_candidate_async` occurs exactly once, inside that arm.

A naming caution for readers: the `M003` in this subsystem is *Eggpack Interop
M003* (source comments cite "M003 §9 Eggsact matrix rows" and "M003 §17"). It is
not the `M003 — Eggfetch 0.1.7 bump` milestone in
`plans/subsystems/distribution-update-release-roadmap.md`.

## Verification and safety

`commit_artifact_set` ([update.rs:1091](../src/update.rs)) is the shared tail for
both sources; only `ArtifactSet` construction differs. The order is fixed:

1. **Private staging.** `prepare()` stages outside the install root; the install
   root is `current_exe.parent()`, so canonical and renamed installations both
   update in place.
2. **SHA-256 integrity.** `verify_integrity()` enforces the `IntegrityRequirement`
   (manifest digest/size, legacy sidecar digest, or the Cargo self-measured digest).
   Under-sizing fails at adapter materialization; over-sizing is rejected during
   acquisition by the tightened ceiling.
3. **Bounded identity validation.** `ExactIdentityValidator` requires the
   candidate to print exactly `eggsact {latest}\n` within 10 seconds, environment
   cleared.
4. **Ownership proof.** `CurrentExeVerifier` returns `Owned` only when the
   destination canonicalizes to the running executable's canonical path and is a
   regular file; a symlink to a different binary is `Foreign`, a missing
   destination `Absent`, unreadable metadata `Unknown`.
5. **Replacement only.** `AbsentPolicy::DenyCreate`, so self-update can never be
   used to plant a file at a fresh path.
6. **Mutation locking with backup/rollback**, producing a structured receipt.

On Unix the disposition maps to three distinct outcomes, and a receipt is never
conflated with success:

| `TransactionDisposition` | CLI result |
|---|---|
| `Committed` | `ReplacementOutcome::Complete` → "updated eggsact from A to B" plus the reconnect note |
| `RolledBack` | hard error: "update rolled back (<phase>: <detail>); previous version preserved" |
| `RecoveryRequired` | hard error: "update requires manual recovery (<phase>: <detail>); evidence at <path>; lock retained" |

**A failed update leaves the existing binary intact.** Failures before the commit
are structurally incapable of touching the live file, and the tests assert the
bytes are unchanged: size mismatch, digest mismatch, and wrong candidate identity
each assert the stale `b"stale-bytes"` content survives, and
`manifest_ownership_conflict_fails_closed_without_mutation` forces a `RolledBack`
receipt through a symlinked foreign destination and asserts the real target is
untouched. Only after validation does the transaction swap, under a lock, with a
backup to roll back to.

`RecoveryRequired` is covered structurally rather than behaviorally: the source
comment at [update.rs:2856](../src/update.rs) records that it cannot be forced
deterministically, so `eggup_receipt_mapping_is_explicit` asserts the mapping
textually instead.

### Windows replacement

A running Windows image cannot be renamed, so the validated staged executable is
handed to the existing detached PowerShell helper rather than replaced inline.
`replace_current` copies the candidate to a sibling path derived from the running
executable's name (its extension replaced with `eggsact-update-<pid>.exe`),
derives a status path the same way (`eggsact-update-<pid>.status`), and spawns
`powershell.exe -NoProfile -NonInteractive -Command <script>` with stdout and
stderr discarded. The script waits for the updater's PID to exit
(`Get-Process -Id $p`, 100 ms polling), retries `Move-Item -Force` up to 50 times
at 100 ms intervals, removes the status file on success, and otherwise writes
`failed: <message>` and exits non-zero. It never enumerates or kills processes —
`windows_replacement_script_waits_and_records_failure_without_killing` asserts no
`Stop-Process` and no `taskkill`.

The CLI reports `update staged` rather than `updated`, names the status file, and
tells the operator to close active MCP clients and retry from an Administrator
PowerShell. `permission_error` / `retry_command` do the same for a failed replace:
on Unix the message prints the exact `sudo <path> update` retry; on Windows the
close-clients/Administrator-PowerShell instruction. No privilege escalation is
performed internally.

## Bootstrap versus in-process boundary

`packaging/install.sh` and `packaging/install.ps1` still use external download
tooling: `install.sh` hard-requires `curl` (`command -v curl … || exit 1`) and
invokes it with `--proto '=https' --tlsv1.2 --silent --show-error --location`;
`install.ps1` uses `Invoke-WebRequest -UseBasicParsing` for both the binary and
its `.sha256` sidecar.

That is not an inconsistency. The installers run *before* eggsact exists, so
there is no in-process HTTP stack to use yet. The boundary is the point: once the
binary is installed the external-tooling dependency is gone, which is what "no
external `curl` after install" means. The two are allowed to have different
transports only because they run on opposite sides of that transition.
`install.*` also recognizes the versionless asset names via
`target_name_for_installer`, including ARMv7, which the updater deliberately does
not publish — see [Release target contract](#release-target-contract).

## `eggsact integrate`

`src/integrate.rs` is 258 lines of pure rendering. It reads `PATH` and
`current_exe()` and writes to stdout only. The guarantee is stated in the
program itself: after rendering, it prints "This command/config is an
instruction only; eggsact does not edit client configuration."

| Client | Rendered form |
|---|---|
| `zed` | `context_servers` JSON with `command` / `args` / `env` |
| `codex` | `[mcp_servers.eggsact]` TOML with `command` / `args` |
| `claude` | `claude mcp add eggsact -- <path> --mcp` |
| `cursor` | `mcpServers` JSON |
| `vscode` | `code --add-mcp '{"name":"eggsact",…}'` |
| `opencode` | `opencode.jsonc` shape with `$schema`, `type: "local"`, command array |

Subcommands: bare `integrate` prints usage plus the client list; `integrate list`
lists clients with descriptions; `integrate detect` reports PATH presence per
client, mapping `vscode` → `code` and `cursor` → `cursor-agent` and also accepting
`.exe`, `.cmd`, and `.bat` on Windows; `integrate <client>` renders one client. An
unknown client is an error listing the supported names, and an empty executable
path is rejected before rendering.

Paths are escaped: JSON via `serde_json::to_string`, shell command forms via
`shell_quote` (`'…'` with `'\''` on Unix, `"…"` with `\"` on Windows).

`--discovery` renders startup args as
`["--mcp", "--mcp-surface", "discovery"]` instead of `["--mcp"]`; direct rendering
stays the default for 1.x compatibility. `render` remains the direct single source
of truth and `render_with_surface` is the explicit discovery variant.
`discovery_renderers_emit_surface_args` asserts all six clients emit the flag in
discovery mode and that direct mode does not.

## CLI surface

Parsing is a hand-written slice match in `parse_args`
([main.rs:27](../src/main.rs)). `--mcp` is scanned first, then `--mcp-surface`
is required to accompany it.

| Invocation | Result |
|---|---|
| `eggsact` / `-h` / `--help` | usage |
| `-V` / `--version` | `eggsact <CARGO_PKG_VERSION>` |
| `--mcp [--mcp-surface direct\|discovery]` | MCP stdio server; default `direct` |
| `--mcp-surface …` without `--mcp` | error, exit 2 |
| `--mcp` with any other argument | error, exit 2 |
| `update` | self-update |
| `integrate` | usage + client list |
| `integrate <client>` | direct render |
| `integrate <client> --discovery` | discovery render |
| `--diagnostics [--format json\|text]` | diagnostics; `--format` is order-independent, default `text` |
| `--format …` without `--diagnostics` | error, exit 2 |
| anything else | calculator expression, args joined with spaces |

`--mcp-surface` takes exactly one value; a repeated flag or a missing value is an
error, and an unknown value reports "expected direct or discovery". An explicit
CLI surface overrides `EGGSACT_MCP_SURFACE`; the flag is re-scanned in `main` so
the `Direct` default never clobbers a valid env-provided `Discovery`.

Exit codes: `update` failure exits 1; `integrate` failure exits 2; a parse error
exits 2 after printing usage. The `Discovery` surface is presentation-only —
see [mcp-server.md](mcp-server.md).

## Release target contract

Five targets are published, named by `RELEASE_TARGETS` and shared with the release
workflow, the installers, and the updater tests; the table and per-host build
qualification are in [cli-binaries.md](cli-binaries.md). Two updater-specific
facts matter here: asset names are versionless, so `releases/latest/download` and
`releases/download/vX.Y.Z` resolve the same file; and ARMv7 is recognized by
`target_name_for_installer` but absent from `RELEASE_TARGETS`, so
`target_for_host("linux", "arm")` returns `None` and the updater takes the Cargo
fallback.

## Test and evidence coverage

All self-update and integrate coverage is **in-crate unit tests** in
`#[cfg(test)] mod tests` at the bottom of each source file
([update.rs:1273](../src/update.rs), [integrate.rs:212](../src/integrate.rs),
`main.rs`), not the `tests/` integration crate. `grep` over `tests/` finds no
reference to `integrate::`, `update::run`, or the integrate subcommands, so this
subsystem is not exercised by `cargo test --test lib`. See
[testing.md](testing.md) for the suite layout.

Covered by tests (all in-crate `#[cfg(test)]`):

- **Release and dependency contract**: `RELEASE_TARGETS` vs
  `release/eggpack/distribution.toml`; exact asset URLs; strict version and
  `--version` parsing; checksum token parsing; HTTP status classification
  (`200`/`404`/`503`, plus 400/403/429 never falling back); four `eggup-*` deps at
  one 40-char rev with one resolved source identity per package and no direct
  `eggpack-*` edge; `/BREPRO` and `/DEBUG:NONE` present for
  `x86_64-pc-windows-msvc` in `.cargo/config.toml` and no other target carrying
  `rustflags`.
- **Transport policy**: distinct connect/total timeouts; strict redirects with
  downgrade denial; proxy selection, `NO_PROXY` bypass, invalid-proxy failure;
  `EggfetchConfig` carrying the user agent, timeouts, and redirect bound; no
  `curl` string in the source. A local `TcpListener` harness on `127.0.0.1:0`
  (never a public endpoint) covers status classification, redirect completion and
  loop bounding, chunk-order streaming with partial-file cleanup, body bounds
  including a chunked body with no `Content-Length`, and post-header stalls timing
  out on both buffered and streamed paths.
- **Ownership and atomicity**: `Owned`/`Foreign`/`Absent`; full manifest-path
  commit; renamed-executable in-place update with no `eggsact` basename created;
  size, digest, and identity mismatches failing before commit with stale bytes
  intact; ownership conflict producing `RolledBack`; legacy 404 path committing.
- **Structural guards** (`eggup_generic_machinery_is_deleted_not_wrapped`,
  `eggup_receipt_mapping_is_explicit`): no reintroduction of a
  `#[cfg(unix)] fn replace_current(` definition or the local `sha256_file` /
  `run_bounded` / `wait_bounded` helpers; the three disposition mappings and
  `AbsentPolicy::DenyCreate` stay present. A separate guard requires
  `bytes_stream` and forbids `.bytes().await` inside `download_to`. The
  byte-level Windows reproducibility proof is the `windows-reproducibility` job of
  `.github/workflows/maintenance.yml`, not a unit test.

Gaps, stated honestly:

- **No live endpoint coverage.** `crates_latest_version_async` is untested because
  it would require contacting crates.io; everything else uses local fixtures.
- **Production error mapping is untested.** `map_eggup_error` and
  `redact_url_for_error` have no direct test — the redaction test exercises the
  `#[cfg(test)]` harness function `map_transport_error`, not the production path.
- **The streaming guard targets the harness.**
  `release_binary_path_streams_without_whole_body_buffer` slices `download_to`,
  which is `#[cfg(test)]`; production streaming lives inside `eggup-eggfetch`, so
  the guard is a proxy rather than a direct proof.
- **`RecoveryRequired` is not behaviorally forced**, as noted above.
- **Windows commit paths are unexercised.** Tests that execute a candidate or
  commit are `#[cfg(unix)]`, and the source records no Windows CI lane for this
  consumer; Windows has script and policy unit tests only.
- **`run_async` orchestration is not unit tested.** Version selection, staging
  setup, and staging cleanup are verified by reading, and end-to-end by the release
  smoke, not by a test.

Parity is excluded from CI (the Python `eggcalc` sibling is not in CI) and
performance evidence is maintainer-run and non-gating
([performance.md](performance.md)). Neither constrains this subsystem: the update
path has no benchmark and no host-specific timing thresholds.

## Invariants and review checklist

1. **No HTTP in the library or MCP API.** Self-update transport is reachable only
   from the `update` subcommand; `eggsact::lib` and the MCP server must never gain
   an HTTP client, listener, or fetch surface.
2. **No retries without measurement.** There is no retry policy. Adding one
   requires a benchmark, a stated threat model, and a maintainer decision.
3. **No daemon.** No restart command, PID file, service unit, watchdog, or
   listener. Success is a file replacement plus an explanation.
4. **No client config mutation.** `integrate` renders text to stdout; it must
   never write a client config or launch an installer.
5. **Manifest cannot select the release.** Product, release, and target stay bound
   to the already-authorized release; mismatch is a hard error.
6. **Only an exact manifest 404 reaches legacy, and only a genuine artifact 404 or
   an unsupported host reaches Cargo.** Checksum, TLS, timeout, 5xx, identity,
   ownership, and manifest-artifact-absence failures stay hard.
7. **Replacement only.** `AbsentPolicy::DenyCreate` and `CurrentExeVerifier` keep
   the updater from creating or overwriting a destination that is not the running
   binary.
8. **One transport configuration point.** TLS, redirect, timeout, proxy, and
   bounds change in `eggup_transport()` / `eggup_limits()` only.
9. **One Eggup revision.** All four `eggup-*` deps stay on a single 40-char rev,
   with no direct producer-schema edge.
10. **No external download tooling after install.** The "no `curl`" guard stays
    green; `packaging/install.*` is exempt because it runs before the binary
    exists.
11. **Published targets match the producer contract.** Any change to
    `release/eggpack/distribution.toml` must land in `RELEASE_TARGETS` in the same
    change, or `published_targets_match_eggpack_contract` fails.
12. **Staging is always cleaned up**, on both the success and error paths.

## Related drift

Eight documentation errors were found while verifying this subsystem and have been
fixed in place. They are listed because each one describes the M003 Eggpack cutover
that this file documents, and a stale version of any of them would misdescribe the
current update path:

- `overview.md` listed `src/update.rs` as ~1250 lines (actual: 3029) and omitted
  `eggup-eggpack` from the dependency set. `eggup-eggpack` is a **direct**
  dependency at the same pinned rev, and the test
  `eggup_tree_has_single_source_per_package_and_no_direct_producer_edge` asserts
  that direct edge.
- `overview.md`, `cli-binaries.md`, and the `AGENTS.md` Gotchas entry all stated
  the `eggup-*` crates were 0.1.0. `Cargo.lock` resolves all four — plus the
  transitive `eggup-archive` — to **0.1.2**. `eggfetch-core` was correctly 0.2.0.
- `overview.md` and `cli-binaries.md` listed `futures-util` among the runtime
  self-update dependencies. It is a **dev**-dependency, as is `eggfetch-core`;
  both are pulled in only by the policy test harness.
- `cli-binaries.md` presented the sidecar chain as *the* authority chain. Since
  M003 the chain first fetches `release-manifest.json`; the SHA-256 sidecar step
  is reachable **only** on an exact 404 from the manifest URL.
- `cli-binaries.md` said a genuine asset 404 falls back to a staged `cargo
  install`. True only on the legacy branch — on the manifest path an artifact 404
  is a hard error that refuses to fall back (`src/update.rs:775-777`), and the
  test asserts the message contains no `cargo`.
- `cli-binaries.md`'s list of policy owned by `src/update.rs` omitted
  manifest/legacy source selection, now its most consequential policy
  (`src/update.rs:6-8`).
- `overview.md`'s runtime dependency count was stale.

One naming trap for readers of the plans: `M003` in
`plans/subsystems/distribution-update-release-roadmap.md` is the **Eggfetch 0.1.7
bump**, which is unrelated to *Eggpack Interop M003* (the commit at HEAD,
`65c916b`). Confirm which one a plan means before citing it as evidence here.
