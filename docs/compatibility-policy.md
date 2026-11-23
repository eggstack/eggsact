# Compatibility Policy

This document defines versioning expectations, stability guarantees, and
deprecation rules for the eggsact crate. It is the single source of truth for
what constitutes a breaking change and how transitions are managed.

## Semantic Versioning

eggsact follows [Semantic Versioning 2.0.0](https://semver.org/):

- **MAJOR:** incompatible API changes, such as breaking Rust API changes, tool
  or machine-code removal, and schema changes that remove accepted inputs.
- **MINOR:** backwards-compatible functionality, such as new tools, optional
  fields, machine codes, or profiles.
- **PATCH:** backwards-compatible bug fixes, such as incorrect calculations,
  schema tightening for undocumented invalid inputs, and documentation fixes.

eggsact publishes 1.x releases under Semantic Versioning. The rules below define
which API and MCP changes are compatible within that series and which require a
major version.

## Minimum Supported Rust Version (MSRV)

`eggsact` requires Rust 1.89.0 or newer. This minimum applies to the library,
binaries, and the tests used for MSRV checks. Any increase will be announced in
a minor release with a changelog entry.

## Public Rust API Stability

Use the [Library API Reference](library-api.md) for supported entry points.
Other low-level items remain public for 1.x compatibility but are not
recommended for new integrations.

Within 1.x, removing or renaming a public item, changing its signature or return
type, or reducing its visibility is a MAJOR change. Adding a public item is
MINOR. Adding an enum variant is MINOR only when the enum has a catch-all
variant; otherwise it is MAJOR.

## MCP Tool Name Stability

Tool names and aliases are stable contracts once released. Renaming or removing
a tool is a MAJOR change; adding one is MINOR. Deprecating a tool is MINOR, but
it remains functional during the deprecation period. Adding an alias is MINOR;
removing one is breaking when a supported client depends on it.

## MCP Input Schema Compatibility

Input schemas define the JSON arguments a tool accepts.

- **Adding an optional property** (one not in `required`) is non-breaking.
- **Adding a new value to an enum** is non-breaking (clients send strings they
  already know).
- **Removing a property** is breaking.
- **Renaming a property** is breaking.
- **Moving a property from optional to required** is breaking.
- **Changing a property's type** is breaking.
- **Tightening validation** (rejecting inputs that were previously accepted) is
  breaking UNLESS the previously accepted input was a documented bug or was
  already rejected by the Python reference.

Schema changes that tighten validation should be documented in `CHANGELOG.md`
under the "Changed" section with a clear explanation of what inputs are now
rejected.

## MCP Output Schema Compatibility

Output schemas define the JSON structure returned by a tool.

- **Adding an optional field** to the output is non-breaking.
- **Adding a new machine code** is non-breaking (it's a new string value in an
  existing field).
- **Removing a field** is breaking.
- **Renaming a field** is breaking.
- **Changing a field's type** is breaking.
- **Changing a machine code's meaning** for an existing response shape is
  breaking.

Machine-code and verdict fields used for programmatic routing are part of the
response contract. Removing them or changing their meaning is breaking.

## Protocol Versions

Modern (`2026-07-28`) responses include structured results and metadata; legacy
(`2025-11-25` and `2024-11-05`) response envelopes remain unchanged. Wire
serialization can vary by protocol era, but tool names, input and output
semantics, machine codes, and profile behavior remain stable. See the
[MCP server architecture](../architecture/mcp-server.md) for negotiation details.

## Machine-Code Stability

Machine codes are stable identifiers for tool errors and findings. Adding a
code or alias is MINOR. Removing a code, changing its value or changing the
meaning of an existing code is MAJOR. Deprecated codes keep their original
value for at least two minor releases.

## Profile and Audience Compatibility

Profiles and audiences control which tools an MCP client can see and call.
Adding a profile or making an additional tool available is MINOR. Removing a
profile or reducing tool availability for a supported integration is MAJOR.
Changes that reduce access must be documented.

## CompatibilityMode Behavior

`CompatibilityMode` controls validation and error type names. The MCP server
uses `EggcalcPython`; in-process APIs default to `StrictNative`. Both reject
booleans for numeric fields. Changing accepted inputs or switching an API's
default mode is a compatibility change and must be documented.

## Deprecation Policy

We announce deprecations with a migration path. Deprecated tools remain
functional for at least two minor releases or six months, whichever is longer.
Deprecated machine codes and profiles remain supported for at least two minor
releases. Removing a deprecated item is a MAJOR change.

## Breaking Changes

Major releases are required to remove or rename a public Rust API item or MCP
tool, remove a machine code or profile, remove an input or output field, rename
an output field or change its type, switch a `CompatibilityMode` default, or
reject previously accepted valid input (except when fixing a documented bug).
New tools, profiles, aliases, machine codes, and optional schema fields are
normally backward-compatible additions.

## Further Reading

- [Library API Reference](library-api.md)
- [MCP Tool Reference](mcp-tools.md)
- [MCP Server Architecture](../architecture/mcp-server.md)
