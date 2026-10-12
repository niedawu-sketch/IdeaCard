#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PACKAGE="$ROOT/src-tauri/native"
DESTINATION="$ROOT/src-tauri/binaries/ideacard-native-aarch64-apple-darwin"

if [[ "$(uname -m)" != "arm64" ]]; then
  echo "build_native_requires_arm64" >&2
  exit 1
fi

swift build --package-path "$PACKAGE" -c release
BIN_PATH="$(swift build --package-path "$PACKAGE" -c release --show-bin-path)"
mkdir -p "$(dirname "$DESTINATION")"
cp "$BIN_PATH/ideacard-native" "$DESTINATION"
chmod 755 "$DESTINATION"
