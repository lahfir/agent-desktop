#!/usr/bin/env python3
"""Render perf-baseline-compare JSON artifacts into one self-contained HTML report.

Usage: python3 scripts/perf_report_html.py <out_dir> --head <sha> --base <sha>
Reads fixture-ab.json, app-*.json, and locator-synthetic.json from <out_dir> (any may
be absent) and writes <out_dir>/report.html. Stdlib only.
"""
import argparse
import datetime
import glob
import json
import os
import sys

import perf_report_svg as chart
from perf_report_svg import esc

CSS = """
:root {
  --surface:#fcfcfb; --page:#f9f9f7; --ink:#0b0b0b; --secondary:#52514e; --muted:#898781;
  --grid:#e1e0d9; --baseline:#c3c2b7; --border:rgba(11,11,11,0.10);
  --head:#2a78d6; --base:#1baf7a; --good:#006300; --status-good:#0ca30c;
  --good-tint:rgba(0,99,0,0.10); --status-good-tint:rgba(12,163,12,0.12);
  --secondary-tint:rgba(82,81,78,0.09); --head-tint:rgba(42,120,214,0.10);
}
@media (prefers-color-scheme:dark) {
  :root {
    --surface:#1a1a19; --page:#0d0d0d; --ink:#ffffff; --secondary:#c3c2b7; --muted:#898781;
    --grid:#2c2c2a; --baseline:#383835; --border:rgba(255,255,255,0.10);
    --head:#3987e5; --base:#199e70; --good:#0ca30c; --status-good:#0ca30c;
    --good-tint:rgba(12,163,12,0.16); --status-good-tint:rgba(12,163,12,0.16);
    --secondary-tint:rgba(195,194,183,0.10); --head-tint:rgba(57,135,229,0.14);
  }
}
* { box-sizing:border-box; }
body { margin:0; background:var(--page); color:var(--ink); line-height:1.5;
  font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,Helvetica,Arial,sans-serif; }
.col { max-width:1040px; margin:0 auto; padding:0 24px 48px; display:flex; flex-direction:column; gap:24px; }
header.band, footer.band { max-width:1040px; margin:0 auto; padding:32px 24px 8px; }
footer.band { padding:8px 24px 40px; }
header.band h1 { font-size:22px; margin:0 0 6px; letter-spacing:-0.01em; }
header.band .subtitle, footer.band p { color:var(--secondary); font-size:13px; margin:4px 0; }
footer.band code, .card code { background:var(--secondary-tint); border-radius:4px; padding:1px 6px; font-size:12px; }
.tiles { max-width:1040px; margin:0 auto; padding:8px 24px 0; display:grid;
  grid-template-columns:repeat(auto-fit,minmax(190px,1fr)); gap:16px; }
.tile { background:var(--surface); border:1px solid var(--border); border-radius:12px; padding:18px 20px; }
.tile-label { font-size:12px; color:var(--muted); margin-bottom:8px; }
.tile-value { font-size:26px; font-weight:600; }
.tile-delta { font-size:12px; margin-top:6px; }
.tile-delta.good { color:var(--good); } .tile-delta.secondary { color:var(--secondary); } .tile-delta.muted { color:var(--muted); }
.card { background:var(--surface); border:1px solid var(--border); border-radius:12px; padding:28px; }
.card-head { display:flex; align-items:center; justify-content:space-between; flex-wrap:wrap; gap:12px; }
.card h2 { font-size:16px; margin:0; }
.hint, .shape-line { font-size:12.5px; color:var(--secondary); }
.hint { margin:6px 0 4px; } .shape-line { margin:12px 0 0; }
.legend, .legend-item { display:flex; align-items:center; }
.legend { gap:16px; font-size:12px; color:var(--secondary); } .legend-item { gap:6px; }
.swatch { width:10px; height:10px; border-radius:2px; display:inline-block; }
.swatch-head { background:var(--head); } .swatch-base { background:var(--base); }
.callout { border:1px solid var(--border); background:var(--head-tint); border-radius:10px;
  padding:14px 16px; font-size:13px; color:var(--secondary); margin:16px 0 4px; }
.callout strong { color:var(--ink); }
svg.chart { display:block; width:100%; height:auto; margin:16px 0 4px; }
svg.chart text { font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,Helvetica,Arial,sans-serif; }
.gridline { stroke:var(--grid); stroke-width:1; } .tick { stroke:var(--ink); stroke-width:1.4; opacity:0.45; }
.parity { stroke:var(--baseline); stroke-width:1.5; stroke-dasharray:3 3; }
.bar-head { fill:var(--head); } .bar-base { fill:var(--base); }
.ink { fill:var(--ink); } .secondary { fill:var(--secondary); } .muted { fill:var(--muted); }
.tabular { font-variant-numeric:tabular-nums; }
.hit { fill:transparent; cursor:pointer; }
.value-label { font-weight:600; }
.chip-good { fill:var(--good-tint); } .chip-secondary, .chip-info { fill:var(--secondary-tint); }
.chip-status-good { fill:var(--status-good-tint); }
.chip-text-good { fill:var(--good); } .chip-text-secondary, .chip-text-info { fill:var(--secondary); }
.chip-text-status-good { fill:var(--status-good); }
details { margin-top:14px; }
summary { cursor:pointer; font-size:13px; color:var(--secondary); }
.table-wrap { overflow-x:auto; }
table { width:100%; border-collapse:collapse; margin-top:12px; font-size:12.5px; font-variant-numeric:tabular-nums; }
th, td { text-align:right; padding:7px 10px; border-bottom:1px solid var(--border); white-space:nowrap; }
th:first-child, td:first-child { text-align:left; }
th { color:var(--muted); font-weight:500; }
.tooltip { position:fixed; pointer-events:none; opacity:0; transition:opacity .08s ease; background:var(--ink);
  color:var(--surface); border-radius:8px; padding:8px 10px; font-size:12px; z-index:20; max-width:240px; }
.tooltip .tt-title { font-weight:600; margin-bottom:4px; }
.tooltip .tt-row { display:flex; justify-content:space-between; gap:12px; opacity:.85; }
@media (max-width:640px) { .card { padding:20px; } .tile-value { font-size:22px; } }
"""

JS = """
(function () {
  var tip = document.getElementById('tt');
  document.querySelectorAll('[data-tt-title]').forEach(function (el) {
    el.addEventListener('mousemove', function (evt) {
      var rows = (el.getAttribute('data-tt-rows') || '').split('|').filter(Boolean);
      var html = '<div class="tt-title">' + el.getAttribute('data-tt-title') + '</div>';
      rows.forEach(function (row) {
        var i = row.indexOf('=');
        html += '<div class="tt-row"><span>' + row.slice(0, i) + '</span><b>' + row.slice(i + 1) + '</b></div>';
      });
      tip.innerHTML = html;
      tip.style.left = Math.min(evt.clientX + 14, window.innerWidth - 250) + 'px';
      tip.style.top = (evt.clientY + 14) + 'px'; tip.style.opacity = '1';
    });
    el.addEventListener('mouseleave', function () { tip.style.opacity = '0'; });
  });
})();
"""

PAGE = """<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8"/>
<meta name="viewport" content="width=device-width, initial-scale=1"/>
<title>{title}</title>
<style>{css}</style>
</head>
<body>
{body}
<div id="tt" class="tooltip" role="status"></div>
<script>{js}</script>
</body>
</html>
"""


def load(path):
    if not os.path.exists(path):
        return None
    with open(path) as handle:
        return json.load(handle)


def fmt_ms(value):
    return f"{value:.1f} ms"


def ok_pct(value):
    return f"{value * 100:.0f}%"


def tip_rows(entry):
    return [("p50", fmt_ms(entry["p50_ms"])), ("p95", fmt_ms(entry["p95_ms"])),
            ("ok rate", ok_pct(entry.get("ok_rate", 1.0)))]


def build_ab_rows(payload):
    rows = []
    for name, sides in payload.get("cases", {}).items():
        head, base = sides.get("HEAD"), sides.get("BASE")
        if not head or not base:
            continue
        delta_text, delta_cls = chart.fmt_delta(head["p50_ms"], base["p50_ms"])
        rows.append({
            "label": name,
            "head": {"value": head["p50_ms"], "tick": head["p95_ms"],
                      "tip_title": f"{name} — HEAD", "tip_rows": tip_rows(head)},
            "base": {"value": base["p50_ms"], "tick": base["p95_ms"],
                      "tip_title": f"{name} — BASE", "tip_rows": tip_rows(base)},
            "delta_text": delta_text, "delta_good": delta_cls == "good",
        })
    return rows


def build_single_rows(payload):
    rows = []
    for name, sides in payload.get("cases", {}).items():
        head, base = sides.get("HEAD"), sides.get("BASE")
        if not head or base:
            continue
        label = name.replace(" (HEAD-only)", "")
        rows.append({
            "label": label, "value": head["p50_ms"], "tick": head["p95_ms"],
            "tip_title": f"{label} — HEAD", "tip_rows": tip_rows(head),
            "badge_text": "new in HEAD - no baseline equivalent",
        })
    return rows


def case_shapes(payload, case_name):
    sides = payload.get("cases", {}).get(case_name, {})
    return (sides.get("HEAD") or {}).get("shape"), (sides.get("BASE") or {}).get("shape")


def shape_line(payload):
    head_shape, base_shape = case_shapes(payload, "snapshot d30")
    if not head_shape or not base_shape:
        return None
    hn, hd, hr = head_shape
    bn, bd, br = base_shape
    return f"HEAD d30: {hn} nodes / depth {hd} / {hr} refs — BASE d30: {bn} nodes / depth {bd} / {br} refs"


def needs_honesty_callout(payload):
    head_shape, base_shape = case_shapes(payload, "snapshot -i")
    if not head_shape or not base_shape:
        return False
    return base_shape[0] > 0 and head_shape[0] < 0.5 * base_shape[0]


def build_speedup_rows(report):
    rows = []
    for sc in report.get("scenarios", []):
        legacy, live, comp = sc["legacy_snapshot"], sc["live_find_selected_refs"], sc["comparison"]
        attr_delta = chart.pct_delta(live["attributes_requested"], legacy["attributes_requested"])
        rows.append({
            "label": sc["name"], "speedup": comp["p50_find_speedup"], "tip_title": sc["name"],
            "tip_rows": [
                ("legacy p50", f'{legacy["p50_us"] / 1000:.2f} ms'), ("live p50", f'{live["p50_us"] / 1000:.2f} ms'),
                ("legacy p95", f'{legacy["p95_us"] / 1000:.2f} ms'), ("live p95", f'{live["p95_us"] / 1000:.2f} ms'),
                ("attrs legacy/live", f'{legacy["attributes_requested"]} / {live["attributes_requested"]}'),
            ],
            "correctness_fix": comp.get("find_correctness_delta", 0) > 0,
            "sub_label": f"{attr_delta:+.0f}% attribute reads" if attr_delta is not None else None,
        })
    return rows


def tile(label, value, head, base):
    delta_text, cls = chart.fmt_delta(head, base)
    return {"label": label, "value": value, "delta_text": f"{delta_text} vs base" if delta_text else "n/a", "cls": cls}


def build_tiles(fixture, synthetic):
    tiles = []
    if fixture:
        click = fixture.get("cases", {}).get("click", {})
        if click.get("HEAD") and click.get("BASE"):
            h, b = click["HEAD"]["p50_ms"], click["BASE"]["p50_ms"]
            tiles.append(tile("Click p50 (fixture)", fmt_ms(h), h, b))
        d30 = fixture.get("cases", {}).get("snapshot d30", {})
        if d30.get("HEAD") and d30.get("BASE"):
            h, b = d30["HEAD"]["p50_ms"], d30["BASE"]["p50_ms"]
            tiles.append(tile("Snapshot d30 p50 (fixture)", fmt_ms(h), h, b))
    scenarios = (synthetic or {}).get("scenarios") or []
    if scenarios:
        best = max(scenarios, key=lambda s: s["comparison"]["p50_find_speedup"])
        tiles.append({"label": "Best synthetic find speedup", "value": f'{best["comparison"]["p50_find_speedup"]:.1f}x',
                       "delta_text": best["name"].replace("_", " "), "cls": "muted"})
        fixes = sum(1 for s in scenarios if s["comparison"].get("find_correctness_delta", 0) > 0)
        tiles.append({"label": "Correctness fixes", "value": f"{fixes} scenario{'' if fixes == 1 else 's'}",
                       "delta_text": "legacy path answered incorrectly" if fixes else "no legacy mismatches observed",
                       "cls": "good" if fixes else "muted"})
    return tiles


def tiles_html(tiles):
    if not tiles:
        return ""
    cells = "".join(
        f'<div class="tile"><div class="tile-label">{esc(t["label"])}</div>'
        f'<div class="tile-value">{esc(t["value"])}</div>'
        f'<div class="tile-delta {t["cls"]}">{esc(t["delta_text"])}</div></div>'
        for t in tiles
    )
    return f'<div class="tiles">{cells}</div>'


LEGEND = ('<div class="legend"><span class="legend-item"><span class="swatch swatch-head"></span>HEAD</span>'
          '<span class="legend-item"><span class="swatch swatch-base"></span>BASE</span></div>')


def table_html(headers, rows):
    thead = "".join(f"<th>{esc(h)}</th>" for h in headers)
    trs = "".join("<tr>" + "".join(f"<td>{esc(c)}</td>" for c in r) + "</tr>" for r in rows)
    return f'<div class="table-wrap"><table><thead><tr>{thead}</tr></thead><tbody>{trs}</tbody></table></div>'


def case_rows(payload, require_base):
    rows = []
    for name, sides in payload.get("cases", {}).items():
        head, base = sides.get("HEAD"), sides.get("BASE")
        if not head or (require_base and not base):
            continue
        delta_text = chart.fmt_delta(head["p50_ms"], base["p50_ms"])[0] if base else None
        ok = (f'{ok_pct(base.get("ok_rate", 1.0))}/{ok_pct(head.get("ok_rate", 1.0))}' if base
              else f'-/{ok_pct(head.get("ok_rate", 1.0))}')
        rows.append([name, fmt_ms(base["p50_ms"]) if base else "-", fmt_ms(base["p95_ms"]) if base else "-",
                     fmt_ms(head["p50_ms"]), fmt_ms(head["p95_ms"]), delta_text or "n/a", ok])
    return rows


def fixture_section(payload):
    if not payload:
        return ""
    rows = build_ab_rows(payload)
    table = table_html(["Case", "BASE p50", "BASE p95", "HEAD p50", "HEAD p95", "Delta", "OK B/H"],
                        case_rows(payload, require_base=True))
    return (
        '<section class="card">'
        f'<div class="card-head"><h2>Fixture A/B (live)</h2>{LEGEND if rows else ""}</div>'
        f'<p class="hint">Alternating rounds, isolated per-binary stores, wall-clock per process invocation '
        f'({payload.get("rounds", "?")} rounds).</p>{chart.grouped_bar_chart(rows)}'
        f'<details><summary>Table view</summary>{table}</details></section>'
    )


def app_section(payload):
    if not payload:
        return ""
    ab_rows, single_rows = build_ab_rows(payload), build_single_rows(payload)
    if needs_honesty_callout(payload):
        for row in ab_rows:
            if row["label"] == "snapshot -i":
                row["delta_text"], row["delta_good"] = "not comparable", False
    body = [f'<div class="card-head"><h2>{esc(payload["app"])}</h2>{LEGEND if ab_rows else ""}</div>']
    body.append(f'<p class="hint">Read-only observation ({payload.get("rounds", "?")} rounds).</p>')
    if ab_rows:
        body.append(chart.grouped_bar_chart(ab_rows))
    if needs_honesty_callout(payload):
        body.append(
            '<div class="callout"><strong>Reading note.</strong> HEAD&rsquo;s default <code>snapshot -i</code> '
            'reads a shallower tree than BASE under the progressive-traversal default-depth change, so the '
            '<code>snapshot -i</code> row above is not like-for-like. The <code>snapshot d30</code> rows are '
            'the apples-to-apples comparison.</div>')
    shape = shape_line(payload)
    if shape:
        body.append(f'<p class="shape-line tabular">{esc(shape)}</p>')
    if single_rows:
        body.append(chart.single_series_bar_chart(single_rows))
    table = table_html(["Case", "BASE p50", "BASE p95", "HEAD p50", "HEAD p95", "Delta", "OK B/H"],
                        case_rows(payload, require_base=False))
    body.append(f'<details><summary>Table view</summary>{table}</details>')
    return f'<section class="card">{"".join(body)}</section>'


def synthetic_section(report):
    if not report:
        return ""
    rows = build_speedup_rows(report)
    method = report.get("methodology", {})
    hint = (f'{method.get("measured_runs", "?")} measured runs, {method.get("warmup_runs", "?")} warmup. '
            f'Legacy: {method.get("legacy_path", "legacy snapshot path")}. '
            f'Live: {method.get("live_find_path", "live find path")}.')
    table_rows = []
    for sc in report.get("scenarios", []):
        legacy, live, comp = sc["legacy_snapshot"], sc["live_find_selected_refs"], sc["comparison"]
        table_rows.append([sc["name"], f'{legacy["p50_us"] / 1000:.2f} ms', f'{legacy["p95_us"] / 1000:.2f} ms',
                            f'{live["p50_us"] / 1000:.2f} ms', f'{live["p95_us"] / 1000:.2f} ms',
                            legacy["attributes_requested"], live["attributes_requested"],
                            "yes" if comp.get("find_correctness_delta", 0) > 0 else "no"])
    table = table_html(["Scenario", "Legacy p50", "Legacy p95", "Live p50", "Live p95", "Legacy attrs", "Live attrs", "Fix"],
                        table_rows)
    return (
        '<section class="card"><div class="card-head"><h2>Synthetic locator benchmark</h2></div>'
        f'<p class="hint">{esc(hint)}</p>{chart.speedup_bar_chart(rows)}'
        f'<details><summary>Table view</summary>{table}</details></section>'
    )


def page_chrome(head, base, rounds, out_dir):
    rounds_part = f" · {rounds} rounds" if rounds else ""
    header = (
        '<header class="band">'
        f'<h1>agent-desktop performance - HEAD {esc(head)} vs base {esc(base)}</h1>'
        f'<p class="subtitle">{datetime.date.today().isoformat()}{rounds_part} · alternating rounds, '
        'isolated per-binary stores, wall-clock per process invocation</p></header>'
    )
    footer = (
        '<footer class="band">'
        '<p>Regenerate: <code>bash scripts/perf-baseline-compare.sh --apps "Slack,Google Chrome"</code></p>'
        f'<p>Artifacts: <code>{esc(out_dir)}</code> · '
        f'generated {esc(datetime.datetime.now().isoformat(timespec="seconds"))}</p></footer>'
    )
    return header, footer


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("out_dir")
    parser.add_argument("--head", required=True)
    parser.add_argument("--base", required=True)
    args = parser.parse_args()

    fixture = load(os.path.join(args.out_dir, "fixture-ab.json"))
    synthetic = load(os.path.join(args.out_dir, "locator-synthetic.json"))
    apps = [load(p) for p in sorted(glob.glob(os.path.join(args.out_dir, "app-*.json")))]
    apps = [a for a in apps if a]
    sections = [fixture_section(fixture)] + [app_section(a) for a in apps] + [synthetic_section(synthetic)]
    tiles = build_tiles(fixture, synthetic)
    rounds = fixture.get("rounds") if fixture else None
    header, footer = page_chrome(args.head, args.base, rounds, os.path.abspath(args.out_dir))
    body = header + tiles_html(tiles) + '<main class="col">' + "".join(sections) + "</main>" + footer
    title = f"agent-desktop performance - HEAD {args.head} vs base {args.base}"
    html_doc = PAGE.format(title=esc(title), css=CSS, body=body, js=JS)

    out_path = os.path.join(args.out_dir, "report.html")
    with open(out_path, "w") as handle:
        handle.write(html_doc)

    print(f"agent-desktop performance report: HEAD {args.head} vs base {args.base}")
    for t in tiles:
        print(f'  {t["label"]}: {t["value"]} ({t["delta_text"]})')
    print(f"  apps covered: {', '.join(a['app'] for a in apps) if apps else 'none'}")
    print(f"  report: {out_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
