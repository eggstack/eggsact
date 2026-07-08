# eggsact CLI Usage

## Overview

`eggsact` is a command-line tool providing deterministic utility tools for coding agents. It evaluates mathematical expressions (including English like "thirty plus five") and can run as an MCP server for AI coding agents.

## Usage

```
eggsact [--mcp | --diagnostics [--format json|text] | expression]
```

- `--mcp` -- Start MCP server mode (reads JSON-RPC from stdin, writes to stdout)
- `--diagnostics` -- Print diagnostic information (version, tool count, profiles, budget tiers, runtime settings, env var names, generated data status)
- `--format json|text` -- Output format for `--diagnostics` (default: text)
- `-h`, `--help` -- Print usage information
- `-V`, `--version` -- Print the installed eggsact version
- `expression` -- Math expression to evaluate (one or more arguments joined with spaces)
- No arguments -- Print usage message

## Modes

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

### Help and Version

```bash
eggsact --help
eggsact --version
```

### Diagnostics

```bash
eggsact --diagnostics
# Prints: version, tool count, active profile, budget tiers, env var names (no values),
# active audience, active schema detail, and runtime limits (max_requests_per_second,
# max_in_flight_requests, max_tool_workers, max_request_bytes, max_output_bytes)

eggsact --diagnostics --format json
# Same information in JSON format
```

### No Arguments

```bash
eggsact
# Output:
# Usage: eggsact [--mcp | --diagnostics [--format json|text] | expression]
#   --mcp          Start MCP server mode
#   --diagnostics  Print diagnostic information
#   --format       Output format for --diagnostics (default: text)
#   -h, --help     Print this help message
#   -V, --version  Print version information
#   expression     Evaluate math expression
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
eggsact "1km in miles"                       # 0.621371...
eggsact "72F in C"                           # 22.2222...
eggsact "1024KB in MB"                       # 1
eggsact "1gal in L"                          # 3.78541...
```

### Functions

```bash
eggsact "sqrt(256)"                          # 16
eggsact "abs(-42)"                           # 42
eggsact "log10(1000)"                        # 3
eggsact "log2(1024)"                         # 10
eggsact "sin(pi)"                            # ~0
eggsact "ceil(3.2)"                          # 4
eggsact "floor(3.8)"                         # 3
```

### Constants

```bash
eggsact "pi"                                 # 3.14159...
eggsact "e"                                  # 2.71828...
eggsact "c"                                  # speed of light
eggsact "gravity"                            # 9.80665
eggsact "na"                                 # Avogadro's number
```

## Error Output

Errors are printed to stderr with a non-zero exit code:

```bash
eggsact "1 / 0"
# stderr: Error: Division by zero
# exit code: 1

eggsact "sqrt(-1)"
# stderr: Error: ...
# exit code: 1
```

## Piping

Standard input/output works normally. The MCP server mode reads from stdin and writes to stdout:

```bash
# MCP mode with piped input
echo '{"jsonrpc":"2.0","method":"initialize","id":1}' | eggsact --mcp

# Expression evaluation writes to stdout only
result=$(eggsact "2 ** 10")
echo $result  # 1024
```
