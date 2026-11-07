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
| `rayon` (with `rayon-core`, `crossbeam-deque`, `crossbeam-epoch`, `crossbeam-utils`, `either`) | td-render | MIT OR Apache-2.0 | Row-parallel Enhanced upscaling and lighting (ADR-0006, spec/enhanced/presentation.md §7) |
| `zip` (with `crc32fast`, `indexmap`, `hashbrown`, `equivalent`, `memchr`, `typed-path`, `bitstream-io`, `no_std_io2`, `getrandom`) | td-assets | `zip`: MIT; the others MIT OR Apache-2.0 (`memchr`: Unlicense OR MIT) | Reading the original distribution ZIP in place (M13-02); features `deflate-flate2` and `legacy-zip` only (stored, Deflate, Shrink, Reduce, Implode; no encryption) |
| `flate2` (with `miniz_oxide`, `adler2`, `simd-adler32`) | td-assets (via `zip`), png | MIT OR Apache-2.0 (`miniz_oxide`: MIT OR Zlib OR Apache-2.0; `adler2`: 0BSD OR MIT OR Apache-2.0; `simd-adler32`: MIT) | Deflate decoding, pure-Rust back end only |

Also linked into the release binaries, without a `Cargo.toml` entry:

| Component | Licence | Notes |
|---|---|---|
| Rust standard library | MIT OR Apache-2.0 (parts under other permissive licences) | Its notices (`COPYRIGHT-library.html` of the toolchain) ship in the packages as `COPYRIGHT-rust-std.html` |
| Code bundled in SDL 3's sources: hidapi, yuv2rgb, libm; on Linux, code generated from the Wayland protocol files | hidapi: BSD-style (one of the three licences it offers; not its GPL option); yuv2rgb: BSD-3-Clause; libm: Sun's permissive notice; Wayland protocols: MIT | In `THIRD_PARTY_NOTICES.txt` under `sdl3-src` |
| Microsoft C/C++ runtime (Windows package only) | Visual Studio redistributable code | Linked statically (`tools/scripts/package.ps1`), so players need no Visual C++ Redistributable |
| GNU C library, libstdc++, libgcc (Linux package only) | LGPL-2.1+ / GPL-3.0 with the GCC Runtime Library Exception | Used from the player's system, not shipped |

The packages carry the full texts in `THIRD_PARTY_NOTICES.txt`, generated for each target by `tools/scripts/gen-notices.py` (docs/RELEASE.md §3.6).

Tools used outside the build (not linked, not distributed): Ghidra (Apache-2.0), DOSBox-X (GPL-2.0, built locally as a reference emulator; **no code from it may enter this repository**).
