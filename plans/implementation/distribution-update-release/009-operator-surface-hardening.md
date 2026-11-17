# Distribution, Update, and Release Milestone 009 — Operator-Surface Hardening

Status: implemented — local full merge gate green; closure record pending; Windows behavior reviewed statically (no native Windows execution)

Repository baseline: `main` at `b4ec154` plus the uncommitted audit dependency refresh

Source roadmap:

- `plans/subsystems/distribution-update-release-roadmap.md#7`
- `plans/subsystems/distribution-update-release-roadmap.md#10`

Long-term requirements:

- `plans/000-long-term-specification.md#3` (self-update exception)
- `plans/000-long-term-specification.md#6` (distribution and release)
- `plans/001-terminology-and-domain-model.md#7` (targets, transport, contract)

Applicable ADRs:

- `plans/adrs/ADR-0001-planning-conventions-adoption.md`

Primary class: invariant + polish

## 1. Objective

Close three local trust and quoting gaps in the operator surface without
changing update transport, read-only guarantees, or any MCP behavior:

1. Windows update replacement invokes `powershell.exe` through the OS search
   order instead of an absolute system path.
2. The update staging directory is created with default permissions and only
   then restricted to `0700`.
3. The Windows `integrate` renderer uses a backslash-quote escape that
   `cmd.exe` does not honor and that leaves `%VAR%` expansion in place.

## 2. Why this milestone is ready

- All three sites are local, small, and have existing test modules.
- No dependency, credential, network, or protocol change is required.
- The fixes are independent of the blocked registry adoption (007) and the
  ADR-gated signing work (008).
- The read-only and transport invariants are preserved, so no roadmap
  decision is needed.

## 3. Current implementation evidence

- `src/update.rs:1098` runs `Command::new("powershell.exe")` with
  `-NoProfile -NonInteractive -Command <script>`. Windows process creation
  searches the application directory and current directory before the system
  directory, so a planted `powershell.exe` can be selected when the user runs
  `eggsact update` from an attacker-influenced location.
- `src/update.rs:1084-1096` builds a nonce-tagged adjacent file and copies the
  candidate before spawning the replacement script; the digest is re-checked
  in the script before `Move-Item`. That integrity behavior must not change.
- `src/update.rs:571-600` creates the staging directory with `fs::create_dir`
  and only then applies `0700` on Unix. The directory is empty during the
  window, which bounds the exposure, but the safe construction is available.
- `src/integrate.rs:134-140` renders Windows shell values as
  `"value"` with `\"` escapes. `cmd.exe` does not treat backslash as an
  escape and expands `%NAME%` inside double quotes. `src/integrate.rs:91-98`
  nests JSON inside that value for VS Code.
- `src/integrate.rs:206` prints that the output is an instruction only;
  eggsact never executes it or edits client configuration.
- Existing tests: `src/integrate.rs:217-256` cover renderers and discovery
  args; `src/update.rs:1538+` cover the replacement script.

## 4. Invariants that must not regress

- `update` and `integrate` remain verified/read-only; no client config is
  edited and no daemon is installed.
- The Windows replacement still verifies the SHA-256 immediately before
  `Move-Item`, never kills the running MCP process, and records failure in
  the status file.
- The staging directory is private from creation; the retry loop on
  `AlreadyExists` and the same-filesystem assumptions are preserved.
- The update transport policy (HTTP/1, rustls, bounded bodies, no retries,
  explicit proxy) is untouched.
- JSON renderers remain valid JSON; `codex`, `cursor`, `zed`, and `opencode`
  outputs keep their shapes.
- `integrate` output remains a valid registration instruction for each
  supported client; it must never be a command that expands an
  attacker-influenced `%VAR%` on Windows.
- No new shell execution, including in tests.

## 5. Scope

### In scope

- Absolute Windows PowerShell resolution with fail-closed behavior.
- Private staging-directory creation on Unix.
- Correct, shell-safe Windows rendering for `integrate`.
- Tests and documentation updates.

### Explicitly out of scope

- Signature verification (008).
- Dependency/source policy (006/007).
- Any update transport, retry, timeout, or feature change.
- Changing the client list or the server registration semantics.
- Executing Windows binaries or shells in tests.

## 6. Required production changes

### Operator surface (CLI, update, integrate)

- Resolve PowerShell from `%SystemRoot%` (for example
  `%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe`). If the
  variable or the file is missing, return an actionable error instead of
  falling back to `PATH`.
- Create the staging directory with mode `0700` at creation time on Unix
  (`DirBuilderExt::mode`), keeping the existing collision retry and cleanup
  on permission failure.
- Replace the Windows `integrate` escaping with a rendering that is safe for
  the shell it names. If no single one-liner is safe for both `cmd.exe` and
  PowerShell, emit a configuration block (with the target file path) instead
  of a shell command on Windows. The exact choice is implementation-defined
  but must satisfy the acceptance tests.

### Core/service/adapter

None.

### Registry, profile, audience, surface

None.

### Protocol and DTOs

None.

### Runtime and concurrency

No change to update ordering or locking. The staging directory fix must not
alter the candidate path layout or the nonce format.

### Documentation and static guards

- `docs/cli.md` / `docs/installation.md`: platform-specific `integrate`
  output and the shell it targets.
- `CHANGELOG.md`: note the Windows rendering change if user-visible text
  changes.
- `architecture/self-update.md`: absolute PowerShell resolution and private
  staging.

## 7. Ordered work packages

### Work package A — Absolute PowerShell resolution

Intent: remove search-order hijacking from the Windows replacement path.

Required changes:

- Add a small resolver that takes the system root as an explicit input and
  returns the absolute PowerShell path, failing closed when absent.
- Use the resolver at the single spawn site; keep the existing arguments and
  stdio redirection.

Acceptance evidence:

- Unit tests cover present/absent/empty `SystemRoot` without spawning a
  process.
- The spawn site no longer contains a bare `powershell.exe`.
- A Windows-gated test (compiled but not run on macOS) covers the resolver's
  real environment input.

### Work package B — Private staging directory

Intent: remove the permission window.

Required changes:

- Create the directory already restricted on Unix; preserve the existing
  `AlreadyExists` retry and the remove-on-failure behavior.
- Do not change the nonce or the adjacent-file logic.

Acceptance evidence:

- A Unix test asserts mode `0700` immediately after creation.
- Collision handling still returns a unique directory under contention.

### Work package C — Safe Windows integrate rendering

Intent: prevent a copy-pasted instruction from being misinterpreted by the
Windows shell.

Required changes:

- Define the per-client rendering contract for Windows.
- Ensure no rendering relies on backslash escaping for quotes and no command
  form can expand `%VAR%` from the executable path.
- Keep JSON documents syntactically valid and parseable in tests.
- Preserve the read-only disclaimer.

Acceptance evidence:

- Tests cover paths containing spaces, single quotes, double quotes (as
  input text, even if not a legal Windows path), `%PATH%`, `&`, and `^`.
- JSON renderers still parse with `serde_json`.
- No test invokes a shell.

### Work package D — Documentation and closure

Update the operator docs and CHANGELOG; collect test evidence.

## 8. Failure, cancellation, truncation, and contention semantics

- Missing `SystemRoot`: fail before spawning, with a message naming the
  required Windows component. Never fall back to a relative executable.
- Staging creation failure or failed permission application: remove the
  directory and return the existing error class.
- Concurrent staging collisions: retry with the next counter; no shared
  state.
- Update cancellation: unchanged; the replacement script and status file
  behavior are untouched.
- Renderer: if a safe command form cannot be produced, emit the
  configuration block; never emit a best-effort quoted command.

## 9. Compatibility and migration

- Unix `integrate` output is unchanged.
- Windows `integrate` output may change from a `code --add-mcp` one-liner to
  a configuration block; document this in `CHANGELOG.md` and the CLI docs.
- JSON renderer shapes do not change.
- No updater behavior changes beyond the executable path and directory mode.

## 10. Required tests

### Focused unit tests

- PowerShell resolver: present, empty, missing, and non-existent file.
- Temp directory mode on Unix (`0700`), plus collision retry.
- Renderer cases: spaces, quotes, percent signs, ampersand, caret; JSON
  parseability; no `\"` escaping in Windows output.

### Integration tests

- `integrate` renderer test module updated for the Windows contract.
- Update tests that construct the replacement script still pass.

### Restart and recovery tests

- Existing staged-replacement failure/retry tests.

### Contention and cancellation tests

- Existing concurrent update tests; no new shared state.

### Negative and boundary tests

- Empty executable path (already rejected).
- Path ending in a separator and a path containing `%`.
- Non-UTF-8 path handling remains `to_string_lossy` behavior; document if a
  renderer cannot represent it safely.

### Parity and compatibility tests

None; no tool behavior changes.

## 11. Required verification commands

```bash
cargo test --locked --test lib integrate -- --test-threads=4
cargo test --locked --test lib update -- --test-threads=4
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --skip parity --test-threads=4
cargo test --locked --doc
```

Windows-only behavior is reviewed statically on this host; record that no
native Windows execution was performed.

## 12. Documentation updates

- `docs/cli.md` — per-platform `integrate` output and target shell.
- `docs/installation.md` — Windows registration guidance if it changes.
- `architecture/self-update.md` — absolute PowerShell path and private
  staging.
- `CHANGELOG.md` — user-visible CLI text change, if any.

## 13. Acceptance criteria

- The update path never invokes a PowerShell resolved from `PATH` or the
  current directory.
- The staging directory is `0700` from creation on Unix.
- Windows `integrate` output is safe to paste into its documented shell and
  cannot expand a `%VAR%` from the executable path.
- JSON outputs remain valid and the read-only disclaimer remains.
- No transport, retry, timeout, or MCP behavior changes.

## 14. Stop conditions

Stop and report when:

- a safe Windows rendering requires changing the supported client list or
  the registration semantics;
- `SystemRoot` is unavailable on a supported Windows configuration and no
  documented absolute fallback exists;
- the directory-mode change cannot preserve the collision/cleanup behavior;
- the fix would require executing a shell or Windows binary in tests;
- scope expands into signing or dependency policy.

## 15. Closure evidence required

- Resolver unit-test output and the spawn-site diff.
- Unix permission test output.
- Renderer fixtures for the hostile-character matrix.
- Full merge-gate output.
- A statement that Windows behavior was reviewed statically, not executed.

## 16. Handoff notes

- Do not change the nonce, the adjacent copy, or the digest check in the
  replacement script; those are deliberately hardened.
- Do not add a `PATH` fallback for PowerShell "for compatibility".
- Prefer emitting a configuration block over an unsafe one-liner on Windows.
- Keep line endings and the generated-doc guard intact; no ToolSpec change is
  expected.
- Implementation note (2026-11-17): `src/update.rs` and `src/integrate.rs` are
  binary modules, so their tests run with `cargo test --locked --bin eggsact
  <filter>`, not `--test lib`. The Windows rendering went to JSON config
  blocks for `claude` and `vscode` (the two command renderers); Unix keeps the
  existing one-liners. `render_for_platform` exposes the platform choice so
  the hostile-character matrix runs on macOS.
- The resolver also rejects relative, drive-relative (`C:Windows`), and
  root-relative (`\Windows`) `SystemRoot` values: joining one would produce a
  relative executable path and reintroduce the search-order risk. Drive-rooted
  (`C:\`, `C:/`) and UNC (`\\server\share`) roots are accepted. A Windows-only
  test covers the real environment input and is compiled but not run on macOS.
  The `integrate list` descriptions and
  `architecture/coding-agent-integration.md` now state the platform-specific
  rendering.
