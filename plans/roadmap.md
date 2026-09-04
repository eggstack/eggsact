# eggsact Roadmap

This is the single living planning document. Completed execution detail is
pruned once a line ships; git history retains the prior plans and evidence.
For current architecture and workflow facts, read `AGENTS.md`,
`architecture/overview.md`, and `docs/verification.md` first.

## Purpose

eggsact is a deterministic local utility layer for coding agents: a CLI
calculator/utility binary, an MCP stdio server exposing curated tools to
models, and an in-process Rust library that harnesses call directly for
preflight and safety checks. Keep the project lightweight, bounded, local,
and exact-input/exact-output. MCP is a transport adapter over the deterministic
tool substrate, not a reason to accumulate unrelated agent features.

## Shipped foundations

- Single-crate Rust implementation with a single-source `ToolSpec` registry.
- 86 tools across 23 categories, with profile/audience/exposure filtering.
- Deterministic math, text, JSON, regex, path, shell, config, patch, repo,
  dependency, network, encoding, and fixed-offset temporal utilities.
- In-process `ToolRegistry` / `ExecutionContext` APIs and typed preflight
  wrappers for codegg-style harness integration.
- Stable machine codes, structured findings/verdicts, bounded execution,
  cooperative cancellation, truncation, and concurrent MCP stdio dispatch.
- Generated tool/profile documentation, property tests, fuzz targets,
  MSRV/cargo-deny policy, and a manual release gate.

## Current release state

Latest published version: **1.2.3**. Current main contains the six deterministic
utility additions introduced after 1.2.3:

- `ip_inspect`
- `cidr_inspect`
- `codec_convert`
- `radix_convert`
- `datetime_convert`
- `cron_inspect`

The expansion kept the original 80-tool registration order as an exact prefix,
added only `base64` and `time` as runtime dependencies, and increased the
stripped release binary from 8,051,496 to 8,229,476 bytes (+177,980; +2.21%).

The first corrective pass (`ae2be1d`, closed in `2cd6b60`) fixed IPv6 CIDR
cardinality, mapped-IPv6 classification, and the original DOM/DOW full-range
cron bug without changing registry/profile/dependency shape. Review after that
closure found one remaining cron dialect mismatch for star-step expressions,
closed by the C6 star-syntax correction below. The utility line is now
**closed** pending release.

---

# Active implementation line: binary distribution, self-update, and MCP client bootstrap

Status: **ready for implementation handoff**.

This line makes Eggsact fast to deploy across SBCs, developer workstations, and
small agent fleets without turning the project into a package-manager or daemon
platform. The primary user path becomes a copy/paste installer that selects a
verified prebuilt GitHub Release binary when available and falls back to Cargo
only when the host has no published binary. The installed binary gains a narrow
self-update command and client-integration helpers for common MCP hosts.

The implementation should reuse the proven release/install/update ideas from
`eggstack/gregg`, but simplify them for Eggsact's single binary and stdio-only
MCP lifecycle. Do not copy Gregg's daemon supervisor layer into Eggsact.

## Architectural decision: stdio MCP remains client-owned

Eggsact's current MCP transport reads JSON-RPC from process stdin and writes to
stdout. EOF ends the server. Zed, Codex, Claude Code, Cursor, VS Code, OpenCode,
and similar local MCP clients launch stdio servers as child processes and own
their pipes/lifetime.

Therefore this implementation line must **not** add:

- `croncheck`;
- `restart` that attempts to kill/relaunch MCP children;
- a systemd unit for `eggsact --mcp`;
- launchd or Windows SCM registration;
- a cron watchdog;
- PID files or a singleton daemon lock;
- an HTTP/socket listener solely to make supervision possible.

A boot-time system service running `eggsact --mcp` without an attached MCP
client is not useful: stdin has no client protocol stream and the process can
exit on EOF. Likewise an MCP subprocess cannot meaningfully restart itself and
reconnect to client-owned stdio pipes.

`eggsact update` updates the executable on disk. Existing client-owned MCP
processes may continue running the old image until the client reconnects or
restarts that MCP server. The command should state this clearly after a
successful replacement; it must not enumerate or kill arbitrary Eggsact
processes.

If a future requirement justifies one persistent Eggsact instance serving
remote/local clients over Streamable HTTP or another durable transport, that is
a separate architecture line. Only then should Gregg-style systemd/launchd/
Windows-service/cron lifecycle management be reconsidered.

## Required end state

For a published `vX.Y.Z` GitHub release, attach stable-name artifacts for the
validated matrix:

```text
eggsact-x86_64-unknown-linux-gnu
eggsact-x86_64-unknown-linux-gnu.sha256
eggsact-aarch64-unknown-linux-gnu
eggsact-aarch64-unknown-linux-gnu.sha256
eggsact-armv7-unknown-linux-gnueabihf
eggsact-armv7-unknown-linux-gnueabihf.sha256
eggsact-x86_64-apple-darwin
eggsact-x86_64-apple-darwin.sha256
eggsact-aarch64-apple-darwin
eggsact-aarch64-apple-darwin.sha256
eggsact-x86_64-pc-windows-msvc.exe
eggsact-x86_64-pc-windows-msvc.exe.sha256
install.sh
install.ps1
```

ARMv7 is desired because it materially helps older Raspberry Pi/32-bit SBC
images, but it is still qualification-gated. If GNU ARMv7 cannot be executed
truthfully in CI/QEMU or proves to have an unacceptable runtime compatibility
floor, omit the asset rather than publishing an unverified binary; the Unix
installer must still recognize `armv7l` and use Cargo fallback. Do not block the
AArch64 SBC release path on ARMv7 qualification.

The release tag provides the version namespace. Do not include the version in
asset filenames. Raw executables are preferred over tar/zip wrappers because
this keeps bootstrap logic to download -> verify -> execute smoke -> install.

The common Unix quick install should converge on:

```bash
curl --proto '=https' --tlsv1.2 -fsSL \
  https://github.com/eggstack/eggsact/releases/latest/download/install.sh | bash
```

Windows should have an equivalent PowerShell copy/paste path using the
published `install.ps1` asset.

## Scope and guardrails

### In scope

- a dedicated tag-driven GitHub binary release workflow;
- Linux x86-64, Linux AArch64, macOS Intel, macOS Apple Silicon, Windows
  x86-64, and qualification-gated Linux ARMv7 artifacts;
- an intentional Linux GNU/glibc portability floor;
- SHA-256 sidecars and candidate-version execution before installation;
- Unix and Windows bootstrap installers;
- binary-first, Cargo-second installation;
- `eggsact update` using crates.io as stable-version authority and GitHub
  Releases as binary source;
- narrow MCP-client integration/configuration helpers;
- documentation and release-runbook changes needed to make the above the
  primary deployment path;
- an optional later MCP Bundle/official MCP Registry assessment after the raw
  binary path is proven.

### Out of scope

- crates.io publication from CI;
- automatic version bumping or tag creation;
- automatic final GitHub Release publication;
- apt/deb/rpm, Homebrew, winget, Chocolatey, MSI, pkg/dmg, containers;
- code signing/notarization in this line;
- generalized release frameworks such as `cargo-dist` or `release-plz` unless
  this plan is explicitly amended after the handwritten approach proves less
  maintainable;
- board-specific Raspberry Pi/Le Potato builds;
- Windows ARM64 unless a concrete deployment need appears;
- background auto-update;
- service-manager integration for the current stdio server;
- production HTTP transport;
- client-specific editor extensions when ordinary MCP registration works;
- automatic editing of arbitrary shell startup files or unrelated IDE config.

## Implementation map

Expected new/changed implementation surfaces:

```text
.github/workflows/release-binaries.yml   release-only build/assembly workflow
packaging/install.sh                     canonical Linux/macOS bootstrap
packaging/install.ps1                    canonical Windows bootstrap
scripts/smoke-mcp-binary.py              staged-binary MCP stdio smoke, if useful
src/main.rs                              command recognition/dispatch
src/update.rs                            binary-first self-update implementation
src/integrate.rs                         MCP client detection/render/install helpers
Cargo.toml / Cargo.lock                  packaging exclusion + minimal updater dep if needed
README.md                                copy/paste install + client quick start
docs/installation.md                     detailed install/update/target behavior
docs/cli.md                              update/integrate command reference
docs/release.md                          manual crates -> tag -> draft binary release flow
docs/verification.md                     release/installer verification tier
architecture/cli-binaries.md             CLI and self-update architecture
architecture/coding-agent-integration.md client integration contract
AGENTS.md                                repository facts/commands/layout after landing
CHANGELOG.md                              user-visible release/deployment changes
```

Do not create a large `cli` framework merely because two subcommands are being
added. The current hand-written parser is still appropriate. Extract the update
and integration implementations out of `main.rs` so the entry point remains
readable.

`packaging/` should be excluded from crates.io packaging if it would otherwise
enter the crate artifact; GitHub Releases, not crates.io, are the installer
script distribution surface.

## P0 — freeze the target and asset contract

Use one public mapping everywhere: workflow staging, installer detection, and
self-update target detection.

| Host | Rust target / asset suffix | Initial build policy |
|---|---|---|
| Linux x86-64 | `x86_64-unknown-linux-gnu` | GNU binary with conservative glibc floor |
| Linux ARM64 | `aarch64-unknown-linux-gnu` | GNU binary with same floor; primary 64-bit SBC asset |
| Linux ARMv7 | `armv7-unknown-linux-gnueabihf` | publish only after build + execution qualification |
| macOS Intel | `x86_64-apple-darwin` | native macOS runner |
| macOS Apple Silicon | `aarch64-apple-darwin` | native ARM macOS runner |
| Windows x86-64 | `x86_64-pc-windows-msvc` | native Windows runner, `.exe` |

For Linux x86-64 and AArch64, start with the same proven approach as Gregg:
release-only Zig + `cargo-zigbuild` targeting an explicit GNU glibc floor,
preferably `2.17` if the actual dependency/toolchain output supports it. Record
the tested floor; do not claim a compatibility floor that was not verified.
Build tooling belongs only in the workflow and must not become an Eggsact
runtime dependency.

For ARMv7, prefer a bounded `cross`/QEMU qualification rather than forcing
`cargo-zigbuild` through ARMv7-specific target quirks. The ARMv7 job must at
least execute `--version` and `--help` and complete the MCP stdio smoke in a
compatible emulator/native environment before the asset is uploaded. If GNU
runtime compatibility remains unclear, leave ARMv7 source-only and record the
reason; evaluate a musl ARMv7 artifact only as a separately justified follow-up,
not as an incidental matrix expansion.

Add focused pure mapping tests in Rust for self-update target selection. The
shell/PowerShell mappings must be checked against the same documented table in
the release assembly job so drift cannot silently publish assets installers do
not know how to request.

## P1 — release-only GitHub Actions pipeline

Add `.github/workflows/release-binaries.yml`. Ordinary `ci.yml` remains focused
on source correctness and read-only permissions.

Trigger policy:

```yaml
on:
  push:
    tags:
      - 'v*'
  workflow_dispatch:
```

Manual dispatch must require an explicit existing tag/ref. It must never create
or move a tag.

Only the release workflow gets `contents: write`. Preserve Eggsact's existing
supply-chain discipline: pin third-party Actions to reviewed commit SHAs rather
than copying unpinned references from Gregg. Prefer the already-installed
GitHub CLI (`gh release view/create/upload`) for release assembly instead of
adding a release-action dependency solely as a wrapper.

### Mandatory preflight

Before builds:

1. read the package version from `Cargo.toml`;
2. require the release tag to be exactly `v${CARGO_PKG_VERSION}`;
3. require the checked-out commit to be exactly the tag target;
4. require a clean checkout;
5. verify the exact Eggsact version is already visible on crates.io;
6. fail clearly and rerun-safely if crates.io indexing has not caught up.

This preserves the existing manual release authority:

```text
maintainer release-check
        -> cargo publish --locked
        -> verify crates.io version visible
        -> create/push annotated vX.Y.Z tag
        -> GitHub release-binaries workflow
        -> draft GitHub Release with binaries/installers
        -> maintainer reviews/publishes draft
```

The workflow must never call `cargo publish`, alter `Cargo.toml`, create a tag,
or push source commits.

### Per-target verification

Each staged executable must run before hashing/upload:

```text
<binary> --version
<binary> --help
```

`--version` must report exactly `eggsact X.Y.Z` for the release version.

Also perform a real stdio MCP smoke against the staged executable rather than
only trusting source tests. A small cross-platform script is acceptable and
should:

1. spawn the exact staged binary with `--mcp`;
2. send `initialize` and validate a successful response/server identity;
3. send `notifications/initialized`;
4. send `tools/list` and confirm a nonempty tool list containing a stable core
   tool such as `math_eval`;
5. close stdin and require bounded clean process shutdown;
6. enforce a timeout so a release job cannot hang indefinitely.

Use the staged artifact path, not `cargo run`, so the smoke proves the bytes
that will be uploaded are executable.

After candidate execution succeeds, generate `<asset>.sha256`. Do not strip,
compress, mutate, or re-sign the executable after hashing. Do not introduce
UPX.

### Release assembly

After all required target jobs pass:

- download the workflow artifacts into one assembly job;
- require exactly one executable and one checksum for every mandatory target;
- include ARMv7 only when its qualification job is enabled/passing;
- syntax-check `install.sh` and perform a PowerShell parser/smoke check on the
  Windows path;
- if no GitHub Release exists for the tag, create a **draft** release;
- if a draft exists on rerun, upload idempotently with clobber semantics;
- if the release is already published, do not silently replace public binary
  assets; fail and require explicit maintainer action/new patch release;
- attach binaries, `.sha256` files, `install.sh`, and `install.ps1`;
- do not publish the draft automatically.

## P2 — bootstrap installers

### Unix installer

Add canonical `packaging/install.sh`, intentionally Bash rather than pretending
to be POSIX `sh` if Bash features are used. If a user pipes it to `sh`, fail
with a concise instruction showing the correct `bash` invocation.

Host mapping:

```text
Linux x86_64/amd64  -> x86_64-unknown-linux-gnu
Linux aarch64/arm64 -> aarch64-unknown-linux-gnu
Linux armv7l        -> armv7-unknown-linux-gnueabihf
Darwin x86_64       -> x86_64-apple-darwin
Darwin arm64        -> aarch64-apple-darwin
unknown             -> no guessed binary; Cargo fallback path
```

Default latest URLs:

```text
https://github.com/eggstack/eggsact/releases/latest/download/eggsact-<target>[.exe]
https://github.com/eggstack/eggsact/releases/latest/download/eggsact-<target>[.exe].sha256
```

Support a small `--version X.Y.Z` option for deterministic/pinned fleet
installation. The pinned URL must use the exact `vX.Y.Z` GitHub Release.

Download contract:

1. require `curl` and fixed HTTPS URLs under `eggstack/eggsact`;
2. download to a newly created temporary directory;
3. distinguish HTTP 404 from transport/server failures;
4. download the matching SHA-256 sidecar;
5. verify with `sha256sum` or `shasum -a 256`;
6. mark the candidate executable;
7. execute the candidate with `--version`;
8. require the output to identify Eggsact and, for pinned installs, the exact
   requested version;
9. only then install it;
10. clean temporary files on success/failure.

A checksum/version failure is a hard integrity error. Do **not** hide it by
compiling from source.

Destination policy:

```text
root invocation     -> /usr/local/bin/eggsact
ordinary user       -> $HOME/.local/bin/eggsact
```

Never silently invoke `sudo`. If the user-local destination is absent from
`PATH`, print the required PATH advice but do not edit `.bashrc`, `.zshrc`, or
other shell files.

Cargo fallback is allowed only when:

- the detected host is intentionally not in the binary matrix; or
- the exact expected asset returns HTTP 404.

Then check for Cargo and use `cargo install eggsact --locked` (or exact
`--version "=X.Y.Z"` when pinned) with a deterministic root matching the chosen
user/system destination. If Cargo is absent, print detected OS/architecture,
the unavailable asset target, and the manual Rust/source option.

Timeouts, 5xx responses, DNS/TLS failure, missing checksum, checksum mismatch,
or wrong candidate version are hard failures, not Cargo-fallback signals.

### Windows installer

Add `packaging/install.ps1` with equivalent semantics using Windows-native
facilities:

- detect x86-64 safely;
- `Invoke-WebRequest` for binary and checksum;
- `Get-FileHash -Algorithm SHA256`;
- candidate `--version` execution before install;
- Administrator install to a stable system location such as
  `%ProgramFiles%\Eggsact\eggsact.exe`;
- ordinary-user install to a stable user location such as
  `%LOCALAPPDATA%\Eggsact\eggsact.exe`;
- print PATH completion instructions if needed rather than silently changing
  persistent user/system PATH in the first implementation;
- Cargo fallback only for unsupported/missing assets;
- no MSI/package-manager layer.

Document the PowerShell copy/paste form and a file-download/inspect-first form
for operators who do not want to pipe network content directly into a shell.
SHA-256 sidecars provide corruption/integrity detection but are fetched from
the same release trust domain as the binary; documentation must not overstate
them as protection from a compromised GitHub release account.

## P3 — `eggsact update`

Add a reserved top-level `update` command while retaining all existing flags and
calculator-expression behavior. Do not adopt Clap solely for this line.

The update authority chain is:

```text
crates.io max stable version
        -> exact GitHub vX.Y.Z release asset
        -> matching SHA-256 sidecar
        -> candidate --version check
        -> replace current executable
        -> Cargo exact-version fallback only if asset is absent
```

### Version lookup

Use crates.io's crate metadata (`max_stable_version`) as the stable version
authority, matching the manual crates-first release process. Reject prerelease
or malformed versions. A small `major.minor.patch` comparator is sufficient;
do not add a semver dependency unless real requirements exceed stable triplets.

Prefer invoking `curl` rather than adding a general async HTTP client/runtime
solely for updater traffic. Bound response sizes and network timeouts. Set a
useful Eggsact User-Agent.

If the current version is equal to or newer than the latest stable version,
report it and exit successfully without touching the executable.

### Candidate staging and verification

Map `std::env::consts::{OS, ARCH}` through the same target contract as P0.
Construct only fixed `https://github.com/eggstack/eggsact/releases/...` URLs.

For a supported target:

1. download the exact release asset and sidecar into a private temporary
   staging location;
2. classify asset HTTP 404 as Cargo-fallback eligible;
3. classify all other download failures as hard errors;
4. parse and validate the 64-hex SHA-256 token;
5. compute SHA-256 with the already-present `sha2` dependency;
6. execute candidate `--version` with a bounded timeout;
7. require exact `eggsact X.Y.Z` identity/version;
8. check that the current executable location is writable;
9. replace only after all validation passes.

Use race-resistant temporary creation. Prefer a small stdlib implementation
(`create_dir`/`create_new`, restrictive Unix permissions, cleanup guard) if it
remains clear and correct; add `tempfile` only if the stdlib implementation
would be less safe or materially more code.

The expected new updater dependency is at most a small self-replacement helper
such as the one already proven in Gregg. Measure `Cargo.lock` and stripped
binary deltas before accepting it. Do not add `reqwest`, another Tokio runtime,
or a release-management framework for this feature.

### Cargo fallback

If no supported binary exists or the exact asset is genuinely 404:

```text
cargo install eggsact --locked --version =X.Y.Z --root <private staging root>
```

Validate the staged Cargo-built executable with the same exact `--version`
contract, then replace the current executable. Do not let Cargo install directly
over the running binary before validation.

### Replacement and active clients

On Unix, replacement should be atomic where practical. On Windows, use a
replacement mechanism that accounts for running-image semantics. If another
active MCP host prevents replacement, fail clearly and instruct the operator to
close/reconnect the affected MCP client and retry; do not kill unrelated
processes.

Permission errors must print the exact elevated retry command rather than
invoking privilege escalation internally, e.g.:

```text
sudo /usr/local/bin/eggsact update
```

After a successful update, print that new MCP launches will use the new version
and already-running stdio sessions may continue using the prior image until the
owning client reconnects. There is intentionally no `restart` command in this
architecture.

### Update tests

Keep most updater behavior factored into deterministic helpers so tests do not
need the public network:

- stable-version parsing/comparison;
- host -> target mapping;
- asset naming/exact URL generation;
- checksum-file parsing;
- checksum mismatch behavior;
- 404 versus transport/server-error classification;
- candidate version parser;
- unsupported-target Cargo fallback selection;
- permission-error message construction.

Use a small injectable command/download seam only where needed for tests; avoid
a generalized process-runner abstraction across the crate.

## P4 — MCP client integration helpers

Add a small top-level integration surface oriented around client-owned stdio:

```text
eggsact integrate list
eggsact integrate detect
eggsact integrate zed
eggsact integrate codex
eggsact integrate claude
eggsact integrate cursor
eggsact integrate vscode
eggsact integrate opencode
```

The default operation is **render/instruct**, not destructive config editing.
Each client adapter should produce the exact current registration command or
config snippet needed to launch:

```text
<resolved-eggsact-path> --mcp
```

Prefer `std::env::current_exe()`/a canonical stable installed path over a bare
`eggsact` command in generated desktop-IDE configuration. GUI applications on
macOS/Windows may not inherit the user's interactive-shell PATH; an absolute
path also remains stable across in-place `eggsact update`.

`integrate detect` should perform only bounded PATH/known-command detection. Do
not recursively scan the filesystem, read unrelated user data, or infer tools
from browser/app history.

### Installation behavior

A later/explicit `--install` flag is allowed when the target client exposes a
stable native registration CLI that can be verified at implementation time.
Examples include Codex/Claude-style `mcp add` commands. In those cases:

1. verify the client executable is present;
2. show the command being executed;
3. use the client's own supported config mutation path;
4. propagate nonzero exit/error output;
5. never silently overwrite a conflicting `eggsact` registration.

For clients that require direct JSON/JSONC/TOML settings edits, the first
implementation should print the file location/snippet rather than building a
multi-format configuration editor inside Eggsact. Only add safe direct editing
when a stable, narrowly parseable config contract justifies it.

Before landing each adapter, re-check the client's current official MCP setup
because editor/agent CLIs change faster than Eggsact. The initial documented
set is:

- Zed;
- OpenAI Codex;
- Claude Code;
- Cursor;
- VS Code / GitHub Copilot agent mode;
- OpenCode.

Do not build a Zed-specific extension merely for Eggsact. Zed can launch a
custom local stdio MCP command directly, and broader MCP packaging/registry
mechanisms are a cleaner future distribution surface.

### Integration tests

Keep client adapters primarily pure rendering logic. Golden/structural tests
should assert:

- server name is `eggsact`;
- command path is the supplied/resolved binary path;
- args contain exactly `--mcp` unless a client format requires equivalent
  representation;
- JSON output/snippets parse where the client format is strict JSON;
- quoting handles spaces in Windows/macOS install paths;
- native `--install` execution is never attempted when the client executable
  is absent.

Do not make ordinary CI install six external IDEs merely to test string
rendering. A small optional/manual smoke against locally available client CLIs
is enough for native registration commands.

## P5 — optional MCP Bundle / official Registry follow-up

After P0-P4 are working in at least one real release, evaluate MCP Bundle
(`.mcpb`) packaging and official MCP Registry publication as a **separate,
non-blocking** distribution enhancement.

The value is one metadata/package format that can eventually be consumed by
multiple MCP hosts rather than maintaining editor-specific extensions. The
Registry does not make crates.io itself a local executable package channel, so
GitHub-hosted compiled bundle artifacts may be the appropriate bridge.

Do not make the first prebuilt-binary release depend on Registry preview
availability or client adoption. Do not add MCPB if it merely duplicates the
working raw binary + client registration path without reducing operator work.

## P6 — documentation, verification, and release closure

### User documentation

Update README installation order so the fastest path appears first:

```text
1. copy/paste binary installer
2. direct GitHub Release assets / pinned install
3. cargo install eggsact
4. source checkout
```

Add `docs/installation.md` if needed rather than overloading README. Document:

- exact supported binary matrix;
- that AArch64 covers normal 64-bit Raspberry Pi/Le Potato Linux images;
- whether ARMv7 qualified or is Cargo fallback only;
- user-local versus system install locations;
- checksum/candidate validation behavior;
- pinned-version installation;
- `eggsact update` authority/fallback semantics;
- active MCP client behavior after update;
- macOS unsigned/notarized status truthfully;
- Windows PATH/install location behavior;
- copy/paste examples for client integration;
- why systemd/cron/launchd/SCM are intentionally absent for stdio mode.

Update `docs/release.md` so release ordering is unambiguous: crates.io manual
publish first, then tag, then draft GitHub binary release, then maintainer final
publication. Existing statements that CI does not publish crates or create tags
remain true.

Update `architecture/cli-binaries.md` and
`architecture/coding-agent-integration.md` with the implementation facts after
the code lands. `AGENTS.md` should describe actual paths/commands only after
those files exist; do not pre-document unimplemented behavior as current fact.

### Verification gates

Normal source gate remains:

```bash
cargo fmt --all -- --check
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
```

Add the cheapest appropriate release-specific checks:

```text
bash -n packaging/install.sh
shellcheck packaging/install.sh               where available
PowerShell parser/smoke on windows-latest
release target/asset contract validation
staged binary --version / --help
staged binary MCP initialize + tools/list smoke
installer bad-checksum hard-failure test
installer missing-asset -> Cargo-fallback selection test
update helper/unit tests
integration rendering tests
```

The release workflow should not rerun the entire ordinary correctness matrix.
It proves artifact construction and executable identity; normal CI proves source
correctness.

For Linux AArch64, prefer an actual GitHub-hosted ARM runner for final native
smoke when available. Before calling the SBC objective closed, record at least
one truthful AArch64 runtime result on a Pi/Le Potato-class Debian/Ubuntu/
Armbian environment if hardware is available; absence of local hardware should
be documented rather than replaced with a false claim. ARMv7 publication
requires its own executable/QEMU/native qualification evidence.

### Dependency/binary-size closure

Before merging the final implementation:

1. record new normal dependencies and explain each one;
2. compare stripped release binary size to the current reference;
3. ensure no HTTP client, CLI framework, release framework, or service-manager
   dependency entered accidentally;
4. ensure MCP/tool behavior and tool-count/profile contracts did not change as
   a side effect of deployment work.

## Acceptance criteria

### Release artifacts

- [ ] Dedicated release-only workflow exists and ordinary CI remains read-only.
- [ ] Tag/workspace version and exact tagged commit are verified before build.
- [ ] Exact Eggsact version must already be visible on crates.io.
- [ ] Linux x86-64 and AArch64 use a documented intentional compatibility floor.
- [ ] macOS Intel/ARM64 and Windows x86-64 binaries are produced.
- [ ] ARMv7 is published only after its qualification gate passes; otherwise the
      installer recognizes it and selects Cargo fallback.
- [ ] Every uploaded executable runs exact `--version`/`--help` and MCP stdio
      smoke before checksum generation/upload.
- [ ] Every executable has a matching valid SHA-256 sidecar.
- [ ] Asset names exactly match the contract in P0.
- [ ] Workflow creates/updates only a draft release and never publishes crates,
      creates tags, or pushes source commits.

### Installers

- [ ] Unix copy/paste installer selects the correct binary on every supported
      Linux/macOS mapping.
- [ ] Windows PowerShell installer installs the x86-64 binary with equivalent
      checksum/version validation.
- [ ] User-local and privileged destinations are deterministic/documented.
- [ ] No installer silently invokes `sudo` or edits shell rc/PATH persistently.
- [ ] HTTP 404/unsupported target can use Cargo fallback when Cargo exists.
- [ ] Network/5xx/TLS/checksum/version failures remain hard failures.
- [ ] Pinned `--version X.Y.Z` installation is supported and exact.

### Self-update

- [ ] `eggsact update` reads latest stable version from crates.io.
- [ ] It downloads the exact matching GitHub Release asset for the host.
- [ ] It verifies SHA-256 and exact candidate `--version` before replacement.
- [ ] Cargo exact-version fallback occurs only for unsupported/404 asset cases.
- [ ] Permission failures print an exact retry command; no internal elevation.
- [ ] Windows replacement failure due to active clients is actionable/non-destructive.
- [ ] Successful update does not kill/restart existing MCP sessions.
- [ ] No background updater is introduced.

### Agent/IDE integration

- [ ] `integrate list/detect` and initial client renderers exist.
- [ ] Zed, Codex, Claude Code, Cursor, VS Code, and OpenCode have current,
      documented stdio registration instructions.
- [ ] Generated desktop-client config prefers a resolved stable Eggsact path.
- [ ] Native client config mutation occurs only under explicit `--install` and
      only through a currently supported client CLI.
- [ ] Eggsact does not become a generic JSONC/TOML IDE-config editor.
- [ ] No systemd/cron/launchd/SCM lifecycle is added for stdio MCP.

### Scope/quality

- [ ] No apt/brew/winget/etc. package pipeline is added.
- [ ] No board-specific binary is needed for Pi/Le Potato-class AArch64 systems.
- [ ] No production HTTP transport is added.
- [ ] No unnecessary Clap/reqwest/cargo-dist-style framework is introduced.
- [ ] New dependency and stripped-binary-size deltas are recorded and justified.
- [ ] Existing 86-tool behavior/profile/order contracts remain unchanged unless
      independently changed by another explicitly scoped line.

## Handoff order

Implement in this order to minimize rework:

1. **P0 + P1:** asset contract and release workflow, including staged MCP smoke.
2. **P2:** bootstrap installers against that frozen asset contract.
3. **P3:** self-update reusing the exact same target/asset/version rules.
4. **P4:** MCP client render/native-registration helpers.
5. **P6:** documentation, verification, dependency/size review, closure.
6. **P5:** only after the first raw-binary release proves the deployment path.

Do not start with client config editing or updater abstractions before the
release asset contract is fixed; those surfaces consume that contract.

## Closure record required

When this line ships, replace this implementation detail with a concise shipped
record containing:

1. implementation commit SHA(s);
2. first GitHub release/tag produced by the pipeline;
3. final target matrix and Linux glibc floor/build mechanism;
4. ARMv7 qualification result;
5. final asset names and installer destinations;
6. `eggsact update` dependencies and stripped binary-size delta;
7. exact clients supported by `integrate` and which support native `--install`;
8. release workflow run proving staged binary/MCP smokes;
9. real/native SBC evidence available at closure;
10. any unsigned macOS/Windows operator caveats;
11. confirmation that no stdio daemon/service-manager layer was introduced.

---

# Completed corrective closure: network + initial cron syntax fix

The prior corrective line remains valid except for its conclusion that only a
bare `*` should carry wildcard/star semantics in DOM/DOW matching.

Completed and retained:

- IPv6 CIDR counts are exact and prefix-only.
- IPv4-mapped IPv6 detection is limited to `::ffff:0:0/96`.
- Explicit full ranges/lists such as DOM `1-31` are not treated as equivalent
  to wildcard syntax.
- Regex safety envelopes map high/medium/low findings to error/warn/info,
  restoring Python-reference parity.
- Regression/property coverage, docs, changelog, AGENTS tree layout, generated
  docs, cargo-deny, packaging, publish dry-run, release-check, and cron fuzz
  smoke all passed.
- The release binary measured 8,229,484 bytes versus the 8,229,476-byte
  post-expansion reference (+8 bytes), with no locked normal dependency delta.

The final cron follow-up below supersedes only the prior `*/n is restricted`
rule and any documentation/tests encoding that rule.

---

# Completed corrective closure: Vixie/Cronie star-step DOM/DOW semantics (C6)

Closed. The prior conclusion that only a bare `*` carries star semantics is
superseded; `*/n` step forms carry Vixie/Cronie star syntax while still
constraining via their parsed value sets.

Retained:

- `CronField` carries `star_syntax: input.starts_with("*")` instead of
  `unrestricted`; the flag is parser/runtime state only, with no wire-shape
  change.
- `day_matches()` uses DOM AND DOW when either field has star syntax,
  otherwise DOM OR DOW.
- `*/1` and `*/2` on either DOM or DOW follow the star-flag/AND path;
  explicit full ranges/lists such as `1-31` remain non-star (OR).
- Direct unit tests cover bare-star, explicit-range, `*/1`, and nontrivial
  star steps on both DOM and DOW, plus Sunday `0`/`7` normalization; the
  independent property test encodes the AND/OR reference rule from case
  metadata and includes both `*/1` and `*/2`.
- Docs corrected everywhere the old rule was stated (`AGENTS.md`,
  `README.md`, `architecture/tools.md`, `docs/mcp-tools.md`, the
  mcp-tools skill, `CHANGELOG.md` `[Unreleased]`); step syntax is described
  as the Vixie/Cronie extension, not POSIX-defined.
- Invariants held: 86 tools / 23 categories, original 80-tool prefix,
  full-profile-only utilities, no dependency delta, no response-schema change.
- Gates passed: fmt, clippy, lib/bins/non-parity tests, doctests,
  generate-docs check, cargo-deny, packaging, and publish dry-run. The cron
  fuzz smoke was skipped (requires nightly cargo-fuzz; stable only locally).
- Release binary delta negligible; dependency graph unchanged.

---

# Parallel future track: MCP 2026-07-28 compatibility

This remains separate from the utility corrective work and the deployment line
above. Do not mix protocol modernization into binary distribution/self-update
unless a concrete compatibility dependency is discovered.

## P0 — Conformance/dependency gate

Before advertising MCP `2026-07-28`:

1. Inventory normative requirements relevant to a stdio tools-only server,
   including stateless per-request metadata, `server/discover`, version errors,
   modern result/cache fields, schema dialect requirements, cancellation, and
   deterministic tool listing.
2. Resolve the specification's JSON Schema 2020-12 requirement. Eggsact's
   current runtime validator intentionally implements only a bounded subset.
3. Evaluate a standards-complete validator only with network/file resolution
   disabled and measure dependency/binary cost. If it is too expensive, keep
   the older advertised protocol support honest rather than writing a custom
   full JSON Schema implementation.
4. Exercise the official MCP conformance scenarios applicable to a stdio
   tools server without adding production HTTP solely for testing.

## P1 — Dual-era protocol implementation

Only after P0 passes:

- add explicit modern request metadata/context types;
- keep legacy initialize/session behavior for existing protocol revisions;
- route modern requests statelessly without reading prior client metadata;
- implement `server/discover` and required unsupported-version responses;
- add modern result/cache fields without changing legacy response shapes;
- preserve server-configured profile/audience behavior;
- do not adopt the Rust MCP SDK unless it demonstrably reduces code and
  maintenance burden;
- do not add Tasks, prompts, resources, sampling, elicitation, HTTP transport,
  or other unrelated MCP capabilities.

## P2 — Protocol closure

Advertise `2026-07-28` as preferred only after conformance passes. Update MCP
architecture/user docs, preserve supported legacy revisions, and add stable
conformance checks at the cheapest appropriate verification tier.

---

## Open opportunities after corrective closure

These remain candidates, not commitments:

1. Deepen actual codegg use of existing typed preflight wrappers before adding
   more MCP-visible tools.
2. Measure high-frequency tool latency only if profiling shows a real need.
3. Revisit schema-detail defaults as model context economics change.
4. Consider YAML only when a concrete Codegg workflow justifies its dependency
   and semantic surface.

## Standing non-goals

- Not a general sandbox: classify risk, do not enforce it.
- Not every utility belongs in MCP: admit specification-heavy exact operations,
  not generic DevUtils feature parity.
- No external services or hidden host-state dependencies.
- No feature growth during corrective/closure passes.
