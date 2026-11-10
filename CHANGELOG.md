# Changelog

What changed in each release of TD2192-Modern, for players and packagers. The format follows [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html) as described in [docs/RELEASE.md](docs/RELEASE.md) §1. Developers' detail is in [docs/STATUS.md](docs/STATUS.md) and [docs/parity/](docs/parity/).

TD2192-Modern contains no part of the original game. You need your own copy of *Traffic Department 2192* version 1.0 (the installed folder or the original ZIP file). See the [player guide](docs/player/README.md), included in the packages as `PLAYER-GUIDE.md`.

## [Unreleased]

## [0.9.1] - 2026-11-10

The second release candidate. The Classic parity checklist now passes 363 of 376 items
(96.5%), above the 95% release bar. The owner approved M12 on 2026-11-10.
Version 1.0 still needs clean-machine install results and the remaining release
criteria. Version 0.9.1 is published as a beta prerelease.

### Added

- **Qt6 offline installers for Windows and Linux:** self-contained setup bundles the precompiled game, imports the recognized files from a local retail folder, offers menu/desktop shortcuts and includes an uninstaller. Installation defaults to the current user; uninstall preserves saves, settings and changed files.
- **Optional analog steering:** under Enhanced, the profile page can enable proportional stick thrust with steady facing. It is off by default; keyboard and D-pad movement keep full strength. Analog recordings use replay format 4 and play back without a controller.
- **Release preparation:** tag-checked Windows and Linux artifact builds, archive/checksum validation, repeatable smoke tests and a generator for an isolated Windows Sandbox test.

### Changed

- **License and distribution:** the project and installer source are now GPL-3.0-or-later. The first public beta is hosted at `jeddhor/TD2192-Modern` on GitHub. Original game data remains bring-your-own.
- **Windows:** opening the game no longer creates an extra console window. Command-line output is retained when launched from a terminal or redirected; Explorer launches show errors in a dialog. Diagnostics are also written to `td2192.log` in the user folder.

### Fixed

- **Held keys across screens:** flight entry keeps the keyboard's last-key history; releasing a later key no longer revives an older held movement key (U-582).
- **Retry prompts:** invisible taunt typing now consumes its original character-count delay before the fade, instead of appearing about half a second early (U-585).
- **Enhanced fixes:** ships with wrapped offsets no longer appear as ghost sprites or engine lights. Classic retains the original rendering, and simulation and replays are unchanged (OB-175).
- **Dialogue:** the first typed line now overlaps the final fade writes, as in the original, rather than starting two frames late (BUG-33). Confirmed with a dense reference recording; the comparison no longer uses a startup timing allowance.
- **Episode 3, flights 15 and 16:** trucks leave the map at the original exit coordinates. Letting flight 16's convoy leave now triggers the transmission and allows the mission to continue (BUG-34). Checked through all three truck departures in the original recording.
- **Episode 1, flight 18:** a retry after losing to the bomb is no longer lost at once. Every flight now starts with the mission's counters, alarms and transmission lamp cleared, as in the original, in all three episodes (BUG-31).
- **Very long flights:** a mission ship that wraps round at the top or bottom edge of the map no longer stops the game with an error when it fires or is hit. Hits, shots and the drawing of such a ship now follow the original's 16-bit arithmetic (BUG-30). Checked against the original in Episode 3, flight 19, for 4,499 ticks.
- **Episodes 2 and 3:** the white flash of a homing-missile kill and the red hit flash are applied in the original's order (BUG-32; visible only in one rare case in the red infra-red mode).

## [0.9.0] - 2026-11-07

The first release candidate for 1.0. It contains what is planned for 1.0, except analog steering:

- all three episodes;
- the faithful Classic profile;
- the Enhanced profile;
- the accessibility options;
- packages for Windows and Linux.

**Why 0.9.0 and not 1.0.0:**

- 90.2 % of the Classic parity checklist passes. The 1.0 bar is 95 %.
- Two major bugs are still open (see Known issues).
- The project owner has not yet signed off the fidelity review.

The criteria for 1.0.0 are listed in [docs/RELEASE.md](docs/RELEASE.md) §2. Further 0.9.x candidates will follow until they are met.

### Added

#### The game

- **All three episodes:**
  - Episode 1 (Vulthaven): 21 flights.
  - Episode 2 (Vultigard): 20 flights.
  - Episode 3 (Kolor): 21 flights.

  Each episode has its main menu, new game, saving and loading, the story scenes and dialogue, briefings, the flights, debriefings, retries and endings.
- **The intro** before Episode 1, as the original launcher played it. The original's **credits** are on the start screen.
- **A start screen** in place of the original's separate programs. It lets you choose an episode, the options, the credits or quit. An episode whose files are missing is shown as not installed. Quitting an episode returns here.
- **Episode 1's script choice:** the original or the edited dialogue, asked at the start as the original did.
- **Saving and loading** in the original's own file format and slots. The saves are kept in your user folder, apart from the original game's own saves (decision D3).
- **The original's five harmless cheat key combinations** work as they did, under both profiles (decision D9).

#### Classic profile (the default): the original, checked against it

Classic shows the original 320 × 200 picture in its 4:3 shape and keeps the original timing (19.2 flight ticks per second), controls, music and sound effects. It also keeps the original rules, including the original's bugs, except those listed under Known differences below.

The port was checked against recordings of the original game running in DOSBox-X:

- **Every flight matches tick for tick:**
  - Covered: all 62 flights of the three episodes, flown through to the end of the mission with the same inputs.
  - Compared after every tick, from setup to the outcome: the random-number stream, the player's ship, every mission ship, every shot, the mission state and the hit flash.
  - Result: no difference in any compared field. That is more than 48,000 ticks. Reports: [M9-playthrough](docs/parity/M9-playthrough.md) and [M10-playthrough](docs/parity/M10-playthrough.md).
- **More flight recordings also match tick for tick:**
  - a death in flight;
  - a flight lost in Episode 1;
  - the pause, the tactical map and the computer menu during a flight;
  - missiles and the afterburner;
  - two flights in one session;
  - music switched off.
- **Screens match pixel for pixel, with every palette entry:**
  - The front-end screens: 35 screens of the three episodes, among them debriefings, orders, dossiers, story pages, stills, endings and end cards. Report: [M12-screens](docs/parity/M12-screens.md).
  - The intro, the dialogue scenes and the flight playfield.
- **Music** sends the same sound-chip register writes as the original, checked write for write against recordings. **Sound effects** decode as the original's sound card played them.
- **Parity checklist:** 339 of the 376 Classic items pass (90.2 %). The rest are partial (29) or not yet covered (8). Each one is documented in [M12-checklist](docs/parity/M12-checklist.md).
- **Soak tests:** more than a billion ticks of random-input flights, and every recorded replay, ran deterministically. They found one crash, which is fixed. Report: [M12-soak](docs/parity/M12-soak.md).

#### Enhanced profile (optional; decision D5)

Choosing **enhanced** turns on a preset. Each part of it can then be changed on its own:

- **16:9 widescreen** in flight. 16:10, 21:9 and "auto" are also offered. Widescreen is marked as affecting gameplay, because you see further.
- **xBR 3× pixel-art upscaling.** Scale2x/3x/4x and xBR 2×/4× are also offered.
- **Subtle dynamic lighting.** "Full" adds a night look with soft shadows. **Bloom** is available and off in the preset.
- **Polyphonic sound effects**, so several effects can play at once.
- **Fixes for the original's annoyances** that Classic keeps, for example music restarting from the top or no way to cancel typing a save name. The full list is in the player guide, section 8.
- The intro can play always, only on the first launch, or never. The credits are shown as pages.

#### Accessibility

These options are under F2, then accessibility. They are off by default and work under both profiles:

- **Colour-blind guidance line:** orange, sky blue and white with a black outline, in place of red, green and cyan.
- **Reduce flashing:** the colours of the flight picture change gradually, under the WCAG 2.3.1 flash limits. It also softens the flight-17 cut-scene and the Enhanced lights, and turns bloom off. It changes only what you see.
- **Text waits for a key:** story pages that close by themselves wait for a key instead.

Also:

- Every flight key can be rebound, on the keyboard and on a controller.
- The whole game can be played with a controller.
- The game can pause while its window is in the background.
- Save names follow your keyboard layout.
- The player guide carries a photosensitivity warning.

#### Installing and game data

- **First start:** choose the installed game folder or the original ZIP file. A ZIP file is read in place. It can also be extracted into your user folder, but only if you agree. The game data can be changed later under F2, game data.
- **File check:**
  - Every file is checked against the 218 files of version 1.0.
  - Files that differ are reported as "modified" and do not stop the game.
  - `td2192 --check` prints the result.
- **Nothing is written to the game folder.** Settings, saves, screenshots and extracted files go to one user folder.
- **Command line:** `--version`, `--help`, `--check`, `--data`, `--episode`, `--no-intro` and `--fullscreen`.

#### Packages (decision D6)

- **Windows x64:** a portable zip.
- **Linux x64:** a tarball. It needs glibc 2.35 or newer and X11 or Wayland.

Both packages:

- are unsigned;
- link SDL 3 statically, and on Windows also the Visual C++ runtime;
- need nothing else installed on a usual desktop system;
- contain `td2192`, the developer helper `tdtool`, `LICENSE`, `README.md`, `CHANGELOG.md`, `THIRD_PARTY.md`, `PLAYER-GUIDE.md` and `THIRD_PARTY_NOTICES.txt` (the licence of every component linked into the program);
- contain no game data.

### Known differences from the original

The full catalogue, with the reasons, is in [docs/spec/original-bugs.md](docs/spec/original-bugs.md).

**Fixed in every profile**, because the original crashes, hangs or blocks progress there:

- A sound effect played very far away stopped the original with a runtime error. Now it is simply not played.
- Missing or short save files crashed the save and load screens. Now the slots are empty, and the first save creates the files.
- A damaged dialogue file could hang the original. It no longer does.
- In Episode 3, the flight-16 speed patch leaked into retries, New Game and loaded games, and could freeze ships and block the campaign. This was decided by the project owner.
- Episode 1, flight 8 could never be completed if both Vulture IIs were gone before you got near them. The flight now goes on. This is the only fix that changes play.
- Holding Alt in an infra-red flight flashed the whole screen about ten times a second. Now Alt switches the view once per press. The project owner decided this on safety grounds.

**Deliberate differences**, because the original's behaviour cannot or should not be reproduced on a modern system:

- Timed screens count frames. The original's 16-bit clock could end a wait at once when it wrapped round; that no longer happens.
- A controller button can close "hit any key" screens, including the transmission screen.
- Episode 1's S key mutes and unmutes the sound effects. The original switched them to the PC speaker, which is not available yet.
- A title-font character the original drew from uninitialised memory is left blank.
- The launcher's memory checks and sound-card setup are replaced by the start screen and the options.

**Left out:**

- The two cheat combinations that changed your disk's volume label (decision D9).
- The TFP "about" and "order" extras run no separate programs. Their pictures are shown in the game instead (decision D4).
- Saves from the original DOS version are not imported (decision D3).

**Fixed only under Enhanced:** the annoyances listed in the player guide, section 8. Classic keeps them, as the original did.

### Known issues

- **Two major bugs are open:**
  - Episode 1, flight 18: retrying after losing to the bomb is lost at once.
  - Very long flights: a mission ship pressed against the top or bottom edge of the map for minutes wraps round, as in the original. For example, this happens after about 3 min 40 s in Episode 3, flight 19. Until it is removed, the game can stop with an error if that ship fires or is hit.
- **Not yet compared with the original:**
  - what carries over from one flight to the next in a long campaign;
  - the joystick;
  - Episode 2 and 3 deaths, aborts and lost flights;
  - positioned sound effects;
  - the typing speed of the transmission screen.
- **Not yet available:**
  - the PC-speaker sound mode;
  - analog steering (an option planned in decision D5);
  - a macOS build.
- **Windows:** `td2192.exe` is a console program, so starting it from Explorer also opens a console window. Neither package is signed, so Windows SmartScreen may warn about an unknown publisher.

### Project decisions

These decisions belong to the project owner ([MDG §14](docs/MASTER_DESIGN_GUIDE.md#14-open-decisions)). This is where they stand for this release:

| ID | Decision | State |
|---|---|---|
| D1 | Language and stack | Decided: Rust and SDL 3 |
| D2 | Licence of the port's code | Decided: MIT |
| D3 | Import the original's save files | Decided: no. The port keeps its own saves in the original format |
| D4 | TFP "about" and "order" extras | Decided: not in 1.0. The menu items show their pictures |
| D5 | Enhanced features in 1.0 | Decided: the Enhanced preset above. Colour-blind guidance, skip intro and analog steering are separate options (analog steering is not implemented yet) |
| D6 | Platforms and signing | Decided: Windows x64 and Linux x64, unsigned. macOS later |
| D7 | Repository hosting and visibility | Open. The repository stays private until a rights review, so there is no public download location yet |
| D8 | Default script variant | Open. The port asks, as the original does (the recommendation) |
| D9 | Original cheat combinations | Decided: the five harmless ones as in the original, in both profiles. The two that touched the host system are never ported |
| D10 | Other versions of the original programs | Open. The port runs with a warning, and text read from the programs may be missing (the recommendation) |
