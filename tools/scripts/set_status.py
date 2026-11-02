"""Update one task row in docs/STATUS.md.

Usage: python tools/scripts/set_status.py <TASK-ID> <STATUS> [--owner NAME] [--notes TEXT]

Only the Status, Owner and Notes cells of the matching row change. The script exits non-zero if
the task ID is not found exactly once.
"""
import argparse
import pathlib
import sys

STATUS_FILE = pathlib.Path(__file__).resolve().parents[2] / 'docs' / 'STATUS.md'
VALID = {'TODO', 'IN PROGRESS', 'PAUSED', 'BLOCKED', 'REVIEW', 'DONE'}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument('task')
    ap.add_argument('status')
    ap.add_argument('--owner', default=None)
    ap.add_argument('--notes', default=None)
    a = ap.parse_args()
    if a.status not in VALID:
        print(f'status must be one of {sorted(VALID)}', file=sys.stderr)
        return 2
    lines = STATUS_FILE.read_text(encoding='utf-8').split('\n')
    hits = [i for i, l in enumerate(lines) if l.startswith(f'| {a.task} |')]
    if len(hits) != 1:
        print(f'{a.task}: found {len(hits)} rows', file=sys.stderr)
        return 1
    cells = [c.strip() for c in lines[hits[0]].strip().strip('|').split('|')]
    # cells: [id, task, hat, depends, status, owner, notes]
    if len(cells) != 7:
        print(f'{a.task}: expected 7 cells, found {len(cells)}', file=sys.stderr)
        return 1
    cells[4] = a.status
    if a.owner is not None:
        cells[5] = a.owner
    if a.notes is not None:
        cells[6] = a.notes.replace('|', '/')
    lines[hits[0]] = '| ' + ' | '.join(cells) + ' |'
    STATUS_FILE.write_text('\n'.join(lines), encoding='utf-8', newline='\n')
    print(lines[hits[0]])
    return 0


if __name__ == '__main__':
    sys.exit(main())
