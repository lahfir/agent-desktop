#!/usr/bin/env python3
import argparse
import json
import os
import tempfile
import types
import unittest
from unittest.mock import patch

from electron_metrics import (
    MeasurementError,
    assert_stable,
    ensure_distinct_comparison,
    order_for,
    read_trace_events,
    verify_exact_namespace,
)
from electron_metrics_report import build_report, paired_summary, summarize_run
from electron_metrics_state import target_window_id


def sample(wall_ms, cpu_ms, correct=True, stats=True):
    return {
        "wall_ms": wall_ms,
        "cpu_ms": cpu_ms,
        "command_success": True,
        "addressable": True,
        "exact_reresolution": correct,
        "correct": correct,
        "failure_kind": None if correct else "reresolution",
        "error_code": None,
        "stats": {
            "elapsed_us": wall_ms * 1000,
            "activation": {"attempted": True, "succeeded": True, "ready": True},
            "traversal": {"nodes_visited": 6, "peak_handles_owned": 3},
            "reads": {
                "attribute_batches": 2,
                "attributes_requested": 8,
                "cannot_complete": 0,
                "deadline_exhausted": 0,
            },
        } if stats else None,
    }


def arguments():
    return argparse.Namespace(
        app="Slack",
        warmups=4,
        samples=2,
        timeout_seconds=20,
        capture_limit_bytes=2 * 1024 * 1024,
    )


def command_result(data):
    return types.SimpleNamespace(
        termination_error=None,
        timed_out=False,
        output_limited=False,
        stdout=json.dumps({"ok": True, "data": data}),
        stderr="",
        returncode=0,
    )


class MetricsSchemaTests(unittest.TestCase):
    def test_reports_valid_per_command_metrics_and_reliability_rates(self):
        failed = sample(30, 5, correct=False)
        run = summarize_run(
            {"path": "/tmp/current", "sha256": "abc", "version": "1.0.0"},
            [sample(10, 2), sample(20, 4), failed],
        )

        self.assertEqual(run["metrics"]["end_to_end_wall_all_attempts"]["p50"], 20)
        self.assertEqual(run["metrics"]["process_cpu_all_attempts"]["p95"], 5)
        self.assertEqual(run["metrics"]["nodes_visited"]["summary"]["p95"], 6)
        self.assertEqual(run["reliability"]["command_success_rate"], 1)
        self.assertEqual(run["reliability"]["correct_result_rate"], 0.666667)
        self.assertNotIn("rss", json.dumps(run).lower())

    def test_missing_baseline_locator_stats_are_not_fabricated(self):
        run = summarize_run(
            {"sha256": "baseline"},
            [sample(10, 2, stats=False), sample(20, 3, stats=False)],
        )

        locator = run["metrics"]["locator_internal"]
        self.assertFalse(locator["available"])
        self.assertEqual(locator["available_samples"], 0)
        self.assertNotIn("summary", locator)

    def test_paired_deltas_preserve_ab_ba_order_and_signed_direction(self):
        pairs = [
            {"order": "AB", "baseline": sample(20, 5), "current": sample(10, 3)},
            {"order": "BA", "baseline": sample(30, 7), "current": sample(40, 8)},
        ]
        summary = paired_summary(pairs)

        self.assertEqual(summary["orders"], {"AB": 1, "BA": 1})
        self.assertEqual(summary["current_minus_baseline"]["wall"]["p50"], -10)
        self.assertEqual(summary["current_minus_baseline"]["wall"]["p95"], 10)
        self.assertEqual(summary["current_minus_baseline"]["current_faster_wall_rate"], 0.5)

    def test_baseline_is_never_inferred(self):
        runs = {"current": {"identity": {"sha256": "abc"}, "samples": [sample(10, 2)]}}
        report = build_report(arguments(), runs, [], {"app": {}, "windows": []})

        self.assertFalse(report["comparison"]["baseline_provided"])
        self.assertNotIn("baseline", report["runs"])
        self.assertNotIn("paired_comparison", report)
        self.assertFalse(report["method"]["rss_reported"])


class MetricsIntegrityTests(unittest.TestCase):
    def test_exact_namespace_accepts_get_property_contract(self):
        responses = [
            command_result({"snapshot_id": "noise"}),
            command_result({"property": "role", "value": "button"}),
        ]
        runner = types.SimpleNamespace(binary="agent-desktop", label="current", environment={})
        sample_data = {"snapshot_id": "target", "ref_id": "@e1", "role": "button"}

        with patch("electron_metrics.run_bounded", side_effect=responses):
            self.assertTrue(
                verify_exact_namespace(runner, arguments(), "w-1", sample_data)
            )

    def test_exact_namespace_rejects_invalid_get_property_contract(self):
        runner = types.SimpleNamespace(binary="agent-desktop", label="current", environment={})
        sample_data = {"snapshot_id": "target", "ref_id": "@e1", "role": "button"}

        for data in (
            {"role": "button"},
            {"property": "name", "value": "button"},
            {"property": "role", "value": "checkbox"},
        ):
            with self.subTest(data=data):
                responses = [command_result({"snapshot_id": "noise"}), command_result(data)]
                with patch("electron_metrics.run_bounded", side_effect=responses):
                    self.assertFalse(
                        verify_exact_namespace(runner, arguments(), "w-1", sample_data)
                    )

    def test_target_window_ignores_invisible_framework_windows(self):
        windows = [
            {"id": "w-hidden", "visible": False, "minimized": False, "is_focused": False},
            {"id": "w-visible", "visible": True, "minimized": False, "is_focused": False},
        ]

        self.assertEqual(target_window_id(windows), "w-visible")

    def test_target_window_rejects_ambiguous_visible_windows(self):
        windows = [
            {"id": "w-1", "visible": True, "minimized": False, "is_focused": False},
            {"id": "w-2", "visible": True, "minimized": False, "is_focused": False},
        ]

        with self.assertRaises(MeasurementError):
            target_window_id(windows)

    def test_order_is_deterministic_and_balanced(self):
        self.assertEqual(order_for(0, True), ["baseline", "current"])
        self.assertEqual(order_for(1, True), ["current", "baseline"])
        self.assertEqual(order_for(2, False), ["current"])

    def test_malformed_trace_line_fails_instead_of_being_ignored(self):
        descriptor, path = tempfile.mkstemp()
        try:
            os.write(descriptor, b'{"event":"meta"}\nnot-json\n')
            os.close(descriptor)
            descriptor = -1
            with self.assertRaises(MeasurementError):
                read_trace_events(path, 0)
        finally:
            if descriptor >= 0:
                os.close(descriptor)
            os.unlink(path)

    def test_trace_offsets_only_return_new_complete_events(self):
        with tempfile.NamedTemporaryFile(mode="w+", encoding="utf-8") as trace:
            trace.write('{"event":"first"}\n')
            trace.flush()
            first, offset = read_trace_events(trace.name, 0)
            trace.write('{"event":"second"}\n')
            trace.flush()
            second, final_offset = read_trace_events(trace.name, offset)

        self.assertEqual([event["event"] for event in first], ["first"])
        self.assertEqual([event["event"] for event in second], ["second"])
        self.assertGreater(final_offset, offset)

    def test_state_drift_is_a_hard_measurement_failure(self):
        reference = {"app": {"pid": 10, "process_instance": "a"}, "windows": []}
        changed = {"app": {"pid": 10, "process_instance": "b"}, "windows": []}
        with self.assertRaises(MeasurementError):
            assert_stable(reference, changed, "after pair")

    def test_identical_binary_hashes_are_not_reported_as_a_comparison(self):
        runners = {
            "baseline": types.SimpleNamespace(identity={"sha256": "same"}),
            "current": types.SimpleNamespace(identity={"sha256": "same"}),
        }
        with self.assertRaises(MeasurementError):
            ensure_distinct_comparison(runners)


if __name__ == "__main__":
    unittest.main()
