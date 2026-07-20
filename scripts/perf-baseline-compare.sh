#!/usr/bin/env bash
# HEAD-vs-baseline performance comparison for agent-desktop.
#
# Builds the current checkout and the merge-base-with-main revision, then:
#   1. runs a fixture A/B (snapshots, reads, and fixture-only actions),
#   2. runs the deterministic synthetic locator benchmark on HEAD,
#   3. optionally probes real apps READ-ONLY (snapshot/find timing only),
# and writes a markdown report + raw JSON.
#
# Usage:
#   scripts/perf-baseline-compare.sh [--base <ref>] [--apps "Slack,Google Chrome"]
#                                    [--rounds N] [--out <dir>] [--skip-fixture]
#
# Real-app probes never dispatch actions. Apps this script launches itself are
# closed again afterwards; apps that were already running are left untouched.
set -euo pipefail

base_ref=""
apps=""
rounds=10
out=""
skip_fixture=0
while [ $# -gt 0 ]; do
    case "$1" in
        --base) base_ref="$2"; shift 2 ;;
        --apps) apps="$2"; shift 2 ;;
        --rounds) rounds="$2"; shift 2 ;;
        --out) out="$2"; shift 2 ;;
        --skip-fixture) skip_fixture=1; shift ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done

repo="$(git rev-parse --show-toplevel)"
cd "$repo"
[ -n "$base_ref" ] || base_ref="$(git merge-base HEAD "$(git rev-parse --verify --quiet origin/main >/dev/null 2>&1 && echo origin/main || echo main)")"
base_sha="$(git rev-parse --short "$base_ref")"
head_sha="$(git rev-parse --short HEAD)"
[ -n "$out" ] || out="$(mktemp -d "/tmp/agent-desktop-perf-${head_sha}-vs-${base_sha}.XXXXXX")"
mkdir -p "$out"
echo "== perf compare: HEAD ${head_sha} vs base ${base_sha} -> ${out}"

echo "== building HEAD release binary"
cargo build --release -p agent-desktop >/dev/null
head_bin="$repo/target/release/agent-desktop"

echo "== building base release binary (${base_sha}) in a temporary worktree"
wt="$out/base-worktree"
git worktree add --detach "$wt" "$base_ref" >/dev/null
trap 'git worktree remove --force "$wt" >/dev/null 2>&1 || true; git worktree prune >/dev/null 2>&1 || true' EXIT
(cd "$wt" && cargo build --release -p agent-desktop >/dev/null)
base_bin="$wt/target/release/agent-desktop"

if [ "$skip_fixture" -ne 1 ]; then
    echo "== fixture A/B (${rounds} rounds; actions run against the fixture only)"
    bash tests/fixture-app/build.sh "$out/fixture" >/dev/null
    open "$out/fixture/AgentDeskFixture.app"
    sleep 3
    python3 scripts/perf_ab_probe.py --mode fixture --app AgentDeskFixture \
        --head-bin "$head_bin" --base-bin "$base_bin" --rounds "$rounds" \
        --json-out "$out/fixture-ab.json" >/dev/null
    pkill -f AgentDeskFixture.app || true
fi

echo "== synthetic locator benchmark (HEAD)"
cargo run -p agent-desktop-core --release --example locator_benchmark \
    > "$out/locator-synthetic.json" 2>/dev/null

if [ -n "$apps" ]; then
    IFS=',' read -ra app_list <<< "$apps"
    for app in "${app_list[@]}"; do
        app="$(echo "$app" | sed 's/^ *//;s/ *$//')"
        slug="$(echo "$app" | tr ' ' '-' | tr '[:upper:]' '[:lower:]')"
        was_running=1
        pgrep -f "/${app}.app/" >/dev/null 2>&1 || was_running=0
        if [ "$was_running" -eq 0 ]; then
            echo "== launching ${app} for read-only observation"
            open -a "$app" || { echo "   skip: cannot open ${app}"; continue; }
            sleep 12
        fi
        echo "== read-only probe: ${app} (${rounds} rounds, snapshots + find only)"
        python3 scripts/perf_ab_probe.py --mode app --app "$app" \
            --head-bin "$head_bin" --base-bin "$base_bin" --rounds "$rounds" \
            --json-out "$out/app-${slug}.json" >/dev/null || echo "   probe failed for ${app}"
        if [ "$was_running" -eq 0 ]; then
            "$head_bin" close-app --app "$app" >/dev/null 2>&1 || true
        fi
    done
fi

python3 scripts/perf_report_html.py "$out" --head "$head_sha" --base "$base_sha"
echo "== artifacts: $out"
