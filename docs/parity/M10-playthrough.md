# Parity: M10 Episodes 2 and 3, whole flights to success (2026-11-06)

Verifier: VE-4 (M10-45; the VE part of M10-04..M10-43). Implementation under test: `crates/td-sim` at the branch head (E2: [e2-mission-setup.md](../spec/sim/e2-mission-setup.md), [e2-mission-logic.md](../spec/sim/e2-mission-logic.md), [e2-mission-ships.md](../spec/sim/e2-mission-ships.md) rev 1; E3: [e3-mission-setup.md](../spec/sim/e3-mission-setup.md), [e3-mission-logic.md](../spec/sim/e3-mission-logic.md), [e3-mission-ships.md](../spec/sim/e3-mission-ships.md) rev 1; the shared E1 specs and [episode-deltas.md](../re/episode-deltas.md) rev 3). The method is VE-2's E1 method ([M9-playthrough.md](M9-playthrough.md)); this report gives only what is new for E2 and E3.

## Direct starts

Every capture starts the episode's program directly in a private rig copy (`re-work\ve4\rigs\a..c`, reset to the install before each run; preset P1, cycles 20000, `joysticktype=none`), so the flight is the first of its program run (`Session::for_episode`). Scenario lines are frame-anchored on file opens (ORACLE.md §5); the flight input is tick-keyed (§5a).

**E2 (`TD2.EXE /sb`)** ([e2-program-flow.md](../spec/flow/e2-program-flow.md) §8):

- **Flight 1, New game** (C-0015's keys): `2` 140 frames after `td-let.cin` opens; Enter 180 frames after `tdmen.pal` opens; Esc 90 frames after `marauder.scr` opens. Tick 1 at frame 664.
- **Flight N = 2..20, restore**: slot 1 of `TD2.SAV` := `NN 00 00` (bytes 0..2: mission, kills to date), run with `-KeepState`. `2` at `td-let.cin`+140; Down at `tdmen.pal`+180 and Enter at +200 (menu item 2, restore); Enter at `td2.sav`+90 (slot 1); Enter at the second `tdmen.pal` open +180 (item 3, next mission); then Esc 90 frames after the block's **first scene card** opens. E2 cards do not flush the keyboard, and the Esc after a tested pre-flight step jumps to the flight call (§4.3, §4.4). The first scene card per block (§4.4, the first `bg[...]` step, after the title card where there is one):

  | Blocks | First scene card |
  |---|---|
  | 1, 16, 19 | `marauder.scr` |
  | 2 | `velsroom.scr` |
  | 3, 10 | `shooting.scr` |
  | 4, 8, 15 | `tdlab.scr` |
  | 5, 7 | `bunker.scr` |
  | 6 | `sewer.scr` |
  | 9, 13, 18, 20 | `tdoffic2.scr` |
  | 11, 12, 14 | `sbar.scr` |
  | 17 | `hyde.scr` |

  Tick 1 comes at frame ~1041 for blocks 3, 5 and 6 (no title card) and ~1441 for the others (the title card runs its 5 s). Checked for all 20 flights by a short run up to tick 2 (`re-work\ve4\prerun.py`): the mission file `mN.td2` opens and the ticks count.
- **Projectile arrays.** E2 and E3 allocate the projectile arrays on the heap at a place that depends on the path (music and scene sizes): E2 flight 1 `656C:0000`, flights 2–20 one of `6290`, `62BD`, `62CE`, `649B`, `656C`; E3 one of `5E9E`, `5ECB`, `5FFA` (every block that starts with its title card), `617A`. The pre-run reads the eight far pointers (E2 DS:3896.., E3 DS:3892..) at tick 1, and the flight run dumps 0x240 bytes from that paragraph; the test takes the segment from the dump's file name.

**E3 (`TD3.EXE /sb`)** ([e3-program-flow.md](../spec/flow/e3-program-flow.md) §8):

- **Flight 1, New game** (C-0036's keys): `2` at `td-let.cin`+140; Enter at `tdmen.pal`+180; Esc at `shuttle0.scr`+121. Tick 1 at frame 693.
- **Flight N, restore**: slot 1 of `TD3.SAV` := `NN 00 00`, the same menu and restore keys as E2, then Esc at the block's first screen: **121** frames after a scene card's `.scr` opens (E3 cards flush the keyboard when their wait starts, after the fade-in), or **90** frames after the title card's `td-let.cin` opens (its second open). First screens: block 1 `shuttle0.scr`; 2 `command.scr`; 3 and 5 `convers.scr`; 6 and 8 `maraud3.scr`; 12, 15 and 19 `kolorvci.scr`; blocks 4, 9, 10, 11, 13, 14, 16, 17 and 18 the title card. Tick 1 at frame ~1035–1069. Checked for flights 1–6 and 8–19 by the pre-run.
- **Flights 7 and 21** have no block of their own (OB-051): F7 follows F6 in block 6 and F21 follows F11 in block 11 (§4.4.1, §4.4.2). They need the previous flight flown to success first; not captured here.

## Planning (`tdtool plan N --episode E`)

The E1 beam search, with the episode's setup (`Session::for_episode`, map set, tile attributes, mission file) and goals:

- **Base tests** as goal view tiles: E2 (102, 104) for flights 1–4 and 7, x 178..183 × y 35 (5), x 118..123 × y 70 (6), view X 7 (8), (87, 28) (9–19), view Y ≤ 7 (20); E3 (167, 66) (1–5), view X 6 × rows 101..114 (6), (17, 164) (7–10, 12, 14–17, 19, 21), (160..161, 75..76) (11), (36..37, 44..45) (13), (23, 31) (18) (e2-mission-logic §4.4 and §5, e3-mission-logic §4.3 and §5). The flights whose base test has no phase condition (E2 6, 7; E3 11, 13, 14, 21) steer home from the start.
- **Comm viewer**: F6 is offered when the episode's transmission table has an entry for the phase.
- **U-160.** Every plan is flown in two worlds: the 342-byte muzzle table and the DGROUP bytes of the capture's pre-run (from the table's start, E2/E3 DS:04BA). A **strict** plan keeps only candidates that behave the same in both. When every branch fails (often an enemy's two-digit volley the player cannot prevent), the search backs off 2..32 decisions and avoids the abandoned nodes. If the strict search gives up, a **dual** plan (`--dual`) keeps diverging candidates as long as both worlds stay alive, open the comm viewer in the same ticks and succeed on the same tick. The L4 replay test then flies the 342-byte world and the oracle comparison the capture's world, two courses with the same inputs and the same end; the table below says which flights are dual.

## Captures and comparison

One oracle run per flight (`re-work\ve4\run_plan.py`; oracle build 2, VE-2's private build copied to `re-work\ve4\bin\oracle-2`): per-tick packed dumps of DGROUP (E2 `3EDA:0000`, E3 `3AE9:0000`, 0x9800 bytes) and of the projectile arrays, a stall record 150 ms after the last change, DGROUP 120 and 240 frames after the last planned tick, a framebuffer every 70 frames. The tick counter `g_MissionTicks` is E2 DS:163C and E3 DS:1638.

`crates/td-sim/tests/oracle_e2_full.rs` and `oracle_e3_full.rs` (L5) use the shared harness `tests/full_flight/mod.rs` (`oracle_e1_full.rs` now uses it too, with the same 19 passes). They compare exactly what the E1 test compares (M9-playthrough, Test), at the E2/E3 offsets of `td2.csv` / `td3.csv`:

| Field | E1 | E2 | E3 |
|---|---|---|---|
| `g_RandSeed` | 04EE | 064C | 064C |
| `g_DrawPage` | 3A30 | 3958 | 3954 |
| phase, comm pending, succeeded | 32BA, 329C, 329E | 31E2, 31C4, 31C6 | 31DE, 31C0, 31C2 |
| kills, lost, outcome | 12CA, 12CC, 12CF | 11EE, 11F0, 11F3 | 11EA, 11EC, 11EF |
| ticker, comm indicator, hit flash | 32BC, 1705, 12C4 | 31E4, 162A, 11E4 | 31E0, 1626, 11E4 |
| armour/shields, speeds, view | 398E, 3995, 3A36 | 38B6, 38BD, 395E | 38B2, 38B9, 395A |
| heading, turn, target heading | 1A56, 1A5A, 1CBB | 197E, 1982, 1BE3 | 197A, 197E, 1BDF |
| weapon/missiles/cooldown, over/end ticks | 1CB8, 32B7 | 1BE0, 31DF | 1BDC, 31DB |
| key table, markers (home = +20) | 1A5C, 3958 | 1984, 3880 | 1980, 387C |
| slot arrays (class), records | 3A7A, 4432 | 39A2, 435A | 399E, 4356 |
| projectile pointers, muzzle table | 396E, 035C | 3896, 04BA | 3892, 04BA |

`replays_e2.rs` and `replays_e3.rs` (L4, `TD2192_DATA`) check every checkpoint and the success on the last tick.

## Results

All comparisons are exact (T1): every tick of the flight, the last one included (stall record), plus setup and the outcome after teardown (outcome, succeeded, lost and `g_PlayerDied`). Slot-ticks count the slots live in the port after each compared tick. "Plan" says whether the two muzzle worlds took the same course (strict) or not (dual, see Planning). The phase events are the original's, from the per-tick records.

**Episode 2: 20 of 20 flights match on every tick** (15,700 ticks, 96,377 slot-ticks).

| Flight | Capture | Plan | Ticks | Slot-ticks | Kills | Outcome | Comm viewer | Result | Phase events in the original (tick) |
|---:|---|---|---:|---:|---:|---:|---|---|---|
| 1 | C-0120 | dual | 1,079 | 9,876 | 11 | 1 | 824 | **PASS** | t821 phase 2, t824 phase 3 (viewer), t891 phase 100 |
| 2 | C-0121 | dual | 1,367 | 8,756 | 29 | 1 | — | **PASS** | t1232 phase 100 |
| 3 | C-0122 | strict | 1,719 | 3,892 | 12 | 1 | — | **PASS** | t1718 phase 100 |
| 4 | C-0123 | strict | 336 | 2,931 | 5 | 1 | — | **PASS** | t224 phase 100 |
| 5 | C-0124 | strict | 631 | 4,128 | 8 | 1 | 162 | **PASS** | t157 phase 2, t162 phase 3, t369 phase 100 |
| 6 | C-0130 | strict | 209 | 2,433 | 0 | 1 | — | **PASS** | no phase script; base strip reached at t209 |
| 7 | C-0131 | strict | 113 | 1,356 | 0 | 1 | — | **PASS** | base test without phase condition: home at t113 |
| 8 | C-0132 | strict | 1,864 | 7,127 | 7 | 1 | — | **PASS** | t495 phase 2, t1000 phase 3, t1861 phase 100 (view X 7) |
| 9 | C-0133 | strict | 432 | 4,070 | 2 | 1 | — | **PASS** | t251 phase 100 (map set 2 from here) |
| 10 | C-0134 | strict | 1,340 | 8,888 | 4 | 1 | — | **PASS** | t157 phase 18, t560 phase 2, t981 phase 100 |
| 11 | C-0137 | strict | 1,525 | 8,264 | 3 | 1 | 8 | **PASS** | t8 phase 2, t9 phase 3, t246 phase 4, t644 phase 5, t1128 phase 100 |
| 12 | C-0138 | strict | 877 | 7,775 | 0 | 1 | — | **PASS** | t876 phase 100 |
| 13 | C-0139 | strict | 301 | 3,021 | 2 | 1 | — | **PASS** | t157 phase 100 |
| 14 | C-0140 | strict | 1,106 | 4,214 | 12 | 1 | — | **PASS** | t807 phase 100 |
| 15 | C-0141 | strict | 145 | 960 | 3 | 1 | — | **PASS** | t92 phase 100 |
| 16 | C-0142 | strict | 299 | 1,978 | 2 | 1 | — | **PASS** | t186 phase 100 |
| 17 | C-0143 | strict | 614 | 6,328 | 0 | 1 | — | **PASS** | t613 phase 100 (not lost) |
| 18 | C-0144 | strict | 992 | 4,832 | 10 | 1 | — | **PASS** | t477 phase 2, t832 phase 100 |
| 19 | C-0145 | strict | 534 | 3,019 | 1 | 1 | 206 | **PASS** | t199 phase 2, t206 phase 3, t351 phase 100 |
| 20 | C-0146 | strict | 217 | 2,529 | 4 | 1 | — | **PASS** | t158 phase 100; success at view Y ≤ 7 |

**Episode 3: 18 of 18 flights with a block of their own match on every tick** (15,364 ticks, 99,966 slot-ticks). Flights 7 and 21 were not flown (Direct starts).

| Flight | Capture | Plan | Ticks | Slot-ticks | Kills | Outcome | Comm viewer | Result | Phase events in the original (tick) |
|---:|---|---|---:|---:|---:|---:|---|---|---|
| 1 | C-0125 | strict | 921 | 2,591 | 16 | 1 | — | **PASS** | t372 phase 3, t497 phase 5, t662 phase 100 |
| 2 | C-0126 | strict | 405 | 4,087 | 8 | 1 | — | **PASS** | t224 phase 100 |
| 3 | C-0127 | strict | 982 | 4,901 | 15 | 1 | — | **PASS** | t362 phase 2, t810 phase 100 |
| 4 | C-0128 | strict | 665 | 5,463 | 1 | 1 | — | **PASS** | t321 phase 100 |
| 5 | C-0129 | strict | 854 | 4,950 | 4 | 1 | — | **PASS** | t853 phase 100 (slot 2 hidden at (80, 199)) |
| 6 | C-0156 | strict | 1,062 | 7,702 | 0 | 1 | — | **PASS** | t713 phase 100 (the convoy gone); base strip at view X 6 |
| 8 | C-0135 | strict | 1,312 | 12,891 | 0 | 0 | — | **PASS** | t539 phase 4, t1311 phase 100; outcome 0 (the convoy did not all reach the hangar) |
| 9 | C-0136 | strict | 1,560 | 5,911 | 4 | 0 | — | **PASS** | t1053 phase 100; outcome 0 |
| 10 | C-0159 | idle, worlds differ | 1,049 | 7,374 | 0 | 0 | — | **PASS** | t10 phase 2, t300 phase 3, t301..336 the phase counter 5..40, t1048 phase 100 (truck destroyed), success at home the next tick |
| 11 | C-0147 | strict | 852 | 10,345 | 2 | 1 | — | **PASS** | positional success, no phase test |
| 12 | C-0148 | strict | 400 | 4,152 | 16 | 1 | — | **PASS** | t215 phase 100 |
| 13 | C-0149 | strict | 336 | 2,662 | 0 | 1 | — | **PASS** | positional success, no phase test |
| 14 | C-0150 | strict | 598 | 6,885 | 1 | 1 | — | **PASS** | t27 phase 2; `home17` without phase condition |
| 15 | C-0151 | strict | 521 | 5,225 | 2 | 1 | 120 | **PASS** | t113 phase 2, t120 phase 3, t272 phase 100 |
| 16 | C-0152 | strict | 908 | 7,175 | 8 | 1 | 278 | **PASS** | the forced transmission (comm request every tick in phase 2) at t278: phase 5, t279 phase 100 |
| 17 | C-0153 | strict | 336 | 1,680 | 0 | 1 | — | **PASS** | home at t336 |
| 18 | C-0154 | strict | 2,162 | 4,928 | 9 | 1 | — | **PASS** | t2160 phase 2, t2161 phase 100; base (23, 31) |
| 19 | C-0155 | strict | 441 | 1,044 | 1 | 1 | 56 | **PASS** | t53 phase 2, t56 phase 3, t215 phase 100 |

**Episode 1, beyond VE-2's success flights** (`oracle_e1_full.rs`, `replays_e1.rs`):

| Replay | Capture | Ticks | Result | What it covers |
|---|---|---:|---|---|
| `e1f01-idle.tdr` | C-0157 | 617 | **PASS** | no input; the enemies destroy the parked ship: armour falls from tick 509, 0 at tick 583 (mission over), the end sequence to 35 end ticks, `g_PlayerDied` = 1 and outcome after teardown |
| `e1f01-modals.tdr` | C-0158 | 617 | **PASS** | the same flight with P in tick 100 (pause; closed by Enter), F5 in tick 200 (tactical map; closed by Esc) and Esc in tick 300 (flight menu; closed by Enter on "return"): the ticks stop while each screen is open, and every tick after it matches, the key tables included (the screens clear the key bytes) |

**Totals.** 38 E2/E3 flights match the original on every tick from setup to success with the same outcome (31,064 ticks, 196,343 slot-ticks), plus the E1 death and the three in-flight screens (1,234 ticks). All 40 replays pass their L4 test.

**What the passes cover** beyond the E2/E3 first ticks of C-0015, C-0016, C-0036 and C-0041:

- every mission script of E2 (flights 1–20) and of E3 flights 1–6 and 8–19 through phase 100 and the base test: the E2 base tests at (102, 104), (178..183, 35), the flight-6 strip, view X 7, (87, 28) with `viewSubY < 32` and view Y ≤ 7; the E3 base tests at (167, 66), the flight-6 strip, `home17`, (23, 31) and the positional flights 11 and 13;
- the E2 per-flight events of flights 17 and 18 (`Msn_E2FlightEvents`; not lost) and the E3 timed events (`Msn_E3TimedEvents`: flight 10's phase counter, flight 16's forced transmission);
- the E2 transmissions of flights 1, 5, 11 and 19 and the E3 transmissions of flights 15, 16 and 19;
- both E2 map sets (set 2 from flight 9) and both E3 maps (`td4.map` from flight 7), with their tile attributes;
- the outcome rules at teardown, including outcome 0 in E3 flights 8, 9 and 10;
- kills by the player, by mission ships and by explosions, exits, spawns, and enemy fire on the player in both episodes;
- E2/E3 setups of every flight (class tables, player class, armour, missiles, start view);
- E1: the player's death and the end sequence; the pause, the F5 map and the flight menu during a flight.

**Not covered:**

- E3 flights 7 and 21 (they follow flights 6 and 11 in one block; a capture would need two flights in one run, with a tick watcher that restarts);
- missiles and Tab, the afterburner (the plans never select missiles or press KP+/KP0);
- any flight not reached by a direct start or New game (Session RNG after a previous flight of the same run);
- E1 flight 21.

## Divergences

**No td-sim divergence was found.** Every flight above matches the original on every compared field and tick. What the trial runs found was in the harness and the oracle schedule:

| # | Flight | Tick | Field | Ours | Original | Events around it | Spec | Resolution |
|---|---:|---:|---|---|---|---|---|---|
| H1 | E3 8 (C-0135) | 1312 (last) | `g_HitFlash` | 8 | 0 | the flight ends by success in tick 1312; the record is the stall record 150 ms later | hit-flash §4.2 (every level of a flight fade zeroes it), flight-screens §4.5.3 (the flight fades out at its end) | Host behaviour, not td-sim: the harness zeroes the hit flash after the last tick, as the end-of-flight fade does. Likewise after the flight menu (its fades) |
| H2 | E1 1 (trial run of `e1f01-modals`) | 300 | — (the flight menu stayed open) | — | — | Esc pressed 200 frames after tick 300 began, held 3 frames | flight-screens §4.1: the menu tests the key bytes once per cursor pulse (32 retraces) | Oracle schedule: the menu is closed with Enter on "return", held 40 frames. Not registered; C-0158 is the run with the fixed schedule |
| H3 | E2 1, E2 2, E3 10, E1 idle | E2 1: 357; E2 2: 107; E3 10: 725; E1 idle: 439 | the two muzzle worlds (U-160 / U-441) | — | — | an enemy fires on a two-digit heading; its shots hit another ship with different offsets in the two worlds, whatever the player does | weapons §3.3 (E1), U-441 (E2/E3) | The L5 tests use the capture's DGROUP bytes and match the original through the end. The L4 tests fly the 342-byte table: another course with the same inputs and the same end tick (both worlds checked by `tdtool plan`/`replay`). See open question 1 |

## Open questions for the coordinator

1. **Dual replays (U-441, U-160).** `e2f01`, `e2f02`, `e3f10` (and `e1f01-idle`, `e1f01-modals` for E1) take a different course in td-sim's 342-byte world than in the capture's world, because an enemy's two-digit volley lands differently; the strict search with back-off found no input that avoids it. When IM-18's E1 muzzle bytes (weapons rev 4 §3.3) land, the E1 idle replays' checkpoints will change from tick 439 on; the new course is the one C-0157/C-0158 confirm, so `tdtool replay --rewrite-checkpoints` is the fix. The E2/E3 dual replays need the same once U-441 is mapped. The tick-1 bytes reproduced every two-digit volley of these 38 flights (evidence added to U-441).
2. **U-430 (new): E3 flight 10, the hangar.** After leaving the home hangar, the planner never found a way back to `home17` (view (17, 164), `viewSubY ≤ 35`) in td-sim within 4,000 ticks (beam 64), while it returns there in flights 8, 9, 12 and 15–19 of the same map; it stops at views (17, 165), (19, 164), (24, 165). Flight 10 draws the overlay cells (21..23, 165..166) over ships (e3-mission-logic §5.10, episode-deltas §4.4). Whether the original lets the player back in was not tested: the committed flight-10 plan never leaves (it succeeds when the truck is destroyed, outcome 0). A capture of a return would settle it.
3. **E3 flights 7 and 21** need a two-flight oracle run: the tick lines of the second flight would fire in the first unless the oracle restarts its tick-keyed lines when the watcher is re-armed (an oracle 3 feature, ORACLE.md §5a).
4. **VE-4's capture range is used up** (C-0120..C-0159). Further captures need a new range.

**Answers (RE-17, 2026-11-06):**

1. U-441: the E2/E3 bytes past the muzzle table are specified in [weapons](../spec/sim/weapons.md) rev 5 §3.3.2 and §3.3.3: constants and start-up values on 0Ch, 0Eh, 20h, 24h, the last front-end file name on 4Ah, the scene palette and two front-end loop counters on 4Eh, 60h, 62h. The construction reproduces the tick-1 bytes of all 38 captures (and C-0015, C-0036, C-0180) byte for byte, and none changes during a flight. Once td-sim builds them (M10-46), `e2f01`, `e2f02` and `e3f10` fly the capture's world in L4 too; rewrite their checkpoints then.
2. U-430: the original lets the player back into `home17` from below (C-0182, C-0183: out to view (17, 166) and back in at tick 70). At the home position sideways thrust does not move the ship (map row 166 is solid except the hangar tile (22, 166)), so re-entry needs the ship lined up under the one-tile opening. td-sim has not been compared with these captures: replay the C-0183 schedule (scenario `re17-e3f10-hangar-c.txt`, tick lines) in td-sim from the same start and compare every tick; if td-sim matches, the planner's failure is its alignment, not the collision.
3. E3 flights 7 and 21: oracle 2 cannot re-arm its watcher; a backward-compatible change is designed in `re-work\re17\ORACLE-REARM.md` (U-481), not built.

**Follow-up (IM-20, M10-46, 2026-11-06):** td-sim now builds the E2/E3 two-digit muzzle bytes (weapons rev 5 §3.3.2, §3.3.3) from the front-end state, so every test and tool flies the capture's world: the built bytes equal the tick-1 DGROUP of all 38 E2/E3 captures (and VE-2's 20 E1 captures) in every record (`oracle_front_end.rs`), and `full_flight` copies no capture bytes; all 60 L5 flights pass. Against the old 342-byte world only the three dual replays take another course (`e2f01` after tick 357, `e2f02` after tick 107, `e3f10` after tick 725, H3); 15 strict replays (`e2f03`, `e2f08`, `e2f10`, `e2f11`, `e2f13`, `e2f19`, `e3f04`, `e3f05`, `e3f06`, `e3f08`, `e3f09`, `e3f11`, `e3f14`, `e3f18`, `e3f19`) keep their course but hash the leftover offsets of freed shot slots last used on a two-digit heading. All 18 got new checkpoints with `tdtool replay --rewrite-checkpoints` (inputs unchanged; same end tick, kills and outcome), and the L4 tests fly the capture's course. Open question 1 is closed for E2 and E3.

## Planner notes

- The E2/E3 base tests with a `viewSubY` limit and the narrow E3 hangar needed a goal term in pixels near the goal (offsets (32, 16)); the view tile alone left the ship stuck one tile short.
- E3 flight 6 is an escort whose phase advances only when the convoy has left: friends count as progress when they leave, and the player waits instead of parking on the convoy's path (the first plan blocked the exit at (1, 108) and the phase never moved).
- The strict search backs off when every branch fails; it found strict plans for 35 of the 38 flights.

## Reproducing

- Pre-run (projectile segment and muzzle bytes): `python -I re-work\ve4\prerun.py <rig> <E> <N>`.
- Plan: `cargo run --release -p tdtool -- plan N --episode E --alt-muzzles re-work\ve4\muzzles\eEfN.bin [--dual]` writes `tests/replays/eEfNN.tdr`.
- Check: `cargo run --release -p tdtool -- replay tests/replays/eEfNN.tdr --verify [--alt-muzzles BYTES]`.
- Death and screens: `tdtool plan 1 --idle [--modals]` writes `e1f01-idle.tdr` / `e1f01-modals.tdr`.
- Oracle run: `python -I re-work\ve4\run_plan.py <rig a|b|c> <E> <N> [replay tag]`; `re-work\ve4\register.py` (or `reg_all.py`) registers it.
- Compare: `cargo test --release -p td-sim --test oracle_e2_full --test oracle_e3_full -- --nocapture` with `TD2192_DATA` and `TD2192_ORACLE` set.
