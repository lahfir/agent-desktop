"""Snapshot promises checked against widget events and independent AT-SPI reads."""

import contextlib
import json
from uuid import uuid4

from oracle.atspi_raw import RawNode, poll
from runner.helper import managed, nodes, one, require, root, snapshot as snapshot_tree
from scenarios.ref_contract_identity import identify, raw_target, same_ref


ACTIONS = {"Click": "click", "Toggle": "toggle", "Check": "check",
           "Uncheck": "uncheck", "TypeText": "type", "SetValue": "set-value",
           "Select": "select", "Expand": "expand", "Collapse": "collapse"}
BLOCKED = {"hidden", "offscreen", "disabled"}
OPENERS = {"zoo-open-dialog": "Zoo Dialog", "zoo-open-chooser": "Zoo Save",
           "zoo-open-message": "Zoo Message",
           "zoo-open-toplevel-button": "Zoo Top Button"}


def expected_outcomes(node):
    """Only the snapshot decides which commands are promised and may refuse."""
    actions = [ACTIONS[a] for a in node.get("available_actions", ()) if a in ACTIONS]
    states = set(node.get("states", ()))
    expected = "land"
    if states & BLOCKED:
        actions = actions[:1]
        expected = ("land-or-refuse" if node.get("name") == "zoo-offscreen"
                    and states & BLOCKED == {"offscreen"} else "refuse")
    return [(action, expected) for action in actions]


def row_verdict(expected, *, refused, log_effect, raw_effect, log_unchanged, raw_unchanged):
    landed = log_effect and raw_effect
    clean_refusal = refused and log_unchanged and raw_unchanged
    return "pass" if ((expected == "land" and landed)
                      or (expected == "refuse" and clean_refusal)
                      or (expected == "land-or-refuse" and (landed or clean_refusal))) else "fail"


def events(path):
    return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []


def since_reset(path):
    records = events(path)
    start = max((i + 1 for i, e in enumerate(records)
                 if e.get("widget") == "zoo" and e.get("event") == "reset"), default=0)
    return records[start:]


def raw_state(tree):
    # Paths identify duplicates; read interfaces directly, without the product.
    return tuple(sorted((node.path, node.name, node.role, tuple(sorted(node.states)),
                         node.text(), node.value()) for node in tree.walk()))


def reset(path):
    before = len(events(path))
    path.with_name(path.name + ".reset").touch()
    require(poll(lambda: any(e.get("widget") == "zoo" and e.get("event") == "reset"
                             for e in events(path)[before:]), 4), "fixture reset not acknowledged")


def ref_nodes(data):
    return [node for node in nodes(data.get("tree", data)) if node.get("ref_id")]


def snapshot(ctx, app, *extra):
    return identify(ctx.tier.state_root, snapshot_tree(ctx, app, *extra))


def named(data, name, role=None):
    names = (name,) if isinstance(name, str) else name
    matches = [node for node in ref_nodes(data) if node.get("name") in names
               and (role is None or node.get("role") == role)]
    require(len(matches) == 1, f"expected one snapshot ref for {name}: {matches}")
    return matches[0]


def fresh(ctx, app, original):
    return same_ref(snapshot(ctx, app), original)


def act(ctx, node, action, value=None):
    argv = [action, node["ref_id"]]
    if value is not None:
        argv.append(str(value))
    return ctx.product(argv, headed=False)


@contextlib.contextmanager
def zoo(ctx):
    # Supply the log argument explicitly instead of depending on a launch default.
    toolkit = ctx.leg.id.split("zoo_", 1)[1].removesuffix("_windows")
    label = {"gtk3": "GTK3", "gtk4": "GTK4", "qt": "Qt"}[toolkit]
    path = ctx.tier.runtime_dir / f"ref-contract-{toolkit}-{uuid4().hex}.log"
    with managed(ctx, "Zoo " + label, (str(path),)) as app:
        reset(path)
        yield app, path


def matching_event(records, widget, event, value=None, *, any_value=False):
    return any(e.get("widget") == widget and e.get("event") == event
               and (any_value or e.get("value") == value) for e in records)


def selected(tree, name):
    nodes = tree.find(name=name)
    matches = [node for node in nodes
               if node.states & {"SELECTABLE", "CHECKABLE"} or node.role in (7, 37, 44)]
    require(len(matches) == 1, f"expected one selectable control for {name}")
    if matches[0].role == 37 and not matches[0].states & {"SELECTABLE", "SELECTED"}:
        pages = [node for node in nodes if node.role == 20]
        require(len(pages) == 1, f"expected one raw tab page for {name}")
        return "SHOWING" in pages[0].states
    return bool(matches[0].states & {"SELECTED", "CHECKED"})


def effect(tree, node, action, value, before, records):
    """Return separate log/raw evidence; never consume an action response."""
    target = raw_target(tree, node)
    name = "zoo-combo" if node.get("role") == "combobox" else node.get("name")
    states = target.states
    if action == "click" and name in ("zoo-entry", "zoo-password", "zoo-spin"):
        return (matching_event(records, name, "focus", True),
                any("FOCUSED" in child.states for child in target.walk()))
    if action == "type" or (action == "set-value" and target.value() is None):
        if name == "zoo-password":
            length = target.get_property("org.a11y.atspi.Text", "CharacterCount")
            return (matching_event(records, name, "changed", len(value)), length == len(value))
        return matching_event(records, name, "changed", value), target.text() == value
    if action == "set-value":
        return matching_event(records, name, "changed", float(value)), target.value() == float(value)
    if name in ("zoo-toggle", "zoo-check", "zoo-switch", "zoo-expander"):
        bit = ("EXPANDED" if "EXPANDABLE" in states else
               "PRESSED" if "PRESSED" in states or "PRESSED" in
               dict((r[0], r[3]) for r in before)[target.path] else "CHECKED")
        want = (action in ("check", "expand") if action in ("check", "uncheck", "expand", "collapse")
                else bit not in dict((r[0], r[3]) for r in before)[target.path])
        event = "expanded" if name == "zoo-expander" else "toggled"
        return matching_event(records, name, event, want), (bit in states) == want
    if name == "zoo-combo":
        if action != "select":
            popups = [n for n in target.walk() if n.role in (31, 33, 41) and "SHOWING" in n.states]
            showing = any(n.name in ("Alpha", "Bravo", "Charlie")
                          and n.role in (32, 35) and "SHOWING" in n.states
                          for popup in popups for n in popup.walk())
            return not records, showing == (action != "collapse")
        texts = [n.text() for n in target.walk()]
        names = [n.name for n in target.walk() if n.states & {"SELECTED", "CHECKED"}]
        if "org.a11y.atspi.Selection" in target.interfaces():
            count = target.get_property("org.a11y.atspi.Selection", "NSelectedChildren")
            for index in range(count):
                bus, path = target.invoke("org.a11y.atspi.Selection", "GetSelectedChild", "(i)", (index,))[0]
                names.append(RawNode(target.connection, bus or target.bus, path).name)
        return matching_event(records, name, "changed", value), value in texts or value in names or target.name == value
    if name in ("Alpha", "Bravo", "Charlie") and node.get("role") == "option":
        return matching_event(records, "zoo-combo", "changed", name), bool(states & {"SELECTED", "CHECKED"})
    if action == "select" and value and value.startswith(("zoo-row-", "zoo-tab-", "zoo-radio-")):
        event = "row" if value.startswith("zoo-row-") else "page" if value.startswith("zoo-tab-") else "selected"
        return matching_event(records, value, event, value), selected(tree, value)
    if name and name.startswith(("zoo-radio-", "zoo-tab-", "zoo-row-")):
        event = "selected" if name.startswith("zoo-radio-") else "page" if name.startswith("zoo-tab-") else "row"
        return matching_event(records, name, event, name), selected(tree, name)
    if name == "zoo-menubutton":
        showing = [n for n in tree.walk() if n.name == "zoo-menu-item" and "SHOWING" in n.states]
        return not records, bool(showing) == (action != "collapse")
    if name in OPENERS:
        from scenarios.ref_contract_windows import raw_window
        # Openers have no event in the fixture contract; their window is the effect.
        return not records, raw_window(tree, OPENERS[name]) is not None
    return (matching_event(records, name, "activate", any_value=True),
            bool(states & {"ENABLED", "SENSITIVE"}) and "SHOWING" in states)


def prepare(ctx, app, tree, node, action):
    """Make idempotent actions observable, then remint the target ref."""
    name = node.get("name")
    if set(node.get("states", ())) & BLOCKED:
        return node
    opposite = {"check": "uncheck", "uncheck": "check", "expand": "collapse", "collapse": "expand"}
    if action in opposite:
        act(ctx, node, opposite[action])
        if name == "zoo-menubutton":
            require(poll(lambda: any(n.name == "zoo-menu-item" and "SHOWING" in n.states
                                     for n in tree.walk()) == (action == "collapse"), 3))
            return fresh(ctx, app, node)
        target = raw_target(tree, node)
        bit = "EXPANDED" if "EXPANDABLE" in target.states else "CHECKED"
        require(poll(lambda: (bit in target.states) == (action in ("uncheck", "collapse")), 3),
                f"could not prepare {action} on {name}")
        return fresh(ctx, app, node)
    if name in ("zoo-radio-a", "zoo-radio-b", "zoo-tab-a", "zoo-tab-b",
                "zoo-row-1", "zoo-row-2", "zoo-row-3") and selected(tree, name):
        if name.startswith("zoo-row-"):
            other = "zoo-row-2" if name == "zoo-row-1" else "zoo-row-1"
        else:
            other = name[:-1] + ("b" if name.endswith("a") else "a")
        other_node = named(snapshot(ctx, app), other, node.get("role"))
        if "Select" in other_node.get("available_actions", ()):
            act(ctx, other_node, "select", other)
        else:
            act(ctx, other_node, "click")
        require(poll(lambda: selected(tree, other) and not selected(tree, name), 3))
        return fresh(ctx, app, node)
    return node


def print_rows(ctx, rows):
    columns = ("name", "role", "states", "action", "expected", "observed", "verdict")
    table = " | ".join(columns) + "\n" + "\n".join(
        " | ".join(str(row.get(key, "")) for key in columns) for row in rows)
    print(table, flush=True)
    ctx.finding(table)


def refs(ctx):
    rows = []
    try:
        with zoo(ctx) as (app, path):
            tree = root(ctx, app)
            initial = ref_nodes(snapshot(ctx, app))
            require(initial, "fixture snapshot has no refs")
            for original in initial:
                key = original.get("name"), original.get("role")
                row = dict(name=key[0], role=key[1], states=original.get("states", []),
                           action=[], expected=[], observed=[], verdict="pass")
                rows.append(row)
                outcomes = expected_outcomes(original)
                if not outcomes:
                    row.update(verdict="fail", observed="no observable primary action advertised")
                    continue
                for action, _ in outcomes:
                    channel = f"ref{len(rows)}.{action}"
                    try:
                        baseline = raw_state(tree)
                        ctx.before("atspi." + channel, baseline)
                        ctx.before("applog." + channel, since_reset(path))
                        node = prepare(ctx, app, tree, fresh(ctx, app, original), action)
                        expected = dict(expected_outcomes(node)).get(action)
                        require(expected is not None, f"{action} disappeared from refreshed snapshot")
                        row["action"].append(action)
                        row["expected"].append(expected)
                        value = (f"marker-{uuid4().hex}" if action == "type" or
                                 (action == "set-value" and node.get("name") in
                                  ("zoo-entry", "zoo-password", "zoo-text")) else
                                 "7" if action == "set-value" else
                                 ("Charlie" if node.get("value") == "Bravo" else "Bravo")
                                 if action == "select" else None)
                        if action == "select" and node.get("role") == "option":
                            value = node.get("name")
                        elif action == "select" and expected != "refuse" and node.get("role") != "combobox":
                            children = [n for n in raw_target(tree, node).walk()
                                        if (n.name.startswith(("zoo-row-", "zoo-radio-")) or
                                            (n.name.startswith("zoo-tab-") and n.role == 37))
                                        and not n.states & {"SELECTED", "CHECKED"}]
                            require(children, "Select ref has no different contract choice")
                            value = children[0].name
                        before, log_before = raw_state(tree), events(path)
                        response = act(ctx, node, action, value)

                        def landed():
                            return all(effect(tree, node, action, value, before,
                                              events(path)[len(log_before):]))

                        poll(landed, 2, floor_s=0.25)
                        after, log_after = raw_state(tree), events(path)
                        log_effect, raw_effect = effect(tree, node, action, value, before,
                                                        log_after[len(log_before):])
                        if node.get("name") == "zoo-menubutton" and action != "collapse" and raw_effect:
                            menu = snapshot(ctx, app, "--surface", "menu")
                            act(ctx, named(menu, "zoo-menu-item"), "click")
                            log_effect = bool(poll(lambda: matching_event(events(path)[len(log_before):],
                                                                        "zoo-menu-item", "activate", any_value=True), 3))
                            log_after = events(path)
                        refused = (response.exit_code == 1 and bool(response.error)
                                   and not response.error.get("code", "").startswith("HARNESS_"))
                        verdict = row_verdict(expected, refused=refused, log_effect=log_effect,
                                              raw_effect=raw_effect, log_unchanged=log_after == log_before,
                                              raw_unchanged=after == before)
                        row["observed"].append(dict(action=action, refused=refused,
                                                    log=log_effect, raw=raw_effect))
                        if verdict == "fail":
                            row["verdict"] = "fail"
                        ctx.after("atspi." + channel, after)
                        ctx.after("applog." + channel, since_reset(path))
                    except Exception as error:
                        row["verdict"] = "fail"
                        row["observed"].append(f"{action}: {error}")
                    finally:
                        reset(path)
                        snapshot(ctx, app)
                    if "atspi." + channel in ctx.reads:
                        restored = raw_state(tree)
                        ctx.restored("atspi." + channel, restored)
                        ctx.restored("applog." + channel, since_reset(path))
                        if restored != baseline or since_reset(path):
                            row["verdict"] = "fail"
                            row["observed"].append("reset did not restore the baseline")
    finally:
        print_rows(ctx, rows)
    require(all(row["verdict"] == "pass" for row in rows), "ref-contract table has failures")
