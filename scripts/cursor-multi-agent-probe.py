#!/usr/bin/env python3
"""Verify three macOS CLI cursors using only the supplied fixture application."""
import argparse
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin", required=True, type=Path)
    parser.add_argument("--fixture", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    state_directory = tempfile.TemporaryDirectory(prefix="adc-", dir="/tmp")
    state = Path(state_directory.name)
    env = dict(os.environ, AGENT_DESKTOP_HOME=str(state))
    env.pop("AGENT_DESKTOP_SESSION", None)
    env.pop("AGENT_DESKTOP_AGENT_ID", None)
    lease_fd = env.get("AGENT_DESKTOP_INTERACTION_LEASE_FD")
    cli_pass_fds = (int(lease_fd),) if lease_fd is not None else ()

    def cli(*command):
        result = subprocess.run([str(args.bin.resolve()), *command], env=env,
                                capture_output=True, text=True, timeout=15,
                                pass_fds=cli_pass_fds)
        value = json.loads(result.stdout)
        assert result.returncode == 0 and value["ok"], value
        return value["data"]

    fixture = subprocess.Popen([str(args.fixture.resolve())], stdout=subprocess.DEVNULL,
                               stderr=subprocess.DEVNULL)
    session_id = None
    other_session = None
    pids = set()
    try:
        time.sleep(2)
        session_id = cli("session", "start", "--cursor", "--multi-agent")["session_id"]
        env["AGENT_DESKTOP_SESSION"] = session_id
        assert not list(state.glob("*.sock")), "setup created an unwanted default cursor"
        snapshot = cli("snapshot", "--app", "AgentDeskFixture", "-i")
        (args.out / "snapshot.json").write_text(json.dumps(snapshot, indent=2))

        def walk(node):
            if isinstance(node, dict):
                yield node
                for value in node.values():
                    yield from walk(value)
            elif isinstance(node, list):
                for value in node:
                    yield from walk(value)

        target = next(node["ref_id"] for node in walk(snapshot)
                      if node.get("name") == "primary-button" and node.get("ref_id"))
        for command in [["click", target], ["batch", json.dumps([
                {"command": "click", "args": {"ref_id": target}}])]]:
            result = subprocess.run([str(args.bin.resolve()), *command], env=env,
                                    capture_output=True, text=True, timeout=15,
                                    pass_fds=cli_pass_fds)
            error = json.loads(result.stdout)
            assert error["error"]["code"] == "INVALID_ARGS", error
            assert "agent-id" in error["error"]["message"], error
        assert not list(state.glob("*.sock"))
        sockets = {}
        previous = set()
        colors = ["#FF3B7B", "#49C98A", "#FFE080"]
        for index, color in enumerate(colors):
            agent = f"worker-{index}"
            if index < 2:
                cli("--agent-id", agent, "cursor-overlay", "enable", "--label", agent,
                    "--fill", color, "--accent", color)
                assert set(state.glob("*.sock")) == previous, "profile created a cursor"
            cli("--agent-id", agent, "click", target)
            paths = set(state.glob("*.sock"))
            assert len(paths) == index + 1, paths
            sockets[agent] = (paths - previous).pop()
            previous = paths
        before = set(state.glob("*.sock"))
        cli("--agent-id", "worker-0", "click", target)
        assert set(state.glob("*.sock")) == before, "repeat identity created another cursor"
        displays = cli("list-displays")
        bounds = displays[0]["bounds"]
        for index, path in enumerate(sorted(before)):
            pid_output = subprocess.check_output(["lsof", "-t", str(path)], text=True)
            pids.update(int(pid) for pid in pid_output.split())
        assert len(pids) == 3, pids
        memory = subprocess.check_output(["ps", "-o", "pid=,rss=", "-p",
                                          ",".join(map(str, sorted(pids)))], text=True)
        for index, color in enumerate(colors):
            agent = f"worker-{index}"
            control = {"action": "present", "session_id": session_id, "agent_id": agent,
                       "style": {"fill": color, "accent": color}, "instruction": {
                           "destination": {"x": bounds["x"] + 200 + index * 260,
                                           "y": bounds["y"] + 250},
                           "label": agent, "click": False, "phase": "travel"}}
            with socket.socket(socket.AF_UNIX) as stream:
                stream.settimeout(2)
                stream.connect(str(sockets[agent]))
                stream.sendall(json.dumps(control).encode())
                stream.shutdown(socket.SHUT_WR)
                assert stream.recv(1) == b"\x01"
        subprocess.run(["screencapture", "-x", str(args.out / "three-cursors.png")], check=True)
        report = {"session_id": session_id, "cursor_count": len(before),
                  "renderer_pids": sorted(pids), "rss_kib": memory,
                  "shared_ref": target, "state_root": str(state),
                  "profile_free_agent": "worker-2"}
        other_session = cli("session", "start", "--cursor", "--multi-agent")["session_id"]
        other_snapshot = cli("--session", other_session, "snapshot", "--app", "AgentDeskFixture", "-i")
        other_target = next(node["ref_id"] for node in walk(other_snapshot)
                            if node.get("name") == "primary-button" and node.get("ref_id"))
        cli("--session", other_session, "--agent-id", "worker-0", "click", other_target)
        other_socket, = set(state.glob("*.sock")) - before
        other_pid = int(subprocess.check_output(["lsof", "-t", str(other_socket)], text=True).strip())
        cli("session", "end")
        def alive(pid):
            try:
                os.kill(pid, 0)
                return True
            except ProcessLookupError:
                return False

        deadline = time.monotonic() + 5
        while time.monotonic() < deadline and (any(path.exists() for path in before) or any(map(alive, pids))):
            time.sleep(0.05)
        assert set(state.glob("*.sock")) == {other_socket}, "session end crossed session boundaries"
        assert not any(map(alive, pids)), "renderer survived session end"
        cli("--session", other_session, "--agent-id", "worker-0", "cursor-overlay", "disable")
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline and (other_socket.exists() or alive(other_pid)):
            time.sleep(0.05)
        assert not list(state.glob("*.sock")), "disable left a cursor socket"
        assert not alive(other_pid), "disable left a renderer"
        report["cleanup_verified"] = True
        report["cross_session_isolation"] = True
        (args.out / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps(report, indent=2))
    finally:
        try:
            if session_id:
                cli("session", "end")
            if other_session:
                cli("session", "end", other_session)
        finally:
            fixture.terminate()
            try:
                fixture.wait(timeout=5)
            except subprocess.TimeoutExpired:
                fixture.kill()
                fixture.wait(timeout=5)
            state_directory.cleanup()


if __name__ == "__main__":
    main()
