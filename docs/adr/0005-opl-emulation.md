# ADR-0005: OPL2 emulation via ymfm

- **Status:** Accepted (D2 = MIT; ymfm is BSD-3-Clause, compatible)
- **Date:** 2026-11-02

## Context

All 17 music tracks are Creative Music Files that drive a Yamaha YM3812 (OPL2). Faithful music needs an accurate OPL2 emulator. Candidates:

| Core | Licence | Notes |
|---|---|---|
| **ymfm** (Aaron Giles) | BSD-3-Clause | Modern C++, accurate, covers the OPL family |
| Nuked-OPL3 | LGPL-2.1 | Very accurate (OPL3; OPL2-compatible mode); LGPL complicates static linking |
| DOSBox DBOPL / woody-opl | GPL | Licence-incompatible unless the project is GPL |
| Write our own | Ours | Large effort; accuracy risk |

## Decision

Use **ymfm** through the existing `ymfm-sys` crate (BSD-3-Clause; `cxx` bridge with a safe API that exposes the YM3812), wrapped by our small `td-opl` crate (`Opl2::write_reg`, `generate`, `reset`). No vendoring and no `unsafe` of our own. Licences are recorded in `THIRD_PARTY.md`. If `ymfm-sys` is ever abandoned, vendor ymfm behind the same `td-opl` API.

## Consequences

- Accurate FM synthesis under a permissive licence.
- One C++ build dependency (MSVC/clang on Windows, system compilers elsewhere).
- The rest of the audio stack (CMF sequencer, VOC/ADPCM, mixer) remains pure Rust.

## Alternatives considered

The others in the table above. If D2 picks GPL, DOSBox's cores become options, but ymfm remains the recommended default.
