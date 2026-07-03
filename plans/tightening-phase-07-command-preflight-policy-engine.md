# Phase 7: Command Preflight Policy Engine

## Goal

Turn `command_preflight` from a useful shell-risk heuristic into a first-class, policy-driven harness API for codegg command execution. The harness should be able to ask one deterministic preflight endpoint whether a model-authored command may run, should require review, or must be blocked.

Phase 7 must preserve the boundary established in phases 01–06: ordinary model-facing MCP calls use model audience, harness enforcement uses the in-process API, and route-critical tools emit stable verdicts, machine codes, summaries, and strict findings.

## Current State

The repo already has the foundation:

- `command_preflight` exists in `src/tools/shell.rs`.
- It is classified as route-critical.
- Typed wrappers expose `CommandPreflightInput`, `CommandPreflightOutput`, `CommandVerdict`, findings, summary, machine code, and optional argv.
- Route-contract tests assert `command_preflight` emits a machine code, verdict, and summary on successful responses.
- `shell_split`, `shell_quote_join`, and `argv_compare` provide deterministic shell helpers.
- Profile/audience dispatch now prevents model-facing MCP calls from directly using harness-only tools.

The missing piece is a configurable policy engine with clear semantics, stable rule identifiers, platform-aware parsing, and high-confidence test coverage.

## Scope

Phase 7 covers shell/command preflight only. Do not add general sandboxing, process execution, daemon workers, terminal integration, or codegg command scheduling in this phase. This phase produces deterministic pre-execution decisions, not actual command execution.

## Workstream A: Define the Command Policy Model

### Problem

`CommandPolicy::{Default, Strict, Permissive}` exists, but policy semantics are too coarse for real harness enforcement. codegg needs stable rule-level decisions that can vary by project, environment, and command class without rewriting the tool.

### Required Work

1. Define a policy input object that can be passed to `command_preflight`.

   Suggested schema:

   ```json
   {
     "mode": "default|strict|permissive|custom",
     "platform": "posix|windows|auto",
     "allow_commands": ["cargo", "git", "rg"],
     "deny_commands": ["rm", "curl", "ssh"],
     "allow_subcommands": {"cargo": ["check", "test", "fmt"]},
     "deny_subcommands": {"git": ["push", "reset", "clean"]},
     "allow_network": false,
     "allow_filesystem_write": false,
     "allow_process_control": false,
     "allow_env_mutation": false,
     "max_command_length": 4096,
     "working_directory": "optional/path/context"
   }
   ```

   Keep the existing simple `policy` enum for compatibility. If a structured `policy_config` is present, it should refine or override the enum according to documented precedence.

2. Define default built-in policies.

   Recommended:

   - `permissive`: block only clearly destructive or exfiltration-like patterns.
   - `default`: allow common read-only and build/test commands, review writes/network/process-control, block obvious destructive patterns.
   - `strict`: allow only explicitly safe commands and require review/block for everything else.

3. Document policy precedence.

   Example:

   - schema validation first.
   - parse command into argv and shell features.
   - deny rules beat allow rules.
   - explicit allow beats generic review if no deny rule matches.
   - block severity dominates review.

### Acceptance Criteria

- Policy semantics are explicit and stable.
- Backward compatibility with existing `policy` enum remains.
- Custom policy input is bounded and validated.

## Workstream B: Platform-Aware Parsing and Shell Feature Extraction

### Problem

Command risk depends on shell grammar. The preflight should extract enough structure to detect risky behavior without pretending to fully emulate every shell.

### Required Work

1. Define supported platforms:

   - `posix`
   - `windows`
   - `auto`

2. For POSIX commands, detect at least:

   - pipes: `|`
   - command chaining: `;`, `&&`, `||`
   - command substitution: `` `cmd` ``, `$(cmd)`
   - redirection: `>`, `>>`, `<`, `2>`, `&>`
   - backgrounding: `&`
   - globbing indicators: `*`, `?`, `[...]`
   - environment assignment prefix: `FOO=bar cmd`
   - subshell grouping: `( ... )`

3. For Windows commands, detect at least:

   - `cmd.exe /c`
   - PowerShell invocation.
   - command chaining with `&`, `&&`, `||`.
   - redirection.
   - encoded PowerShell command flags.

4. Represent extracted features in result JSON.

   Suggested result fields:

   ```json
   {
     "argv": ["cargo", "test"],
     "program": "cargo",
     "subcommand": "test",
     "features": ["pipe", "redirection"],
     "platform": "posix"
   }
   ```

5. Keep parsing deterministic and non-executing. Do not call shell commands.

### Acceptance Criteria

- The parser handles quoted strings and common escapes robustly enough for preflight.
- Extracted features are stable and covered by tests.
- Unsupported shell constructs produce review/block findings rather than panic.

## Workstream C: Rule Evaluation and Finding Taxonomy

### Problem

The harness needs actionable findings, not just a generic `SHELL_RISK` code.

### Required Work

1. Define rule IDs and finding codes.

   Candidate codes:

   - `COMMAND_OK`
   - `SHELL_PARSE_ERROR`
   - `SHELL_RISK`
   - `SHELL_NETWORK_ACCESS`
   - `SHELL_FILESYSTEM_WRITE`
   - `SHELL_DESTRUCTIVE_COMMAND`
   - `SHELL_PROCESS_CONTROL`
   - `SHELL_ENV_MUTATION`
   - `SHELL_PRIVILEGE_ESCALATION`
   - `SHELL_COMMAND_SUBSTITUTION`
   - `SHELL_REDIRECTION`
   - `SHELL_PIPELINE`
   - `SHELL_BACKGROUND_EXECUTION`
   - `SHELL_UNAPPROVED_COMMAND`

2. Add or reuse machine-code constants.

   If new constants are added, update:

   - `src/mcp/machine_codes.rs`
   - `machine_codes::ALL`
   - `architecture/machine-codes.md`
   - generated docs if relevant.

3. Define severity/disposition mapping.

   Suggested mapping:

   - allow: no findings or info-only findings.
   - review: medium severity/caution findings.
   - block: high/critical severity/blocking findings.

4. Implement deterministic primary-code selection.

   Priority example:

   1. parse error.
   2. destructive command.
   3. privilege escalation.
   4. network access denied.
   5. filesystem write denied.
   6. process control denied.
   7. command substitution.
   8. redirection/pipeline/background review.
   9. unapproved command.
   10. ok.

5. Add `recommended_next_tool` where useful.

   Examples:

   - `shell_split` when parsing is ambiguous.
   - `argv_compare` when a harness has both raw command and argv expectation.
   - `text_security_inspect` if Unicode/suspicious text is detected in command text.

### Acceptance Criteria

- Findings have stable codes, severity, message, and disposition.
- Primary machine code selection is deterministic.
- Verdict, machine code, and findings cannot disagree.

## Workstream D: Safe Command Profiles for codegg

### Problem

codegg needs practical defaults. Common development commands should not constantly require review, while dangerous commands must not slip through.

### Required Work

1. Define a default allow/review/block matrix for common tools.

   Suggested allow under default policy:

   - `cargo check`
   - `cargo test`
   - `cargo fmt --check`
   - `cargo clippy`
   - `rustc --version`
   - `python -m pytest` if Python projects are relevant.
   - `rg`, `grep`, `find` with read-only options.
   - `git status`, `git diff`, `git log`, `git show`.

   Suggested review:

   - `cargo fmt` without `--check`.
   - `cargo fix`.
   - `git add`, `git checkout`, `git restore`.
   - package-manager install/update commands.
   - network fetches: `curl`, `wget`, `git fetch`, `git pull`.

   Suggested block:

   - `rm -rf` outside allowed temp paths.
   - `sudo`, `su`, privilege escalation.
   - `chmod -R 777`.
   - `chown -R`.
   - shell pipes into shell interpreters: `curl ... | sh`.
   - background daemons without explicit harness approval.
   - destructive git commands: `git reset --hard`, `git clean -fdx`, `git push --force`.

2. Put policy tables in code as data, not scattered conditionals.

3. Add tests for every table entry.

4. Avoid overfitting to Rust only. Keep Rust commands first-class, but make the engine extensible.

### Acceptance Criteria

- Common codegg build/test commands are allowed under default policy.
- Dangerous commands are blocked.
- Ambiguous write/network commands route to review.

## Workstream E: Typed API and Schema Updates

### Required Work

1. Extend `CommandPreflightInput` with policy config fields.

2. Extend `CommandPreflightOutput` with structured fields:

   - `program`
   - `subcommand`
   - `features`
   - `policy_mode`
   - `matched_rules`
   - `working_directory` if accepted as context.

3. Update JSON schemas in `src/mcp/schemas/shell.rs`.

4. Update `ToolSpec` descriptions and generated tool cards.

5. Add strict wrapper parsing tests for new fields if they are route-critical.

### Acceptance Criteria

- Typed wrappers expose all route-critical data codegg needs.
- Schema validation catches invalid policy configs.
- Generated docs are current.

## Workstream F: Tests and Fixtures

### Required Test Groups

1. Parser fixtures:

   - quotes and escapes.
   - pipes and redirections.
   - command substitution.
   - background execution.
   - Windows PowerShell encoded command.

2. Policy fixtures:

   - default allow commands.
   - default review commands.
   - default block commands.
   - strict mode.
   - permissive mode.
   - custom allow/deny overrides.

3. Route-contract fixtures:

   - success emits `COMMAND_OK`.
   - review emits `SHELL_RISK` or more specific code.
   - block emits specific block code.
   - findings are strict and structured.

4. Regression fixtures:

   - `curl URL | sh` blocks.
   - `rm -rf /` blocks.
   - `git reset --hard` blocks or reviews according to policy.
   - `cargo test` allows.

## Verification Commands

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo run --bin generate-docs -- --check
cargo package --verbose
```

## Final Acceptance Criteria

Phase 7 is complete when:

- `command_preflight` has explicit policy semantics.
- Platform-aware parsing extracts stable features.
- Built-in policies are data-driven and tested.
- Dangerous commands block, common build/test commands allow, ambiguous commands review.
- Typed wrappers and schemas reflect the policy engine.
- Route-critical contract tests pass.
- Generated docs/tool cards are current.
