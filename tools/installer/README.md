# Qt 6 installer build

Installer source code is licensed under GPL-3.0-or-later; the full GNU GPLv3
text accompanies each installer and its source ZIP. Bundled third-party
components retain the licences recorded in their notices.

The installer wraps precompiled portable game binaries. It never compiles Rust,
downloads retail data or includes retail content. Build separately on Windows x64
and Ubuntu 22.04 x64; PyInstaller does not cross-compile.

The maintained entry point is ui.py (PySide6 QtWidgets), with core.py alongside.
Frozen resources are payload/, asset-manifest.json, payload-manifest.json,
installer-notices/ and installer-source/ under sys._MEIPASS. Complete licence
and source trees are held in LICENSES.zip and SOURCE.zip inside those directories
to keep onefile startup fast. NOTICE.txt, GPL/LGPL texts, runtime inventory,
README.md and the Windows shortcut helper remain immediately readable. Source execution uses
the installer directory as its resource root; --prepare-only prints a generated
resource tree for local testing.

## Native build

Use Python 3.10+ in a virtual environment and install the pinned build requirements:

    python -m pip install -r tools/installer/requirements-build.txt

Windows, using the already-built portable package:

    python tools/installer/build.py --payload P:/msdos/TD2192/dist/TD2192-Modern-0.9.1-windows-x64 --output P:/msdos/TD2192/dist/installers

Ubuntu 22.04 / WSL, using its own Python environment:

    /mnt/p/msdos/TD2192/re-work/scratch/installer-build/linux-venv/bin/python tools/installer/build.py --payload /mnt/p/msdos/TD2192/dist/TD2192-Modern-0.9.1-linux-x64 --output /mnt/p/msdos/TD2192/dist/installers

The default output is a Windows .exe or executable Linux .run, plus its SHA-256
sidecar and a source/notices ZIP. The Windows bootloader uses the GUI subsystem.
The recipient needs no Python, pip, Qt SDK or compiler. Linux still needs a
supported desktop/display stack and glibc 2.35 or newer; this is not a container.

The build refuses existing installer output and payloads that contain anything
beyond the nine portable-package files. Work and resources are retained below
the output directory for inspection. Use a new output directory for another
candidate. Nothing is published by this script.

## Retail import metadata

asset-manifest.json is generated from the checked-in Rust inventory facts.
Every used asset is included except original documents, config, saves, unused
presentations and unused executables. Four executables (file0000.exe,
file0001.exe, td2.exe and td3.exe) are required data-only text/table resources:
their known original hashes are exact requirements, and they are never executed.
The installer imports these files only from the user's local retail copy.

Manifest entries contain name, size, sha256 and required. The four resource
executables additionally have exact:true. Optional intro, dialogue, font and
music assets retain their original required flags. No retail bytes enter the
installer build. payload-manifest.json records the bundled port binaries/docs
separately.

## Runtime licences and rebuilding

Qt/PySide6/Shiboken6 use the LGPLv3 option. PyInstaller's COPYING.txt contains
its GPL bootloader exception and runtime-hook terms. The game retains its own
licence and existing third-party notices; installer runtime notices are separate.

Build preparation downloads exact-version upstream source archives over HTTPS,
verifies their official SHA-256 sidecars and retains the licence directories and
third-party attribution records. The source URLs/hashes and runtime versions
are in installer-notices/build-info.json. PyInstaller's actual runtime binary
inventory is recorded after Analysis. On Ubuntu the corresponding Debian package
copyright files and package versions are copied for bundled system libraries.
CPython's complete licence is copied from the installed Windows distribution or
its pinned upstream release. Windows Microsoft runtime origin/terms are identified
separately. The licence tree deliberately includes a superset of QtBase, QtWayland
and QtSvg notices, without claiming every upstream component is linked. Linux's
wheel-bundled ICU version is read through its API, and its matching Unicode
licence/source URL and checksum are recorded too.

--cache DIR shares these source downloads across native builds. --offline requires
that cache to be populated already. --prepare-only generates resources/notices
without freezing an executable. There is no download at install time.

All installer Python source, numeric manifest_data.rs, generated asset/payload manifests, this recipe and pinned requirements accompany the
installer in installer-source/ and the separate source ZIP. Keep the source ZIP
with distributed installers. A standalone source ZIP rebuild uses:

    python build.py --payload /path/to/extracted/portable-package --output /path/to/new-output --manifest-source manifest_data.rs

The portable payload comes from the matching public package; retail data is never
a build input. The fallback paths in build.py support this standalone layout.

To debug or replace the dynamically linked Qt
libraries, rebuild with --onedir in a clean virtual environment, then replace the
PySide6/Qt libraries below its runtime folder with compatible modified versions.
The build script's version assertions are intentionally explicit; adjust the
PINS/requirements together when rebuilding against a modified Qt package.
The onedir form supports debugging/relinking and retains its runtime directory
when installed. The default production installer is onefile. Do not distribute
an onedir executable without its complete runtime folder.

Upstream references:

- Qt/PySide6: https://doc.qt.io/qtforpython-6/
- Source archives: https://download.qt.io/official_releases/QtForPython/pyside6/
- QtBase source: https://download.qt.io/official_releases/qt/
- PyInstaller licensing: https://pyinstaller.org/en/stable/license.html
- PyInstaller operation: https://pyinstaller.org/en/stable/operating-mode.html

## Verification before handoff

Inspect PyInstaller warnings and installer-notices/runtime-binaries.json.
On Linux, run ldd on the frozen Qt platform plugins and confirm none is missing.
Smoke-test the native installer with its offscreen Qt platform, then perform a
desktop install/uninstall using a copied retail source. Check shortcuts, data-only
EXE import, launch, hash validation, interrupted install cleanup, and preservation
of saves/settings and files the installer does not own. Never test by writing to
the pristine retail directory.


## Ubuntu private runtime dependency

Qt 6.5+ needs libxcb-cursor.so.0 for the X11 platform plugin. On a builder without
that library, download the Ubuntu libxcb-cursor0 package into a scratch cache
with apt-get download (no system installation), extract it with dpkg-deb -x,
and use --runtime-root DIR. DIR contains usr/lib and usr/share/doc from the
package plus packages.json, a JSON array of records with name, version, source,
sha256 (the downloaded .deb), origin and files (relative paths from its tar
contents, including symlinks). The build adds these libraries, copies their
copyright files, and records the complete package provenance in the notices.
The cache used for the local candidate is
P:/msdos/TD2192/re-work/scratch/installer-build/private-runtime.

The actual Linux candidate build command is:

    /mnt/p/msdos/TD2192/re-work/scratch/installer-build/linux-venv/bin/python tools/installer/build.py --payload /mnt/p/msdos/TD2192/dist/TD2192-Modern-0.9.1-linux-x64 --output /tmp/td2192-installer-dist-final --cache /mnt/p/msdos/TD2192/re-work/scratch/installer-build/source-cache --runtime-root /mnt/p/msdos/TD2192/re-work/scratch/installer-build/private-runtime

Build work stays on the Linux filesystem for speed. Copy the resulting .run,
.sha256 and source ZIP to the desired output directory before leaving the build
session; /tmp may be cleaned when the environment restarts. The Qt platform
plugin's final ldd check must report no missing libraries.
