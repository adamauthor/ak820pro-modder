#!/bin/sh
# Build, sign and install AK820 Pro Modder into /Applications.
#
# macOS ties Accessibility to the code signature. An ad-hoc signature changes
# with every build, so the permission is lost after each update. Signing with
# a certificate (an "Apple Development" one is enough) keeps it:
#   AK820_SIGN_IDENTITY="Apple Development: you@example.com (TEAMID)" scripts/install-mac.sh
# `security find-identity -p codesigning -v` lists the available identities.
set -eu
cd "$(dirname "$0")/.."
identity="${AK820_SIGN_IDENTITY:--}"
app="AK820 Pro Modder.app"
target="${CARGO_TARGET_DIR:-target}"

pnpm install --frozen-lockfile
pnpm tauri build --bundles app
built="$target/release/bundle/macos/$app"
codesign --force --deep --sign "$identity" "$built"
codesign --verify --deep "$built"

# Quit through the app so it restores the original knob mappings first.
osascript -e 'tell application "AK820 Pro Modder" to quit' 2>/dev/null || true
i=0
while pgrep -f "/Applications/$app/Contents/MacOS/ak820pro-app\$" >/dev/null && [ $i -lt 20 ]; do
  sleep 0.5; i=$((i + 1))
done
rm -rf "/Applications/$app"
ditto "$built" "/Applications/$app"
open "/Applications/$app"
if [ "$identity" = "-" ]; then
  echo "Installed with an ad-hoc signature: re-add the app under Accessibility after this update."
fi
