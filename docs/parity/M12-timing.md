# Parity: M12-02 timing audit (2026-11-06, VE-3; rows 5–8, 11, 12, 19 fixed by IM-19 the same day, §7)

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

## 7. Port fixes (IM-19, 2026-11-06, M12-13 and M3-09)

| # | Fix | Port now | Original | Test |
|---:|---|---|---|---|
| 5 | Scene fades follow the non-zero components of the faded palette (`td_render::fade::frames_per_level`, ≥ 380 → 2 frames per level, U-461); `g_Palette` kept across the program run (`Program::front_end.scene_palette`, the state IM-18 added for the OB-017 muzzle bytes); the title card fades it with entry 85 set | first title card after program start: 16 visible frames in, 16 out (1 frame per level); after story so far: 31 in, 31 out | C-0002 18 in, 20 out; C-0170 34 | `td-flow/tests/story_parity.rs` (two new tests) |
| 6–8 | `Flow_WaitSecondsOrKey(n)` counts round(n × 18.2065) BIOS ticks at 19.2015 Hz (`td_flow::timing::dos_seconds_frames`, `BIOS_TICK_HZ` = `FLIGHT_TICK_HZ`, ADR-0007) for every wait of timing §4.4; `Crt.Delay` stays real ms | 5 s 332, 10 s 664, 15 s 996, 25 s 1,661, 30 s 1,993, 45 s 2,989 frames | 5 s 328, 10 s 664 (C-0002, C-0170) | `timing::dos_time_waits_count_bios_ticks` |
| 11 | Flight menu, help and ship-library fades at `frames_per_level` (2 for flight palettes); the flight-end `FadeOut` (34 frames) in the td2192 host's teardown, followed by the wait for WORX's playing status | writes over 33 frames out and 33 in | C-0012 33 out, 34 in | `flight_screens_parity` (new test), td2192 `teardown_fades_out_in_34_frames` |
| 12 | Flight menu item ramp 11 steps per frame | 11, 11, 10 steps | C-0012 f3628..3630: 11, 11, 10 | as row 11 |
| 19 | E2/E3 menu on `MainMenu` (`MenuStyle::Later`, OB-041): fade-in, start ramp once at loop entry, ramp on cursor changes only, fade-out | E2 (34, 4, 34) frames; E3 (34, 4, 34) | C-0120 (34, 4, 34); C-0125 (34, 4, 34) | `td-flow/tests/menu_pacing.rs` |

Still open (all taken up in §8): the flight-start `FadeIn` (34 frames before the first tick) is not ported; the port draws each fade level in the last frame of its pair, which matches the menus exactly but gives the flight fade-in 33 frames against 34; between a menu fade-out and the next screen the original spends 1–2 frames more than the port (C-0120, C-0125).

## 8. Follow-ups (IM-21, 2026-11-06, M12-14)

Method: every fade's `frames.tsv` DAC index writes, cumulated per level (255 or 256 entries). The writes of a frame start and stop at positions inside it that follow from the write rates (a picture-palette level writes from the retrace, 0.92 into a frame, for 1.5 frames; level 0 for 0.2 frame; a 253-entry `SetDac` loop 0.2 frame). A fb dump labelled N shows the DAC at the start of frame N (C-0017 f490: entries 1..14 at level 16, the rest at 15, level 16 having begun at f489.92). "Shown" below means the first frame whose dump holds the level complete, counted from the frame in which the fade starts.

| # | Item | Port now | Original | Test |
|---:|---|---|---|---|
| 1 | Flight-start `FadeIn` (td2192 `Opening`: black playfield, the bar as setup drew it with `td.win`, the weapon icon and the missile column via `Hud::start`, DAC entries 1..255 (E3 0..255) from black to `g_FlightPalette` in the start mode; keys pressed during it reach tick 1) | levels shown 2, 4, …, 34 frames after the flight opens; tick 1 runs in frame 34; mode 1 (E1 4, 8; E2 3, 10): the start formula and tick 1 in frame 35 | E1 C-0002 f3439 → 3441..3473, tick 1's playfield in f3473; C-0017 f458 → 460..492; E2 C-0015 f631 → 633..665; E3 C-0036 f660 → 662..694; mode 1 C-0019 f457 → 459..491, start formula complete f492. Pixels of C-0017 f490, C-0015 f660, C-0036 f690 exact; their DACs at the port's level or the next | td2192 `opening_fades_in_before_tick_1`, `opening_frames_match_the_original` |
| 2 | Fade phase rule (`td_render::fade::level_frames`, `first_level_frames`): level 0 takes one frame; a level is shown from the frame after its last write, so the first level waits one frame more than its cost; applied to every 17-level fade (main menu, story screens, dialogue, save screens, flight menu, flight start and end, flight 17/19 screens) | fade-in 2, 4, …, 34; fade-out 3, 5, …, 33, 34; nearly black 2, 3, …, 18 | the same in C-0002 (menu in/out, title card in/out), C-0012 (Esc menu out), C-0017, C-0015, C-0036 (flight start), C-0067 and C-0069 (all four fades), C-0069 (flight end, f1809), C-0120 (menu in/out), C-0170 (story so far, title card out). The old port wrote fade-out levels 16..1 a frame early (the main menu's two frames early) and the "33 against 34 frames" of the flight menu fade-in was a counting artefact: both fades' writes span 34 frames because the first level starts at 0.92 of its first frame, C-0012 f3555 holding 14 entries of the fade-out's level 16 | td-render `level_zero_takes_one_frame_and_the_first_level_one_more`; `flight_screens_parity`, `story_parity`, `menu_pacing`, main_menu and td2192 unit tests updated |
| 3 | Main menu → next screen | the action 2 frames after the fade-out's last level (`main_menu::NEXT_SCREEN_INIT_FRAMES`: the next screen's mode X initialisation); E1 title card levels shown 38, 39, …, 54 frames after Enter | C-0002 Enter f1027: menu levels f1030..1061, mode X initialisation f1060..1062 (49, 55, 1 retrace-bit hits), title card from f1063.92, levels shown f1065..1081 (38..54 after Enter). E2 C-0120 and E3 C-0125: the same plus a song start (1 frame) and the `.scr` read (about 18 frames) before the card fades in; not modelled (U-512) | `menu_pacing` (action at black + 2), `story_parity::main_menu_boxes_and_fades` |
| 4 | Flight 17/19 screens' fades (`interlude.rs`) | `level_frames` with the flight palette (2 a level, 1 for level 0); split line off 2 frames after the opening fade-out, the picture read 14 more (cut-scene), split line on 1 frame after the closing fade-out | C-0069: fade-in from f1267.92 after the fade-out's last level at f1266 (2); C-0067: picture fade-in from f926.92 after f911 (16); closing fade-ins from f1515.92 and f1477.92 after f1515 and f1477 (1) | `interlude_parity` (unchanged tolerances) |
| 5 | U-451 `Crt.Delay` rate | real milliseconds (unchanged) | C-0067 6,000 ms in 427.9..429.7 frames (1.018..1.022 × real); C-0069 2,000 ms in 141.5..142.5 (1.009..1.017): no common factor, so not modelled; observation in timing §4.4 item 2 | — |
| 6 | U-450 keys after a real-time screen | unchanged (`held_controls`) | C-0067 tick 107 and C-0069 tick 205 agree with it but involve no byte the game cleared; no capture holds a key into a clearing screen (VE-2's comm-viewer runs release every key before F6, C-0158 holds nothing) | — |
| 7 | Found on the way: no E2 or E3 flight arena could be built (`HudAssets::new` accepted only E1's 71-entry radar table; E2's and E3's have 80), so the td2192 campaign fell back to the placeholder flight in both episodes | `HudAssets::new` takes 71 or 80 entries; the opening tests build E2 and E3 arenas | — | td2192 `opening_*` |

Still open: U-510 (an SP line for the level-0 cost and the phase), U-511 (tick 1 is drawn into the page on screen, so the original shows its playfield unscrolled in the fade's last frame and the next, C-0002 f3473..3474; the port draws it scrolled), U-512 (the mode X initialisation of the other screens, picture reads, the flight menu's page read of 7 frames, C-0012 f3589..3594). The first tick after the fade comes one flight period after tick 1 here; in the original it waits for the next BIOS tick boundary after setup's DOS-time sample, 0..1 period later (C-0002 and C-0017: tick 2's image 6 frames after tick 1's playfield, against 3.65 here; T2).
