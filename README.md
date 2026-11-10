# TD2192-Modern

A clean-room, modern reimplementation of **Traffic Department 2192** (Safari Software, 1994), the top-down hoverskid combat game set on the desert planet Seche, for current Windows and Linux (macOS later).

> **Status:** public beta **0.9.1**, not yet 1.0.
>
> - **What is in it:** all three episodes, which play from the intro to their endings, with saving, the options, controller support, the Enhanced profile and accessibility options.
> - **How it is checked:** against recordings of the original game. Every flight of the three episodes matches the original tick for tick, and 96.5 % of the parity checklist passes (see [docs/parity/](docs/parity/)).
> - **What 1.0 waits for:** clean-machine install tests and the remaining release checklist.
>
> What is in this release and what is still missing: [CHANGELOG.md](CHANGELOG.md). Progress: [docs/STATUS.md](docs/STATUS.md).

## What this is

- A from-scratch engine that **plays your own copy** of the original game data. No original game files are included or distributed.
- Faithful by default (the **Classic** profile): original resolution and aspect, timing, controls, music and sound. The optional **Enhanced** profile adds conveniences.
- Built from written behaviour specifications produced by reverse engineering the original, not from its source code, which is not available (see [ADR-0002](docs/adr/0002-clean-room-protocol.md)).

## Running it

**Players: see the [player guide](docs/player/README.md)** for installing, pointing the game at your copy, controls, options, Classic and Enhanced, and troubleshooting.

You need an original Traffic Department 2192 installation: the v1.0 file set of 218 files, including `TD1.EXE` and `FILE0001.EXE`, as an installed folder or the original ZIP file. On first start the game asks for it; a ZIP is read in place and can be extracted into your user folder. You can also pass it with `--data <folder or zip>` or set the `TD2192_DATA` environment variable, and change it later under Options, game data.

```text
td2192                     start screen: choose an episode, options, quit
td2192 --episode N         start Episode N (1, 2 or 3) directly, as the original does
td2192 --check             report whether the installation is complete
td2192 --fullscreen
td2192 --version
```

Building from source needs the Rust toolchain pinned in `rust-toolchain.toml`. SDL 3 is built by Cargo, which needs CMake and a C/C++ compiler. Run `cargo run --release -p td2192`. The release packages are built with `tools/scripts/package.ps1` (Windows) and `tools/scripts/package.sh` (Linux); see [docs/RELEASE.md](docs/RELEASE.md).

### Offline desktop setup

Download the Windows or Linux installer and its matching source/notices ZIP from the
[0.9.1 beta release](https://github.com/jeddhor/TD2192-Modern/releases/tag/v0.9.1).
The release also provides SHA-256 checksums and optional portable packages.

The Qt6 setup executable for Windows and `.run` installer for Linux contain the
precompiled game and installer runtime. Run setup, choose a writable installation
folder and point it at your local retail game folder. Setup copies the recognized
game assets into its own `game/` directory and offers Start Menu/Applications and
desktop shortcuts. Qt and Python do not need to be installed separately.

Defaults are `%LOCALAPPDATA%/Programs/TD2192-Modern` on Windows and
`~/Games/TD2192-Modern` on Linux. Run the installed uninstaller to remove the
game; saves, settings and changed files are kept. Windows also lists it under your
installed applications. For Linux, make the downloaded `.run` executable first.
Use `Play.cmd` / `play` in the installation folder when launching without shortcuts.
See [installer build and verification instructions](tools/installer/README.md).

### Controls (as in the original)

| Key | In flight |
|---|---|
| Numeric keypad 8 / 2 / 4 / 6, or the arrow keys | Thrust up / down / left / right (the ship turns toward the direction) |
| Space | Fire (hold for the cannons; press once per missile) |
| Tab | Switch between cannons and missiles |
| Keypad + or keypad 0 (Ins) | Afterburner burst (Stiletto-class ships) |
| Alt | Infra-red view on/off (where available) |
| F5 / F6 | Tactical map / incoming transmissions |
| M | Repeat the last message |
| P | Pause (any key resumes) |
| Esc | Computer menu (ship library, help, abort the flight) |

A game controller works as the original joystick. Use the left stick or D-pad to steer, South (A) to fire and East (B) for the afterburner. By default Start opens the flight menu (and acts as Esc everywhere), Back the tactical map, North switches weapons, West toggles infra-red and the shoulder buttons read and repeat transmissions.

Every flight key can be rebound to another key or controller button under Options, Controls (saved in the `[controls]` section of `settings.toml`). On the port's own screens the stick or D-pad moves, South confirms and East goes back.

Host keys, available everywhere:

| Key | Action |
|---|---|
| F2 | Options (video, audio, controls, profile, accessibility, game data); the game waits meanwhile |
| F10 | Change the scale mode |
| F11 | Toggle fullscreen |
| F12 | Save a screenshot (with Enhanced features on, also the Enhanced picture at its full size) |

Developer views: `--dev-flight N` (fly flight N), `--dev-dialogue SCENE`, `--dev-mapview MAP`.

## Documentation

| Document | Purpose |
|---|---|
| [docs/player/README.md](docs/player/README.md) | Player guide: install, game data, controls, options, Classic and Enhanced, saves, troubleshooting |
| [CHANGELOG.md](CHANGELOG.md) | What each release contains, its known differences from the original and known issues |
| [docs/RELEASE.md](docs/RELEASE.md) | Version scheme, criteria for 1.0, release checklist, install tests |
| [docs/MASTER_DESIGN_GUIDE.md](docs/MASTER_DESIGN_GUIDE.md) | Vision, scope, architecture, subsystem designs, milestones, risks, open decisions |
| [docs/AGENT_DEVELOPMENT_GUIDE.md](docs/AGENT_DEVELOPMENT_GUIDE.md) | How AI agents (and humans) work on the project: roles, clean-room protocol, RE/spec/implementation/testing playbooks |
| [AGENTS.md](AGENTS.md) | Always-on rules for agents (short) |
| [docs/STATUS.md](docs/STATUS.md) | Task board for every milestone |
| [docs/re/](docs/re/) | Reverse-engineering knowledge base: file inventory, formats, executables, unknowns |
| [docs/spec/](docs/spec/) | Behaviour specifications |
| [docs/parity/](docs/parity/) | How closely each part matches the original, with the evidence |
| [docs/adr/](docs/adr/) | Architecture decision records |

## Legal

Traffic Department 2192 and its content belong to their respective rights holders. This project contains no original game data; you must supply your own copy. TD2192-Modern source code and documentation are Copyright (C) 2026 TD2192-Modern contributors and are free software: you may redistribute and modify them under the [GNU General Public License, version 3 or (at your option) any later version](LICENSE) (SPDX: `GPL-3.0-or-later`). This program comes without any warranty. Third-party components retain their own licences, listed in [THIRD_PARTY.md](THIRD_PARTY.md); release packages include their full notices.
