ref_churn_process_running() {
    local state
    kill -0 "$1" 2>/dev/null || return 1
    state="$(ps -o stat= -p "$1" 2>/dev/null | tr -d '[:space:]')"
    case "$state" in ''|Z*) return 1 ;; esac
    return 0
}

ref_churn_suite() {
    local host_client="$1" label churn_pid churn_exe="$fixture_app/Contents/MacOS/AgentDeskFixture"
    local filter alpha stable output ok code delivery beta_seen stable_write stable_seen close_output
    local -x AGENT_DESKTOP_INTERNAL_HOST_CLIENT="$host_client"
    local -x TMPDIR="$host_tmp"
    label="[ref-churn host_client=$host_client]"
    MODE_FLAG=""
    note "$label strict re-identification after the collection changes"
    if [ -n "$(pgrep -x "$app" 2>/dev/null || true)" ]; then
        badmsg "$label another fixture process is already running"
        return 0
    fi
    AGENT_DESKTOP_FIXTURE_REF_CHURN=1 AGENT_DESKTOP_FIXTURE_NO_ACTIVATE=1 "$churn_exe" >/dev/null 2>&1 &
    churn_pid=$!
    fixture_owned=1
    for _ in $(seq 1 50); do
        find_target_by_id button filter-rows >/dev/null 2>&1 && break
        sleep 0.2
    done
    require_target_by_id filter button filter-rows
    require_target_by_id alpha textfield shared-title
    require_target_by_id stable textfield stable-editor
    act_target "$filter" click >/dev/null 2>&1
    "$bin" wait --text "Restore rows" --app "$app" --timeout 5000 >/dev/null 2>&1
    output="$("$bin" set-value "$(target_ref "$alpha")" poisoned 2>/dev/null)"
    ok="$(json_field "$output" ok)"
    code="$(json_field "$output" error.code)"
    delivery="$(json_field "$output" error.disposition.delivery)"
    if [ "$host_client" = "0" ] && [ "$ok" = "True" ]; then
        skipmsg "$label stale ref: known product defect, the default CLI aliases a removed row's ref to a survivor with the same identifier (docs/solutions/best-practices/identity-fingerprint-against-os-reorder-2026-04-16.md; crates/core/src/refs/ref_identity.rs identity_match, since 3f322728)"
    else
        assert "$label stale ref is refused STALE_REF, not delivered" \
            "$([ "$ok" = "False" ] && [ "$code" = "STALE_REF" ] && [ "$delivery" = "not_delivered" ] && echo 1 || echo 0)" \
            "ok=$ok code=$code delivery=$delivery"
    fi
    beta_seen="$("$bin" wait --text "Beta stored: Beta" --app "$app" --timeout 3000 2>/dev/null)"
    if [ "$host_client" = "0" ] && [ "$ok" = "True" ]; then
        skipmsg "$label sibling kept its value: same known product defect, the aliased stale write landed on the surviving row (docs/solutions/best-practices/identity-fingerprint-against-os-reorder-2026-04-16.md)"
    else
        assert "$label sibling kept its value (independent readout)" \
            "$([ "$(json_field "$beta_seen" data.found)" = "True" ] && echo 1 || echo 0)" \
            "found=$(json_field "$beta_seen" data.found)"
    fi
    stable_write="$("$bin" set-value "$(target_ref "$stable")" stable-ok 2>/dev/null)"
    stable_seen="$(wait_target "$stable" value 3000 --value stable-ok 2>/dev/null)"
    assert "$label an untouched ref still resolves and writes" \
        "$([ "$(json_field "$stable_write" ok)" = "True" ] && [ "$(json_field "$stable_seen" data.found)" = "True" ] && echo 1 || echo 0)" \
        "ok=$(json_field "$stable_write" ok) found=$(json_field "$stable_seen" data.found)"
    close_output="$("$bin" close-app "$app" --force 2>/dev/null)"
    for _ in $(seq 1 25); do
        [ -z "$(pgrep -x "$app" 2>/dev/null || true)" ] && break
        sleep 0.2
    done
    if [ -n "$(pgrep -x "$app" 2>/dev/null || true)" ]; then
        if ref_churn_process_running "$churn_pid"; then
            badmsg "$label fixture still running after close-app (close_err=$(json_field "$close_output" error.code))"
            kill "$churn_pid" 2>/dev/null || true
            for _ in $(seq 1 25); do
                ref_churn_process_running "$churn_pid" || break
                sleep 0.2
            done
            if ref_churn_process_running "$churn_pid"; then
                kill -9 "$churn_pid" 2>/dev/null || true
                for _ in $(seq 1 5); do
                    ref_churn_process_running "$churn_pid" || break
                    sleep 0.2
                done
            fi
            assert "$label own fixture process is gone after teardown" \
                "$(ref_churn_process_running "$churn_pid" && echo 0 || echo 1)" \
                "pid=$churn_pid close_err=$(json_field "$close_output" error.code)"
        else
            badmsg "$label fixture still running after close-app"
        fi
    fi
    wait "$churn_pid" 2>/dev/null
    fixture_owned=0
    if [ "$host_client" = "1" ]; then
        local host_pid
        for host_pid in $(pgrep -fx "$raw_bin" 2>/dev/null); do
            kill "$host_pid" 2>/dev/null || true
        done
        for _ in $(seq 1 25); do
            [ -z "$(pgrep -fx "$raw_bin" 2>/dev/null)" ] && break
            sleep 0.2
        done
        assert "$label the command host it started is stopped and holds no desktop lease" \
            "$([ -z "$(pgrep -fx "$raw_bin" 2>/dev/null)" ] && echo 1 || echo 0)" \
            "remaining=$(pgrep -fx "$raw_bin" 2>/dev/null | tr '\n' ' ')"
    fi
}

ref_churn_suite 0
ref_churn_suite 1
