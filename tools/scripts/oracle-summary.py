"""Summarise an oracle run folder (frames.tsv, int21.tsv, pit.tsv, events.tsv) as Markdown.

Usage:
  python tools/scripts/oracle-summary.py <run-dir> [--loads] [--pit] [--window N]
                                            [--range A-B:label ...] [--int21-filter PROG]

Sections (all by default when no section flag is given):
  --loads    file opens in order: frame, ms, program, caller (image-relative), file, mode,
             bytes read while open, number of reads and seeks
  --pit      PIT channel programming (complete counts and control words)
  --window N per-frame counter statistics over consecutive windows of N frames
  --range    per-frame counter statistics over named frame ranges, e.g. --range 4000-4400:menu

Outputs only numbers and file names from the logs (no game content), so the tables may be
quoted in repo docs. The run folders themselves stay in the RE work area.
"""
import argparse
import csv
import pathlib
import statistics
import sys


def read_tsv(path: pathlib.Path):
    if not path.exists():
        return []
    with path.open(newline='', encoding='utf-8', errors='replace') as f:
        return list(csv.DictReader(f, delimiter='\t'))


def loads(run: pathlib.Path, prog_filter: str | None) -> None:
    rows = read_tsv(run / 'int21.tsv')
    open_fns = {'3C', '3D', '5A', '5B', '6C'}
    sessions = []          # dicts
    by_handle = {}         # (prog, handle) -> session
    for r in rows:
        if prog_filter and r['prog'].upper() != prog_filter.upper():
            continue
        ah = r['ah']
        if ah in open_fns and r['cf'] == '0':
            s = {'frame': r['frame'], 'ms': r['ms'], 'prog': r['prog'], 'rel': r['rel'], 'file': r['file'],
                 'arg': r['arg'], 'bytes': 0, 'reads': 0, 'seeks': 0, 'closed': ''}
            sessions.append(s)
            by_handle[r['handle']] = s
        elif ah in ('3F', '42', '3E') and r['handle'] in by_handle:
            s = by_handle[r['handle']]
            if ah == '3F' and r['cf'] == '0' and r['result'].startswith('bytes='):
                s['bytes'] += int(r['result'][6:])
                s['reads'] += 1
            elif ah == '42':
                s['seeks'] += 1
            elif ah == '3E':
                s['closed'] = r['frame']
                del by_handle[r['handle']]
        elif ah == '4B':
            sessions.append({'frame': r['frame'], 'ms': r['ms'], 'prog': r['prog'], 'rel': r['rel'],
                             'file': 'EXEC ' + r['file'], 'arg': r['arg'], 'bytes': '', 'reads': '', 'seeks': '',
                             'closed': ''})
    print('## File opens in order\n')
    print('| # | frame | ms | prog | caller (rel) | file | mode | bytes read | reads | seeks | closed at frame |')
    print('|---|---|---|---|---|---|---|---|---|---|---|')
    for i, s in enumerate(sessions, 1):
        print(f"| {i} | {s['frame']} | {float(s['ms']):.0f} | {s['prog']} | {s['rel']} | {s['file']} | {s['arg']} | "
              f"{s['bytes']} | {s['reads']} | {s['seeks']} | {s['closed']} |")
    print()


def pit(run: pathlib.Path) -> None:
    rows = read_tsv(run / 'pit.tsv')
    print('## PIT programming\n')
    print('| frame | ms | port | value | decoded |')
    print('|---|---|---|---|---|')
    for r in rows:
        if 'count=' in r['decoded'] or 'control' in r['decoded']:
            print(f"| {r['frame']} | {float(r['ms']):.1f} | {r['port']} | {r['val']} | {r['decoded']} |")
    print()


COUNTERS = ['dur_ms', 'irq0', 'pit_w', 'dac_3c8', 'dac_3c9', 'p3da', 'p3da_vr', 'int21']


def stats_table(frames, ranges) -> None:
    print('| range | label | frames | mode | ' + ' | '.join(f'{c} (min/med/max)' for c in COUNTERS) + ' |')
    print('|---|---|---|---|' + '---|' * len(COUNTERS))
    for a, b, label in ranges:
        sel = [f for f in frames if a <= int(f['frame']) <= b]
        if not sel:
            continue
        modes = sorted({f['mode'] for f in sel})
        cells = []
        for c in COUNTERS:
            vals = [float(f[c]) for f in sel]
            fmt = (lambda v: f'{v:.3f}') if c == 'dur_ms' else (lambda v: f'{v:g}')
            cells.append(f'{fmt(min(vals))}/{fmt(statistics.median(vals))}/{fmt(max(vals))}')
        print(f"| {a}-{b} | {label} | {len(sel)} | {','.join(modes)} | " + ' | '.join(cells) + ' |')
    print()


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument('run')
    ap.add_argument('--loads', action='store_true')
    ap.add_argument('--pit', action='store_true')
    ap.add_argument('--window', type=int, default=0)
    ap.add_argument('--range', action='append', default=[])
    ap.add_argument('--int21-filter')
    a = ap.parse_args()
    run = pathlib.Path(a.run)
    everything = not (a.loads or a.pit or a.window or a.range)
    if a.loads or everything:
        loads(run, a.int21_filter)
    if a.pit or everything:
        pit(run)
    frames = read_tsv(run / 'frames.tsv')
    if a.range:
        ranges = []
        for spec in a.range:
            span, _, label = spec.partition(':')
            lo, _, hi = span.partition('-')
            ranges.append((int(lo), int(hi), label))
        print('## Per-frame counters by range\n')
        stats_table(frames, ranges)
    if a.window or everything:
        n = a.window or 700
        last = int(frames[-1]['frame']) if frames else 0
        print(f'## Per-frame counters, windows of {n} frames\n')
        stats_table(frames, [(s, min(s + n - 1, last), '') for s in range(1, last + 1, n)])
    return 0


if __name__ == '__main__':
    sys.exit(main())
