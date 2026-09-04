# CLI and Binary Distribution

eggsact is a single executable with a deliberately small hand-written parser.
The CLI owns calculator expressions, MCP stdio, diagnostics, self-update, and
read-only MCP client integration rendering. It does not supervise processes or
edit arbitrary client configuration files.

## Commands

```text
eggsact [--mcp | --diagnostics [--format json|text] | update | integrate <client> | expression]
```

`update` and `integrate` are reserved top-level commands. Everything else that
is not a recognized flag remains calculator input and is joined with spaces.

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

Only stable `major.minor.patch` versions are accepted. Network access is kept
out of the library dependency graph: the updater invokes the platform's
`curl`, bounds requests to 120 seconds, caps candidate execution at 10 seconds,
and uses the existing `sha2` dependency for hashing. A supported target with a
genuine asset HTTP 404, or an unsupported host, uses a staged exact-version
`cargo install` fallback. HTTP errors, TLS/DNS failures, missing checksums,
checksum mismatches, and wrong candidate identities are hard failures.

On Unix, the validated executable is copied beside the current binary and
atomically renamed into place. On Windows, a PowerShell helper waits for the
updater image to exit before replacing the executable; if an active MCP client
still holds the image, the error tells the operator to close/reconnect it and
retry. No privilege escalation is performed internally. A permission error
prints the exact `sudo <path> update` retry on Unix.

After success, the command explains that new MCP launches use the new image but
existing client-owned stdio sessions may continue using the old image until the
client reconnects. There is intentionally no `restart`, daemon, PID file,
service-manager unit, cron watchdog, or HTTP listener.

## Release target contract

The public binary names are versionless and are shared by the release workflow,
installers, and updater tests:

| Host | Target / asset | Build and runtime qualification |
|---|---|---|
| Linux x86-64 | `x86_64-unknown-linux-gnu` | Zig cross-build, glibc 2.17 link floor; staged smoke |
| Linux AArch64 | `aarch64-unknown-linux-gnu` | Zig cross-build, glibc 2.17 link floor; staged smoke |
| macOS Intel | `x86_64-apple-darwin` | native runner; staged smoke |
| macOS Apple Silicon | `aarch64-apple-darwin` | native runner; staged smoke |
| Windows x86-64 | `x86_64-pc-windows-msvc` | native runner; staged smoke |
| Linux ARMv7 | `armv7-unknown-linux-gnueabihf` | installer-recognized Cargo fallback; not published until qualified |

The glibc floor is an intentional initial build target, not a claim of
hardware testing. A real SBC result should be recorded before describing the
AArch64 path as hardware-qualified. ARMv7 requires its own executable/QEMU or
native result and is not silently included in a release.

## Release workflow

`.github/workflows/release-binaries.yml` is tag-driven and is the only workflow
with `contents: write`. It requires an existing `vX.Y.Z` tag, an exact tagged
checkout, a clean tree, and matching crates.io metadata before any build. It
never publishes crates, creates or moves tags, pushes source commits, or
publishes a GitHub release. Assembly creates or updates only a draft release.

Each staged binary is checked with `--version`, `--help`, and
`scripts/smoke-mcp-binary.py` before its sidecar is generated. The assembly job
requires one binary and checksum for each mandatory target and attaches the two
installer scripts. `scripts/check-release-contract.py` catches matrix drift.

The workflow does not build ARMv7 until its qualification gate is added.

## `generate-docs` binary

The separate `generate-docs` binary remains the registry documentation
generator. It is not involved in release binary assembly.
