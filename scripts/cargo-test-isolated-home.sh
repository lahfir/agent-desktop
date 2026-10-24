#!/usr/bin/env bash
set -euo pipefail

original_home="${HOME:?HOME is required}"
cargo_home="${CARGO_HOME:-$original_home/.cargo}"
rustup_home="${RUSTUP_HOME:-$original_home/.rustup}"
test_home="$(mktemp -d "${TMPDIR:-/tmp}/agent-desktop-test-home.XXXXXX")"

cleanup() {
    rm -rf -- "$test_home"
}
trap cleanup EXIT

HOME="$test_home" CARGO_HOME="$cargo_home" RUSTUP_HOME="$rustup_home" cargo "$@"
