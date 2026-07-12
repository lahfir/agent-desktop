#!/usr/bin/env python3
import dataclasses
import json
import os
import resource
import selectors
import signal
import subprocess
import sys
import time


DEFAULT_TIMEOUT_SECONDS = 20.0
DEFAULT_MAX_CAPTURE_BYTES = 2 * 1024 * 1024
MAX_JSON_INPUT_BYTES = 4 * 1024 * 1024
INTERACTION_LEASE_FD_ENV = "AGENT_DESKTOP_INTERACTION_LEASE_FD"
INHERIT_INTERACTION_LEASE_ENV = "AGENT_DESKTOP_E2E_INHERIT_LEASE"


@dataclasses.dataclass
class BoundedResult:
    args: list[str]
    returncode: int
    stdout: str
    stderr: str
    timed_out: bool
    output_limited: bool
    termination_error: str | None
    wall_ms: float
    cpu_ms: float


def _terminate_group(process):
    if process.poll() is not None:
        return None
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        return None
    except OSError as error:
        try:
            process.terminate()
        except OSError:
            pass
        return f"process-group SIGTERM failed: {error}"
    try:
        process.wait(timeout=0.25)
        return None
    except subprocess.TimeoutExpired:
        pass
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        return None
    except OSError as error:
        try:
            process.kill()
        except OSError:
            pass
        return f"process-group SIGKILL failed: {error}"
    return None


def _merge_error(current, additional):
    if not additional:
        return current
    return f"{current}; {additional}" if current else additional


def _interaction_lease_fds(env):
    environment = os.environ if env is None else env
    raw_fd = environment.get(INTERACTION_LEASE_FD_ENV)
    if raw_fd is None:
        return ()
    try:
        fd = int(raw_fd)
    except ValueError as error:
        raise ValueError("interaction lease FD must be a decimal integer") from error
    if fd < 0:
        raise ValueError("interaction lease FD must be nonnegative")
    os.fstat(fd)
    return (fd,)


def _close_streams(selector, process):
    for stream in (process.stdout, process.stderr):
        if stream is None:
            continue
        try:
            selector.unregister(stream)
        except (KeyError, ValueError):
            pass
        stream.close()


def run_bounded(
    command,
    timeout_seconds=None,
    max_capture_bytes=None,
    env=None,
    inherit_interaction_lease=False,
):
    timeout_seconds = float(
        timeout_seconds
        if timeout_seconds is not None
        else os.environ.get("AGENT_DESKTOP_E2E_TIMEOUT_SECONDS", DEFAULT_TIMEOUT_SECONDS)
    )
    max_capture_bytes = int(
        max_capture_bytes
        if max_capture_bytes is not None
        else os.environ.get("AGENT_DESKTOP_E2E_MAX_CAPTURE_BYTES", DEFAULT_MAX_CAPTURE_BYTES)
    )
    if timeout_seconds <= 0 or max_capture_bytes <= 0:
        raise ValueError("timeout and capture limits must be positive")

    usage_before = resource.getrusage(resource.RUSAGE_CHILDREN)
    started = time.perf_counter()
    child_env = dict(os.environ if env is None else env)
    pass_fds = _interaction_lease_fds(child_env) if inherit_interaction_lease else ()
    if not inherit_interaction_lease:
        child_env.pop(INTERACTION_LEASE_FD_ENV, None)
    child_env.pop(INHERIT_INTERACTION_LEASE_ENV, None)
    process = subprocess.Popen(
        command,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
        env=child_env,
        pass_fds=pass_fds,
    )
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ, "stdout")
    selector.register(process.stderr, selectors.EVENT_READ, "stderr")
    chunks = {"stdout": [], "stderr": []}
    captured = 0
    deadline = started + timeout_seconds
    timed_out = False
    output_limited = False
    termination_error = None

    while selector.get_map():
        remaining = deadline - time.perf_counter()
        if remaining <= 0:
            timed_out = True
            termination_error = _merge_error(termination_error, _terminate_group(process))
            _close_streams(selector, process)
            break
        events = selector.select(min(remaining, 0.1))
        if not events:
            process.poll()
            continue
        for key, _ in events:
            data = os.read(key.fileobj.fileno(), 65536)
            if not data:
                selector.unregister(key.fileobj)
                key.fileobj.close()
                continue
            available = max_capture_bytes - captured
            if len(data) > available:
                if available > 0:
                    chunks[key.data].append(data[:available])
                    captured += available
                output_limited = True
                termination_error = _merge_error(termination_error, _terminate_group(process))
                _close_streams(selector, process)
                break
            chunks[key.data].append(data)
            captured += len(data)
        if output_limited:
            break

    selector.close()
    if not timed_out and not output_limited:
        remaining = max(0.0, deadline - time.perf_counter())
        try:
            process.wait(timeout=remaining)
        except subprocess.TimeoutExpired:
            timed_out = True
            termination_error = _merge_error(termination_error, _terminate_group(process))
    try:
        process.wait(timeout=1)
    except subprocess.TimeoutExpired:
        termination_error = _merge_error(termination_error, _terminate_group(process))
        try:
            process.wait(timeout=1)
        except subprocess.TimeoutExpired:
            termination_error = _merge_error(
                termination_error, "child could not be reaped after termination"
            )

    usage_after = resource.getrusage(resource.RUSAGE_CHILDREN)
    cpu_before = usage_before.ru_utime + usage_before.ru_stime
    cpu_after = usage_after.ru_utime + usage_after.ru_stime
    if timed_out:
        returncode = 124
    elif output_limited:
        returncode = 125
    elif process.returncode is None:
        returncode = 126
    else:
        returncode = int(process.returncode)
    return BoundedResult(
        args=list(command),
        returncode=returncode,
        stdout=b"".join(chunks["stdout"]).decode("utf-8", errors="replace"),
        stderr=b"".join(chunks["stderr"]).decode("utf-8", errors="replace"),
        timed_out=timed_out,
        output_limited=output_limited,
        termination_error=termination_error,
        wall_ms=(time.perf_counter() - started) * 1000,
        cpu_ms=max(0.0, (cpu_after - cpu_before) * 1000),
    )


def read_json():
    payload = sys.stdin.buffer.read(MAX_JSON_INPUT_BYTES + 1)
    if len(payload) > MAX_JSON_INPUT_BYTES:
        raise SystemExit("JSON input exceeds capture limit")
    return json.loads(payload)


def render(value):
    if isinstance(value, bool):
        print("True" if value else "False")
    elif value is None:
        print("")
    elif isinstance(value, (dict, list)):
        print(json.dumps(value, separators=(",", ":"), sort_keys=True))
    else:
        print(value)


def get_path(data, path):
    value = data
    for part in path.split("."):
        value = value[int(part)] if isinstance(value, list) else value[part]
    return value


def find_node(node, name):
    if node.get("name") == name:
        return node
    for child in node.get("children", []):
        found = find_node(child, name)
        if found is not None:
            return found
    return None


def find_drill_anchor(node):
    if node.get("children_count") and node.get("ref_id"):
        return node["ref_id"]
    for child in node.get("children", []):
        found = find_drill_anchor(child)
        if found:
            return found
    return None


def command_get(path):
    try:
        render(get_path(read_json(), path))
    except (KeyError, IndexError, TypeError, ValueError, json.JSONDecodeError):
        print("")


def command_target():
    data = read_json()
    if data.get("ok") is not True:
        raise SystemExit(2)
    body = data.get("data", {})
    match = body.get("match")
    if not isinstance(match, dict):
        raise SystemExit(3)
    ref_id = match.get("ref_id")
    snapshot_id = body.get("snapshot_id")
    if not ref_id or not snapshot_id:
        raise SystemExit(4)
    print(f"{ref_id}\t{snapshot_id}")


def command_match_value():
    data = read_json()
    match = data.get("data", {}).get("match")
    if data.get("ok") is not True or not isinstance(match, dict) or "value" not in match:
        raise SystemExit(2)
    print("" if match["value"] is None else match["value"])


def command_tree(name, mode):
    data = read_json()
    node = find_node(data.get("data", {}).get("tree", {}), name)
    if node is None:
        raise SystemExit(2)
    if mode == "ref":
        ref_id = node.get("ref_id")
        if not ref_id:
            raise SystemExit(3)
        print(ref_id)
        return
    if mode == "has-ref":
        render(bool(node.get("ref_id")))
        return
    bounds = node.get("bounds")
    if not isinstance(bounds, dict):
        raise SystemExit(4)
    if mode == "center":
        print(f"{bounds['x'] + bounds['width'] / 2},{bounds['y'] + bounds['height'] / 2}")
    else:
        y = bounds["y"] + bounds["height"] / 2
        print(f"{bounds['x'] + 20},{y} {bounds['x'] + bounds['width'] - 20},{y}")


def command_duplicate_ids(title):
    data = read_json()
    ids = [
        window.get("id")
        for window in data.get("data", [])
        if window.get("title") == title and window.get("id")
    ]
    if len(ids) != 2 or len(set(ids)) != 2:
        raise SystemExit(2)
    print("\t".join(ids))


def command_drill_anchor():
    anchor = find_drill_anchor(read_json().get("data", {}).get("tree", {}))
    if not anchor:
        raise SystemExit(2)
    print(anchor)


def command_window_focused(window_id):
    windows = read_json().get("data", [])
    match = next((window for window in windows if window.get("id") == window_id), None)
    if match is None:
        raise SystemExit(2)
    render(match.get("is_focused"))


def command_run(output_path, command):
    result = run_bounded(
        command,
        inherit_interaction_lease=os.environ.get(INHERIT_INTERACTION_LEASE_ENV) == "1",
    )
    payload = result.stdout if result.stdout.strip() else result.stderr
    with open(output_path, "w", encoding="utf-8") as output:
        output.write(payload)
    print(f"{result.returncode}\t{result.wall_ms:.1f}")


def command_exec(command):
    try:
        result = run_bounded(
            command,
            inherit_interaction_lease=os.environ.get(INHERIT_INTERACTION_LEASE_ENV) == "1",
        )
    except OSError as error:
        print(f"could not start command: {error}", file=sys.stderr)
        raise SystemExit(126) from error
    sys.stdout.write(result.stdout)
    sys.stderr.write(result.stderr)
    if result.timed_out:
        print("process group exceeded the absolute timeout", file=sys.stderr)
    if result.output_limited:
        print("process group exceeded the capture limit", file=sys.stderr)
    if result.termination_error:
        print(result.termination_error, file=sys.stderr)
    raise SystemExit(result.returncode)


def main():
    command = sys.argv[1]
    if command == "get":
        command_get(sys.argv[2])
    elif command == "target":
        command_target()
    elif command == "match-value":
        command_match_value()
    elif command == "tree":
        command_tree(sys.argv[2], sys.argv[3])
    elif command == "duplicate-ids":
        command_duplicate_ids(sys.argv[2])
    elif command == "drill-anchor":
        command_drill_anchor()
    elif command == "window-focused":
        command_window_focused(sys.argv[2])
    elif command == "run":
        command_run(sys.argv[2], sys.argv[3:])
    elif command == "exec":
        command_exec(sys.argv[2:])
    else:
        raise SystemExit(f"unknown command: {command}")


if __name__ == "__main__":
    main()
