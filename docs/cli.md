# CLI Usage

`eggsact` evaluates expressions, runs an MCP server, prints diagnostics, updates
the installed binary, and renders MCP client setup instructions.

## Commands

```text
eggsact [options] [expression]
eggsact update
eggsact integrate [client] [--discovery]
```

| Command or option | Description |
|---|---|
| `expression` | Evaluate math. Multiple arguments are joined with spaces. |
| `--mcp` | Start the MCP server over stdio. The client should launch this process. |
| `--mcp-surface direct or discovery` | Select the MCP presentation surface. Requires `--mcp`; defaults to `direct`. |
| `--diagnostics` | Print version, tool counts, active settings, and runtime limits. |
| `--format json or text` | Set diagnostics output format. Requires `--diagnostics`; defaults to `text`. |
| `update` | Update to latest stable; see [installation details](installation.md). |
| `integrate` | List supported clients. `integrate detect` reports installed clients. |
| `integrate <client>` | Print setup instructions for `zed`, `codex`, `claude`, `cursor`, `vscode`, or `opencode`. |
| `--discovery` | Render setup for the discovery surface. Use with `integrate <client>`. |
| `-h`, `--help` | Print command help. |
| `-V`, `--version` | Print the installed version. |

Running `eggsact` with no arguments prints help.

## Calculator

Quote the expression when your shell would otherwise interpret its symbols:

```bash
eggsact "thirty plus five"
eggsact "2 ** 10"
eggsact "30m to ft"
```

Multiple arguments are joined with spaces, so this also works in many shells:

```bash
eggsact 5 + 3
```

The calculator accepts natural-language math, functions, constants, and unit
conversions. `**` means power; `^` means bitwise XOR. `g` means gram. Use
`gravity` or `standardgravity` for standard gravity. See
[Math Features](math-features.md) for supported operations and units.

Errors go to stderr and return a non-zero exit code:

```console
$ eggsact "1 / 0"
Error: Division by zero
```

## MCP server

Start the stdio server with:

```bash
eggsact --mcp
```

The default `direct` surface advertises tools allowed by the active profile and
audience. Use `discovery` when the client supports deferred tool discovery. It
shows a smaller set of entry tools plus search and invoke, without changing
access policy:

```bash
eggsact --mcp --mcp-surface discovery
# Or set the server-wide default:
EGGSACT_MCP_SURFACE=discovery eggsact --mcp
```

Other profile and audience settings may hide tools. See [MCP tools](mcp-tools.md)
for behavior and [server architecture](../architecture/mcp-server.md) for
protocol details.

## Client setup

`integrate` prints instructions or configuration snippets. It does not edit
client files.

```bash
eggsact integrate list
eggsact integrate detect
eggsact integrate codex
eggsact integrate codex --discovery
```

Replace `codex` with another supported client name. On Unix, `claude` and
`vscode` print copyable shell commands. On Windows, they print a JSON
configuration block and its target file. See the
[client integration guide](../architecture/coding-agent-integration.md).

## Diagnostics

```bash
eggsact --diagnostics
eggsact --diagnostics --format json
```

Diagnostics include the version, tool counts by profile, active profile,
audience, schema detail, MCP surface, configured limits, and environment
variable names. Environment variable values are not printed.
