"""Build a native Qt installer around an existing data-free portable game package.

No Rust compilation or retail data access. Generated resources, downloads and
PyInstaller work stay outside the repository under --output.
"""
from __future__ import annotations

import argparse
import ctypes
import hashlib
import fnmatch
import posixpath
import importlib.metadata as md
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import urllib.request
import zipfile

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1] if (HERE.parents[1] / "crates/td-assets/src/manifest_data.rs").is_file() else None
PINS = {"PySide6-Essentials": "6.11.1", "shiboken6": "6.11.1",
        "PyInstaller": "6.20.0", "pyinstaller-hooks-contrib": "2026.5"}
DOCS = {"LICENSE", "README.md", "CHANGELOG.md", "THIRD_PARTY.md", "PLAYER-GUIDE.md",
        "THIRD_PARTY_NOTICES.txt", "COPYRIGHT-rust-std.html"}
RESOURCE_EXES = {"file0000.exe", "file0001.exe", "td2.exe", "td3.exe"}


def sha256(path):
    h = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def json_file(path, data):
    Path(path).write_text(json.dumps(data, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def asset_manifest(path, version):
    """Convert numeric inventory facts; original EXEs are data-only resources."""
    pattern = re.compile(
        r'ManifestEntry \{ name: "([^"]+)", size: (\d+), sha256: "([0-9a-f]{64})", '
        r'category: Category::(\w+), used_by: (.+), required: (true|false) \},')
    files, seen = [], set()
    for line in Path(path).read_text(encoding="utf-8").splitlines():
        if "ManifestEntry {" not in line:
            continue
        match = pattern.fullmatch(line.strip())
        if not match:
            raise ValueError("Unsupported inventory syntax; update this parser explicitly.")
        name, size, digest, category, usage, required = match.groups()
        exact = name in RESOURCE_EXES
        if not exact and (category in {"Exe", "Doc", "Config", "Save", "Presentation"}
                          or usage == "Usage::NONE"):
            continue
        if name != PurePosixPath(name).name or "\\" in name or name in seen:
            raise ValueError(f"Unsafe or duplicate asset name: {name}")
        seen.add(name)
        entry = dict(name=name, size=int(size), sha256=digest, required=exact or required == "true")
        if exact:
            entry["exact"] = True
        files.append(entry)
    if not RESOURCE_EXES <= seen:
        raise ValueError("Inventory omits an executable required for text/table resources.")
    return dict(version=version, files=files)


def payload_inventory(payload, target):
    binaries = {"td2192.exe", "tdtool.exe"} if target == "windows-x64" else {"td2192", "tdtool"}
    entries = list(payload.iterdir())
    if {p.name for p in entries} != DOCS | binaries or any(
            p.is_symlink() or not p.is_file() or p.stat().st_size == 0 for p in entries):
        raise ValueError("Payload must contain exactly nine portable-package files; no links/game data.")
    exe = payload / ("td2192.exe" if target == "windows-x64" else "td2192")
    with exe.open("rb") as source:
        magic = source.read(4)
    if not (magic.startswith(b"MZ") if target == "windows-x64" else magic == b"\x7fELF"):
        raise ValueError("Payload executable is not native to this platform.")
    return [dict(name=p.name, size=p.stat().st_size, sha256=sha256(p)) for p in sorted(entries)]


def download(url, cache, offline=False):
    cache.mkdir(parents=True, exist_ok=True)
    path = cache / url.rsplit("/", 1)[-1]
    if not path.is_file():
        if offline:
            raise ValueError(f"Not cached for offline build: {url}")
        print(f"Downloading {url}", flush=True)
        temporary = path.with_suffix(path.suffix + ".part")
        with urllib.request.urlopen(url, timeout=120) as source, temporary.open("wb") as out:
            shutil.copyfileobj(source, out)
        temporary.replace(path)
    return path


def source_notices(url, cache, dest, offline):
    archive = download(url, cache, offline)
    checksum = download(url + ".sha256", cache, offline)
    digest = checksum.read_text(encoding="utf-8").split()[0].lower()
    if not re.fullmatch("[0-9a-f]{64}", digest) or sha256(archive) != digest:
        raise ValueError(f"Source checksum mismatch: {archive}")
    count = 0
    with tarfile.open(archive, "r:*") as source:
        members = {item.name: item for item in source.getmembers() if item.isfile() and item.size <= 2_000_000}
        selected = set()
        for name, item in members.items():
            rel = PurePosixPath(name)
            if rel.is_absolute() or ".." in rel.parts:
                raise ValueError(f"Unsafe source path: {rel}")
            base = rel.name.lower()
            if ("LICENSES" in rel.parts or base == "qt_attribution.json" or
                    base.startswith(("license", "licence", "copying", "copyright", "notice", "authors"))):
                selected.add(name)
            if base == "qt_attribution.json":
                raw = source.extractfile(item).read().decode("utf-8")
                # Upstream attribution JSON occasionally embeds literal linebreaks.
                try:
                    items = json.loads(raw, strict=False)
                    items = items if isinstance(items, list) else [items]
                except json.JSONDecodeError:
                    items = []
                for attribution in items:
                    references = attribution.get("LicenseFiles", attribution.get("LicenseFile", []))
                    references = [references] if isinstance(references, str) else references
                    for reference in references:
                        pattern = posixpath.normpath(str(rel.parent / reference))
                        selected.update(filename for filename in members if fnmatch.fnmatchcase(filename, pattern))
        for name in sorted(selected):
            rel = PurePosixPath(name)
            output = dest.joinpath(*rel.parts[1:])
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_bytes(source.extractfile(members[name]).read())
            count += 1
    if not count:
        raise ValueError(f"No source licence files: {archive}")
    return dict(url=url, sha256=digest, notice_files=count)


def distribution_notices(name, dest):
    dist = md.distribution(name)
    count = 0
    for item in dist.files or []:
        rel = PurePosixPath(str(item))
        if rel.is_absolute() or ".." in rel.parts:
            continue
        if ("/licenses/" in str(rel).lower() or
                rel.name.lower().startswith(("license", "copying", "notice"))):
            source = Path(dist.locate_file(item))
            if source.is_file() and source.stat().st_size < 2_000_000:
                output = dest.joinpath(*rel.parts)
                output.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(source, output)
                count += 1
    if not count:
        if name not in {"PySide6-Essentials", "shiboken6"}:
            raise ValueError(f"Missing installed licence: {name}")
        # Some official wheels omit licence files; the matching source archive
        # has already supplied the full LGPL/GPL texts and third-party notices.
        dest.mkdir(parents=True, exist_ok=True)
        (dest / "WHEEL-METADATA.txt").write_text(dist.read_text("METADATA") or "", encoding="utf-8")
    return dict(version=dist.version, license=dist.metadata.get("License", ""),
                project_urls=dist.metadata.get_all("Project-URL", []))



def filter_qt_plugins(kind, rows):
    # QtWidgets needs native/offscreen desktop platforms, not PDF, QtQuick's
    # virtual keyboard, networking touch drivers or optional image codecs.
    if kind in {"imageformats", "iconengines", "generic"}:
        return []
    result = []
    for filename, destination in rows:
        name = Path(filename).name.lower()
        if "virtualkeyboard" in name:
            continue
        if kind == "platforms" and not any(
                token in name for token in ("qwindows.", "qoffscreen.", "qminimal.", "qxcb.",
                                            "qwayland")):
            continue
        result.append((filename, destination))
    return result


def filter_runtime_binaries(rows):
    if os.name != "nt":
        return rows
    # Windows 10/11 supply UCRT/API-set libraries. Ignore unrelated PATH copies
    # (for example a JDK) and Qt's unused Mesa OpenGL software-renderer fallback.
    return [entry for entry in rows
            if not Path(entry[1]).name.lower().startswith(("api-ms-win-", "ext-ms-win-"))
            and Path(entry[1]).name.lower() not in {"ucrtbase.dll", "opengl32sw.dll"}]



def bundle_resources(stage, bundle):
    stage, bundle = Path(stage), Path(bundle)
    bundle.mkdir()
    shutil.copytree(stage / "payload", bundle / "payload")
    for name in ("asset-manifest.json", "payload-manifest.json"):
        shutil.copy2(stage / name, bundle / name)
    for name, overview, archive in (
            ("installer-notices", "NOTICE.txt", "LICENSES.zip"),
            ("installer-source", "README.md", "SOURCE.zip")):
        dest = bundle / name
        dest.mkdir()
        shutil.copy2(stage / name / overview, dest / overview)
        if name == "installer-source" and (stage / name / "windows-shortcut.cs").is_file():
            shutil.copy2(stage / name / "windows-shortcut.cs", dest / "windows-shortcut.cs")
        if name == "installer-notices":
            for filename in ("runtime-binaries.json", "build-info.json", "Microsoft-VC-runtime.txt"):
                shutil.copy2(stage / name / filename, dest / filename)
            for licence in (stage / name / "qtbase/LICENSES").glob("*"):
                if licence.name.upper().startswith(("GPL-", "LGPL-")):
                    shutil.copy2(licence, dest / licence.name)
        with zipfile.ZipFile(dest / archive, "w", zipfile.ZIP_DEFLATED) as out:
            for path in sorted((stage / name).rglob("*")):
                if path.is_file():
                    out.write(path, str(path.relative_to(stage / name)))
    # The full uncompressed trees remain in stage and the external companion ZIP.


def filter_runtime_datas(rows):
    # The setup UI is English. Qt's hundreds of unused translation files would
    # needlessly slow onefile startup; keep all code and platform plugins.
    return [row for row in rows if "/translations/" not in ("/" + row[0].replace("\\", "/"))]


def finalize_runtime_notices(binaries, dest):
    """Invoked by the PyInstaller spec after dependency analysis."""
    dest = Path(dest)
    qt_roots = [Path(md.distribution(name).locate_file(folder)).resolve()
                for name, folder in [("PySide6-Essentials", "PySide6"), ("shiboken6", "shiboken6")]]
    records, packages = [], set()
    private_root = os.environ.get("TD2192_INSTALLER_RUNTIME_ROOT")
    private_root = Path(private_root).resolve() if private_root else None
    private_packages = (json.loads((private_root / "packages.json").read_text(encoding="utf-8"))
                        if private_root else [])
    for name, filename, _kind in binaries:
        path = Path(filename).resolve()
        record = dict(name=name, size=path.stat().st_size, sha256=sha256(path))
        if any(path.is_relative_to(base) for base in qt_roots):
            record["notices"] = "Qt/PySide6 source notices; unmodified wheel libraries"
        elif os.name == "nt" and path.is_relative_to(Path(sys.base_prefix).resolve()):
            record["notices"] = "Python/LICENSE.txt"
        elif private_root and path.is_relative_to(private_root):
            relative = path.relative_to(private_root).as_posix()
            owner = next((p for p in private_packages if relative in p["files"]), None)
            if not owner:
                raise ValueError(f"Private library has no package provenance: {relative}")
            copyright_file = private_root / "usr/share/doc" / owner["name"] / "copyright"
            output = dest / "system-libraries" / owner["name"] / "copyright"
            output.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(copyright_file, output)
            record["notices"] = f"system-libraries/{owner['name']}/copyright"
            record["package"] = owner
        elif os.name != "nt":
            owners = None
            for candidate in (Path(filename), path):
                result = subprocess.run(["dpkg-query", "-S", str(candidate)], capture_output=True, text=True)
                if result.returncode == 0:
                    owners = result.stdout
                    break
            if not owners:
                raise ValueError(f"Runtime binary has no Debian package owner: {path}")
            package = owners.splitlines()[0].rsplit(": ", 1)[0]
            short = package.split(":")[0]
            copyright_file = Path("/usr/share/doc") / short / "copyright"
            if not copyright_file.is_file():
                raise ValueError(f"Missing copyright: {package}")
            output = dest / "system-libraries" / short / "copyright"
            output.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(copyright_file, output)
            packages.add(package)
            record["notices"] = f"system-libraries/{short}/copyright"
        elif re.match(r"(msvcp|vcruntime|concrt|vcomp)\d", path.name, re.I):
            record["notices"] = "Microsoft-VC-runtime.txt"
        else:
            raise ValueError(f"Unclassified runtime dependency: {path}")
        records.append(record)
    json_file(dest / "runtime-binaries.json", records)
    if private_packages:
        json_file(dest / "private-runtime-packages.json", private_packages)
    if packages:
        fmt = chr(36) + "{Package} " + chr(36) + "{Version}\n"
        result = subprocess.run(["dpkg-query", "-W", "-f=" + fmt, *sorted(packages)],
                                capture_output=True, text=True, check=True)
        (dest / "system-library-versions.txt").write_text(result.stdout, encoding="utf-8")


def prepare(args, stage, target, version):
    from PySide6.QtCore import qVersion
    for name, expected in PINS.items():
        if md.version(name) != expected:
            raise ValueError(f"{name}: expected {expected}, found {md.version(name)}")
    payload = payload_inventory(args.payload, target)
    shutil.copytree(args.payload, stage / "payload")
    assets = asset_manifest(args.manifest_source, version)
    json_file(stage / "asset-manifest.json", assets)
    json_file(stage / "payload-manifest.json", dict(version=version, target=target, files=payload))
    notices = stage / "installer-notices"
    notices.mkdir()
    qt = qVersion()
    series = ".".join(qt.split(".")[:2])
    sources = {}
    # Preserve a superset of the modules/plugins collected for QtWidgets.
    for module in ("qtbase", "qtwayland", "qtsvg"):
        url = f"https://download.qt.io/official_releases/qt/{series}/{qt}/submodules/{module}-everywhere-src-{qt}.tar.xz"
        sources[module] = source_notices(url, args.cache, notices / module, args.offline)
    pyside = PINS["PySide6-Essentials"]
    url = f"https://download.qt.io/official_releases/QtForPython/pyside6/PySide6-{pyside}-src/pyside-setup-everywhere-src-{pyside}.tar.xz"
    sources["PySide6-Shiboken6"] = source_notices(url, args.cache, notices / "PySide6", args.offline)
    if os.name != "nt":
        import PySide6
        for library in (Path(PySide6.__file__).parent / "Qt/lib").glob("libicuuc.so.*"):
            major = library.name.rsplit(".", 1)[-1]
            values = (ctypes.c_uint8 * 4)()
            getattr(ctypes.CDLL(str(library)), "u_getVersion_" + major)(values)
            release = "-".join(str(n) for n in list(values)[:2])
            url = f"https://raw.githubusercontent.com/unicode-org/icu/release-{release}/icu4c/LICENSE"
            licence = download(url, args.cache / ("ICU-" + release), args.offline)
            (notices / "ICU").mkdir(exist_ok=True)
            shutil.copy2(licence, notices / "ICU/LICENSE")
            sources["ICU"] = dict(version=".".join(str(n) for n in list(values)[:2]),
                                 url=url, sha256=sha256(licence),
                                 source=f"https://github.com/unicode-org/icu/tree/release-{release}")
    distributions = {name: distribution_notices(name, notices / name) for name in PINS}
    python_license = Path(sys.base_prefix) / "LICENSE.txt"
    if not python_license.is_file():
        python_license = download(
            f"https://raw.githubusercontent.com/python/cpython/v{platform.python_version()}/LICENSE",
            args.cache / ("Python-" + platform.python_version()), args.offline)
    (notices / "Python").mkdir()
    shutil.copy2(python_license, notices / "Python/LICENSE.txt")
    (notices / "Microsoft-VC-runtime.txt").write_text(
        "Microsoft Visual C++ Runtime binaries, when present, come unmodified from official "
        "Python/Qt wheels. Copyright Microsoft Corporation. All rights reserved.\n"
        "Terms: https://visualstudio.microsoft.com/license-terms/\n"
        "Inventory and hashes: runtime-binaries.json\n", encoding="utf-8")
    source = stage / "installer-source"
    source.mkdir()
    for path in sorted(HERE.iterdir()):
        if path.is_file() and (path.suffix in {".py", ".cs"} or path.name in ("README.md", "requirements-build.txt")):
            shutil.copy2(path, source / path.name)
    shutil.copy2((REPO / "LICENSE") if REPO else (HERE / "LICENSE"), source / "LICENSE")
    shutil.copy2(args.manifest_source, source / "manifest_data.rs")
    shutil.copy2(stage / "asset-manifest.json", source / "asset-manifest.json")
    shutil.copy2(stage / "payload-manifest.json", source / "payload-manifest.json")
    info = dict(version=version, target=target, python=platform.python_version(), qt=qt,
                distributions=distributions, sources=sources, payload_files=payload,
                retail_asset_count=len(assets["files"]), manifest_source_sha256=sha256(args.manifest_source),
                python_source=f"https://www.python.org/ftp/python/{platform.python_version()}/Python-{platform.python_version()}.tar.xz",
                pyinstaller_source=f"https://github.com/pyinstaller/pyinstaller/tree/v{PINS['PyInstaller']}",
                qt_linkage="dynamic, unmodified wheel libraries")
    json_file(notices / "build-info.json", info)
    json_file(source / "build-info.json", info)
    (notices / "NOTICE.txt").write_text(
        "Installer runtime notices\nQt/PySide6/Shiboken6: LGPLv3 option; complete GPL/LGPL texts "
        "and exception/third-party notices from pinned upstream sources accompany this build.\n"
        "qtbase/qtwayland/qtsvg preserve a superset of upstream notices, not a claim every "
        "upstream module is linked. Actual runtime binaries: runtime-binaries.json.\n"
        "PyInstaller COPYING.txt includes its GPL bootloader exception and Apache-2.0 runtime-hook terms.\n"
        "Python/LICENSE.txt covers CPython and third-party code in the official binary.\n"
        "Linux shared-library copyright files are added after analysis.\n"
        "Installer source/rebuild recipe: ../installer-source; --onedir permits runtime-library "
        "replacement. Modification/replacement for debugging is permitted.\n"
        "Source URLs, versions and hashes: build-info.json. No retail game data is bundled.\n", encoding="utf-8")
    return info


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--payload", type=Path, required=True)
    ap.add_argument("--output", type=Path, required=True)
    ap.add_argument("--cache", type=Path)
    ap.add_argument("--version")
    ap.add_argument("--runtime-root", type=Path, help="private extracted Debian runtime packages with packages.json")
    ap.add_argument("--manifest-source", type=Path, default=(REPO / "crates/td-assets/src/manifest_data.rs") if REPO else (HERE / "manifest_data.rs"))
    ap.add_argument("--prepare-only", action="store_true")
    ap.add_argument("--offline", action="store_true")
    ap.add_argument("--onedir", action="store_true", help="debug/relinkable folder instead of one file")
    args = ap.parse_args()
    if sys.platform not in ("win32", "linux") or platform.machine().lower() not in ("amd64", "x86_64"):
        ap.error("Build natively on Windows x64 or Linux x64.")
    args.payload = args.payload.resolve(strict=True)
    args.output = args.output.resolve()
    if REPO and (args.output == REPO or args.output.is_relative_to(REPO)):
        ap.error("Generated installers/resources must stay outside the repository.")
    args.output.mkdir(parents=True, exist_ok=True)
    args.cache = (args.cache or args.output / ".source-cache").resolve()
    if args.runtime_root:
        args.runtime_root = args.runtime_root.resolve(strict=True)
        if not (args.runtime_root / "packages.json").is_file():
            ap.error("--runtime-root requires packages.json with package names/versions/hashes/file lists")
    target = "windows-x64" if os.name == "nt" else "linux-x64"
    match = re.fullmatch(r"TD2192-Modern-(\d+\.\d+\.\d+)-(windows|linux)-x64", args.payload.name)
    version = args.version or (match.group(1) if match else None)
    if not version or not re.fullmatch(r"(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)", version):
        ap.error("Supply --version MAJOR.MINOR.PATCH or a versioned package folder.")
    name = f"TD2192-Modern-{version}-{target}-setup"
    filename = name if args.onedir else name + (".exe" if os.name == "nt" else ".run")
    destination = args.output / filename
    if destination.exists() and not args.prepare_only:
        ap.error(f"Refusing to replace existing output: {destination}")
    work = Path(tempfile.mkdtemp(prefix=f".{name}-", dir=args.output))
    stage = work / "resources"
    stage.mkdir()
    info = prepare(args, stage, target, version)
    print(f"Resources: {stage}\nRetail metadata: {info['retail_asset_count']} files; no retail content.", flush=True)
    if args.prepare_only:
        return
    for entry in ("core.py", "ui.py"):
        if not (HERE / entry).is_file():
            raise ValueError(f"Installer source not ready: {entry}")
    import PySide6
    excluded = sorted({p.stem for p in Path(PySide6.__file__).parent.glob("Qt*.pyi")} -
                      {"QtCore", "QtGui", "QtWidgets"})
    excludes = ["PySide6." + name for name in excluded] + ["PyQt5", "PyQt6", "PySide2", "tkinter"]
    spec = work / (name + ".spec")
    extra_binaries = []
    if args.runtime_root:
        for path in sorted((args.runtime_root / "usr/lib").rglob("*.so*")):
            if path.is_file():
                extra_binaries.append((str(path), "."))
    prefix = (
        "import importlib.util\nfrom PyInstaller.building.datastruct import Tree\n"
        f"_spec=importlib.util.spec_from_file_location('installer_build', {str(Path(__file__).resolve())!r})\n"
        "_build=importlib.util.module_from_spec(_spec); _spec.loader.exec_module(_build)\n"
        "from PyInstaller.utils.hooks.qt import QtLibraryInfo\n"
        "_collect=QtLibraryInfo.collect_plugins\n"
        "QtLibraryInfo.collect_plugins=lambda self,kind: _build.filter_qt_plugins(kind,_collect(self,kind))\n"
        f"a=Analysis([{str(HERE / 'ui.py')!r}], pathex=[{str(HERE)!r}], binaries={extra_binaries!r}, datas=[], "
        f"hiddenimports=['core'], hookspath=[], hooksconfig={{}}, runtime_hooks=[], excludes={excludes!r}, noarchive=False)\n"
        "a.binaries = _build.filter_runtime_binaries(a.binaries)\n"
        f"_build.finalize_runtime_notices(a.binaries, {str(stage / 'installer-notices')!r})\n"
        f"_build.bundle_resources({str(stage)!r}, {str(work / 'bundle')!r})\n"
        "a.datas = _build.filter_runtime_datas(a.datas)\n"
        f"a.datas += Tree({str(work / 'bundle')!r}, prefix='')\npyz=PYZ(a.pure)\n")
    if args.onedir:
        suffix = (f"exe=EXE(pyz,a.scripts,[],exclude_binaries=True,name={name!r},debug=False,strip=False,upx=False,console={os.name != 'nt'!r})\n"
                  f"coll=COLLECT(exe,a.binaries,a.datas,strip=False,upx=False,name={name!r})\n")
    else:
        suffix = f"exe=EXE(pyz,a.scripts,a.binaries,a.datas,[],name={name!r},debug=False,strip=False,upx=False,console={os.name != 'nt'!r})\n"
    spec.write_text(prefix + suffix, encoding="utf-8")
    build_env = os.environ.copy()
    build_env["PYINSTALLER_CONFIG_DIR"] = str(work / "pyinstaller-cache")
    if args.runtime_root:
        build_env["TD2192_INSTALLER_RUNTIME_ROOT"] = str(args.runtime_root)
        libdir = str(args.runtime_root / "usr/lib/x86_64-linux-gnu")
        build_env["LD_LIBRARY_PATH"] = libdir + os.pathsep + build_env.get("LD_LIBRARY_PATH", "")
    subprocess.run([sys.executable, "-m", "PyInstaller", "--noconfirm", "--clean",
                    "--distpath", str(work / "dist"), "--workpath", str(work / "work"), str(spec)],
                   check=True, env=build_env)
    built = work / "dist" / (name + (".exe" if os.name == "nt" and not args.onedir else ""))
    if args.onedir:
        shutil.copytree(built, destination)
    else:
        shutil.copy2(built, destination)
        destination.chmod(destination.stat().st_mode | 0o111)
        (args.output / (filename + ".sha256")).write_text(f"{sha256(destination)}  {filename}\n", encoding="utf-8")
    companion = args.output / (name + "-source.zip")
    with zipfile.ZipFile(companion, "w", zipfile.ZIP_DEFLATED) as out:
        for folder in ("installer-source", "installer-notices"):
            for path in sorted((stage / folder).rglob("*")):
                if path.is_file():
                    out.write(path, str(path.relative_to(stage)))
    print(f"Installer: {destination}\nSource/notices: {companion}\nBuild record: {stage / 'installer-notices/build-info.json'}")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.CalledProcessError, tarfile.TarError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        sys.exit(1)
