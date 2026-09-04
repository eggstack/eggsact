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

Latest published version: **1.2.3**. The deterministic utility and cron
corrective lines are closed. The original 80-tool registration order remains an
exact prefix, with the six later utilities in the full profile only.

The binary distribution/self-update/MCP bootstrap implementation landed in
`75bf52f`, with closure documentation in `216a68f`. The corrective
implementation below is now on `main`, but the line remains open until the
first real binary-bearing release supplies the cross-platform evidence that
source CI cannot provide. Do not publish installer URLs or treat v1.2.3 as a
binary release.

---

# Active corrective line: binary release and updater deployment closure

Status: **implementation complete; release execution evidence pending**.

Implementation completed in this pass:

- Linux AArch64 builds and executable smokes run on `ubuntu-24.04-arm`; the
  workflow checks compatible runner architecture before every staged smoke.
- Release-only Zig 0.14.1 archives are downloaded by architecture and checked
  against pinned SHA-256 values; cargo-zigbuild 0.23.3 is installed explicitly.
- Unix and Windows installer contracts now distinguish 404 fallback from hard
  failures, reject `sh install.sh` before Bash-only options, support Windows
  unsupported-architecture Cargo fallback, and use semicolon-aware PATH checks.
- Windows self-update reports `update staged` and leaves a bounded helper
  failure status file rather than reporting deferred replacement as complete.
- README and installation/release/verification/architecture guidance now keep
  Cargo as the live path until a new binary-bearing tag is published; v1.2.3 is
  not retrofitted.

Local source and contract gates passed on the corrective commit. The remaining
P4 evidence is maintainer-only: publish a new semver crate, push its annotated
tag, run the tagged workflow, inspect the draft assets, and exercise installers
and Windows replacement where runners/environments permit.

## Objective

Correct the real release/deployment path without expanding Eggsact's scope.
Preserve the existing implementation shape — one binary, client-owned stdio MCP,
manual crates.io publication, a tag-triggered draft GitHub Release, binary-first
installers, and a narrow self-update command — while fixing the defects that
would prevent or misreport a real cross-platform release.

This is a corrective pass, not a redesign. Prefer small changes to the existing
workflow/scripts/updater over new abstractions, dependencies, or packaging
systems.

## Confirmed defects to close

### C1 — Linux AArch64 release smoke is not executable on the configured runner

`.github/workflows/release-binaries.yml` currently builds
`aarch64-unknown-linux-gnu.2.17` on `ubuntu-latest` and then directly executes
the staged binary for `--version`, `--help`, and the MCP stdio smoke. The
x86-64 hosted runner cannot execute the AArch64 artifact without emulation.

Required correction:

- keep the glibc-floor build reproducible;
- execute the final AArch64 release bytes on a native ARM64 GitHub-hosted
  runner when available, preferably `ubuntu-24.04-arm`;
- if the build must remain cross-produced on x86-64, transfer the exact staged
  bytes to the ARM runner before smoke/checksum publication, or use explicitly
  configured QEMU/binfmt with an equally truthful executable smoke;
- do not mark AArch64 qualified based only on cross-compilation;
- do not weaken the staged `--version`, `--help`, initialize/initialized/
  `tools/list`, and stdin-EOF shutdown smoke.

The preferred minimal design is to let the native ARM64 runner perform both the
AArch64 `cargo zigbuild` and executable smoke if the runner/toolchain supports
that cleanly. Avoid a multi-stage evidence framework unless a split build/smoke
is actually required.

### C2 — release workflow assumes `apt install zig`

The Linux build jobs currently install Zig through the Ubuntu runner package
manager. That package is not a stable/reliable contract for the hosted Ubuntu
version and can make the release workflow fail before compilation.

Required correction:

- install a pinned Zig release explicitly in the release job, following the
  proven Gregg approach or an equivalently small reviewed mechanism;
- verify the downloaded Zig artifact by a pinned SHA-256 (or use a pinned
  action only if it is clearly smaller and satisfies Eggsact's action-pinning
  policy);
- put Zig/cargo-zigbuild only in release tooling, never runtime Cargo deps;
- keep the intended Linux GNU glibc floor at 2.17 unless actual compiler output
  proves a different minimum is required;
- record the exact Zig and cargo-zigbuild versions used by the release workflow
  so a future runner image change cannot silently change the release toolchain.

Do not introduce `cargo-dist`, containers, Nix, or another generalized release
framework to solve this bounded tooling problem.

### C3 — documentation advertises installer assets before any such release exists

`README.md` and `docs/installation.md` currently present
`releases/latest/download/install.sh` as the primary installation path and use
v1.2.3 as a pinned installer example, but the published v1.2.3 GitHub Release
contains no installer/binary assets.

Required correction:

- do not mutate or retrofit the already-published v1.2.3 release with current
  main artifacts;
- the first binary-bearing release must use a new semver version/tag after the
  source line is ready;
- until that release exists, documentation must not present the binary
  installer as an already-working current path;
- either gate the README fast-path wording until the first successful binary
  release lands, or phrase it explicitly as available starting with the next
  binary-bearing release;
- remove the invalid `v1.2.3/install.sh` pinned example and use a placeholder
  `vX.Y.Z` or the actual first binary release version only after it exists;
- after the first successful release, verify both `latest/download/install.sh`
  and the exact-tag installer URL before declaring the path live.

The release version bump/publication itself remains a maintainer release action,
not something CI performs automatically.

### C4 — Windows updater reports success before deferred replacement is proven

The current Windows updater copies a candidate beside the running executable,
spawns detached PowerShell to wait for the updater process to exit, and returns
success immediately. The actual `Move-Item -Force` can subsequently fail, while
the user has already been told the update succeeded.

Required correction:

- use a replacement mechanism whose success semantics are truthful;
- prefer the proven Gregg/self-replacement approach if it can be reused with a
  small dependency/binary-size cost;
- otherwise design a bounded Windows replacement helper protocol that cannot
  print final success until the replacement contract is guaranteed, and that
  leaves an actionable failure marker/message if post-exit replacement fails;
- never kill arbitrary Eggsact/MCP processes;
- permission/lock failures must tell the user to close/reconnect the relevant
  client and retry from an Administrator PowerShell when needed;
- preserve Unix atomic-adjacent rename behavior unless a concrete bug is found;
- keep candidate checksum and exact `--version` validation before replacement.

If a new replacement crate is added, measure the locked dependency and stripped
binary delta and record it in closure evidence. Do not add a network client or
updater framework as collateral.

### C5 — Unix Bash guard runs too late

`packaging/install.sh` currently executes `set -euo pipefail` before checking
`BASH_VERSION`. A user invoking it through `sh` can fail on unsupported
`pipefail` syntax before seeing the intended instruction.

Required correction:

- make the interpreter guard execute before any Bash-only option or syntax;
- `curl ... | bash` remains the documented path;
- accidental `sh install.sh` should fail with one concise message explaining
  that Bash is required;
- keep the rest of the script Bash-specific rather than attempting an
  unnecessary POSIX-shell rewrite.

### C6 — Windows installer does not follow unsupported-target Cargo fallback

`packaging/install.ps1` currently rejects non-X64 Windows before checking Cargo,
while the documented contract says unsupported targets may use Cargo fallback.

Required correction:

- map Windows x86-64 to the prebuilt MSVC asset;
- on Windows ARM64/other unsupported architectures, skip binary download and
  enter the same staged Cargo fallback path when Cargo is available;
- if Cargo is unavailable, report the detected architecture and that no
  prebuilt asset exists;
- preserve hard failure for checksum, version, TLS/network, or other failures
  after a matching binary path has been selected.

Do not add a Windows ARM64 release artifact in this corrective pass unless it is
independently qualified and the plan is amended first.

### C7 — Windows PATH detection uses the wrong delimiter model

The PowerShell installer checks PATH membership using colon-delimited matching,
but Windows PATH is semicolon-separated.

Required correction:

- compare normalized path entries using `[Environment]::GetEnvironmentVariable`
  plus `-split ';'` (or another explicit Windows-native path-list operation);
- handle trailing separators/case-insensitivity reasonably;
- continue to print advice only; do not persistently edit PATH in this line.

## P0 — fix and locally validate the release workflow contract

Modify `.github/workflows/release-binaries.yml` and any tiny release helper
needed to implement C1/C2.

Required matrix after correction:

```text
x86_64-unknown-linux-gnu     build + execute smoke
AArch64-unknown-linux-gnu    build + native/QEMU execute smoke
x86_64-apple-darwin          native build + execute smoke
aarch64-apple-darwin         native build + execute smoke
x86_64-pc-windows-msvc       native build + execute smoke
```

ARMv7 remains recognized/source-only until its separate qualification gate
passes. Do not make this corrective line contingent on ARMv7.

Preserve:

- exact tag/Cargo version equality;
- exact tag commit checkout;
- crates.io stable-version visibility before binary assembly;
- pinned third-party Actions;
- release-only `contents: write`;
- draft-only release creation/update;
- refusal to replace assets on an already-published release;
- stable raw executable asset names and SHA-256 sidecars;
- exact staged-binary MCP smoke before publication.

Add a workflow-level sanity check ensuring every build job that calls an
executable smoke is running on a compatible architecture or explicitly has an
emulation layer configured. A comment alone is not sufficient evidence.

## P1 — correct installer contracts

Fix C5-C7 in `packaging/install.sh` and `packaging/install.ps1` without changing
the public command surface.

Add/retain cheap deterministic checks for:

- Bash-required invocation failure before Bash-only syntax;
- Linux/macOS target mapping;
- ARMv7 -> Cargo fallback selection;
- Windows X64 -> binary path;
- Windows ARM64 -> Cargo fallback selection;
- binary 404 -> Cargo fallback;
- binary transport/5xx -> hard failure;
- checksum mismatch -> hard failure;
- candidate-version mismatch -> hard failure;
- pinned exact version propagation;
- Windows PATH membership detection using semicolon-separated entries.

Avoid introducing a shell-testing framework. Small helper seams/scripts and
runner smoke commands are sufficient.

## P2 — correct Windows self-update completion semantics

Refactor only the Windows replacement portion of `src/update.rs` (and dependency
files if justified).

Required behavior:

1. obtain crates.io latest stable version;
2. fetch or Cargo-build the exact candidate;
3. verify checksum where applicable;
4. verify exact candidate `eggsact X.Y.Z`;
5. establish a replacement path that is safe for the running Windows image;
6. report success only under a truthful replacement contract;
7. leave no silent deferred failure state;
8. never restart/kill client-owned MCP processes.

Add focused tests for replacement command construction/state reporting where
possible without requiring destructive replacement of the test runner binary.
Use an integration/manual Windows smoke for the actual self-update behavior.

If Windows cannot support a fully synchronous in-process success confirmation,
the CLI may report a distinct state such as `update staged; replacement will
complete after exit` rather than `updated` — but only if the deferred helper has
a reliable failure-report/recovery mechanism and docs describe it accurately.
Prefer a proven self-replacement primitive instead of inventing a complex
protocol.

## P3 — make docs truthful before and after the first binary release

Update `README.md`, `docs/installation.md`, `docs/release.md`,
`docs/verification.md`, `AGENTS.md`, and `plans/roadmap.md` as needed.

Before the first successful binary-bearing release:

- Cargo/source installation remains the guaranteed live path;
- binary installer text is clearly marked as landing with the next release or
  otherwise not presented as a currently working `latest` URL;
- no v1.2.3 installer example remains.

After the first successful binary-bearing release:

- switch README to the binary-first copy/paste path;
- use a real published version in pinned examples;
- document the actually validated matrix and glibc floor;
- record whether AArch64 was native-smoked and on which runner/host;
- leave ARMv7 documented as source-only unless it independently qualifies;
- keep macOS/Windows unsigned/notarized status truthful.

Do not make the documentation depend on an unpublished future tag while calling
the path available today.

## P4 — release-path execution gate

This corrective line cannot close based only on ordinary `ci.yml`, YAML review,
or local x86-64 testing.

Before closure, obtain evidence from the actual release workflow. Preferred
path:

1. prepare the next real semver release using the existing manual release gate;
2. manually publish the crate to crates.io;
3. verify crates.io exposes the exact stable version;
4. create/push the annotated `vX.Y.Z` tag;
5. let `release-binaries.yml` execute the full matrix;
6. require all target build/smoke jobs and installer checks to pass;
7. inspect the draft release asset set before publication;
8. install at least one Linux/macOS artifact through `install.sh` and one
   Windows artifact through `install.ps1` where runner/environment access
   permits;
9. verify `releases/latest/download/install.sh` only after the draft is
   intentionally published;
10. record the workflow run ID and release tag in this roadmap.

If using a disposable/manual-dispatch tag/ref for pre-release validation is
possible without corrupting public release history, that may be used first, but
the first real binary-bearing release remains the authoritative closure proof.

## Verification

Run the normal source gates after implementation:

```bash
cargo fmt --all -- --check
cargo run --locked --features dev-tools --bin generate-docs -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
cargo deny check advisories bans licenses sources
cargo package --locked --list
cargo publish --locked --dry-run
```

Release/deployment-specific checks:

```text
python3 scripts/check-release-contract.py
bash -n packaging/install.sh
shellcheck packaging/install.sh                  when available
PowerShell parser + installer mapping smoke
staged x86-64 Linux --version/--help/MCP smoke
staged AArch64 Linux --version/--help/MCP smoke on native ARM64 or explicit QEMU
macOS Intel/ARM64 staged binary smokes
Windows staged binary smoke
installer 404 -> Cargo fallback tests
installer 5xx/checksum/version -> hard-failure tests
Windows unsupported-arch -> Cargo fallback test
Windows PATH detection test
Windows self-update/replacement smoke
```

Ordinary CI should remain small. Put expensive cross-platform executable proof
in the release/manual tier rather than expanding every push/PR run.

## Acceptance criteria

### Release workflow

- [ ] No Linux job relies on `apt install zig`.
- [ ] Zig/cargo-zigbuild versions are intentional and reproducible.
- [ ] Linux x86-64 retains a verified documented glibc floor.
- [ ] Linux AArch64 final release bytes execute successfully on native ARM64 or
      explicitly configured emulation before checksum/upload.
- [ ] Every published target passes `--version`, `--help`, and the real MCP
      initialize/tools-list/EOF smoke using the staged release binary.
- [ ] Release workflow remains crates-first, tag-driven, draft-only, and
      idempotent for drafts.
- [ ] ARMv7 is not published without its separate execution qualification.

### Installers

- [ ] `sh install.sh` fails with the intended Bash-required guidance rather
      than a `pipefail` parser/option error.
- [ ] `curl ... | bash` works.
- [ ] Binary 404 is the only matching-target network condition that selects
      Cargo fallback.
- [ ] Windows unsupported architectures use Cargo fallback when available.
- [ ] Windows PATH detection understands semicolon-separated PATH entries.
- [ ] Checksum/version/transport failures remain hard errors.

### Self-update

- [ ] Windows no longer prints final `updated` success before replacement can
      truthfully be considered successful/staged under the documented contract.
- [ ] Replacement failure is visible/actionable and never silently deferred.
- [ ] Unix update behavior remains atomic-adjacent and validated.
- [ ] No MCP process enumeration, killing, daemonization, or `restart` command
      is introduced.
- [ ] Any new normal dependency has a documented justification and measured
      stripped-binary delta.

### Documentation and closure

- [ ] No current docs claim the v1.2.3 release contains installer/binary assets.
- [ ] No current docs advertise a `latest/download/install.*` path as live
      before such an asset is actually published.
- [ ] The first binary-bearing release uses a new version/tag.
- [ ] The actual release workflow has completed successfully at least once.
- [ ] The draft asset set contains every mandatory binary, checksum, and both
      installer scripts.
- [ ] At least one end-to-end installer path has been exercised against the
      published release; Windows is exercised where available.
- [ ] Closure records the workflow run ID, release tag, target matrix, Zig
      version, glibc floor, AArch64 execution environment, installer smokes,
      Windows update result, and final binary-size/dependency delta.

## Scope control

Do not use this corrective pass to add:

- systemd, launchd, Windows SCM, cron, PID files, or `restart`;
- Streamable HTTP or another MCP transport;
- apt/deb/rpm, Homebrew, winget, Chocolatey, MSI, containers;
- code signing/notarization infrastructure;
- automatic crates.io publishing or tag creation;
- `cargo-dist`, `release-plz`, or a generalized packaging framework;
- editor-specific extensions;
- unrelated MCP protocol modernization;
- new deterministic tools or profile/schema changes.

The correct end state remains a small stdio MCP/utility binary with a reliable,
verified release path — not a deployment platform.

## Closure record

When this corrective line passes, replace its execution detail with a concise
completed record containing:

1. corrective implementation SHA(s);
2. first successful binary-bearing release tag;
3. release workflow run ID;
4. exact five-target published matrix;
5. Zig and cargo-zigbuild versions;
6. Linux glibc floor;
7. AArch64 native/emulated execution evidence;
8. ARMv7 status;
9. Unix/Windows installer smoke results;
10. Windows self-update/replacement result;
11. dependency and stripped-binary delta from the `75bf52f` baseline;
12. ordinary CI/release-gate results.

Do not mark the line complete again until the release workflow itself has run
successfully and produced a verified asset set.

---

## Future opportunities after corrective closure

1. Evaluate MCP Bundle/official MCP Registry distribution after a raw-binary
   release proves the deployment path; keep it non-blocking.
2. Deepen actual coding-agent use of the existing typed preflight wrappers.
3. Measure high-frequency tool latency only if profiling shows a real need.
4. Revisit schema-detail defaults as model context economics change.
5. Consider YAML only when a concrete workflow justifies its dependency and
   semantic surface.

## Standing non-goals

- Not a general sandbox: classify risk, do not enforce it.
- Not every utility belongs in MCP: admit specification-heavy exact operations,
  not generic DevUtils feature parity.
- No external services or hidden host-state dependencies in deterministic tools.
- No systemd/cron/launchd/SCM lifecycle for the current client-owned stdio
  transport, and no production HTTP transport in this line.
