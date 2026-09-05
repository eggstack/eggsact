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
readme = (ROOT / "README.md").read_text()
installation = (ROOT / "docs/installation.md").read_text()
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
if "apt-get install" in workflow or "apt install zig" in workflow:
    errors.append("release workflow must not install Zig through apt")
for fragment in [
    "ubuntu-24.04-arm", "zig_version=0.14.1", "cargo_zigbuild_version=0.23.3",
    "sha256sum --check --status", "shasum -a 256", "smoke_arch: aarch64",
    "must build and smoke on", "aarch64:arm64",
    "--user-agent 'eggsact-release-preflight/1'",
]:
    if fragment not in workflow:
        errors.append(f"release workflow missing reproducible/native smoke guard: {fragment}")
for fragment in [
    'zig_dir="$RUNNER_TEMP/zig"',
    'mkdir -p "$zig_dir"',
    'tar -xJf "$RUNNER_TEMP/$zig_archive" -C "$zig_dir" --strip-components=1',
    'echo "$zig_dir" >> "$GITHUB_PATH"',
    '"$zig_dir/zig" version',
]:
    if fragment not in workflow:
        errors.append(f"release workflow missing deterministic Zig extraction contract: {fragment}")
if '"$RUNNER_TEMP/zig-${zig_version}"' in workflow:
    errors.append("release workflow must not assume Zig's archive directory name")
if "if: runner.os == 'Windows'" not in workflow:
    errors.append("release workflow is missing the Windows architecture smoke guard")
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
if errors:
    print("release contract errors:", file=sys.stderr)
    print("\n".join(f"- {error}" for error in errors), file=sys.stderr)
    sys.exit(1)
print("release target/asset contract passed")
