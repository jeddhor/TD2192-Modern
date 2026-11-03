"""Generate THIRD_PARTY_NOTICES.txt for a release build of td2192 (M13-03).

Walks the runtime (normal) dependency graph of the `td2192` binary for the target platform with
`cargo metadata` (plus the `*-src` build dependencies, whose bundled C sources are compiled into the
binary), and writes, for every third-party crate, its name, version, licence expression
and the text of every licence/notice file in its package. Crates that bundle C/C++ sources (SDL 3
via `sdl3-src`, ymfm via `ymfm-sys`) carry those projects' licence files in their packages, so
their texts are included the same way.

Usage: python tools/scripts/gen-notices.py --out <file> [--target x86_64-pc-windows-msvc]
The output is a build artefact; it is not committed.
"""
import argparse
import json
import pathlib
import subprocess
import sys

LICENCE_NAMES = ('license', 'licence', 'copying', 'notice', 'unlicense', 'authors')


def runtime_packages(target: str):
    meta = json.loads(subprocess.run(
        ['cargo', 'metadata', '--format-version', '1', '--filter-platform', target],
        check=True, capture_output=True, text=True).stdout)
    packages = {p['id']: p for p in meta['packages']}
    nodes = {n['id']: n for n in meta['resolve']['nodes']}
    root = next(i for i, p in packages.items() if p['name'] == 'td2192' and p['source'] is None)
    seen, stack = set(), [root]
    while stack:
        i = stack.pop()
        if i in seen:
            continue
        seen.add(i)
        for dep in nodes[i]['deps']:
            kinds = {k['kind'] for k in dep['dep_kinds']}
            if None in kinds:
                stack.append(dep['pkg'])
            elif 'build' in kinds and packages[dep['pkg']]['name'].endswith('-src'):
                # Bundled C/C++ sources compiled into the binary (SDL 3 via sdl3-src).
                seen.add(dep['pkg'])
    third = [packages[i] for i in seen if packages[i]['source'] is not None]
    return sorted(third, key=lambda p: (p['name'], p['version']))


def licence_files(pkg):
    root = pathlib.Path(pkg['manifest_path']).parent
    files = []
    for path in sorted(root.rglob('*')):
        rel = path.relative_to(root)
        # Top level, or one directory down (bundled C sources keep theirs there), not tests.
        if len(rel.parts) > 3 or not path.is_file() or 'test' in str(rel).lower():
            continue
        if path.name.lower().startswith(LICENCE_NAMES) and path.stat().st_size < 200_000:
            files.append(path)
    return root, files


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument('--out', required=True)
    ap.add_argument('--target', default='x86_64-pc-windows-msvc')
    args = ap.parse_args()
    out = [
        'TD2192-Modern: third-party notices',
        '',
        'This program is distributed under the MIT licence (see LICENSE). It includes the',
        'following third-party software, each under its own licence, reproduced below.',
        '',
    ]
    missing = []
    for pkg in runtime_packages(args.target):
        root, files = licence_files(pkg)
        out += ['=' * 78, f"{pkg['name']} {pkg['version']}",
                f"Licence: {pkg.get('license') or pkg.get('license_file') or 'see below'}"]
        if pkg.get('repository'):
            out.append(f"Source: {pkg['repository']}")
        out.append('')
        if not files:
            missing.append(pkg['name'])
        for f in files:
            out += [f'--- {f.relative_to(root).as_posix()} ---',
                    f.read_text(encoding='utf-8', errors='replace').rstrip(), '']
    pathlib.Path(args.out).write_text('\n'.join(out) + '\n', encoding='utf-8', newline='\n')
    print(f'wrote {args.out}')
    if missing:
        print('WARNING: no licence file found in:', ', '.join(missing), file=sys.stderr)
    return 0


if __name__ == '__main__':
    sys.exit(main())
