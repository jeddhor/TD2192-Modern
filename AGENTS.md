# AGENTS.md: always-on rules for AI agents

**TD2192-Modern** is a clean-room reimplementation of the 1994 DOS game *Traffic Department 2192*. It runs on modern platforms from the user's original data files.

**Before your first task in a session**, read [docs/AGENT_DEVELOPMENT_GUIDE.md](docs/AGENT_DEVELOPMENT_GUIDE.md) (how we work) and the relevant parts of [docs/MASTER_DESIGN_GUIDE.md](docs/MASTER_DESIGN_GUIDE.md) (what we build). Check [docs/STATUS.md](docs/STATUS.md) for claims, blockers and the current milestone.

## Paths

| What | Where |
|---|---|
| This repo | `P:\msdos\TD2192\TD2192-Modern` |
| Original game (**read-only**) | `P:\msdos\TD2192\TD2192` (set `TD2192_DATA` to this) |
| RE work area (Ghidra project, exports, DOSBox copy, captures) | `P:\msdos\TD2192\re-work` (outside the repo) |
| Ghidra 12.2 | `C:\Users\jason\Desktop\ghidra_12.2_PUBLIC` (needs `JAVA_HOME=C:\Program Files\Eclipse Adoptium\jdk-25.0.5.8-hotspot`) |
| DOSBox-X source (GPL) / our debug build | `P:\msdos\TD2192\dosbox-x` (a tool only: **never copy or paraphrase its code into the port**; preserve the clean-room boundary even though this port is now GPL-3.0-or-later) |

## Non-negotiables

1. **Never write to the original game directory.** DOSBox runs a copy.
2. **Never commit game data or anything derived from it:** files, slices, PNG/WAV renders, extracted text. Hashes, sizes, offsets and numeric facts are fine.
3. **Never commit decompiler output or disassembly, and never transliterate it into code.**
4. **One hat per task:** Archaeologist (RE), Spec author (SP), Implementer (IM), Verifier (VE). State your hat. Implementers do **not** open Ghidra, `re-work\exports`, or disassembly; they work from `docs/spec/` and file questions when a spec is insufficient. Never do RE and IM on the same subsystem in one session.
5. **Specs before code** for original behaviour. Code cites `spec path + revision`. Constants cite provenance.
6. **`td-sim` is deterministic:** no floats in Classic gameplay paths, no wall clock, no unseeded or extra RNG calls, no `HashMap` iteration, explicit 16-bit wrapping where the original wraps.
7. **Respect crate boundaries** (MDG §8.2). No upward dependencies. No `unsafe` anywhere (only `td-platform` may get a justified, reviewed exception).
8. **Green tree:** `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` pass with **and** without `TD2192_DATA`. Data-gated tests use `require_data!()`.
9. **Write knowledge down in the same change:** `docs/re/FORMATS.md`, `docs/re/UNKNOWNS.md`, `docs/re/symbols/*.csv`, `docs/spec/**`, `docs/STATUS.md`. If you stop mid-task, write a handoff in `docs/handoffs/`.
10. **Never reproduce hostile behaviour** (for example the `label.exe` disk-relabel path).
11. **Ask the user before** pushing, publishing, creating remotes, deleting non-scratch files, accepting ADRs, adding copyleft dependencies, or setting persistent environment variables. Open decisions D1–D10 (MDG §14) are the user's to make.
12. **Report honestly:** failing tests, skipped steps and confidence levels, stated plainly.

## Task loop

Claim in STATUS → read spec/docs/handoff → plan → small green commits → verify (look at or listen to outputs, compare with captures) → update docs → STATUS `DONE` or handoff. Definition of Done: ADG §6.7.

## Quick facts

- Original: Borland Pascal 7 + WORX Toolkit 2.1, real-mode MZ, unpacked, no overlays. Episodes: `file0001.exe` (E1), `td2.exe`, `td3.exe`; intro `file0000.exe`; launcher `td1.exe`.
- Screens: 768-byte 6-bit palette + 320×200. Tiles: 80 × 32×32. Maps: 201×201 (intro 20×2000). Sprites: 20 × 32×32 + 40×40 rotation sets. Missions: 20,080 bytes. Music: CMF/OPL2. SFX: VOC, mostly Creative ADPCM.
- DOS 8.3 names matter (`helihangar.scr` → `helihang.scr`). Text is lowercase, and `"` stands in for the apostrophe.
- Address notation: `<exe>:<image-relative seg>:<off>` (ADG §7.8).
