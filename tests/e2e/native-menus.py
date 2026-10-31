#!/usr/bin/env python3
"""Check native TextEdit menu behavior with no document window open."""

import argparse
import json
import os
import platform
import subprocess
import sys
import tempfile
from pathlib import Path


class Failure(RuntimeError):
    pass


def need(condition, message):
    if not condition:
        raise Failure(message)


def osa(source):
    result = subprocess.run(
        ["/usr/bin/osascript", "-e", source], capture_output=True, text=True, timeout=15
    )
    if result.returncode:
        raise Failure(result.stderr.strip() or "AppleScript failed")
    return result.stdout.strip()


def textedit_running():
    result = subprocess.run(["/usr/bin/pgrep", "-x", "TextEdit"], capture_output=True)
    need(result.returncode in (0, 1), "could not check whether TextEdit is running")
    return result.returncode == 0


def process_name(pid):
    return osa(f'''tell application "System Events"
set matches to every process whose unix id is {pid}
if (count of matches) is 0 then return ""
return name of item 1 of matches as text
end tell''')


def require_owned(pid):
    need(process_name(pid) == "TextEdit", "task-created TextEdit process changed")


def frontmost():
    value = osa('''tell application "System Events"
set matches to every process whose frontmost is true
if (count of matches) is 0 then return ""
set p to item 1 of matches
return (name of p as text) & tab & (unix id of p as text)
end tell''')
    if not value:
        return None
    name, pid = value.split("\t", 1)
    return name, int(pid)


def documents():
    value = osa('''tell application id "com.apple.TextEdit"
set docs to every document
set resultText to count of docs as text
repeat with d in docs
set resultText to resultText & linefeed & (name of d as text) & tab & ((modified of d) as text) & tab & ((length of (text of d as text)) as text)
end repeat
return resultText
end tell''')
    lines = value.splitlines()
    need(bool(lines), "TextEdit document state was empty")
    result = [tuple((parts[0], parts[1].lower() == "true", int(parts[2])))
              for parts in (line.split("\t", 2) for line in lines[1:])]
    need(int(lines[0]) == len(result), "TextEdit document inventory was inconsistent")
    return result


def ax_windows(pid):
    return int(osa(f'''tell application "System Events"
set matches to every process whose unix id is {pid}
if (count of matches) is 0 then return "0"
return count of windows of item 1 of matches as text
end tell'''))


def require_windowless(pid):
    require_owned(pid)
    need(ax_windows(pid) == 0, "TextEdit acquired an AX window during the regression")


def menu_state(pid):
    value = osa(f'''tell application "System Events"
set p to first process whose unix id is {pid}
set menuItems to every menu bar item of menu bar 1 of p
set names to {{}}
set selectedNames to {{}}
repeat with menuItem in menuItems
set end of names to name of menuItem as text
try
if selected of menuItem then set end of selectedNames to name of menuItem as text
end try
end repeat
set AppleScript's text item delimiters to linefeed
return (names as text) & tab & (selectedNames as text)
end tell''')
    names, _, selected = value.partition("\t")
    return names.splitlines(), selected.splitlines() if selected else []


def cancel_open_menus(pid):
    value = osa(f'''tell application "System Events"
set p to first process whose unix id is {pid}
set selectedItems to every menu bar item of menu bar 1 of p whose selected is true
set cancelCount to 0
repeat with selectedItem in selectedItems
set openMenu to menu 1 of selectedItem
set actionNames to name of every action of openMenu
if actionNames contains "AXCancel" then
set ignoredResult to perform action "AXCancel" of openMenu
set cancelCount to cancelCount + 1
end if
end repeat
set selectedItems to every menu bar item of menu bar 1 of p whose selected is true
return (cancelCount as text) & tab & (count of selectedItems as text)
end tell''')
    _, remaining = value.split("\t", 1)
    need(int(remaining) == 0, "a native menu remains selected after AXCancel")


def cli(binary, env, *args):
    try:
        result = subprocess.run(
            [str(binary), *args], capture_output=True, text=True, timeout=60, env=env
        )
    except subprocess.TimeoutExpired as error:
        raise Failure(f"agent-desktop timed out: {' '.join(args)}") from error
    try:
        return result.returncode, json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise Failure(result.stderr.strip() or "agent-desktop returned non-JSON output") from error


def ok(result, label):
    code, value = result
    need(code == 0 and value.get("ok") is True, f"{label} failed: {value}")
    return value


def walk(node):
    yield node
    for child in node.get("children", []):
        yield from walk(child)


def refs_from(snapshot):
    data = snapshot.get("data", {})
    need(data.get("app") == "TextEdit" and "window" not in data, "menubar snapshot was not app-owned")
    need(data.get("complete") is True, "menubar snapshot was incomplete")
    found = {name: [] for name in ("File", "Edit", "Help")}
    for node in walk(data.get("tree", {})):
        if node.get("role") == "menuitem" and node.get("name") in found:
            found[node["name"]].append(node.get("ref_id"))
    refs = {}
    for name, matches in found.items():
        need(len(matches) == 1 and isinstance(matches[0], str)
             and matches[0].startswith("@") and ":" in matches[0], f"missing qualified {name} ref")
        refs[name] = matches[0]
    need(len({ref.split(":", 1)[0] for ref in refs.values()}) == 1, "menu refs came from different snapshots")
    return refs


def menu_id(binary, env):
    value = ok(cli(binary, env, "list-surfaces", "--app", "TextEdit"), "list-surfaces")
    menus = [item for item in value.get("data", {}).get("surfaces", []) if item.get("type") == "menu"]
    need(len(menus) == 1 and isinstance(menus[0].get("id"), str), "expected one native menu surface")
    return menus[0]["id"]


def close_blank_document(pid):
    require_owned(pid)
    value = osa('''tell application id "com.apple.TextEdit"
if (count of documents) is not 1 then error "document count changed"
set d to first document
if (name of d as text) is not "Untitled" then error "document is not the new untitled document"
if modified of d then error "document was modified"
if (length of (text of d as text)) is not 0 then error "document is not blank"
close d saving no
return "closed"
end tell''')
    need(value == "closed", "TextEdit did not confirm closing the blank document")


def cleanup(binary, env, pid, close_attempted):
    require_owned(pid)
    cancel_open_menus(pid)
    name = process_name(pid)
    if name:
        need(name == "TextEdit", "task-created TextEdit process changed")
        docs = documents()
        if docs:
            if close_attempted or docs != [("Untitled", False, 0)]:
                raise Failure("TextEdit left running because a document was added, changed, or was not blank")
            close_blank_document(pid)
            if process_name(pid):
                require_owned(pid)
                need(documents() == [], "new blank document remained open after close")
    need(ax_windows(pid) == 0, "TextEdit still has AX windows")
    if process_name(pid):
        require_owned(pid)
        need(documents() == [], "TextEdit opened a document before graceful quit")
        need(ax_windows(pid) == 0, "TextEdit gained an AX window before graceful quit")
        code, value = cli(binary, env, "close-app", "TextEdit")
        need(code == 0 and value.get("ok") is True, f"graceful close-app failed: {value}")
        need(process_name(pid) == "", "task-created TextEdit did not quit")
    return "task-created TextEdit quit after the zero-window check"


def foreground_unchanged(previous, where):
    need(frontmost() == previous, f"foreground changed after {where}; no restoration was attempted")


def run(binary):
    need(platform.system() == "Darwin", "this regression runs only on macOS")
    need(binary.is_absolute() and binary.is_file() and os.access(binary, os.X_OK), "--bin must be an executable absolute path")
    need(textedit_running() is False, "TextEdit is already running; close it before this manual regression")
    previous = frontmost()
    failures = []
    pid = None
    close_attempted = False

    with tempfile.TemporaryDirectory(prefix="agent-desktop-native-menus-") as state_home:
        env = os.environ.copy()
        env["AGENT_DESKTOP_HOME"] = state_home
        env.pop("AGENT_DESKTOP_SESSION", None)
        env.pop("AGENT_DESKTOP_AGENT_ID", None)
        try:
            need(textedit_running() is False, "TextEdit started during preflight; no app action was attempted")
            launch = ok(cli(binary, env, "launch", "TextEdit"), "quiet TextEdit launch")
            pid = int(launch.get("data", {}).get("pid"))
            require_owned(pid)
            foreground_unchanged(previous, "launch")

            need(documents() == [("Untitled", False, 0)], "expected only TextEdit's new blank unmodified document")
            close_attempted = True
            close_blank_document(pid)
            if process_name(pid):
                require_owned(pid)
                need(documents() == [], "TextEdit still has a document after the guarded close")
            need(ax_windows(pid) == 0, "TextEdit did not reach zero AX windows")
            foreground_unchanged(previous, "blank-document close")

            require_windowless(pid)
            snapshot = ok(cli(binary, env, "snapshot", "--app", "TextEdit", "--surface", "menubar"), "menubar snapshot")
            refs = refs_from(snapshot)
            require_windowless(pid)
            help_result = ok(cli(binary, env, "--wait-for", "menuitem:TextEdit Help", "--wait-timeout", "2000", "click", refs["Help"]), "headless Help click")
            help_data = help_result.get("data", {})
            need("window" not in help_data, "Help click returned data.window")
            need(any(node.get("role") == "menuitem" and node.get("name") == "TextEdit Help" for node in walk(help_data.get("tree", {}))), "Help wait returned without the open submenu item")
            names, selected = menu_state(pid)
            need("Help" in names and selected == ["Help"], f"native AX did not show Help selected: {names}, {selected}")
            foreground_unchanged(previous, "Help click")

            require_windowless(pid)
            code, result = cli(binary, env, "--wait-for-gone", "menuitem:File", "--wait-timeout", "600", "click", refs["File"])
            error = result.get("error", {})
            need(code != 0 and result.get("ok") is False and error.get("code") == "TIMEOUT", f"File wait did not time out: {result}")
            need(error.get("disposition", {}).get("retry") == "unsafe", f"File click was not marked unsafe: {result}")
            need(not result.get("data", {}).get("gone"), "File wait reported false absence")
            names, _ = menu_state(pid)
            need("File" in names, "native AX no longer lists File")
            foreground_unchanged(previous, "File wait")

            require_windowless(pid)
            ok(cli(binary, env, "click", refs["Edit"]), "fresh Edit menu action")
            names, selected = menu_state(pid)
            need("Edit" in names and selected == ["Edit"], f"native AX did not select Edit: {names}, {selected}")
            edit_id = menu_id(binary, env)
            need(menu_id(binary, env) == edit_id, "Edit surface ID changed between reads")

            require_windowless(pid)
            ok(cli(binary, env, "click", refs["File"]), "File menu switch from Edit")
            names, selected = menu_state(pid)
            need("File" in names and selected == ["File"], f"native AX did not select File: {names}, {selected}")
            file_id = menu_id(binary, env)
            need(menu_id(binary, env) == file_id, "File surface ID changed between reads")
            need(file_id != edit_id, f"File and Edit reused surface ID {file_id}")
            foreground_unchanged(previous, "menu inventory")
        except Exception as error:
            failures.append(str(error))
        finally:
            if pid is None:
                failures.append("launch returned no safe process ID; app cleanup was not attempted")
            else:
                try:
                    print(cleanup(binary, env, pid, close_attempted))
                except Exception as error:
                    failures.append(f"cleanup: {error}")
            try:
                foreground_unchanged(previous, "cleanup")
            except Exception as error:
                failures.append(str(error))

    for failure in failures:
        print(f"FAIL: {failure}", file=sys.stderr)
    if failures:
        return 1
    print("PASS: native TextEdit menu regression passed")
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin", required=True, help="absolute path to the built agent-desktop binary")
    return run(Path(parser.parse_args().bin))


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as error:
        print(f"FAIL: {error}", file=sys.stderr)
        sys.exit(2)
