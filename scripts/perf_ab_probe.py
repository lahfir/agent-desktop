#!/usr/bin/env python3
"""A/B latency probe: two agent-desktop binaries against one target app.

Modes:
  fixture  -- full probe incl. mutating actions; ONLY safe against the
              AgentDeskFixture app.
  app      -- strictly read-only observation (snapshot/find timing) for
              arbitrary real apps (Slack, Chrome, ...). Never dispatches
              actions.

Each binary runs under an isolated HOME so cross-revision store formats
never collide. Rounds alternate between binaries to cancel machine drift.
"""
import argparse
import json
import os
import statistics
import subprocess
import sys
import tempfile
import time

ENVS = {}


def env_for(binary):
    if binary not in ENVS:
        env = dict(os.environ)
        env["HOME"] = tempfile.mkdtemp(prefix="ad-perf-home-")
        ENVS[binary] = env
    return ENVS[binary]


def run(binary, *args, timeout=90):
    start = time.monotonic()
    proc = subprocess.run(
        [binary, *args], capture_output=True, text=True, timeout=timeout, env=env_for(binary)
    )
    elapsed = (time.monotonic() - start) * 1000
    return elapsed, proc


def envelope(proc):
    try:
        return json.loads(proc.stdout)
    except ValueError:
        return {}


def walk(node):
    yield node
    for child in node.get("children", []):
        yield from walk(child)


def tree_stats(env):
    tree = env.get("data", {}).get("tree", {})
    nodes = sum(1 for _ in walk(tree))
    depth = 0

    def measure(node, level):
        nonlocal depth
        depth = max(depth, level)
        for child in node.get("children", []):
            measure(child, level + 1)

    measure(tree, 1)
    return nodes, depth, env.get("data", {}).get("ref_count")


def fixture_refs(binary, app):
    _, proc = run(binary, "snapshot", "--app", app, "-i")
    env = envelope(proc)
    if "data" not in env:
        raise SystemExit(f"{binary}: fixture snapshot failed: {proc.stdout[:300]}")
    button = textfield = None
    for node in walk(env["data"]["tree"]):
        ref = node.get("ref") or node.get("ref_id")
        if not ref:
            continue
        name = (node.get("name") or "").lower()
        if node.get("role") == "button" and "primary" in name:
            button = button or ref
        if node.get("role") == "textfield":
            textfield = textfield or ref
    if not (button and textfield):
        raise SystemExit(f"{binary}: fixture targets missing (button={button}, field={textfield})")
    return button, textfield


def fixture_cases(binary, app):
    button, textfield = fixture_refs(binary, app)
    return [
        ("snapshot -i", ["snapshot", "--app", app, "-i"]),
        ("snapshot skeleton", ["snapshot", "--app", app, "--skeleton"]),
        ("snapshot d30", ["snapshot", "--app", app, "--max-depth", "30"]),
        ("get", ["get", button]),
        ("is visible", ["is", button, "--property", "visible"]),
        ("click", ["click", button]),
        ("set-value", ["set-value", textfield, "ab-probe"]),
        ("type", ["type", textfield, "x"]),
    ]


def app_cases(app):
    return [
        ("snapshot -i", ["snapshot", "--app", app, "-i"]),
        ("snapshot skeleton", ["snapshot", "--app", app, "--skeleton"]),
        ("snapshot d30", ["snapshot", "--app", app, "--max-depth", "30"]),
    ]


def collect(binaries, cases_for, rounds):
    cases = {label: cases_for(label) for label in binaries}
    results = {label: {} for label in binaries}
    shapes = {label: {} for label in binaries}
    for _ in range(rounds):
        for label, binary in binaries.items():
            for name, args in cases[label]:
                elapsed, proc = run(binary, *args)
                env = envelope(proc)
                ok = bool(env.get("ok"))
                results[label].setdefault(name, []).append((elapsed, ok))
                if ok and name.startswith("snapshot") and name not in shapes[label]:
                    shapes[label][name] = tree_stats(env)
    return results, shapes


def summarize(results, shapes):
    report = {}
    for label, cases in results.items():
        for name, samples in cases.items():
            times = sorted(t for t, ok in samples if ok)
            report.setdefault(name, {})[label] = {
                "p50_ms": round(statistics.median(times), 1) if times else None,
                "p95_ms": round(times[max(0, int(len(times) * 0.95) - 1)], 1) if times else None,
                "ok_rate": sum(1 for _, ok in samples if ok) / len(samples),
                "shape": shapes[label].get(name),
            }
    return report


def head_only_find(binary, app, rounds, report):
    times, matches = [], None
    for _ in range(rounds):
        elapsed, proc = run(binary, "find", "--app", app, "--role", "button", "--first")
        env = envelope(proc)
        if env.get("ok"):
            times.append(elapsed)
            matches = env.get("data", {}).get("match", {}).get("role")
    if times:
        times.sort()
        report["find --role button --first (HEAD-only)"] = {
            "HEAD": {
                "p50_ms": round(statistics.median(times), 1),
                "p95_ms": round(times[max(0, int(len(times) * 0.95) - 1)], 1),
                "ok_rate": len(times) / rounds,
                "matched_role": matches,
            }
        }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=["fixture", "app"], required=True)
    parser.add_argument("--app", required=True)
    parser.add_argument("--head-bin", required=True)
    parser.add_argument("--base-bin", required=True)
    parser.add_argument("--rounds", type=int, default=10)
    parser.add_argument("--json-out", default="")
    args = parser.parse_args()

    binaries = {"HEAD": args.head_bin, "BASE": args.base_bin}
    if args.mode == "fixture":
        results, shapes = collect(binaries, lambda label: fixture_cases(binaries[label], args.app), args.rounds)
    else:
        results, shapes = collect(binaries, lambda _label: app_cases(args.app), args.rounds)
    report = summarize(results, shapes)
    if args.mode == "app":
        head_only_find(args.head_bin, args.app, args.rounds, report)

    payload = {"mode": args.mode, "app": args.app, "rounds": args.rounds, "cases": report}
    if args.json_out:
        with open(args.json_out, "w") as handle:
            json.dump(payload, handle, indent=1)
    json.dump(payload, sys.stdout, indent=1)
    print()
    if any(side["ok_rate"] < 1.0 for case in report.values() for side in case.values()):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
