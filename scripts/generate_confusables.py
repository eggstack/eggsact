#!/usr/bin/env python3
"""Parse Unicode confusables.txt and generate a compact static confusables table.

This script downloads the confusables.txt from Unicode consortium
and generates a sorted static Rust table keyed by numeric code point.

The source is pinned to a specific expected version and SHA-256 checksum.
A mismatch in either value causes a loud failure before any output is written.

Usage: python3 scripts/generate_confusables.py
Output: src/text/confusables_generated.rs
"""

from __future__ import annotations

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


def parse_code_point(s: str) -> int | None:
    """Parse a hex code point like '05AD' or '041F' into an integer.

    Returns the code point integer, or None if invalid.
    """
    s = s.strip()
    if not s:
        return None
    match = re.fullmatch(r"([0-9A-Fa-f]{4,6})", s)
    if not match:
        return None
    return int(s, 16)


def parse_line(line: str) -> tuple[int, str] | None:
    """Parse a single line from confusables.txt.

    Returns (source_code_point, substitution_string) tuple, or None if skip.
    Format: CODEPOINT ; SUBSTITUTION ; TYPE # ... comment
    """
    line = line.strip()
    if not line or line.startswith("#"):
        return None

    parts = line.split(";")
    if len(parts) < 2:
        return None

    source_str = parts[0].strip()
    substitution_str = parts[1].strip()

    source_cp = parse_code_point(source_str)
    if source_cp is None:
        return None

    sub_parts = substitution_str.split()
    if not sub_parts:
        return None

    try:
        sub_cps = " ".join(
            f"U+{int(p.strip(), 16):04X}" for p in sub_parts
        )
        return (source_cp, sub_cps)
    except (ValueError, OverflowError):
        return None


def parse_confusables(content: str) -> dict[int, str]:
    """Parse confusables.txt content into a dictionary keyed by code point."""
    result: dict[int, str] = {}
    lines = content.split("\n")

    data_started = False
    for line in lines:
        stripped = line.strip()
        if not data_started:
            if stripped.startswith("#") or not stripped:
                continue
            data_started = True

        parsed = parse_line(line)
        if parsed:
            source_cp, sub = parsed
            result[source_cp] = sub

    return result


def generate_rust_file(
    confusables: dict[int, str], version: str, checksum: str
) -> str:
    """Generate Rust source for the static confusables table.

    The output is a bare array literal suitable for inclusion via
    ``&include!(\"confusables_generated.rs\")`` in confusables.rs.
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


def main() -> None:
    """Main entry point.

    Safe-write sequence: fetch → checksum verify → parse → version verify →
    generate in memory → write both files.  No output is written until all
    checks pass.
    """
    OUTPUT_FILE.parent.mkdir(parents=True, exist_ok=True)

    # 1. fetch bytes
    raw = fetch_confusables_bytes()
    print(f"Downloaded {len(raw)} bytes")

    # 2. compute and compare SHA-256
    verify_checksum(raw)

    # 3. decode
    content = raw.decode("utf-8")

    # 4. extract and compare header version
    verify_version(content)

    # 5. parse confusables
    confusables = parse_confusables(content)
    print(f"Parsed {len(confusables)} confusable entries")

    # 6. compute checksum of the decoded content for the header
    checksum = hashlib.sha256(raw).hexdigest()
    version = UNICODE_SECURITY_VERSION

    # 7. generate both outputs in memory
    rust_source = generate_rust_file(confusables, version, checksum)

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

    # 8. write both files only after all checks pass
    OUTPUT_FILE.write_text(rust_source)
    print(f"Wrote {OUTPUT_FILE}")
    print(f"Generated {len(rust_source)} bytes of Rust code")

    DATA_OUTPUT.write_text(full_rust)
    print(f"Wrote {DATA_OUTPUT}")


if __name__ == "__main__":
    main()
