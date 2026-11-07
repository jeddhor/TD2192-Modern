# Parity: M12-06 replay soak, fuzzing and profiling (2026-11-06, VE-8)

Verifier: VE-8 (Verifier hat; no disassembly, decompiler output or RE exports read; no oracle run). Machine: Intel i7-10700F (8 cores, 16 threads), Windows 11, Rust 1.95.0 release builds unless stated. Other agents were building on the same machine; the CPU load before each timing run is given. Tree under test: this branch (main `7afcae6` plus the commits named below).

MDG §11 targets checked here: "Zero panics in 10 h of replay soak; parsers fuzzed for 1 h without failure", "Simulation: exactly the original tick rate", "Memory: under 100 MB RSS", and the Enhanced budget of [presentation §7](../spec/enhanced/presentation.md) (at most 6 ms per frame for the heaviest setting).

## 1. Summary

| Check | Result |
|---|---|
| Committed replays, `tdtool replay --verify`, 20 rounds × 68 replays in 16 parallel processes | **PASS**: 1,360 runs in 12.1 s, every replay prints the same result in every run (§2.1) |
| Committed replays on many threads in one process | **PASS**: 8,704 runs (68 × 16 threads × 8 rounds), every run's checkpoint timeline equal to the single-threaded one (§2.1) |
| Random-input flights (every flight of every episode, 128 seeds, up to 100,000 ticks) | **FAIL, one crasher**: 7,808 flights, 526,469,363 ticks (7,616 h of game time) twice each, deterministic across runs and threads; **E3 flight 19 panics in tick 4235 with any input** (BUG-18, major) (§2.2) |
| Arithmetic overflow in debug builds | td-sim checks overflow in every profile, so §2.2 covers it; the fuzz tests found two overflows in caller-supplied parser sizes (BUG-15, fixed); debug runs of the real binary found none (§2.3) |
| Parsers fuzzed | **PASS** after four small fixes (BUG-14..BUG-17): every `td-formats` parser, the ZIP import, the exe-resource extraction and the replay parser (§3) |
| Enhanced frame cost, heaviest setting (21:9, xbr4, full lighting, bloom) | **PASS on this machine**: median 4.2–4.5 ms, p95 5.1–5.3 ms on 16 threads; median 5.0 / p95 6.1 ms on 6 threads; **over budget on 4 threads** (6.4 / 7.2 ms) (§4) |
| Enhanced frame cost, D5 preset (16:9, xbr3, subtle) | **PASS**: median 1.2–1.3 ms, p95 1.6–1.7 ms (§4) |
| Classic frame cost | **PASS**: 0.10–0.12 ms median (compose and palette), 0.02 ms per tick (§4) |
| Tick rate in the real binary (BUG-5 re-check) | **PASS**: campaign flights 19.18–19.20 ticks/s (19.2015), front-end screens 70.08 ticks/s (70.086), each within the ±1-tick quantisation of the window (§5) |
| Memory (peak working set, real binary) | **PASS**: 67.8–72.3 MB (§5) |

## 2. Soak

### 2.1 Committed replays

- `tools/scripts/replay-soak.ps1 -Rounds 20 -Parallel 16` runs `tdtool replay <file> --verify` (release) for every `tests/replays/*.tdr`. `--verify` now ends its line with the final state hash.
  - 66 replays verify every checkpoint, and every one of their 20 runs prints the same outcome and final hash.
  - `e3f10-hangar-b.tdr` and `e3f10-hangar-c.tdr` fail `--verify` at the tick-200 checkpoint, the same way in all 20 runs. This is by design: their checkpoints were computed with the capture's muzzle bytes (RE-17's U-430 runs), and `oracle_e3_full::flight_10_hangar_return_*` verifies them that way. `tdtool replay` has no option to load those bytes for `--verify` (only `--alt-muzzles` for a comparison).
- `td-sim/tests/soak.rs::replays_agree_across_threads` (in the suite, with data) flies all 68 replays on 4 threads at once. `long_soak` flew them on 16 threads in 8 rounds: 8,704 runs, each run's state hash at every checkpoint and at the end equal to the single-threaded reference, which equals the file's checkpoints (except the two hangar runs).

### 2.2 Random-input flights

`td-sim/tests/soak.rs`: a seeded generator holds and releases keys like a player (steering and fire often; afterburner, infra-red, Tab, M, S, Esc, P, F5, F6 now and then; joystick bits and the cheat-chord letters rarely; typematic make codes while held). Each flight starts as its replays start it (the first flight of a program run; E1 21, E3 7 and E3 21 after their predecessors' success replays). Each tick is checked for a panic and the tick count; a clone stepped every 500 ticks must give the same state hash; every job runs twice on different threads, in a different order, and both runs must end identically.

`long_soak`, release, 16 threads, `TD2192_SOAK_SEEDS=128 TD2192_SOAK_TICKS=100000 TD2192_SOAK_SEED=20261106`, 2026-11-06:

| | |
|---|---|
| Flights | 61 (E1 1–21, E2 1–20, E3 1–19 and 21) × 128 seeds = 7,808 |
| Ticks | 526,469,363 per run (7,616 h of game time at 19.2015 Hz), each flight run twice: 236 s of wall time |
| Endings | 2,851 finished (496 succeeded, 392 lost, 1,963 died), 4,829 still flying at 100,000 ticks, 128 panicked |
| Determinism | both runs identical for every flight (ticks, outcome, kills, state hash); every clone check passed |
| Panics | **all 128 seeds of E3 flight 19, in tick 4235, "attempt to add with overflow" at `td-sim/src/flight/ships.rs` drift** |

**BUG-18 (major).** The panic does not depend on the input: with no input at all, slot 2 (class 0) parks at tile (102, 199) with down speed 23..24 and goal (102, 200), and its offset Y grows by about 8.5 a tick (1,023 at tick 500, 32,762 at tick 4234) until the i16 add overflows. Mission-ships §4.2.3 says "Y never wraps at row 199 (offsetY grows)" but not what happens at 32,767; the original's 16-bit ADD wraps (U-570). td-sim keeps overflow checks in release builds (MDG §8.6), so the game crashes 3 min 40 s into E3 flight 19. The success replay ends at tick 441, so no existing test reached it. Pinned by the ignored test `soak.rs::e3_flight_19_idle_outlives_tick_4235`. No other flight hit an overflow or a panic.

The default (non-ignored) soak tests are cheap: a synthetic world without data (3 sessions × 2 seeds × 1,500 ticks) and, with data, 9 flights (E1 1, 8, 21; E2 3, 10, 17; E3 5, 7, 16) × 2 seeds × 6,000 ticks; 0.3–1 s in a debug build.

### 2.3 Overflow in debug builds

- td-sim has `overflow-checks = true` in release (workspace `Cargo.toml`), so §2.2 is also the overflow check for the simulation. The cheap soak tests run in debug in the suite.
- The fuzz tests run in debug in the suite: they found the parser overflows of BUG-15 (caller-supplied sizes), now fixed.
- The real binary in a debug build (`target/debug/td2192`, `SDL_VIDEODRIVER=dummy`, `SDL_AUDIODRIVER=dummy`, `--dev-keys`), 150 s each: Episode 1 (script choice, menu, block 1 skipped, flight 1 to its death, end sequence, debrief, the retry and a second flight of 107 s firing all the time), Episode 2 and Episode 3 front ends (menus, story, about screens; these two runs did not reach a flight). No panic.

## 3. Fuzzing

Stable toolchain, so no `cargo-fuzz`: `proptest` (dev-dependency only) plus seeded mutation loops. Files: `crates/td-formats/tests/fuzz.rs`, `crates/td-assets/tests/fuzz.rs`, `crates/td-sim/tests/fuzz_replay.rs`.

- **Inputs:** random bytes (to 64 KiB, and every format's exact size ± 2); synthetic inputs of the right shape with 0–4 mutations; with `TD2192_DATA`, every original file of the format, unmodified, truncated (every length up to 4 KiB, else both ends and a stride) and with seeded mutations (bit flips, byte sets, inserts, deletes, count fields set to 0, FFh, FFFFh, the length). Accessors that index into a parsed result are walked after every successful parse.
- **Hangs:** a per-input time limit (5 s; 10 s for td-assets) in every test, and a watchdog thread in the long runs.
- **Long runs:** `long_fuzz` in each file (ignored), driven by `TD2192_FUZZ_SECS` and `TD2192_FUZZ_SEED`.

| Parser | Covered | Original files |
|---|---|---|
| palette (`parse`, `parse_prefix`) | random, synthetic, real | 2 |
| screen (`Screen`, `PagedScreens`) | random, synthetic, real | 46 + `td.hlp` |
| tileset (`td.blk`, E2/E3 tail bytes) | random, synthetic, real | 3 |
| map (`parse_episode`, `parse_any`, `get`) | random, synthetic, real | 5 |
| mission (`parse`, `path`, steps, `to_bytes` round trip) | random, synthetic, real | 60 |
| MZ header, `file_offset`, `pstring`, `pstring_seq` | random, synthetic, real | 11 executables |
| `ByteReader` (every method, invariants) | proptest | — |
| CMF (header, instruments, events) | random, synthetic, real | 17 |
| VOC (blocks, rates) | random, synthetic, real | 17 |
| dialogue (strict parse, `read_scene` for ids 0..=255) | random, synthetic, real | 6 |
| fonts (`td.lt2`, `td-let.cin`, third kind; every Latin-1 char) | random, synthetic, real | 3 |
| stills and cell sheets (10 still layouts, 13 cell layouts, caller layouts) | random, synthetic, real | 23 |
| portraits (`by_face_byte` 0..=255) | random, synthetic, real | 3 |
| saves (`td*.sav`, `td*.stt`, round trips) | random, synthetic, real | 7 |
| sprites (`parse_base`, `parse_rotation_set`, any frame size, `frame(i)`) | random, synthetic, real | 7 |
| ZIP import (`archive::read_zip_from`: stored, Deflate, Shrink, Reduce, Implode, unsupported methods, sizes past the limits) | random, writer-made and hand-built ZIPs, truncated and mutated; 10 original files zipped in memory | — |
| exe resources (`ExeResources::load`), `validate`, `dos83`, manifest lookups | random and mutated executables, folders and archives, arbitrary names | 4 executables |
| replays (`Replay::from_bytes`, `verify`) | random, any version and counts, truncations, the 68 committed replays mutated | — |

**Findings** (all fixed in one `fix:` commit, each with a unit test; none is reachable from an original file):

| Bug | Where | What |
|---|---|---|
| BUG-14 | `picture::parse_cells` | a layout with an empty cell panicked (`chunks_exact(0)`) |
| BUG-15 | `Picture::parse`, `parse_cells`, `SpriteSheet::parse` | caller-supplied sizes multiplied unchecked: overflow panic in debug, wrap in release |
| BUG-16 | `mz::pstring_seq` | reserved the caller's count up front: "capacity overflow" for `usize::MAX` |
| BUG-17 | `Replay::from_bytes` | reserved up to 256 MiB from a 24-byte header before noticing it was truncated |

Also noted, not filed: `Cmf::parse` reserves up to 65,535 instrument slots (704 KiB) from the header before reading them; `dos83("abcdefg.xyz.")` gives `abcdefg.` while `dos83("abcdefg.")` gives `abcdefg`, so a ZIP or Linux folder entry whose name ends in a dot is listed but cannot be read by that name (no game file has such a name). `exe_res` extraction runs only on executables whose SHA-256 matches, so mutated executables reach only the version check; its pstring readers are fuzzed directly.

**Runs:**
- Default tests, debug: td-formats 0.8 s without data and 11.9 s with; td-assets 0.4 / 1.0 s; td-sim 0.6 / 0.5 s.
- Long runs, release, with data (helper run before the fixes, 100 s each): 1,018,080 td-formats inputs; about 1.02 million archives, 3.27 million names and 3,193 exe loads; 13,287,088 replay inputs. No failure (one test-side expectation error in the td-assets mutation check was fixed and the seed re-run).
- Long runs after the fixes: see §3.1.

### 3.1 One-hour run

LONG_FUZZ_RESULTS

## 4. Frame cost (CPU)

`td2192::flight::lights::tests::frame_cost` (ignored; release; data): real flights flown with fire held and turns now and then, every frame from tick 40 to tick 440, each frame composed and presented in four settings. Times per frame: the composed indexed frame (`draw_frame`), the light scene, and the pipeline (palette to RGBA, upscale, lighting, bloom). The Classic path is the 320-wide frame plus its palette-to-RGBA conversion (the CPU side of `present`). The GPU's window scaling is not included. Flights E1 1 and 13 (19 lights at most); "busy" is E2 13, E3 15 and E1 7. `RAYON_NUM_THREADS` limits the upscaler and lighting threads.

| Setting | Threads | Load before | Median ms | p95 ms | Max ms |
|---|---:|---:|---:|---:|---:|
| Classic 320 | 16 | 9–15 % | 0.106 / 0.106 / 0.119 | 0.15–0.16 | 0.25 |
| Preset: 16:9, xbr3, subtle | 16 | 9–15 % | 1.226 / 1.202 / 1.270 | 1.64–1.67 | 3.1–3.5 |
| 16:9, xbr3, subtle, bloom | 16 | 9–15 % | 1.930 / 1.915 / 2.015 | 2.45–2.50 | 3.2–3.8 |
| **Heaviest: 21:9, xbr4, full, bloom** | 16 | 9–15 % | **4.218 / 4.209 / 4.502** | **5.11–5.28** | 6.9–8.1 |
| Heaviest, busy flights | 16 | 26 % | 4.381 | 5.22 | 9.4 |
| Preset | 6 | 35 % | 1.513 | 1.92 | 3.3 |
| Heaviest | 6 | 35 % | **5.009** | **6.12** | 7.7 |
| Preset | 4 | 3 % | 1.881 | 2.39 | 3.8 |
| Heaviest | 4 | 3 % | **6.387** | **7.25** | 12.2 |

- Of the heaviest frame (16 threads), compose takes 0.08 ms, the light scene 0.003 ms and the pipeline 4.1–4.4 ms. The synthetic busy night of `td-render/tests/enhanced_lighting_timing.rs` (49 lights) takes 3.55 ms for lighting and bloom, plus 1.28 ms for xbr4 with the palette (`enhanced_timing.rs`): 4.8 ms. Both of those tests pass their budgets.
- One tick (td-sim and the host's flight screen) takes 0.012–0.018 ms (median), at most 0.08 ms.
- **Against the 6 ms budget:** the heaviest setting fits at the median and the 95th percentile on this 8-core machine; on 6 threads the median fits but the 95th percentile is 2 % over; on 4 threads (a 2018-era 4-core laptop) it is over budget at 6.4 ms. The maxima (7–12 ms) are single frames during other agents' builds. The D5 preset (1.2–1.9 ms) and Classic (0.1 ms) are far inside both the 6 ms budget and MDG §11's "< 2 ms CPU per frame".
- Recommendation (IM, not filed as a bug): bloom costs about 0.7 ms at 3× and about 2 ms at 4×. IM-13's suggestion of fusing the bright pass into the light map, or a half-resolution bloom at 4×, would bring the heaviest setting under 6 ms on 6 threads. The player guide's advice for slower computers (smaller upscaler, lighting off) is right.

## 5. Tick rate and memory in the real binary

`td2192 --dev-stats` logs once a second (target `td2192::stats`) the rate the run loop uses, the ticks and frames of that second and every rate change; `--dev-keys` presses keys at given ticks so a headless run (`SDL_VIDEODRIVER=dummy`, `SDL_AUDIODRIVER=dummy`) reaches a campaign flight through the real program (Episode 1: `1` at the script choice, Enter for a new game, Esc through block 1's story; E2/E3: `1`, Enter, Space every 20 ticks). Rates below are from the log timestamps over each stretch at one rate, leaving out its first three seconds (the screen change itself costs up to a frame, and loading can stall a frame; the loop clamps a frame to 250 ms and does not catch up, ADR-0004).

| Run | Screen | Rate set | Ticks | Seconds | Measured | Deviation |
|---|---|---:|---:|---:|---:|---:|
| E1, release, Classic | script choice | 70.086 | 1,999 | 28.52 | 70.080 | −0.01 % |
| E1, release, Classic | **campaign flight 1** | 19.2015 | 547 | 28.48 | **19.203** | +0.01 % |
| E1, release, Classic | end sequence, debrief | 70.086 | 997 | 14.23 | 70.080 | −0.01 % |
| E1, release, D5 preset | campaign flight 1 | 19.2015 | 521 | 27.19 | 19.160 | −0.22 % (1 tick) |
| E1, debug, Classic | campaign flight 1 (second try) | 19.2015 | 1,980 | 103.22 | 19.183 | −0.10 % |
| E2, release, Classic | campaign flight 1 | 19.2015 | 468 | 24.40 | 19.182 | −0.10 % |
| E3, release, Classic | front end | 70.086 | 5,065 | 72.28 | 70.076 | −0.01 % |
| E3, release, Classic | campaign flight 1 | 19.2015 | 156 | 8.16 | 19.124 | −0.40 % (1 tick) |

- **BUG-5 re-check: PASS.** Campaign flights of all three episodes run at the flight rate and front-end screens at the retrace rate; every deviation is within one tick of the measuring window plus a frame. The rate switches at the tick the flight opens and back when it ends ("tick rate now 19.2015 Hz at tick 352", "… 70.0860 Hz at tick 968").
- One E2 stall: about 1.9 s in one frame shortly before the first E2 flight opened (32 ticks in 2.15 s, then the flight); E1 and E3 showed none. The clamp drops that time (no catch-up), so the rate is unaffected, but it is a visible hitch. Not located; worth a profile (IM).
- Frame time under the dummy driver (SDL's software renderer, no GPU): about 29 ms per frame for Classic and 11–12 ms for the Enhanced preset. It measures SDL's software scaling, not the game.
- **Memory:** peak working set 70.5 MB (E1 Classic), 67.8 MB (E1 Classic, longer), 71.3 MB (E1 D5 preset), 69.7–72.3 MB (debug E1–E3), 67.9–68.0 MB (E2/E3 release). MDG §11: under 100 MB. **PASS.**

## 6. Reproducing

```text
cargo build --release -p tdtool -p td2192
pwsh tools/scripts/replay-soak.ps1 -Rounds 20 -Parallel 16 -Data $env:TD2192_DATA
TD2192_SOAK_SEEDS=128 TD2192_SOAK_TICKS=100000 TD2192_SOAK_SEED=20261106 cargo test -p td-sim --release --test soak long_soak -- --ignored --nocapture
TD2192_FUZZ_SECS=3600 cargo test -p td-formats --release --test fuzz long_fuzz -- --ignored --nocapture
cargo test --release -p td2192 frame_cost -- --ignored --nocapture        # RAYON_NUM_THREADS=6 for a 6-core budget
cargo test --release -p td-render --test enhanced_timing --test enhanced_lighting_timing -- --ignored --nocapture
SDL_VIDEODRIVER=dummy SDL_AUDIODRIVER=dummy target/release/td2192 --episode 1 --no-intro --dev-stats --dev-keys "2000=1,2200=enter,2300-2340/20=escape"
```

All need `TD2192_DATA`. The headless run reads the player's `settings.toml` (it writes it only when it is missing) and must be stopped by hand.
