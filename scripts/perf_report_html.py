#!/usr/bin/env python3
"""Render fixture-ab.json, app-*.json and locator-synthetic.json to report.html."""
import argparse
import datetime
import json
import math
from html import escape
from pathlib import Path


def table(headers, rows):
    head = ''.join(f'<th scope="col">{escape(str(h))}</th>' for h in headers)
    body = ''.join('<tr>' + ''.join(f'<td>{escape(str(c))}</td>' for c in row) + '</tr>' for row in rows)
    return f'<div class="scroll"><table><thead><tr>{head}</tr></thead><tbody>{body}</tbody></table></div>'


def number(value, scale=1):
    return f'{value / scale:.2f}' if isinstance(value, (int, float)) and math.isfinite(value) else 'unknown'


def outcome(value):
    return 'PASS' if value is True else 'FAIL' if value is False else 'unknown'


def live_report(payload):
    rows = []
    for name, sides in payload.get('cases', {}).items():
        head, base = sides.get('HEAD') or {}, sides.get('BASE') or {}
        shapes_match = head.get('shape') == base.get('shape')
        shapes_known = not name.startswith('snapshot') or head.get('shape') is not None
        h, b = head.get('p50_ms'), base.get('p50_ms')
        comparable = (head.get('ok_rate') == base.get('ok_rate') == 1 and shapes_match
                      and shapes_known and number(h) != 'unknown' and number(b) != 'unknown' and b > 0)
        delta = f'{(h / b - 1) * 100:+.1f}%' if comparable else 'not comparable'
        for label, side in [('BASE', base), ('HEAD', head)]:
            shape = side.get('shape')
            rows.append([name, label, number(side.get('p50_ms')), number(side.get('p95_ms')),
                         number(side.get('ok_rate'), 0.01), json.dumps(shape) if shape is not None else 'unknown',
                         delta if label == 'HEAD' else '—'])
    intro = (f'<p>{escape(str(payload.get("app", "unknown app")))}; '
             f'{escape(str(payload.get("rounds", "unknown")))} rounds. '
             'Latency uses successful samples only. OK rate includes failures; 100% OK does not verify action effects. '
             'Shape = [nodes, depth, refs] from the first successful snapshot, not proof of identical content. '
             'Deltas require successful runs and matching known snapshot shapes.</p>')
    return intro + (table(['Case', 'Side', 'p50 ms', 'p95 ms', 'OK %', 'Shape', 'p50 delta'], rows)
                    if rows else '<p>No case measurements available.</p>')


def synthetic_report(payload):
    body = ['<p>HEAD-only synthetic CPU benchmark: legacy and live paths in the same revision. '
            'This is not revision A/B and does not measure native IPC latency or target-app CPU usage.</p>',
            '<pre>' + escape(json.dumps(payload.get('methodology', {}), indent=2)) + '</pre>']
    scenarios = payload.get('scenarios', [])
    if not scenarios:
        body.append('<p>No synthetic scenarios available.</p>')
    for scenario in scenarios:
        body.append('<h3>' + escape(str(scenario.get('name', 'unknown scenario'))) + '</h3>')
        rows = []
        for key in ['legacy_snapshot', 'live_arena_direct', 'live_count_no_refmap', 'live_find_selected_refs']:
            path = scenario.get(key) or {}
            rows.append([key, number(path.get('p50_us'), 1000), number(path.get('p95_us'), 1000),
                         path.get('attributes_requested', 'unknown'), outcome(path.get('correct_all_runs')),
                         path.get('observed_matches', 'unknown'), path.get('ref_count', 'not applicable'),
                         outcome(path.get('selected_refs_reresolvable')) if key == 'live_find_selected_refs' else 'not applicable'])
        body.append('<p>Expected matches: ' + escape(str(scenario.get('expectation', {}).get('matches', 'unknown'))) + '</p>')
        body.append(table(['Path', 'p50 ms', 'p95 ms', 'Attributes', 'Correct all runs', 'Matches', 'Refs', 'Refs re-resolvable'], rows))
    return ''.join(body)


def section(path, synthetic=False):
    heading = '<h2>' + escape(path.name) + '</h2>'
    if not path.exists():
        return heading + '<p>Not collected: artifact missing.</p>'
    link = f'<p><a href="{escape(path.name, quote=True)}">Raw JSON</a></p>'
    try:
        payload = json.loads(path.read_text())
    except (OSError, ValueError) as error:
        return heading + link + '<p>Cannot read artifact: ' + escape(str(error)) + '</p>'
    return heading + link + (synthetic_report(payload) if synthetic else live_report(payload))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('out_dir', type=Path)
    parser.add_argument('--head', required=True)
    parser.add_argument('--base', required=True)
    args = parser.parse_args()
    title = f'Performance: HEAD {args.head} vs BASE {args.base}'
    body = ['<h1>' + escape(title) + '</h1>', '<p>Generated ' + datetime.datetime.now().isoformat(timespec='seconds') + '</p>',
            '<p>Live probes alternate revisions with isolated stores and measure wall-clock process latency. '
            'CPU consumption is not measured by the live probe.</p>', section(args.out_dir / 'fixture-ab.json')]
    apps = sorted(args.out_dir.glob('app-*.json'))
    body.extend(section(path) for path in apps)
    if not apps:
        body.append('<h2>Real applications</h2><p>No real-app artifacts collected.</p>')
    body.append(section(args.out_dir / 'locator-synthetic.json', synthetic=True))
    css = ('body{font:15px system-ui;margin:2rem;line-height:1.5}table{border-collapse:collapse}'
           'th,td{border:1px solid #aaa;padding:.4rem;text-align:left}th{background:#eee}'
           '.scroll{overflow-x:auto}pre{white-space:pre-wrap}td{font-variant-numeric:tabular-nums}')
    document = ('<!doctype html><html lang="en"><meta charset="utf-8">'
                '<meta name="viewport" content="width=device-width,initial-scale=1">'
                '<title>' + escape(title) + '</title><style>' + css + '</style><body>' + ''.join(body) + '</body></html>')
    output = args.out_dir / 'report.html'
    output.write_text(document)
    print(f'Performance report: {output}')


if __name__ == '__main__':
    main()
