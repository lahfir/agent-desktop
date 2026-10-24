#!/usr/bin/env bash
# Smoke run of the Linux adapter in Docker containers, with no remote box. Prints PASS or FAIL per step.
# Usage: scripts/linux-container-smoke.sh   (SMOKE_KEEP=1 leaves the session container running)
# Needs Docker and a Debian or Ubuntu image (SMOKE_IMAGE, default desk-lab:latest) with user "me", xvfb,
# openbox, dbus, at-spi2-core, chromium. Volumes ad-cargo and ad-smoke-target cache the registry and target dir.
set -u

cd "$(dirname "$0")/.." || exit 2
ROOT=$(pwd)
IMAGE=${SMOKE_IMAGE:-desk-lab:latest}
NAME=${SMOKE_NAME:-ad-smoke}
RUST_IMAGE=${SMOKE_RUST_IMAGE:-rust:$(sed -n 's/^channel *= *"\(.*\)"/\1/p' rust-toolchain.toml)-bookworm}
WORK=$(mktemp -d)
failures=0

cleanup() { [ "${SMOKE_KEEP:-0}" = 1 ] || docker rm -f "$NAME" "$NAME-build" >/dev/null 2>&1; rm -rf "$WORK" "$WORK.log"; }
trap cleanup EXIT
docker rm -f "$NAME" "$NAME-build" >/dev/null 2>&1

step() {
    label=$1
    shift
    if "$@" >"$WORK.log" 2>&1; then echo "PASS  $label"; else echo "FAIL  $label"; tail -6 "$WORK.log" | sed 's/^/        /'; failures=$((failures + 1)); fi
}
run() { docker exec -u me "$NAME" bash -c ". /tmp/session.env && . /usr/local/bin/lib.sh && $1"; }

build() {
    docker run --name "$NAME-build" -v "$ROOT":/src:ro -v ad-cargo:/usr/local/cargo/registry -v ad-smoke-target:/target \
        -e CARGO_TARGET_DIR=/target -w /src "$RUST_IMAGE" cargo build --locked --release -p agent-desktop -p agent-desktop-linux --bins || return 1
    docker cp "$NAME-build:/target/release/agent-desktop" "$WORK/" &&
        docker cp "$NAME-build:/target/release/agent-desktop-linux-clipboard-owner" "$WORK/"
}

start_session() {
    docker run -d --init --name "$NAME" --shm-size=1g "$IMAGE" sleep infinity >/dev/null || return 1
    docker exec -u root "$NAME" sh -c 'apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq \
        --no-install-recommends gnome-calculator gnome-text-editor jq' || return 1
    docker cp "$WORK/." "$NAME:/usr/local/bin/" || return 1
    docker exec -d -u me "$NAME" dbus-run-session -- bash /usr/local/bin/session.sh
    for _ in $(seq 30); do docker exec -u me "$NAME" test -s /tmp/session.env && return 0; sleep 1; done
    return 1
}

cat >"$WORK/session.sh" <<'SESSION'
export XDG_RUNTIME_DIR=/tmp/xdg-me DISPLAY=:99
mkdir -m 700 -p "$XDG_RUNTIME_DIR"
Xvfb "$DISPLAY" -screen 0 1280x800x24 >/tmp/xvfb.log 2>&1 &
until [ -S /tmp/.X11-unix/X99 ]; do sleep 0.1; done
/usr/libexec/at-spi-bus-launcher --launch-immediately >/tmp/atspi.log 2>&1 &
openbox >/tmp/openbox.log 2>&1 &
printf "export XDG_RUNTIME_DIR=%s DISPLAY=%s DBUS_SESSION_BUS_ADDRESS='%s'\n" \
    "$XDG_RUNTIME_DIR" "$DISPLAY" "$DBUS_SESSION_BUS_ADDRESS" >/tmp/session.env
exec sleep infinity
SESSION

cat >"$WORK/lib.sh" <<'LIB'
FILE=/tmp/smoke-note.txt
VERIFIED='.data.disposition.delivery == "delivered_verified"'
ok() { jq -e "$1" >/dev/null; }
value_is() { agent-desktop wait --element "$1" --predicate value --value "$2" --timeout 5000 | ok .ok; }
ref() { agent-desktop find --app "$1" --role "$2" "${@:3}" | jq -er '.data.matches[0].ref_id'; }
t_status() { agent-desktop status | ok '.ok and (.data.permissions.accessibility.state == "granted")'; }
t_launch_calc() { agent-desktop launch gnome-calculator | ok '.ok and (.data.window.title == "Calculator")'; }
t_multiply() {
    for key in 7 × 6 =; do agent-desktop click "$(ref gnome-calculator button --name "$key" --exact)" | ok .ok || return 1; done
    value_is "$(ref gnome-calculator textfield)" 42
}
t_headed_click() {
    for key in C 7; do agent-desktop --headed click "$(ref gnome-calculator button --name "$key" --exact)" | ok .ok || return 1; done
    value_is "$(ref gnome-calculator textfield)" 7
}
t_screenshot() { agent-desktop screenshot --app gnome-calculator /tmp/smoke.png | ok .ok && [ "$(head -c 4 /tmp/smoke.png | tail -c 3)" = PNG ]; }
t_window_ops() {
    agent-desktop resize-window --app gnome-calculator --width 500 --height 640 | ok "$VERIFIED" &&
        agent-desktop move-window --app gnome-calculator --x 60 --y 40 | ok "$VERIFIED" &&
        agent-desktop minimize --app gnome-calculator | ok "$VERIFIED" &&
        agent-desktop wait 500 | ok .ok && agent-desktop restore --app gnome-calculator | ok .ok
}
t_close_calc() { agent-desktop close-app gnome-calculator | ok "$VERIFIED"; }
t_editor() {
    printf 'original\n' >"$FILE"
    agent-desktop launch gnome-text-editor --arg="$FILE" | ok .ok || return 1
    agent-desktop wait --window "Text Editor" --app gnome-text-editor --timeout 10000 | ok .ok || return 1
    agent-desktop type "$(ref gnome-text-editor textfield)" "smoke line " | ok "$VERIFIED" || return 1
    agent-desktop get "$(ref gnome-text-editor textfield)" --property value | ok '.data.value | contains("smoke line")'
}
t_headed_type() {
    agent-desktop --headed type "$(ref gnome-text-editor textfield)" "H-" | ok .ok || return 1
    agent-desktop get "$(ref gnome-text-editor textfield)" --property value | ok '.data.value | contains("H-")'
}
t_discard() {
    agent-desktop close-app gnome-text-editor >/dev/null
    discard=$(agent-desktop snapshot --app gnome-text-editor -i --compact |
        jq -er '[.. | objects | select(.name? == "Discard" and .ref_id?) | .ref_id][0]') || return 1
    batch=$(jq -nc --arg r "$discard" '[{command:"click",args:{ref_id:$r}},{command:"wait",args:{event:"app-terminated",app:"gnome-text-editor",timeout:8000}}]')
    agent-desktop batch "$batch" --stop-on-error | ok '.data.results[1].data.found' && [ "$(cat "$FILE")" = original ]
}
t_chromium() {
    printf '<title>cx</title><button onclick="document.title=1">go</button><input aria-label="Name field">' >/tmp/page.html
    agent-desktop launch chromium --env ACCESSIBILITY_ENABLED=1 --arg=--no-sandbox --arg=--force-renderer-accessibility \
        --arg=file:///tmp/page.html | ok .ok || return 1
    agent-desktop wait --window cx --app chromium --timeout 20000 | ok .ok || return 1
    agent-desktop click "$(ref chromium button --name go --exact)" | ok .ok || return 1
    agent-desktop wait --window "1 - Chromium" --app chromium --timeout 5000 | ok .ok || return 1
    agent-desktop --headed type "$(ref chromium textfield --name "Name field" --exact)" ABC | ok .ok || return 1
    value_is "$(ref chromium textfield --name "Name field" --exact)" ABC && agent-desktop close-app chromium | ok "$VERIFIED"
}
LIB

echo "== smoke run: image=$IMAGE rust=$RUST_IMAGE"
step "build the release binary in $RUST_IMAGE" build
step "start the desktop session in container $NAME" start_session
[ "$failures" -eq 0 ] || exit 1
for t in status launch_calc multiply headed_click screenshot window_ops close_calc editor headed_type discard chromium; do
    step "$t" run "t_$t"
done
echo "== $failures failed step(s)"
[ "$failures" -eq 0 ]
