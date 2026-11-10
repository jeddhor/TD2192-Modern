"""Offline per-user installation. No retail assets are distributed by this project.

The manifest contains names, sizes and hashes only. Install into a new/empty directory;
uninstall removes only unchanged files recorded by this installation.
"""
from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile

PRODUCT = "TD2192-Modern"
RECORD = "installed.json"


def resource_root() -> Path:
    return Path(getattr(sys, "_MEIPASS", Path(__file__).resolve().parent))


def default_target() -> Path:
    if sys.platform == "win32":
        return Path(os.environ.get("LOCALAPPDATA", Path.home() / "AppData/Local")) / "Programs" / PRODUCT
    # Keep programs separate from SDL's existing ~/.local/share user data tree.
    return Path.home() / "Games" / PRODUCT


def default_source() -> Path | None:
    value = os.environ.get("TD2192_DATA")
    return Path(value) if value and Path(value).is_dir() else None


def _hash(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest() if hasattr(hashlib, "file_digest") else _hash_stream(stream)


def _hash_stream(stream) -> str:
    digest = hashlib.sha256()
    for block in iter(lambda: stream.read(1024 * 1024), b""):
        digest.update(block)
    return digest.hexdigest()


def _linked(path: Path) -> bool:
    info = path.lstat()
    return stat.S_ISLNK(info.st_mode) or bool(getattr(info, "st_file_attributes", 0) & 0x400)


def _plain_path(path: Path) -> Path:
    path = Path(os.path.abspath(path.expanduser()))
    if any(ord(char) < 32 for char in str(path)):
        raise ValueError("Folder names must not contain control characters.")
    for part in [path, *path.parents]:
        if part.exists() or part.is_symlink():
            if _linked(part):
                raise ValueError(f"Choose a folder without symbolic links or junctions: {part}")
    return path


def _dos_name(name: str) -> str:
    base, dot, extension = name.lower().rpartition(".")
    return base[:8] + "." + extension[:3] if dot else name.lower()[:8]


def _manifest() -> dict:
    return json.loads((resource_root() / "asset-manifest.json").read_text(encoding="utf-8"))


def validate_source(source: Path) -> list[tuple[dict, Path]]:
    source = _plain_path(source)
    if not source.is_dir():
        raise ValueError("Choose the folder containing your retail game's data files.")
    names: dict[str, Path] = {}
    for item in sorted(source.iterdir()):
        if item.is_file() and not _linked(item):
            key = _dos_name(item.name)
            if key in names:
                raise ValueError(f"Ambiguous DOS file name in the game folder: {key}")
            names[key] = item
    found = []
    missing = []
    for entry in _manifest()["files"]:
        name = entry["name"]
        if Path(name).name != name or "/" in name or "\\" in name or name in (".", ".."):
            raise ValueError("Invalid bundled asset manifest.")
        item = names.get(name)
        if item is None:
            if entry["required"]:
                missing.append(name)
        else:
            if item.stat().st_size == 0:
                raise ValueError(f"The game file is empty: {name}")
            if entry.get("exact", False) and _hash(item) != entry["sha256"]:
                raise ValueError(f"Unsupported retail version of {name}. The port needs the reference version to read its text and tables.")
            found.append((entry, item))
    if missing:
        raise ValueError(f"This folder is missing {len(missing)} required game files: " + ", ".join(missing[:12]))
    if not found:
        raise ValueError("No supported retail game data was found in that folder.")
    return found


def _report(callback, message: str, percent: int) -> None:
    if callback:
        callback(message, percent)


def _powershell(script: str, *, detached: bool = False):
    encoded = base64.b64encode(script.encode("utf-16le")).decode("ascii")
    args = ["powershell.exe", "-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-EncodedCommand", encoded]
    flags = getattr(subprocess, "CREATE_NO_WINDOW", 0)
    if detached:
        return subprocess.Popen(args, creationflags=flags, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, close_fds=True)
    return subprocess.run(args, check=True, capture_output=True, creationflags=flags)


def _ps(value) -> str:
    return "'" + str(value).replace("'", "''") + "'"


def _desktop_arg(value) -> str:
    # Desktop Entry spec: escaping occurs twice (string then Exec quoting).
    value = str(value).replace("%", "%%")
    for char in ("\\", '"', "`", "$"):
        value = value.replace(char, "\\" + char)
    return '"' + value.replace("\\", "\\\\") + '"'


def _desktop_entry(target: Path, *, uninstall: bool = False) -> str:
    executable = target / ("uninstall" if uninstall else "td2192")
    arguments = " --uninstall --target " + _desktop_arg(target) if uninstall else " --data " + _desktop_arg(target / "game")
    name = "Uninstall " + PRODUCT if uninstall else "Traffic Department 2192"
    # GLib checks the first executable token before expanding %% field escapes.
    # A fixed env launcher lets literal % in the chosen folder round-trip too.
    return f"[Desktop Entry]\nType=Application\nName={name}\nExec=/usr/bin/env -- {_desktop_arg(executable)}{arguments}\nTerminal=false\nCategories=Game;ActionGame;\n"


def _shortcut_locations(start_menu: bool, desktop: bool) -> list[tuple[Path, bool]]:
    # Tests can replace this function; no persistent environment changes.
    if not start_menu and not desktop:
        return []
    if sys.platform == "win32":
        from PySide6.QtCore import QStandardPaths
        menu = Path(QStandardPaths.writableLocation(QStandardPaths.StandardLocation.ApplicationsLocation))
        desk = Path(QStandardPaths.writableLocation(QStandardPaths.StandardLocation.DesktopLocation))
        return ([(menu / PRODUCT / "Traffic Department 2192.lnk", False), (menu / PRODUCT / "Uninstall.lnk", True)] if start_menu else []) + ([(desk / "Traffic Department 2192.lnk", False)] if desktop else [])
    from PySide6.QtCore import QStandardPaths
    applications = Path(os.environ.get("XDG_DATA_HOME", str(Path.home() / ".local/share"))) / "applications"
    desk = Path(QStandardPaths.writableLocation(QStandardPaths.StandardLocation.DesktopLocation))
    return ([(applications / "td2192-modern.desktop", False), (applications / "td2192-modern-uninstall.desktop", True)] if start_menu else []) + ([(desk / "td2192-modern.desktop", False)] if desktop else [])


def _create_shortcuts(target: Path, start_menu: bool, desktop: bool) -> list[dict]:
    created = []
    try:
        for path, is_uninstall in _shortcut_locations(start_menu, desktop):
            path = _plain_path(path)
            if path.exists():
                raise ValueError(f"A shortcut already exists; move or rename it before installing: {path}")
            path.parent.mkdir(parents=True, exist_ok=True)
            if sys.platform == "win32":
                executable = target / ("uninstall.exe" if is_uninstall else "td2192.exe")
                arguments = '--uninstall --target "' + str(target) + '"' if is_uninstall else '--data "' + str(target / "game") + '"'
                helper = resource_root() / "windows-shortcut.cs"
                if not helper.is_file():
                    helper = resource_root() / "installer-source/windows-shortcut.cs"
                if not helper.is_file():
                    raise ValueError("The installer is missing its Unicode Windows shortcut helper.")
                script = "$ErrorActionPreference='Stop'; Add-Type -Path " + _ps(helper) + "; [TD2192UnicodeLink]::Save(" + ",".join(_ps(value) for value in (path, executable, arguments, target)) + ")"
                _powershell(script)
            else:
                with path.open("x", encoding="utf-8") as stream:
                    stream.write(_desktop_entry(target, uninstall=is_uninstall))
                path.chmod(0o755)
            created.append({"path": str(path), "sha256": _hash(path)})
        return created
    except Exception:
        _remove_shortcuts(created)
        raise


def _remove_shortcuts(shortcuts: list[dict]) -> None:
    allowed = {path.absolute() for path, _ in _shortcut_locations(True, True)} if shortcuts else set()
    for item in shortcuts:
        path = Path(item["path"])
        try:
            if path.absolute() not in allowed:
                continue
            _plain_path(path)
            if path.is_file() and _hash(path) == item["sha256"]:
                path.unlink()
                if sys.platform == "win32" and path.parent.name == PRODUCT:
                    try:
                        path.parent.rmdir()
                    except OSError:
                        pass
        except (OSError, ValueError):
            pass


def _registry(target: Path, *, remove: bool = False) -> None:
    if sys.platform != "win32":
        return
    import winreg
    key = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\TD2192Modern-" + hashlib.sha256(str(target).encode()).hexdigest()[:12]
    if remove:
        try:
            winreg.DeleteKey(winreg.HKEY_CURRENT_USER, key)
        except FileNotFoundError:
            pass
        return
    with winreg.CreateKey(winreg.HKEY_CURRENT_USER, key) as handle:
        values = {"DisplayName": "Traffic Department 2192 Modern", "DisplayVersion": _manifest()["version"], "Publisher": "TD2192-Modern contributors", "InstallLocation": str(target), "UninstallString": '"' + str(target / "uninstall.exe") + '" --uninstall --target "' + str(target) + '"'}
        for name, value in values.items():
            winreg.SetValueEx(handle, name, 0, winreg.REG_SZ, value)
        for name in ("NoModify", "NoRepair"):
            winreg.SetValueEx(handle, name, 0, winreg.REG_DWORD, 1)


def install(source: Path, target: Path, *, start_menu: bool = True, desktop: bool = False, progress=None) -> dict:
    source, target = _plain_path(source), _plain_path(target)
    if target == source or target in source.parents or source in target.parents:
        raise ValueError("The installation folder must be separate from your retail game folder.")
    if target.exists() and (not target.is_dir() or any(target.iterdir())):
        raise ValueError("Choose a new or empty installation folder. Uninstall an existing installation first.")
    _report(progress, "Checking your retail game files…", 2)
    assets = validate_source(source)
    payload = resource_root() / "payload"
    executable = "td2192.exe" if sys.platform == "win32" else "td2192"
    if not (payload / executable).is_file():
        raise ValueError("The installer is missing its precompiled game payload.")
    target.parent.mkdir(parents=True, exist_ok=True)
    stage = Path(tempfile.mkdtemp(prefix=".td2192-install-", dir=target.parent))
    moved = False
    shortcuts = []
    try:
        _report(progress, "Copying the game and its dependencies…", 10)
        shutil.copytree(payload, stage, dirs_exist_ok=True)
        for folder in ("installer-notices", "installer-source"):
            origin = resource_root() / folder
            if origin.is_dir():
                shutil.copytree(origin, stage / folder)
        (stage / "game").mkdir()
        modified = []
        for index, (entry, original) in enumerate(assets):
            _plain_path(original)
            dest = stage / "game" / entry["name"]
            shutil.copyfile(original, dest)
            if entry.get("exact", False) and _hash(dest) != entry["sha256"]:
                raise ValueError(f"The retail file changed while importing: {entry['name']}")
            if _hash(dest) != entry["sha256"]:
                modified.append(entry["name"])
            _report(progress, f"Importing {entry['name']}…", 15 + index * 65 // len(assets))
        if getattr(sys, "frozen", False):
            uninstaller = stage / ("uninstall.exe" if sys.platform == "win32" else "uninstall")
            shutil.copyfile(sys.executable, uninstaller)
            uninstaller.chmod(0o755)
            internal = Path(sys.executable).parent / "_internal"
            if internal.is_dir() and resource_root().resolve() == internal.resolve():
                # Rebuilt --onedir installers retain replaceable dynamic libraries.
                shutil.copytree(internal, stage / "_internal")
        else:
            # Development mode only; release builds always copy the self-contained executable.
            script = stage / ("uninstall.cmd" if sys.platform == "win32" else "uninstall")
            if sys.platform == "win32":
                script.write_text(f'@echo off\r\n"{sys.executable}" "{resource_root() / "ui.py"}" --uninstall --target "%~dp0"\r\n', encoding="utf-8")
            else:
                import shlex
                script.write_text(f'#!/bin/sh\nexec {shlex.quote(sys.executable)} {shlex.quote(str(resource_root() / "ui.py"))} --uninstall --target {shlex.quote(str(target))}\n', encoding="utf-8")
            script.chmod(0o755)
        if sys.platform == "win32":
            (stage / "Play.cmd").write_text('@echo off\r\n"%~dp0td2192.exe" --data "%~dp0game" %*\r\n', encoding="utf-8")
        else:
            (stage / executable).chmod(0o755)
            if (stage / "tdtool").exists():
                (stage / "tdtool").chmod(0o755)
            (stage / "play").write_text('#!/bin/sh\nHERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)\nexec "$HERE/td2192" --data "$HERE/game" "$@"\n', encoding="utf-8")
            (stage / "play").chmod(0o755)
        files = [{"path": str(p.relative_to(stage)).replace("\\", "/"), "sha256": _hash(p)} for p in sorted(stage.rglob("*")) if p.is_file()]
        directories = [p.relative_to(stage).as_posix() for p in stage.rglob("*") if p.is_dir()]
        if target.exists():
            target.rmdir()  # Only the empty directory checked above; a race makes rmdir fail safely.
        stage.rename(target)
        moved = True
        _report(progress, "Creating your shortcuts…", 85)
        shortcuts = _create_shortcuts(target, start_menu, desktop)
        record = {"product": PRODUCT, "version": _manifest()["version"], "target": str(target), "files": files, "directories": directories, "shortcuts": shortcuts, "modified_assets": modified}
        (target / RECORD).write_text(json.dumps(record, indent=2), encoding="utf-8")
        _registry(target)
        _report(progress, "Installation complete.", 100)
        return {"target": str(target), "files": len(files), "modified_assets": modified}
    except Exception:
        _remove_shortcuts(shortcuts)
        _registry(target, remove=True)
        if moved:
            # Roll back only our newly created paths. Never recursively delete target.
            for item in files:
                path = target / item["path"]
                if path.is_file() and not _linked(path) and _hash(path) == item["sha256"]:
                    path.unlink()
            (target / RECORD).unlink(missing_ok=True)
            _prune(target, directories)
        raise
    finally:
        if stage.exists():
            shutil.rmtree(stage)  # Unique staging directory created by this invocation.


def _prune(target: Path, directories: list[str]) -> None:
    for name in sorted(directories, key=lambda name: len(Path(name).parts), reverse=True):
        path = target / name
        try:
            _plain_path(path)
        except ValueError:
            continue
        if path.is_dir():
            try:
                path.rmdir()
            except OSError:
                pass
    try:
        target.rmdir()
    except OSError:
        pass


def uninstall(target: Path, *, remove_user_data: bool = False, progress=None) -> dict:
    if remove_user_data:
        raise ValueError("Saves and settings are always preserved; remove them separately if desired.")
    target = _plain_path(target)
    record_path = target / RECORD
    _plain_path(record_path)
    record = json.loads(record_path.read_text(encoding="utf-8"))
    if record.get("product") != PRODUCT or Path(record.get("target", "")) != target:
        raise ValueError("This folder does not contain a matching installation record.")
    # Validate the entire record before changing anything.
    for name in [item["path"] for item in record["files"]] + record.get("directories", []):
        rel = Path(name)
        if rel.is_absolute() or ".." in rel.parts or not rel.parts or "\\" in name or ":" in name:
            raise ValueError("The installation record contains an unsafe file path.")
    preserved = []
    delayed = []
    for index, item in enumerate(record["files"]):
        path = target / item["path"]
        try:
            _plain_path(path)
            if path.is_file():
                if _hash(path) == item["sha256"]:
                    try:
                        path.unlink()
                    except PermissionError:
                        if sys.platform == "win32" and getattr(sys, "frozen", False) and path == Path(sys.executable):
                            delayed.append(item)
                        else:
                            raise
                else:
                    preserved.append(item["path"])
        except ValueError:
            preserved.append(item["path"])
        _report(progress, "Removing installed files…", index * 90 // max(len(record["files"]), 1))
    _remove_shortcuts(record.get("shortcuts", []))
    _registry(target, remove=True)
    record_path.unlink()
    _prune(target, record.get("directories", []))
    if delayed:
        # A Windows executable cannot unlink itself while its bootloader is running.
        # A hidden helper waits for both child and parent, verifies the exact file hash,
        # then removes just that executable and the directory if it is empty.
        for item in delayed:
            path = target / item["path"]
            script = f"foreach ($cleanupId in @({os.getpid()},{os.getppid()})) {{ try {{ $cleanupProcess=[Diagnostics.Process]::GetProcessById($cleanupId); $cleanupProcess.WaitForExit(); $cleanupProcess.Dispose() }} catch {{}} }}; "
            # Framework APIs also work when the host's inherited PSModulePath
            # prevents loading optional PowerShell cmdlets such as Get-FileHash.
            script += "$cleanupPath=" + _ps(path) + "; if ([IO.File]::Exists($cleanupPath)) { $cleanupStream=[IO.File]::OpenRead($cleanupPath); $cleanupSha=[Security.Cryptography.SHA256]::Create(); try { $cleanupHash=[BitConverter]::ToString($cleanupSha.ComputeHash($cleanupStream)).Replace('-','') } finally { $cleanupStream.Dispose(); $cleanupSha.Dispose() }; if ($cleanupHash -eq " + _ps(item["sha256"]) + ") { [IO.File]::Delete($cleanupPath) } }; try { [IO.Directory]::Delete(" + _ps(target) + ") } catch {}"
            _powershell(script, detached=True)
    _report(progress, "Uninstalled. Your saves, settings and modified files were kept.", 100)
    return {"target": str(target), "files": len(record["files"]), "preserved": preserved}


def launch(target: Path) -> None:
    binary = target / ("td2192.exe" if sys.platform == "win32" else "td2192")
    subprocess.Popen([str(binary), "--data", str(target / "game")], cwd=target, close_fds=True)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="TD2192-Modern offline installer")
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument("--install", action="store_true")
    action.add_argument("--uninstall", action="store_true")
    parser.add_argument("--source", type=Path)
    parser.add_argument("--target", type=Path, default=default_target())
    parser.add_argument("--start-menu", action=argparse.BooleanOptionalAction, default=True)
    parser.add_argument("--desktop", action=argparse.BooleanOptionalAction, default=False)
    parser.add_argument("--quiet", action="store_true")
    parser.add_argument("--result", type=Path, help="Write JSON success/error details, useful with the Windows GUI executable")
    args = parser.parse_args(argv)
    try:
        if args.install:
            if args.source is None:
                parser.error("--install requires --source")
            result = install(args.source, args.target, start_menu=args.start_menu, desktop=args.desktop)
        else:
            result = uninstall(args.target)
        result["ok"] = True
        code = 0
    except Exception as error:
        result = {"ok": False, "error": str(error)}
        code = 1
    if args.result:
        args.result.write_text(json.dumps(result, indent=2), encoding="utf-8")
    if not args.quiet and sys.stdout:
        print(json.dumps(result))
    return code
