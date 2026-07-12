#!/usr/bin/env python3
import os
import sys
import tempfile
import unittest
from unittest import mock

from json_tool import run_bounded


class RunBoundedTests(unittest.TestCase):
    def test_marked_child_receives_the_canonical_interaction_lease_fd(self):
        with tempfile.TemporaryFile() as lease:
            os.set_inheritable(lease.fileno(), True)
            environment = os.environ.copy()
            environment["AGENT_DESKTOP_INTERACTION_LEASE_FD"] = str(lease.fileno())
            result = run_bounded(
                [
                    sys.executable,
                    "-c",
                    "import os; os.fstat(int(os.environ['AGENT_DESKTOP_INTERACTION_LEASE_FD'])); print('inherited')",
                ],
                timeout_seconds=2,
                max_capture_bytes=1024,
                env=environment,
                inherit_interaction_lease=True,
            )

        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stdout, "inherited\n")

    def test_generic_shell_child_does_not_receive_the_interaction_lease(self):
        with tempfile.TemporaryFile() as lease:
            os.set_inheritable(lease.fileno(), True)
            environment = os.environ.copy()
            environment["AGENT_DESKTOP_INTERACTION_LEASE_FD"] = str(lease.fileno())
            result = run_bounded(
                [
                    "/bin/sh",
                    "-c",
                    'test -z "${AGENT_DESKTOP_INTERACTION_LEASE_FD+x}" && '
                    'test ! -e "/dev/fd/$1" && printf "not inherited\\n"',
                    "lease-check",
                    str(lease.fileno()),
                ],
                timeout_seconds=2,
                max_capture_bytes=1024,
                env=environment,
            )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "not inherited\n")

    def test_captures_a_successful_child(self):
        result = run_bounded(
            [sys.executable, "-c", "print('ok')"],
            timeout_seconds=2,
            max_capture_bytes=1024,
        )

        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stdout, "ok\n")
        self.assertFalse(result.timed_out)
        self.assertFalse(result.output_limited)
        self.assertIsNone(result.termination_error)

    def test_kills_the_process_group_at_the_absolute_timeout(self):
        result = run_bounded(
            [
                sys.executable,
                "-c",
                "import subprocess,sys,time; "
                "subprocess.Popen([sys.executable,'-c','import time; time.sleep(30)']); "
                "time.sleep(30)",
            ],
            timeout_seconds=0.2,
            max_capture_bytes=1024,
        )

        self.assertEqual(result.returncode, 124)
        self.assertTrue(result.timed_out)
        self.assertLess(result.wall_ms, 2000)

    def test_descendant_holding_output_pipe_cannot_extend_the_deadline(self):
        result = run_bounded(
            [
                sys.executable,
                "-c",
                "import subprocess,sys,time; "
                "subprocess.Popen([sys.executable,'-c','import time; time.sleep(30)']); "
                "print('ready', flush=True); time.sleep(30)",
            ],
            timeout_seconds=0.2,
            max_capture_bytes=1024,
        )

        self.assertTrue(result.timed_out)
        self.assertIn("ready", result.stdout)
        self.assertLess(result.wall_ms, 2000)

    def test_group_signal_failure_is_reported_without_blocking(self):
        with mock.patch("json_tool.os.killpg", side_effect=PermissionError("denied")):
            result = run_bounded(
                [sys.executable, "-c", "import time; time.sleep(30)"],
                timeout_seconds=0.05,
                max_capture_bytes=1024,
            )

        self.assertTrue(result.timed_out)
        self.assertIn("SIGTERM failed", result.termination_error)
        self.assertLess(result.wall_ms, 2000)

    def test_stops_unbounded_output(self):
        result = run_bounded(
            [sys.executable, "-c", "import sys; sys.stdout.write('x' * 100000)"],
            timeout_seconds=2,
            max_capture_bytes=4096,
        )

        self.assertEqual(result.returncode, 125)
        self.assertTrue(result.output_limited)
        self.assertLessEqual(len(result.stdout.encode()), 4096)


if __name__ == "__main__":
    unittest.main()
