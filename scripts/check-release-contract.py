#!/usr/bin/env python3
"""Keep the published target/asset matrix aligned across release surfaces.

Eggpack is producer authority since Ecosystem M001: the canonical
target set, asset names, and checksum sidecars come from
`release/eggpack/distribution.toml`, and the checked-in release workflow
is generated from that configuration (drift-guarded by CI running
`eggpack ci check` at the pinned tool revision). This script therefore
compares product mappings against the Eggpack configuration instead of
duplicating producer facts, and retains only eggsact-owned invariants.
"""
from pathlib import Path
import json
import re
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]
EGG = ROOT / "release/eggpack"

# Frozen public asset names. Sourced from the Eggpack contract below and
# compared against this table so a rename can never slip through silently.
PUBLIC_ASSETS = {
    "x86_64-unknown-linux-gnu": "eggsact-x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu": "eggsact-aarch64-unknown-linux-gnu",
    "x86_64-apple-darwin": "eggsact-x86_64-apple-darwin",
    "aarch64-apple-darwin": "eggsact-aarch64-apple-darwin",
    "x86_64-pc-windows-msvc": "eggsact-x86_64-pc-windows-msvc.exe",
}

with open(EGG / "distribution.toml", "rb") as handle:
    contract = tomllib.load(handle)
product = contract["product"]["id"]
contracted = {}
for target in contract["targets"]:
    triple = target["triple"]
    asset = target["asset"]["asset"].replace("{product}", product).replace("{target}", triple)
    contracted[triple] = asset

workflow = (ROOT / ".github/workflows/release-binaries.yml").read_text()
installer = (ROOT / "packaging/install.sh").read_text()
powershell = (ROOT / "packaging/install.ps1").read_text()
readme = (ROOT / "README.md").read_text()
installation = (ROOT / "docs/installation.md").read_text()
update_rs = (ROOT / "src/update.rs").read_text()
cargo_toml = (ROOT / "Cargo.toml").read_text()
policy = json.loads((EGG / "github-policy.json").read_text())
pack = (EGG / "pack.toml").read_text()
qual_bindings = (EGG / "qualification-bindings.toml").read_text()
validators = json.loads((EGG / "consumer-validators.json").read_text())
errors = []

# Producer mapping: contract expansion must equal the frozen public names.
if contracted != PUBLIC_ASSETS:
    errors.append(f"Eggpack contract asset expansion differs from public names: {contracted}")

for target in PUBLIC_ASSETS:
    if target not in workflow:
        errors.append(f"generated workflow does not mention target {target}")

# Exactly one write-authorized job (the staging job); nothing else may write.
if workflow.count("contents: write") != 1:
    errors.append("generated workflow must grant contents: write to exactly one job")

# Initial adoption is manual-dispatch only with the exact existing tag.
if "release_tag" not in workflow:
    errors.append("generated workflow must accept the exact release_tag dispatch input")
if re.search(r"(?m)^\s*push\s*:", workflow):
    errors.append("generated workflow must not trigger on push during initial adoption")

# No legacy clobber path: no clobber uploads, no publication, no tag mutation.
for forbidden in ["--clobber", "gh release publish", "gh release create --latest",
                  "git tag ", "git push origin --tags", "release publish"]:
    if forbidden in workflow:
        errors.append(f"generated workflow must not contain {forbidden!r}")
if "apt-get install" in workflow or "apt install zig" in workflow:
    errors.append("generated workflow must not install Zig through apt")

# Immutable tool pin: full commit SHA, never a floating branch or tag.
revision = policy.get("eggpack_tool", {}).get("revision", "")
if not re.fullmatch(r"[0-9a-f]{40}", revision):
    errors.append("eggpack_tool.revision must be an exact 40-hex commit revision")

# Cross-toolchain parity with the legacy floor.
for pinned in ['zig = "0.14.1"', 'cargo_zigbuild = "0.23.3"']:
    if pinned not in pack:
        errors.append(f"pack.toml must pin the legacy cross toolchain: {pinned}")

# Every published target is qualified and consumer-validated.
for target in PUBLIC_ASSETS:
    if f'[targets."{target}".smoke]' not in qual_bindings:
        errors.append(f"qualification bindings lack a smoke for {target}")
    if target not in validators:
        errors.append(f"consumer validators lack an entry for {target}")

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
if "set -euo pipefail" in installer and installer.index("set -euo pipefail") < installer.index("BASH_VERSION"):
    errors.append("Unix installer enables Bash-only options before its Bash guard")
if '"${BASH##*/}" = "sh"' not in installer:
    errors.append("Unix installer must reject Bash invoked through sh")
for fragment in ["-split ';'", "GetEnvironmentVariable(\"Path\"", "if ($arch -eq \"X64\")", "if (-not $candidate)"]:
    if fragment not in powershell:
        errors.append(f"PowerShell installer missing contract fragment {fragment}")
for document, name in [(readme, "README"), (installation, "installation docs")]:
    for installer_name in ["install.sh", "install.ps1"]:
        if f"releases/latest/download/{installer_name}" not in document:
            errors.append(f"{name} must advertise the published latest {installer_name} URL")
if "v1.2.3/install.sh" in installation:
    errors.append("installation docs must not use the pre-binary v1.2.3 installer example")
# Self-update must stay self-contained: no external curl process after install.
# Narrow to the updater transport only; bootstrap installers/docs legitimately use curl.
if 'Command::new("curl")' in update_rs:
    errors.append("src/update.rs must not spawn curl for self-update networking")
if "eggfetch-core" not in cargo_toml:
    errors.append("Cargo.toml must declare the qualified eggfetch-core updater transport")
if errors:
    print("release contract errors:", file=sys.stderr)
    print("\n".join(f"- {error}" for error in errors), file=sys.stderr)
    sys.exit(1)
print("release target/asset contract passed")
