#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

read_toml_version() {
  sed -n 's/^version *= *"\([^"]*\)".*/\1/p' Cargo.toml | head -1
}

read_json_version() {
  local file="$1"
  sed -n 's/.*"version" *: *"\([^"]*\)".*/\1/p' "$file" | head -1
}

read_manifest_version() {
  sed -n 's/.*"\." *: *"\([^"]*\)".*/\1/p' .release-please-manifest.json | head -1
}

lock_version_for() {
  local package="$1"
  awk -v target="$package" '
    /^\[\[package\]\]/ {
      if (name == target) {
        print version
        found = 1
        exit
      }
      name = ""
      version = ""
      next
    }
    /^name = / {
      name = $3
      gsub(/"/, "", name)
      next
    }
    /^version = / {
      version = $3
      gsub(/"/, "", version)
      next
    }
    END {
      if (!found && name == target) {
        print version
      }
    }
  ' Cargo.lock
}

workspace_version="$(read_toml_version)"
npm_version="$(read_json_version npm/package.json)"
manifest_version="$(read_manifest_version)"

[[ -n "$workspace_version" ]] || fail "could not read workspace version from Cargo.toml"
[[ -n "$npm_version" ]] || fail "could not read npm package version"
[[ -n "$manifest_version" ]] || fail "could not read release-please manifest version"

[[ "$npm_version" == "$workspace_version" ]] \
  || fail "npm/package.json version ${npm_version} does not match Cargo.toml ${workspace_version}"

[[ "$manifest_version" == "$workspace_version" ]] \
  || fail ".release-please-manifest.json version ${manifest_version} does not match Cargo.toml ${workspace_version}"

packages=(
  agent-desktop
  agent-desktop-core
  agent-desktop-ffi
  agent-desktop-linux
  agent-desktop-macos
  agent-desktop-windows
)

for package in "${packages[@]}"; do
  lock_version="$(lock_version_for "$package")"
  [[ -n "$lock_version" ]] || fail "Cargo.lock is missing ${package}"
  [[ "$lock_version" == "$workspace_version" ]] \
    || fail "Cargo.lock ${package} version ${lock_version} does not match Cargo.toml ${workspace_version}"
done

cargo metadata --locked --no-deps --format-version 1 >/dev/null
echo "OK: release metadata is internally consistent for ${workspace_version}"
