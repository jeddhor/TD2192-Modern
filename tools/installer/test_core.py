"""Installer contract tests using entirely synthetic assets and isolated shortcuts.

Run: python -m unittest discover -s tools/installer -p test_core.py -v
No retail data, GUI dependency, registry writes or application launches are used.
"""
from __future__ import annotations

from contextlib import ExitStack
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock


def load_core():
    path = Path(os.environ.get("TD2192_INSTALLER_CORE", Path(__file__).with_name("core.py")))
    spec = importlib.util.spec_from_file_location("installer_core_under_test", path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


core = load_core()


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


class InstallerTests(unittest.TestCase):
    def setUp(self):
        self.stack = ExitStack()
        self.addCleanup(self.stack.close)
        self.root = Path(self.stack.enter_context(tempfile.TemporaryDirectory(prefix="td2192-installer-test-")))
        self.source = self.root / "Retail data"
        self.source.mkdir()
        self.bundle = self.root / "bundle"
        self.payload = self.bundle / "payload"
        self.payload.mkdir(parents=True)
        # Both names let the same synthetic fixture test both platform branches.
        (self.payload / "td2192.exe").write_bytes(b"synthetic Windows application")
        (self.payload / "td2192").write_bytes(b"synthetic Linux application")
        (self.payload / "LICENSE").write_bytes(b"synthetic licence")
        self.assets = {"td.pal": b"synthetic palette", "m1.td": b"synthetic mission"}
        self.entries = [
            {"name": name, "size": len(data), "sha256": digest(data), "required": True}
            for name, data in self.assets.items()
        ]
        self.entries.append({"name": "m21.td3", "size": 8, "sha256": digest(b"optional"), "required": False})
        for name, data in self.assets.items():
            (self.source / name.upper()).write_bytes(data)
        self.write_manifest()
        self.target = self.root / "Installed game"
        self.shortcut_root = self.root / "shortcuts"
        self.shortcuts = [
            (self.shortcut_root / "applications" / "td2192-modern.desktop", False),
            (self.shortcut_root / "applications" / "td2192-modern-uninstall.desktop", True),
            (self.shortcut_root / "Desktop" / "td2192-modern.desktop", False),
        ]
        self.stack.enter_context(mock.patch.object(core, "resource_root", return_value=self.bundle))
        self.registry = self.stack.enter_context(mock.patch.object(core, "_registry"))
        self.stack.enter_context(mock.patch.object(core, "_shortcut_locations", side_effect=self.shortcut_locations))
        self.stack.enter_context(mock.patch.dict(os.environ, {
            "HOME": str(self.root / "home"),
            "USERPROFILE": str(self.root / "home"),
            "APPDATA": str(self.root / "appdata"),
            "LOCALAPPDATA": str(self.root / "localappdata"),
            "XDG_DATA_HOME": str(self.root / "xdg-data"),
            "XDG_CONFIG_HOME": str(self.root / "xdg-config"),
        }))

    def shortcut_locations(self, start_menu, desktop):
        return (self.shortcuts[:2] if start_menu else []) + (self.shortcuts[2:] if desktop else [])

    def write_manifest(self):
        (self.bundle / "asset-manifest.json").write_text(
            json.dumps({"version": "0.9.1-test", "files": self.entries}), encoding="utf-8"
        )

    def install(self, **kwargs):
        return core.install(self.source, self.target, start_menu=False, desktop=False, **kwargs)

    def record(self):
        return json.loads((self.target / "installed.json").read_text(encoding="utf-8"))

    def write_record(self, record):
        (self.target / "installed.json").write_text(json.dumps(record), encoding="utf-8")

    def link(self, path, destination, *, directory=False):
        try:
            path.symlink_to(destination, target_is_directory=directory)
        except (OSError, NotImplementedError) as error:
            self.skipTest(f"Symbolic link creation unavailable: {error}")

    def test_validate_maps_case_insensitive_names_and_skips_absent_optional(self):
        found = core.validate_source(self.source)
        self.assertEqual({entry["name"] for entry, _ in found}, set(self.assets))
        self.assertEqual({path.name for _, path in found}, {name.upper() for name in self.assets})

    def test_validate_accepts_legacy_long_basename_alias(self):
        data = b"synthetic legacy basename"
        (self.source / "HELIHANGAR.SCR").write_bytes(data)
        self.entries.append({"name": "helihang.scr", "size": len(data), "sha256": digest(data), "required": True})
        self.write_manifest()
        found = dict((entry["name"], path.name) for entry, path in core.validate_source(self.source))
        self.assertEqual(found["helihang.scr"], "HELIHANGAR.SCR")

    def test_ambiguous_dos_aliases_fail(self):
        (self.source / "helicoptr.scr").write_bytes(b"first")
        (self.source / "helicopter.scr").write_bytes(b"second")
        with self.assertRaises(ValueError):
            core.validate_source(self.source)

    def test_missing_required_asset_fails_before_creating_target_parent(self):
        (self.source / "TD.PAL").unlink()
        target = self.root / "not yet created" / "game"
        with self.assertRaises(ValueError):
            core.install(self.source, target, start_menu=False)
        self.assertFalse(target.parent.exists())

    def test_empty_required_asset_fails(self):
        (self.source / "TD.PAL").write_bytes(b"")
        with self.assertRaises(ValueError):
            core.validate_source(self.source)

    def test_nonempty_modified_retail_assets_are_copied_and_reported(self):
        changed = b"nonempty custom user palette with a different size"
        (self.source / "TD.PAL").write_bytes(changed)
        result = self.install()
        self.assertEqual(result["modified_assets"], ["td.pal"])
        self.assertEqual(self.record()["modified_assets"], ["td.pal"])
        self.assertEqual((self.target / "game" / "td.pal").read_bytes(), changed)

    def test_optional_asset_is_copied_when_present(self):
        (self.source / "M21.TD3").write_bytes(b"optional")
        self.install()
        self.assertEqual((self.target / "game" / "m21.td3").read_bytes(), b"optional")

    def add_exact_executable_data(self):
        data = b"MZ synthetic executable resources, not runnable"
        path = self.source / "FILE0001.EXE"
        path.write_bytes(data)
        self.entries.append({"name": "file0001.exe", "size": len(data), "sha256": digest(data), "required": True, "exact": True})
        self.write_manifest()
        return path, data

    def test_allowlisted_executable_resources_are_copied_as_data_never_executed(self):
        _, data = self.add_exact_executable_data()
        with mock.patch.object(core.subprocess, "Popen") as launch, mock.patch.object(core.subprocess, "run") as run:
            self.install()
        self.assertEqual((self.target / "game" / "file0001.exe").read_bytes(), data)
        launch.assert_not_called()
        run.assert_not_called()

    def test_exact_executable_hash_mismatch_fails_before_creating_target(self):
        path, _ = self.add_exact_executable_data()
        path.write_bytes(b"different unsupported executable version")
        with self.assertRaises(ValueError):
            self.install()
        self.assertFalse(self.target.exists())
        self.assertFalse(list(self.root.glob(".td2192-install-*")))

    def test_exact_executable_changed_during_copy_rolls_back_staging(self):
        exact_source, _ = self.add_exact_executable_data()
        original_copy = core.shutil.copyfile

        def concurrent_change(source, destination, *args, **kwargs):
            if Path(source) == exact_source:
                Path(destination).write_bytes(b"changed while copying")
                return destination
            return original_copy(source, destination, *args, **kwargs)

        with mock.patch.object(core.shutil, "copyfile", side_effect=concurrent_change):
            with self.assertRaises(ValueError):
                self.install()
        self.assertFalse(self.target.exists())
        self.assertFalse(list(self.root.glob(".td2192-install-*")))

    def test_only_allowlisted_data_is_copied_and_source_is_unchanged(self):
        excluded = ["td.cfg", "tdstatus.dat", "td.sav", "td1.exe", "file0001.exe", "helpme.doc", "order.tfp"]
        for name in excluded:
            (self.source / name).write_bytes(b"synthetic excluded user file")
        before = {p.name: p.read_bytes() for p in self.source.iterdir()}
        self.install()
        self.assertEqual({p.name for p in (self.target / "game").iterdir()}, set(self.assets))
        self.assertEqual(before, {p.name: p.read_bytes() for p in self.source.iterdir()})

    def test_manifest_path_traversal_fails_without_installing(self):
        self.entries[0]["name"] = "../outside"
        self.write_manifest()
        with self.assertRaises(ValueError):
            self.install()
        self.assertFalse(self.target.exists())

    def test_source_asset_symlink_is_not_followed(self):
        original = self.source / "TD.PAL"
        original.unlink()
        outside = self.root / "outside palette"
        outside.write_bytes(self.assets["td.pal"])
        self.link(original, outside)
        with self.assertRaises(ValueError):
            self.install()
        self.assertFalse(self.target.exists())
        self.assertEqual(outside.read_bytes(), self.assets["td.pal"])

    def test_source_parent_symlink_is_rejected(self):
        link = self.root / "retail link"
        self.link(link, self.source, directory=True)
        with self.assertRaises(ValueError):
            core.validate_source(link)

    def test_nonempty_target_is_rejected_and_preserved(self):
        self.target.mkdir()
        sentinel = self.target / "user document.txt"
        sentinel.write_bytes(b"keep me")
        with self.assertRaises(ValueError):
            self.install()
        self.assertEqual(sentinel.read_bytes(), b"keep me")
        self.assertEqual(list(self.target.iterdir()), [sentinel])

    def test_install_target_control_characters_are_rejected(self):
        target = self.root / "invalid\nfolder"
        with self.assertRaises(ValueError):
            core.install(self.source, target, start_menu=False)
        self.assertFalse(target.exists())

    def test_existing_empty_target_is_supported(self):
        self.target.mkdir()
        self.install()
        self.assertTrue((self.target / "installed.json").is_file())

    def test_target_must_not_overlap_source(self):
        for target in (self.source, self.source / "new child", self.root):
            with self.subTest(target=target), self.assertRaises(ValueError):
                core.install(self.source, target, start_menu=False)
        self.assertFalse((self.source / "new child").exists())
        self.assertTrue((self.source / "TD.PAL").exists())

    def test_target_parent_symlink_is_rejected(self):
        actual = self.root / "actual parent"
        actual.mkdir()
        link = self.root / "linked parent"
        self.link(link, actual, directory=True)
        with self.assertRaises(ValueError):
            core.install(self.source, link / "new target", start_menu=False)
        self.assertEqual(list(actual.iterdir()), [])

    def test_missing_payload_fails_before_creating_target(self):
        name = "td2192.exe" if sys.platform == "win32" else "td2192"
        (self.payload / name).unlink()
        with self.assertRaises(ValueError):
            self.install()
        self.assertFalse(self.target.exists())

    def test_success_record_hashes_every_installed_file_and_progress_finishes(self):
        progress = []
        result = self.install(progress=lambda message, percent: progress.append((message, percent)))
        record = self.record()
        self.assertEqual(record["product"], "TD2192-Modern")
        self.assertEqual(record["version"], "0.9.1-test")
        self.assertEqual(Path(record["target"]), self.target)
        paths = {entry["path"] for entry in record["files"]}
        actual = {p.relative_to(self.target).as_posix() for p in self.target.rglob("*") if p.is_file() and p.name != "installed.json"}
        self.assertEqual(paths, actual)
        self.assertEqual(result["files"], len(actual))
        for entry in record["files"]:
            self.assertEqual(entry["sha256"], digest((self.target / entry["path"]).read_bytes()))
        self.assertEqual([n for _, n in progress], sorted(n for _, n in progress))
        self.assertEqual(progress[-1][1], 100)

    def test_frozen_install_copies_self_contained_uninstaller(self):
        installer = self.root / "synthetic installer.exe"
        installer.write_bytes(b"synthetic frozen installer")
        with mock.patch.object(core.sys, "frozen", True, create=True), mock.patch.object(core.sys, "executable", str(installer)):
            self.install()
        name = "uninstall.exe" if sys.platform == "win32" else "uninstall"
        self.assertEqual((self.target / name).read_bytes(), installer.read_bytes())

    def test_shortcut_failure_rolls_back_install_and_preserves_external_file(self):
        sentinel = self.root / "existing shortcut"
        sentinel.write_bytes(b"user shortcut")
        with mock.patch.object(core, "_create_shortcuts", side_effect=ValueError("shortcut already exists")):
            with self.assertRaises(ValueError):
                self.install()
        self.assertFalse(self.target.exists())
        self.assertEqual(sentinel.read_bytes(), b"user shortcut")
        self.assertFalse(list(self.root.glob(".td2192-install-*")))

    def test_uninstall_removes_unchanged_install_but_preserves_source(self):
        self.install()
        core.uninstall(self.target)
        self.assertFalse(self.target.exists())
        for name, data in self.assets.items():
            self.assertEqual((self.source / name.upper()).read_bytes(), data)

    def test_uninstall_preserves_modified_installed_and_untracked_user_files(self):
        self.install()
        modified = self.target / "game" / "td.pal"
        modified.write_bytes(b"edited after installation")
        user = self.target / "saves" / "campaign.json"
        user.parent.mkdir()
        user.write_bytes(b"player save")
        result = core.uninstall(self.target)
        self.assertEqual(modified.read_bytes(), b"edited after installation")
        self.assertEqual(user.read_bytes(), b"player save")
        self.assertIn("game/td.pal", result["preserved"])
        self.assertFalse((self.target / "game" / "m1.td").exists())

    def test_uninstall_refuses_user_data_removal_flag_without_mutation(self):
        self.install()
        with self.assertRaises(ValueError):
            core.uninstall(self.target, remove_user_data=True)
        self.assertTrue((self.target / "game" / "td.pal").exists())
        self.assertTrue((self.target / "installed.json").exists())

    def test_forged_record_paths_are_rejected_before_any_file_is_deleted(self):
        self.install()
        base = self.record()
        outside = self.root / "outside.bin"
        outside.write_bytes(b"outside sentinel")
        unsafe = ["../outside.bin", str(outside), "C:\\outside.bin", "game/../../outside.bin", "game\\td.pal"]
        for name in unsafe:
            with self.subTest(path=name):
                record = dict(base)
                record["files"] = list(base["files"]) + [{"path": name, "sha256": digest(outside.read_bytes())}]
                self.write_record(record)
                with self.assertRaises(ValueError):
                    core.uninstall(self.target)
                self.assertTrue((self.target / "game" / "td.pal").exists())
                self.assertEqual(outside.read_bytes(), b"outside sentinel")

    def test_forged_record_directories_cannot_prune_outside_installation(self):
        self.install()
        outside = self.root / "empty user folder"
        outside.mkdir()
        record = self.record()
        record["directories"] = list(record.get("directories", [])) + ["../empty user folder"]
        self.write_record(record)
        with self.assertRaises(ValueError):
            core.uninstall(self.target)
        self.assertTrue(outside.is_dir())
        self.assertTrue((self.target / "game" / "td.pal").exists())

    def test_uninstall_preserves_untracked_empty_user_directory(self):
        self.install()
        user = self.target / "my empty save folder"
        user.mkdir()
        core.uninstall(self.target)
        self.assertTrue(user.is_dir())

    def test_record_target_must_match_requested_install(self):
        self.install()
        record = self.record()
        record["target"] = str(self.root / "different installation")
        self.write_record(record)
        with self.assertRaises(ValueError):
            core.uninstall(self.target)
        self.assertTrue((self.target / "game" / "td.pal").exists())

    def test_uninstall_does_not_follow_tracked_file_replaced_by_symlink(self):
        self.install()
        path = self.target / "game" / "td.pal"
        path.unlink()
        outside = self.root / "outside.bin"
        outside.write_bytes(self.assets["td.pal"])
        self.link(path, outside)
        result = core.uninstall(self.target)
        self.assertEqual(outside.read_bytes(), self.assets["td.pal"])
        self.assertTrue(path.is_symlink())
        self.assertIn("game/td.pal", result["preserved"])

    def test_uninstall_does_not_prune_directories_through_untracked_link(self):
        self.install()
        outside = self.root / "outside directory"
        (outside / "keep empty child").mkdir(parents=True)
        link = self.target / "user linked directory"
        self.link(link, outside, directory=True)
        core.uninstall(self.target)
        self.assertTrue((outside / "keep empty child").is_dir())
        self.assertTrue(link.is_symlink())

    def test_uninstall_preserves_modified_shortcut_and_removes_unchanged_one(self):
        # Real Desktop Entry files in a redirected synthetic menu; no Qt/COM writes.
        with mock.patch.object(core.sys, "platform", "linux"):
            core.install(self.source, self.target, start_menu=True, desktop=True)
            changed = self.shortcuts[0][0]
            changed.write_bytes(b"user changed this shortcut")
            core.uninstall(self.target)
        self.assertEqual(changed.read_bytes(), b"user changed this shortcut")
        self.assertFalse(self.shortcuts[1][0].exists())
        self.assertFalse(self.shortcuts[2][0].exists())

    def test_forged_shortcut_cannot_delete_arbitrary_external_file(self):
        self.install()
        outside = self.root / "user document.txt"
        outside.write_bytes(b"outside user document")
        record = self.record()
        record["shortcuts"] = [{"path": str(outside), "sha256": digest(outside.read_bytes())}]
        self.write_record(record)
        try:
            core.uninstall(self.target)
        except ValueError:
            pass  # Rejecting the record before removal is also safe.
        self.assertEqual(outside.read_bytes(), b"outside user document")

    def test_launch_passes_installed_game_folder_as_separate_argument(self):
        with mock.patch.object(core.subprocess, "Popen") as launch:
            core.launch(self.target)
        name = "td2192.exe" if sys.platform == "win32" else "td2192"
        self.assertEqual(launch.call_args.args[0], [str(self.target / name), "--data", str(self.target / "game")])
        self.assertEqual(launch.call_args.kwargs["cwd"], self.target)
        self.assertNotIn("shell", launch.call_args.kwargs)

    def test_cli_failure_reports_json_without_creating_target(self):
        result = self.root / "result.json"
        code = core.main(["--install", "--source", str(self.root / "missing"), "--target", str(self.target), "--quiet", "--result", str(result)])
        self.assertEqual(code, 1)
        response = json.loads(result.read_text(encoding="utf-8"))
        self.assertFalse(response["ok"])
        self.assertTrue(response["error"])
        self.assertFalse(self.target.exists())

    def test_default_target_is_per_user_on_both_platforms(self):
        with mock.patch.object(core.sys, "platform", "win32"):
            self.assertEqual(core.default_target(), self.root / "localappdata" / "Programs" / "TD2192-Modern")
        with mock.patch.object(core.sys, "platform", "linux"), mock.patch.object(core.Path, "home", return_value=self.root / "home"):
            self.assertEqual(core.default_target(), self.root / "home" / "Games" / "TD2192-Modern")


if __name__ == "__main__":
    unittest.main()
