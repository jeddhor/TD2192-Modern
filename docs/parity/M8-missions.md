# Parity: M8/M9 Episode 1 mission logic, flights 2–20 (2026-11-05, in progress)

Verifier: VE-1 (M8-11, M9-21). Implementation under test: `crates/td-sim/src/flight/` at main 934b05d (`logic.rs` = [mission-logic.md](../spec/sim/mission-logic.md) rev 1, `setup.rs` = [mission-setup.md](../spec/sim/mission-setup.md) rev 1, `ships.rs` = [mission-ships.md](../spec/sim/mission-ships.md) rev 2). Flight 1 is covered by `oracle_ships.rs` (C-0009) and `oracle_mission.rs` (C-0010).

## Method

**Captures.** One oracle run per flight (C-0017..C-0035, `firstticks`), plus four runs with F6 (C-0037..C-0040, `comm`), made with the RE-8 direct-start method in a private rig copy (`re-work\ve1\dosbox`, preset P1, cycles 20000):

- the game copy is reset to the install, then `tdstatus.dat` is written as status 1, kills 0, mission N, menu cursor 3, joystick calibration 0, script 1 (14 bytes). `FILE0001.EXE` truncates the file at start, so it is rewritten before every run. `td.cfg` stays the shipped (2, 2, 1);
- `FILE0001.EXE` is started with `-KeepState`: Enter at the main menu (`open:tdmen.pal+210`), Esc at the mission card (`open:td-let.cin+60`). `mN.td` opens at frame 423 and tick 1 starts at frame 489..493;
- `firstticks`: no flight input. `comm` (flights 2, 6, 9, 10): F6 for 6 frames some ticks after the transmission is announced, Space presses for the prompts (transmission 1: 2 prompts; transmission 3 needed more than 4, so its run presses Space 12 times), then F6 three times, 100 frames apart; the first F6 closes the screen and the later ones are consumed by the viewer in a phase with no transmission. While the screen is open the dumps are every 10th frame (no tick runs);
- the whole DGROUP and the projectile heap arrays (5C4Eh:0000, 240h bytes) are dumped every frame from f420 to f1900, and a framebuffer every 35 frames. DGROUP is segment **4467h** when the program is started directly: PSP 0814h, image base 0824h, DGROUP image-relative 3C43h. C-0009's 4FEBh is the same DGROUP under `td1.exe`.

The runner and the registration helper are `re-work\ve1\run_flight.py` and `re-work\ve1\register.py`. Each capture's scenario is in its `scenario.txt` (`# ve1-e1f<N>-firstticks`).

**Test.** `crates/td-sim/tests/oracle_e1_flights.rs`, one test per flight. It needs `TD2192_DATA` and `TD2192_ORACLE` and finds each capture by its scenario's first line. It reads the roughly 3,000 dumps per flight in parallel and takes about 20 s per flight on an idle drive.

- **Session.** A direct start is the first flight of a program run. In every capture the seed after setup is exactly 40 `Random` steps from seed 0 (20 at program start, 20 at flight start; mission-ships §4.1), so the session is `Session::new()` and the test adds no steps. The test asserts the 40. Class tables, ship records, AI player tile and mission globals are the program-start ones.
- **Setup** is compared with the last frame before tick 1. In these runs the fade-in ends in that frame, and the first frame of tick 1 can already show tick 1's ship movement. So the setup frame is taken before the setup `AI_ChooseWaypoint` pass (mission-ships §4.1 step 6). The record bytes that pass writes (pathCursor, prev, return, moveDir, nearFlag, target) are left out there; tick 1 checks them. Compared: seed, flight number, player class, palette mode, phase, ticker, home marker, the 20 markers, every slot's arrays (class, tile, offsets, headings, turn, four speeds, goal) and its whole 1,065-byte record. The shields and armour of mission-file class FFh slots are left out here and tested on their own (D1).
- **Each tick t.** The state after tick t is the last frame whose tick counter is t (mission-ships §6). When that frame was taken before tick t's mission logic and ships had run, the state is in the first frame of tick t + 1 instead. That happens for a tick shorter than a frame, seen only around tick 1 and counted as `late_frames`. Compared:
  - phase, comm pending, succeeded, lost, kills, home marker, the 20 markers, ticker position and comm indicator;
  - arrays and record of every slot live on either side;
  - the player group: armour, shields, view tile and offsets, heading, mission over, end ticks. It may match either frame on its own, because tick-order steps 26–33 run after the tick limiter.

  The capture's last tick has no next frame and is not compared. The run stops at the first divergent tick.
- The ticker **message** is drawn into VGA memory, not DGROUP, so it is not compared directly. Each post shows as a ticker restart at the tick of the phase step that posts it. Message ids follow from the phase step.
- **Input.** Tick t gets the original's key bytes (DS:1A5C + scan code) as the first frame of tick t + 1 holds them, as down and made (the `oracle_flight.rs` rule). In the first-ticks captures they are all 0. When a tick returns `Modal::CommViewer`, the test resumes it with no keys; no tick runs in the original while the screen is open.

## Results

All comparisons are exact (T1). "Events" are the changes in the original over the compared ticks: phase, comm pending, ticker restarts (posts), slot class changes (spawn, respawn, removal or destruction). Kills and the end flags did not change in any run.

| # | Flight | Capture | Setup | Ticks | Slot-ticks | Result | Events in the original (tick) |
|---|---:|---|---|---|---:|---|---|
| 1 | 2 | C-0017 | PASS (D1 excluded) | 1–386 | 1,930 | **PASS** | t157 comm pending 1 + post (transmission announced, `M1[5]`, rule A); t313 re-post (rule A repeats while the phase is 1) |
| 2 | 3 | C-0018 | PASS | 1–387 | 2,807 | **PASS** | t157 phase 2 + post + slot 5 class 8 (rule A at the first idle ticker); t255 slot 2 removed |
| 3 | 4 | C-0019 | PASS | 1–386 | 3,088 | **PASS** | t157 phase 2 + post (rule A) |
| 4 | 5 | C-0020 | PASS | 1–387 | 2,709 | **PASS** | t1 slot 2 removed |
| 5 | 6 | C-0021 | PASS | 1–387 | 3,870 | **PASS** | t352 phase 3 + comm pending 1 + post (rule A: slot 3 within 40 of the view tile) |
| 6 | 7 | C-0022 | PASS | 1–387 | 3,608 | **PASS** | mines laid: slot 2 class 3 at t1, slot 3 at t65, slot 4 at t231, slot 5 at t356 |
| 7 | 8 | C-0023 | PASS | 1–386 | 4,114 | **PASS** | t157 phase 2 + post; slots 16, 15, 1, 6 removed at t337, t345, t352, t382 |
| 8 | 9 | C-0024 | PASS | 1–387 | 6,330 | **PASS** | t1 slot 13 removed; t157 phase 2 + comm pending 1 + post; slots 15, 1, 14 removed at t194, t351, t369 |
| 9 | 10 | C-0025 | PASS | 1–387 | 4,257 | **PASS** | t1 slot 7 removed; t157 phase 2 + comm pending 1 + post |
| 10 | 11 | C-0026 | PASS | 1–387 | 5,418 | **PASS** | t1 slots 8 and 9 respawn as class 7 (`Msn_UpdateEvents`; answers U-183) |
| 11 | 12 | C-0027 | PASS | 1–387 | 4,569 | **PASS** | t1 slot 13 removed; slots 5, 4, 2 removed at t345, t358, t385 |
| 12 | 13 | C-0028 | PASS | 1–387 | 3,870 | **PASS** | none (start view (177, 153)) |
| 13 | 14 | C-0029 | PASS | 1–387 | 4,257 | **PASS** | none |
| 14 | 15 | C-0030 | PASS | 1–387 | 2,377 | **PASS** | slots 2, 1, 16 removed at t266, t279, t287 |
| 15 | 16 | C-0031 | PASS | 1–387 | 2,709 | **PASS** | none |
| 16 | 17 | C-0032 | PASS | 1–387 | 774 | **PASS** | none |
| 17 | 18 | C-0033 | PASS | 1–387 | 3,256 | **PASS** | slots 7, 6, 1 removed at t204, t362, t371 |
| 18 | 19 | C-0034 | PASS | 1–387 | 3,870 | **PASS** | none (phase stays 1, so the flight-19 pin is not exercised) |
| 19 | 20 | C-0035 | PASS | 1–387 | 2,709 | **PASS** | none (slots 1–5 sink every tick; no objective, no ticker) |
| 20 | 2 + F6 | C-0037 | PASS (D1 excluded) | 1–472 | 2,360 | **PASS** | as C-0017 to t157; F6 seen at t168; t170 comm viewer shows transmission 1, phase 1 → 2, comm pending 0; t199 and t226 F6 requests consumed with no transmission |
| 21 | 6 + F6 | C-0040 | PASS | 1–666 | 6,660 | **PASS** | as C-0021 to t352; t364 transmission 3, phase 3 → 4, comm pending 0; later F6 requests consumed |
| 22 | 9 + F6 | C-0038 | PASS | 1–471 | 7,506 | **PASS** | as C-0024 to t157; t170 transmission 5, phase 2 → 3, comm pending 0; removals as C-0024 |
| 23 | 10 + F6 | C-0039 | PASS | 1–471 | 5,181 | **PASS** | as C-0025 to t157; t170 transmission 6, phase 2 → 3, comm pending 0 |

What the passes cover, per flight:

- **Setup.** Player class, start view, palette mode (flights 4, 8 infra-red), late draw (flights 12, 15: view (102, 100)), slot classes, markers (32h, 39h, home 3Dh; flight 19's home marker 32h), aggression, armour, shields, path copies, hand bits (the 20 `Random(2)` draws of the flight), class-table patches through the ships' first moves (flights 12, 15, 20).
- **Ticks.** AI waypoints, attack orders from the engagement rules, detours, movement, removal at exits and map edges, enemy fire near the parked player (player armour and shields compared every tick; the player's armour never changed in these runs), the ticker running out at tick 156 and the first idle-ticker phase steps, the comm indicator.
- **Comm viewer** (C-0037..C-0040): the F6 request (set at step 29 while the key byte is 1), the viewer at the next back-page-0 tick, the (flight, phase) table of mission-logic §4.6 for transmissions 1, 3, 5 and 6, the phase advance and comm pending := 0, requests in a phase with no transmission, and 100–300 ticks of mission logic after the screen.

Not covered yet: later phases (they need the player to fly, or targets to leave or die, after the captured ticks), teardown outcomes, deaths and end sequences, transmissions 2, 4, 7, 8, 9, flight 21.

## Divergences

| # | Flight | Tick | Field | Ours | Original | Hypothesis | Test | Fix task |
|---|---:|---:|---|---|---|---|---|---|
| D1 | 2 (all flights with mission-file class FFh slots) | setup | record shields (+3F8h) and armour (+3FAh) of slots whose **mission-file class is FFh** (flight 2: slots 8, 10–13, 15–17) | 0 / 0 | 34 / 68 | U-143: setup step 3 reads `k_ClassShields[FFh]` = DS:03D1 and 2 × `k_ClassArmour[FFh]` = 2 × DS:03C7. Both bytes are 22h in every capture (initialised data inside the muzzle-table region). `ClassTables` returns 0 past the table. It matters only when mission logic spawns such a slot: ours would be removed at once (armour < 1) | `class_ffh_slots_setup_shields_and_armour` (ignored at the time; un-ignored and passing since M8-12, VE-6 2026-11-06) | M8-12 (BUG-1), fixed |

No other divergence was found in setup or in the compared ticks of flights 2–20 (19 first-ticks runs, 4 comm runs).

## Sampling notes for later Verifiers

- A directly started flight's first tick can be shorter than a frame. The last frame with counter 1 was then taken before tick 1's step 8. In C-0018, C-0021, C-0024, C-0027, C-0030, C-0033, C-0038 and C-0040, tick 1 needed the first frame of tick 2. The tick that opens the comm viewer and the one after it need it too (the last frame of that tick was taken while the screen was open).
- The last frame of the capture can be mid-tick: compare up to the second-to-last tick.
- The DGROUP bytes past the muzzle table that ships read on two-digit headings (U-160) are taken from each capture's own setup frame.
