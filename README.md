# eggsact

[![Crates.io](https://img.shields.io/crates/v/eggsact)](https://crates.io/crates/eggsact)
[![Downloads](https://img.shields.io/crates/d/eggsact)](https://crates.io/crates/d/eggsact)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Deterministic MCP server and in-process utility library for coding agents.
**86 tools across 23 categories** — math, text, JSON, regex, path, shell,
config, patch, dependency, analysis, network, encoding, and time — with no
network, clock, or filesystem state in the tool cores.

## Install

```bash
curl --proto '=https' --tlsv1.2 -fsSL \
  https://github.com/eggstack/eggsact/releases/latest/download/install.sh | bash
```

Windows:

```powershell
irm https://github.com/eggstack/eggsact/releases/latest/download/install.ps1 | iex
```

Or via Cargo (any host, needs Rust 1.89.0+):

```bash
cargo install eggsact
```

## Quick start

### Calculator from the shell

```bash
$ eggsact "thirty plus five"
35
$ eggsact "2 ** 10"
1024
$ eggsact "30m to ft"
98.42519685039369 ft
```

It also parses natural language, unit conversions, and ~90 math functions.
Note `^` is XOR (use `**` for power) and `g` means gram — use `gravity` for
standard gravity. Full example set:
[docs/cli.md](docs/cli.md).

### MCP server

```bash
eggsact --mcp                              # stdio JSON-RPC, full catalog
eggsact --mcp --mcp-surface discovery      # low-context: 7 front doors + search/invoke
eggsact --diagnostics                      # tool counts per profile, versions, limits
```

Point your client at it — `eggsact integrate <client>` renders a read-only
config snippet for `zed`, `codex`, `claude`, `cursor`, `vscode`, or `opencode`:

```bash
eggsact integrate detect    # which clients are installed
eggsact integrate codex     # config snippet
```

### Library

```rust
use eggsact::{run, evaluate};

assert_eq!(run("thirty plus five").unwrap().0, "35");
assert_eq!(evaluate("2 ** 10").unwrap().0, "1024");
```

In-process tool dispatch through the typed agent API:

```rust
use eggsact::agent::{ToolRegistry, ExecutionContext, Profile, ToolAudience};

let registry = ToolRegistry::default();
let ctx = ExecutionContext::agent_default(Profile::Full, ToolAudience::Model);
let response = registry.call_json_with_execution_context(
    "math_eval",
    serde_json::json!({"expression": "2 + 3"}),
    &ctx,
).unwrap();

assert!(response.ok);
// result: {"value": "5", "type": "int"}
```

## Documentation

| Topic | Link |
|-------|------|
| **Getting started** | |
| CLI usage, all flags, worked examples | [docs/cli.md](docs/cli.md) |
| Installation, updates, target matrix | [docs/installation.md](docs/installation.md) |
| MCP client setup | [architecture/coding-agent-integration.md](architecture/coding-agent-integration.md) |
| **Using the tools** | |
| MCP tool reference (86 tools) | [docs/mcp-tools.md](docs/mcp-tools.md) |
| Math features, functions, constants, units | [docs/math-features.md](docs/math-features.md) |
| Library API hierarchy | [docs/library-api.md](docs/library-api.md) |
| **Reference** | |
| Compatibility policy | [docs/compatibility-policy.md](docs/compatibility-policy.md) |
| Python `eggcalc` parity status | [docs/parity.md](docs/parity.md) |
| **Architecture** | |
| Overview (start here) | [architecture/overview.md](architecture/overview.md) |
| MCP server and protocol eras | [architecture/mcp-server.md](architecture/mcp-server.md) |
| Registry, profiles, audiences | [architecture/registry-profiles.md](architecture/registry-profiles.md) |
| Tool adapters | [architecture/tools.md](architecture/tools.md) |
| Text processing library | [architecture/text-library.md](architecture/text-library.md) |
| Calculator core | [architecture/calculator.md](architecture/calculator.md) |
| Agent API | [architecture/agent-api.md](architecture/agent-api.md) |
| Typed preflight wrappers | [architecture/preflight.md](architecture/preflight.md) |
| Machine codes | [architecture/machine-codes.md](architecture/machine-codes.md) |
| Budget and concurrency | [architecture/budget-concurrency.md](architecture/budget-concurrency.md) |
| Binary distribution | [architecture/cli-binaries.md](architecture/cli-binaries.md) |
| Self-update | [architecture/self-update.md](architecture/self-update.md) |
| **Contributing** | |
| Contributing guide | [docs/contributing.md](docs/contributing.md) |
| Testing patterns | [architecture/testing.md](architecture/testing.md) |
| Verification doctrine | [docs/verification.md](docs/verification.md) |
| Fuzzing | [docs/fuzzing.md](docs/fuzzing.md) |
| Performance evidence | [architecture/performance.md](architecture/performance.md) |
| Release process | [docs/release.md](docs/release.md) |
| Minimum supported Rust version | [docs/msrv.md](docs/msrv.md) |

`AGENTS.md` and `.opencode/skills/` hold agent-facing conventions and per-task
playbooks.

## Supported platforms

| Tier | Platform | Status |
|------|----------|--------|
| 1 | Ubuntu latest (x86_64) | Full CI gate |
| 2 | Windows latest (x86_64) | Compile check + byte-reproducible release build |
| 2 | macOS latest (ARM64) | Compile check + release build |

## Relationship to Python eggcalc

`eggsact` is a Rust reimplementation of the Python `eggcalc` project. Core math,
unit conversion, and text operations are equivalent; 37 known behavioral
differences are tracked in [docs/parity.md](docs/parity.md).

## License

MIT — see [LICENSE](LICENSE).
