#!/usr/bin/env bash
set -euo pipefail

original_home="${HOME:?HOME is required}"
cargo_home="${CARGO_HOME:-$original_home/.cargo}"
rustup_home="${RUSTUP_HOME:-$original_home/.rustup}"
test_home="$(mktemp -d "${TMPDIR:-/tmp}/agent-desktop-test-home.XXXXXX")"

cleanup() {
    if command -v trash >/dev/null 2>&1; then
        trash "$test_home"
    else
        printf 'Retained isolated test home because trash is unavailable: %s\n' "$test_home" >&2
    fi
}
trap cleanup EXIT

HOME="$test_home" CARGO_HOME="$cargo_home" RUSTUP_HOME="$rustup_home" cargo "$@"
