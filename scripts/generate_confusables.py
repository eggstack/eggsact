#!/usr/bin/env python3
"""Parse Unicode confusables.txt and generate a compact static confusables table.

This script downloads the confusables.txt from Unicode consortium
and generates a sorted static Rust table keyed by numeric code point.

The source is pinned to a specific expected version and SHA-256 checksum.
A mismatch in either value causes a loud failure before any output is written.

Generation is fail-closed: malformed data rows, invalid Unicode scalar
values (including surrogates), empty substitutions, and duplicate source
mappings all abort generation and no output file is written.

Usage:
    python3 scripts/generate_confusables.py            # regenerate both outputs
    python3 scripts/generate_confusables.py --check    # maintainer check: fetch
        pinned source, regenerate in memory, fail if either checked-in
        output differs (writes nothing; requires network to unicode.org)
    python3 scripts/generate_confusables.py --self-test  # offline fixture
        suite for the strict parser (no network)

Output:
    src/text/confusables_generated.rs  (bare array literal for include!())
    data/confusables.rs                (standalone reference copy)
"""

from __future__ import annotations

import argparse
import hashlib
import re
import sys
import urllib.request
from pathlib import Path

# Pinned source — update these values intentionally when upgrading Unicode.
UNICODE_SECURITY_VERSION = "17.0.0"
CONFUSABLES_URL = "https://www.unicode.org/Public/17.0.0/security/confusables.txt"
EXPECTED_SHA256 = "091c7f82fc39ef208faf8f94d29c244de99254675e09de163160c810d13ef22a"

OUTPUT_FILE = Path(__file__).parent.parent / "src" / "text" / "confusables_generated.rs"
DATA_OUTPUT = Path(__file__).parent.parent / "data" / "confusables.rs"

HEX_CP_RE = re.compile(r"([0-9A-Fa-f]{4,6})")


class ConfusablesParseError(ValueError):
    """Strict parse failure: malformed row, invalid scalar, or duplicate."""


def fetch_confusables_bytes() -> bytes:
    """Download the confusables.txt file and return raw bytes."""
    print(f"Fetching {CONFUSABLES_URL}...")
    with urllib.request.urlopen(CONFUSABLES_URL, timeout=30) as response:
        return response.read()


def verify_checksum(raw: bytes) -> None:
    """Verify the downloaded bytes match the expected SHA-256 checksum."""
    actual = hashlib.sha256(raw).hexdigest()
    if actual != EXPECTED_SHA256:
        print(
            f"ERROR: checksum mismatch\n"
            f"  Expected: {EXPECTED_SHA256}\n"
            f"  Observed: {actual}\n"
            f"  Pinned version: {UNICODE_SECURITY_VERSION}\n"
            f"  Source URL: {CONFUSABLES_URL}",
            file=sys.stderr,
        )
        sys.exit(1)
    print(f"Source checksum verified: {actual}")


def extract_version(content: str) -> str:
    """Extract the Unicode Security Mechanisms version from the file header."""
    for line in content.split("\n"):
        if line.strip().startswith("# Version:"):
            return line.split(":", 1)[1].strip()
    return "unknown"


def check_scalar(cp: int, context: str) -> int:
    """Reject non-scalars (out of range or surrogate halves)."""
    if not (0 <= cp <= 0x10FFFF) or 0xD800 <= cp <= 0xDFFF:
        raise ConfusablesParseError(f"{context}: invalid Unicode scalar U+{cp:04X}")
    return cp


def parse_code_point(s: str, lineno: int) -> int:
    """Parse a hex code point like '05AD' or '041F' into an integer (strict)."""
    s = s.strip()
    if not HEX_CP_RE.fullmatch(s):
        raise ConfusablesParseError(f"line {lineno}: malformed source code point {s!r}")
    return check_scalar(int(s, 16), f"line {lineno} source")


def parse_line(line: str, lineno: int) -> tuple[int, str] | None:
    """Parse a single line from confusables.txt (strict).

    Returns (source_code_point, substitution_string) tuple, or None for
    blank/comment lines. Anything else malformed raises
    ConfusablesParseError instead of being silently skipped.

    Format: CODEPOINT ; SUBSTITUTION ; TYPE # ... comment
    """
    stripped = line.strip()
    if not stripped or stripped.startswith("#"):
        return None

    parts = line.split(";")
    if len(parts) < 3:
        raise ConfusablesParseError(
            f"line {lineno}: malformed data row (expected 'CP ; SUB ; TYPE'): {line.strip()!r}"
        )

    source_str = parts[0].strip()
    substitution_str = parts[1].strip()
    type_str = parts[2].split("#")[0].strip()
    if not type_str:
        raise ConfusablesParseError(f"line {lineno}: missing mapping type column")

    source_cp = parse_code_point(source_str, lineno)

    sub_parts = substitution_str.split()
    if not sub_parts:
        raise ConfusablesParseError(f"line {lineno}: empty substitution")

    normalized: list[str] = []
    for p in sub_parts:
        p = p.strip()
        if not HEX_CP_RE.fullmatch(p):
            raise ConfusablesParseError(
                f"line {lineno}: malformed substitution code point {p!r}"
            )
        cp = check_scalar(int(p, 16), f"line {lineno} substitution")
        normalized.append(f"U+{cp:04X}")
    return (source_cp, " ".join(normalized))


def parse_confusables(content: str) -> dict[int, str]:
    """Parse confusables.txt content into a dictionary keyed by code point.

    Duplicate source mappings raise instead of last-write-wins.
    """
    result: dict[int, str] = {}
    data_started = False
    for lineno, line in enumerate(content.split("\n"), start=1):
        stripped = line.strip()
        if not data_started:
            if stripped.startswith("#") or not stripped:
                continue
            data_started = True

        parsed = parse_line(line, lineno)
        if parsed is None:
            continue
        source_cp, sub = parsed
        if source_cp in result:
            raise ConfusablesParseError(
                f"line {lineno}: duplicate source mapping for U+{source_cp:04X}"
            )
        result[source_cp] = sub

    if not result:
        raise ConfusablesParseError("no confusable mappings parsed")
    return result


def generate_rust_file(
    confusables: dict[int, str], version: str, checksum: str
) -> str:
    """Generate Rust source for the static confusables table.

    The output is a bare array literal suitable for inclusion via
    ``&include!("confusables_generated.rs")`` in confusables.rs.
    """
    lines = [
        f"// Unicode version: {version}",
        f"// Source checksum (SHA-256): {checksum}",
        "[",
    ]

    sorted_items = sorted(confusables.items())
    for source_cp, sub in sorted_items:
        lines.append(f"    (0x{source_cp:04X}, \"{sub}\"),")

    lines.append("]")
    return "\n".join(lines)


def render_data_output(
    confusables: dict[int, str], version: str, checksum: str
) -> str:
    """Render the standalone `data/confusables.rs` reference copy."""
    full_rust = f"""// Auto-generated from confusables.txt (Unicode UTS #39).
// Unicode version: {version}
// Source: {CONFUSABLES_URL}
// Source checksum (SHA-256): {checksum}
// DO NOT EDIT - regenerate with scripts/generate_confusables.py

/// Sorted static table of Unicode confusable mappings.
/// Key: source code point (u32). Value: substitution string (e.g. "U+0041").
pub static CONFUSABLES: &[(u32, &'static str)] = &[
"""
    for source_cp, sub in sorted(confusables.items()):
        full_rust += f"    (0x{source_cp:04X}, \"{sub}\"),\n"
    full_rust += "];\n"
    return full_rust


def verify_version(content: str) -> None:
    """Verify the file header reports the expected Unicode Security version."""
    actual = extract_version(content)
    if actual != UNICODE_SECURITY_VERSION:
        print(
            f"ERROR: version mismatch\n"
            f"  Expected: {UNICODE_SECURITY_VERSION}\n"
            f"  Observed: {actual}\n"
            f"  Source URL: {CONFUSABLES_URL}",
            file=sys.stderr,
        )
        sys.exit(1)
    print(f"Source version verified: {actual}")


def build_outputs(raw: bytes) -> tuple[str, str, dict[int, str]]:
    """Verify, parse, and render both outputs in memory (no writes)."""
    verify_checksum(raw)
    content = raw.decode("utf-8")
    verify_version(content)
    confusables = parse_confusables(content)
    print(f"Parsed {len(confusables)} confusable entries")
    checksum = hashlib.sha256(raw).hexdigest()
    rust_source = generate_rust_file(confusables, UNICODE_SECURITY_VERSION, checksum)
    full_rust = render_data_output(confusables, UNICODE_SECURITY_VERSION, checksum)
    return rust_source, full_rust, confusables


def run_check() -> int:
    """Maintainer freshness check: regenerate in memory, diff, write nothing.

    Requires network access to the pinned unicode.org URL; this is a
    maintainer/release check, NOT an ordinary merge-CI gate.
    """
    raw = fetch_confusables_bytes()
    rust_source, full_rust, confusables = build_outputs(raw)
    failures: list[str] = []
    if not OUTPUT_FILE.exists() or OUTPUT_FILE.read_text() != rust_source:
        failures.append(str(OUTPUT_FILE))
    if not DATA_OUTPUT.exists() or DATA_OUTPUT.read_text() != full_rust:
        failures.append(str(DATA_OUTPUT))
    if failures:
        print(
            "ERROR: generated confusables outputs are stale:\n  "
            + "\n  ".join(failures)
            + f"\nRegenerate with: python3 scripts/generate_confusables.py "
            f"({len(confusables)} entries, Unicode {UNICODE_SECURITY_VERSION})",
            file=sys.stderr,
        )
        return 1
    print(
        f"Confusables outputs are fresh "
        f"({len(confusables)} entries, Unicode {UNICODE_SECURITY_VERSION})"
    )
    return 0


def run_self_test() -> int:
    """Offline deterministic fixture suite for the strict parser (no network)."""
    failures: list[str] = []

    def expect_ok(name: str, text: str, expected: dict[int, str]) -> None:
        try:
            got = parse_confusables(text)
        except ConfusablesParseError as e:
            failures.append(f"{name}: unexpected failure: {e}")
            return
        if got != expected:
            failures.append(f"{name}: got {got!r}, want {expected!r}")

    def expect_fail(name: str, text: str) -> None:
        try:
            parse_confusables(text)
        except ConfusablesParseError:
            return
        failures.append(f"{name}: parsed without error, want failure")

    header = "# confusables.txt\n# Version: 17.0.0\n"
    expect_ok(
        "known-good miniature",
        header + "0041 ; 0061 ; MA # A -> a\n00E6 ; 0041 0045 ; MA # AE -> AE\n",
        {0x0041: "U+0061", 0x00E6: "U+0041 U+0045"},
    )
    expect_fail("duplicate source", header + "0041 ; 0061 ; MA\n0041 ; 0062 ; MA\n")
    expect_fail("malformed row (missing columns)", header + "0041 ; 0061\n")
    expect_fail("malformed source", header + "ZZZZ ; 0061 ; MA\n")
    expect_fail("empty substitution", header + "0041 ;  ; MA\n")
    expect_fail("malformed substitution", header + "0041 ; 00ZZ ; MA\n")
    expect_fail("surrogate source", header + "D800 ; 0061 ; MA\n")
    expect_fail("surrogate substitution", header + "0041 ; DFFF ; MA\n")
    expect_fail("out-of-range substitution", header + "0041 ; 110000 ; MA\n")
    expect_fail("missing type column", header + "0041 ; 0061 ;\n")
    expect_fail("empty file", header + "# no data\n")

    if failures:
        print("self-test FAILURES:", file=sys.stderr)
        for f in failures:
            print(f"  - {f}", file=sys.stderr)
        return 1
    print("generator self-test: all strict-parser fixtures pass")
    return 0


def main() -> None:
    """Main entry point.

    Safe-write sequence: fetch → checksum verify → parse → version verify →
    generate in memory → write both files.  No output is written until all
    checks pass.
    """
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="maintainer freshness check (fetch, regenerate in memory, "
        "fail if checked-in outputs differ; writes nothing)",
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="offline strict-parser fixture suite (no network)",
    )
    args = parser.parse_args()

    if args.self_test:
        sys.exit(run_self_test())

    if args.check:
        sys.exit(run_check())

    OUTPUT_FILE.parent.mkdir(parents=True, exist_ok=True)

    # 1. fetch bytes
    raw = fetch_confusables_bytes()
    print(f"Downloaded {len(raw)} bytes")

    # 2-6. verify + parse + render in memory (no writes until all checks pass)
    try:
        rust_source, full_rust, confusables = build_outputs(raw)
    except ConfusablesParseError as e:
        print(f"ERROR: strict parse failure: {e}", file=sys.stderr)
        sys.exit(1)

    # 7. write both files only after all checks pass
    OUTPUT_FILE.write_text(rust_source)
    print(f"Wrote {OUTPUT_FILE}")
    print(f"Generated {len(rust_source)} bytes of Rust code")

    DATA_OUTPUT.write_text(full_rust)
    print(f"Wrote {DATA_OUTPUT}")


if __name__ == "__main__":
    main()
