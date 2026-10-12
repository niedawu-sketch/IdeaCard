#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEVELOPER_ROOT="${DEVELOPER_DIR:-$(xcode-select -p)}"
TESTING_FRAMEWORKS="$DEVELOPER_ROOT/Library/Developer/Frameworks"
TESTING_LIBRARIES="$DEVELOPER_ROOT/Library/Developer/usr/lib"

swift test \
  --package-path "$ROOT/src-tauri/native" \
  -Xswiftc -F \
  -Xswiftc "$TESTING_FRAMEWORKS" \
  -Xlinker -rpath \
  -Xlinker "$TESTING_FRAMEWORKS" \
  -Xlinker -rpath \
  -Xlinker "$TESTING_LIBRARIES" \
  "$@"
