# Qt6 installers: accepted local candidates (2026-11-10)

The owner approved M12 ("audio is great, enhanced features are all awesome.") and
requested Qt6 self-contained, self-extracting Windows/Linux installers. This change
adds the offline installers around the precompiled **0.9.1** game. No Rust gameplay
code changed during this installer task. No commit, tag, remote or publication was
requested or performed.

## Accepted artifacts

Directory: `P:/msdos/TD2192/dist/installers`. Each artifact has a `.sha256` sidecar;
`SHA256SUMS` covers all four files. Both source ZIPs must accompany distributed
installers. Hashes were independently verified after copying the accepted candidates.

| File | Bytes | SHA-256 |
|---|---:|---|
| `TD2192-Modern-0.9.1-windows-x64-setup.exe` | 27,503,278 | `4ae6fa6404586d2c1dc6873cc29d61a327842b2f19147c0603dbff41d91248b5` |
| `TD2192-Modern-0.9.1-linux-x64-setup.run` | 58,205,712 | `aaf6ef5815496d1fbb142aaa7b747f8d84200efc1365222ac72dcec6ca69865b` |
| `TD2192-Modern-0.9.1-windows-x64-setup-source.zip` | 908,621 | `cf7845e287534caada64100beb9f56b9c18987d7899e0f398d28a98e41feea60` |
| `TD2192-Modern-0.9.1-linux-x64-setup-source.zip` | 1,218,951 | `08391b6ec26090c480d16a9a588173b44000e732fd31659501a3bdb6057a878e` |

These contain no retail game content. The matching portable game binaries are
unchanged from the earlier [0.9.1 candidate](2026-11-09-release-candidate.md).
Both source ZIPs' seven maintained installer source/build/test files were checked
byte-for-byte against the working tree before acceptance.

## Behavior

- Per-user defaults: `%LOCALAPPDATA%/Programs/TD2192-Modern` and
  `~/Games/TD2192-Modern`. Linux programs are deliberately separate from SDL's
  pre-existing user-data directory. Other writable new/empty folders may be chosen.
- Native Qt6 install/uninstall wizard; background copying and validation; no separate
  Qt or Python installation, elevation, network access or persistent environment changes.
- Folder picker imports 193 allowlisted retail assets into `game/`. Four reference
  DOS executables are read as text/table data and never run. Their reference hashes
  are required. Original saves/configuration, unused programs and extras are excluded.
  Modified non-executable assets are copied and identified in the result.
- Optional Start Menu/Applications entries (initially selected) and desktop shortcut
  (initially unchecked), with explicit installed-data arguments. Windows has a per-user
  installed-applications entry. `Play.cmd` / `play` also launch the installed data.
- Tracked-file uninstaller preserves saves, settings, new files and modified files or
  shortcuts. Windows removes its running uninstaller after exit using a hidden,
  hash-checking helper. Source/destination overlap, linked paths and nonempty targets
  are refused; installation stages separately and rolls back failures.
- Complete runtime licences, source URLs, dependency hashes and installer source are
  included. Licence/source trees are archived to keep self-extraction quick. The
  wizard displays licence text without extracting the archives. Game licensing at the time of this handoff
  remains separate from the owner-requested Qt/PySide6 LGPL runtime.

## Verification

| Check | Result |
|---|---|
| Synthetic core regression suite | **37/37 PASS** on Windows Python 3.10.11 and Ubuntu Python 3.10.12, final sources |
| Frozen Qt GUI | Native Windows and Ubuntu offscreen/X11 (WSLg) render and exit successfully; screenshots inspected |
| Windows final install | 193 imported files match the retail source; all three episodes playable; self-contained uninstaller matches setup |
| Windows final uninstall | 217 tracked files processed; user file and modified palette kept; original 218 files unchanged; uninstaller removes itself |
| Windows clean cycle with real shortcuts | Three Start Menu/desktop `.lnk` files created and inspected through native Unicode `IShellLinkW`; exact target/arguments/working directory; installed-apps registry entry created and removed; all shortcuts and empty target removed |
| Windows isolated shortcut edge cases | Six links round-trip Unicode, spaces, apostrophes, dollars and punctuation; five unchanged links removed, modified link retained; collision rollback verified |
| Linux final default install | Uses `~/Games/TD2192-Modern` with existing SDL user data; 193 exact imported assets; three episodes playable; three Applications/desktop entries created |
| Linux imported-data runtime | Episode 3 flight 15 runs 60 ticks headlessly; no panic/error; expected timing mode reached |
| Linux final uninstall | Modified desktop entry, modified asset, user file and real game settings retained; other shortcuts, game and uninstaller removed; original 218 files unchanged |
| Linux second clean cycle | A fresh installation with no modifications is completely removed, including its directory |
| Linux desktop argument parsing | Native GIO launcher round-trips game and uninstall arguments with Unicode, quotes, backslashes, dollars, literal percent/field-code-like text and punctuation; no shell expansion |
| Linux Qt runtime dependencies | All four frozen desktop platform plugins resolve their shared libraries; private `libxcb-cursor0` is bundled with package version/hash/copyright |
| Artifact/source integrity | All four accepted SHA-256 values verified after promotion; source/build/test inputs identical to maintained files |

Evidence is outside the repository under `re-work/scratch/installer-native-windows-last`,
`installer-native-linux-last` and `installer-build`. Native screenshot PNGs contain only
installer UI. Windows uses the actual user Start Menu/Desktop for the final clean cycle;
all created entries were removed. Linux uses isolated HOME/XDG paths and does not modify
the regular Linux user profile.

Tooling: Qt/PySide6/Shiboken6 **6.11.1**, PyInstaller **6.20.0**, hooks **2026.5**.
Windows runtime inventory has 33 binaries; Linux has 143. The source/notices archives
record exact Qt/PySide sources, Python versions, runtime packages and ICU attribution.

## Limits and release work remaining

These are local integration checks on the Windows workstation and Ubuntu 22.04 WSL.
A fresh Windows/Linux machine or VM was not provisioned. Windows Sandbox remains
disabled; no Windows feature or global package was installed. Manual clean-machine
picture/audio/controller/X11/Wayland checks remain the existing release gate.

**Later owner update (2026-11-10):** The owner reports that clean-machine install
tests have since been completed and that both Windows and Linux installers
work. Individual machine and checklist details were not supplied; the results above
remain the earlier local integration record.

The earlier complete Rust gates (1,031 passed, 0 failed, 9 ignored with and without
original data), fmt and Clippy remain the game candidate's record; installer work
changed Python/C# and documentation only. The existing hosted workflow builds portable
packages and has not run; installer CI, D7 hosting/visibility, rights review and the
other previously documented public-release decisions remain outstanding. This is
still 0.9.1, not a claim that 1.0 has been released.

Build/use instructions: [tools/installer/README.md](../../tools/installer/README.md).
Contract: [installer spec](../spec/frontend/installer.md). Release checklist:
[RELEASE.md](../RELEASE.md#39-offline-qt6-installers).
