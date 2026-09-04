#!/usr/bin/env python3
"""Keep the published target/asset matrix aligned across release surfaces."""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
targets = {
    "x86_64-unknown-linux-gnu": "eggsact-x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu": "eggsact-aarch64-unknown-linux-gnu",
    "x86_64-apple-darwin": "eggsact-x86_64-apple-darwin",
    "aarch64-apple-darwin": "eggsact-aarch64-apple-darwin",
    "x86_64-pc-windows-msvc": "eggsact-x86_64-pc-windows-msvc.exe",
}
workflow = (ROOT / ".github/workflows/release-binaries.yml").read_text()
installer = (ROOT / "packaging/install.sh").read_text()
powershell = (ROOT / "packaging/install.ps1").read_text()
errors = []
for target, asset in targets.items():
    if target not in workflow:
        errors.append(f"workflow does not mention target {target}")
    if asset not in workflow:
        errors.append(f"workflow does not stage asset {asset}")
for fragment in [
    "Linux:x86_64|Linux:amd64", "Linux:aarch64|Linux:arm64", "Linux:armv7l",
    "Darwin:x86_64", "Darwin:arm64", "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu", "armv7-unknown-linux-gnueabihf",
    "x86_64-apple-darwin", "aarch64-apple-darwin",
]:
    if fragment not in installer:
        errors.append(f"Unix installer missing mapping fragment {fragment}")
if "x86_64-pc-windows-msvc.exe" not in powershell:
    errors.append("PowerShell installer missing Windows asset")
if "armv7-unknown-linux-gnueabihf" not in workflow and "armv7-unknown-linux-gnueabihf" not in installer:
    errors.append("ARMv7 must be recognized by the Unix installer")
if "contents: write" not in workflow:
    errors.append("release workflow must explicitly grant contents: write")
if errors:
    print("release contract errors:", file=sys.stderr)
    print("\n".join(f"- {error}" for error in errors), file=sys.stderr)
    sys.exit(1)
print("release target/asset contract passed")
