#!/usr/bin/env python3
"""Exercise the gate's failure and coverage boundary without a desktop."""

from contextlib import redirect_stdout
import importlib.util
import io
import json
import sys
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


sys.path.insert(0, str(Path(__file__).resolve().parent))

spec = importlib.util.spec_from_file_location("gate", Path(__file__).with_name("linux-live-gate.py"))
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)


class GateTests(unittest.TestCase):
    def test_merge_allows_unscheduled_shards_but_requires_scheduled_results(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            name = "notifications::live_tests::one"
            manifest = directory / "manifest.json"
            shards = gate.plan([name], {name}, 3)
            binary = directory / "libtest"
            binary.write_bytes(b"frozen binary")
            gate.save(manifest, dict(tests=[name], ignored=[name], shards=shards,
                                     binary=str(binary), artifacts={}, timeout=780,
                                     selection=dict(modules=[], ignored_only=False),
                                     binary_sha256=gate.hashlib.sha256(binary.read_bytes()).hexdigest(),
                                     source_sha256=gate.source_hash()))
            for tier in gate.TIERS:
                (directory / tier).mkdir()
                gate.save(directory / tier / "summary.json", dict(passed=True))
            with redirect_stdout(io.StringIO()):
                self.assertFalse(gate.merge(manifest))
                gate.save(directory / "mutter/shard-0.json",
                          [dict(passed=True, results=[[name, "ok"]])])
                self.assertTrue(gate.merge(manifest))
            blocked = directory / "blocked"
            blocked.write_text("a file cannot contain the session base")
            for override in ("", str(blocked / "sessions")):
                with self.subTest(override=override), \
                     patch.dict(gate.os.environ, AGENT_DESKTOP_SESSION_BASE=override,
                                XDG_RUNTIME_DIR=str(blocked)), \
                     patch.object(gate.sys, "argv", ["linux-live-gate.py", "--merge",
                                                    "--output", str(directory)]), \
                     patch.object(gate.signal, "signal"), redirect_stdout(io.StringIO()):
                    self.assertEqual(gate.main(), 0)
                    self.assertTrue(json.loads((directory / "summary.json").read_text())["passed"])

    def test_targeted_selection_is_ignored_only_and_rejects_mistyped_modules(self):
        names = ["tree::focus::unit", "tree::focus::live", "tree::focus_other::live",
                 "notifications::live_tests::live"]
        ignored = set(names[1:])
        self.assertEqual(gate.select_tests(names, ignored, ["tree::focus"], True), [names[1]])
        self.assertEqual(gate.select_tests(names, ignored, [], False), names)
        with self.assertRaises(ValueError):
            gate.select_tests(names, ignored, ["tree::typo"], True)

    def test_private_services_get_their_own_session(self):
        cases = (
            "input::physical::target_pointer::shell_live_tests::a_completed_wayland_click_with_refused_stop_is_delivered_once_and_unsafe_to_retry",
            "input::physical::target_pointer::shell_live_tests::a_missing_mutter_capability_refuses_before_session_creation",
        )
        for name in cases:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary)
                (directory / "mutter").mkdir()
                manifest = directory / "manifest.json"
                gate.save(manifest, dict(binary="/unused/libtest", timeout=10,
                                         shards=gate.plan([name], {name}, 1)))
                commands = []

                def launch(command, log, timeout):
                    commands.append(command)
                    Path(command[command.index("--logfile") + 1]).write_text(f"ok {name}\n")
                    return 0, 0.0

                with patch.object(gate, "run_bounded", side_effect=launch), redirect_stdout(io.StringIO()):
                    gate.worker(manifest, "mutter", 0)
                self.assertEqual(commands[0][:3], ["bash", str(gate.ROOT / "scripts/private-session.sh"), "--"])

    def test_global_lease_consumers_share_one_writer_while_reads_run_elsewhere(self):
        writers = ["system::interaction::interaction_lease::tests::unit", "actions::dispatch_tests::unit",
                   "actions::live::live_widget_tests::live", "tree::live::physical_live_tests::live",
                   "system::focus::focus_window_live_tests::live",
                   "tree::resolve::resolve_source::action_live_tests::live"]
        readers = ["tree::live::resolve_live_tests::live", "tree::live::qt_live_tests::live",
                   "system::capture::capture_live_tests::live"]
        names = writers + readers
        ignored = set(names[2:])
        for jobs in (1, 2, 3):
            shards = gate.plan(names, ignored, jobs)["openbox"]
            assigned = [{name for task in tasks for name in task["tests"]} for tasks in shards]
            self.assertTrue(set(writers) <= assigned[0])
            self.assertFalse(any(set(writers) & values for values in assigned[1:]))
            if jobs > 1:
                self.assertFalse(set(readers) & assigned[0])
                self.assertEqual(set(readers), set.union(*assigned[1:]))

    def test_partition_keeps_every_case_on_each_applicable_tier(self):
        wayland = [gate.CURSOR_WAYLAND, *(module + "::case" for module in gate.WAYLAND_MODULES),
                   "system::process::protected::live_tests::a_native_wayland_application_is_cleared_by_its_tree_and_a_windowless_one_is_not",
                   "input::wayland::live_tests::reply_loss::case",
                   "system::shell::live_wayland_tests::native_case"]
        mutter = ["notifications::live_tests::case",
                  "system::shell::outage_live_tests::case"]
        cases = ["unit::fast", "tree::live::one", "tree::live::two", *wayland, *mutter]
        ignored = set(cases[1:])
        for jobs in (1, 2, 3):
            shards = gate.plan(cases, ignored, jobs)
            actual = {tier: [name for tasks in workers for task in tasks for name in task["tests"]]
                      for tier, workers in shards.items()}
            self.assertCountEqual(actual["openbox"], cases[:3])
            self.assertCountEqual(actual["mutter"], [*cases[1:3], *mutter])
            self.assertCountEqual(actual["wayland"], wayland)

    def test_receipts_reject_failure_omission_duplicate_and_missing_shard(self):
        for mode in ("pass", "failure", "omission", "duplicate", "setup_failure"):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary)
                binary = directory / "libtest"
                binary.write_text(
                    "#!/usr/bin/env python3\nimport sys\nfrom pathlib import Path\n"
                    "names = sys.argv[sys.argv.index('--exact') + 1:]\n"
                    "log = Path(sys.argv[sys.argv.index('--logfile') + 1:][0])\n"
                    f"mode = {mode!r}\n"
                    "lines = []\n"
                    "for name in names:\n"
                    "    if mode != 'omission':\n"
                    "        lines.append(('failed' if mode == 'failure' else 'ok') + ' ' + name)\n"
                    "    if mode == 'duplicate':\n"
                    "        lines.append('ok ' + name)\n"
                    "log.write_text('\\n'.join(lines) + '\\n')\n"
                    "sys.exit(1 if mode == 'failure' else 0)\n"
                )
                binary.chmod(0o700)
                names = ["checks::one", "checks::two", "checks::three"]
                manifest = directory / "manifest.json"
                gate.save(manifest, dict(binary=str(binary), timeout=10,
                                         shards=gate.plan(names, set(names), 3)))
                bounded = gate.run_bounded

                def launch(command, log, timeout):
                    if command[0] != "bash":
                        return bounded(command, log, timeout)
                    if mode == "setup_failure":
                        return 2, 0.0
                    return gate.worker(manifest, command[-2], int(command[-1])), 0.0

                with patch.object(gate, "run_bounded", side_effect=launch), redirect_stdout(io.StringIO()):
                    passed = gate.run_tier(manifest, "openbox")
                summary = json.loads((directory / "openbox/summary.json").read_text())
                self.assertEqual(passed, mode == "pass")
                self.assertEqual(summary["expected"], 3)
                if mode == "pass":
                    self.assertEqual(summary["counts"], {"ok": 3})
                    self.assertTrue(summary["complete"])


if __name__ == "__main__":
    unittest.main()
