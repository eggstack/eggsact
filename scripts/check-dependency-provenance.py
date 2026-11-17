#!/usr/bin/env python3
"""Release-gate dependency provenance preflight.

Published artifacts must be registry-sourced. A dependency declared with
``git`` and no ``version`` cannot be packaged at all; adding a ``version`` key
to make packaging pass would make ``cargo publish`` resolve the registry
release instead of the pinned revision, silently changing the dependency graph.

Exit codes:
  0  every publishable dependency is registry-sourced with a version
  1  at least one dependency is git-sourced without a version
  2  usage, toolchain, or manifest error (fail closed)
"""

from __future__ import annotations

import sys
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:  # Python < 3.11
    sys.stderr.write(
        "dependency provenance: requires Python 3.11+ (tomllib); "
        f"found {sys.version.split()[0]}\n"
    )
    raise SystemExit(2)

PLAN = "plans/implementation/distribution-update-release/007-eggup-registry-adoption.md"
DEPENDENCY_TABLES = ("dependencies", "dev-dependencies", "build-dependencies")
USAGE = "usage: check-dependency-provenance.py [Cargo.toml | --self-test]"


def dependency_tables(manifest):
    """Yield (label, table) for every table that can declare a dependency."""
    for name in DEPENDENCY_TABLES:
        table = manifest.get(name)
        if isinstance(table, dict):
            yield name, table

    workspace = manifest.get("workspace")
    if isinstance(workspace, dict):
        table = workspace.get("dependencies")
        if isinstance(table, dict):
            yield "workspace.dependencies", table

    targets = manifest.get("target")
    if isinstance(targets, dict):
        for target, config in targets.items():
            if not isinstance(config, dict):
                continue
            for name in DEPENDENCY_TABLES:
                table = config.get(name)
                if isinstance(table, dict):
                    yield f"target.{target}.{name}", table


def versionless_git_dependencies(manifest):
    """Return (table, dependency, source) triples for versionless git deps."""
    violations = []
    for table_name, table in dependency_tables(manifest):
        for name, spec in table.items():
            if isinstance(spec, dict) and "git" in spec and "version" not in spec:
                violations.append((table_name, name, str(spec["git"])))
    return violations


def check_manifest_bytes(data, label):
    """Return (exit_code, lines). Never raises for malformed input."""
    try:
        manifest = tomllib.loads(data.decode("utf-8"))
    except UnicodeDecodeError as error:
        return 2, [f"dependency provenance: {label} is not valid UTF-8: {error}"]
    except tomllib.TOMLDecodeError as error:
        return 2, [f"dependency provenance: cannot parse {label}: {error}"]

    if not isinstance(manifest, dict):
        return 2, [f"dependency provenance: {label} is not a TOML document"]
    if "package" not in manifest and "workspace" not in manifest:
        return 2, [
            f"dependency provenance: {label} is not a Cargo manifest "
            "(missing [package] or [workspace])"
        ]

    violations = versionless_git_dependencies(manifest)
    if not violations:
        return 0, []

    lines = ["dependency provenance: publishable dependencies must be registry-sourced"]
    for table_name, name, source in violations:
        lines.append(
            f"  - {name} in [{table_name}] is git-sourced without a version: {source}"
        )
    lines.extend(
        [
            "",
            "Why this cannot be repaired in place:",
            "  cargo package requires a version for every dependency, but adding one",
            "  makes cargo publish resolve the registry release instead of the pinned",
            "  revision, silently changing the shipped dependency graph.",
            "",
            f"Remediation: {PLAN}",
            "  Requires a published Eggup registry release whose dependency graph",
            "  resolves zip at or above 9.6.1 (the pinned commit line).",
            "  Do not add version keys to the git declarations.",
        ]
    )
    return 1, lines


def check_path(path):
    """Return (exit_code, lines) for one manifest path."""
    try:
        data = path.read_bytes()
    except FileNotFoundError:
        return 2, [f"dependency provenance: manifest not found: {path}"]
    except OSError as error:
        return 2, [f"dependency provenance: cannot read {path}: {error}"]
    return check_manifest_bytes(data, str(path))


SELF_TESTS = (
    (
        "registry-only dependency with a version passes",
        """
[package]
name = "fixture"
version = "0.1.0"

[dependencies]
eggup-core = { version = "0.2.1" }
""",
        0,
        None,
    ),
    (
        "git without version fails and names the owning plan",
        """
[package]
name = "fixture"
version = "0.1.0"

[dependencies]
eggup-acquisition = { git = "https://github.com/eggstack/eggup", rev = "abc123" }
""",
        1,
        "eggup-acquisition",
    ),
    (
        "git plus version passes (the source policy owns that case)",
        """
[package]
name = "fixture"
version = "0.1.0"

[dependencies]
eggup-core = { git = "https://github.com/eggstack/eggup", version = "0.2.1" }
""",
        0,
        None,
    ),
    (
        "malformed TOML fails closed with a parse message",
        """
[package
name = "fixture"
""",
        2,
        "cannot parse",
    ),
    (
        "target-specific dependency tables are scanned",
        """
[package]
name = "fixture"
version = "0.1.0"

[target.'cfg(unix)'.dependencies]
eggup-eggpack = { git = "https://github.com/eggstack/eggup" }
""",
        1,
        "target.cfg(unix).dependencies",
    ),
    (
        "dev-dependencies are scanned",
        """
[package]
name = "fixture"
version = "0.1.0"

[dev-dependencies]
eggup-eggfetch = { git = "https://github.com/eggstack/eggup" }
""",
        1,
        "dev-dependencies",
    ),
    (
        "workspace dependency tables are scanned",
        """
[workspace]
members = []

[workspace.dependencies]
eggup-core = { git = "https://github.com/eggstack/eggup" }
""",
        1,
        "workspace.dependencies",
    ),
    (
        "path and workspace-only entries pass",
        """
[package]
name = "fixture"
version = "0.1.0"

[dependencies]
local-helper = { path = "../local-helper" }
serde = { workspace = true }

[workspace.dependencies]
serde = { version = "1.0" }
""",
        0,
        None,
    ),
    (
        "a package without a dependency table passes",
        """
[package]
name = "fixture"
version = "0.1.0"
""",
        0,
        None,
    ),
    (
        "an empty document fails closed as a non-Cargo manifest",
        "",
        2,
        "not a Cargo manifest",
    ),
)


def self_test():
    failures = 0
    for name, manifest, expected_code, expected_text in SELF_TESTS:
        code, lines = check_manifest_bytes(manifest.encode("utf-8"), name)
        text = "\n".join(lines)
        passed = code == expected_code and (
            expected_text is None or expected_text in text
        )
        if passed:
            print(f"  ok  {name}")
        else:
            failures += 1
            print(f"  FAIL {name}: exit {code}, expected {expected_code}")
            if text:
                print(text)
    if failures:
        sys.stderr.write(f"dependency provenance self-test: {failures} failure(s)\n")
        return 1
    print(f"dependency provenance self-test: {len(SELF_TESTS)} checks passed")
    return 0


def main(argv):
    if "--help" in argv or "-h" in argv:
        print(USAGE)
        return 0
    if argv and argv[0] == "--self-test":
        return self_test()
    if len(argv) > 1:
        sys.stderr.write(f"{USAGE}\n")
        return 2

    path = Path(argv[0]) if argv else Path(__file__).resolve().parent.parent / "Cargo.toml"
    code, lines = check_path(path)
    if code == 0:
        print(f"dependency provenance: ok ({path})")
    else:
        for line in lines:
            print(line, file=sys.stderr)
    return code


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
