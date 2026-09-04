# eggsact

[![Crates.io](https://img.shields.io/crates/v/eggsact)](https://crates.io/crates/eggsact)
[![Downloads](https://img.shields.io/crates/d/eggsact)](https://crates.io/crates/eggsact)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Deterministic MCP and in-process utility tools for coding agents. 86 tools across 23 categories: math, text, JSON, regex, path, shell, config, patch, dependency, analysis, network, encoding, temporal, and more. Includes a natural language math evaluator that parses expressions like "thirty plus five" or "30m + 100ft".

## Installation

Fastest path on Linux and macOS (verified release binary, with Cargo fallback
only for an unsupported host or missing asset):

```bash
curl --proto '=https' --tlsv1.2 -fsSL \
  https://github.com/eggstack/eggsact/releases/latest/download/install.sh | bash
```

Windows PowerShell:

```powershell
irm https://github.com/eggstack/eggsact/releases/latest/download/install.ps1 | iex
```

Pinned installs use `--version X.Y.Z` for `install.sh` and `-Version X.Y.Z`
for `install.ps1`. See [Installation](docs/installation.md) for the supported
binary matrix, validation behavior, and inspect-first forms.

Cargo remains available when a source install is preferred:

```bash
cargo install eggsact
```

Or from source:

```bash
git clone https://github.com/eggstack/eggsact
cd eggsact
cargo install --path .
```

**Minimum Rust version**: 1.89.0.

## Quick Start

### CLI

```bash
# Natural language
eggsact "thirty plus five"            # 35

# Standard math
eggsact "2 ** 10"                     # 1024

# Unit conversions
eggsact "30m to ft"                   # 98.425...

# MCP server mode (stdio JSON-RPC)
eggsact --mcp
```

### MCP client setup

Render a read-only, absolute-path registration instruction for a client-owned
stdio process:

```bash
eggsact integrate list
eggsact integrate detect
eggsact integrate codex    # zed, claude, cursor, vscode, or opencode
```

Update an installed binary with `eggsact update`. It verifies the crates.io
stable version, GitHub asset checksum, and candidate `--version` before
replacement. Existing MCP sessions continue until their owning client
reconnects. Eggsact does not install a daemon or edit client configuration.

### Library

```rust
use eggsact::{run, evaluate};

// Natural language
let (result, _typ) = run("thirty plus five").unwrap();
assert_eq!(result, "35");

// Direct math
let (result, _typ) = evaluate("2 ** 10").unwrap();
assert_eq!(result, "1024");
```

### In-Process Agent API

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
```

## Supported Platforms

| Tier | Platform | Status |
|------|----------|--------|
| 1 | Ubuntu latest (x86_64) | Full CI gate |
| 2 | Windows latest (x86_64) | Compile check |
| 2 | macOS latest (ARM64) | Compile check |

## Documentation

| Topic | Link |
|-------|------|
| CLI usage | [docs/cli.md](docs/cli.md) |
| Installation, updates, and client setup | [docs/installation.md](docs/installation.md) |
| Library API | [docs/library-api.md](docs/library-api.md) |
| MCP tool reference (86 tools) | [docs/mcp-tools.md](docs/mcp-tools.md) |
| Math features, functions, constants, units | [docs/math-features.md](docs/math-features.md) |
| Architecture overview | [architecture/overview.md](architecture/overview.md) |
| Calculator core | [architecture/calculator.md](architecture/calculator.md) |
| MCP server internals | [architecture/mcp-server.md](architecture/mcp-server.md) |
| Agent API deep dive | [architecture/agent-api.md](architecture/agent-api.md) |
| Preflight wrappers | [architecture/preflight.md](architecture/preflight.md) |
| Machine codes | [architecture/machine-codes.md](architecture/machine-codes.md) |
| Profiles and audiences | [architecture/registry-profiles.md](architecture/registry-profiles.md) |
| Text processing library | [architecture/text-library.md](architecture/text-library.md) |
| Budget and concurrency | [architecture/budget-concurrency.md](architecture/budget-concurrency.md) |
| Compatibility policy | [docs/compatibility-policy.md](docs/compatibility-policy.md) |
| Verification doctrine | [docs/verification.md](docs/verification.md) |
| Testing patterns | [architecture/testing.md](architecture/testing.md) |
| Contributing | [docs/contributing.md](docs/contributing.md) |
| Fuzzing | [docs/fuzzing.md](docs/fuzzing.md) |
| Release process | [docs/release.md](docs/release.md) |

## Key Gotchas

- **`^` is XOR, not exponentiation.** Use `**` for power. Matches Python.
- **`g` means gram** in unit expressions. Use `gravity` for standard gravity.
- `serde_json` uses `preserve_order` — key order is intentional in serialized JSON.
- Network, encoding, datetime, and cron utilities are deterministic and use no system/network state; temporal conversions accept fixed offsets only. IPv6 CIDR counts are exact decimal powers of two, mapped-IPv6 metadata is limited to `::ffff:0:0/96`, and cron DOM/DOW matching follows Vixie/Cronie star syntax (either field starting with `*`, including `*/n` steps, requires both fields to match; otherwise either may match; explicit full ranges are not star syntax).

## Relationship to Python eggcalc

`eggsact` is a Rust reimplementation of the Python `eggcalc` project. The two projects are functionally equivalent for core math, unit conversion, and text processing operations. See [docs/parity.md](docs/parity.md) for known differences.

## License

MIT -- see [LICENSE](LICENSE) for details.
