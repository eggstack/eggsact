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
`75bf52f`. The first corrective deployment pass landed in `427a93e` and fixed
the previously identified AArch64 runner, installer, documentation, and Windows
self-update issues. Ordinary CI for `427a93e` passed.

The binary-release line remains open because the release workflow has not yet
successfully executed and one release-only Zig bootstrap defect remains. Do not
publish installer URLs or treat v1.2.3 as a binary-bearing release.

---

# Active corrective line: Zig bootstrap and first binary release qualification

Status: **source correction implemented; binary-release qualification pending**.

## Objective

Fix the final known release-only bootstrap defect with the smallest possible
change, add a cheap deterministic guard against recurrence, then qualify the
actual five-target binary release path before closing this line.

This is not a release-system redesign. Preserve the existing workflow,
installer, updater, client-owned stdio lifecycle, manual crates.io publication,
and draft-only GitHub Release assembly.

## Completed corrective baseline

Commit `427a93e` is the implementation baseline for this pass. It already:

- moves Linux AArch64 build/smoke to `ubuntu-24.04-arm` and verifies runner
  architecture before executing staged binaries;
- pins Zig 0.14.1 archives and architecture-specific SHA-256 values;
- pins cargo-zigbuild 0.23.3 in release-only tooling;
- keeps release assembly as the only job with `contents: write`;
- fixes the Unix Bash guard ordering;
- gives unsupported Windows architectures Cargo fallback;
- fixes Windows semicolon-separated PATH detection;
- makes Windows self-update report `update staged` with bounded deferred
  replacement failure reporting rather than premature final success;
- keeps Cargo/source installation as the live documented path until a real
  binary-bearing release exists.

Do not reopen those areas unless a concrete regression is found.

## C8 — Zig archive extraction path is wrong — implemented

`.github/workflows/release-binaries.yml` downloads one of:

```text
zig-x86_64-linux-0.14.1.tar.xz
zig-aarch64-linux-0.14.1.tar.xz
```

The archive extracts with an architecture-qualified top-level directory. The
workflow previously added `$RUNNER_TEMP/zig-0.14.1` to `GITHUB_PATH` and
executed `$RUNNER_TEMP/zig-0.14.1/zig`, which did not match the extracted
archive layout and could fail both Linux release jobs before `cargo zigbuild`.
The workflow now extracts into a fixed `$RUNNER_TEMP/zig` directory with
`--strip-components=1` and uses that same directory for both PATH setup and
the direct version check.

### Required correction

Prefer one deterministic extraction directory independent of archive naming:

```bash
zig_dir="$RUNNER_TEMP/zig"
mkdir -p "$zig_dir"
tar -xJf "$RUNNER_TEMP/$zig_archive" -C "$zig_dir" --strip-components=1
echo "$zig_dir" >> "$GITHUB_PATH"
"$zig_dir/zig" version
```

An equivalent implementation may derive the actual architecture-qualified
extracted directory from `zig_archive`, but a fixed directory with
`--strip-components=1` is simpler and already proven in sibling repo release
logic.

Preserve all current security/reproducibility properties:

- Zig version remains explicit;
- x86-64 and AArch64 archive hashes remain pinned;
- hash verification happens before extraction;
- cargo-zigbuild version remains explicit;
- Zig/cargo-zigbuild stay release-only and do not enter runtime dependencies;
- Linux GNU targets retain the intended `.2.17` glibc floor;
- the exact staged executable still must pass `--version`, `--help`, and MCP
  initialize/initialized/tools-list/EOF smoke before checksum/upload.

Do not solve this with `apt install zig`, `cargo-dist`, containers, Nix, or a
new release framework.

## P0 — add a cheap Zig-bootstrap contract check

The defect survived ordinary CI because the tag-only release workflow is not
executed on normal pushes. Add a small deterministic check that validates the
release script's extraction contract without downloading/building Zig on every
push.

Acceptable minimal approaches include:

- extend `scripts/check-release-contract.py` to require a fixed `zig_dir`,
  `--strip-components=1`, and use of that same path for `GITHUB_PATH`/`zig
  version`; or
- move the tiny archive-install shell fragment into a reusable release helper
  with a local syntax/contract test, only if doing so is actually simpler.

Prefer extending the existing contract checker. Do not introduce a shell test
framework or turn ordinary CI into a release build.

The check should fail if the workflow again extracts one directory but invokes
Zig from another.

## P1 — verify the corrected source line

After C8 is fixed, run the normal source gates appropriate to the changed files,
including at minimum:

```bash
python3 scripts/check-release-contract.py
bash -n packaging/install.sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity --test-threads=4
```

Also inspect the release workflow to confirm:

- Linux x86-64 uses the x86-64 Zig archive and hash;
- Linux AArch64 uses the AArch64 Zig archive and hash;
- the extracted `zig` executable path exists by construction;
- `cargo zigbuild --target <target>.2.17` remains unchanged;
- architecture checks happen before staged executable smoke;
- only the assembly job has write permission;
- mandatory assets/checksums are unchanged.

Ordinary source CI passing is necessary but still not sufficient for closure.

## P2 — first real binary-release execution gate

Once C8 is merged, stop doing source-only corrective passes unless the release
run exposes another concrete defect. Qualify the actual deployment path.

Required sequence:

1. prepare the next real semver release using the existing manual release gate;
2. publish the crate manually to crates.io;
3. wait until crates.io exposes that exact version as `max_stable_version`;
4. create and push the annotated `vX.Y.Z` tag pointing at the release commit;
5. allow `.github/workflows/release-binaries.yml` to execute;
6. require successful jobs for:
   - Linux x86-64 GNU;
   - Linux AArch64 GNU on native ARM64;
   - macOS Intel;
   - macOS Apple Silicon;
   - Windows x86-64;
   - PowerShell installer parsing;
   - draft release assembly;
7. require each staged executable to pass `--version`, `--help`, and the real
   MCP stdio smoke before its SHA-256 sidecar is generated;
8. inspect the draft release and verify the exact mandatory asset set;
9. test at least one Unix installer path against the release and Windows where
   environment access permits;
10. intentionally publish the draft only after inspection;
11. verify the exact-tag installer URL and then
    `releases/latest/download/install.sh` after publication;
12. update README/docs from Cargo-first to binary-first only after those URLs
    are live.

Do not retrofit v1.2.3. The first binary-bearing GitHub Release must use a new
version/tag.

## Mandatory release assets

The first qualified binary release must contain:

```text
eggsact-x86_64-unknown-linux-gnu
eggsact-x86_64-unknown-linux-gnu.sha256
eggsact-aarch64-unknown-linux-gnu
eggsact-aarch64-unknown-linux-gnu.sha256
eggsact-x86_64-apple-darwin
eggsact-x86_64-apple-darwin.sha256
eggsact-aarch64-apple-darwin
eggsact-aarch64-apple-darwin.sha256
eggsact-x86_64-pc-windows-msvc.exe
eggsact-x86_64-pc-windows-msvc.exe.sha256
install.sh
install.ps1
```

ARMv7 remains recognized by the Unix installer but Cargo fallback/source-only
until it receives separate executable qualification. Do not make ARMv7 a
blocker for this closure.

## Acceptance criteria

### Zig bootstrap

- [x] The downloaded Zig archive is verified before extraction.
- [x] Extraction produces a deterministic directory used consistently for
      `GITHUB_PATH` and direct `zig version` execution.
- [x] No workflow path assumes a nonexistent `zig-0.14.1` directory.
- [x] Zig 0.14.1 and cargo-zigbuild 0.23.3 remain explicit release-tooling
      versions.
- [x] The release contract checker catches extraction/invocation path drift.

### Release workflow

- [ ] Linux x86-64 staged release binary executes successfully.
- [ ] Linux AArch64 staged release binary executes successfully on
      `ubuntu-24.04-arm` or an explicitly documented replacement ARM64 runner.
- [ ] Both Linux artifacts retain the verified glibc 2.17 build floor.
- [ ] macOS Intel/Apple Silicon and Windows x86-64 staged binaries execute
      successfully.
- [ ] Every mandatory binary passes `--version`, `--help`, and MCP smoke before
      checksum/upload.
- [ ] Assembly creates/updates only a draft and refuses to overwrite a published
      release.
- [ ] The mandatory asset set is complete.

### Installation/update/documentation

- [ ] Existing Unix/Windows installer corrections remain intact.
- [ ] Windows staged self-update semantics remain truthful and actionable.
- [ ] No current documentation claims v1.2.3 contains binary assets.
- [ ] Binary-first README instructions are enabled only after the first real
      binary-bearing release is published and installer URLs are verified.
- [ ] No daemon/service/restart behavior is introduced for stdio MCP.

### Closure evidence

- [ ] Actual release workflow completed successfully at least once.
- [ ] Closure records the release tag and workflow run ID.
- [ ] Closure records exact runner/target matrix, Zig/cargo-zigbuild versions,
      glibc floor, and AArch64 execution environment.
- [ ] Closure records Unix installer smoke and Windows installer/update results
      where available.
- [ ] Dependency/binary-size delta remains bounded; no release-only tool becomes
      a runtime dependency.

## Scope control

Do not use this pass to add:

- systemd, launchd, Windows SCM, cron, PID files, or `restart`;
- Streamable HTTP or another MCP transport;
- apt/deb/rpm, Homebrew, winget, Chocolatey, MSI, or container distribution;
- code-signing/notarization infrastructure;
- automatic crates.io publishing or tag creation;
- `cargo-dist`, `release-plz`, or a generalized packaging framework;
- Windows ARM64 binaries without a separate qualification decision;
- editor-specific extensions;
- unrelated MCP protocol changes;
- new deterministic tools or profile/schema changes.

The correct end state remains a small stdio MCP/utility binary with a reliable,
verified release path — not a deployment platform.

## Closure record

When this line passes, prune the active implementation detail and record:

1. C8 corrective implementation SHA;
2. first successful binary-bearing release tag;
3. release workflow run ID;
4. exact five-target published matrix;
5. Zig and cargo-zigbuild versions;
6. Linux glibc floor;
7. AArch64 native execution evidence;
8. ARMv7 status;
9. Unix/Windows installer smoke results;
10. Windows self-update/replacement result;
11. dependency/stripped-binary delta from the binary-distribution baseline;
12. ordinary CI and release-gate results.

Do not mark the binary-distribution line complete until the actual release
workflow has successfully produced and validated the asset set.

---

## Future opportunities after binary-release closure

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
