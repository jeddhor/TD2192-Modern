# TD2192-Modern

A clean-room, modern reimplementation of **Traffic Department 2192** (Safari Software, 1994), the top-down hoverskid combat game set on the desert planet Seche, for current Windows, Linux and macOS.

> **Status:** in development, not yet a release. Episode 1 starts from script choice through the main menu, the story scenes and dialogue, into flights with the original flight model, weapons and mission ships. Each of these is checked against recordings of the original game (see [docs/parity/](docs/parity/)). Still missing: mission objectives (so flights cannot be won yet), the HUD instruments, the in-flight menus, saving, and Episodes 2 and 3. Progress is tracked in [docs/STATUS.md](docs/STATUS.md).

## What this is

- A from-scratch engine that **plays your own copy** of the original game data. No original game files are included or distributed.
- Faithful by default (the **Classic** profile): original resolution and aspect, timing, controls, music and sound. The optional **Enhanced** profile adds conveniences.
- Built from written behaviour specifications produced by reverse engineering the original, not from its source code, which is not available (see [ADR-0002](docs/adr/0002-clean-room-protocol.md)).

## Running it

You need an original Traffic Department 2192 installation: the v1.0 file set of 218 files, including `TD1.EXE` and `FILE0001.EXE`. On first start the game asks for that folder. You can also pass it with `--data <folder>` or set the `TD2192_DATA` environment variable.

```text
td2192                     start screen: choose an episode, options, quit
td2192 --episode N         start Episode N (1, 2 or 3) directly, as the original does
td2192 --check             report whether the installation is complete
td2192 --fullscreen
```

Building from source needs the Rust toolchain pinned in `rust-toolchain.toml`. SDL 3 is built by Cargo, which needs CMake and a C/C++ compiler. Run `cargo run --release -p td2192`.

### Controls (as in the original)

| Key | In flight |
|---|---|
| Numeric keypad 8 / 2 / 4 / 6, or the arrow keys | Thrust up / down / left / right (the ship turns toward the direction) |
| Space | Fire (hold for the cannons; press once per missile) |
| Tab | Switch between cannons and missiles |
| Keypad + or keypad 0 (Ins) | Afterburner burst (Stiletto-class ships) |
| Alt | Infra-red view on/off (where available) |
| F5 / F6 | Ship switch / incoming transmissions |
| P | Pause (any key resumes) |
| Esc | Flight menu (for now: abort the flight) |

A game controller works as the original joystick. Use the left stick or D-pad to steer, South (A) to fire and East (B) for the afterburner. By default Start opens the flight menu (and acts as Esc everywhere), Back the tactical map, North switches weapons, West toggles infra-red and the shoulder buttons read and repeat transmissions.

Every flight key can be rebound to another key or controller button under Options, Controls (saved in the `[controls]` section of `settings.toml`). On the port's own screens the stick or D-pad moves, South confirms and East goes back.

Host keys, available everywhere:

| Key | Action |
|---|---|
| F2 | Options (video, audio, controls, profile, accessibility); the game waits meanwhile |
| F10 | Change the scale mode |
| F11 | Toggle fullscreen |
| F12 | Save a screenshot |

Developer views: `--dev-flight N` (fly flight N), `--dev-dialogue SCENE`, `--dev-mapview MAP`.

## Documentation

| Document | Purpose |
|---|---|
| [docs/MASTER_DESIGN_GUIDE.md](docs/MASTER_DESIGN_GUIDE.md) | Vision, scope, architecture, subsystem designs, milestones, risks, open decisions |
| [docs/AGENT_DEVELOPMENT_GUIDE.md](docs/AGENT_DEVELOPMENT_GUIDE.md) | How AI agents (and humans) work on the project: roles, clean-room protocol, RE/spec/implementation/testing playbooks |
| [AGENTS.md](AGENTS.md) | Always-on rules for agents (short) |
| [docs/STATUS.md](docs/STATUS.md) | Task board for every milestone |
| [docs/re/](docs/re/) | Reverse-engineering knowledge base: file inventory, formats, executables, unknowns |
| [docs/spec/](docs/spec/) | Behaviour specifications |
| [docs/parity/](docs/parity/) | How closely each part matches the original, with the evidence |
| [docs/adr/](docs/adr/) | Architecture decision records |

## Legal

Traffic Department 2192 and its content belong to their respective rights holders. This project contains no original game data; you must supply your own copy. This project's code is licensed under the [MIT License](LICENSE). Third-party components and their licences are listed in [THIRD_PARTY.md](THIRD_PARTY.md); release packages include their full notices.
