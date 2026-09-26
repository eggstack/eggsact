#!/usr/bin/env python3
"""Generate Unicode 18.0.0 security-relevant property tables for UTS #39.

Pinned sources (Unicode 18.0.0 UCD):
- DerivedCoreProperties.txt (Default_Ignorable_Code_Point)
- Scripts.txt (Script)
- ScriptExtensions.txt (Script_Extensions overrides)
- extracted/DerivedBidiClass.txt (Bidi_Class)
- BidiMirroring.txt (Bidi_Mirroring_Glyph)
- BidiBrackets.txt (Bidi_Paired_Bracket)
- PropertyValueAliases.txt (Script long<->short normalization)

Generation is fail-closed: checksum/version mismatch, malformed rows,
invalid scalars (including surrogates), duplicate/overlapping ranges, and
unknown Bidi_Class values abort with no output written.

Usage:
    python3 scripts/generate_unicode_security_properties.py
    python3 scripts/generate_unicode_security_properties.py --check
    python3 scripts/generate_unicode_security_properties.py --self-test

Output:
    src/text/unicode_properties_generated.rs (generated module, never hand-edit)
"""

from __future__ import annotations

import argparse
import hashlib
import re
import sys
import urllib.request
from pathlib import Path

UNICODE_VERSION = "18.0.0"
UCD_BASE = "https://www.unicode.org/Public/18.0.0/ucd/"

SOURCES: dict[str, tuple[str, str]] = {
    "DerivedCoreProperties.txt": (
        UCD_BASE + "DerivedCoreProperties.txt",
        "09c928886a178fcafd93c29e4bd59073a058e5a100b716d425cb563ab50f68c9",
    ),
    "Scripts.txt": (
        UCD_BASE + "Scripts.txt",
        "0071fd81b6aeae25f6e8bce8efec3066a6476a91b49bdb2f52dc76e817862a6a",
    ),
    "ScriptExtensions.txt": (
        UCD_BASE + "ScriptExtensions.txt",
        "5c9d34a922f687726f2a8bcf57d49f905987e51f1b21b58c95a00fbe255cec23",
    ),
    "DerivedBidiClass.txt": (
        UCD_BASE + "extracted/DerivedBidiClass.txt",
        "d9e23222522551348ea1ccfbb4f62efbf98982afb95840f8959c08ed992c5607",
    ),
    "BidiMirroring.txt": (
        UCD_BASE + "BidiMirroring.txt",
        "cd54810ebf52f0e61a730c8b9cb25975de6c85f6d788a559b416afd548923fd6",
    ),
    "BidiBrackets.txt": (
        UCD_BASE + "BidiBrackets.txt",
        "4b3b62e4a14b84ee752808c810c602534921c09a4a1bf78cfbee566d66c125b3",
    ),
    "PropertyValueAliases.txt": (
        UCD_BASE + "PropertyValueAliases.txt",
        "06c4c8eaf7b0bf34abe73b113da1215bd784ac254d4c223600b90267caa4bbbd",
    ),
}

OUTPUT_FILE = (
    Path(__file__).parent.parent / "src" / "text" / "unicode_properties_generated.rs"
)

HEX_RE = re.compile(r"[0-9A-Fa-f]{4,6}")

# Strict UAX #44 `@missing` directive: `# @missing: START..END; Value`.
MISSING_RE = re.compile(
    r"^#\s*@missing:\s*([0-9A-Fa-f]{4,6})\.\.([0-9A-Fa-f]{4,6})\s*;\s*([A-Za-z_]+)\s*(?:#.*)?$"
)

VALID_BIDI_CLASSES = {
    "L", "R", "AL", "EN", "ES", "ET", "AN", "CS", "NSM", "BN",
    "FSI", "LRI", "RLI", "PDI", "LRO", "RLO", "PDF", "LRE", "RLE",
    "WS", "ON", "B", "S",
}


class PropertyParseError(ValueError):
    pass


def fetch_bytes(name: str) -> bytes:
    url, _ = SOURCES[name]
    print(f"Fetching {url}...")
    with urllib.request.urlopen(url, timeout=60) as r:
        return r.read()


def verify_checksum(name: str, raw: bytes) -> None:
    _, expected = SOURCES[name]
    actual = hashlib.sha256(raw).hexdigest()
    if actual != expected:
        print(
            f"ERROR: checksum mismatch for {name}\n"
            f"  Expected: {expected}\n"
            f"  Observed: {actual}",
            file=sys.stderr,
        )
        sys.exit(1)
    print(f"Checksum verified {name}: {actual[:12]}...")


def check_scalar(cp: int, context: str) -> int:
    if not (0 <= cp <= 0x10FFFF) or 0xD800 <= cp <= 0xDFFF:
        raise PropertyParseError(f"{context}: invalid Unicode scalar U+{cp:04X}")
    return cp


def parse_range_field(field: str, lineno: int, context: str) -> tuple[int, int]:
    field = field.strip()
    if ".." in field:
        lo_s, hi_s = field.split("..", 1)
        lo_s, hi_s = lo_s.strip(), hi_s.strip()
        if not HEX_RE.fullmatch(lo_s) or not HEX_RE.fullmatch(hi_s):
            raise PropertyParseError(f"{context} line {lineno}: malformed range {field!r}")
        lo = check_scalar(int(lo_s, 16), f"{context} line {lineno}")
        hi = check_scalar(int(hi_s, 16), f"{context} line {lineno}")
        if lo > hi:
            raise PropertyParseError(f"{context} line {lineno}: inverted range {field!r}")
        return (lo, hi)
    else:
        if not HEX_RE.fullmatch(field):
            raise PropertyParseError(f"{context} line {lineno}: malformed code point {field!r}")
        cp = check_scalar(int(field, 16), f"{context} line {lineno}")
        return (cp, cp)


def check_sorted_non_overlapping(ranges: list[tuple[int, int]], context: str) -> None:
    for i in range(1, len(ranges)):
        plo, phi = ranges[i - 1]
        clo, chi = ranges[i]
        _ = chi
        if clo <= phi:
            raise PropertyParseError(
                f"{context}: overlapping/unsorted ranges "
                f"U+{plo:04X}..U+{phi:04X} and U+{clo:04X}..U+{chi:04X}"
            )


def parse_sc_aliases(content: str) -> tuple[dict[str, str], dict[str, str]]:
    """Map Script long names (and short) to canonical short codes.

    Parses PropertyValueAliases.txt `sc` rows: `sc ; Short ; Long (; Long2...)`.
    Returns (alias_to_short, short_to_canonical_long).
    """
    mapping: dict[str, str] = {}
    short_to_long: dict[str, str] = {}
    for lineno, line in enumerate(content.split("\n"), start=1):
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        if not stripped.startswith("sc "):
            continue
        # sc ; Short ; Long ; Long2 ...
        parts = [p.strip() for p in line.split(";")]
        if len(parts) < 3:
            raise PropertyParseError(f"PropertyValueAliases line {lineno}: malformed sc row")
        short = parts[1]
        if not short:
            raise PropertyParseError(f"PropertyValueAliases line {lineno}: empty short code")
        mapping[short] = short
        long_name = parts[2].split("#")[0].strip()
        if long_name and short not in short_to_long:
            short_to_long[short] = long_name
        for alias in parts[2:]:
            alias = alias.split("#")[0].strip()
            if alias:
                # First alias wins; later duplicates must agree or fail.
                if alias in mapping and mapping[alias] != short:
                    raise PropertyParseError(
                        f"PropertyValueAliases line {lineno}: conflicting alias {alias!r}"
                    )
                mapping[alias] = short
    if "Latin" not in mapping or mapping.get("Latin") != "Latn":
        raise PropertyParseError("sc alias table missing Latin->Latn")
    if "Common" not in mapping or mapping.get("Common") != "Zyyy":
        raise PropertyParseError("sc alias table missing Common->Zyyy")
    return mapping, short_to_long


def parse_default_ignorables(content: str) -> list[tuple[int, int]]:
    out: list[tuple[int, int]] = []
    for lineno, line in enumerate(content.split("\n"), start=1):
        s = line.strip()
        if not s or s.startswith("#"):
            continue
        if "Default_Ignorable_Code_Point" not in line:
            continue
        # Format: range ; Property # comment
        semi = line.split(";")
        if len(semi) < 2:
            raise PropertyParseError(f"DerivedCoreProperties line {lineno}: malformed row")
        rng = parse_range_field(semi[0], lineno, "DerivedCoreProperties")
        prop = semi[1].split("#")[0].strip()
        if prop != "Default_Ignorable_Code_Point":
            continue
        out.append(rng)
    out.sort()
    check_sorted_non_overlapping(out, "Default_Ignorable_Code_Point")
    if not out:
        raise PropertyParseError("no Default_Ignorable_Code_Point ranges parsed")
    return out


def parse_scripts(content: str, aliases: dict[str, str]) -> list[tuple[int, int, str]]:
    out: list[tuple[int, int, str]] = []
    for lineno, line in enumerate(content.split("\n"), start=1):
        s = line.strip()
        if not s or s.startswith("#"):
            continue
        parts = line.split(";")
        if len(parts) < 2:
            raise PropertyParseError(f"Scripts line {lineno}: malformed row")
        rng = parse_range_field(parts[0], lineno, "Scripts")
        script_long = parts[1].split("#")[0].strip()
        if script_long not in aliases:
            raise PropertyParseError(f"Scripts line {lineno}: unknown script {script_long!r}")
        out.append((rng[0], rng[1], aliases[script_long]))
    out.sort(key=lambda t: (t[0], t[1]))
    check_sorted_non_overlapping([(a, b) for a, b, _ in out], "Scripts")
    if not out:
        raise PropertyParseError("no Script ranges parsed")
    return out


def parse_script_extensions(content: str) -> list[tuple[int, int, tuple[str, ...]]]:
    out: list[tuple[int, int, tuple[str, ...]]] = []
    for lineno, line in enumerate(content.split("\n"), start=1):
        s = line.strip()
        if not s or s.startswith("#"):
            continue
        parts = line.split(";")
        if len(parts) < 2:
            raise PropertyParseError(f"ScriptExtensions line {lineno}: malformed row")
        rng = parse_range_field(parts[0], lineno, "ScriptExtensions")
        # Second field: space-separated short codes, before '#'
        sc_field = parts[1].split("#")[0].strip().split()
        if not sc_field:
            raise PropertyParseError(f"ScriptExtensions line {lineno}: empty script list")
        for sc in sc_field:
            if not re.fullmatch(r"[A-Za-z]{4}", sc):
                raise PropertyParseError(
                    f"ScriptExtensions line {lineno}: malformed script code {sc!r}"
                )
        out.append((rng[0], rng[1], tuple(sc_field)))
    out.sort(key=lambda t: (t[0], t[1]))
    check_sorted_non_overlapping([(a, b) for a, b, _ in out], "ScriptExtensions")
    if not out:
        raise PropertyParseError("no ScriptExtensions entries parsed")
    return out


def parse_bidi_class(content: str) -> list[tuple[int, int, str]]:
    out: list[tuple[int, int, str]] = []
    for lineno, line in enumerate(content.split("\n"), start=1):
        s = line.strip()
        if not s or s.startswith("#"):
            continue
        parts = line.split(";")
        if len(parts) < 2:
            raise PropertyParseError(f"DerivedBidiClass line {lineno}: malformed row")
        rng = parse_range_field(parts[0], lineno, "DerivedBidiClass")
        bc = parts[1].split("#")[0].strip()
        if bc not in VALID_BIDI_CLASSES:
            raise PropertyParseError(f"DerivedBidiClass line {lineno}: unknown class {bc!r}")
        out.append((rng[0], rng[1], bc))
    out.sort(key=lambda t: (t[0], t[1]))
    check_sorted_non_overlapping([(a, b) for a, b, _ in out], "DerivedBidiClass")
    if not out:
        raise PropertyParseError("no Bidi_Class ranges parsed")
    return out


def parse_bc_aliases(content: str) -> dict[str, str]:
    """Map Bidi_Class long names (and short codes) to canonical short aliases.

    Parses PropertyValueAliases.txt `bc` rows: `bc ; Short ; Long`.
    Returns alias -> short. First alias wins; conflicts fail closed.
    """
    mapping: dict[str, str] = {}
    for lineno, line in enumerate(content.split("\n"), start=1):
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        if not stripped.startswith("bc "):
            continue
        parts = [p.strip() for p in line.split(";")]
        if len(parts) < 3:
            raise PropertyParseError(f"PropertyValueAliases line {lineno}: malformed bc row")
        short = parts[1]
        if short not in VALID_BIDI_CLASSES:
            raise PropertyParseError(
                f"PropertyValueAliases line {lineno}: unknown Bidi_Class short {short!r}"
            )
        mapping.setdefault(short, short)
        long_name = parts[2].split("#")[0].strip()
        if not long_name:
            raise PropertyParseError(f"PropertyValueAliases line {lineno}: empty bc long name")
        if long_name in mapping and mapping[long_name] != short:
            raise PropertyParseError(
                f"PropertyValueAliases line {lineno}: conflicting bc alias {long_name!r}"
            )
        mapping[long_name] = short
    for probe in ("Left_To_Right", "Right_To_Left", "Arabic_Letter", "European_Terminator"):
        if probe not in mapping:
            raise PropertyParseError(f"bc alias table missing {probe}")
    return mapping


def parse_bidi_class_missing(
    content: str, bc_aliases: dict[str, str]
) -> list[tuple[int, int, str]]:
    """Parse ordered Bidi_Class `@missing` defaults (UAX #44 machine-readable).

    Only lines matching the strict `@missing` directive syntax are directives;
    arbitrary comments merely containing the text `@missing` are ignored, while
    malformed `@missing:` directives fail closed. Source order is preserved
    because later directives override earlier ones; overlapping ranges are
    therefore expected and NOT rejected here.
    """
    out: list[tuple[int, int, str]] = []
    for lineno, line in enumerate(content.split("\n"), start=1):
        s = line.strip()
        if not s.startswith("#") or "@missing" not in s:
            continue
        if "@missing:" not in s:
            # Informational comment (e.g. "For details see the @missing lines
            # below"): not a directive, never parsed as one.
            continue
        m = MISSING_RE.match(s)
        if m is None:
            raise PropertyParseError(
                f"DerivedBidiClass line {lineno}: malformed @missing directive {s!r}"
            )
        lo = check_scalar(int(m.group(1), 16), f"DerivedBidiClass line {lineno} @missing")
        hi = check_scalar(int(m.group(2), 16), f"DerivedBidiClass line {lineno} @missing")
        if lo > hi:
            raise PropertyParseError(
                f"DerivedBidiClass line {lineno}: inverted @missing range {s!r}"
            )
        value = m.group(3)
        if value not in bc_aliases:
            raise PropertyParseError(
                f"DerivedBidiClass line {lineno}: unknown @missing Bidi_Class {value!r}"
            )
        short = bc_aliases[value]
        if short not in VALID_BIDI_CLASSES:
            raise PropertyParseError(
                f"DerivedBidiClass line {lineno}: @missing Bidi_Class {value!r} "
                f"normalizes to unknown short {short!r}"
            )
        out.append((lo, hi, short))
    if not out:
        raise PropertyParseError("no Bidi_Class @missing defaults parsed")
    if out[0] != (0x0000, 0x10FFFF, "L"):
        raise PropertyParseError(
            f"first Bidi_Class @missing directive must be the global L default, got {out[0]!r}"
        )
    return out


def parse_bidi_mirroring(content: str) -> list[tuple[int, int]]:
    out: list[tuple[int, int]] = []
    seen: set[int] = set()
    for lineno, line in enumerate(content.split("\n"), start=1):
        s = line.strip()
        if not s or s.startswith("#"):
            continue
        parts = line.split(";")
        if len(parts) < 2:
            raise PropertyParseError(f"BidiMirroring line {lineno}: malformed row")
        src_s = parts[0].strip()
        mir_s = parts[1].split("#")[0].strip()
        if not HEX_RE.fullmatch(src_s) or not HEX_RE.fullmatch(mir_s):
            raise PropertyParseError(f"BidiMirroring line {lineno}: malformed code points")
        src = check_scalar(int(src_s, 16), f"BidiMirroring line {lineno} source")
        mir = check_scalar(int(mir_s, 16), f"BidiMirroring line {lineno} mirror")
        if src in seen:
            raise PropertyParseError(f"BidiMirroring line {lineno}: duplicate source U+{src:04X}")
        seen.add(src)
        out.append((src, mir))
    out.sort()
    if not out:
        raise PropertyParseError("no Bidi_Mirroring entries parsed")
    return out


def parse_bidi_brackets(content: str) -> list[tuple[int, int, bool]]:
    out: list[tuple[int, int, bool]] = []
    seen: set[int] = set()
    for lineno, line in enumerate(content.split("\n"), start=1):
        s = line.strip()
        if not s or s.startswith("#"):
            continue
        parts = line.split(";")
        if len(parts) < 3:
            raise PropertyParseError(f"BidiBrackets line {lineno}: malformed row")
        code_s = parts[0].strip()
        pair_s = parts[1].strip()
        type_s = parts[2].split("#")[0].strip()
        if not HEX_RE.fullmatch(code_s) or not HEX_RE.fullmatch(pair_s):
            raise PropertyParseError(f"BidiBrackets line {lineno}: malformed code points")
        if type_s not in ("o", "c"):
            raise PropertyParseError(f"BidiBrackets line {lineno}: bad open/close {type_s!r}")
        code = check_scalar(int(code_s, 16), f"BidiBrackets line {lineno}")
        pair = check_scalar(int(pair_s, 16), f"BidiBrackets line {lineno} pair")
        if code in seen:
            raise PropertyParseError(f"BidiBrackets line {lineno}: duplicate U+{code:04X}")
        seen.add(code)
        out.append((code, pair, type_s == "o"))
    out.sort()
    if not out:
        raise PropertyParseError("no BidiBrackets entries parsed")
    return out


def render_rust(
    di: list[tuple[int, int]],
    scripts: list[tuple[int, int, str]],
    scx: list[tuple[int, int, tuple[str, ...]]],
    bidi: list[tuple[int, int, str]],
    bidi_defaults: list[tuple[int, int, str]],
    mirr: list[tuple[int, int]],
    brackets: list[tuple[int, int, bool]],
    short_to_long: dict[str, str],
    checksums: dict[str, str],
) -> str:
    header_lines = [
        "// Auto-generated Unicode 18.0.0 security property tables. DO NOT EDIT.",
        f"// Unicode version: {UNICODE_VERSION}",
    ]
    for name in SOURCES:
        url, _ = SOURCES[name]
        header_lines.append(f"// Source {name}: {url}")
        header_lines.append(f"// Source checksum (SHA-256) {name}: {checksums[name]}")
    header_lines.append(
        "// Generation command: python3 scripts/generate_unicode_security_properties.py"
    )
    header_lines.append(f"// Entry counts: default_ignorable={len(di)} ranges, "
                        f"scripts={len(scripts)} ranges, script_extensions={len(scx)} ranges, "
                        f"bidi_class={len(bidi)} ranges, bidi_class_defaults={len(bidi_defaults)} ranges, "
                        f"bidi_mirroring={len(mirr)} entries, "
                        f"bidi_brackets={len(brackets)} entries.")
    out = ["\n".join(header_lines), ""]
    out.append("/// Default_Ignorable_Code_Point ranges as (start, end) inclusive.")
    out.append("pub static DEFAULT_IGNORABLE_RANGES: &[(u32, u32)] = &[")
    for lo, hi in di:
        out.append(f"    (0x{lo:04X}, 0x{hi:04X}),")
    out.append("];\n")
    out.append("/// Script ranges as (start, end, short script code). Sorted, non-overlapping.")
    out.append("pub static SCRIPT_RANGES: &[(u32, u32, &str)] = &[")
    for lo, hi, sc in scripts:
        out.append(f"    (0x{lo:04X}, 0x{hi:04X}, \"{sc}\"),")
    out.append("];\n")
    out.append("/// Script_Extensions overrides as (start, end, space-separated short codes).")
    out.append("pub static SCRIPT_EXTENSION_OVERRIDES: &[(u32, u32, &str)] = &[")
    for lo, hi, scs in scx:
        out.append(f"    (0x{lo:04X}, 0x{hi:04X}, \"{' '.join(scs)}\"),")
    out.append("];\n")
    out.append("/// Bidi_Class ranges as (start, end, class). Sorted, non-overlapping.")
    out.append("pub static BIDI_CLASS_RANGES: &[(u32, u32, &str)] = &[")
    for lo, hi, bc in bidi:
        out.append(f"    (0x{lo:04X}, 0x{hi:04X}, \"{bc}\"),")
    out.append("];\n")
    out.append("/// Bidi_Class `@missing` defaults as (start, end, class) in UAX #44 source")
    out.append("/// order. Later directives override earlier ones: runtime lookup scans")
    out.append("/// this table in reverse after the explicit table misses.")
    out.append("pub static BIDI_CLASS_DEFAULT_RANGES: &[(u32, u32, &str)] = &[")
    for lo, hi, bc in bidi_defaults:
        out.append(f"    (0x{lo:04X}, 0x{hi:04X}, \"{bc}\"),")
    out.append("];\n")
    out.append("/// Bidi_Mirroring_Glyph entries as (source, mirror). Sorted by source.")
    out.append("pub static BIDI_MIRRORING_ENTRIES: &[(u32, u32)] = &[")
    for src, mir in mirr:
        out.append(f"    (0x{src:04X}, 0x{mir:04X}),")
    out.append("];\n")
    out.append("/// Bidi_Paired_Bracket entries as (code, pair, is_open). Sorted by code.")
    out.append("pub static BIDI_BRACKET_ENTRIES: &[(u32, u32, bool)] = &[")
    for code, pair, is_open in brackets:
        out.append(f"    (0x{code:04X}, 0x{pair:04X}, {'true' if is_open else 'false'}),")
    out.append("];\n")
    out.append("/// Canonical Script short-code to long-name mapping (from PropertyValueAliases).")
    out.append("pub static SCRIPT_SHORT_TO_LONG: &[(&str, &str)] = &[")
    for short in sorted(short_to_long):
        long_name = short_to_long[short].replace('"', "'")
        out.append(f"    (\"{short}\", \"{long_name}\"),")
    out.append("];")
    return "\n".join(out) + "\n"


def build_tables(raw_map: dict[str, bytes]):
    for name, raw in raw_map.items():
        verify_checksum(name, raw)
    texts = {n: raw_map[n].decode("utf-8") for n in raw_map}
    aliases, short_to_long = parse_sc_aliases(texts["PropertyValueAliases.txt"])
    bc_aliases = parse_bc_aliases(texts["PropertyValueAliases.txt"])
    di = parse_default_ignorables(texts["DerivedCoreProperties.txt"])
    scripts = parse_scripts(texts["Scripts.txt"], aliases)
    scx = parse_script_extensions(texts["ScriptExtensions.txt"])
    bidi = parse_bidi_class(texts["DerivedBidiClass.txt"])
    bidi_defaults = parse_bidi_class_missing(texts["DerivedBidiClass.txt"], bc_aliases)
    mirr = parse_bidi_mirroring(texts["BidiMirroring.txt"])
    brackets = parse_bidi_brackets(texts["BidiBrackets.txt"])
    checksums = {n: hashlib.sha256(raw_map[n]).hexdigest() for n in raw_map}
    rust = render_rust(di, scripts, scx, bidi, bidi_defaults, mirr, brackets, short_to_long, checksums)
    counts = {
        "default_ignorable": len(di),
        "scripts": len(scripts),
        "script_extensions": len(scx),
        "bidi_class": len(bidi),
        "bidi_class_defaults": len(bidi_defaults),
        "bidi_mirroring": len(mirr),
        "bidi_brackets": len(brackets),
    }
    return rust, counts


def run_check() -> int:
    raw_map = {n: fetch_bytes(n) for n in SOURCES}
    try:
        rust, counts = build_tables(raw_map)
    except PropertyParseError as e:
        print(f"ERROR: strict parse failure: {e}", file=sys.stderr)
        return 1
    if not OUTPUT_FILE.exists() or OUTPUT_FILE.read_text() != rust:
        print(
            f"ERROR: generated Unicode property output is stale: {OUTPUT_FILE}\n"
            f"Regenerate with: python3 scripts/generate_unicode_security_properties.py "
            f"({counts}, Unicode {UNICODE_VERSION})",
            file=sys.stderr,
        )
        return 1
    print(f"Unicode property outputs are fresh ({counts}, Unicode {UNICODE_VERSION})")
    return 0


def run_self_test() -> int:
    failures: list[str] = []

    def fail(msg: str) -> None:
        failures.append(msg)

    # Miniature parser fixtures (offline, no network).
    try:
        aliases, short_to_long = parse_sc_aliases(
            "# PropertyValueAliases\nsc ; Latn ; Latin\nsc ; Zyyy ; Common\n"
            "sc ; Zinh ; Inherited\nsc ; Hani ; Han\nsc ; Hira ; Hiragana\n"
            "sc ; Kana ; Katakana\nsc ; Hang ; Hangul\nsc ; Bopo ; Bopomofo\n"
        )
        if aliases.get("Latin") != "Latn" or aliases.get("Common") != "Zyyy":
            fail("sc alias miniature mismatch")
        if short_to_long.get("Latn") != "Latin":
            fail("sc short_to_long miniature mismatch")
    except PropertyParseError as e:
        fail(f"sc alias miniature unexpected failure: {e}")

    try:
        di = parse_default_ignorables(
            "# DerivedCoreProperties\n00AD ; Default_Ignorable_Code_Point # SOFT HYPHEN\n"
            "200B..200F ; Default_Ignorable_Code_Point # range\n"
        )
        if di != [(0x00AD, 0x00AD), (0x200B, 0x200F)]:
            fail(f"default-ignorable miniature mismatch: {di!r}")
    except PropertyParseError as e:
        fail(f"default-ignorable miniature unexpected failure: {e}")

    try:
        parse_default_ignorables("00AD ; Default_Ignorable_Code_Point\n00AD ; Default_Ignorable_Code_Point\n")
        # Overlapping duplicate ranges must fail.
        fail("default-ignorable duplicate overlap parsed without error")
    except PropertyParseError:
        pass

    try:
        sc = parse_scripts(
            "0041..005A ; Latin # ...\n00AA ; Latin # ...\n", aliases
        )
        if sc[0][2] != "Latn":
            fail(f"scripts miniature mismatch: {sc!r}")
    except PropertyParseError as e:
        fail(f"scripts miniature unexpected failure: {e}")

    try:
        parse_scripts("0041 ; UnknownScriptXYZ # ...\n", aliases)
        fail("scripts unknown script parsed without error")
    except PropertyParseError:
        pass

    try:
        scx = parse_script_extensions("00B7 ; Latn Grek Hani # ...\n02C9..02CB ; Bopo Latn # ...\n")
        if scx[0][2] != ("Latn", "Grek", "Hani"):
            fail(f"script-extensions miniature mismatch: {scx!r}")
    except PropertyParseError as e:
        fail(f"script-extensions miniature unexpected failure: {e}")

    try:
        bc = parse_bidi_class("0041..005A ; L # ...\n05D0..05EA ; R # ...\n")
        if bc[0][2] != "L":
            fail(f"bidi-class miniature mismatch: {bc!r}")
    except PropertyParseError as e:
        fail(f"bidi-class miniature unexpected failure: {e}")

    try:
        parse_bidi_class("0041 ; QQQ # ...\n")
        fail("bidi-class unknown class parsed without error")
    except PropertyParseError:
        pass

    try:
        bc_aliases = parse_bc_aliases(
            "# PropertyValueAliases\n"
            "bc ; L ; Left_To_Right\n"
            "bc ; R ; Right_To_Left\n"
            "bc ; AL ; Arabic_Letter\n"
            "bc ; ET ; European_Terminator\n"
        )
        if bc_aliases.get("Left_To_Right") != "L":
            fail("bc alias Left_To_Right miniature mismatch")
        if bc_aliases.get("Arabic_Letter") != "AL":
            fail("bc alias Arabic_Letter miniature mismatch")
        if bc_aliases.get("ET") != "ET":
            fail("bc alias short-code identity miniature mismatch")
    except PropertyParseError as e:
        fail(f"bc alias miniature unexpected failure: {e}")

    try:
        parse_bc_aliases("bc ; QQQ ; Bogus_Class\n")
        fail("bc alias unknown short parsed without error")
    except PropertyParseError:
        pass

    try:
        bc_aliases = parse_bc_aliases(
            "bc ; L ; Left_To_Right\nbc ; R ; Right_To_Left\n"
            "bc ; AL ; Arabic_Letter\nbc ; ET ; European_Terminator\n"
            "bc ; AL ; Arabic_Letter\nbc ; ET ; European_Terminator\n"
        )
        missing = parse_bidi_class_missing(
            "# DerivedBidiClass\n"
            "# For details see the @missing lines below.\n"
            "# @missing: 0000..10FFFF; Left_To_Right\n"
            "# @missing: 0590..05FF; Right_To_Left\n"
            "# @missing: 0600..07BF; Arabic_Letter\n"
            "0041..005A ; L # ...\n",
            bc_aliases,
        )
        if missing != [
            (0x0000, 0x10FFFF, "L"),
            (0x0590, 0x05FF, "R"),
            (0x0600, 0x07BF, "AL"),
        ]:
            fail(f"@missing miniature mismatch: {missing!r}")

        # Ordered override semantics: the last matching directive wins, and
        # explicit rows take precedence over every default.
        explicit = [(0x0041, 0x005A, "L")]

        def resolve(cp: int) -> str:
            for lo, hi, tag in explicit:
                if lo <= cp <= hi:
                    return tag
            for lo, hi, tag in reversed(missing):
                if lo <= cp <= hi:
                    return tag
            raise AssertionError(f"U+{cp:04X} uncovered")

        if resolve(0x0041) != "L":
            fail("explicit row must win over @missing defaults")
        if resolve(0x0590) != "R":
            fail("@missing R override did not apply")
        if resolve(0x0600) != "AL":
            fail("@missing AL override did not apply")
        if resolve(0x0378) != "L":
            fail("global L default did not apply")

        # Later directives override earlier ones on overlap.
        overlap = parse_bidi_class_missing(
            "# @missing: 0000..10FFFF; Left_To_Right\n"
            "# @missing: 0600..07BF; Arabic_Letter\n"
            "# @missing: 0700..074F; Right_To_Left\n",
            bc_aliases,
        )
        winner = next(tag for lo, hi, tag in reversed(overlap) if lo <= 0x0710 <= hi)
        if winner != "R":
            fail(f"later @missing default must win, got {winner!r}")
    except PropertyParseError as e:
        fail(f"@missing miniature unexpected failure: {e}")

    try:
        bc_aliases = parse_bc_aliases(
            "bc ; L ; Left_To_Right\nbc ; R ; Right_To_Left\n"
            "bc ; AL ; Arabic_Letter\nbc ; ET ; European_Terminator\n"
        )
        parse_bidi_class_missing("# @missing: 0590..05FF; Backwards_Class\n", bc_aliases)
        fail("@missing unknown value parsed without error")
    except PropertyParseError:
        pass

    try:
        bc_aliases = parse_bc_aliases(
            "bc ; L ; Left_To_Right\nbc ; R ; Right_To_Left\n"
            "bc ; AL ; Arabic_Letter\nbc ; ET ; European_Terminator\n"
        )
        parse_bidi_class_missing("# @missing: 05FF..0590; Right_To_Left\n", bc_aliases)
        fail("@missing inverted range parsed without error")
    except PropertyParseError:
        pass

    try:
        bc_aliases = parse_bc_aliases(
            "bc ; L ; Left_To_Right\nbc ; R ; Right_To_Left\n"
            "bc ; AL ; Arabic_Letter\nbc ; ET ; European_Terminator\n"
        )
        parse_bidi_class_missing("# @missing: not-a-range; Right_To_Left\n", bc_aliases)
        fail("@missing malformed directive parsed without error")
    except PropertyParseError:
        pass

    try:
        bc_aliases = parse_bc_aliases(
            "bc ; L ; Left_To_Right\nbc ; R ; Right_To_Left\n"
            "bc ; AL ; Arabic_Letter\nbc ; ET ; European_Terminator\n"
        )
        # A bare comment mentioning @missing is not a directive and must be
        # ignored rather than parsed or rejected.
        missing = parse_bidi_class_missing(
            "# For details see the @missing lines below.\n"
            "# @missing: 0000..10FFFF; Left_To_Right\n",
            bc_aliases,
        )
        if missing != [(0x0000, 0x10FFFF, "L")]:
            fail(f"informational @missing comment mishandled: {missing!r}")
    except PropertyParseError as e:
        fail(f"informational @missing comment unexpected failure: {e}")

    try:
        mirr = parse_bidi_mirroring("0028; 0029 # LEFT PARENTHESIS\n")
        if mirr != [(0x0028, 0x0029)]:
            fail(f"mirroring miniature mismatch: {mirr!r}")
    except PropertyParseError as e:
        fail(f"mirroring miniature unexpected failure: {e}")

    try:
        parse_bidi_mirroring("0028; 0029 # ...\n0028; 0029 # dup\n")
        fail("mirroring duplicate parsed without error")
    except PropertyParseError:
        pass

    try:
        br = parse_bidi_brackets("0028; 0029; o # ...\n0029; 0028; c # ...\n")
        if br[0] != (0x0028, 0x0029, True):
            fail(f"brackets miniature mismatch: {br!r}")
    except PropertyParseError as e:
        fail(f"brackets miniature unexpected failure: {e}")

    try:
        parse_range_field("D800", 1, "test")
        fail("surrogate parsed without error")
    except PropertyParseError:
        pass

    try:
        parse_range_field("005B..0041", 1, "test")
        fail("inverted range parsed without error")
    except PropertyParseError:
        pass

    if failures:
        print("self-test FAILURES:", file=sys.stderr)
        for f in failures:
            print(f"  - {f}", file=sys.stderr)
        return 1
    print("generator self-test: all strict-parser fixtures pass")
    return 0


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="freshness check (fetch, compare, write nothing)")
    parser.add_argument("--self-test", action="store_true", help="offline parser fixtures (no network)")
    args = parser.parse_args()

    if args.self_test:
        sys.exit(run_self_test())
    if args.check:
        sys.exit(run_check())

    OUTPUT_FILE.parent.mkdir(parents=True, exist_ok=True)
    raw_map = {n: fetch_bytes(n) for n in SOURCES}
    print(f"Downloaded {sum(len(v) for v in raw_map.values())} bytes total")
    try:
        rust, counts = build_tables(raw_map)
    except PropertyParseError as e:
        print(f"ERROR: strict parse failure: {e}", file=sys.stderr)
        sys.exit(1)
    OUTPUT_FILE.write_text(rust)
    print(f"Wrote {OUTPUT_FILE}")
    print(f"Counts {counts}, Unicode {UNICODE_VERSION}")


if __name__ == "__main__":
    main()
