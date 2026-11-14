# eggsact

[![Crates.io](https://img.shields.io/crates/v/eggsact)](https://crates.io/crates/eggsact)
[![Downloads](https://img.shields.io/crates/d/eggsact)](https://crates.io/crates/d/eggsact)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A deterministic MCP server and Rust utility library for coding agents. It
provides 86 tools across 23 categories, including math, text, JSON, validation,
repository analysis, configuration, patches, and time. Tool cores use explicit
inputs and do not access the network, clock, or filesystem.

## Install

macOS and Linux:

```bash
curl --proto '=https' --tlsv1.2 -fsSL \
  https://github.com/eggstack/eggsact/releases/latest/download/install.sh | bash
```

Windows PowerShell:

```powershell
irm https://github.com/eggstack/eggsact/releases/latest/download/install.ps1 | iex
```

Or install from source with Cargo. This requires Rust 1.89.0 or newer:

```bash
cargo install eggsact
```

See [installation details](docs/installation.md) for platform support, pinned
versions, and inspect-first instructions.

## Quick start

Use the calculator from your shell:

```console
$ eggsact "thirty plus five"
35
$ eggsact "2 ** 10"
1024
$ eggsact "30m to ft"
98.42519685039369 ft
```

Natural-language math, functions, and unit conversions are supported. `^` is
XOR, so use `**` for powers. `g` means gram; use `gravity` for standard gravity.
See [CLI usage](docs/cli.md) and [math features](docs/math-features.md).

## Use with an MCP client

`eggsact --mcp` starts the stdio MCP server. The client launches and manages this
process. The default `direct` surface lists tools allowed by the active profile
and audience. `discovery` lists a smaller set of entry tools while keeping the
same access rules.

```bash
eggsact --mcp
eggsact --mcp --mcp-surface discovery
eggsact integrate detect
eggsact integrate codex
```

`eggsact integrate` prints setup instructions and does not edit client
configuration. It supports Zed, Codex, Claude, Cursor, VS Code, and OpenCode.
See the [MCP tool reference](docs/mcp-tools.md) for tool behavior and
[installation](docs/installation.md) for client setup.

## Use from Rust

```toml
[dependencies]
eggsact = "1.2.7"
```

```rust
use eggsact::run;

let (answer, _kind) = run("thirty plus five").unwrap();
assert_eq!(answer, "35");
```

The [Library API reference](docs/library-api.md) covers calculator functions,
text utilities, typed preflight workflows, and in-process tool dispatch.

## Documentation

- [Installation and updates](docs/installation.md)
- [CLI usage](docs/cli.md)
- [MCP tools](docs/mcp-tools.md)
- [Math and units](docs/math-features.md)
- [Rust library API](docs/library-api.md)
- [Compatibility policy](docs/compatibility-policy.md)
- [Python `eggcalc` parity status](docs/parity.md)
- [Architecture overview](architecture/overview.md)
- [Contributing](docs/contributing.md)

`eggsact` is a Rust implementation of the Python `eggcalc` MCP server with a
larger tool catalog. Its parity report lists 39 accepted test cases, represented
by 40 ignored test instances, separately from regressions.

## License

MIT. See [LICENSE](LICENSE).
