#!/bin/sh
# Opens test-vectors/push.json (sealed by the Rust code) with the Swift
# implementation the iOS notification extension uses. Needs a Swift toolchain
# with CryptoKit: Xcode's, or the command line tools'.
set -e
here="$(cd "$(dirname "$0")" && pwd)"
out="$(mktemp -d)"
trap 'rm -rf "$out"' EXIT
swiftc -O "$here/PushBlob.swift" "$here/main.swift" -o "$out/check"
"$out/check" "$here/../test-vectors/push.json"
