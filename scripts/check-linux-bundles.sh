#!/usr/bin/env bash
# Inspect the two native Linux desktop packages, without starting the webview.
set -euo pipefail

bundle_dir="${1:?usage: check-linux-bundles.sh <bundle directory>}"
expected_version="$(node -p "require('./src-tauri/tauri.conf.json').version")"
if [[ -n "${LINUX_PACKAGE_REPORT:-}" ]]; then
  exec > >(tee "$LINUX_PACKAGE_REPORT") 2>&1
fi

fail() { echo "ERROR: $*" >&2; exit 1; }
require_file() { test -f "$1" || fail "missing $1"; }
require_executable() { require_file "$1"; test -x "$1" || fail "not executable: $1"; }
check_elf() {
  require_executable "$1"
  file "$1"
  file "$1" | grep -Eq 'ELF 64-bit.*x86-64' || fail "wrong architecture: $1"
  if ldd "$1" | grep -q 'not found'; then
    ldd "$1"
    fail "unresolved library: $1"
  fi
}
check_contents() {
  local root="$1" main="$2" bd="$3" server="$4" license resources
  license="$(find "$root/usr/lib" -type f -path '*/binaries/LICENSE-bd' -print -quit)"
  [[ -n "$license" ]] || fail "missing bundled bd license in $root/usr/lib"
  resources="$(dirname "$(dirname "$license")")"
  check_elf "$main"
  check_elf "$bd"
  check_elf "$server"
  "$bd" --version
  "$server" --help > /dev/null
  require_file "$resources/binaries/LICENSE-bd"
  require_file "$resources/resources/superpowers/skills/using-superpowers/SKILL.md"
  require_file "$resources/resources/smetana/skills/running-tasks/SKILL.md"
  [[ -n "$(find "$root/usr/share/applications" -type f -name '*.desktop' -print -quit)" ]] || fail "missing desktop file in $root"
  [[ -n "$(find "$root/usr/share/icons" -type f \( -name '*.png' -o -name '*.svg' \) -print -quit)" ]] || fail "missing icon in $root"
  echo "PASS package contents: $root"
}

shopt -s nullglob
debs=("$bundle_dir"/deb/*.deb)
images=("$bundle_dir"/appimage/*.AppImage)
((${#debs[@]} == 1)) || fail "expected one .deb, found ${#debs[@]}"
((${#images[@]} == 1)) || fail "expected one AppImage, found ${#images[@]}"
deb="${debs[0]}"
image="${images[0]}"
[[ "$(basename "$image")" == *"$expected_version"* ]] || fail "AppImage version differs from $expected_version"
[[ "$(dpkg-deb -f "$deb" Architecture)" == amd64 ]] || fail "deb architecture is not amd64"
[[ "$(dpkg-deb -f "$deb" Version)" == "$expected_version" ]] || fail "deb version differs from $expected_version"
[[ "$(dpkg-deb -f "$deb" Package)" == smetana ]] || fail "deb package name is not smetana"
echo "deb metadata: $(dpkg-deb -f "$deb" Package Version Architecture Depends)"

scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
dpkg-deb -x "$deb" "$scratch/deb"
# Cargo.toml's package name is `app`; productName controls the package label.
check_contents "$scratch/deb" "$scratch/deb/usr/bin/app" "$scratch/deb/usr/bin/bd" "$scratch/deb/usr/bin/smetanad"

require_executable "$image"
file "$image" | grep -Eq 'ELF 64-bit.*x86-64' || fail "AppImage is not x86_64"
if [[ -n "${APPIMAGE_RUNTIME:-}" ]]; then
  (cd "$scratch" && "$APPIMAGE_RUNTIME" "$OLDPWD/$image" --appimage-extract > /dev/null)
else
  (cd "$scratch" && "$OLDPWD/$image" --appimage-extract > /dev/null)
fi
app="$scratch/squashfs-root"
require_executable "$app/AppRun"
# AppRun sets APPDIR and points the loader at the AppImage's own libraries.
# Check the sidecars and main ELF under that environment, not only against
# libraries that happen to be installed on the build runner.
APPDIR="$app" LD_LIBRARY_PATH="$app/usr/lib:$app/usr/lib/x86_64-linux-gnu" \
  check_contents "$app" "$app/usr/bin/app" "$app/usr/bin/bd" "$app/usr/bin/smetanad"

echo "PASS Linux desktop bundles: $expected_version ($deb, $image)"
