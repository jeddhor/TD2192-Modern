# Release checklist

How a TD2192-Modern release is versioned, built, checked and handed over (M13-03, M13-05). The packaging scripts are [tools/scripts/package.ps1](../tools/scripts/package.ps1) (Windows x64) and [tools/scripts/package.sh](../tools/scripts/package.sh) (Linux x64); the platforms are decision D6 (Windows x64 and Linux x64, unsigned).

**The project owner controls publication.** Agents may bump the version, update the changelog, build and test the packages, and record the results here. Creating a remote, pushing, tagging and publishing require the owner's direction (AGENTS.md rule 11, ADG §15 "M11–M13"). On 2026-11-10 the owner expressly directed publication of a public GitHub beta at `jeddhor/TD2192-Modern`. §6 lists the steps.

## 1. Version scheme

TD2192-Modern uses [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html).

**One version for everything.** The version is `[workspace.package] version` in the root `Cargo.toml`, and every crate inherits it. Nothing else holds a copy. These all read it:

- `td2192 --version` and `tdtool --version`;
- the first line of `td2192 --check`;
- the corner of the start screen;
- the package names `TD2192-Modern-<version>-windows-x64.zip` and `TD2192-Modern-<version>-linux-x64.tar.gz`.

The test `crates/td2192/tests/release.rs` checks two things:

- the binary prints the version;
- `CHANGELOG.md` has a section for it.

Both packaging scripts refuse to build without that section.

**What counts as the public interface.** For a game, SemVer's "public API" is what players and packagers rely on:

- the files the port reads and writes: `settings.toml` and the save files, which use the original's format;
- the command-line options for players (`--data`, `--episode`, `--check`, `--fullscreen`, `--no-intro`, `--version`, `--help`);
- the package layout and the supported platforms;
- the supported game data (the v1.0 file set);
- how the Classic profile plays.

The `--dev-*` options, `tdtool` and the replay files (`.tdr`) are developer tools, not part of this interface. Replays are only guaranteed to play back on the version that recorded them.

| Bump | When | Examples |
|---|---|---|
| **MAJOR** | Something a player or packager relies on breaks | `settings.toml` or saves of an earlier release no longer load; a player option is removed or changes meaning; a platform is dropped; the package layout changes incompatibly |
| **MINOR** | Something new that is compatible | a new option or Enhanced feature; a new platform; a new accessibility setting |
| **PATCH** | Fixes | bug fixes; Classic parity fixes (making Classic more like the original is a fix); documentation and packaging fixes |

The format version inside `settings.toml` (`version = 2`) is separate from the release version. It changes only when the file's layout changes, and the port migrates older files.

**Before 1.0.** Version 0.1.0 was a placeholder from M0. Version **0.9.0** was the first release candidate:

- The 1.0 scope is in: all three episodes, Classic, the Enhanced preset (D5), the accessibility options and the D6 packages.
- The 1.0 criteria of §2 are not met yet.

The rules for later candidates and for 1.0:

- **Later candidates** are 0.9.1, 0.9.2 and so on: one PATCH bump for each build handed to anyone. Under SemVer's 0.x rules a candidate may still change anything.
- **1.0.0** comes once every criterion of §2 holds. Pre-release tags such as `1.0.0-rc.1` are not used, so that the 0.9.x line stays the only one.
- **Tags**, made by the project owner (§6), are `v<version>`, for example `v0.9.0`.

## 2. Criteria for 1.0.0

All must hold at the commit to be tagged `v1.0.0`:

1. **Parity bar (MDG §12, M12 exit):** at least 95 % of the Classic items of [parity/M12-checklist.md](parity/M12-checklist.md) PASS, and the rest are documented. The current checklist records 363 of 376 Classic items PASS (96.5 %), above this bar; the historical 0.9.0 result is retained in §7.
2. **No known crasher or progression blocker.** Every bug in [STATUS.md](STATUS.md) "Bugs" of severity major or blocker is FIXED. BUG-30 (edge-wrap arithmetic) and BUG-31 (E1 flight-start state) are fixed. BUG-34 (E3 flight-16 convoy progress) is also fixed in 0.9.1; check STATUS and the candidate validation record before tagging.
3. **Whole-flight parity holds:** the L5 tests `oracle_e1_full`, `oracle_e2_full` and `oracle_e3_full` pass with `TD2192_DATA` and `TD2192_ORACLE`.
4. **M12 and M13 are DONE:** every row in STATUS is DONE, not REVIEW, and the project owner has signed off M12 (its exit criterion).
5. **Install tests pass** on Windows x64 and Linux x64. This means:
   - the smoke tests of §4 on this machine;
   - the clean-machine tests of §5, which the project owner runs.
6. **Open decisions are settled:**
   - D7 (where the release is published) is decided.
   - The rights review of risk R5 (MDG §13) is done before anything is published.
   - D8 and D10 are confirmed, or decided otherwise and implemented.
7. **Built by CI from the tag.** The M13 exit criterion asks for tagged `v1.0.0` artefacts built by CI, so this needs:
   - a remote (D7);
   - a successful run of [the release-artifacts workflow](../.github/workflows/release.yml) from the matching version tag.

   The workflow is prepared locally. Its first hosted run still needs the owner's remote/tag steps; local builds do not satisfy this CI criterion.
8. `CHANGELOG.md` has its `## [1.0.0]` section, and the version is `1.0.0`.

## 3. Checklist for a release (candidate or final)

Work in a clean checkout of the commit to be released (`git status` clean, main merged). Record the results in §7.

### 3.1 Version and changelog

- [ ] Choose the new version by the rules of §1.
- [ ] Set it in the root `Cargo.toml` (`[workspace.package] version`).
- [ ] Run any `cargo build` so that `Cargo.lock` picks up the new version.
- [ ] In `CHANGELOG.md`:
  - Rename `## [Unreleased]` to `## [<version>] - <YYYY-MM-DD>` and open a new, empty `## [Unreleased]` above it.
  - Write the section for players and packagers: what was added, changed and fixed, and the known issues.
  - Update the parity figures and the state of D1–D10.
- [ ] Commit `Cargo.toml`, `Cargo.lock` and `CHANGELOG.md` together, in a commit such as `build: version <version>`.

### 3.2 Gate

Run from the repository root:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace                                   # without the variables
$env:TD2192_DATA = 'P:\msdos\TD2192\TD2192'
$env:TD2192_ORACLE = 'P:\msdos\TD2192\re-work\captures\oracle'
cargo test --workspace                                   # with them: no data-gated test skips
```

- [ ] All four are green. Write down the passed, failed and ignored counts of both test runs.
- [ ] `crates/td2192/tests/release.rs` passes: the version and the changelog agree.

### 3.3 Parity bar

- [ ] [parity/M12-checklist.md](parity/M12-checklist.md) is current. Give its PASS share of the Classic items in the changelog. For 1.0 it must be at least 95 %.
- [ ] The whole-flight tests (`oracle_e1_full`, `oracle_e2_full`, `oracle_e3_full`) passed in the run with the variables.
- [ ] Replay determinism: `pwsh tools/scripts/replay-soak.ps1 -Rounds 5 -Parallel 16 -Data $env:TD2192_DATA` (needs a release build of `tdtool`) reports every replay identical.

### 3.4 Windows package (on this workstation)

```powershell
pwsh tools/scripts/package.ps1 -Out P:\msdos\TD2192\dist -RequireRuntimeCheck
```

The script builds into `target\package` with the static C runtime and locked dependency versions. It also checks that `td2192.exe` imports no `VCRUNTIME*`/`MSVCP*` DLL, so the package needs no Visual C++ Redistributable. `-RequireRuntimeCheck` makes a missing `dumpbin` a failure, as in CI; without it the local script warns and skips that check.

- [ ] The script ends with `Package: ...\TD2192-Modern-<version>-windows-x64.zip`, and its runtime check reported no Visual C++ DLL.

### 3.5 Linux package (in WSL Ubuntu 22.04)

```bash
cd /mnt/p/msdos/TD2192/TD2192-Modern            # or the worktree being released
source ~/.cargo/env
CMAKE=/usr/bin/cmake bash tools/scripts/package.sh --out /mnt/p/msdos/TD2192/dist --target-dir ~/td2192-linux-target
```

Two settings matter on this workstation:

- `CMAKE=/usr/bin/cmake` is needed because the machine-wide `P:\msdos\TD2192\.cargo\config.toml` points CMake at the Windows executable.
- `--target-dir` keeps the Linux build on the Linux file system.

Ubuntu 22.04 sets the minimum glibc (2.35) for the binary. Build on 22.04, not on a newer distribution.

- [ ] The script lists the tarball, and every entry is owned by `0/0` with modes 755 (binaries) and 644 (documents).
- [ ] `ldd td2192` shows only the C runtime libraries: `libc`, `libm`, `libstdc++`, `libgcc_s` and the loader. SDL 3 is linked in statically.

### 3.6 Package contents and notices

Each package must hold exactly these files, and nothing from the game:

| File | What |
|---|---|
| `td2192` / `td2192.exe` | The game |
| `tdtool` / `tdtool.exe` | The developer helper (`--no-tools` / `-NoTools` leaves it out) |
| `LICENSE` | GNU GPL version 3; the port grants version 3 or any later version in README.md |
| `README.md` | The project page |
| `CHANGELOG.md` | This release's notes |
| `THIRD_PARTY.md` | The direct dependencies and their licences |
| `PLAYER-GUIDE.md` | `docs/player/README.md` |
| `THIRD_PARTY_NOTICES.txt` | The licence texts of every linked component, for this target |
| `COPYRIGHT-rust-std.html` | The Rust standard library's notices, from the toolchain that built the binary |

`tools/scripts/gen-notices.py` writes the last two files. It covers:

- every crate that `cargo tree` builds into `td2192` for the target, without procedural macros;
- the bundled C sources: SDL 3, with hidapi, yuv2rgb, its libm notice and, on Linux, the Wayland protocol notices; and ymfm;
- the Rust standard library.

It exits with an error if a component has no licence file.

- [ ] The file list matches the table above. No `.exe` of the original, no `.scr`, `.pic`, `.voc`, `.cmf` or other game file, and no save or screenshot is in the package.
- [ ] `gen-notices.py` printed no error.
- [ ] The notices name SDL 3 (`sdl3-src`) and ymfm (`ymfm-sys`).
- [ ] Every licence in the notices is permissive. Check that THIRD_PARTY.md still matches the direct dependencies in `Cargo.toml`:

  ```powershell
  Select-String -Path P:\msdos\TD2192\dist\TD2192-Modern-<version>-windows-x64\THIRD_PARTY_NOTICES.txt -Pattern '^Licence:' | Group-Object Line | Select-Object Count, Name
  ```

  The licences allowed are MIT, Apache-2.0, BSD-2/3-Clause, Zlib, 0BSD, ISC, Unlicense and Unicode-3.0, alone or in OR expressions. A GPL or LGPL licence that is the only choice stops the portable-game release (AGENTS.md rule 11). The owner-authorized Qt6 installer has a separate runtime and notices under §3.9; it does not change the Rust game's dependency set.

### 3.7 Smoke tests

Run the tests of §4 on both packages.

### 3.8 CI artifacts from a matching tag

[release.yml](../.github/workflows/release.yml) runs after the owner pushes a `v*` tag, or by **Run workflow** with an existing `tag` input. It checks out that tag and refuses to build unless its commit is HEAD, its version matches the workspace, and the changelog contains that version. Only `vMAJOR.MINOR.PATCH` tags are accepted (§1).

Windows x64 runs on Windows Server 2022 with the static-CRT import check required. Linux x64 runs on Ubuntu 22.04 to preserve the glibc 2.35 floor. Both run synthetic helper tests, fmt, clippy and workspace tests with no game data or oracle, package with locked dependencies, validate the file list/notices, and run packaged `--version`/`--help` smoke checks. Linux also checks its shared-library list.

Each job uploads an Actions artifact holding its archive and a SHA-256 sidecar. A final job verifies both archives and uploads `SHA256SUMS` in the `release-checksums` artifact. Downloads follow repository visibility; retention is 30 days. The workflow has read-only repository permissions and no GitHub Release, deployment, registry upload or tag-writing step. It never fetches or uploads original game data.

A hosted run does **not** cover data-gated parity, a headless flight with data, first-run behavior, sound, saves or clean-machine desktop checks. Those still require §3.2–§5. Record the run URL and tag commit with the package checksums.

### 3.9 Offline Qt6 installers

The owner requested per-user Windows/Linux Qt6 setup on 2026-11-10. Build the
self-extracting installers from the already-built portable folders with
[tools/installer/build.py](../tools/installer/build.py); see its
[build guide](../tools/installer/README.md) for pinned tooling, source/notices,
private Linux runtime dependencies and exact native build commands.

Installer outputs are `TD2192-Modern-<version>-windows-x64-setup.exe` and
`TD2192-Modern-<version>-linux-x64-setup.run`, each with a checksum and source/notices
companion ZIP. Keep each companion with the installer when distributing it. They
contain Qt6/PySide6, Python and installer runtime dependencies, without retail
content; no Qt/Python installation or online download is needed on the destination.
Qt's runtime licensing is separate from the GPL-3.0-or-later game and its portable-package audit.

Defaults are `%LOCALAPPDATA%/Programs/TD2192-Modern` and `~/Games/TD2192-Modern`.
Import copies the 193 recognized retail assets, including four DOS executables used
only as text/table data. Original saves/configuration and extras are excluded.
The Qt6 uninstaller preserves saves/settings, user files, modified installed files
and modified shortcuts; it removes unchanged tracked files and matching shortcuts.
Windows also registers a per-user installed-applications entry.

Run `python -m unittest discover -s tools/installer -p test_core.py -v` on both
platforms. Test the *frozen binaries* too: render the wizard, install into a fresh
folder, compare imported hashes, check all episodes, verify shortcuts and the Windows
registry entry, uninstall, and check self-removal plus file/settings preservation.
Repeat with no user modifications to confirm empty-directory cleanup. This is a
local integration gate; fresh-machine desktop checks of §5 remain separate.

Current results and installer hashes: [2026-11-10 installer record](handoffs/2026-11-10-qt6-installers.md).
The existing hosted workflow builds portable packages; installer builds are native
local steps until a hosted installer job is added and actually run.

### 3.10 Hand-over

- [ ] Record the results in §7: date, commit, versions, package SHA-256 values, test outcomes.
- [ ] Update STATUS (M13 rows) and commit.
- [ ] Give the project owner the steps of §6.

## 4. Smoke tests (no system changes)

These run on the workstation. They need no installation, no new WSL distribution, no Docker and no Windows feature. Each package is unpacked into a new, empty temporary folder, outside the repository and the build directories. The game data is read in place and read-only: `P:\msdos\TD2192\TD2192` on Windows, `/mnt/p/msdos/TD2192/TD2192` in WSL.

The repeatable helper is [check-package.py](../tools/scripts/check-package.py), using Python 3.10+ and its standard library only. Replace `<version>` with the candidate version.

**Windows** (PowerShell):

```powershell
python tools/scripts/check-package.py --archive P:/msdos/TD2192/dist/TD2192-Modern-<version>-windows-x64.zip --version <version> --smoke --data P:/msdos/TD2192/TD2192 --seconds 20 --keep
```

**Linux** (existing WSL Ubuntu):

```bash
python3 tools/scripts/check-package.py --archive /mnt/p/msdos/TD2192/dist/TD2192-Modern-<version>-linux-x64.tar.gz --version <version> --smoke --data /mnt/p/msdos/TD2192/TD2192 --seconds 20 --keep
```

The helper validates the exact allowlist, version, target notices and permissive licences. It rejects extra files, unsafe paths, links, duplicates and missing documents; Linux requires owner 0/0 and modes 755/644. It prints SHA-256; `--sha256-out <path>` writes a `sha256sum`-compatible sidecar. Archive-only validation runs on either host; `--smoke` requires the matching target OS and x64.

With `--smoke` it unpacks validated regular files into a fresh temporary directory and runs both binaries' `--version` and the game's `--help`. Linux also rejects unexpected or missing shared libraries. With explicit `--data` it requires 218 matching files and three playable episodes from `--check`, then supervises a headless E1 flight for `--seconds` (20 by default, at least 10). It rejects an early exit, a missing 19.2015 Hz flight rate, a final average outside 10 % of that rate (or less than three seconds sampled), a panic or an ERROR log. Inspect `flight.log` for performance and warnings; stopping at the requested duration is expected.

Omit `--data` for the no-data checks used in CI; the helper removes inherited `TD2192_DATA` and `TD2192_ORACLE` from child processes. Omit `--smoke` to inspect without executing. `--keep` retains successful logs; failure logs are always retained and their temporary path printed. No screenshot or game-derived output is copied into the repository.

A screenshot is written only by `--dev-exit-after`, which these tests do not use. With `--check`, `td2192` creates its user folder and a default `settings.toml` there if none exists:

- on Linux, the test points `XDG_DATA_HOME` at the temporary folder, so this happens there;
- on Windows it is the real `%APPDATA%\TD2192-Modern\TD2192-Modern`, which the tests leave as it is when it already exists.

| Check | Pass when |
|---|---|
| `--version` (both binaries) | prints `td2192 <version>` / `tdtool <version>`, exit 0 |
| `--help` | prints the option list, exit 0 |
| `--check --data` | the version and SDL version, `218 of 218 files match`, all three episodes playable, exit 0 |
| Headless flight | the log reaches `tick rate now 19.2015 Hz` and keeps running at about 19.2 ticks/s, with no panic or error, until it is stopped |
| Linux libraries | `ldd` lists only `libc`, `libm`, `libstdc++`, `libgcc_s` and the loader, all found |
| Windows runtime | the packaging script's dumpbin check passed (no `VCRUNTIME*`/`MSVCP*`) |

M12 owner sign-off was received on 2026-11-10: audio and Enhanced features approved.

## 5. Clean-machine tests (project owner)

**Owner result, 2026-11-10:** The project owner reports that clean-machine install
tests are finished and that both the Windows and Linux Qt6 installers work. This
is an owner-reported PASS for both installers; machine details, tested file hashes,
and itemized results from the checks below were not provided. The published
[v0.9.1 beta](https://github.com/jeddhor/TD2192-Modern/releases/tag/v0.9.1)
is the current release.

The smoke tests run on a development machine, so they cannot show what is missing on a new one. Provisioning a new environment needs system changes. The owner runs or authorizes that provisioning, and performs the interactive checks in it:

- Windows Sandbox or a VM;
- a new WSL distribution or a container;
- a desktop Linux machine.

Each needs a copy of the game data, which stays with the tester and is never put into a package.

**Windows: Windows Sandbox** (a Windows feature, Pro or Enterprise; enable it once: Settings, System, Optional features, More Windows features, Windows Sandbox, then restart). A sandbox is a fresh Windows without the Visual C++ Redistributable or anything else a developer machine has.

1. Start Windows Sandbox. Copy the release zip and a copy of the game folder (or the original ZIP) into it.
2. Unpack the zip and double-click `td2192.exe`. Expected:
   - the first-start box asks for the game (Folder..., ZIP file..., Quit);
   - after choosing it, the game data page shows `218 of 218` and the three episodes;
   - continue leads to the start screen.
3. Start Episode 1: the intro, then the script choice. Play into flight 1 and check:
   - picture, music and sound effects;
   - F11 fullscreen;
   - F2 options;
   - F12 writes a screenshot into `%APPDATA%\TD2192-Modern\TD2192-Modern\screenshots`.
4. Save a game from the episode's menu, quit to the start screen, quit, start again and load the save.
5. Choose the ZIP file instead (F2, game data, choose a zip file), then **extract the zip**. Check that the files appear under `...\TD2192-Modern\game`.
6. Note any SmartScreen prompt (expected: unsigned, "More info", "Run anyway") and anything the player guide does not explain.

A config can be prepared without enabling or launching Sandbox:

```powershell
pwsh tools/scripts/new-sandbox-config.ps1 -Archive P:/msdos/TD2192/dist/TD2192-Modern-<version>-windows-x64.zip -Data P:/msdos/TD2192/TD2192 -Out P:/msdos/TD2192/dist/sandbox/candidate.wsb
```

The helper only writes a `.wsb` and sibling `.files` folder with a copy of the archive and startup script, refusing to overwrite either. The owner opens the `.wsb` when Sandbox is available. It maps package and data folders **read-only**, disables networking/clipboard/microphone/printer sharing, unpacks inside the sandbox, checks versions/help, and opens Explorer. The first-run player flow remains uninitialized for the manual tests above. For a data ZIP, use a dedicated host folder: its whole parent folder is mapped read-only. Record results before closing the disposable environment. Fields follow [Microsoft's Sandbox documentation](https://learn.microsoft.com/en-us/windows/security/application-security/application-isolation/windows-sandbox/windows-sandbox-configure-using-wsb-file).

Without Windows Sandbox, use a new Windows 10 or 11 VM (Hyper-V quick create) with nothing installed.

**Linux: a new WSL distribution** (WSLg gives it a display and sound):

```powershell
wsl --install -d Ubuntu-24.04 --name td2192-clean     # a second, empty distribution
```

Copy the tarball and the game data into it, unpack, run `./td2192`, and repeat steps 2 to 5 above. The user folder is `~/.local/share/TD2192-Modern/TD2192-Modern/`. Afterwards remove the distribution with `wsl --unregister td2192-clean`.

**Linux: oldest supported glibc.** Ubuntu 22.04 (glibc 2.35) is the floor. A container shows whether the binary starts there with only the base libraries:

```bash
docker run --rm --network none -v "$PWD:/pkg:ro" ubuntu:22.04 /pkg/TD2192-Modern-<version>-linux-x64/td2192 --version
```

Use an already available image/runtime where possible; installing Docker or pulling an image is separate from preparing this checklist. The `--version` container check only tests process startup, not display/audio/data.

**Linux desktop.** Also run the game on a real desktop (X11 and Wayland) and, if possible, on a Steam Deck in desktop mode (MDG M13-01). Check controller input there.

Record the results in §7.

## 6. Publishing (owner directed)

The owner authorized these steps for the public beta on 2026-11-10:

1. **Decide where to publish (D7)**, and finish the rights review (R5) before anything is public.
2. **Tag the release commit:** `git tag -a v<version> -m "TD2192-Modern <version>"`, on the commit recorded in §7.
3. **Push** the branch and the tag to the chosen remote, once one exists. Review the matching-tag workflow (§3.8), download both artifacts and `SHA256SUMS`, and complete the install-test record against those exact archives.
4. **Publish the packages** with checksums. Attach both packages and a `SHA256SUMS` file, made with `Get-FileHash -Algorithm SHA256` or `sha256sum`. Use the changelog section as the release notes.
5. **After the release:** the next change opens `## [Unreleased]` work toward the next version. The version in `Cargo.toml` is bumped when the next candidate is prepared (§3.1), not before.

## 7. Release and preparation records

Record the candidate commit, date, checksums and checks from §3–§5. Keep historical results identified by version; never present them as validation of a later candidate. The current handoff is [2026-11-09-release-candidate.md](handoffs/2026-11-09-release-candidate.md).

### Candidate 0.9.1 (2026-11-10)

Local build and smoke checks PASS on Windows x64 and Ubuntu 22.04 Linux x64. Both complete workspace runs pass (1,031 passed, 0 failed, 9 ignored); fmt and all-target Clippy are clean with/without data. All whole-flight oracle suites pass; 450 packaged replay runs are deterministic. Package checksums, reviewed changes, test limits and owner-only gates are in [the candidate record](handoffs/2026-11-09-release-candidate.md). No hosted CI run, clean-machine interactive test, tag or publication is claimed.

### Public beta 0.9.1 staging (2026-11-10)

The owner directed GPL-3.0-or-later licensing, a public `jeddhor/TD2192-Modern`
repository and a `v0.9.1` GitHub prerelease. The final GPL packages and both Qt6
installers were rebuilt from this source tree. The portable packages pass archive,
licence, no-data and binary smoke checks; both frozen installers render correctly
on the native Windows and Ubuntu desktops. The installer source ZIPs include the
GNU GPLv3 text and complete build/dependency notices. Python release-helper tests
(11) and installer tests (37) pass; fmt and all-target Clippy pass.

The staged release downloads are in `P:/msdos/TD2192/dist/beta-0.9.1-final/release`
outside the repository. `SHA256SUMS` covers these six files:

| Asset | SHA-256 |
|---|---|
| Windows installer `.exe` | `57c67d2faab4cde79994f5b88588e86e248c4a2f62f260bcc4613fd1fb2c85a8` |
| Windows installer source/notices `.zip` | `ba8689c002f8bb9cd9511e3fc60fc7542fb48fac715b185f03a45ed2c8d4ff17` |
| Linux installer `.run` | `9988f9ff1a2885aa987a96d304cb874519f706bf9c39793cc0dcea981405c749` |
| Linux installer source/notices `.zip` | `6a1829ba941737f20d8fcfbec1ab93194b0cd1a1487a9980ec0d3380d50bbac9` |
| Windows portable package `.zip` | `91464f9de266d740004180782cc1df9e64d2e1e92b75a95e1d39e148eaae1a81` |
| Linux portable package `.tar.gz` | `a3556cd99c8a19802847f7c513d453d1c84f1ca018ae74aeaa05d0cff9f98fa1` |

The read-only R5 technical audit found no original game assets, disassembly files
or credentials in the repository. The installers contain only numeric retail-file
metadata and require the user to supply game data. External rights advice is not
recorded. The owner subsequently reported that clean-machine installation succeeded
with the Windows and Linux installers (2026-11-10; §5). The corrected hosted
[release-artifact workflow](https://github.com/jeddhor/TD2192-Modern/actions/runs/42925074757)
and [main CI](https://github.com/jeddhor/TD2192-Modern/actions/runs/42925050093)
both completed successfully. Other 1.0 criteria remain in §2.

### Automation preparation (2026-11-09)

Implementer hat; no exports, Ghidra or disassembly consulted. Prepared the artifact-only workflow, locked packaging/notice resolution, archive/smoke helpers, and optional Sandbox-config generator. No remote, tag or publication was created, and no system feature, distribution, image or runtime was installed.

Read-only survey: WSL lists existing `Ubuntu` and `docker-desktop` distributions (both stopped at survey time); the Docker CLI is absent from PATH, but the installed client at `C:\Program Files\Docker\Docker\resources\bin\docker.exe` reports 29.5.3. Its `desktop-linux` daemon is unavailable (named pipe absent). `Containers-DisposableClientVM` reports `InstallState = 2` (disabled); `C:\Windows\System32\WindowsSandbox.exe` is absent. This records availability, not a successful clean-machine test.

Helper validation and final-candidate outcomes belong in the current handoff linked above. The figures below remain specific to 0.9.0.

### Historical 0.9.0 run

**Version 0.9.0, 2026-11-07, IM-27.** The packages were built from commit `daf2f12` on IM-27's branch, with main `948cc5f` merged. The later commits on the branch change only this record and STATUS, so the packaged files are the same.

| Item | Result |
|---|---|
| Toolchain | Rust 1.95.0. Windows: MSVC 14.51 (VS 2026) with the static C runtime. Linux: WSL Ubuntu 22.04.5, CMake 3.22.1. SDL 3.4.18 built from source |
| Gate (§3.2) | fmt and clippy clean. `cargo test --workspace` without the variables: 958 passed, 0 failed, 9 ignored (75 test binaries). With `TD2192_DATA` and `TD2192_ORACLE`: 958 passed, 0 failed, 9 ignored |
| Whole-flight L5 | `oracle_e1_full` 29/29, `oracle_e2_full` 21/21, `oracle_e3_full` 23/23; `screens_parity` 18/18; `release` 3/3 |
| Parity bar (§3.3) | 90.2 % of the Classic items (339 of 376): **below the 1.0 bar**, as expected for a candidate |
| Windows package | `TD2192-Modern-0.9.0-windows-x64.zip`, 4,608,750 bytes, SHA-256 `91EBD0851D6B8563DA8F707DAF00FBED6CE2F9D83D80AC67B9B07D407105761E`. `td2192.exe` imports only Windows system DLLs (no `VCRUNTIME140`/`MSVCP140`, no `api-ms-win-crt-*`). Console subsystem |
| Linux package | `TD2192-Modern-0.9.0-linux-x64.tar.gz`, 5,256,063 bytes, SHA-256 `0ccb312d57b28a883a424cb4b7dbe6dbb0787f7827f2ed9a93e17670dbcd02a2`. Entries owned by 0/0, modes 755/644. `ldd`: `libm`, `libstdc++`, `libgcc_s`, `libc` and the loader only. Newest glibc symbol `GLIBC_2.34` |
| Contents (§3.6) | Both packages hold exactly the nine files of the table, and no game file |
| Notices | 82 entries for Windows and 78 for Linux, each with its licence text, including `sdl3-src` (SDL, hidapi, yuv2rgb, libm; Wayland protocols on Linux), `ymfm-sys` and the Rust standard library. Every licence is permissive: MIT, Apache-2.0, Zlib, BSD-3-Clause, 0BSD, Unlicense and Unicode-3.0 |
| Smoke tests, Windows (§4) | Unpacked into a new folder under `%TEMP%`. `--version` gives `td2192 0.9.0` and `tdtool 0.9.0`. `--help` gives the list, exit 0. `--check --data P:\msdos\TD2192\TD2192` gives `218 of 218 files match` and all three episodes playable. Headless E1 flight 1 with the dummy drivers: 25 s, `tick rate now 19.2015 Hz at tick 37`, 19.15 ticks/s on average, about 29 ms per frame (SDL's software renderer), peak working set 67 MB, no warning or error |
| Smoke tests, Linux (§4) | Unpacked into a new `/tmp` folder, with `XDG_DATA_HOME` pointing at a new, empty folder. The same results; `--check` created `TD2192-Modern/TD2192-Modern/settings.toml` under it. Headless flight: 25 s, 19.19 ticks/s, about 9 ms per frame, no warning or error |
| Found and fixed | The Windows executable needed the Visual C++ Redistributable (`VCRUNTIME140.dll`, `MSVCP140.dll`), which a new Windows does not have. It now links the static runtime (§3.4) |
| Not run | §5, the clean-machine tests: Windows Sandbox or a VM, a new WSL distribution or a container, a Linux desktop and a Steam Deck. They are for the project owner |
