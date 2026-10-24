#!/usr/bin/env python3
"""Check probes/linux/FINDINGS.md: row shape, vocabularies, unique ids, deferral closure, and no box identity."""
import argparse
import os
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
STACKS = {"dbus", "gi-atspi", "tool"}
SCOPES = {"api-contract", "app", "box"}
VERDICT = re.compile(r"(CONFIRMS|CONTRADICTS|NEW-EDGE|DEFERRED)(?: \([^()]*\))?")
ROW = re.compile(r"^\|\s*(L\d+-\d+)\s*\|")
SEPARATOR = re.compile(r"^\|[\s:|-]*-[\s:|-]*\|?\s*$")
CLOSURE = re.compile(r"closure:\s*3\.(?:1[0-5]|[0-9])(?:\.\d+)?(?!\d)")
IDENTITY = [
    (re.compile(r"/home/\w+"), "home path"),
    (re.compile(r"(?<![\d.])100\.(?:6[4-9]|[7-9]\d|1[01]\d|12[0-7])\.\d{1,3}\.\d{1,3}(?![\d])"), "tailnet address"),
    (re.compile(r"\.ts\.net\b"), "tailnet host name"),
    (re.compile(r"(?<![0-9a-fA-F])[0-9a-fA-F]{32}(?![0-9a-fA-F])"), "machine-id style token"),
]


def split_cells(row):
    cells, current, in_code, escaped = [], [], False, False
    for char in row.strip():
        if escaped:
            current.append(char)
            escaped = False
        elif char == "\\":
            current.append(char)
            escaped = True
        elif char == "`":
            in_code = not in_code
            current.append(char)
        elif char == "|" and not in_code:
            cells.append("".join(current).strip())
            current = []
        else:
            current.append(char)
    cells.append("".join(current).strip())
    return cells[1:-1]


def check_row(cells):
    if len(cells) != 8:
        return [f"expected 8 cells, found {len(cells)}"]
    _, _, stack, scope, _, _, verdict, action = cells
    problems = []
    if not stack or set(stack.split(" + ")) - STACKS:
        problems.append(f"stack {stack!r} is not dbus, gi-atspi, tool or a ' + ' combination")
    if scope not in SCOPES:
        problems.append(f"scope {scope!r} is not api-contract, app or box")
    match = VERDICT.fullmatch(verdict)
    if not match:
        problems.append(f"verdict {verdict!r} is not CONFIRMS, CONTRADICTS, NEW-EDGE or DEFERRED, optionally with one parenthetical note")
    if match and match.group(1) == "DEFERRED" and not ("needs:" in action and CLOSURE.search(action)):
        problems.append("DEFERRED action needs 'needs:' and 'closure: 3.<n>'")
    return problems


def table_rows(text):
    rows, invalid, in_table, header = [], [], False, ""
    for number, line in enumerate(text.splitlines(), 1):
        if not line.startswith("|"):
            in_table, header = False, ""
        elif SEPARATOR.match(line):
            in_table = split_cells(header)[:1] == ["id"]
        elif not in_table:
            header = line
        elif match := ROW.match(line):
            rows.append((match.group(1), split_cells(line)))
        else:
            invalid.append((f"line {number}", "findings row without a valid L<n>-<n> id"))
    return rows, invalid


def identity_problems(text):
    patterns = list(IDENTITY)
    for token in filter(None, (t.strip() for t in os.environ.get("AGENT_DESKTOP_BOX_IDENTITY", "").split(","))):
        patterns.append((re.compile(re.escape(token), re.I), "configured box identity token"))
    return [f"box identity ({label}) present" for pattern, label in patterns if pattern.search(text)]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--ledger", type=Path, default=ROOT / "probes/linux/FINDINGS.md")
    parser.add_argument("--sub-phase")
    args = parser.parse_args()
    if not args.ledger.exists():
        print("check-linux-findings: no Linux ledger, nothing to check")
        return 0
    text = args.ledger.read_text()
    rows, invalid = table_rows(text)
    if args.sub_phase:
        named = re.compile(re.escape(args.sub_phase) + r"(?!\d|\.\d)")
        for row_id, cells in rows:
            if len(cells) == 8 and named.search(cells[7]):
                print(f"{row_id}\t{cells[6]}\t{cells[7]}")
        return 0
    violations, seen = list(invalid), set()
    for row_id, cells in rows:
        if row_id in seen:
            violations.append((row_id, "duplicate id"))
        seen.add(row_id)
        violations += [(row_id, p) for p in check_row(cells)]
    violations += [(args.ledger.name, p) for p in identity_problems(text)]
    for where, problem in violations:
        print(f"check-linux-findings: {where}: {problem}")
    if violations:
        return 1
    print(f"check-linux-findings: OK, {len(rows)} rows")
    return 0


if __name__ == "__main__":
    sys.exit(main())
