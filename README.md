# TD2192-Modern

A clean-room, modern reimplementation of **Traffic Department 2192** (Safari Software, 1994), the top-down hoverskid combat game set on the desert planet Seche, for current Windows, Linux and macOS.

> **Status:** planning. No code yet. Development starts at milestone M0 once the open decisions in the design guide are made.

## What this is

- A from-scratch engine that **plays your own copy** of the original game data. No original game files are included or distributed.
- Faithful by default (the **Classic** profile): original resolution and aspect, timing, controls, music and sound. The optional **Enhanced** profile adds conveniences.
- Built from written behaviour specifications produced by reverse engineering the original, not from its source code, which is not available (see [ADR-0002](docs/adr/0002-clean-room-protocol.md)).

## Documentation

| Document | Purpose |
|---|---|
| [docs/MASTER_DESIGN_GUIDE.md](docs/MASTER_DESIGN_GUIDE.md) | Vision, scope, architecture, subsystem designs, milestones, risks, open decisions |
| [docs/AGENT_DEVELOPMENT_GUIDE.md](docs/AGENT_DEVELOPMENT_GUIDE.md) | How AI agents (and humans) work on the project: roles, clean-room protocol, RE/spec/implementation/testing playbooks |
| [AGENTS.md](AGENTS.md) | Always-on rules for agents (short) |
| [docs/STATUS.md](docs/STATUS.md) | Task board for every milestone |
| [docs/re/](docs/re/) | Reverse-engineering knowledge base: file inventory, formats, executables, unknowns |
| [docs/spec/](docs/spec/) | Behaviour specifications |
| [docs/adr/](docs/adr/) | Architecture decision records |

## Requirements (planned)

- An original Traffic Department 2192 installation (v1.0 file set; 218 files).
- Rust stable toolchain. SDL3 is fetched or built by Cargo.

## Legal

Traffic Department 2192 and its content belong to their respective rights holders. This project contains no original game data, and it requires the user to supply their own copy. This project's code is licensed under the [MIT License](LICENSE).
