#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64) NAME=agent-desktop-darwin-arm64 ;;
  Darwin-x86_64) NAME=agent-desktop-darwin-x64 ;;
  *)
    echo "Unsupported smoke-test platform: $(uname -s)-$(uname -m)"
    exit 1
    ;;
esac
HELPER_NAME=agent-desktop-macos-helper
SOURCE=target/release/agent-desktop
HELPER_SOURCE=target/release/agent-desktop-macos-helper
SOURCE_SHA=$(shasum -a 256 "$SOURCE" | awk '{print $1}')
HELPER_SOURCE_SHA=$(shasum -a 256 "$HELPER_SOURCE" | awk '{print $1}')
cp "$SOURCE" "npm/bin/${NAME}"
cp "$HELPER_SOURCE" "npm/bin/${HELPER_NAME}"
chmod +x "npm/bin/${NAME}"
chmod +x "npm/bin/${HELPER_NAME}"
COPY_SHA=$(shasum -a 256 "npm/bin/${NAME}" | awk '{print $1}')
HELPER_COPY_SHA=$(shasum -a 256 "npm/bin/${HELPER_NAME}" | awk '{print $1}')
if [ "$SOURCE_SHA" != "$COPY_SHA" ] || [ "$HELPER_SOURCE_SHA" != "$HELPER_COPY_SHA" ]; then
  echo "NPM smoke executable copy failed immutable identity verification" >&2
  exit 1
fi
node npm/bin/agent-desktop.js version > /tmp/agent-desktop-version.json
node -e "
  const out = require('fs').readFileSync('/tmp/agent-desktop-version.json', 'utf8');
  const json = JSON.parse(out);
  if (json.ok !== true || !json.data || typeof json.data.version !== 'string') {
    throw new Error('agent-desktop npm wrapper did not return version JSON');
  }
"
if [ "$SOURCE_SHA" != "$(shasum -a 256 "$SOURCE" | awk '{print $1}')" ] || \
   [ "$COPY_SHA" != "$(shasum -a 256 "npm/bin/${NAME}" | awk '{print $1}')" ] || \
   [ "$HELPER_SOURCE_SHA" != "$(shasum -a 256 "$HELPER_SOURCE" | awk '{print $1}')" ] || \
   [ "$HELPER_COPY_SHA" != "$(shasum -a 256 "npm/bin/${HELPER_NAME}" | awk '{print $1}')" ]; then
  echo "NPM smoke executable changed while it was executed" >&2
  exit 1
fi
