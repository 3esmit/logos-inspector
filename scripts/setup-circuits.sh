#!/usr/bin/env bash
set -euo pipefail

version="${1:-v0.4.2}"
install_dir="${2:-$HOME/.logos-blockchain-circuits}"
repo="logos-blockchain/logos-blockchain-circuits"

case "$(uname -s)" in
  Linux*) os="linux" ;;
  Darwin*) os="macos" ;;
  MINGW*|MSYS*|CYGWIN*) os="windows" ;;
  *) echo "unsupported OS: $(uname -s)" >&2; exit 1 ;;
esac

case "$(uname -m)" in
  x86_64) arch="x86_64" ;;
  aarch64|arm64) arch="aarch64" ;;
  *) echo "unsupported architecture: $(uname -m)" >&2; exit 1 ;;
esac

artifact="logos-blockchain-circuits-${version}-${os}-${arch}.tar.gz"
url="https://github.com/${repo}/releases/download/${version}/${artifact}"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

echo "downloading ${url}"
curl -fsSL "$url" -o "${tmp_dir}/${artifact}"

rm -rf "$install_dir"
mkdir -p "$install_dir"
tar -xzf "${tmp_dir}/${artifact}" -C "$install_dir" --strip-components=1

echo "installed ${version} at ${install_dir}"
echo "export LOGOS_BLOCKCHAIN_CIRCUITS=${install_dir}"

