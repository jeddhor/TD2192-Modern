# Parity: M12-02 timing audit (2026-11-06, VE-3)

Timing of every kind of screen in the original (oracle captures, preset P1 unless stated) against the port (static reading of `crates/` at this branch, plus the L5 tests that already check timing). Frames are 70.086 Hz retraces ("f"). Tiers per the specs: flight steps per tick T1, wall-clock rates, fades, holds and pacing T2 (a difference of more than about one BIOS tick, 3.65 f, or one fade level is a failure).

## 1. Method

- **Flight ticks:** `g_MissionTicks` (DS:1718) and the BIOS data area dumped every frame, so nothing slows the game the way breakpoints would. The tick rate is ticks over the emulated time between the first and the last change; frame quantisation gives about ±0.02 Hz over 30 s. Captures: C-0074 (9 runs: 3 sound configurations × cycles 3,000 to 60,000), C-0072, C-0075 (850 s), C-0009, C-0010 (378 s, counter every 35 frames).
- **Fades and holds:** runs of DAC writes per frame in `frames.tsv` (a full fade of 17 levels writes about 13,000 3C9h values). A hold is the gap between the end of a fade-in and the start of the fade-out. Captures: C-0002 (front end, story, dialogue), C-0012 (flight screens), C-0047 / C-0081 (intro).
- **Port values:** constants and the code that drives them. File references are to this branch.

## 2. Flight tick rate (M1-06 M1–M7)

| Configuration | 3,000 cycles | 8,000 | 20,000 | 60,000 | Expected (timing §4.2) | IRQ 0 |
|---|---:|---:|---:|---:|---:|---:|
| SB + music (`td.cfg` as shipped) | 13.35 | 19.21 | 19.21 (19.208 over 378.5 s, C-0010) | 19.23 | 19.2015 | 96.01 Hz |
| SB, music off (`td.cfg` 01 02 02) | 13.39 | | 18.52 (C-0072) | 18.53 | 18.521 | 500.28 Hz |
| `/nosound` | 13.90 | | 18.21 | 18.22 | 18.2065 | 18.21 Hz |

Ticks per second; E1 flight 1, no input, 617 ticks per run (the ship is destroyed and the counter stops at 617 in every configuration: the simulation is the same per tick).

- At 8,000 cycles and above, one game tick happens per BIOS tick, and the BIOS tick count (`0040:006C`) runs at the same rate. The game tick starts in the frame of the BIOS tick in 74–85 % of ticks and otherwise one frame later, after the retrace wait.
- At 3,000 cycles the game is CPU-bound (4–9 frames per tick) and never catches up.
- Steady flight has one retrace wait per tick (93 % of ticks show exactly one retrace-hit frame; the rest are the end-of-flight and destruction sequences with 20–60 hits). So there is one flip per tick, and the display rate equals the tick rate.
- dynamic-observations §6's "update every 3.4 frames" is the gap between retrace-hit frames, not the tick. The tick is 3.65 frames (19.2 Hz); the 178 image changes in 700 frames of C-0002 are ticks where something visible moved.
- `g_MissionTicks` increases by exactly 1 per tick in every frame-by-frame run (5,553 ticks). In C-0075 the parked, invulnerable flight 1 ended at tick 14,766, before the 15,000-tick rule (U-403).
- Keyboard (M7, U-024): in flight the game's INT 09h handler does not chain ordinary keys to the BIOS. `q`, `z`, `x` and keypad 8 leave the BIOS buffer head and tail unchanged.

## 3. Proposal: canonical Classic tick rate (decision needed)

The handoff expected about 19.20 Hz. **Proposal: Classic runs flight at 1,193,182 / (12,428 × 5) = 19.2015 Hz** (52.079 ms per tick):

- It is the rate of the default and most common configuration (Sound Blaster with FM music, which also plays in every flight). It is measured at 19.208 ± 0.005 Hz over 6.3 minutes in C-0010, and 19.21–19.23 Hz from 8,000 to 60,000 cycles.
- The port's `FLIGHT_TICK_HZ = 19.202` is within 0.003 % of it. Keep it, or write 19.2015 with this derivation, and remove ASSUMPTION(U-001, U-032) once the ADR is accepted.
- Alternatives:
  - 18.521 Hz (SB, no music) is the speed of players who turned the music off.
  - 18.2065 Hz (`/nosound`) is the speed without a sound card.
  - Both are 3.5 % and 5.2 % slower. A port option "match the original's speed without music" would be T3 and is not recommended.
- The same rate drives the DOS clock that every timed wait reads (§4 rows 6–8). While a song plays, DOS time runs 19.2015 / 18.2065 = **1.0547×** fast, and the intro already models this.

An SP revision of timing.md and an ADR for the user should record the decision. This report does not change the constant.

## 4. Screens

| # | Screen | Original | Port | Δ | Tier | Result | Fix needed |
|---:|---|---|---|---|---|---|---|
| 1 | Flight tick, `--flight` dev mode (`FlightArena`) | 19.21 Hz (§2) | 19.202 Hz (`td_flow::FLIGHT_TICK_HZ`, `td2192/src/flight.rs:444`) | −0.04 % | T2 | **PASS** | none (§3) |
| 2 | Flight tick inside an episode (campaign child) | 19.21 Hz | **70.086 Hz**: `Episode1` / `LaterEpisode` do not override `Screen::tick_hz`, so `Flow::tick_hz` reports `RETRACE_HZ`, and `campaign.rs:923` ticks the flight child once per episode tick | **+265 %** | T2 | **FAIL** (BUG-5) | The episode screens report their flight child's rate: `tick_hz()` returns `FLIGHT_TICK_HZ` while the child is a flight not showing a modal screen, as `FlightArena::tick_hz` does. Everything tick-based in flight (IR notice, the 35 end ticks, ticker, typematic repeat) scales with it. Code reading only; confirm with a runtime tick count after the fix |
| 3 | Flight presentation | one image per tick after a retrace wait | one present per loop, no interpolation (`run.rs:180`), no vsync request (`td-platform` `into_canvas()`) | — | T2 | PASS (rate); vsync open | Present cadence follows the tick; consider requesting vsync (M4) |
| 4 | Front-end scene fades (main menu, hangar, dialogue) | 33–34 f: 17 levels, 2 f per level (C-0002: menu in 835..868, out 1028..1060; hangar in 1449..1482, out 2147..2179; dialogue 2186..2219) | 17 levels × 2 f = 34 f (`dialogue.rs:48`, `main_menu.rs`, `story.rs`) | 0–1 f | T2 | **PASS** | none |
| 5 | Mission title card fades (`td-let.cin` text card) | **18 f in, 20 f out** (C-0002 1063..1080, 1409..1428: 17 levels at about 1 f per level) | 34 f (the scene fade) | +16 f | T2 | **FAIL** (BUG-6) | A one-level-per-frame fade for the text cards; spec screens/text needs the measured rate (SP), then IM |
| 6 | Timed card hold, 5 s (mission title) | **328 f** (C-0002 1081..1408, `menu.cmf` playing: 5 s of DOS time / 1.0547 = 332 f ± 4) | 350 f (`story.rs:502`, `seconds(5)`) | +22 f (+6.7 %) | T2 | **FAIL** (BUG-7) | `Flow_WaitSecondsOrKey`-style waits count DOS time, so scale the port's seconds by 18.2065 / f_BIOS: 0.9482 while a song plays (FM music on), 0.9830 with WORX and no song, 1 with no sound card. Not a one-line constant (depends on the sound configuration); needs a spec line in screens/story (SP) |
| 7 | Scene hold, 10 s (hangar) | **664 f** (C-0002 1483..2146, `hangar.cmf`: 10 s / 1.0547 = 664.6 f) | 701 f (`story.rs:412`) | +37 f (+5.6 %) | T2 | **FAIL** (BUG-7) | as row 6 |
| 8 | Other timed story waits (15, 25, 30, 45 s; debrief 10 s; pilot dossier; orders 5/10 s; E2/E3 cards, aboutus/order) | DOS time, assumed 1.0547× fast with music (rows 6–7) | real seconds × 70.086 (`story.rs`, `program.rs`, `scenes_e2.rs`, `scenes_e3.rs`, `extras.rs`) | +5.5 % each | T2 | **FAIL** by inference (BUG-7) | as row 6; the delays in ms (`Crt.Delay`) are CPU-calibrated busy loops, not DOS time, and are right as real milliseconds |
| 9 | Dialogue typing | 1 character per frame (110 in 110 f, C-0002 2463..2573) | 1 f per character (`small_text.rs:36`) | 0 | T2 | **PASS** | none (`dialogue_parity.rs`) |
| 10 | E1 main-menu cursor pulse | 11 levels per frame (U-302, C-0002 f868..872) | 11 per frame, 5 f per sweep (`main_menu.rs:50`) | 0 | T2 | **PASS** | none |
| 11 | Flight Esc-menu fades | **33–34 f** (C-0012: 3556..3588 out, 3595..3628 in, and 3976/4015, 4175/4212 pairs) | 17 f, 1 level per retrace (`flight_screens/menu.rs:329-341`) | −17 f | T2 | **FAIL** (BUG-8) | 2 f per level as the front-end fade; flight-screens §4.1 needs the measured rate (SP) |
| 12 | Flight menu pulse, key poll | pulse not measured; poll `Crt.Delay(100)` | 1 step per retrace; 7 f per poll (`menu.rs:33-39`) | — | T2 | not measured | measure the pulse in C-0012's fb dumps (next VE) |
| 13 | Comm screen (F6) | 1 character per retrace (flight-screens §4.2, static) | 1 per retrace (`comm.rs:40`) | — | T2 | not measured | a frame-cadence check on C-0037..C-0040 fb dumps (next VE) |
| 14 | Tactical map (F5), pause, help, retry, hint | wait for a key | wait for a key | 0 | T1 | PASS | none |
| 15 | Intro: date card, cards, holds 132/366/334 f, full fades 65 f, colour fades 7 f | C-0047 | the same constants (`intro/mod.rs:62-70`) | 0 | T2 | **PASS** | none (`intro_parity.rs` v3, v5, v7; `intro/tests.rs`) |
| 16 | Intro flyover steps | 1,618 steps at the 19.2 Hz BIOS tick, 3 or 4 f each, span 5,899 f (C-0047) | modelled BIOS clock 19.2015 Hz (`flyover.rs:291-356`) | worst 3 f, span ±3 | T2 | **PASS** | none |
| 17 | Intro effects | onsets at the step + 1.1..1.6 ms (intro §8 Q2) | effect issued in the step's frame | < 1 f + device latency | T2 | PASS (timing); schedule per intro rev 3 is IM-17's | none here |
| 18 | Credits mode B (4 s pages, 65 f fades) | no original (T3 design) | 280 f, 65 f | — | T3 | n/a | none |
| 19 | E2/E3 main menu | fades and cursor pulse as E1 (E2 spec §4.2) | **no fades, no pulse** (`program_e2.rs:20`) | — | T2 | **FAIL** by spec, not measured | implement per spec (IM); measure on an E2 capture |
| 20 | Flight 17 cut-scene, flight 19 V.C.I. screen (`Crt.Delay` 1000/2000 ms) | per spec | not shown: the host never reads `mission.scene` | — | T2 | **FAIL** (missing; BUG-3, VE-2) | IM task |
| 21 | Rate switch between screens | — | a new `FixedStep` on each change drops the accumulator remainder (`run.rs:167-172`) | < 1 tick per switch | T2 | PASS | none |

Summary: **10 PASS** (row 3 with a vsync note, row 17 timing only), **8 FAIL** (rows 2, 5–8, 11, 19, 20), 2 not measured, 1 n/a. The one large deviation is row 2: real-campaign flights run 3.65× fast.

## 5. Changes in this task

- **No timing code changed.** None of the failures is a one-line constant backed by a spec:
  - row 2 is a missing delegation;
  - rows 5, 6 and 11 need measured spec values first;
  - `FLIGHT_TICK_HZ` waits for the §3 decision.
- Spec evidence added:
  - timing.md §6 vectors 3–6 and §8 M1–M7 results;
  - U-001 and U-032 (resolved by measurement);
  - U-024 (keyboard chaining);
  - U-403.
- Bugs filed in STATUS: BUG-5 (episode flight tick), BUG-6 (title-card fade), BUG-7 (DOS-time waits), BUG-8 (flight-menu fades).

## 6. Open

- U-403: what ends parked flight 1 at tick 14,766? The 15,000-tick rule (`Msn_UpdateEvents`) has not been observed.
- Rows 12 and 13 (flight-menu pulse, comm typing) need fb-cadence checks.
- Row 19 needs an E2 or E3 front-end capture.
- `td_flow::DEFAULT_TICK_HZ` (70 Hz placeholder) is exported but unused, and STATUS M4-02 still calls the flight rate a placeholder. Remove both with the §3 decision.
