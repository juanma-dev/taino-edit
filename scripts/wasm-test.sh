#!/usr/bin/env bash
# Run wasm-bindgen browser tests in headless Chromium 148.
#
# First-time setup:
#   1. ./scripts/install-wasm-test-runner.sh  # patched wasm-bindgen-cli
#   2. ./scripts/wasm-test.sh                  # bootstraps .tools/ on first run
#
# Subsequent runs: just `./scripts/wasm-test.sh [extra cargo-test args]`.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TOOLS="$REPO_ROOT/.tools"
CHROME_DIR="$TOOLS/chrome-linux64"
CD_DIR="$TOOLS/chromedriver-linux64"
WEBDRIVER_JSON="$REPO_ROOT/webdriver.json"

CFT_VERSION="${CFT_VERSION:-148.0.7778.178}"
CFT_BASE="https://storage.googleapis.com/chrome-for-testing-public/$CFT_VERSION/linux64"

bootstrap_tools() {
  echo "bootstrapping $TOOLS (Chrome for Testing $CFT_VERSION + chromedriver)..."
  mkdir -p "$TOOLS"
  curl -sSL -o "$TOOLS/chrome.zip" "$CFT_BASE/chrome-linux64.zip"
  curl -sSL -o "$TOOLS/cd.zip" "$CFT_BASE/chromedriver-linux64.zip"
  python3 -m zipfile -e "$TOOLS/chrome.zip" "$TOOLS"
  python3 -m zipfile -e "$TOOLS/cd.zip" "$TOOLS"
  # python3 -m zipfile loses executable bits; restore them on every ELF.
  find "$CHROME_DIR" -maxdepth 1 -type f -exec sh -c \
    'file "$1" 2>/dev/null | grep -q "ELF.*executable" && chmod +x "$1"' _ {} \;
  chmod +x "$CD_DIR/chromedriver"
  rm -f "$TOOLS/chrome.zip" "$TOOLS/cd.zip"
}

ensure_webdriver_json() {
  if [ ! -f "$WEBDRIVER_JSON" ]; then
    echo "writing default $WEBDRIVER_JSON..."
    cat > "$WEBDRIVER_JSON" <<EOF
{
  "goog:chromeOptions": {
    "binary": "$CHROME_DIR/chrome",
    "args": ["--headless=new", "--no-sandbox", "--disable-dev-shm-usage", "--disable-gpu"]
  }
}
EOF
  fi
}

if [ ! -x "$CHROME_DIR/chrome" ] || [ ! -x "$CD_DIR/chromedriver" ]; then
  bootstrap_tools
fi
ensure_webdriver_json

# Default to the dom crate; pass through any other args (e.g. --test focus).
pkill -9 chromedriver chrome 2>/dev/null || true

if [ "$#" -eq 0 ]; then
  set -- -p taino-edit-dom
fi

CHROMEDRIVER="$CD_DIR/chromedriver" \
CHROMEDRIVER_ARGS="--allowed-origins=*" \
WASM_BINDGEN_TEST_WEBDRIVER_JSON="$WEBDRIVER_JSON" \
  cargo test --target wasm32-unknown-unknown "$@"
