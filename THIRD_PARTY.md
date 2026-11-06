# Third-party components

Every direct dependency, with its licence. All must be compatible with the project's MIT licence (MDG §5.4). Update this file in the same change that adds or removes a dependency.

| Component | Used by | Licence | Notes |
|---|---|---|---|
| SDL 3 (built from source via `sdl3` / `sdl3-sys`) | td-platform | Zlib | Statically linked |
| `sdl3` crate | td-platform | MIT | |
| ymfm (via `ymfm-sys`) | td-opl | BSD-3-Clause | OPL2 emulation (ADR-0005); C++ built with `cxx` |
| `ymfm-sys` crate | td-opl | BSD-3-Clause | |
| `thiserror` | libraries | MIT OR Apache-2.0 | |
| `anyhow` | binaries | MIT OR Apache-2.0 | |
| `clap` | binaries | MIT OR Apache-2.0 | |
| `tracing`, `tracing-subscriber` | several | MIT | |
| `serde` | several | MIT OR Apache-2.0 | |
| `toml` | settings | MIT OR Apache-2.0 | |
| `sha2` | assets, tests | MIT OR Apache-2.0 | |
| `png` | tdtool | MIT OR Apache-2.0 | Tool output only (never committed) |
| `hound` | tdtool | Apache-2.0 | Tool output only (never committed) |
| `proptest` | tests | MIT OR Apache-2.0 | |
| `rayon` | td-render | MIT OR Apache-2.0 | Row-parallel enhanced presentation stages (ADR-0006, spec/enhanced/presentation.md §7) |

Tools used outside the build (not linked, not distributed): Ghidra (Apache-2.0), DOSBox-X (GPL-2.0, built locally as a reference emulator; **no code from it may enter this repository**).
