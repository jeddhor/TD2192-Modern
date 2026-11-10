"""Validate a release archive, write its SHA-256 and optionally smoke-test it.

Python 3.10+, standard library only. No game data is needed by default. --smoke
runs on the archive's target OS; --data additionally checks data and a headless
flight. Nothing is installed and no screenshot is requested. Logs are retained
on failure, or with --keep. Windows data checks use the current user profile.
"""
import argparse
import hashlib
import os
import pathlib
import platform
import re
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import zipfile

DOCUMENTS = {
    'LICENSE', 'README.md', 'CHANGELOG.md', 'THIRD_PARTY.md', 'PLAYER-GUIDE.md',
    'THIRD_PARTY_NOTICES.txt', 'COPYRIGHT-rust-std.html',
}
ALLOWED_LICENCES = {
    'MIT', 'Apache-2.0', 'BSD-2-Clause', 'BSD-3-Clause', 'Zlib', '0BSD', 'ISC',
    'Unlicense', 'Unicode-3.0',
}


def inspect_archive(archive, version, no_tools):
    if not re.fullmatch(r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)', version):
        raise ValueError('Version must be MAJOR.MINOR.PATCH.')
    windows = archive.name.endswith('-windows-x64.zip')
    target = 'windows-x64' if windows else 'linux-x64'
    root = f'TD2192-Modern-{version}-{target}'
    extension = '.zip' if windows else '.tar.gz'
    if archive.name != root + extension:
        raise ValueError(f'Expected archive name {root + extension}.')
    binaries = {'td2192.exe' if windows else 'td2192'}
    if not no_tools:
        binaries.add('tdtool.exe' if windows else 'tdtool')
    expected = {f'{root}/{name}' for name in DOCUMENTS | binaries}
    files, directories, contents = set(), set(), {}

    def record(name, is_dir, regular, mode, size, read):
        if '\\' in name or name.startswith('/') or any(p in ('..', '.') for p in name.split('/')):
            raise ValueError(f'Unsafe archive path: {name!r}.')
        if is_dir:
            if name.rstrip('/') != root or name.rstrip('/') in directories:
                raise ValueError(f'Unexpected/duplicate directory: {name!r}.')
            if not windows and mode != 0o755:
                raise ValueError(f'{name}: directory mode must be 755.')
            directories.add(name.rstrip('/'))
            return
        if not regular or name not in expected or name in files:
            raise ValueError(f'Unexpected, duplicate or non-regular entry: {name!r}.')
        if size <= 0:
            raise ValueError(f'Empty package entry: {name}.')
        files.add(name)
        if not windows:
            wanted = 0o755 if name.split('/')[-1] in binaries else 0o644
            if mode != wanted:
                raise ValueError(f'{name}: mode {mode:o}, expected {wanted:o}.')
        if name.endswith(('THIRD_PARTY_NOTICES.txt', 'CHANGELOG.md')):
            contents[name.split('/')[-1]] = read().decode('utf-8')

    if windows:
        with zipfile.ZipFile(archive) as package:
            for entry in package.infolist():
                mode = entry.external_attr >> 16
                if stat.S_ISLNK(mode):
                    raise ValueError(f'Symlink entry: {entry.filename!r}.')
                record(entry.filename, entry.is_dir(), stat.S_IFMT(mode) in (0, stat.S_IFREG),
                       mode & 0o777, entry.file_size, lambda e=entry: package.read(e))
    else:
        with tarfile.open(archive, 'r:gz') as package:
            for entry in package.getmembers():
                if entry.uid != 0 or entry.gid != 0:
                    raise ValueError(f'{entry.name}: owner must be 0/0.')
                record(entry.name, entry.isdir(), entry.isfile(), entry.mode,
                       entry.size, lambda e=entry: package.extractfile(e).read())
    if files != expected:
        raise ValueError(f'Missing package entries: {sorted(expected - files)}.')
    notices = contents['THIRD_PARTY_NOTICES.txt']
    triple = 'x86_64-pc-windows-msvc' if windows else 'x86_64-unknown-linux-gnu'
    if f'Target: {triple}.' not in notices:
        raise ValueError('Notices name the wrong target.')
    for component in ('sdl3-src', 'ymfm-sys', 'Rust standard library'):
        if component not in notices:
            raise ValueError(f'Notices omit {component}.')
    licences = re.findall(r'^Licence: (.+)$', notices, re.M)
    if not licences:
        raise ValueError('Notices contain no licence expressions.')
    for expression in licences:
        # Fail closed on new SPDX constructs; review before broadening this rule.
        normalized = expression.strip().replace('MIT/Apache-2.0', 'MIT OR Apache-2.0')
        choices = normalized.replace('(', '').replace(')', '').split(' OR ')
        if not all(choice.strip() in ALLOWED_LICENCES for choice in choices):
            raise ValueError(f'Licence needs review: {expression}.')
    if not re.search(rf'^## \[{re.escape(version)}\](?:\s.*)?$', contents['CHANGELOG.md'], re.M):
        raise ValueError('Packaged changelog does not contain this version.')
    print(f'Archive: {len(files)} expected files; no game data; target notices and licences OK.')
    return root, windows



def check_flight_log(flight):
    if re.search(r'panicked|\bERROR\b', flight):
        raise ValueError('Headless flight logged a panic or error.')
    samples = re.findall(
        r'rate 19\.2015 Hz: [^;]+; ([0-9.]+) ticks/s over ([0-9.]+) s', flight)
    if 'tick rate now 19.2015 Hz' not in flight or not samples:
        raise ValueError('Headless flight did not reach the expected tick rate.')
    rate, span = map(float, samples[-1])
    if span < 3 or not 17.28 <= rate <= 21.12:
        raise ValueError(f'Headless flight rate {rate:g} ticks/s over {span:g}s is outside the smoke tolerance.')
    print(f'Headless flight rate: {rate:g} ticks/s over {span:g}s (within 10% of 19.2015).')


def smoke(archive, root, windows, version, no_tools, data, seconds, keep):
    if windows != (os.name == 'nt') or platform.machine().lower() not in ('amd64', 'x86_64'):
        raise ValueError('Smoke test must run on the archive target OS and x64 architecture.')
    work = pathlib.Path(tempfile.mkdtemp(prefix='td2192-smoke-'))
    print(f'Smoke directory: {work}', flush=True)
    passed = False
    try:
        # Copy only inspected regular entries; never use an unrestricted extractall.
        if windows:
            with zipfile.ZipFile(archive) as package:
                for entry in package.infolist():
                    if not entry.is_dir():
                        dest = work / entry.filename
                        dest.parent.mkdir(parents=True, exist_ok=True)
                        dest.write_bytes(package.read(entry))
        else:
            with tarfile.open(archive, 'r:gz') as package:
                for entry in package.getmembers():
                    if entry.isfile():
                        dest = work / entry.name
                        dest.parent.mkdir(parents=True, exist_ok=True)
                        dest.write_bytes(package.extractfile(entry).read())
                        dest.chmod(entry.mode)
        env = os.environ.copy()
        for name in ('TD2192_DATA', 'TD2192_ORACLE', 'RUST_LOG', 'RUST_LOG_STYLE'):
            env.pop(name, None)
        # SDL's Windows get_pref_path uses the roaming known folder. APPDATA alone
        # cannot redirect it reliably. --version/--help precede settings loading.
        env.update(XDG_DATA_HOME=str(work / 'xdg'), SDL_VIDEODRIVER='dummy',
                   SDL_AUDIODRIVER='dummy', NO_COLOR='1')
        bindir = work / root
        suffix = '.exe' if windows else ''
        game = bindir / f'td2192{suffix}'

        def run(name, command):
            result = subprocess.run(command, cwd=bindir, env=env, capture_output=True,
                                    text=True, encoding='utf-8', errors='replace', timeout=60)
            output = result.stdout + result.stderr
            (work / f'{name}.log').write_text(output, encoding='utf-8')
            if result.returncode:
                raise ValueError(f'{name} exited {result.returncode}; see {work / (name + ".log")}.')
            return output

        for name in ('td2192',) if no_tools else ('td2192', 'tdtool'):
            binary = str(bindir / (name + suffix))
            if run(f'{name}-version', [binary, '--version']).strip() != f'{name} {version}':
                raise ValueError(f'{name} --version disagrees with {version}.')
        help_text = run('help', [str(game), '--help'])
        for option in ('--data', '--episode', '--check', '--fullscreen', '--no-intro', '--version', '--help'):
            if option not in help_text:
                raise ValueError(f'--help omits {option}.')
        if not windows:
            libraries = run('ldd', ['ldd', str(game)])
            allowed = ('linux-vdso.so.', 'libc.so.', 'libm.so.', 'libstdc++.so.', 'libgcc_s.so.', 'ld-linux-x86-64.so.')
            for line in libraries.splitlines():
                soname = line.strip().split()[0].split('/')[-1]
                if 'not found' in line or not soname.startswith(allowed):
                    raise ValueError(f'Unexpected/missing Linux library: {line.strip()}.')
        if data:
            if windows:
                print('Windows data checks use the current user profile; use Sandbox for a fresh profile.')
            check = run('data-check', [str(game), '--check', '--data', str(data)])
            if '218 of 218 files match' not in check:
                raise ValueError('Data check did not match all 218 files.')
            for episode in (1, 2, 3):
                if not re.search(rf'Episode {episode}:\s+playable', check):
                    raise ValueError(f'Episode {episode} is not playable.')
            with (work / 'flight.log').open('w', encoding='utf-8') as log:
                process = subprocess.Popen([str(game), '--data', str(data), '--dev-flight', '1', '--dev-stats'],
                                           cwd=bindir, env=env, stdout=log, stderr=log)
                try:
                    process.wait(timeout=seconds)
                    raise ValueError(f'Headless flight exited early ({process.returncode}).')
                except subprocess.TimeoutExpired:
                    process.terminate()
                    try:
                        process.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=10)
            flight = (work / 'flight.log').read_text(encoding='utf-8', errors='replace')
            check_flight_log(flight)
            print(f'Data and {seconds:g}-second headless flight: PASS (review flight.log for timing warnings).')
        else:
            print('Data/flight checks: NOT RUN (supply --data for these checks).')
        print('Binary versions and help: PASS.' + (' Linux shared libraries: PASS.' if not windows else ''))
        passed = True
    finally:
        if passed and not keep:
            shutil.rmtree(work)
        else:
            print(f'Retained smoke logs/settings: {work}')


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--archive', type=pathlib.Path, required=True)
    ap.add_argument('--version', required=True)
    ap.add_argument('--no-tools', action='store_true')
    ap.add_argument('--sha256-out', type=pathlib.Path, help='write a sha256sum-compatible sidecar')
    ap.add_argument('--smoke', action='store_true')
    ap.add_argument('--data', type=pathlib.Path, help='read-only game data folder or original ZIP; requires --smoke')
    ap.add_argument('--seconds', type=float, default=20, help='headless flight duration, at least 10 seconds')
    ap.add_argument('--keep', action='store_true', help='retain successful smoke logs/settings too')
    args = ap.parse_args()
    if args.data and not args.smoke:
        ap.error('--data requires --smoke')
    if not 10 <= args.seconds <= 300:
        ap.error('--seconds must be between 10 and 300')
    archive = args.archive.resolve(strict=True)
    data = args.data.resolve(strict=True) if args.data else None
    root, windows = inspect_archive(archive, args.version, args.no_tools)
    checksum = f'{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n'
    print(checksum, end='')
    if args.sha256_out:
        if args.sha256_out.resolve() == archive:
            raise ValueError('Checksum output must not overwrite the archive.')
        args.sha256_out.write_bytes(checksum.encode('ascii'))
    if args.smoke:
        smoke(archive, root, windows, args.version, args.no_tools, data, args.seconds, args.keep)


if __name__ == '__main__':
    try:
        main()
    except (ValueError, OSError, subprocess.SubprocessError, tarfile.TarError, zipfile.BadZipFile) as error:
        print(f'ERROR: {error}', file=sys.stderr)
        sys.exit(1)
