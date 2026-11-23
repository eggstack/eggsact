# Installation and Updates

The installers use the latest stable release by default. Pass a release version
to pin an installation. See the platform table below for published binaries.

## Install

### macOS and Linux

```bash
curl --proto '=https' --tlsv1.2 -fsSL \
  https://github.com/eggstack/eggsact/releases/latest/download/install.sh | bash
```

To pin a version, replace `X.Y.Z` with the release version:

```bash
curl --proto '=https' --tlsv1.2 -fsSL \
  https://github.com/eggstack/eggsact/releases/latest/download/install.sh \
  | bash -s -- --version X.Y.Z
```

The installer requires Bash and `curl`. It installs to `/usr/local/bin` when run
as root and to `$HOME/.local/bin` for a regular user. It prints PATH guidance
when needed and does not edit shell startup files.

### Windows

Run in PowerShell:

```powershell
irm https://github.com/eggstack/eggsact/releases/latest/download/install.ps1 | iex
```

To pin a version, download its versioned `install.ps1` and run it with
`-Version X.Y.Z`; the inspect steps below show the commands. The installer uses
`%ProgramFiles%\Eggsact` for administrator installs and
`%LOCALAPPDATA%\Eggsact` for regular users. It prints PATH guidance when
needed.

### Cargo

Install from the published crate with Cargo if you prefer a source build or
need a platform without a published binary:

```bash
cargo install eggsact --locked
```

Cargo requires Rust 1.89.0 or newer. Installers use Cargo when the host has no
published binary or the expected release asset returns HTTP 404. Checksum,
version, and other download errors stop installation instead of triggering a
source build.

## Inspect before installing

For published binaries, the installers use HTTPS, verify the SHA-256 sidecar,
and check the candidate version before installation. A checksum detects
corruption but does not protect against a compromised release account because
the checksum and binary come from the same GitHub release. macOS binaries are
not signed or notarized. Windows binaries are not code-signed.

To inspect a versioned Unix installer before running it, replace `X.Y.Z` with
the release version:

```bash
curl --proto '=https' --tlsv1.2 -fsSL \
  https://github.com/eggstack/eggsact/releases/download/vX.Y.Z/install.sh \
  -o install.sh
less install.sh
bash install.sh --version X.Y.Z
```

For Windows, replace `X.Y.Z` with the release version, then download and inspect
the script before invoking it:

```powershell
Invoke-WebRequest `
  https://github.com/eggstack/eggsact/releases/download/vX.Y.Z/install.ps1 `
  -OutFile .\install.ps1
Get-Content .\install.ps1
& .\install.ps1 -Version X.Y.Z
```

## Published binaries

| Platform | Binary | Notes |
|---|---|---|
| Linux x86-64 | Yes | glibc 2.17 or newer |
| Linux AArch64 | Yes | glibc 2.17 or newer |
| macOS Intel | Yes | Unsigned and not notarized |
| macOS Apple Silicon | Yes | Unsigned and not notarized |
| Windows x86-64 | Yes | Not code-signed |
| Linux ARMv7 | No | Installer uses Cargo fallback |
| Windows ARM64 | No | Installer uses Cargo fallback |

Published binaries have stable asset names and matching `.sha256` files.

## Update

Run:

```bash
eggsact update
```

The command checks crates.io for the latest stable version, then downloads and
verifies the matching release binary when one is available. Unsupported hosts
and missing binary assets use an exact-version Cargo install. Other failures
stop the update.

On Windows, the command may report `update staged`. Close the running process so
the replacement can finish. MCP clients start a fresh process when they
reconnect. If replacement fails, the command leaves a status file with the
failure details and prints its path.

## MCP client setup

The `integrate` commands print setup instructions without editing client
configuration:

```bash
eggsact integrate list
eggsact integrate detect
eggsact integrate codex
eggsact integrate codex --discovery
```

Replace `codex` with `zed`, `claude`, `cursor`, `vscode`, or `opencode` for
another supported client. On Windows, `integrate claude` and `integrate vscode`
print a JSON configuration block and its target file rather than a shell
command. See [CLI usage](cli.md) for all commands and
[client integration details](../architecture/coding-agent-integration.md) for
configuration formats.
