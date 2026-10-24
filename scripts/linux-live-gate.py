#!/usr/bin/env python3
"""Shard libtest cases across private desktops and require complete receipts."""

import argparse
from collections import Counter, defaultdict
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import time


ROOT = Path(__file__).resolve().parent.parent
from linux_live_gate_rules import (
    BARE_CASES, CLI_WAYLAND, CURSOR_WAYLAND, EXTERNAL_WAYLAND, HELPER_PARENTS,
    MUTTER_MODULES, OPENBOX_CASES, PARALLEL_READS, TIERS, WAYLAND_MODULES,
)

RESULT = re.compile(r"^(ok|failed|ignored) (\S+)\s*$", re.MULTILINE)
ACTIVE = []
STOPPING = False


def stop_workers(_signal, _frame):
    global STOPPING
    STOPPING = True
    for process in list(ACTIVE):
        try:
            os.killpg(process.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass


def read_tests(binary, ignored=False):
    command = [str(binary), "--list"] + (["--ignored"] if ignored else [])
    output = subprocess.check_output(command, text=True)
    names = [line[:-6] for line in output.splitlines() if line.endswith(": test")]
    if not names or len(names) != len(set(names)):
        raise ValueError("empty or duplicate libtest inventory")
    return sorted(names)


def build():
    env = dict(os.environ, CARGO_INCREMENTAL="0")
    subprocess.run(["cargo", "build", "-p", "agent-desktop", "-p", "agent-desktop-linux",
                    "--bins"], env=env, check=True)
    result = subprocess.run(
        ["cargo", "test", "-p", "agent-desktop-linux", "--lib", "--no-run",
         "--message-format=json"], env=env, text=True, stdout=subprocess.PIPE,
    )
    rows = [json.loads(line) for line in result.stdout.splitlines()]
    for row in rows:
        if row.get("reason") == "compiler-message":
            print(row["message"]["rendered"], file=sys.stderr, end="")
    result.check_returncode()
    binaries = [row["executable"] for row in rows
                if row.get("reason") == "compiler-artifact" and row.get("executable")
                and row["target"]["name"] == "agent_desktop_linux"]
    if len(binaries) != 1:
        raise ValueError(f"expected one Linux library test executable, got {binaries}")
    return Path(binaries[0])


def tiers_for(name, ignored):
    if name not in ignored:
        return ("openbox",)
    if name in EXTERNAL_WAYLAND or any(name.startswith(module + "::") for module in WAYLAND_MODULES):
        return ("wayland",)
    if name in OPENBOX_CASES:
        return ("openbox",)
    if any(name.startswith(module + "::") for module in MUTTER_MODULES):
        return ("mutter",)
    return ("openbox", "mutter")


def validate_tiers(names):
    required = set((*EXTERNAL_WAYLAND, *BARE_CASES, *OPENBOX_CASES, *HELPER_PARENTS,
                    *HELPER_PARENTS.values()))
    missing = required - set(names)
    modules = (*WAYLAND_MODULES, *MUTTER_MODULES)
    missing.update(module for module in modules if not any(
        name.startswith(module + "::") for name in names))
    if missing:
        raise ValueError(f"tier rules reference missing tests or modules: {sorted(missing)}")


def plan(names, ignored, jobs):
    groups = {tier: defaultdict(list) for tier in TIERS}
    for name in names:
        for tier in tiers_for(name, ignored):
            module = name.rsplit("::", 1)[0]
            if name in (*BARE_CASES, *EXTERNAL_WAYLAND):
                module = name
            groups[tier][module].append(name)
    shards = {}
    for tier in TIERS:
        loads = [0] * jobs
        shards[tier] = [[] for _ in range(jobs)]
        tasks = []
        for module, cases in sorted(groups[tier].items()):
            # Each portal case boots its own compositor; keep those independent boots parallel.
            width = max(1, (len(cases) + jobs - 1) // jobs) if tier == "wayland" else len(cases)
            for offset in range(0, len(cases), width):
                selected = cases[offset:offset + width]
                weight = sum(20 if name in ignored else 0.1 for name in selected)
                tasks.append((weight, module, selected))
        for weight, module, cases in sorted(tasks, reverse=True):
            # Native Wayland fixtures use their own portal sessions/local tokens, not
            # the cross-process interaction lease. X11 mutations must share a writer.
            readers = module in PARALLEL_READS or module.startswith("system::capture::")
            candidates = range(jobs) if tier == "wayland" else (
                range(1, jobs) if jobs > 1 and readers and all(name in ignored for name in cases)
                else (0,))
            shard = min(candidates, key=lambda index: loads[index])
            shards[tier][shard].append({"module": module, "tests": cases})
            loads[shard] += weight
    expected = Counter((tier, name) for name in names for tier in tiers_for(name, ignored))
    assigned = Counter((tier, name) for tier, workers in shards.items()
                       for tasks in workers for task in tasks for name in task["tests"])
    if assigned != expected:
        raise ValueError("shards do not cover the tier inventory exactly once")
    return shards


def save(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def select_tests(names, ignored, modules, ignored_only):
    selected = [name for name in names if (not ignored_only or name in ignored)
                and (not modules or any(name.startswith(module + "::") for module in modules))]
    missing = [module for module in modules if not any(
        name.startswith(module + "::") for name in selected)]
    if not selected or missing:
        raise ValueError(f"empty test selection or unmatched modules: {missing}")
    return selected


def source_hash():
    files = [ROOT / "Cargo.lock", ROOT / "Cargo.toml", ROOT / "rust-toolchain.toml"]
    if (ROOT / ".cargo/config.toml").exists():
        files.append(ROOT / ".cargo/config.toml")
    for folder in (ROOT / "crates", ROOT / "src"):
        files.extend(folder.rglob("*.rs"))
        files.extend(folder.rglob("Cargo.toml"))
    files.extend(ROOT / "scripts" / name for name in (
        "linux-live-gate.py", "linux_live_gate_rules.py", "linux-live-gate.sh", "private-session.sh",
        "private-mutter-shell.sh", "private-wayland-shell.sh"))
    digest = hashlib.sha256()
    for path in sorted(files):
        digest.update(str(path.relative_to(ROOT)).encode())
        digest.update(path.read_bytes())
    return digest.hexdigest()


def merge(manifest_path):
    manifest = json.loads(manifest_path.read_text())
    actual = Counter()
    summaries = []
    passed = True
    for tier in TIERS:
        directory = manifest_path.parent / tier
        summary = json.loads((directory / "summary.json").read_text())
        summaries.append(summary)
        passed &= summary["passed"]
        for shard in range(len(manifest["shards"][tier])):
            if not manifest["shards"][tier][shard]:
                continue
            receipt = directory / f"shard-{shard}.json"
            if not receipt.exists():
                passed = False
                continue
            for row in json.loads(receipt.read_text()):
                passed &= row["passed"]
                actual.update((tier, name) for name, status in row["results"] if status == "ok")
    ignored = set(manifest["ignored"])
    expected = Counter((tier, name) for name in manifest["tests"]
                       for tier in tiers_for(name, ignored))
    passed &= actual == expected
    helpers = {name: parent for name, parent in HELPER_PARENTS.items() if name in manifest["tests"]}
    for helper, parent in helpers.items():
        passed &= all(actual[tier, parent] == 1 for tier in tiers_for(helper, ignored))
    report = dict(passed=passed, complete=actual == expected, tiers=summaries,
                  unique_tests=len(manifest["tests"]), ignored_tests=len(ignored),
                  helpers_covered_by_parent=helpers,
                  binary_sha256=manifest["binary_sha256"], source_sha256=manifest["source_sha256"])
    save(manifest_path.parent / "summary.json", report)
    print(json.dumps(report), flush=True)
    return passed


def run_bounded(command, log, timeout):
    if STOPPING:
        return 130, 0.0
    started = time.monotonic()
    with log.open("w") as stream:
        process = subprocess.Popen(command, stdout=stream, stderr=subprocess.STDOUT,
                                   start_new_session=True)
        ACTIVE.append(process)
        try:
            while True:
                try:
                    code = process.wait(timeout=1)
                    break
                except subprocess.TimeoutExpired:
                    if STOPPING or time.monotonic() - started >= timeout:
                        raise
        except (subprocess.TimeoutExpired, KeyboardInterrupt):
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
            code = 124
        finally:
            ACTIVE.remove(process)
    return code, round(time.monotonic() - started, 3)


def worker(manifest_path, tier, shard):
    manifest = json.loads(manifest_path.read_text())
    if "cli" in manifest:
        os.environ["AGENT_DESKTOP_TEST_BINARY"] = manifest["cli"]
    directory = manifest_path.parent / tier
    save(directory / f"session-{shard}.json", {key: os.environ.get(key) for key in (
        "DISPLAY", "DBUS_SESSION_BUS_ADDRESS", "XDG_RUNTIME_DIR",
        "AGENT_DESKTOP_PRIVATE_SESSION", "AGENT_DESKTOP_HOME", "TMPDIR")})
    rows = []
    for index, task in enumerate(manifest["shards"][tier][shard]):
        if STOPPING:
            return 130
        log = directory / f"shard-{shard}-module-{index}.log"
        results = log.with_suffix(".results")
        command = [manifest["binary"], "--include-ignored", "--test-threads=1",
                   "--logfile", str(results), "--exact",
                   *task["tests"]]
        if any(name in EXTERNAL_WAYLAND for name in task["tests"]):
            command = ["bash", str(ROOT / "scripts/private-wayland-shell.sh"), "--", *command]
        if any(name in BARE_CASES for name in task["tests"]):
            command = ["bash", str(ROOT / "scripts/private-session.sh"), "--", *command]
        code, seconds = run_bounded(command, log, manifest["timeout"])
        reported = [(name, status) for status, name in RESULT.findall(
            results.read_text(errors="replace") if results.exists() else "")]
        observed = Counter(name for name, _ in reported)
        complete = observed == Counter(task["tests"])
        passed = complete and code == 0 and all(status == "ok" for _, status in reported)
        row = dict(task, exit_code=code, seconds=seconds, complete=complete, passed=passed,
                   results=reported, log=str(log), helpers_covered_by_parent={
                       name: HELPER_PARENTS[name] for name in task["tests"] if name in HELPER_PARENTS})
        rows.append(row)
        save(directory / f"shard-{shard}.json", rows)
        print(f"{tier}/{shard} {task['module']}: {len(reported)}/{len(task['tests'])} "
              f"{'PASS' if passed else 'FAIL'} {seconds}s", flush=True)
    return 0 if all(row["passed"] for row in rows) else 1


def run_tier(manifest_path, tier):
    manifest = json.loads(manifest_path.read_text())
    directory = manifest_path.parent / tier
    directory.mkdir()
    started = time.monotonic()

    def launch(shard):
        command = ["bash", str(ROOT / "scripts/private-session.sh")]
        if tier in ("openbox", "wayland"):
            command.append("--wm")
        command.append("--")
        if tier == "mutter":
            command += ["env", "AGENT_DESKTOP_LIVE_WM=mutter", "bash",
                        str(ROOT / "scripts/private-mutter-shell.sh"), "--"]
        command += [sys.executable, str(Path(__file__).resolve()), "--worker",
                    str(manifest_path), tier, str(shard)]
        return run_bounded(command, directory / f"shard-{shard}.log", manifest["timeout"])

    with ThreadPoolExecutor(max_workers=len(manifest["shards"][tier])) as pool:
        statuses = list(pool.map(launch, range(len(manifest["shards"][tier]))))
    rows = []
    for shard in range(len(statuses)):
        receipt = directory / f"shard-{shard}.json"
        if receipt.exists():
            rows.extend(json.loads(receipt.read_text()))
    expected = Counter(name for tasks in manifest["shards"][tier]
                       for task in tasks for name in task["tests"])
    actual = Counter(name for row in rows for name, _ in row["results"])
    counts = Counter(status for row in rows for _, status in row["results"])
    passed = all(code == 0 for code, _ in statuses) and actual == expected and all(
        row["passed"] for row in rows)
    summary = dict(tier=tier, passed=passed, seconds=round(time.monotonic() - started, 3),
                   expected=sum(expected.values()), counts=dict(counts), shards=statuses,
                   standalone_helpers=sum(expected[t] for t in HELPER_PARENTS),
                   complete=actual == expected,
                   serial_module_seconds=round(sum(row["seconds"] for row in rows), 3))
    save(directory / "summary.json", summary)
    print(json.dumps(summary), flush=True)
    return passed


def main():
    signal.signal(signal.SIGINT, stop_workers)
    signal.signal(signal.SIGTERM, stop_workers)
    if len(sys.argv) > 1 and sys.argv[1] == "--worker":
        return worker(Path(sys.argv[2]), sys.argv[3], int(sys.argv[4]))
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tier", choices=("all", *TIERS), default="all")
    parser.add_argument("--jobs", type=int, choices=range(1, 4), default=3)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--timeout", type=int, default=780)
    parser.add_argument("--binary", type=Path, help="use an already built library test executable")
    parser.add_argument("--list", action="store_true", help="write the manifest without starting sessions")
    parser.add_argument("--module", action="append", default=[], help="select a Rust module prefix; repeatable")
    parser.add_argument("--ignored-only", action="store_true", help="select only ignored live tests")
    parser.add_argument("--merge", action="store_true", help="verify and merge three completed tier receipts")
    args = parser.parse_args()
    if args.timeout <= 0:
        parser.error("--timeout must be positive")
    os.chdir(ROOT)
    base = Path(
        os.environ.get("AGENT_DESKTOP_SESSION_BASE")
        or (Path(os.environ["XDG_RUNTIME_DIR"]) / "agent-desktop-sessions"
            if os.environ.get("XDG_RUNTIME_DIR") else Path(f"/tmp/agent-desktop-sessions-{os.getuid()}"))
    )
    if args.output is None or not (args.merge or args.list):
        base.mkdir(mode=0o700, parents=True, exist_ok=True)
    output = (args.output or base / f"live-gate-{time.time_ns()}").resolve()
    output.mkdir(parents=True, exist_ok=True)
    manifest_path = output / "manifest.json"
    if manifest_path.exists():
        manifest = json.loads(manifest_path.read_text())
        if manifest["source_sha256"] != source_hash():
            raise ValueError("source changed since the manifest was prepared; use a new output directory")
        if hashlib.sha256(Path(manifest["binary"]).read_bytes()).hexdigest() != manifest["binary_sha256"]:
            raise ValueError("the frozen test executable changed")
        for path, expected in manifest["artifacts"].items():
            if hashlib.sha256(Path(path).read_bytes()).hexdigest() != expected:
                raise ValueError(f"the frozen artifact changed: {path}")
        if args.jobs != len(manifest["shards"]["openbox"]) or args.timeout != manifest["timeout"]:
            raise ValueError("output contains a different shard or deadline configuration")
        if manifest.get("selection", {}) != dict(modules=args.module, ignored_only=args.ignored_only):
            raise ValueError("output contains a different test selection")
    else:
        if args.merge:
            parser.error("--merge requires --output with an existing manifest")
        binary = (args.binary or build()).resolve()
        names = read_tests(binary)
        ignored = set(read_tests(binary, ignored=True))
        if not ignored <= set(names):
            raise ValueError("ignored tests are missing from the complete inventory")
        validate_tiers(names)
        names = select_tests(names, ignored, args.module, args.ignored_only)
        ignored.intersection_update(names)
        frozen = output / "debug/deps/libtest"
        frozen.parent.mkdir(parents=True)
        shutil.copy2(binary, frozen)
        profile = binary.parent.parent
        artifacts = {}
        for name in ("agent-desktop", "agent-desktop-linux-clipboard-owner"):
            target = output / "debug" / name
            shutil.copy2(profile / name, target)
            artifacts[str(target)] = hashlib.sha256(target.read_bytes()).hexdigest()
        manifest = dict(binary=str(frozen), binary_sha256=hashlib.sha256(frozen.read_bytes()).hexdigest(),
                        cli=str(output / "debug/agent-desktop"), artifacts=artifacts,
                        source_sha256=source_hash(), tests=names, ignored=sorted(ignored),
                        selection=dict(modules=args.module, ignored_only=args.ignored_only),
                        timeout=args.timeout, shards=plan(names, ignored, args.jobs))
        save(manifest_path, manifest)
    print(f"Manifest: {manifest_path}; {len(manifest['tests'])} tests, "
          f"{len(manifest['ignored'])} ignored", flush=True)
    if args.merge:
        return 0 if merge(manifest_path) else 1
    if args.list:
        return 0
    tiers = TIERS if args.tier == "all" else (args.tier,)
    success = [run_tier(manifest_path, tier) for tier in tiers]
    if all((output / tier / "summary.json").exists() for tier in TIERS):
        success.append(merge(manifest_path))
    return 0 if all(success) else 1


if __name__ == "__main__":
    sys.exit(main())
