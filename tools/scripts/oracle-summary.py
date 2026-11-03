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
                s['bytes'] += int(r['result'][6:].split(',')[0])
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
    print('| frame | ms | prog | caller (rel) | port | value | decoded |')
    print('|---|---|---|---|---|---|---|')
    for r in rows:
        if 'count=' in r['decoded'] or 'control' in r['decoded']:
            print(f"| {r['frame']} | {float(r['ms']):.1f} | {r.get('prog', '')} | {r.get('rel', '')} | {r['port']} | "
                  f"{r['val']} | {r['decoded']} |")
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


def cadence_table(frames, ranges) -> None:
    """Means and retrace-wait cadence per range (timing questions, U-001)."""
    print('| range | label | frames | IRQ0/frame (mean) | IRQ0 rate Hz | frames with 3DAh reads | frames with a retrace hit '
          '| gap between retrace-hit frames (mean / most common) | 3DAh reads per polling frame (mean) '
          '| DOS clock calls/frame (mean) | 3C9h writes/frame (mean) | frames with >=768 3C9h writes |')
    print('|---|---|---|---|---|---|---|---|---|---|---|---|')
    for a, b, label in ranges:
        sel = [f for f in frames if a <= int(f['frame']) <= b]
        if not sel:
            continue
        n = len(sel)
        dur = sum(float(f['dur_ms']) for f in sel)
        irq = sum(int(f['irq0']) for f in sel)
        polling = [f for f in sel if int(f['p3da']) > 0]
        hits = [int(f['frame']) for f in sel if int(f['p3da_vr']) > 0]
        gaps = [y - x for x, y in zip(hits, hits[1:])]
        gap_mean = f'{statistics.mean(gaps):.3f}' if gaps else '-'
        gap_mode = statistics.mode(gaps) if gaps else '-'
        reads = f"{statistics.mean(int(f['p3da']) for f in polling):.0f}" if polling else '-'
        clock = f"{statistics.mean(int(f['int21_clock']) for f in sel):.1f}" if 'int21_clock' in sel[0] else 'n/a'
        dac = statistics.mean(int(f['dac_3c9']) for f in sel)
        full = sum(1 for f in sel if int(f['dac_3c9']) >= 768)
        print(f'| {a}-{b} | {label} | {n} | {irq / n:.3f} | {irq * 1000 / dur:.2f} | {len(polling)} | {len(hits)} '
              f'| {gap_mean} / {gap_mode} | {reads} | {clock} | {dac:.1f} | {full} |')
    print()


def fb_cadence(run: pathlib.Path, tag: str) -> None:
    """For consecutive-frame framebuffer dumps with a tag: how often the displayed image changes,
    and where the changes fall relative to frames that saw the start of vertical retrace."""
    import collections
    frames = {int(r['frame']): r for r in read_tsv(run / 'frames.tsv')}
    fb = {}
    # FB rows carry more tab-separated fields than the header names, so split raw lines
    with (run / 'events.tsv').open(encoding='utf-8') as f:
        next(f)
        for line in f:
            p = line.rstrip('\n').split('\t')
            if p[2] == 'FB' and p[3].endswith(f'_{tag}.vga'):
                fb[int(p[0])] = next(x for x in p if x.startswith('fnv_pixels='))
    fs = sorted(fb)
    if len(fs) < 2:
        print(f'no consecutive dumps tagged {tag}')
        return
    changes = [b for a, b in zip(fs, fs[1:]) if fb[a] != fb[b] and b == a + 1]
    gaps = [b - a for a, b in zip(changes, changes[1:])]
    hits = [f for f in fs if f in frames and int(frames[f]['p3da_vr']) > 0]
    rel = collections.Counter()
    for c in changes:
        prev = [h for h in hits if h <= c]
        rel[(c - prev[-1]) if prev else 'none'] += 1
    print(f'## Image-change cadence for dumps tagged `{tag}`\n')
    print(f'- dumped frames: {len(fs)} ({fs[0]}..{fs[-1]})')
    print(f'- frames whose image differs from the previous frame: {len(changes)}')
    if gaps:
        print(f'- frames between image changes: mean {statistics.mean(gaps):.3f}, '
              f'distribution {dict(sorted(collections.Counter(gaps).items()))}')
    print(f'- frames with a retrace hit (3DAh bit 3 seen): {len(hits)}')
    print(f'- image-change frame minus latest retrace-hit frame: {dict(sorted(rel.items(), key=str))}')
    print()


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument('run')
    ap.add_argument('--loads', action='store_true')
    ap.add_argument('--pit', action='store_true')
    ap.add_argument('--window', type=int, default=0)
    ap.add_argument('--range', action='append', default=[])
    ap.add_argument('--int21-filter')
    ap.add_argument('--cadence', action='store_true', help='with --range: means and retrace-wait cadence')
    ap.add_argument('--fb-cadence', metavar='TAG', help='image-change cadence of consecutive fb dumps with this tag')
    a = ap.parse_args()
    run = pathlib.Path(a.run)
    if a.fb_cadence:
        fb_cadence(run, a.fb_cadence)
        return 0
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
        if a.cadence:
            print('## Cadence by range\n')
            cadence_table(frames, ranges)
        else:
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
