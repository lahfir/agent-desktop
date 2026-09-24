note "Background pointer (--background) leaves the cursor and frontmost app alone"

background_cursor() {
    python3 - <<'PY'
import ctypes

class CGPoint(ctypes.Structure):
    _fields_ = [("x", ctypes.c_double), ("y", ctypes.c_double)]

cg = ctypes.cdll.LoadLibrary(
    "/System/Library/Frameworks/ApplicationServices.framework/ApplicationServices"
)
cg.CGEventCreate.restype = ctypes.c_void_p
cg.CGEventCreate.argtypes = [ctypes.c_void_p]
cg.CGEventGetLocation.restype = CGPoint
cg.CGEventGetLocation.argtypes = [ctypes.c_void_p]
cg.CFRelease.argtypes = [ctypes.c_void_p]
event = cg.CGEventCreate(None)
point = cg.CGEventGetLocation(event)
cg.CFRelease(event)
print(f"{point.x:.0f},{point.y:.0f}")
PY
}

background_frontmost_pid() {
    "$bin" list-windows 2>/dev/null | python3 -c '
import json, sys
focused = [w for w in json.load(sys.stdin).get("data", []) if w.get("is_focused") is True]
print(focused[0]["pid"] if len(focused) == 1 else "none")
' 2>/dev/null
}

background_suite() {
    local primary bounds window_json window_id point cursor_before cursor_after
    local front_before front_after status_before status_after output ok delivery
    local before_number after_number

    require_target primary button primary-button
    act_target "$primary" scroll-to >/dev/null 2>&1
    require_target primary button primary-button
    bounds="$(get_target "$primary" --property bounds 2>/dev/null)"
    window_json="$("$bin" list-windows --app "$app" 2>/dev/null)"
    window_id="$(json_field "$window_json" data.0.id)"
    point="$(printf '%s' "$bounds" | python3 -c '
import json, sys
value = json.load(sys.stdin)["data"]["value"]
print("%.0f,%.0f" % (value["x"] + value["width"] / 2, value["y"] + value["height"] / 2))
' 2>/dev/null)"
    if [ -z "$window_id" ] || [ -z "$point" ]; then
        badmsg "background: fixture window id or primary-button bounds unavailable (window='$window_id' point='$point')"
        return
    fi

    "$bin" focus-window --app Finder >/dev/null 2>&1
    sleep 0.5
    front_before="$(background_frontmost_pid)"
    if [ "$front_before" = "none" ] || [ "$front_before" = "$fixture_pid" ]; then
        skipmsg "background: could not put another app in front of the fixture (frontmost=$front_before)"
        return
    fi

    require_value status_before click-status
    cursor_before="$(background_cursor)"
    output="$("$bin" mouse-click --background --window-id "$window_id" --xy "$point" 2>&1)"
    sleep 0.6
    cursor_after="$(background_cursor)"
    front_after="$(background_frontmost_pid)"
    require_value status_after click-status

    before_number="${status_before##*-}"
    after_number="${status_after##*-}"
    case "$before_number" in *[!0-9]*|'') before_number=0 ;; esac
    case "$after_number" in *[!0-9]*|'') after_number=0 ;; esac
    ok="$(json_field "$output" ok)"
    delivery="$(json_field "$output" data.disposition.delivery)"

    assert "background mouse-click presses the fixture button exactly once" \
        "$([ "$ok" = "True" ] && [ "$delivery" = "delivered_unverified" ] && [ "$after_number" -eq $((before_number + 1)) ] && echo 1 || echo 0)" \
        "before=$status_before after=$status_after ok=$ok delivery=$delivery window=$window_id point=$point"
    assert "background mouse-click leaves the frontmost app unchanged" \
        "$([ "$front_after" = "$front_before" ] && echo 1 || echo 0)" \
        "before=$front_before after=$front_after focus_change=$(json_field "$output" data.background.focus_change)"
    assert "background mouse-click leaves the real cursor where it was" \
        "$([ -n "$cursor_before" ] && [ "$cursor_after" = "$cursor_before" ] && echo 1 || echo 0)" \
        "before=$cursor_before after=$cursor_after"
}

background_suite
"$bin" focus-window --app "$app" >/dev/null 2>&1
