# Offline desktop installer (revision 1)

Modern-only infrastructure requested by the owner on 2026-11-10. This is not DOS behavior.

- Windows x64 and Linux x64 installers contain the precompiled 0.9.1 game/tool,
  dependencies, Qt6/PySide6 GUI runtime, licence notices and installer source. They
  extract themselves temporarily and require neither Qt nor Python on the destination.
- Default installation is per-user: `%LOCALAPPDATA%/Programs/TD2192-Modern` on
  Windows and `~/Games/TD2192-Modern` on Linux. No elevation or persistent
  environment changes. Users may choose another writable new/empty directory.
- A local retail folder is required. The import allowlist is generated from the
  checked-in asset manifest; original saves/configuration, documentation, unused
  executables and TFP extras are excluded (D3/D4). Four original executables are
  imported **as data only** because the port reads text and tables from them. They
  are never launched. Their hashes must match the resource maps' reference version;
  an unknown version is rejected with a message naming the file (D10).
- Retail paths and files are read only. Source/destination overlap and linked path
  components are refused. Missing required files stop before installation. Modified
  non-executable assets are allowed, identified by reference hash, and reported.
- Installation uses a temporary sibling directory, then an atomic rename. Existing
  nonempty destinations are refused. Failure rolls back the new files and shortcuts.
- Optional per-user Start Menu/Applications entries are selected by default;
  desktop shortcuts are optional and initially unchecked. Existing shortcuts are
  not overwritten. Every launch explicitly supplies the installed `game/` folder.
- A bundled Qt6 uninstaller removes unchanged tracked files and matching shortcuts.
  Windows adds an entry to the current user's installed-applications registry.
  Uninstall retains changed files, unrelated files/directories, saves and settings.
  Only empty installer-created directories are removed. No registry/elevation or
  arbitrary recursive deletion is used to remove game data. A hidden Windows helper
  deletes only the verified running uninstaller after its processes exit.
- GUI file checks, copying and removal run off the UI thread; closing is disabled
  during operations. CLI install/uninstall and JSON result reports support native
  regression tests without a display.

Qt/PySide6 are separate LGPL runtime dependencies expressly requested by the owner;
they retain their own licences alongside the Rust game's GPL-3.0-or-later
licence. Build notices include corresponding source locations, LGPL/GPL texts,
third-party attributions, PyInstaller's exception,
and a reproducible onedir mode for replacing the dynamic Qt libraries.
