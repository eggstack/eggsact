# Coding-Agent MCP Integration

eggsact exposes two integration paths: the in-process Rust API for harnesses,
and MCP stdio for external clients. The `eggsact integrate` command supports
the second path by rendering setup instructions for a client-owned process.

## Client-owned stdio contract

Every renderer points at a resolved Eggsact executable path and passes exactly
`--mcp`. This avoids GUI applications depending on an interactive-shell PATH
and remains stable across an in-place `eggsact update`.

```text
client -> eggsact --mcp stdin/stdout pipes
```

EOF closes that one session. Eggsact does not add a daemon, restart command,
systemd/launchd/SCM registration, cron watchdog, PID file, singleton lock, or
HTTP transport. The client owns process lifetime and reconnects after updates.

## CLI surface

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

`list` describes the supported adapters. `detect` only checks known command
names on `PATH`; it does not recursively scan the filesystem or read unrelated
configuration. A client name renders an instruction and never mutates files.

## Rendered formats

| Client | Output |
|---|---|
| Zed | `context_servers.eggsact` JSON settings block |
| Codex | `mcp_servers.eggsact` TOML block and the current `codex mcp add` shape |
| Claude Code | `claude mcp add eggsact -- <path> --mcp` |
| Cursor | `mcpServers.eggsact` JSON block |
| VS Code / Copilot | `code --add-mcp` JSON command |
| OpenCode v2 | `mcp.servers.eggsact` local-server JSONC block |

These are renderers rather than a configuration abstraction. JSON renderers
are parsed in unit tests, and all adapters are checked for the server name,
resolved path, and stdio argument. Paths containing spaces are quoted for the
command form and JSON-escaped for config forms.

Codex and Claude Code currently expose native registration CLIs, and VS Code
exposes `code --add-mcp`; Eggsact still prints those commands without running
them. A future explicit `--install` may use a verified native CLI, but direct
JSONC/TOML editing is intentionally out of scope.

## Configuration examples

The command output is authoritative for the installed binary path. The shapes
below show the contracts without assuming a particular installation location.

```json
{"context_servers":{"eggsact":{"command":"/absolute/path/eggsact","args":["--mcp"],"env":{}}}}
```

```json
{"mcpServers":{"eggsact":{"command":"/absolute/path/eggsact","args":["--mcp"]}}}
```

```jsonc
{"mcp":{"servers":{"eggsact":{"type":"local","command":["/absolute/path/eggsact","--mcp"]}}}}
```

Use `eggsact --diagnostics` or the binary's `--version` before registering a
path copied from another machine.
