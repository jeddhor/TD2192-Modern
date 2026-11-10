"""Synthetic release-tool checks. No game data, tags, remotes or builds."""
import contextlib
import importlib.util
import io
import json
import pathlib
import stat
import sys
import tarfile
import tempfile
import unittest
import warnings
import zipfile
from unittest import mock


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, pathlib.Path(__file__).with_name(filename))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


package = load('check_package', 'check-package.py')
tag = load('check_release_tag', 'check-release-tag.py')


class PackageChecks(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.folder = pathlib.Path(self.temp.name)

    def make(self, windows=True, change=None, no_tools=False):
        target = 'windows-x64' if windows else 'linux-x64'
        root = f'TD2192-Modern-0.9.0-{target}'
        extension = '.zip' if windows else '.tar.gz'
        archive = self.folder / (root + extension)
        entries = {f'{root}/{name}': (b'document\n', 0o644) for name in package.DOCUMENTS}
        notices = ('Target: ' + ('x86_64-pc-windows-msvc' if windows else 'x86_64-unknown-linux-gnu') +
                   '.\nsdl3-src\nymfm-sys\nRust standard library\nLicence: MIT OR Apache-2.0\n')
        entries[f'{root}/THIRD_PARTY_NOTICES.txt'] = (notices.encode(), 0o644)
        entries[f'{root}/CHANGELOG.md'] = (b'# Changelog\n## [0.9.0] - 2026-11-07\n', 0o644)
        for name in ('td2192',) if no_tools else ('td2192', 'tdtool'):
            entries[f'{root}/{name}' + ('.exe' if windows else '')] = (b'binary', 0o755)
        if change:
            change(entries, root)
        if windows:
            with zipfile.ZipFile(archive, 'w') as out:
                for name, (content, mode) in entries.items():
                    entry = zipfile.ZipInfo(name)
                    entry.external_attr = (stat.S_IFREG | mode) << 16
                    out.writestr(entry, content)
        else:
            with tarfile.open(archive, 'w:gz') as out:
                for name, (content, mode) in entries.items():
                    entry = tarfile.TarInfo(name)
                    entry.mode, entry.size = mode, len(content)
                    out.addfile(entry, io.BytesIO(content))
        return archive

    def inspect(self, archive, no_tools=False):
        with contextlib.redirect_stdout(io.StringIO()):
            return package.inspect_archive(archive, '0.9.0', no_tools)

    def test_valid_targets_and_without_tools(self):
        for windows in (False, True):
            with self.subTest(windows=windows):
                self.assertEqual(self.inspect(self.make(windows))[1], windows)
                self.inspect(self.make(windows, no_tools=True), no_tools=True)

    def test_unexpected_game_file_rejected(self):
        archive = self.make(change=lambda e, r: e.update({f'{r}/game.scr': (b'fake', 0o644)}))
        with self.assertRaisesRegex(ValueError, 'Unexpected'):
            self.inspect(archive)

    def test_traversal_and_symlink_rejected(self):
        archive = self.make(change=lambda e, r: e.update({f'{r}/../escape': (b'fake', 0o644)}))
        with self.assertRaisesRegex(ValueError, 'Unsafe'):
            self.inspect(archive)
        archive = self.make()
        with zipfile.ZipFile(archive, 'a') as out:
            entry = zipfile.ZipInfo('link')
            entry.external_attr = (stat.S_IFLNK | 0o777) << 16
            out.writestr(entry, 'destination')
        with self.assertRaises(ValueError):
            self.inspect(archive)

    def test_missing_file_rejected(self):
        archive = self.make(change=lambda e, r: e.pop(f'{r}/LICENSE'))
        with self.assertRaisesRegex(ValueError, 'Missing'):
            self.inspect(archive)

    def test_nonpermissive_expression_rejected(self):
        def change(entries, root):
            entries[f'{root}/THIRD_PARTY_NOTICES.txt'] = (
                b'Target: x86_64-pc-windows-msvc.\nsdl3-src\nymfm-sys\nRust standard library\nLicence: GPL-3.0\n', 0o644)
        with self.assertRaisesRegex(ValueError, 'Licence needs review'):
            self.inspect(self.make(change=change))

    def test_wrong_linux_permissions_and_owner_rejected(self):
        archive = self.make(False, change=lambda e, r: e.update({f'{r}/td2192': (b'binary', 0o777)}))
        with self.assertRaisesRegex(ValueError, 'mode'):
            self.inspect(archive)
        archive = self.make(False)
        with tarfile.open(archive, 'w:gz') as out:
            entry = tarfile.TarInfo('unexpected')
            entry.uid = 1000
            out.addfile(entry)
        with self.assertRaisesRegex(ValueError, 'owner'):
            self.inspect(archive)

    def test_duplicate_entry_rejected(self):
        archive = self.make()
        with zipfile.ZipFile(archive, 'a') as out, warnings.catch_warnings():
            warnings.simplefilter('ignore', UserWarning)
            out.writestr('TD2192-Modern-0.9.0-windows-x64/LICENSE', 'duplicate')
        with self.assertRaisesRegex(ValueError, 'duplicate'):
            self.inspect(archive)

    def test_wrong_version_rejected(self):
        with self.assertRaisesRegex(ValueError, 'Expected archive name'):
            package.inspect_archive(self.make(), '0.9.1', False)



class FlightChecks(unittest.TestCase):
    def test_sustained_rate_and_failures(self):
        prefix = 'tick rate now 19.2015 Hz at tick 37\n'
        good = prefix + 'rate 19.2015 Hz: 19.00 ticks/s this second; 19.19 ticks/s over 8.0 s at this rate;\n'
        with contextlib.redirect_stdout(io.StringIO()):
            package.check_flight_log(good)
        for log in (good + 'ERROR crash', good + 'panicked', prefix,
                    good.replace('19.19 ticks/s', '10.00 ticks/s'),
                    good.replace('8.0 s', '1.0 s')):
            with self.subTest(log=log), self.assertRaises(ValueError):
                package.check_flight_log(log)


class TagChecks(unittest.TestCase):
    def check(self, value='v0.9.0', head='abc', tagged='abc', version='0.9.0', changelog=True):
        with tempfile.TemporaryDirectory() as folder:
            root = pathlib.Path(folder)
            (root / 'CHANGELOG.md').write_text('## [0.9.0]\n' if changelog else '# Empty\n')
            def command(args, **kwargs):
                if args[0] == 'cargo':
                    return json.dumps({'packages': [{'name': 'td2192', 'version': version}]})
                return head if args[-1] == 'HEAD' else tagged
            with mock.patch.object(tag, '__file__', str(root / 'tools/scripts/check-release-tag.py')), \
                    mock.patch.object(tag.subprocess, 'check_output', side_effect=command), \
                    mock.patch.object(sys, 'argv', ['check-release-tag.py', '--tag', value]), \
                    contextlib.redirect_stdout(io.StringIO()):
                tag.main()

    def test_matching_tag(self):
        self.check()

    def test_invalid_tag_mismatch_commit_version_notes(self):
        for kwargs, message in [
            ({'value': 'v00.9.0'}, 'Release tag'),
            ({'tagged': 'other'}, 'checked-out commit'),
            ({'version': '0.9.1'}, 'workspace version'),
            ({'changelog': False}, 'no section'),
        ]:
            with self.subTest(kwargs=kwargs), self.assertRaisesRegex(ValueError, message):
                self.check(**kwargs)


if __name__ == '__main__':
    unittest.main()
