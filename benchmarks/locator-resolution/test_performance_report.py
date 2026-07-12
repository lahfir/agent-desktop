import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


MODULE_PATH = Path(__file__).with_name("generate_performance_report.py")
SPEC = importlib.util.spec_from_file_location("performance_report", MODULE_PATH)
REPORT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(REPORT)


def synthetic_fixture():
    return {
        "scenarios": [
            {
                "name": name,
                "comparison": {
                    "p50_find_speedup": 1.25 + index / 10,
                    "p95_find_speedup": 1.35 + index / 10,
                },
                "legacy_snapshot": {"correct_all_runs": True},
                "live_find_selected_refs": {
                    "correct_all_runs": True,
                    "selected_refs_reresolvable": True,
                },
            }
            for index, name in enumerate(REPORT.SCENARIO_LABELS)
        ]
    }


def live_fixture():
    samples = [
        {
            "pair_index": index,
            "wall_ms": wall,
            "command_success": True,
            "stats": {"private_ui_text": "SECRET-SAMPLE-SENTINEL"},
        }
        for index, wall in enumerate((300.0, 320.0, 340.0))
    ]
    current_samples = [dict(sample, wall_ms=sample["wall_ms"] - 70.0) for sample in samples]
    reliability = {
        "correct_result_rate": 0.0,
        "addressable_result_rate": 0.0,
        "exact_reresolution_rate": 0.0,
    }
    return {
        "app": "SECRET-APP-SENTINEL",
        "state_reference": {
            "target_window_id": "SECRET-WINDOW-SENTINEL",
            "windows": [{"title": "SECRET-TITLE-SENTINEL", "pid": 987654}],
        },
        "paired_comparison": {
            "comparable_successful_pairs": 3,
            "current_minus_baseline": {
                "current_faster_wall_rate": 1.0,
                "wall": {"p50": -70.0},
            },
        },
        "runs": {
            "baseline": {
                "binary": {"sha256": "baseline-build-sha"},
                "samples": samples,
                "metrics": {
                    "end_to_end_wall_all_attempts": {"p50": 300.0, "p95": 340.0},
                    "process_cpu_all_attempts": {"p50": 50.0, "p95": 60.0},
                },
                "reliability": reliability,
            },
            "current": {
                "binary": {"sha256": "measured-build-sha"},
                "samples": current_samples,
                "metrics": {
                    "end_to_end_wall_all_attempts": {"p50": 230.0, "p95": 270.0},
                    "process_cpu_all_attempts": {"p50": 31.0, "p95": 40.0},
                },
                "reliability": {key: 1.0 for key in reliability},
            },
        },
    }


class PerformanceReportTests(unittest.TestCase):
    def test_report_contains_metrics_and_excludes_sensitive_input(self):
        output = REPORT.render_report(synthetic_fixture(), live_fixture(), 1_793_312, 2_162_480)
        self.assertIn("Slack (read-only)", output)
        self.assertIn("230.0", output)
        self.assertIn("20.59%", output)
        self.assertIn("Paired live wall-time deltas", output)
        self.assertIn("aria-label=", output)
        self.assertIn("<script>", output)
        for sentinel in (
            "SECRET-APP-SENTINEL",
            "SECRET-WINDOW-SENTINEL",
            "SECRET-TITLE-SENTINEL",
            "SECRET-SAMPLE-SENTINEL",
            "987654",
            "state_reference",
        ):
            self.assertNotIn(sentinel, output)

    def test_cli_writes_standalone_report(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            synthetic_path = root / "synthetic.json"
            live_path = root / "live.json"
            output_path = root / "report.html"
            synthetic_path.write_text(json.dumps(synthetic_fixture()), encoding="utf-8")
            live_path.write_text(json.dumps(live_fixture()), encoding="utf-8")
            REPORT.main(
                [
                    "--synthetic",
                    str(synthetic_path),
                    "--live",
                    str(live_path),
                    "--output",
                    str(output_path),
                ]
            )
            self.assertTrue(output_path.read_text(encoding="utf-8").startswith("<!doctype html>"))

    def test_summary_uses_measured_reliability_instead_of_a_literal_claim(self):
        live = live_fixture()
        live["runs"]["current"]["reliability"]["addressable_result_rate"] = 0.8

        output = REPORT.render_report(synthetic_fixture(), live, 1_793_312, 2_162_480)

        self.assertIn("<strong>80.0%</strong>", output)
        self.assertNotIn("<strong>100%</strong><span>current correct", output)

    def test_summary_describes_regressions_without_malformed_signs(self):
        live = live_fixture()
        live["paired_comparison"]["current_minus_baseline"]["wall"]["p50"] = 12.5

        output = REPORT.render_report(synthetic_fixture(), live, 2_200_000, 2_000_000)

        self.assertIn("12.5 ms</strong><span>live paired p50 wall-time increase", output)
        self.assertIn("Current is 9.09% smaller (-200,000 bytes)", output)
        self.assertNotIn("larger (+-", output)

    def test_mismatched_final_build_is_labeled_as_pre_final_live_evidence(self):
        output = REPORT.render_report(
            synthetic_fixture(),
            live_fixture(),
            1_793_312,
            2_162_480,
            current_sha256="final-build-sha",
        )

        self.assertIn("could not be re-probed", output)
        self.assertIn("pre-final evidence", output)


if __name__ == "__main__":
    unittest.main()
