#!/usr/bin/env python3
"""Parse Unicode confusables.txt and generate confusables.rs.

This script downloads the latest confusables.txt from Unicode consortium
and generates a Rust HashMap for use in text processing.

Usage: python3 scripts/generate_confusables.py
Output: data/confusables.rs
"""

from __future__ import annotations

import re
import urllib.request
from pathlib import Path

CONFUSABLES_URL = "https://www.unicode.org/Public/security/latest/confusables.txt"
OUTPUT_FILE = Path(__file__).parent.parent / "src" / "text" / "confusables_generated.rs"
DATA_OUTPUT = Path(__file__).parent.parent / "data" / "confusables.rs"


def fetch_confusables_txt() -> str:
    """Download the confusables.txt file."""
    print(f"Fetching {CONFUSABLES_URL}...")
    with urllib.request.urlopen(CONFUSABLES_URL, timeout=30) as response:
        return response.read().decode("utf-8")


def parse_code_point(s: str) -> str | None:
    """Parse a hex code point like '05AD' or '041F' into Unicode char.

    Returns the character, or None if invalid.
    """
    s = s.strip()
    if not s:
        return None
    match = re.fullmatch(r"([0-9A-Fa-f]{4,6})", s)
    if not match:
        return None
    return chr(int(s, 16))


def parse_line(line: str) -> tuple[str, str] | None:
    """Parse a single line from confusables.txt.

    Returns (source_char, substitution) tuple, or None if skip.
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

    source_char = parse_code_point(source_str)
    if source_char is None:
        return None

    sub_parts = substitution_str.split()
    if not sub_parts:
        return None

    try:
        substitution = "".join(chr(int(p.strip(), 16)) for p in sub_parts)
        return (source_char, substitution)
    except (ValueError, OverflowError):
        return None


def parse_confusables(content: str) -> dict[str, str]:
    """Parse confusables.txt content into a dictionary."""
    result: dict[str, str] = {}
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
            source, sub = parsed
            result[source] = sub

    return result


def generate_rust_file(confusables: dict[str, str]) -> str:
    """Generate Rust source for confusables.rs."""
    lines = [
        "// Auto-generated from confusables.txt (Unicode UTS #39).",
        "// DO NOT EDIT - regenerate with scripts/generate_confusables.py",
    ]

    sorted_items = sorted(confusables.items(), key=lambda x: ord(x[0]))

    for source, sub in sorted_items:
        source_cp = f"U+{ord(source):04X}"
        sub_cps = " ".join(f"U+{ord(c):04X}" for c in sub)
        lines.append(f'm.insert("{source_cp}", "{sub_cps}");')

    return "\n".join(lines)


def main() -> None:
    """Main entry point."""
    OUTPUT_FILE.parent.mkdir(parents=True, exist_ok=True)

    content = fetch_confusables_txt()
    print(f"Downloaded {len(content)} bytes")

    confusables = parse_confusables(content)
    print(f"Parsed {len(confusables)} confusable entries")

    rust_source = generate_rust_file(confusables)

    OUTPUT_FILE.write_text(rust_source)
    print(f"Wrote {OUTPUT_FILE}")

    print(f"Generated {len(rust_source)} bytes of Rust code")

    full_rust = f'''// Auto-generated from confusables.txt (Unicode UTS #39).
// DO NOT EDIT - regenerate with scripts/generate_confusables.py

use once_cell::sync::Lazy;
use std::collections::HashMap;

pub static CONFUSABLES: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {{
    let mut m: HashMap<&'static str, &'static str> = HashMap::new();
{chr(10).join("    " + line for line in rust_source.splitlines())}
    m
}});
'''
    DATA_OUTPUT.write_text(full_rust)
    print(f"Wrote {DATA_OUTPUT}")


if __name__ == "__main__":
    main()
