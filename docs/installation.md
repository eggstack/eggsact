# Installation and Updates

## Current installation path

The latest published release is available through crates.io. Until the first
binary-bearing GitHub Release is published, install from Cargo:

```bash
cargo install eggsact
```

The binary installers below are release-ready but should be used only after a
published binary release has been verified. A maintainer must update this page
with that release tag before documenting a live `latest` or exact-tag URL.

For the eventual Unix fast path:

```bash
curl --proto '=https' --tlsv1.2 -fsSL \
  https://github.com/eggstack/eggsact/releases/download/vX.Y.Z/install.sh \
  | bash -s -- --version X.Y.Z
```

The script intentionally requires Bash. Operators who prefer to inspect first
can download `install.sh`, review it, and run `bash install.sh --version X.Y.Z`.
It does not invoke `sudo` or edit shell startup files.

The eventual Windows PowerShell fast path is:

```powershell
irm https://github.com/eggstack/eggsact/releases/download/vX.Y.Z/install.ps1 | iex
```

The inspect-first form is:

```powershell
Invoke-WebRequest https://github.com/eggstack/eggsact/releases/download/vX.Y.Z/install.ps1 -OutFile install.ps1
Get-Content .\install.ps1
. .\install.ps1
```

Use `-Version X.Y.Z` for a pinned install. The PowerShell installer maps
Windows x86-64 to the prebuilt asset and uses the same Cargo fallback for
Windows ARM64 and other unsupported architectures.

## Published binary matrix

| Host | Asset | Status |
|---|---|---|
| Linux x86-64 / amd64 | `eggsact-x86_64-unknown-linux-gnu` | first binary release target; glibc 2.17 build floor |
| Linux AArch64 / arm64 | `eggsact-aarch64-unknown-linux-gnu` | first binary release target; native ARM smoke required |
| macOS Intel | `eggsact-x86_64-apple-darwin` | first binary release target; unsigned/not notarized |
| macOS Apple Silicon | `eggsact-aarch64-apple-darwin` | first binary release target; unsigned/not notarized |
| Windows x86-64 | `eggsact-x86_64-pc-windows-msvc.exe` | first binary release target; no code-signing claim |
| Linux ARMv7 | `armv7-unknown-linux-gnueabihf` | recognized, Cargo fallback only until qualification |

Raw executables use stable, versionless asset names. Each published executable
has a matching `.sha256` sidecar. A sidecar detects corruption and accidental
replacement; because it is fetched from the same GitHub release trust domain,
it is not a defense against a compromised release account.

Root Unix installs go to `/usr/local/bin/eggsact`. Ordinary users install to
`$HOME/.local/bin/eggsact`. The script prints PATH advice when needed and never
edits `.bashrc`, `.zshrc`, or another startup file. Administrator Windows
installs go to `%ProgramFiles%\Eggsact\eggsact.exe`; ordinary users go to
`%LOCALAPPDATA%\Eggsact\eggsact.exe`, with PATH advice only.

## Cargo and source fallback

Cargo is used only when the host is outside the published binary matrix or the
expected binary asset returns HTTP 404. Checksum, version, 5xx, TLS, DNS, and
other transport failures are hard errors and are never hidden by compiling a
different candidate. If Cargo is unavailable, the installer reports the
detected host and gives the Rust/source path.

## Self-update

```bash
eggsact update
```

The command reads `max_stable_version` from crates.io, then requests the exact
matching GitHub Release asset and checksum. It validates the checksum and exact
`eggsact X.Y.Z` candidate version before replacement. Unsupported hosts and
asset 404s use a staged exact-version `cargo install`; all other failures stop.

Permission errors print an elevated retry command. Eggsact never kills or
enumerates other processes. Unix replacement completes before `updated` is
printed. Windows reports `update staged` because the running image must exit;
the detached helper retries the move and removes its success marker, or leaves
an adjacent status file containing the failure. Read that path, close active
MCP clients, and retry from an Administrator PowerShell if needed. Existing
MCP stdio sessions may continue running the old image until their client
reconnects. There is no background updater or `restart` command.

## MCP client setup

Render a current, absolute-path setup instruction with:

```bash
eggsact integrate list
eggsact integrate detect
eggsact integrate zed       # or codex, claude, cursor, vscode, opencode
```

The command is read-only. It prints the exact current registration command or
JSON/JSONC/TOML snippet and does not edit client files. See
`architecture/coding-agent-integration.md` for the supported shapes.

## Stdio lifecycle note

`eggsact --mcp` is a client-owned stdio child process. Running it as a boot-time
system service has no attached MCP protocol stream and normally exits on EOF,
so this project intentionally ships no systemd, launchd, Windows SCM, cron, or
singleton-daemon integration.
