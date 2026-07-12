import re
import os
import subprocess
import tempfile
import unittest
from pathlib import Path


E2E_ROOT = Path(__file__).resolve().parent
SOURCED_SUITE_FILES = [
    E2E_ROOT / "lib.sh",
    *sorted(E2E_ROOT.glob("scenarios-*.sh")),
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
        self.assertEqual(len(abort_calls), 9, abort_calls)
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
            *(E2E_ROOT.parent / "fixture-app").glob("*.sh"),
            *(E2E_ROOT.parent.parent / "scripts").glob("*.sh"),
        ]
        for path in sorted(shell_paths):
            if permanent_delete.search(path.read_text()):
                offenders.append(path.name)

        self.assertEqual(offenders, [])

    def test_wait_scenarios_use_independent_oracles(self):
        source = (E2E_ROOT / "scenarios-reliability.sh").read_text()

        self.assertIn('is_target "$delayed" enabled', source)
        self.assertIn('is_target "$primary" visible', source)
        self.assertIn('get_target "$text_input" --property value', source)
        self.assertIn("require_value actionable_before click-status", source)
        self.assertIn("require_value actionable_after click-status", source)

    def test_fixture_status_oracles_use_stable_native_identifiers(self):
        source = (E2E_ROOT / "lib.sh").read_text()

        self.assertIn('--role statictext --native-id "$name" --first', source)

    def test_sheet_scenarios_scroll_the_button_before_clicking(self):
        fixtures = [
            (
                "scenarios-surfaces.sh",
                'act_target "$open_sheet" scroll-to',
                'act_target "$open_sheet" click',
            ),
            (
                "scenarios-acceptance.sh",
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
