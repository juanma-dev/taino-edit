#!/usr/bin/env bash
# Fetch wasm-bindgen-cli 0.2.121 from crates.io, apply the WSL/W3C/HTTP
# patches in vendor/, and install the patched binaries to ~/.cargo/bin.
# Required because upstream 0.2.121 doesn't speak strict W3C ChromeDriver
# and trips an HTTP-layer reset on this environment; see vendor/README.md.

set -euo pipefail

VERSION="0.2.121"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PATCH="$REPO_ROOT/vendor/wasm-bindgen-cli-w3c-$VERSION.patch"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

if [ ! -f "$PATCH" ]; then
  echo "patch not found at $PATCH" >&2
  exit 1
fi

echo "fetching wasm-bindgen-cli $VERSION from crates.io..."
curl -sSL -o "$WORK/cli.crate" \
  "https://crates.io/api/v1/crates/wasm-bindgen-cli/$VERSION/download"

mkdir -p "$WORK/src"
tar -xzf "$WORK/cli.crate" -C "$WORK/src"
CRATE_DIR="$WORK/src/wasm-bindgen-cli-$VERSION"

echo "applying patch..."
( cd "$CRATE_DIR" && patch -p1 < "$PATCH" )

echo "cargo install --path $CRATE_DIR --force --bins ..."
cargo install --path "$CRATE_DIR" --force --bins

echo
echo "done. patched wasm-bindgen / wasm-bindgen-test-runner / wasm2es6js"
echo "are now in ~/.cargo/bin and override the upstream binaries."
