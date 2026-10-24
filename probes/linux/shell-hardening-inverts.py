#!/usr/bin/env python3
"""Break one shell contract, require its live test to fail, then restore and pass."""
import os
from pathlib import Path
import signal
import subprocess
import sys

LOSS = "bus_live_loss_keeps_cold_start_recovery_without_claiming_delivery"
CASES = {
    "transport": ("crates/linux/src/tree/properties/node_read.rs",
                  "Err(error) if crate::system::is_connection_failure(&error) => {",
                  "Err(error) if false && crate::system::is_connection_failure(&error) => {", LOSS),
    "mapping": ("crates/linux/src/system/bus_fault.rs",
                "if matches!(\n                    io.kind(),",
                "if matches!(io.kind(), ErrorKind::NotFound | ErrorKind::ConnectionRefused) && matches!(\n                    io.kind(),", LOSS),
    "shell-read": ("crates/linux/src/system/shell/identity.rs",
                   "let root = nodes.read(app)?;",
                   "let Ok(root) = nodes.read(app) else { return Ok(None); };", LOSS),
    "empty-window": ("crates/linux/src/system/shell/identity.rs",
                     "\n    Ok(None)\n}", "\n    Ok(Some(app.address()))\n}", LOSS),
    "stopped-app": ("crates/linux/src/system/proc_stat.rs",
                    "matches!(read_stat(pid), Ok(Some(stat)) if stat.state == ProcState::Stopped)",
                    "false && matches!(read_stat(pid), Ok(Some(stat)) if stat.state == ProcState::Stopped)",
                    "bus_live_stopped_app_is_unresponsive_while_the_bus_still_answers"),
    "shell-root": ("crates/linux/src/system/shell/identity.rs",
                   "path: window.path().to_owned(),", "path: app.path().to_owned(),",
                   "shell_live_window_matches_direct_bus_reads_across_notification_posts"),
    "stopped-shell": ("crates/linux/src/system/shell/identity.rs",
                      "if crate::system::proc_stat::is_stopped(pid) {",
                      "if false && crate::system::proc_stat::is_stopped(pid) {",
                      "shell_live_stopped_process_is_unresponsive_instead_of_a_window_outage"),
    "role-loss": ("crates/linux/src/system/shell/light.rs",
                  "Err(error) if crate::system::is_connection_failure(&error) => {",
                  "Err(error) if false && crate::system::is_connection_failure(&error) => {",
                  "shell_live_lost_role_connection_is_not_a_destroyed_node"),
}
REASONS = {
    "transport": ("transport failure must not become a partial tree",),
    "mapping": ("PlatformNotSupported",),
    "shell-read": ("an unreadable shell is not a missing window",
                   "a server read deadline must not become a missing window"),
    "empty-window": ("an empty app must never be substituted for its missing window",),
    "stopped-app": ("stopped app", "AppUnresponsive"),
    "shell-root": ("assertion `left == right` failed",),
    "stopped-shell": ("AppUnresponsive",),
    "role-loss": ("lost role transport cannot mean a destroyed node",),
}


def interrupted(signum, _frame):
    raise RuntimeError(f"interrupted by signal {signum}")


for sig in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP):
    signal.signal(sig, interrupted)

assert os.environ.get("AGENT_DESKTOP_PRIVATE_SESSION"), "private session required"
case = sys.argv[1]
path, old, new, test = CASES[case]
source = Path(path)
original = source.read_bytes()
text = original.decode()
assert text.count(old) == 1, f"mutation anchor count: {case}"
base = Path(os.environ.get("AGENT_DESKTOP_SESSION_BASE")
            or (Path(os.environ["XDG_RUNTIME_DIR"]) / "agent-desktop-sessions"
                if os.environ.get("XDG_RUNTIME_DIR") else Path(f"/tmp/agent-desktop-sessions-{os.getuid()}")))
base.mkdir(mode=0o700, parents=True, exist_ok=True)
logs = base / "linux-3.15-shell-proof"
logs.mkdir(parents=True, exist_ok=True)
command = ("export CARGO_INCREMENTAL=0 && cargo test -p agent-desktop-linux --lib "
           + test + " -- --ignored --nocapture --test-threads=1")


def run(label):
    result = subprocess.run(["bash", "-c", command], text=True,
                            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=120)
    (logs / f"invert-{case}-{label}.log").write_text(result.stdout)
    print(result.stdout, flush=True)
    return result


try:
    source.write_text(text.replace(old, new))
    broken = run("broken")
    assert broken.returncode == 101 and "test result: FAILED" in broken.stdout, \
        f"{case}: mutation did not produce a live assertion failure"
    assert any(reason in broken.stdout for reason in REASONS[case]), \
        f"{case}: failure did not match the intended regression"
finally:
    source.write_bytes(original)
restored = run("restored")
assert restored.returncode == 0 and "1 passed; 0 failed" in restored.stdout, \
    f"{case}: restoration did not pass"
assert source.read_bytes() == original, "source restoration"
print(f"INVERT PASS {case}: live assertion failed; byte-exact restoration passed", flush=True)
