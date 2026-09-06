"""Run with python3 scripts/perf_ab_probe_test.py."""
from unittest.mock import patch

import perf_ab_probe as probe
import perf_report_html as renderer


def check_balanced_order_and_percentile():
    calls = []
    with patch.object(probe, "run", side_effect=lambda binary, case: (
        calls.append((binary, case)) or 1, None
    )), patch.object(probe, "envelope", return_value={"ok": True}):
        results, shapes = probe.collect(
            {"HEAD": "new", "BASE": "old"},
            lambda _: [("first", ["first"]), ("second", ["second"])],
            2,
        )
    assert calls == [
        ("new", "first"), ("old", "first"),
        ("new", "second"), ("old", "second"),
        ("old", "first"), ("new", "first"),
        ("old", "second"), ("new", "second"),
    ]
    results["HEAD"]["first"] = [(n, True) for n in range(1, 11)] + [(999, False)]
    summary = probe.summarize(results, shapes)["first"]["HEAD"]
    assert summary["p50_ms"] == 5.5
    assert summary["p95_ms"] == 10
    assert summary["ok_rate"] == 10 / 11


def check_report_and_cases():
    assert probe.app_cases("fixture")[-1][1] == [
        "find", "--app", "fixture", "--role", "button", "--first"
    ]
    assert probe.tree_stats({"data": {"tree": {"children": [{}]}, "ref_count": 1}}) == (2, 2, 1)
    payload = {"app": "<script>", "cases": {"find": {
        "HEAD": {"p50_ms": None, "p95_ms": None, "ok_rate": 0},
        "BASE": {"p50_ms": 2, "p95_ms": 3, "ok_rate": 1},
    }}}
    html = renderer.live_report(payload)
    assert "&lt;script&gt;" in html and "<script>" not in html
    assert "not comparable" in html and ">0.00<" in html and ">3.00<" in html
    html = renderer.synthetic_report({"scenarios": [{"legacy_snapshot": {"correct_all_runs": False}}]})
    assert "FAIL" in html and "not revision A/B" in html and "Refs re-resolvable" in html


if __name__ == "__main__":
    check_balanced_order_and_percentile()
    check_report_and_cases()
