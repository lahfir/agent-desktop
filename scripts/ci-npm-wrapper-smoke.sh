#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64) NAME=agent-desktop-darwin-arm64; HELPER_NAME=agent-desktop-macos-helper; EXPECTED_OS=macos ;;
  Darwin-x86_64) NAME=agent-desktop-darwin-x64; HELPER_NAME=agent-desktop-macos-helper; EXPECTED_OS=macos ;;
  Linux-x86_64) NAME=agent-desktop-linux-x64; HELPER_NAME=agent-desktop-linux-clipboard-owner; EXPECTED_OS=linux ;;
  Linux-aarch64) NAME=agent-desktop-linux-arm64; HELPER_NAME=agent-desktop-linux-clipboard-owner; EXPECTED_OS=linux ;;
  *)
    echo "Unsupported smoke-test platform: $(uname -s)-$(uname -m)"
    exit 1
    ;;
esac

digest() {
  if [ "$(uname -s)" = "Linux" ]; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

SOURCE=target/release/agent-desktop
HELPER_SOURCE="target/release/${HELPER_NAME}"
WORK=$(mktemp -d "${TMPDIR:-/tmp}/agent-desktop-npm.XXXXXX")
PACKAGE_DIR="$WORK/package"
VERSION_JSON="$WORK/version.json"
cleanup() {
  if command -v trash >/dev/null 2>&1; then
    trash "$WORK"
  else
    printf 'Cleanup retained %s: trash is unavailable\n' "$WORK" >&2
  fi
}
trap cleanup EXIT
TARBALL="$WORK/$(npm pack ./npm --pack-destination "$WORK" --silent)"
mkdir -p "$PACKAGE_DIR/bin"
cp npm/bin/agent-desktop "$PACKAGE_DIR/bin/"
cp -R npm/lib "$PACKAGE_DIR/lib"

SOURCE_SHA=$(digest "$SOURCE")
HELPER_SOURCE_SHA=$(digest "$HELPER_SOURCE")
cp "$SOURCE" "$PACKAGE_DIR/bin/${NAME}"
cp "$HELPER_SOURCE" "$PACKAGE_DIR/bin/${HELPER_NAME}"
chmod +x "$PACKAGE_DIR/bin/${NAME}"
chmod +x "$PACKAGE_DIR/bin/${HELPER_NAME}"
COPY_SHA=$(digest "$PACKAGE_DIR/bin/${NAME}")
HELPER_COPY_SHA=$(digest "$PACKAGE_DIR/bin/${HELPER_NAME}")
if [ "$SOURCE_SHA" != "$COPY_SHA" ] || [ "$HELPER_SOURCE_SHA" != "$HELPER_COPY_SHA" ]; then
  echo "NPM smoke executable copy failed immutable identity verification" >&2
  exit 1
fi
if [ ! -x "$PACKAGE_DIR/bin/${HELPER_NAME}" ]; then
  echo "NPM smoke helper ${HELPER_NAME} is not executable beside ${NAME}" >&2
  exit 1
fi
node "$PACKAGE_DIR/bin/agent-desktop" version > "$VERSION_JSON"
EXPECTED_OS="$EXPECTED_OS" VERSION_JSON="$VERSION_JSON" node -e "
  const out = require('fs').readFileSync(process.env.VERSION_JSON, 'utf8');
  const json = JSON.parse(out);
  if (json.ok !== true || !json.data || typeof json.data.version !== 'string') {
    throw new Error('agent-desktop npm wrapper did not return version JSON');
  }
  if (json.data.os !== process.env.EXPECTED_OS) {
    throw new Error('agent-desktop npm wrapper reported os ' + json.data.os + ', expected ' + process.env.EXPECTED_OS);
  }
"
if [ "$SOURCE_SHA" != "$(digest "$SOURCE")" ] || \
   [ "$COPY_SHA" != "$(digest "$PACKAGE_DIR/bin/${NAME}")" ] || \
   [ "$HELPER_SOURCE_SHA" != "$(digest "$HELPER_SOURCE")" ] || \
   [ "$HELPER_COPY_SHA" != "$(digest "$PACKAGE_DIR/bin/${HELPER_NAME}")" ]; then
  echo "NPM smoke executable changed while it was executed" >&2
  exit 1
fi

PREFIX="$WORK/prefix"
AGENT_DESKTOP_BINARY_PATH="$ROOT/$SOURCE" npm install -g "$TARBALL" --prefix "$PREFIX" --no-audit --no-fund
INSTALLED_BIN="$PREFIX/lib/node_modules/agent-desktop/bin"
if ! [ "$INSTALLED_BIN/agent-desktop" -ef "$INSTALLED_BIN/${NAME}" ]; then
  echo "Installed bin entry is not the native binary ${NAME}" >&2
  exit 1
fi
PATH="$PREFIX/bin:/usr/bin:/bin" agent-desktop version > "$WORK/installed-version.json"
EXPECTED_OS="$EXPECTED_OS" node -e "
  const json = JSON.parse(require('fs').readFileSync(process.argv[1], 'utf8'));
  if (json.ok !== true || typeof json.data?.version !== 'string') {
    throw new Error('installed agent-desktop command did not return version JSON');
  }
  if (json.data.os !== process.env.EXPECTED_OS) {
    throw new Error('installed agent-desktop reported os ' + json.data.os + ', expected ' + process.env.EXPECTED_OS);
  }
" "$WORK/installed-version.json"
