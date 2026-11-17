# Installation and Updates

The current binary release is v1.2.7. Installers use the latest release by
default. Pass a version to pin an installation.

## Install

### macOS and Linux

```bash
curl --proto '=https' --tlsv1.2 -fsSL \
  https://github.com/eggstack/eggsact/releases/latest/download/install.sh | bash
```

To pin v1.2.7, append the installer argument:

```bash
curl --proto '=https' --tlsv1.2 -fsSL \
  https://github.com/eggstack/eggsact/releases/latest/download/install.sh \
  | bash -s -- --version 1.2.7
```

The installer requires Bash and `curl`. It installs to `/usr/local/bin` when run
as root and to `$HOME/.local/bin` for a regular user. It prints PATH guidance
when needed and does not edit shell startup files.

### Windows

Run in PowerShell:

```powershell
irm https://github.com/eggstack/eggsact/releases/latest/download/install.ps1 | iex
```

Pin a version with `-Version 1.2.7`. The installer uses
`%ProgramFiles%\Eggsact` for administrator installs and
`%LOCALAPPDATA%\Eggsact` for regular users. It prints PATH guidance when needed.

### Cargo

Use Cargo if you prefer a source install or your system has no published binary:

```bash
cargo install eggsact
```

Cargo requires Rust 1.89.0 or newer. The installers use Cargo only when the host
has no published binary or the expected binary asset returns HTTP 404. Checksum,
version, and other transport errors stop installation instead of triggering a
source build.

## Inspect before installing

Both installers download binaries over HTTPS, verify the SHA-256 sidecar, and
check the candidate's version before installation. The checksum sidecar comes
from the same GitHub release, so it detects corruption but does not protect
against a compromised release account. macOS binaries are not signed or
notarized. Windows binaries are not code-signed.

To inspect the Unix installer for v1.2.7 before running it:

```bash
curl --proto '=https' --tlsv1.2 -fsSL \
  https://github.com/eggstack/eggsact/releases/download/v1.2.7/install.sh \
  -o install.sh
less install.sh
bash install.sh --version 1.2.7
```

For Windows, download and inspect the script before invoking it:

```powershell
Invoke-WebRequest `
  https://github.com/eggstack/eggsact/releases/download/v1.2.7/install.ps1 `
  -OutFile .\install.ps1
Get-Content .\install.ps1
. .\install.ps1 -Version 1.2.7
```

## Published binaries

| Platform | Binary | Notes |
|---|---|---|
| Linux x86-64 | Yes | glibc 2.17 or newer |
| Linux AArch64 | Yes | |
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
