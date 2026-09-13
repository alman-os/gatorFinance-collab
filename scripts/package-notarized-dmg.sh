#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

APP_NAME="${APP_NAME:-gatorFinance}"
VOL_NAME="${VOL_NAME:-$APP_NAME}"
SIGNING_IDENTITY="${SIGNING_IDENTITY:-Developer ID Application: ALMAN GONZALEZ (CNFQ3Q4EK2)}"
NOTARY_PROFILE="${NOTARY_PROFILE:-AudioGrabberNotary}"

APP_SOURCE="${APP_SOURCE:-$ROOT_DIR/src-tauri/target/universal-apple-darwin/release/bundle/macos/$APP_NAME.app}"
BACKGROUND_SOURCE="${BACKGROUND_SOURCE:-$ROOT_DIR/wrap_folder/gatorFinance_window.png}"
DIST_DIR="${DIST_DIR:-$ROOT_DIR/dist}"
WORK_DIR="$ROOT_DIR/build/dmg-wrap"
STAGE_DIR="$WORK_DIR/stage"
MOUNT_DIR="$WORK_DIR/mount"

WINDOW_WIDTH=600
WINDOW_HEIGHT=400
WINDOW_POS_X=200
WINDOW_POS_Y=120
ICON_SIZE=100
APP_ICON_X=175
APP_ICON_Y=250
APPLICATIONS_X=425
APPLICATIONS_Y=250
TEXT_SIZE=14

if [[ ! -d "$APP_SOURCE" ]]; then
  echo "Missing app bundle: $APP_SOURCE" >&2
  exit 1
fi

if [[ ! -f "$BACKGROUND_SOURCE" ]]; then
  echo "Missing DMG background: $BACKGROUND_SOURCE" >&2
  exit 1
fi

for command in create-dmg codesign hdiutil spctl xcrun; do
  command -v "$command" >/dev/null || { echo "Missing required command: $command" >&2; exit 1; }
done

read -r bg_width bg_height < <(
  sips -g pixelWidth -g pixelHeight "$BACKGROUND_SOURCE" |
    awk '/pixelWidth/ {w=$2} /pixelHeight/ {h=$2} END {print w, h}'
)
if [[ "$bg_width" != "1200" || "$bg_height" != "800" ]]; then
  echo "DMG background must be 1200x800; found ${bg_width}x${bg_height}." >&2
  exit 1
fi

VERSION="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$APP_SOURCE/Contents/Info.plist")"
FINAL_DMG="$DIST_DIR/${APP_NAME}_${VERSION}_macOS-universal_notarized.dmg"
TMP_DMG="$WORK_DIR/${APP_NAME}_${VERSION}_macOS-universal_notarized.dmg"
NOTARY_LOG="$WORK_DIR/notarytool-dmg.json"

cleanup() {
  if mount | grep -q " on $MOUNT_DIR "; then
    hdiutil detach "$MOUNT_DIR" -quiet || true
  fi
}
trap cleanup EXIT

rm -rf "$STAGE_DIR" "$MOUNT_DIR"
mkdir -p "$STAGE_DIR" "$MOUNT_DIR" "$DIST_DIR"
ditto "$APP_SOURCE" "$STAGE_DIR/$APP_NAME.app"

echo "Verifying notarized app before packaging..."
codesign --verify --deep --strict --verbose=2 "$STAGE_DIR/$APP_NAME.app"
xcrun stapler validate "$STAGE_DIR/$APP_NAME.app"
spctl -a -vvv -t exec "$STAGE_DIR/$APP_NAME.app"

rm -f "$TMP_DMG" "$FINAL_DMG"
echo "Creating signed DMG with a ${WINDOW_WIDTH}x${WINDOW_HEIGHT} Finder window..."
create-dmg \
  --volname "$VOL_NAME" \
  --background "$BACKGROUND_SOURCE" \
  --window-pos "$WINDOW_POS_X" "$WINDOW_POS_Y" \
  --window-size "$WINDOW_WIDTH" "$WINDOW_HEIGHT" \
  --icon-size "$ICON_SIZE" \
  --text-size "$TEXT_SIZE" \
  --icon "$APP_NAME.app" "$APP_ICON_X" "$APP_ICON_Y" \
  --hide-extension "$APP_NAME.app" \
  --app-drop-link "$APPLICATIONS_X" "$APPLICATIONS_Y" \
  --no-internet-enable \
  --format UDZO \
  --codesign "$SIGNING_IDENTITY" \
  "$TMP_DMG" \
  "$STAGE_DIR"

echo "Submitting DMG for notarization..."
xcrun notarytool submit "$TMP_DMG" \
  --keychain-profile "$NOTARY_PROFILE" \
  --wait \
  --output-format json | tee "$NOTARY_LOG"

echo "Stapling and validating DMG..."
xcrun stapler staple "$TMP_DMG"
xcrun stapler validate "$TMP_DMG"
hdiutil verify "$TMP_DMG"
ditto "$TMP_DMG" "$FINAL_DMG"

hdiutil attach "$FINAL_DMG" -nobrowse -readonly -mountpoint "$MOUNT_DIR" >/dev/null
echo "Verifying mounted app with Gatekeeper..."
spctl -a -vvv -t exec "$MOUNT_DIR/$APP_NAME.app"
hdiutil detach "$MOUNT_DIR" -quiet

echo "Verifying final DMG with Gatekeeper..."
spctl -a -vvv -t install "$FINAL_DMG"
hdiutil verify "$FINAL_DMG"
(cd "$DIST_DIR" && shasum -a 256 "$(basename "$FINAL_DMG")") | tee "$FINAL_DMG.sha256"

echo "Packaged notarized DMG: $FINAL_DMG"
