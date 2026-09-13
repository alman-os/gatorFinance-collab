#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_NAME="${APP_NAME:-gatorFinance}"
NOTARY_PROFILE="${NOTARY_PROFILE:-AudioGrabberNotary}"
APP_PATH="$ROOT_DIR/src-tauri/target/universal-apple-darwin/release/bundle/macos/$APP_NAME.app"
NOTARY_DIR="$ROOT_DIR/build/notary"
APP_ZIP="$NOTARY_DIR/$APP_NAME.zip"

cd "$ROOT_DIR"

# Rust panic metadata can otherwise expose the local builder's home directory in
# the release binary. Keep useful source locations while removing that username.
export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }--remap-path-prefix=$HOME=/Users/builder"

echo "Checking notarization profile..."
xcrun notarytool history \
  --keychain-profile "$NOTARY_PROFILE" \
  --output-format json >/dev/null

echo "Building universal macOS app..."
pnpm tauri build --bundles app --target universal-apple-darwin

if [[ ! -d "$APP_PATH" ]]; then
  echo "Universal app bundle was not produced at $APP_PATH" >&2
  exit 1
fi

echo "Verifying universal binary and signature..."
lipo "$APP_PATH/Contents/MacOS/gatorfinance" -verify_arch x86_64 arm64
codesign --verify --deep --strict --verbose=2 "$APP_PATH"

rm -rf "$NOTARY_DIR"
mkdir -p "$NOTARY_DIR"
ditto -c -k --keepParent "$APP_PATH" "$APP_ZIP"

echo "Submitting app bundle for notarization..."
xcrun notarytool submit "$APP_ZIP" \
  --keychain-profile "$NOTARY_PROFILE" \
  --wait \
  --output-format json | tee "$NOTARY_DIR/notarytool-app.json"

echo "Stapling and validating app bundle..."
xcrun stapler staple "$APP_PATH"
xcrun stapler validate "$APP_PATH"
spctl -a -vvv -t exec "$APP_PATH"

APP_SOURCE="$APP_PATH" "$ROOT_DIR/scripts/package-notarized-dmg.sh"
