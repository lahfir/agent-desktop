#!/usr/bin/env bash
set -uo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ -z "${AGENT_DESKTOP_INTERACTION_LEASE_FD:-}" ]; then
    exec python3 "$here/interaction_lock.py" run "$0" "$@"
fi
# shellcheck source=tests/e2e/lib.sh
source "$here/lib.sh"
trap cleanup EXIT

note "Global prerequisite gate"
if [ "$(uname -s)" != "Darwin" ]; then
    echo "macOS is required for the native accessibility E2E suite" >&2
    exit 2
fi
if [ ! -x "$release_bin" ]; then
    echo "release binary missing at $release_bin; run 'cargo build --release'" >&2
    exit 2
fi
if ! prepare_native_harness; then
    exit 2
fi
permission_json="$("$bin" permissions 2>/dev/null)"
ax_state="$(json_field "$permission_json" data.accessibility.state)"
if [ "$ax_state" != "granted" ]; then
    echo "accessibility permission not granted (state='${ax_state:-unknown}')." >&2
    echo "Grant trust to this terminal/runner in System Settings > Privacy & Security > Accessibility." >&2
    exit 2
fi
guard_exec 120 4194304 "$repo/tests/fixture-app/build.sh" "$suite_root/fixture" >/dev/null || {
    echo "fixture build failed; cannot run E2E" >&2
    exit 2
}

guard_exec 10 1048576 open "$fixture_app" || {
    echo "fixture could not be opened" >&2
    exit 2
}

ready=""
tries=0
for _ in $(seq 1 20); do
    tries=$((tries + 1))
    "$bin" focus-window --app "$app" >/dev/null 2>&1 || true
    if find_target button primary-button >/dev/null; then
        ready=1
        break
    fi
    sleep 0.5
done
if [ -z "$ready" ]; then
    badmsg "fixture launched but primary-button was not exposed after $tries attempts"
    finish
    exit 1
fi
if ! record_fixture_process; then
    finish
    exit 1
fi
okmsg "fixture launched and primary-button exposed with exact snapshot namespace"

check_fixture_contention || { finish; exit 1; }
# shellcheck source=tests/e2e/scenarios-observation.sh
source "$here/scenarios-observation.sh"
check_fixture_contention || { finish; exit 1; }
# shellcheck source=tests/e2e/scenarios-interaction.sh
source "$here/scenarios-interaction.sh"
check_fixture_contention || { finish; exit 1; }
# shellcheck source=tests/e2e/scenarios-acceptance.sh
source "$here/scenarios-acceptance.sh"
check_fixture_contention || { finish; exit 1; }
# shellcheck source=tests/e2e/scenarios-reliability.sh
source "$here/scenarios-reliability.sh"
check_fixture_contention || { finish; exit 1; }
# shellcheck source=tests/e2e/scenarios-surfaces.sh
source "$here/scenarios-surfaces.sh"
check_fixture_contention || { finish; exit 1; }
# shellcheck source=tests/e2e/scenarios-trace-performance.sh
source "$here/scenarios-trace-performance.sh"
check_fixture_contention || { finish; exit 1; }
# shellcheck source=tests/e2e/scenarios-notifications.sh
source "$here/scenarios-notifications.sh"

finish
