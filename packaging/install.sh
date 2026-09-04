#!/usr/bin/env bash
# Download and install a verified Eggsact release binary.
set -euo pipefail

if [[ -z "${BASH_VERSION:-}" ]]; then
  echo "This installer requires Bash; run it with: bash install.sh" >&2
  exit 2
fi

readonly REPOSITORY="eggstack/eggsact"
readonly BASE_URL="https://github.com/${REPOSITORY}/releases"
requested_version=""

usage() { echo "Usage: install.sh [--version X.Y.Z]"; }

while (($#)); do
  case "$1" in
    --version)
      [[ $# -ge 2 ]] || { echo "--version requires X.Y.Z" >&2; exit 2; }
      requested_version="$2"
      shift 2
      ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done

if [[ -n "$requested_version" && ! "$requested_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "invalid version '$requested_version'; expected X.Y.Z" >&2
  exit 2
fi

command -v curl >/dev/null 2>&1 || { echo "curl is required" >&2; exit 1; }
os="$(uname -s)"
arch="$(uname -m)"
case "${os}:${arch}" in
  Linux:x86_64|Linux:amd64) target="x86_64-unknown-linux-gnu" ;;
  Linux:aarch64|Linux:arm64) target="aarch64-unknown-linux-gnu" ;;
  Linux:armv7l) target="armv7-unknown-linux-gnueabihf" ;;
  Darwin:x86_64) target="x86_64-apple-darwin" ;;
  Darwin:arm64) target="aarch64-apple-darwin" ;;
  *) target="" ;;
esac

tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/eggsact-install.XXXXXX")"
trap 'rm -rf "$tmp_dir"' EXIT

download() {
  local url="$1" destination="$2" result rc status
  if result="$(curl --proto '=https' --tlsv1.2 --silent --show-error --location \
      --connect-timeout 10 --max-time 120 --output "$destination" \
      --write-out $'\n%{http_code}' "$url" 2>"$tmp_dir/curl.stderr")"; then
    rc=0
  else
    rc=$?
  fi
  status="${result##*$'\n'}"
  if [[ "$status" == "404" ]]; then return 44; fi
  if ((rc != 0)) || [[ ! "$status" =~ ^2[0-9][0-9]$ ]]; then
    cat "$tmp_dir/curl.stderr" >&2 || true
    echo "download failed (HTTP ${status:-unknown}) for $url" >&2
    return 1
  fi
}

install_candidate() {
  local candidate="$1" destination
  if ((EUID == 0)); then destination="/usr/local/bin/eggsact"; else destination="${HOME:?}/.local/bin/eggsact"; fi
  mkdir -p "$(dirname "$destination")"
  install -m 0755 "$candidate" "$destination"
  echo "Installed $destination"
  case ":${PATH}:" in
    *:"$(dirname "$destination")":*) ;;
    *) echo "Add $(dirname "$destination") to PATH to run eggsact directly." ;;
  esac
}

validate_candidate() {
  local candidate="$1" output
  chmod 0755 "$candidate"
  output="$($candidate --version)" || { echo "candidate --version failed" >&2; return 1; }
  if [[ -n "$requested_version" ]]; then
    [[ "$output" == "eggsact $requested_version" ]] || { echo "candidate version '$output' does not match requested $requested_version" >&2; return 1; }
  elif [[ ! "$output" =~ ^eggsact\ [0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "candidate reported unexpected version '$output'" >&2
    return 1
  fi
}

cargo_fallback() {
  command -v cargo >/dev/null 2>&1 || {
    echo "No published binary is available for ${os}/${arch}${target:+ ($target)}, and Cargo is not installed." >&2
    echo "Install Rust from https://rustup.rs/ or build Eggsact from source." >&2
    exit 1
  }
  local cargo_root="$tmp_dir/cargo-root" candidate
  local -a args=(install eggsact --locked --root "$cargo_root")
  [[ -n "$requested_version" ]] && args+=(--version "=$requested_version")
  echo "Installing with Cargo fallback..."
  cargo "${args[@]}"
  candidate="$cargo_root/bin/eggsact"
  [[ -x "$candidate" ]] || { echo "Cargo did not produce $candidate" >&2; exit 1; }
  validate_candidate "$candidate"
  install_candidate "$candidate"
}

if [[ -z "$target" ]]; then cargo_fallback; exit 0; fi
if [[ -n "$requested_version" ]]; then release_path="download/v${requested_version}"; else release_path="latest/download"; fi
binary_name="eggsact-${target}"
binary_path="$tmp_dir/$binary_name"
binary_url="${BASE_URL}/${release_path}/${binary_name}"
if download "$binary_url" "$binary_path"; then
  :
else
  status=$?
  if ((status == 44)); then cargo_fallback; exit 0; fi
  exit "$status"
fi

checksum_path="$tmp_dir/$binary_name.sha256"
download "$binary_url.sha256" "$checksum_path" || {
  echo "The binary exists but its checksum sidecar could not be downloaded; refusing to install." >&2
  exit 1
}
expected_hash="$(awk 'NF { print $1; exit }' "$checksum_path")"
[[ "$expected_hash" =~ ^[[:xdigit:]]{64}$ ]] || { echo "checksum sidecar does not contain a 64-hex SHA-256 digest" >&2; exit 1; }
if command -v sha256sum >/dev/null 2>&1; then
  actual_hash="$(sha256sum "$binary_path" | awk '{print $1}')"
else
  command -v shasum >/dev/null 2>&1 || { echo "sha256sum or shasum is required" >&2; exit 1; }
  actual_hash="$(shasum -a 256 "$binary_path" | awk '{print $1}')"
fi
[[ "${actual_hash,,}" == "${expected_hash,,}" ]] || { echo "checksum mismatch; refusing to install" >&2; exit 1; }
validate_candidate "$binary_path"
install_candidate "$binary_path"
