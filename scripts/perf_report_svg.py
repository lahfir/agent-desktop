#!/usr/bin/env python3
"""Pure SVG chart builders for the agent-desktop performance report.

No third-party deps, no file or network I/O. Every public function takes
plain-Python row dicts (already formatted by the caller) and returns a
self-contained ``<svg>`` string. Colors are applied only via CSS classes
(``bar-head``, ``bar-base``, ``good``, ``secondary``, ``muted``, ``status-good``,
``chip-*``) so the same markup adapts to the host page's light/dark tokens.
"""
import math

VB_W = 860
BAR_H = 16
BAR_GAP = 2
ROW_GAP = 20
TOP_PAD = 18
AXIS_H = 30
CHIP_GAP = 14
LABEL_GAP = 14
CHECK = "✓"


def esc(value):
    text = str(value)
    return (
        text.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace('"', "&quot;")
        .replace("'", "&#39;")
    )


def pct_delta(head, base):
    if not base:
        return None
    return (head - base) / base * 100.0


def fmt_delta(head, base):
    delta = pct_delta(head, base)
    if delta is None:
        return None, "muted"
    return f"{delta:+.0f}%", ("good" if head < base else "secondary")


def nice_ticks(max_value, target=4):
    if max_value <= 0:
        return [0, 1]
    raw_step = max_value / target
    magnitude = 10 ** math.floor(math.log10(raw_step))
    step = magnitude
    for mult in (1, 2, 2.5, 5, 10):
        step = mult * magnitude
        if step >= raw_step:
            break
    ticks, value, limit = [], 0.0, max_value + step * 0.5
    while value <= limit:
        ticks.append(round(value, 6))
        value += step
    return ticks


def _clamp(value, lo, hi):
    return max(lo, min(hi, value))


def _measure(text, size):
    return len(str(text)) * size * 0.54


def _bar(x, y, w, h, cls):
    w = max(w, 0.0)
    if w <= 0:
        return ""
    r = min(4.0, w / 2, h / 2)
    out = [f'<rect class="{cls}" x="{x:.1f}" y="{y:.1f}" width="{w:.1f}" height="{h:.1f}" rx="{r:.1f}" ry="{r:.1f}"/>']
    if w > r:
        out.append(f'<rect class="{cls}" x="{x:.1f}" y="{y:.1f}" width="{r:.1f}" height="{h:.1f}"/>')
    return "".join(out)


def _tick(x, y, h, cls="tick"):
    return f'<line class="{cls}" x1="{x:.1f}" y1="{y - 3:.1f}" x2="{x:.1f}" y2="{y + h + 3:.1f}"/>'


def _hit(x, y, w, h, title, rows):
    row_str = esc("|".join(f"{label}={value}" for label, value in rows))
    return (
        f'<rect class="hit" x="{x:.1f}" y="{y:.1f}" width="{w:.1f}" height="{h:.1f}" '
        f'data-tt-title="{esc(title)}" data-tt-rows="{row_str}"/>'
    )


def _chip(x_right, y_mid, text, cls, size=11):
    w = _measure(text, size) + 18
    h = 20
    x, y = x_right - w, y_mid - h / 2
    return (
        f'<rect class="chip chip-{cls}" x="{x:.1f}" y="{y:.1f}" width="{w:.1f}" height="{h}" rx="10" ry="10"/>'
        f'<text class="chip-text chip-text-{cls}" x="{x_right - 9:.1f}" y="{y_mid + 4:.1f}" '
        f'text-anchor="end" font-size="{size}">{esc(text)}</text>'
    )


def _axis(sx, ticks, top, bottom, unit, suffix_x=False):
    out = []
    for tick in ticks:
        tx = sx(tick)
        out.append(f'<line class="gridline" x1="{tx:.1f}" y1="{top - 6:.1f}" x2="{tx:.1f}" y2="{bottom + 6:.1f}"/>')
        label = f"{tick:g}x" if suffix_x else f"{tick:g}{unit}"
        out.append(
            f'<text class="axis-tick muted tabular" x="{tx:.1f}" y="{bottom + 22:.1f}" '
            f'text-anchor="middle" font-size="10.5">{label}</text>'
        )
    return "".join(out)


def _grouped_summary(rows):
    best = max(rows, key=lambda r: abs(pct_delta(r["head"]["value"], r["base"]["value"]) or 0))
    delta = pct_delta(best["head"]["value"], best["base"]["value"]) or 0
    return (
        f"Grouped horizontal bar chart comparing HEAD and BASE p50 latency across {len(rows)} cases. "
        f"Largest change: {best['label']} at {delta:+.0f}%."
    )


def grouped_bar_chart(rows, unit=" ms"):
    if not rows:
        return '<svg class="chart" viewBox="0 0 860 40" role="img" aria-label="No data available"></svg>'
    label_w = _clamp(max(_measure(r["label"], 12.5) for r in rows) + LABEL_GAP, 120, 300)
    chip_w = _clamp(
        max((_measure(r["delta_text"], 11) + 18 for r in rows if r.get("delta_text")), default=50), 50, 200
    )
    x0, x1 = label_w, VB_W - chip_w - CHIP_GAP
    plot_w = max(x1 - x0, 40)
    data_max = max(max(r["head"]["value"], r["head"]["tick"], r["base"]["value"], r["base"]["tick"]) for r in rows)
    ticks = nice_ticks(data_max, 4)
    domain_max = (ticks[-1] or 1) * 1.24
    group_h = BAR_H * 2 + BAR_GAP
    row_pitch = group_h + ROW_GAP
    top = TOP_PAD
    bottom = top + row_pitch * len(rows) - ROW_GAP
    height = bottom + AXIS_H

    def sx(v):
        return x0 + (v / domain_max) * plot_w

    parts = [
        f'<svg class="chart" viewBox="0 0 {VB_W} {height:.0f}" width="100%" role="img" '
        f'aria-label="{esc(_grouped_summary(rows))}">',
        _axis(sx, ticks, top, bottom, unit),
    ]
    for i, row in enumerate(rows):
        y = top + i * row_pitch
        y_head, y_base, cy = y, y + BAR_H + BAR_GAP, y + group_h / 2
        parts.append(
            f'<text class="row-label ink" x="{x0 - LABEL_GAP:.1f}" y="{cy + 4:.1f}" '
            f'text-anchor="end" font-size="12.5">{esc(row["label"])}</text>'
        )
        for series, sy, cls in (("head", y_head, "bar-head"), ("base", y_base, "bar-base")):
            d = row[series]
            w = max((d["value"] / domain_max) * plot_w, 0)
            parts.append(_bar(x0, sy, w, BAR_H, cls))
            if d.get("tick") is not None:
                parts.append(_tick(sx(d["tick"]), sy, BAR_H))
            parts.append(
                f'<text class="value-label ink tabular" x="{x0 + w + 6:.1f}" y="{sy + BAR_H / 1.4:.1f}" '
                f'font-size="12">{d["value"]:.1f}{unit}</text>'
            )
            parts.append(_hit(0, sy, VB_W, BAR_H, d["tip_title"], d["tip_rows"]))
        if row.get("delta_text"):
            parts.append(_chip(VB_W, cy, row["delta_text"], "good" if row.get("delta_good") else "secondary"))
    parts.append("</svg>")
    return "".join(parts)


def _single_summary(rows):
    return f"Bar chart of {len(rows)} HEAD-only measurement(s) with no baseline equivalent."


def single_series_bar_chart(rows, unit=" ms"):
    if not rows:
        return '<svg class="chart" viewBox="0 0 860 40" role="img" aria-label="No data available"></svg>'
    label_w = _clamp(max(_measure(r["label"], 12.5) for r in rows) + LABEL_GAP, 120, 300)
    chip_w = _clamp(
        max((_measure(r["badge_text"], 10.5) + 24 for r in rows if r.get("badge_text")), default=0), 0, 320
    )
    gap = CHIP_GAP if chip_w else 0
    x0, x1 = label_w, VB_W - chip_w - gap
    plot_w = max(x1 - x0, 40)
    data_max = max(max(r["value"], r.get("tick") or 0) for r in rows)
    ticks = nice_ticks(data_max, 4)
    domain_max = (ticks[-1] or 1) * 1.24
    row_pitch = BAR_H + ROW_GAP
    top = TOP_PAD
    bottom = top + row_pitch * len(rows) - ROW_GAP
    height = bottom + AXIS_H

    def sx(v):
        return x0 + (v / domain_max) * plot_w

    parts = [
        f'<svg class="chart" viewBox="0 0 {VB_W} {height:.0f}" width="100%" role="img" '
        f'aria-label="{esc(_single_summary(rows))}">',
        _axis(sx, ticks, top, bottom, unit),
    ]
    for i, row in enumerate(rows):
        y = top + i * row_pitch
        cy = y + BAR_H / 2
        parts.append(
            f'<text class="row-label ink" x="{x0 - LABEL_GAP:.1f}" y="{cy + 4:.1f}" '
            f'text-anchor="end" font-size="12.5">{esc(row["label"])}</text>'
        )
        w = max((row["value"] / domain_max) * plot_w, 0)
        parts.append(_bar(x0, y, w, BAR_H, "bar-head"))
        if row.get("tick") is not None:
            parts.append(_tick(sx(row["tick"]), y, BAR_H))
        parts.append(
            f'<text class="value-label ink tabular" x="{x0 + w + 6:.1f}" y="{cy + 4:.1f}" '
            f'font-size="12">{row["value"]:.1f}{unit}</text>'
        )
        if row.get("badge_text"):
            parts.append(_chip(VB_W, cy, row["badge_text"], "info", size=10.5))
        parts.append(_hit(0, y, VB_W, BAR_H, row["tip_title"], row["tip_rows"]))
    parts.append("</svg>")
    return "".join(parts)


def _speedup_summary(rows):
    best = max(rows, key=lambda r: r["speedup"])
    worst = min(rows, key=lambda r: r["speedup"])
    return (
        f"Horizontal speedup bars for {len(rows)} synthetic scenarios, live find path versus legacy snapshot path. "
        f"Speedups range from {worst['speedup']:.1f}x to {best['speedup']:.1f}x against a 1.0x parity reference."
    )


def speedup_bar_chart(rows):
    if not rows:
        return '<svg class="chart" viewBox="0 0 860 40" role="img" aria-label="No data available"></svg>'
    badge_text = f"{CHECK} legacy answered wrong - live path correct"
    label_w = _clamp(max(_measure(r["label"], 11.5) for r in rows) + LABEL_GAP, 140, 340)
    chip_w = _clamp(_measure(badge_text, 10.5) + 24 if any(r.get("correctness_fix") for r in rows) else 0, 0, 340)
    gap = CHIP_GAP if chip_w else 0
    x0, x1 = label_w, VB_W - chip_w - gap
    plot_w = max(x1 - x0, 40)
    data_max = max([r["speedup"] for r in rows] + [1.0])
    ticks = nice_ticks(data_max, 4)
    domain_max = (ticks[-1] or 1) * 1.2
    row_pitch = BAR_H + ROW_GAP + 14
    top = TOP_PAD + 6
    bottom = top + row_pitch * len(rows) - ROW_GAP - 14
    height = bottom + AXIS_H

    def sx(v):
        return x0 + (v / domain_max) * plot_w

    parts = [
        f'<svg class="chart" viewBox="0 0 {VB_W} {height:.0f}" width="100%" role="img" '
        f'aria-label="{esc(_speedup_summary(rows))}">',
        _axis(sx, [t for t in ticks if abs(t - 1.0) > 1e-9], top, bottom, "", suffix_x=True),
    ]
    px = sx(1.0)
    parts.append(f'<line class="parity" x1="{px:.1f}" y1="{top - 6:.1f}" x2="{px:.1f}" y2="{bottom + 6:.1f}"/>')
    parts.append(
        f'<text class="muted" x="{px:.1f}" y="{top - 9:.1f}" text-anchor="middle" font-size="10.5">parity</text>'
    )
    for i, row in enumerate(rows):
        y = top + i * row_pitch
        cy = y + BAR_H / 2
        parts.append(
            f'<text class="row-label ink" x="{x0 - LABEL_GAP:.1f}" y="{cy + 4:.1f}" '
            f'text-anchor="end" font-size="11.5">{esc(row["label"])}</text>'
        )
        w = max((row["speedup"] / domain_max) * plot_w, 0)
        parts.append(_bar(x0, y, w, BAR_H, "bar-head"))
        end_x = x0 + w + 6
        parts.append(
            f'<text class="value-label ink tabular" x="{end_x:.1f}" y="{cy + 4:.1f}" '
            f'font-size="12">{row["speedup"]:.1f}x</text>'
        )
        if row.get("sub_label"):
            parts.append(
                f'<text class="muted" x="{end_x:.1f}" y="{cy + 17:.1f}" font-size="10">{esc(row["sub_label"])}</text>'
            )
        if row.get("correctness_fix"):
            parts.append(_chip(VB_W, cy, badge_text, "status-good", size=10.5))
        parts.append(_hit(0, y - 7, VB_W, BAR_H + 14, row["tip_title"], row["tip_rows"]))
    parts.append("</svg>")
    return "".join(parts)
