# CLI and Binary Distribution

eggsact is a single executable with a deliberately small hand-written parser.
The CLI owns calculator expressions, MCP stdio, diagnostics, self-update, and
read-only MCP client integration rendering. It does not supervise processes or
edit arbitrary client configuration files.

## Commands

```text
eggsact [--mcp [--mcp-surface direct|discovery] | --diagnostics [--format json|text] | update | integrate [list | detect | <client> [--discovery]] | expression]
```

`update` and `integrate` are reserved top-level commands. Everything else that
is not a recognized flag remains calculator input and is joined with spaces.
Bare `integrate` prints usage plus the supported-client list; `integrate
list` lists clients, `integrate detect` reports PATH presence per client, and
`integrate <client> [--discovery]` renders a setup instruction without
mutating any files.

### `update`

`src/update.rs` contains the updater and pure release-contract helpers. The
authority chain is:

```text
crates.io max_stable_version
  -> exact GitHub vX.Y.Z asset
  -> SHA-256 sidecar
  -> candidate eggsact X.Y.Z --version
  -> executable replacement
```

Only stable `major.minor.patch` versions are accepted. Local
verified-transaction mechanics are owned by `eggup-core` 0.1.0 and network
acquisition by `eggup-eggfetch` 0.1.0 over `eggfetch-core` 0.2.0
(`http1,tls-rustls,tls-native-roots,proxy`); direct `eggfetch-core` use remains
only in the policy test harness. `src/update.rs` keeps release/update policy
(version selection, asset naming, Cargo fallback, checksum sidecar parsing, CLI
presentation). Transport policy (single configuration point):
HTTP/1 only, redirects followed with strict HTTPS -> HTTP downgrade rejection,
native roots with packaged WebPKI fallback and full certificate/hostname
verification, explicit opt-in environment proxy routing (invalid proxy fails
closed), distinct 10-second connect and 120-second total wall-clock timeouts,
no retries, and streamed release-binary downloads (small metadata/checksum
bodies bounded at 1 MiB / 64 KiB, authoritative even when `Content-Length` is
absent or false). Staging, SHA-256 integrity verification, bounded 10-second
candidate `--version` validation with cleared environment and exact
`eggsact X.Y.Z` identity, current-executable ownership proof, mutation locking
with backup/rollback, and structured receipts are owned by Eggup. No external
`curl` is required after install. Bootstrap installers (`packaging/install.*`)
still use external download tooling because they run before Eggsact exists. A
supported target with a genuine asset HTTP 404, or an unsupported host, uses a
staged exact-version `cargo install` fallback. Checksum/TLS/timeout/5xx
failures never fall back; HTTP errors, TLS/DNS failures, missing checksums,
checksum mismatches, and wrong candidate identities are hard failures.

On Unix, the validated candidate is committed through the Eggup verified
transaction (prepare/verify/validate/commit with ownership proof and locking):
`Committed` replaces the live binary, `RolledBack` preserves the previous
version, and `RecoveryRequired` leaves evidence at a reported path with the
lock retained. On Windows, the running image cannot be renamed, so the
validated staged executable is handed to the existing detached PowerShell
helper, which waits for the updater image to exit and retries the replacement
for a bounded period.
The CLI reports `update staged` rather than `updated`; the helper removes its
success marker after replacement and leaves a status file containing an
actionable failure if the move cannot complete. The message names that file
and tells operators to close active MCP clients and retry from an Administrator
PowerShell. The helper never enumerates or kills processes. No privilege
escalation is performed internally. A permission error prints the exact
`sudo <path> update` retry on Unix.

After success, the command explains that new MCP launches use the new image but
existing client-owned stdio sessions may continue using the old image until the
client reconnects. There is intentionally no `restart`, daemon, PID file,
service-manager unit, cron watchdog, or HTTP listener.

## Release target contract

The public binary names are versionless and are shared by the release workflow,
installers, and updater tests:

| Host | Target / asset | Build and runtime qualification |
|---|---|---|
| Linux x86-64 | `x86_64-unknown-linux-gnu` | pinned Zig cross-build, glibc 2.17 link floor; staged smoke |
| Linux AArch64 | `aarch64-unknown-linux-gnu` | native ARM runner build/smoke, pinned Zig, glibc 2.17 link floor |
| macOS Intel | `x86_64-apple-darwin` | native runner; staged smoke |
| macOS Apple Silicon | `aarch64-apple-darwin` | native runner; staged smoke |
| Windows x86-64 | `x86_64-pc-windows-msvc` | native runner; staged smoke |
| Linux ARMv7 | `armv7-unknown-linux-gnueabihf` | installer-recognized Cargo fallback; not published until qualified |

The v1.2.4 release qualified the five published targets, including native
AArch64 build and executable smoke on `ubuntu-24.04-arm`. The glibc 2.17 floor
is an intentional compatibility target, not a claim of broad hardware
coverage. ARMv7 requires its own executable/QEMU or native result and remains
installer-recognized Cargo fallback only.

## Release workflow

`.github/workflows/release-binaries.yml` is tag-driven and is the only workflow
with `contents: write`. It requires an existing `vX.Y.Z` tag, an exact tagged
checkout, a clean tree, and matching crates.io metadata before any build. It
never publishes crates, creates or moves tags, pushes source commits, or
publishes a GitHub release. Assembly creates or updates only a draft release.
The first successful five-target run was workflow `33944943782` for tag
`v1.2.4`; the draft was inspected and published after all jobs passed.

Linux release tooling uses an explicitly downloaded and SHA-256-pinned Zig
release plus a pinned `cargo-zigbuild` version. The workflow checks the runner
architecture before executable smoke, so cross-compilation alone cannot
qualify an artifact. Each staged binary is checked with `--version`, `--help`, and
`scripts/smoke-mcp-binary.py` before its sidecar is generated. The assembly job
requires one binary and checksum for each mandatory target and attaches the two
installer scripts. Zig archives are extracted into a fixed temporary directory
with their architecture-qualified wrapper directory stripped; the same path
is used for `GITHUB_PATH` and the direct version check.
`scripts/check-release-contract.py` catches matrix and Zig bootstrap contract
drift.

The workflow does not build ARMv7 until its qualification gate is added.

## `generate-docs` binary

The separate `generate-docs` binary remains the registry documentation
generator. It is not involved in release binary assembly. It accepts
`--check` (fail if generated blocks are stale, for CI) and `--output-dir
<dir>` (rewrite paths relative to `<dir>` instead of the current directory,
used by release tooling). The canonical invocations (always `--locked`) are
`cargo run --locked --features dev-tools --bin generate-docs` to regenerate
and the same command with `-- --check` as the CI freshness gate.
