"""Generate THIRD_PARTY_NOTICES.txt for a release build of td2192 (M13-03, M13-05).

What is linked into the `td2192` binary for the target platform:

- every third-party crate on its runtime (normal) dependency graph, from `cargo metadata`, as
  far as `cargo tree` says the crate is really built (`cargo metadata` also lists weak optional
  dependencies that no feature turns on, such as SDL_image; `cargo tree` does not) and leaving
  out procedural macros, which run in the compiler and are not linked;
- the `*-src` build dependencies, whose bundled C sources are compiled into the binary (SDL 3
  via `sdl3-src`); ymfm comes with `ymfm-sys` the same way;
- the Rust standard library.

For each crate the file gives its name, version, licence expression and the text of every
licence or notice file in its package. Bundled C sources carry the licence files of the projects
they include (for SDL 3 also hidapi and yuv2rgb at any depth, the Sun notice of its libm and, on
Linux, the copyright notices of the Wayland protocols it generates code from). The standard
library's own notices (`COPYRIGHT-library.html` of the Rust toolchain) are copied next to the
output with `--std-copyright-out`.

The script exits with status 1 if a crate has no licence file.

Usage: python tools/scripts/gen-notices.py --out <file> [--target x86_64-pc-windows-msvc]
           [--std-copyright-out <file>]
The outputs are build artefacts; they are not committed.
"""
import argparse
import json
import pathlib
import re
import shutil
import subprocess
import sys

LICENCE_NAMES = ('license', 'licence', 'copying', 'notice', 'unlicense', 'authors')


def run(args):
    return subprocess.run(args, check=True, capture_output=True, text=True).stdout


def built_packages(target: str):
    """(name, version) of every package `cargo tree` builds for td2192 on `target`."""
    out = run(['cargo', 'tree', '-p', 'td2192', '-e', 'normal,build,no-proc-macro',
               '--target', target, '--prefix', 'none', '--format', '{p}'])
    built = set()
    for line in out.splitlines():
        parts = line.split()
        if len(parts) >= 2 and parts[1].startswith('v'):
            built.add((parts[0], parts[1][1:]))
    return built


def is_proc_macro(pkg) -> bool:
    return any('proc-macro' in t['kind'] for t in pkg['targets'])


def runtime_packages(target: str):
    meta = json.loads(run(['cargo', 'metadata', '--format-version', '1',
                           '--filter-platform', target]))
    built = built_packages(target)
    packages = {p['id']: p for p in meta['packages']}
    nodes = {n['id']: n for n in meta['resolve']['nodes']}
    root = next(i for i, p in packages.items() if p['name'] == 'td2192' and p['source'] is None)

    def wanted(i):
        p = packages[i]
        return (p['name'], p['version']) in built and not is_proc_macro(p)

    seen, stack = set(), [root]
    while stack:
        i = stack.pop()
        if i in seen:
            continue
        seen.add(i)
        for dep in nodes[i]['deps']:
            kinds = {k['kind'] for k in dep['dep_kinds']}
            if not wanted(dep['pkg']):
                continue
            if None in kinds:
                stack.append(dep['pkg'])
            elif 'build' in kinds and packages[dep['pkg']]['name'].endswith('-src'):
                # Bundled C/C++ sources compiled into the binary (SDL 3 via sdl3-src).
                seen.add(dep['pkg'])
    third = [packages[i] for i in seen if packages[i]['source'] is not None]
    return sorted(third, key=lambda p: (p['name'], p['version']))


def licence_files(pkg):
    root = pathlib.Path(pkg['manifest_path']).parent
    bundled = pkg['name'].endswith('-src')
    files = []
    for path in sorted(root.rglob('*')):
        rel = path.relative_to(root)
        if not path.is_file() or 'test' in str(rel).lower():
            continue
        # Crates: top level, or two directories down (bundled C sources keep theirs there).
        # Bundled C sources (`*-src`): any depth, for the projects they include in turn.
        if len(rel.parts) > 3 and not bundled:
            continue
        name = path.name.lower()
        # hidapi (in SDL 3) offers GPL-3.0, BSD or its original licence; SDL uses it under
        # the BSD or original terms, so the GPL alternative is not one that applies here.
        if bundled and 'gpl' in name:
            continue
        if name.startswith(LICENCE_NAMES) and path.stat().st_size < 200_000:
            files.append(path)
    return root, files


def sdl_extra_notices(root: pathlib.Path, target: str):
    """Notices inside SDL 3's sources that are not separate licence files."""
    out = []
    sdl = root / 'SDL'
    # libm (from uClibc/FreeBSD, originally Sun): the same notice in every file.
    blocks = []
    for f in sorted((sdl / 'src' / 'libm').glob('*.c')):
        text = f.read_text(encoding='utf-8', errors='replace')
        m = re.search(r'/\*\s*\n\s*\*\s*=+\s*\n(.*?)\n\s*\*\s*=+\s*\n', text, re.S)
        if m and 'Sun Microsystems' in m.group(1):
            block = '\n'.join(line.strip().lstrip('*').strip() for line in m.group(1).splitlines())
            if block not in blocks:
                blocks.append(block)
    for b in blocks:
        out += ['--- SDL/src/libm (notice in each file) ---', b, '']
    # Wayland protocols: Linux builds generate code from these XML files.
    if 'linux' in target:
        for f in sorted((sdl / 'wayland-protocols').glob('*.xml')):
            text = f.read_text(encoding='utf-8', errors='replace')
            for m in re.finditer(r'<copyright>(.*?)</copyright>', text, re.S):
                lines = [line.strip() for line in m.group(1).strip().splitlines()]
                out += [f'--- SDL/wayland-protocols/{f.name} ---', '\n'.join(lines), '']
    return out


def rust_std_entry(std_copyright_out):
    sysroot = pathlib.Path(run(['rustc', '--print', 'sysroot']).strip())
    version = run(['rustc', '--version']).strip()
    doc = sysroot / 'share' / 'doc' / 'rust'
    out = ['=' * 78, f'Rust standard library ({version})', 'Licence: MIT OR Apache-2.0',
           'Source: https://github.com/rust-lang/rust', '']
    found = False
    for name in ('MIT.txt', 'Apache-2.0.txt'):
        f = doc / 'licenses' / name
        if f.is_file():
            found = True
            out += [f'--- {name} ---', f.read_text(encoding='utf-8', errors='replace').rstrip(),
                    '']
    copyright_html = doc / 'COPYRIGHT-library.html'
    if std_copyright_out and copyright_html.is_file():
        shutil.copyfile(copyright_html, std_copyright_out)
        out += [f'The copyright notices of the standard library and of the crates it is built '
                f'from are in {pathlib.Path(std_copyright_out).name}, next to this file.', '']
    elif std_copyright_out:
        found = False
    return out, found


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument('--out', required=True)
    ap.add_argument('--target', default='x86_64-pc-windows-msvc')
    ap.add_argument('--std-copyright-out', default=None,
                    help="where to copy the Rust toolchain's COPYRIGHT-library.html")
    args = ap.parse_args()
    out = [
        'TD2192-Modern: third-party notices',
        '',
        'This program is distributed under the MIT licence (see LICENSE). It includes the',
        'following third-party software, each under its own licence, reproduced below.',
        f'Target: {args.target}.',
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
        if pkg['name'] == 'sdl3-src':
            out += sdl_extra_notices(root, args.target)
    std, std_found = rust_std_entry(args.std_copyright_out)
    out += std
    if not std_found:
        missing.append('Rust standard library')
    pathlib.Path(args.out).write_text('\n'.join(out) + '\n', encoding='utf-8', newline='\n')
    print(f'wrote {args.out}')
    if missing:
        print('ERROR: no licence file found for:', ', '.join(missing), file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
