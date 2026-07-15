import re
import os
import subprocess
import tempfile
import unittest
from pathlib import Path


E2E_ROOT = Path(__file__).resolve().parent
SOURCED_SUITE_FILES = [
    E2E_ROOT / "lib.sh",
    *sorted((E2E_ROOT / "scenarios").glob("*.sh")),
]


class HarnessContractTests(unittest.TestCase):
    def test_sourced_suite_has_one_canonical_exit_path(self):
        exits = []
        abort_calls = []
        for path in SOURCED_SUITE_FILES:
            source = path.read_text()
            for line_number, line in enumerate(source.splitlines(), start=1):
                if re.fullmatch(r"\s*exit 1\s*", line):
                    exits.append((path.name, line_number))
                if re.search(r"\babort_suite\s+\"", line):
                    abort_calls.append((path.name, line_number))

        self.assertEqual(len(exits), 1, exits)
        self.assertEqual(exits[0][0], "lib.sh")
        self.assertTrue(abort_calls)
        source = (E2E_ROOT / "lib.sh").read_text()
        abort = re.search(r"abort_suite\(\) \{(?P<body>.*?)\n\}", source, re.DOTALL)
        self.assertIsNotNone(abort)
        body = abort.group("body")
        self.assertIn('badmsg "$1"', body)
        self.assertIn("finish", body)
        self.assertNotIn("cleanup", body)
        self.assertRegex(body, r"(?m)^\s*exit 1\s*$")
        finish = re.search(r"finish\(\) \{(?P<body>.*?)\n\}", source, re.DOTALL)
        self.assertIsNotNone(finish)
        self.assertNotIn("abort_suite", finish.group("body"))

    def test_shell_harness_never_permanently_removes_artifacts(self):
        permanent_delete = re.compile(r"(?m)(?:^|[;&|({])\s*rm(?:\s|$)")
        offenders = []
        shell_paths = [
            *E2E_ROOT.glob("*.sh"),
            *(E2E_ROOT / "scenarios").glob("*.sh"),
            *(E2E_ROOT.parent / "fixture-app").glob("*.sh"),
            *(E2E_ROOT.parent.parent / "scripts").glob("*.sh"),
        ]
        for path in sorted(shell_paths):
            if permanent_delete.search(path.read_text()):
                offenders.append(path.name)

        self.assertEqual(offenders, [])

    def test_wait_scenarios_use_independent_oracles(self):
        source = (E2E_ROOT / "scenarios" / "reliability.sh").read_text()

        self.assertIn('is_target "$delayed" enabled', source)
        self.assertIn('is_target "$primary" visible', source)
        self.assertIn('get_target "$text_input" --property value', source)
        self.assertIn("require_value actionable_before click-status", source)
        self.assertIn("require_value actionable_after click-status", source)

    def test_acceptance_visibility_uses_a_qualified_zero_bounds_ref(self):
        source = (E2E_ROOT / "scenarios" / "acceptance.sh").read_text()

        self.assertIn("require_target zero_target button zero-bounds-button", source)
        self.assertIn('is_target "$zero_target" visible', source)
        self.assertIn('data.applicable)" = "True"', source)
        self.assertIn('data.result)" = "False"', source)

    def test_hover_uses_the_headed_qualified_ref_pipeline(self):
        source = (E2E_ROOT / "scenarios" / "interaction.sh").read_text()

        self.assertIn("require_target hover_target button hover-target", source)
        self.assertIn('act_target "$hover_target" hover', source)
        self.assertIn("require_value hover_after hover-status", source)
        self.assertNotIn("hover --xy", source)

    def test_denied_automation_skips_when_notification_center_cannot_be_closed(self):
        source = (E2E_ROOT / "scenarios" / "acceptance.sh").read_text()

        unavailable = source.index(
            "Notification Center could not be proven closed after best-effort preparation"
        )
        permission_gate = source.index(
            'if [ "$(json_field "$nc_probe" error.code)" != "POLICY_DENIED" ]'
        )
        denial_call = source.index('denied_output="$("$bin" --headed list-notifications')
        self.assertLess(permission_gate, unavailable)
        self.assertLess(unavailable, denial_call)
        self.assertIn("skipmsg", source[permission_gate:denial_call])
        self.assertNotIn("badmsg", source[permission_gate:denial_call])

    def test_native_runner_gates_on_strict_headless_non_interference(self):
        source = (E2E_ROOT / "run.sh").read_text()

        safe_gate = source.index('bash "$here/safe-semantic.sh"')
        focused_fixture = source.index("prepare_native_harness")
        self.assertLess(safe_gate, focused_fixture)

    def test_native_runner_builds_locked_canonical_artifacts_before_desktop_lock(self):
        source = (E2E_ROOT / "run.sh").read_text()

        cli_build = source.index("cargo build --locked --release -p agent-desktop")
        ffi_build = source.index("cargo build --locked --profile release-ffi")
        desktop_lock = source.index('exec python3 "$here/interaction_lock.py"')
        self.assertLess(cli_build, desktop_lock)
        self.assertLess(ffi_build, desktop_lock)
        self.assertIn(
            'AGENT_DESKTOP_E2E_RELEASE_BIN="$repo/target/release/agent-desktop"',
            source,
        )
        self.assertIn(
            'AGENT_DESKTOP_E2E_RELEASE_FFI="$repo/target/release-ffi/libagent_desktop_ffi.dylib"',
            source,
        )

    def test_post_launch_readiness_failure_still_closes_owned_fixture(self):
        run_source = (E2E_ROOT / "run.sh").read_text()
        opened = run_source.index('guard_exec 10 1048576 open "$fixture_app"')
        owned = run_source.index("fixture_owned=1", opened)
        readiness = run_source.index('if [ -z "$ready" ]', owned)
        self.assertLess(opened, owned)
        self.assertLess(owned, readiness)

        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "calls"
            mock_bin = Path(directory) / "agent-desktop"
            mock_bin.write_text('#!/bin/sh\nprintf "%s\\n" "$*" >> "$MOCK_CALLS"\n')
            mock_bin.chmod(0o700)
            script = r'''
source "$1"
bin="$2"
fixture_owned=1
fixture_started=0
cleanup_isolated_environment() { :; }
release_exclusive_lock() { :; }
cleanup
'''
            environment = os.environ.copy()
            environment["MOCK_CALLS"] = str(log)
            subprocess.run(
                [
                    "bash",
                    "-c",
                    script,
                    "cleanup-test",
                    str(E2E_ROOT / "lib.sh"),
                    str(mock_bin),
                ],
                check=True,
                env=environment,
            )

            self.assertEqual(log.read_text().strip(), "close-app AgentDeskFixture --force")

    def test_success_checks_every_staged_native_artifact_before_returning(self):
        source = (E2E_ROOT / "lib.sh").read_text()
        finish = re.search(r"finish\(\) \{(?P<body>.*?)\n\}", source, re.DOTALL)

        self.assertIsNotNone(finish)
        body = finish.group("body")
        self.assertIn('verify_immutable_binary "$raw_bin" "$raw_bin_sha"', body)
        self.assertIn('verify_immutable_binary "$ffi_dylib" "$ffi_dylib_sha"', body)
        self.assertIn('verify_immutable_binary "$ffi_helper" "$ffi_helper_sha"', body)
        decision = body.index('if [ "$fail" -gt 0 ]')
        self.assertLess(body.index('verify_immutable_binary "$ffi_dylib"'), decision)
        self.assertLess(body.index('verify_immutable_binary "$ffi_helper"'), decision)

    def test_fixture_status_oracles_use_stable_native_identifiers(self):
        source = (E2E_ROOT / "lib.sh").read_text()

        self.assertIn('--role statictext --native-id "$name" --first', source)

    def test_trace_artifact_actions_use_the_session_that_created_their_refs(self):
        source = (E2E_ROOT / "scenarios" / "trace_performance.sh").read_text()

        self.assertIn('"$bin" --session "$trace_session" find', source)
        self.assertIn('trace_click="$("$bin" --session "$trace_session" click', source)
        self.assertIn(
            'trace_session_type="$("$bin" --headed --session "$trace_session" type',
            source,
        )

    def test_sheet_scenarios_scroll_the_button_before_clicking(self):
        fixtures = [
            (
                "scenarios/surfaces.sh",
                'act_target "$open_sheet" scroll-to',
                'act_target "$open_sheet" click',
            ),
            (
                "scenarios/acceptance.sh",
                'act_target "$open_sheet" scroll-to',
                'batch_payload="$(python3',
            ),
        ]
        for filename, scroll_marker, click_marker in fixtures:
            with self.subTest(filename=filename):
                source = (E2E_ROOT / filename).read_text()
                scroll = source.index(scroll_marker)
                click = source.index(click_marker)
                self.assertLess(scroll, click)

    def test_recoverable_trash_moves_artifact_with_available_backend(self):
        with tempfile.TemporaryDirectory() as root:
            root_path = Path(root)
            artifact = root_path / "artifact"
            artifact.mkdir()
            trash_dir = root_path / "trash"
            trash_dir.mkdir()
            bin_dir = root_path / "bin"
            bin_dir.mkdir()
            trash = bin_dir / "trash"
            trash.write_text('#!/bin/sh\n/bin/mv "$1" "$FAKE_TRASH/"\n')
            trash.chmod(0o700)

            result = self.run_trash_helper(artifact, bin_dir, trash_dir)

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertFalse(artifact.exists())
            self.assertTrue((trash_dir / artifact.name).is_dir())

    def test_recoverable_trash_retains_artifact_without_backend(self):
        with tempfile.TemporaryDirectory() as root:
            artifact = Path(root) / "artifact"
            artifact.mkdir()

            result = self.run_trash_helper(artifact, Path(root) / "missing", Path(root))

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertTrue(artifact.is_dir())
            self.assertIn("retained artifact", result.stderr)

    def run_trash_helper(self, artifact: Path, path: Path, trash_dir: Path):
        script = f'''
source "{E2E_ROOT / "harness.sh"}"
host_home="{artifact.parent}"
host_tmp="{artifact.parent}"
PATH="{path}"
if trash_recoverably "{artifact}"; then
    test ! -e "{artifact}"
else
    test -e "{artifact}"
fi
'''
        env = os.environ.copy()
        env["FAKE_TRASH"] = str(trash_dir)
        return subprocess.run(
            ["/bin/bash", "-c", script],
            capture_output=True,
            check=False,
            text=True,
            env=env,
        )


if __name__ == "__main__":
    unittest.main()
