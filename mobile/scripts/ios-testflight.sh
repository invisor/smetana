#!/usr/bin/env bash
# Builds the iOS companion, signs it for distribution and uploads it to
# TestFlight's internal testing. `npm run testflight` from mobile/.
#
# Signing is automatic: tauri hands the App Store Connect API key to xcodebuild
# with -allowProvisioningUpdates, which creates the certificate and profiles
# itself. Nobody signs in to Xcode and nothing is made by hand.
#
# The key lives outside the repository, in $SMETANA_SECRETS (by default
# ~/Desktop/Projects/smetana-secrets): asc-api.p8, and asc-api.txt with the Key
# ID on its first line and the Issuer ID on its second.
set -euo pipefail

mobile="$(cd "$(dirname "$0")/.." && pwd)"
apple="$mobile/src-tauri/gen/apple"
secrets="${SMETANA_SECRETS:-$HOME/Desktop/Projects/smetana-secrets}"

die() { echo "ios-testflight: $*" >&2; exit 1; }

key_path="$secrets/asc-api.p8"
ids="$secrets/asc-api.txt"
[ -f "$key_path" ] || die "missing $key_path (the App Store Connect API key)"
[ -f "$ids" ] || die "missing $ids (line 1 the Key ID, line 2 the Issuer ID)"
key_id="$(sed -n 1p "$ids" | tr -d '[:space:]')"
issuer="$(sed -n 2p "$ids" | tr -d '[:space:]')"
[ -n "$key_id" ] && [ -n "$issuer" ] || die "$ids must hold the Key ID on line 1 and the Issuer ID on line 2"

# xcode-select may point at the Command Line Tools, which carry no iOS SDK.
# tauri runs xcodebuild with an explicit environment that drops DEVELOPER_DIR,
# so a wrapper first on PATH is what carries the choice through.
if [ -z "${DEVELOPER_DIR:-}" ]; then
  DEVELOPER_DIR="$(xcode-select -p)"
  case "$DEVELOPER_DIR" in
    */CommandLineTools) DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer ;;
  esac
fi
[ -x "$DEVELOPER_DIR/usr/bin/xcodebuild" ] || die "no Xcode at $DEVELOPER_DIR (set DEVELOPER_DIR)"
export DEVELOPER_DIR

scratch="$(mktemp -d)"
mkdir "$scratch/bin"
cat >"$scratch/bin/xcodebuild" <<EOF
#!/bin/sh
DEVELOPER_DIR='$DEVELOPER_DIR' exec '$DEVELOPER_DIR/usr/bin/xcodebuild' "\$@"
EOF
chmod +x "$scratch/bin/xcodebuild"
export PATH="$scratch/bin:$PATH"

# The build rewrites tracked files under gen/apple: tauri merges Info.plist and
# writes signing settings into the project, and agvtool stamps the build number
# into both Info.plists. Put every tracked file back as it was, whatever the
# outcome, so a release leaves no diff behind.
cd "$mobile"
git ls-files -z -- "$apple" | tar --null -T - -cf "$scratch/apple.tar"
restore() {
  tar -xf "$scratch/apple.tar" -C "$mobile"
  rm -rf "$scratch"
}
trap restore EXIT

version="$(node -p "require('./src-tauri/tauri.conf.json').version")"
# App Store Connect wants every upload's build number above the last one's.
# UTC minutes grow on their own and need no file to remember them.
build="$(date -u +%Y%m%d%H%M)"

# tauri sets the app's short version and nothing of the extension's; App Store
# Connect refuses an extension whose version differs from its app's.
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $version" \
  "$apple/NotificationService/Info.plist"

rm -f "$apple"/build/arm64/*.ipa
APPLE_API_KEY="$key_id" APPLE_API_ISSUER="$issuer" APPLE_API_KEY_PATH="$key_path" \
  npm run tauri -- ios build --ci --export-method app-store-connect \
  --config "{\"bundle\":{\"iOS\":{\"bundleVersion\":\"$build\"}}}"

ipa="$(ls "$apple"/build/arm64/*.ipa)"
echo "ios-testflight: uploading $ipa ($version, build $build)"
xcrun altool --upload-app -t ios -f "$ipa" \
  --api-key "$key_id" --api-issuer "$issuer" --p8-file-path "$key_path"
echo "ios-testflight: uploaded $version ($build)"
