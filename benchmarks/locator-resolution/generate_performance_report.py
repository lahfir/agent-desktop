#!/usr/bin/env python3

import argparse
import html
import json
from pathlib import Path


DEFAULT_BASELINE_BYTES = 1_793_312
DEFAULT_CURRENT_BYTES = 2_162_496
SCENARIO_LABELS = {
    "deep_anonymous_has_text": "Deep anonymous text",
    "duplicate_button_role_and_name": "Duplicate role + name",
    "electron_dual_identifier_moving_bounds": "Moving Electron bounds",
    "large_nested_channel_has_unread": "Large nested tree",
}


def require(mapping, *keys):
    value = mapping
    for key in keys:
        if not isinstance(value, dict) or key not in value:
            raise ValueError(f"missing required field: {'.'.join(keys)}")
        value = value[key]
    return value


def load_json(path):
    with path.open(encoding="utf-8") as source:
        return json.load(source)


def fmt(value, digits=1):
    return f"{float(value):,.{digits}f}"


def rate(value):
    return float(value) * 100.0


def directional_change(value, lower_label, higher_label, unchanged_label):
    value = float(value)
    if value < 0:
        return abs(value), lower_label
    if value > 0:
        return value, higher_label
    return 0.0, unchanged_label


def size_change(baseline_bytes, current_bytes):
    delta = current_bytes - baseline_bytes
    percent = abs(delta) / baseline_bytes * 100.0
    if delta < 0:
        return percent, "smaller", f"{delta:,}"
    if delta > 0:
        return percent, "larger", f"+{delta:,}"
    return 0.0, "the same size", "0"


def paired_wall_deltas(live):
    baseline = {
        int(sample["pair_index"]): float(sample["wall_ms"])
        for sample in require(live, "runs", "baseline", "samples")
        if sample.get("command_success") and isinstance(sample.get("wall_ms"), (int, float))
    }
    current = {
        int(sample["pair_index"]): float(sample["wall_ms"])
        for sample in require(live, "runs", "current", "samples")
        if sample.get("command_success") and isinstance(sample.get("wall_ms"), (int, float))
    }
    indexes = sorted(baseline.keys() & current.keys())
    return [current[index] - baseline[index] for index in indexes]


def svg_grouped_bars(title, baseline, current, unit):
    labels = ("p50", "p95")
    values = [baseline[0], current[0], baseline[1], current[1]]
    maximum = max(values) * 1.18
    bars = []
    for group, label in enumerate(labels):
        group_x = 86 + group * 200
        for offset, (series, value, css_class) in enumerate(
            (("Baseline", baseline[group], "baseline"), ("Current", current[group], "current"))
        ):
            height = 155 * value / maximum
            x = group_x + offset * 66
            y = 193 - height
            bars.append(
                f'<rect class="{css_class}" x="{x}" y="{y:.2f}" width="48" height="{height:.2f}" rx="4" />'
                f'<text x="{x + 24}" y="{max(20, y - 7):.2f}" text-anchor="middle">{fmt(value)} {unit}</text>'
                f'<text class="muted" x="{x + 24}" y="216" text-anchor="middle">{series}</text>'
            )
        bars.append(f'<text x="{group_x + 57}" y="240" text-anchor="middle">{label}</text>')
    summary = (
        f"{title}. Baseline p50 {fmt(baseline[0])} and p95 {fmt(baseline[1])} {unit}; "
        f"current p50 {fmt(current[0])} and p95 {fmt(current[1])} {unit}."
    )
    return (
        f'<svg class="chart" viewBox="0 0 480 255" role="img" aria-label="{html.escape(summary)}">'
        f"<title>{html.escape(title)}</title><desc>{html.escape(summary)}</desc>"
        '<line class="axis" x1="54" y1="193" x2="450" y2="193" />'
        + "".join(bars)
        + "</svg>"
    )


def svg_delta_plot(deltas):
    if not deltas:
        raise ValueError("live report has no comparable successful wall-time pairs")
    ordered = sorted(deltas)
    limit = max(abs(min(ordered)), abs(max(ordered)), 1.0) * 1.1
    zero_y = 132
    scale = 102 / limit
    plot_width = 610
    bar_width = max(3.0, plot_width / len(ordered) - 3.0)
    bars = []
    for index, value in enumerate(ordered):
        x = 49 + index * plot_width / len(ordered)
        height = abs(value) * scale
        y = zero_y if value >= 0 else zero_y - height
        css_class = "slower" if value > 0 else "faster"
        bars.append(
            f'<rect class="{css_class}" x="{x:.2f}" y="{y:.2f}" width="{bar_width:.2f}" height="{height:.2f}">'
            f"<title>Pair {index + 1}: {fmt(value)} ms</title></rect>"
        )
    faster = sum(value < 0 for value in ordered)
    summary = (
        f"Ordered current minus baseline wall-time deltas for {len(ordered)} pairs. "
        f"Current was faster in {faster} pairs. Negative values are faster."
    )
    return (
        '<svg class="chart wide" viewBox="0 0 680 280" role="img" '
        f'aria-label="{html.escape(summary)}"><title>Paired wall-time deltas</title>'
        f"<desc>{html.escape(summary)}</desc>"
        '<text class="muted" x="10" y="30">faster</text><text class="muted" x="10" y="252">slower</text>'
        '<line class="axis" x1="47" y1="132" x2="665" y2="132" />'
        + "".join(bars)
        + f'<text x="49" y="270">{fmt(min(ordered))} ms</text>'
        + f'<text x="665" y="270" text-anchor="end">{fmt(max(ordered))} ms</text>'
        + "</svg>"
    )


def svg_rate_chart(live):
    metrics = (
        ("Correct", "correct_result_rate"),
        ("Addressable", "addressable_result_rate"),
        ("Exact re-resolution", "exact_reresolution_rate"),
    )
    rows = []
    summaries = []
    for index, (label, key) in enumerate(metrics):
        baseline = rate(require(live, "runs", "baseline", "reliability", key))
        current = rate(require(live, "runs", "current", "reliability", key))
        summaries.append(f"{label}: baseline {fmt(baseline, 0)} percent, current {fmt(current, 0)} percent")
        y = 42 + index * 67
        rows.append(f'<text x="8" y="{y + 14}">{html.escape(label)}</text>')
        for offset, (value, css_class, series) in enumerate(
            ((baseline, "baseline", "Baseline"), (current, "current", "Current"))
        ):
            bar_y = y + offset * 22
            rows.append(
                f'<rect class="track" x="148" y="{bar_y}" width="390" height="16" rx="3" />'
                f'<rect class="{css_class}" x="148" y="{bar_y}" width="{3.9 * value:.2f}" height="16" rx="3" />'
                f'<text x="548" y="{bar_y + 13}">{series} {fmt(value, 0)}%</text>'
            )
    summary = "; ".join(summaries)
    return (
        '<svg class="chart wide" viewBox="0 0 680 250" role="img" '
        f'aria-label="{html.escape(summary)}"><title>Live reliability</title>'
        f"<desc>{html.escape(summary)}</desc>"
        + "".join(rows)
        + "</svg>"
    )


def svg_speedups(synthetic):
    speedups = [
        (
            SCENARIO_LABELS.get(item["name"], item["name"].replace("_", " ").title()),
            float(require(item, "comparison", "p50_find_speedup")),
            float(require(item, "comparison", "p95_find_speedup")),
        )
        for item in require(synthetic, "scenarios")
    ]
    maximum = max(max(row[1:]) for row in speedups) * 1.15
    rows = []
    for index, (label, p50, p95) in enumerate(speedups):
        y = 34 + index * 64
        rows.append(f'<text x="8" y="{y + 13}">{html.escape(label)}</text>')
        for offset, (value, css_class, percentile) in enumerate(
            ((p50, "current", "p50"), (p95, "synthetic-p95", "p95"))
        ):
            bar_y = y + offset * 22
            width = value / maximum * 365
            rows.append(
                f'<rect class="{css_class}" x="220" y="{bar_y}" width="{width:.2f}" height="16" rx="3" />'
                f'<text x="{min(650, 228 + width):.2f}" y="{bar_y + 13}">{percentile} {fmt(value, 2)}×</text>'
            )
    summary = "; ".join(f"{label}: p50 {fmt(p50, 2)} times, p95 {fmt(p95, 2)} times" for label, p50, p95 in speedups)
    return (
        '<svg class="chart wide" viewBox="0 0 680 310" role="img" '
        f'aria-label="Synthetic find speedup. {html.escape(summary)}">'
        "<title>Synthetic find speedup</title>"
        f"<desc>{html.escape(summary)}</desc>"
        + "".join(rows)
        + "</svg>"
    )


def svg_binary_size(baseline_bytes, current_bytes):
    maximum = max(baseline_bytes, current_bytes) * 1.15
    bars = []
    for index, (label, value, css_class) in enumerate(
        (("Baseline", baseline_bytes, "baseline"), ("Current", current_bytes, "current"))
    ):
        width = value / maximum * 470
        y = 45 + index * 62
        bars.append(
            f'<text x="8" y="{y + 17}">{label}</text>'
            f'<rect class="{css_class}" x="96" y="{y}" width="{width:.2f}" height="24" rx="4" />'
            f'<text x="{min(650, 106 + width):.2f}" y="{y + 17}">{value / 1_000_000:.3f} MB</text>'
        )
    return (
        '<svg class="chart wide" viewBox="0 0 680 180" role="img" '
        f'aria-label="Baseline binary {baseline_bytes} bytes; current binary {current_bytes} bytes.">'
        "<title>Release binary size</title><desc>Decimal megabytes. The current binary remains below the 15 megabyte project ceiling.</desc>"
        + "".join(bars)
        + "</svg>"
    )


def render_report(synthetic, live, baseline_bytes, current_bytes, current_sha256=None):
    wall = tuple(
        require(live, "runs", name, "metrics", "end_to_end_wall_all_attempts", percentile)
        for name in ("baseline", "current")
        for percentile in ("p50", "p95")
    )
    cpu = tuple(
        require(live, "runs", name, "metrics", "process_cpu_all_attempts", percentile)
        for name in ("baseline", "current")
        for percentile in ("p50", "p95")
    )
    pairs = int(require(live, "paired_comparison", "comparable_successful_pairs"))
    faster_rate = rate(require(live, "paired_comparison", "current_minus_baseline", "current_faster_wall_rate"))
    delta_p50 = require(live, "paired_comparison", "current_minus_baseline", "wall", "p50")
    wall_change, wall_direction = directional_change(
        delta_p50, "reduction", "increase", "change"
    )
    reliability_keys = (
        "correct_result_rate",
        "addressable_result_rate",
        "exact_reresolution_rate",
    )
    current_reliability = min(
        rate(require(live, "runs", "current", "reliability", key))
        for key in reliability_keys
    )
    synthetic_scenarios = require(synthetic, "scenarios")
    synthetic_total = len(synthetic_scenarios)
    legacy_synthetic_correct = sum(
        bool(require(item, "legacy_snapshot", "correct_all_runs"))
        for item in synthetic_scenarios
    )
    current_synthetic_correct = sum(
        bool(require(item, "live_find_selected_refs", "correct_all_runs"))
        and bool(require(item, "live_find_selected_refs", "selected_refs_reresolvable"))
        for item in synthetic_scenarios
    )
    size_percent, size_direction, size_delta = size_change(baseline_bytes, current_bytes)
    captured_at = html.escape(str(live.get("generated_at", "unknown time")))
    measured_sha = str(require(live, "runs", "current", "binary", "sha256"))
    if current_sha256 and measured_sha != current_sha256:
        live_evidence_note = (
            f'<p class="evidence-warning">Live evidence was captured at {captured_at} '
            f'against build <code>{html.escape(measured_sha[:12])}</code>. The final build '
            f'<code>{html.escape(current_sha256[:12])}</code> differs and could not be re-probed; '
            'treat live latency as pre-final evidence.</p>'
        )
    else:
        live_evidence_note = (
            f'<p class="note">Live evidence captured at {captured_at} against build '
            f'<code>{html.escape(measured_sha[:12])}</code>.</p>'
        )
    return f'''<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>agent-desktop performance evidence</title>
<style>
:root{{--bg:#f6f7f9;--surface:#fff;--text:#172033;--muted:#566176;--border:#d9deea;--baseline:#667085;--current:#167a5b;--p95:#2463a3;--faster:#167a5b;--slower:#b54708;--track:#e8ebf1}}
@media(prefers-color-scheme:dark){{:root{{--bg:#10131a;--surface:#181d27;--text:#eef1f7;--muted:#aeb7c8;--border:#343c4b;--baseline:#98a2b3;--current:#55c69a;--p95:#6da9e3;--faster:#55c69a;--slower:#f0a35b;--track:#2a3140}}}}
*{{box-sizing:border-box}} body{{margin:0;background:var(--bg);color:var(--text);font:15px/1.55 system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}} main{{max-width:1120px;margin:auto;padding:32px 20px 64px}} header{{display:flex;justify-content:space-between;gap:20px;align-items:flex-start;margin-bottom:28px}} h1{{font-size:clamp(1.7rem,4vw,2.7rem);line-height:1.1;margin:0 0 10px}} h2{{font-size:1.35rem;margin:0 0 8px}} p{{margin:0 0 12px}} .lede,.note{{color:var(--muted)}} .evidence-warning{{color:var(--slower);font-weight:600}} button{{font:inherit;color:var(--text);background:var(--surface);border:1px solid var(--border);border-radius:7px;padding:8px 12px;cursor:pointer}} button:focus-visible{{outline:3px solid var(--p95);outline-offset:2px}} .summary{{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:14px;margin:24px 0 34px}} .stat,.panel{{background:var(--surface);border:1px solid var(--border);border-radius:10px}} .stat{{padding:16px}} .stat strong{{display:block;font-size:1.55rem;font-weight:500}} .stat span{{color:var(--muted)}} .grid{{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:18px}} .panel{{padding:18px;min-width:0}} .panel.wide{{grid-column:1/-1}} .chart{{display:block;width:100%;height:auto;margin-top:8px}} .chart text{{fill:var(--text);font:12px system-ui,sans-serif}} .chart .muted{{fill:var(--muted)}} .axis{{stroke:var(--border);stroke-width:2}} .baseline{{fill:var(--baseline)}} .current{{fill:var(--current)}} .synthetic-p95{{fill:var(--p95)}} .track{{fill:var(--track)}} .faster{{fill:var(--faster)}} .slower{{fill:var(--slower)}} .legend{{display:flex;gap:18px;flex-wrap:wrap;color:var(--muted);font-size:.9rem}} .swatch{{display:inline-block;width:11px;height:11px;border-radius:2px;margin-right:6px}} .limitations{{margin-top:28px;padding-top:24px;border-top:1px solid var(--border)}} li{{margin:6px 0}} code{{font-size:.9em}} @media(max-width:720px){{header{{display:block}} header button{{margin-top:12px}} .summary,.grid{{grid-template-columns:1fr}} .panel.wide{{grid-column:auto}} main{{padding:24px 12px 48px}}}}
@media print{{button{{display:none}} body{{background:#fff}} .panel,.stat{{break-inside:avoid}}}}
</style></head><body><main>
<header><div><h1>agent-desktop performance evidence</h1><p class="lede">Baseline versus current branch, with live read-only Electron evidence separated from deterministic synthetic locator evidence.</p></div><button id="print-report" type="button">Print report</button></header>
{live_evidence_note}
<section class="summary" aria-label="Key results"><div class="stat"><strong>{fmt(wall_change)} ms</strong><span>live paired p50 wall-time {wall_direction}</span></div><div class="stat"><strong>{fmt(faster_rate, 1)}%</strong><span>live pairs where current was faster</span></div><div class="stat"><strong>{fmt(current_reliability, 1)}%</strong><span>lowest current correctness reliability rate</span></div></section>
<div class="grid">
<section class="panel"><h2>Live wall time</h2><p class="note">Slack (read-only), {pairs} successful paired runs. Lower is better.</p><div class="legend"><span><i class="swatch baseline"></i>Baseline</span><span><i class="swatch current"></i>Current</span></div>{svg_grouped_bars("Live end-to-end wall time", (wall[0], wall[1]), (wall[2], wall[3]), "ms")}</section>
<section class="panel"><h2>Live process CPU</h2><p class="note">Per-command user + system CPU. Lower is better.</p><div class="legend"><span><i class="swatch baseline"></i>Baseline</span><span><i class="swatch current"></i>Current</span></div>{svg_grouped_bars("Live process CPU", (cpu[0], cpu[1]), (cpu[2], cpu[3]), "ms")}</section>
<section class="panel wide"><h2>Paired live wall-time deltas</h2><p class="note">All {pairs} current-minus-baseline deltas, ordered by value. Negative means current is faster.</p>{svg_delta_plot(paired_wall_deltas(live))}</section>
<section class="panel wide"><h2>Live locator reliability</h2><p class="note">Command exit success alone was 100% for both binaries; correctness, addressability, and exact re-resolution distinguish usable results.</p>{svg_rate_chart(live)}</section>
<section class="panel wide"><h2>Synthetic locator speedup</h2><p class="note">31 measured runs after 5 warmups per deterministic Chromium/Electron fixture. Current find was correct and re-resolvable in {current_synthetic_correct}/{synthetic_total} scenarios; legacy was correct in {legacy_synthetic_correct}/{synthetic_total}. Speed ratios are timing evidence, not a substitute for correctness. These timings exclude native accessibility IPC.</p><div class="legend"><span><i class="swatch current"></i>p50 find speedup</span><span><i class="swatch synthetic-p95"></i>p95 find speedup</span></div>{svg_speedups(synthetic)}</section>
<section class="panel wide"><h2>Release binary size</h2><p class="note">Current is {fmt(size_percent, 2)}% {size_direction} ({size_delta} bytes), while remaining far below the 15 MB ceiling.</p>{svg_binary_size(baseline_bytes, current_bytes)}</section>
</div>
<section class="limitations"><h2>Scope and limitations</h2><ul><li>Live data is one machine, one app state, and one macOS accessibility-permission state; it is not a cross-machine claim.</li><li>The live operation was read-only <code>find</code> plus exact ref re-resolution. No click, typing, navigation, or message mutation occurred.</li><li>Synthetic data measures platform-neutral locator work on deterministic Electron-shaped fixtures; it does not measure native IPC or whole-command latency.</li><li>Binary size is {fmt(size_percent, 2)}% {size_direction}; performance evidence does not erase packaging changes.</li></ul></section>
</main><script>document.getElementById("print-report").addEventListener("click",function(){{window.print()}});</script></body></html>'''


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description="Generate the sanitized agent-desktop performance report")
    parser.add_argument("--synthetic", type=Path, required=True, help="synthetic locator benchmark JSON")
    parser.add_argument("--live", type=Path, required=True, help="paired live benchmark JSON")
    parser.add_argument("--output", type=Path, required=True, help="standalone HTML destination")
    parser.add_argument("--baseline-bytes", type=int, default=DEFAULT_BASELINE_BYTES)
    parser.add_argument("--current-bytes", type=int, default=DEFAULT_CURRENT_BYTES)
    parser.add_argument("--current-sha256")
    return parser.parse_args(argv)


def main(argv=None):
    args = parse_args(argv)
    if args.baseline_bytes <= 0 or args.current_bytes <= 0:
        raise ValueError("binary sizes must be positive")
    report = render_report(
        load_json(args.synthetic),
        load_json(args.live),
        args.baseline_bytes,
        args.current_bytes,
        args.current_sha256,
    )
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(report, encoding="utf-8")


if __name__ == "__main__":
    main()
