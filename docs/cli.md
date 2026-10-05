# eggsact CLI Usage

## Overview

`eggsact` is a command-line tool providing deterministic utility tools for coding agents. It evaluates mathematical expressions (including English like "thirty plus five") and can run as an MCP server for AI coding agents.

## Usage

```
eggsact [--mcp [--mcp-surface direct|discovery] | --diagnostics [--format json|text] | update | integrate <client> [--discovery] | expression]
```

- `--mcp` -- Start MCP server mode (reads JSON-RPC from stdin, writes to stdout)
- `--mcp-surface direct|discovery` -- Presentation surface for `--mcp` (default: `direct`; `discovery` advertises pinned front doors + `tool_search`/`tool_invoke`). `EGGSACT_MCP_SURFACE` sets the same; CLI overrides env.
- `--diagnostics` -- Print diagnostic information (version, tool count, profiles, budget tiers, runtime settings, env var names, generated data status)
- `--format json|text` -- Output format for `--diagnostics` (default: text)
- `-h`, `--help` -- Print usage information
- `-V`, `--version` -- Print the installed eggsact version
- `update` -- Download and verify the latest stable release, then replace the executable
- `integrate <client>` -- Render read-only MCP setup for `zed`, `codex`, `claude`, `cursor`, `vscode`, or `opencode`; `list` and `detect` are also available
- `expression` -- Math expression to evaluate (one or more arguments joined with spaces)
- No arguments -- Print usage message

## Modes

### Self-update

```bash
eggsact update
```

The updater uses crates.io `max_stable_version` as its authority, then fetches
the exact GitHub Release binary and SHA-256 sidecar for the host. It executes
the candidate and requires exact `eggsact X.Y.Z` output before replacement.
Unsupported hosts and genuine binary HTTP 404s use a staged exact-version Cargo
fallback. Transport, checksum, and candidate-version failures are hard errors.
The updater is self-contained (in-process HTTP/TLS, no external `curl`);
bootstrap installers still use external download tooling.

It never invokes `sudo`, kills MCP clients, or restarts sessions. Permission
errors print the elevated retry command. Existing client-owned stdio sessions
may continue using the previous image until their client reconnects.

### MCP client integration

```bash
eggsact integrate list
eggsact integrate detect
eggsact integrate zed
eggsact integrate zed --discovery
```

Integration output uses the resolved executable path and exactly `--mcp`. It is
an instruction/snippet, not a configuration mutation. See
`architecture/coding-agent-integration.md` for the current client formats.

### Expression Evaluation (default)

Pass a math expression as a quoted argument:

```bash
eggsact "5 + 3"
# Output: 8

eggsact "thirty plus five"
# Output: 35
```

Multiple arguments are joined with spaces, so quotes are optional in some shells:

```bash
eggsact 5 + 3
# Output: 8
```

### MCP Server Mode

Start the JSON-RPC 2.0 server over stdio:

```bash
eggsact --mcp
```

The server reads requests from stdin and writes responses to stdout. This mode is intended for integration with AI agent frameworks.

```bash
eggsact --mcp                                  # full catalog (77 tools at the default Model audience)
eggsact --mcp --mcp-surface discovery          # 7 front doors + tool_search/tool_invoke
# or: EGGSACT_MCP_SURFACE=discovery eggsact --mcp
```

The registry declares 86 tools. A `tools/list` response is filtered by the
active profile and audience, so a default Model-audience session sees 77; the
`full` profile exposes all 86. Run `eggsact --diagnostics` for the per-profile
counts.

Discovery mode advertises only the pinned front doors plus
`tool_search`/`tool_invoke`; all profile/audience-allowed capabilities stay
reachable through the facades. Direct mode (default) preserves the full catalog
for clients that implement their own deferred loading.

A legacy session requires `initialize` -> `notifications/initialized` before
`tools/list` or `tools/call`:

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"t","version":"1.0"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"text_measure","arguments":{"text":"the quick brown fox"}}}' \
  | eggsact --mcp
```

The `tools/call` response carries the tool's result as text content:

```json
{"ok": true, "tool": "text_measure", "result": {"bytes_utf8": 19, "words": 4, ...}}
```

Each stdio process is pinned to one protocol era by its first classifiable
message: an enveloped modern claim selects `2026-07-28`, while `initialize` or
any claim-less opening selects the legacy path. See
[MCP server internals](../architecture/mcp-server.md) for the era rules and the
modern handshake.

### Help and Version

```bash
eggsact --help
eggsact --version
```

### Diagnostics

```bash
eggsact --diagnostics
# Prints: version, tool count, active profile, budget tiers, env var names (no values),
# active audience, active schema detail, and runtime limits (
# max_in_flight_requests, max_tool_workers, max_request_bytes, max_output_bytes)

eggsact --diagnostics --format json
# Same information in JSON format
```

### No Arguments

```bash
eggsact
# Output:
# Usage: eggsact [--mcp [--mcp-surface direct|discovery] | --diagnostics [--format json|text] | update | integrate <client> [--discovery] | expression]
#   --mcp              Start MCP server mode
#   --mcp-surface      Presentation surface: direct (default) or discovery (pinned front doors + tool_search/tool_invoke)
#   --diagnostics      Print diagnostic information
#   --format json|text Output format for --diagnostics (default: text)
#   -h, --help         Print this help message
#   -V, --version      Print version information
#   update             Update from the latest stable crates.io release
#   integrate <name>   Render MCP setup for a client (or list/detect) [--discovery renders --mcp-surface discovery args]
#   expression         Evaluate math expression
```

## Examples

### Natural Language Math

```bash
eggsact "five plus three"                    # 8
eggsact "twenty times six"                   # 120
eggsact "one hundred divided by four"        # 25
eggsact "what is the square root of 144"    # 12
eggsact "calculate 2 to the power of 10"    # 1024
eggsact "50 percent of 200"                  # 100
eggsact "the sum of ten and twenty"          # 30
```

### Standard Math

```bash
eggsact "5 + 3"                              # 8
eggsact "2 ** 10"                            # 1024
eggsact "sqrt(144)"                          # 12
eggsact "sin(pi / 2)"                        # 1
eggsact "log(e)"                             # 1
eggsact "(10 + 2) / 4"                       # 3
eggsact "3**2 + 4**2"                        # 25
```

### Unit Conversions

```bash
eggsact "30m + 100ft"                        # 60.480000000000004 m
eggsact "1km in miles"                       # 0.621371192237334 mi
eggsact "72F in C"                           # 22.22222222222222 C
eggsact "1024 kilobytes in megabytes"        # 1 MB
eggsact "8 bits in bytes"                   # 1 B
eggsact "1gal in L"                          # 3.785411784 L
```

Data units use spelled-out names (`kilobyte`, `megabyte`, `gigabyte`, `byte`,
`bit`). The short symbols `KB`/`MB` are not accepted from the CLI:

```bash
eggsact "1024KB in MB"
# Error: Unknown unit: kb
```

### Functions

```bash
eggsact "sqrt(256)"                          # 16
eggsact "abs(-42)"                           # 42
eggsact "log(1024, 2)"                       # 10
eggsact "log(1000, 10)"                      # 2.9999999999999996
eggsact "log(e)"                             # 1
eggsact "sin(pi)"                            # 0.00000000000000012246467991473532
eggsact "ceil(3.2)"                          # 4
eggsact "floor(3.8)"                         # 3
```

`log(x)` is the natural logarithm; `log(x, base)` takes an explicit base.

> **Known limitation:** `log10(x)`, `log2(x)`, and `log1p(x)` are implemented in
> the expression parser but currently fail on the natural-language path used by
> this CLI and by the `math_eval` tool, which reports
> `Error: Unknown constant: log`. Use the two-argument `log(x, base)` form until
> this is fixed.

### Constants

```bash
eggsact "pi"                                 # 3.141592653589793
eggsact "e"                                  # 2.718281828459045
eggsact "c"                                  # 299792458 m/s
eggsact "gravity"                            # 9.80665
eggsact "na"                                 # 602214076000000000000000
```

`g` is gram, not gravity. Use `gravity` or `standardgravity` for standard
gravity.

## Error Output

Errors are printed to stderr with a non-zero exit code:

```bash
eggsact "1 / 0"
# stderr: Error: Division by zero
# exit code: 1

eggsact "sqrt(-1)"
# stderr: Error: Invalid operation: square root of negative number
# exit code: 1

eggsact "1024KB in MB"
# stderr: Error: Unknown unit: kb
# exit code: 1
```

MCP tool errors return `ok: false` in a JSON body with a stable `machine_code`
rather than a non-zero process exit. See
[Machine codes](../architecture/machine-codes.md).

## Piping

Standard input/output works normally. The MCP server mode reads from stdin and writes to stdout:

```bash
# MCP mode with piped input
echo '{"jsonrpc":"2.0","method":"initialize","id":1}' | eggsact --mcp

# Expression evaluation writes to stdout only
result=$(eggsact "2 ** 10")
echo $result  # 1024
```
