# Parity: M9 Episode 1 playthrough, whole flights to success (2026-11-06)

Verifier: VE-2 (M9-21, the success replays of M9-01..M9-17, the replay check of M6-02). Implementation under test: `crates/td-sim` at the branch head (`flight/logic.rs` = [mission-logic.md](../spec/sim/mission-logic.md) rev 2, `setup.rs` = [mission-setup.md](../spec/sim/mission-setup.md) rev 1, `ships.rs` = [mission-ships.md](../spec/sim/mission-ships.md) rev 2, `shots.rs` = [weapons.md](../spec/sim/weapons.md) rev 3, tick order = [tick-order.md](../spec/sim/tick-order.md) rev 1). VE-1's report for the first ~387 ticks is [M8-missions.md](M8-missions.md).

## Method

**Plans.** `tdtool plan N` searches td-sim for inputs that complete flight N with success (`crates/tdtool/src/plan/`). It is a beam search: every 3 ticks each of the 24 kept flights is cloned once per action (one of the 8 keypad directions, nothing, or an F6 tap with every key released when a transmission waits) and run for 3 ticks. Space is held by a rule whenever a ship other than a friend (marker 39h) is first on a clear line of fire along the current heading. Clones are scored by mission phase, the number and hit points of live targets (marker 32h), the grid distance to the nearest cell with a line of fire on a target (or to the base once the phase is 100), a bonus while a target is in the line of fire, and the damage the player took. A clone whose player is destroyed or whose mission is lost is dropped. Only keypad keys, Space and F6 are used; the afterburner keys are never pressed. The flight is started as the first of a program run (`Session::new()`), with the 342-byte muzzle table of the executable (the world the game uses).

**Replays.** The plan is written as `tests/replays/e1fNN.tdr` (TDR v2, our data): one `TickControls` word per tick, a state hash every 100 ticks. `tdtool replay FILE --verify` re-runs it and checks every hash; `crates/td-sim/tests/replays_e1.rs` (L4, `TD2192_DATA`) does the same and checks that the flight ends with success on the last tick and not earlier. Replay semantics ([`td_sim::replay::step`](../../crates/td-sim/src/replay.rs)): a modal screen the tick opens is closed at once with no input.

**Oracle input: the tick rule.** The DOSBox-X oracle build 2 (VE-2 private build, `re-work\ve2\bin\oracle-2`; `re-work\dosbox\ORACLE.md` §5a) watches `g_MissionTicks` (DS:1718) at every emulated millisecond. `tdtool replay FILE --oracle-schedule` turns the replay into `tick:T down|up <key>` lines. The exact rule:

- The game increments the counter at step 1 of tick T, right after tick T − 1's input steps 23–33, which follow a vertical-retrace wait (tick-order rev 1 §4).
- The oracle sees the new value within 1 ms (20,000 cycles at preset P1) and injects the transitions of `tick:T` in that millisecond; each scan-code byte reaches the INT 09h handler about 0.3 ms later. The game is then in steps 2–4 (drawing the 66-tile window takes several ms).
- Tick T's input steps run after the tick wait and a retrace wait, at least ~12 ms later, so they see every transition of `tick:T`; tick T − 1's had already run. So port tick T gets: keys down = the keys down after `tick:T`, keys made = the keys that went down at `tick:T`.
- Exception: the afterburner keys are also read at step 5, early in the tick. VE-5 refined the rule for them below (**Step-5 keys**); VE-2's plans never pressed them.
- Typematic repeat (DOSBox-X repeats the last key pressed after 500 ms, then every 33 ms) sets a byte the game has cleared, which a replay word cannot express. The plans hold the keys the game clears (afterburner, Tab, Alt, F6) for at most 5 ticks and never select missiles, so Space is never cleared.
- **Comm viewer.** While the transmission screen is open the counter stays at the tick T that opened it. The schedule presses Space and F6 alternately every 15 frames from 20 frames after T with `intick:T+N` lines, which run only while the counter still shows T: Space answers each prompt, the first F6 after the last prompt closes the screen, and the presses that come later are skipped. No key reaches a flight tick, as in the replay semantics.

**Step-5 keys: the afterburner (VE-5).** Keypad + and Ins (`kpplus`, `kp0`) are read at step 5 (flight-model §4.7): with class 2 or 5 and the mission not over, a set byte starts a burst when none runs and the heading is principal, and is cleared in every case (one burst per make code). Step 5 follows step 4, the 66-tile window, so the refined rule is:

- A `tick:T down kpplus` reaches the key table before step 5 of tick T: the oracle sees the new counter value within 1 ms of step 1 and injects at once, the scan code (one byte, 4Eh or 52h) reaches INT 09h about 0.3 ms later, and steps 2–4 (the per-tick bytes, the turn step and 66 tile blits) take longer than that at preset P1. Port tick T therefore gets the make in its input word, and td-sim's step 5 consumes it in tick T, as the original does.
- The game clears the byte when it reads it, whether or not a burst starts, so holding the key does not start another burst: only a new make does, or a typematic repeat (500 ms, then every 33 ms, for the last key pressed only), which a replay word cannot express. Plans therefore tap keypad + for one tick (it is in `CLEARABLE`, at most 5 ticks) and never hold it.
- A key-up of keypad + changes nothing the game reads after step 5 of the tick that consumed the make.
- Classes other than 2 and 5 never clear the byte: there it simply follows make and break like any key (no plan presses it there).

Checked by C-0205 (E1 flight 8, three bursts): every burst starts in the port's tick, and the burst frame `g_BurstFrame` (E1 DS:1836, E2 DS:175A, E3 DS:1756; the burst heading while a burst runs) is compared after every tick in every whole-flight test since VE-5 (all earlier passes unchanged: 0 throughout).

**Missiles and Tab (VE-5).** With the missile weapon selected a launch clears the Space byte (weapons §4.1), so a plan that presses Tab treats Space as clearable too (held at most 5 ticks; `schedule::check`); `tdtool plan --weapons` taps Space once per missile (never two ticks running) and releases it for a tick after 5 ticks of volley fire. Tab is a one-tick tap with every other key released.

The rule was checked by the comparison itself: the test compares the port's key table after every tick with the original's key bytes (the record of tick t + 1 is taken before tick t + 1's transitions), and every tick of the flights below matches.

**Captures.** One oracle run per flight with VE-1's direct start (`re-work\ve2\run_plan.py`, private rig `re-work\ve2\dosbox`, preset P1, cycles 20000): the game copy is reset to the install, `tdstatus.dat` = status 1, kills 0, mission N, menu cursor 3, script 1; `FILE0001.EXE` with `-KeepState`; Enter at the menu, Esc at the mission card; the watcher is armed one frame after `mN.td` opens. Per-tick packed dumps of DGROUP (4467:0000, 0x9DF0) and of the projectile heap arrays (5C4E:0000, 0x240), a stall record 150 ms after the last change, a DGROUP dump 240 frames after the last planned tick (teardown and outcome), a framebuffer every 70 frames.

**Test.** `crates/td-sim/tests/oracle_e1_full.rs` (L5, `TD2192_DATA` + `TD2192_ORACLE`), one test per flight; it finds the capture by its scenario's first line (`# ve2-e1fN-plan`) and skips flights without a replay or a capture.

- **Sampling.** The record of value t + 1 is taken within 1 ms after the counter became t + 1: the state after tick t, plus tick t + 1's counter, per-tick flags and turn step (the heading may be one turn step ahead). The last tick is compared with the stall record. No frame has to be guessed, no record is torn, and no field needs a second frame.
- **Setup** is compared with the record of tick 1 (the setup `AI_ChooseWaypoint` pass has run, so the whole record is compared).
- **Each tick:** the `Random` seed, draw page, phase, comm pending, succeeded, lost, kills, home marker, the 20 markers, ticker, comm indicator, hit flash; the player's view, four speeds, heading (or its next turn step), target heading, turn, armour, shields, weapon, missiles, cooldown, mission over, end ticks; the key bytes of all 20 flight keys; arrays and the whole 1,065-byte record of every slot live on either side; the 51 projectile slots.
- **End:** the original's last tick is the replay's last tick; outcome (DS:12CF), succeeded and lost after teardown equal the port's.
- **World.** The muzzle bytes past the table (U-160) are taken from the capture's tick-1 record, as VE-1 did. *Since IM-18 (2026-11-06):* the test builds the port's own direct-start bytes (weapons rev 4 §3.3, `td_sim::flight::front_end`, OB-017 preserved), first checking them against the capture's tick-1 record (all 272 bytes and the table equal in all 20 captures); `tdtool plan`/`replay` and the L4 replays use the same world. The replays use the 342-byte table of the executable (what the game uses); every replay was checked to take exactly the same course with the capture's bytes (`tdtool replay FILE --alt-muzzles BYTES`: no difference other than the two-digit shots themselves), so the L4 replay and the oracle comparison test the same flight.
- **Host emulation.** *Since IM-18 none: td-sim zeroes `g_HitFlash` when it reports the cut-scene or the V.C.I. screen (both fade, hit-flash rev 4 §4.2), and td2192 shows both screens (BUG-3 fixed).* Before: `g_HitFlash` is presentation state owned by the host. The test zeroes it in the tick that requests the flight-17 cut-scene (its fades zero it, hit-flash rev 2 §4.2; td2192 does not show the cut-scene yet, BUG-3). It is not zeroed after the comm viewer: the transmission screen does not fade (C-0058 tick 318: 10 after the screen), and td2192 zeroes it only after the Esc menu, which is right. hit-flash rev 2 §4.2 names the comm viewer among the faded screens; that sentence is wrong (open question 3 below).
- **Known divergence copied:** none since IM-18 (BUG-2 fixed: the session carries `g_Weapon`, 0 at program start). Before: BUG-2 (flight 19: `g_Weapon` 0 in the original after setup, 1 in td-sim); the test copied the original's value at setup.

**Planner robustness (U-160).** The first plans held Space while the ship turned, and the volley then left on a two-digit heading, which reads its muzzle offsets past the table. The port's (0, 0) policy and the original's bytes then differ. In flights 8, 15 and 16 the original was still flying when the policy's flight had already succeeded (trial runs, not registered). In flights 2, 3 and 6 the original's volley killed a target earlier, but the flight ended on the same tick. With the capture's bytes, td-sim matched the original on every tick of those runs. The planner now fires only on the heading the next turn step leaves, and `--alt-muzzles` keeps only candidates that behave the same in both worlds. All 20 replays were made or checked that way.

## Results

All comparisons are exact (T1). "Compared" counts the ticks after which the whole state listed above was compared. For a PASS it is every tick of the flight, the last one included (stall record), plus setup and the outcome after teardown. Slot-ticks count the slots live in the port after each compared tick. The events are the original's, taken from the per-tick records.

| Flight | Capture | Replay ticks | Compared | Slot-ticks | Kills | Outcome | Comm viewer (tick: transmission) | Result | Events in the original (tick) |
|---:|---|---:|---:|---:|---:|---:|---|---|---|
| 1 | C-0051 | 917 | 1–917 | 9,742 | 11 | 1 | — | **PASS** | t93 phase 2 (trucks 1, 2 dead), t123 phase 3, t347 phase 4, then phase 5 and 100; t917 succeeded |
| 2 | C-0052 | 1,587 | 1–1,587 | 5,541 | 7 | 1 | 162: 1 | **PASS** | t157 comm pending, t162 phase 2 (viewer), t986 phase 3 (slot 2 left), t1159 phase 4, t1422 phase 100 |
| 3 | C-0053 | 1,651 | 1–1,651 | 8,499 | 5 | 1 | — | **PASS** | t157 phase 2; t795 Montgomery arrives (slot 4 removed at (107, 106)); t951, t1107, t1390 phases 3–5; t1545 phase 100 |
| 4 | C-0054 | 1,215 | 1–1,215 | 8,963 | 5 | 1 | — | **PASS** | infra-red; t323 phase 3 and t324 phase 4 (B one tick after C); t909 phase 5; t1132 phase 100 |
| 5 | C-0055 | 1,123 | 1–1,123 | 5,088 | 4 | 1 | 468: 2 | **PASS** | t465 phase 2 (Demeter at (83, 174)); t468 viewer; t912 phase 4 (Demeter left at (199, 122)); t1067 phase 100 |
| 6 | C-0056 | 858 | 1–858 | 6,707 | 5 | 1 | 164: 3 | **PASS** | t157 phase 3, t164 viewer, t327 phase 5, t719 phase 100 |
| 7 | C-0057 | 837 | 1–837 | 3,861 | 7 | 1 | — | **PASS** | mines laid (slots 2, 3 become class 3); all 12 marked slots gone at t648 → phase 100; t775 respawn of slot 14 |
| 8 | C-0058 | 397 | 1–397 | 2,785 | 10 | 1 | 318: 4 | **PASS** | infra-red; t157 phase 3; t313 phase 4 (player near slot 8); t318 viewer → phase 5, with the hit flash (10) kept across it; then phase 100 |
| 9 | C-0059 | 1,758 | 1–1,758 | 15,457 | 10 | 1 | 162: 5 | **PASS** | t157 phase 2; t162 viewer; the four radar trucks and six guards dead → phase 100 |
| 10 | C-0060 | 662 | 1–662 | 5,161 | 4 | 1 | 162: 6 | **PASS** | t162 viewer; t369 phase 100 (slots 7–12 dead or gone) |
| 11 | C-0061 | 651 | 1–651 | 7,802 | 4 | 1 | — | **PASS** | t1 slots 8, 9 respawn as class 7 (U-183); t317 phase 2 (Dispater spawns); t473 phase 100 |
| 12 | C-0062 | 697 | 1–697 | 7,348 | 0 | 0 | — | **PASS** | helicopter (late draw, no map collision); the convoy is destroyed by its attackers; t696 phase 100; base (102, 99..100) |
| 13 | C-0063 | 974 | 1–974 | 5,305 | 8 | 1 | — | **PASS** | t458 phase 2; t663 phase 100; base at the depot (175..177, 152..154) |
| 14 | C-0064 | 301 | 1–301 | 3,186 | 1 | 1 | 162: 7 | **PASS** after the D2 fix (IM-18) | t157 phase 2 (comm pending); t162 viewer → phase 100, markers of slots 1–6 |
| 15 | C-0065 | 693 | 1–693 | 3,723 | 10 | 1 | — | **PASS** | helicopter; t243 phase 2 (second convoy); t673 phase 100 |
| 16 | C-0066 | 806 | 1–806 | 6,875 | 6 | 0 | — | **PASS** | t225 phase 2 (convoy 1–3 shot down, hence outcome 0); t695 phase 100 |
| 17 | C-0067 | 1,046 | 1–1,046 | 2,688 | 9 | 1 (not lost) | 110: 8 | **PASS** | t107 Amiel dead: cut-scene request, phase 2 → 3 in the same tick; t110 viewer; t111 phase 5; t355 phase 6; t634 phase 7; then phase 100 |
| 18 | C-0068 | 817 | 1–817 | 4,477 | 4 | 1 | 654: 9 | **PASS** | t650 phase 2 (player near the bomb truck); t654 viewer; t684 truck dead → phase 100, bomb timer stopped |
| 19 | C-0069 | 287 | 1–287 | 2,953 | 0 | 1 | — | **PASS** (D3 fixed, IM-18) | t205 the truck reaches the V.C.I.: class 8, armour 1, slot 19 = the truck; t206 phase 100 |
| 20 | C-0090 | 424 | 1–424 | 2,968 | 0 | 1 | — | **PASS** after the D1 fix | the lander's bombs, one of them on heading 5 (D1); t424 succeeded by `Msn_UpdateEvents` |

**Totals.** 19 of the 20 flights match the original on every tick, from setup to the end of the flight, with the same outcome: 17,563 ticks and 115,943 slot-ticks compared (flight 14 up to tick 162). All 20 replays succeed in td-sim (`replays_e1.rs`). *IM-18 (2026-11-06): all 20 flights match on every tick, flight 14 included (17,701 ticks and 119,129 slot-ticks), with nothing copied from the captures; the replays of flights 3, 5, 9, 11, 14, 18 and 19 got new checkpoint hashes (same inputs; the hashes cover the offsets of shots on two-digit headings, which now hold the direct start's bytes, `g_Weapon` in flight 19 and the corrected flight-14 orders), each confirmed by this comparison. VE-4's death and modal-screen replays `e1f01-idle` and `e1f01-modals` were re-recorded the same way (course changed from tick 439 by the direct start's bytes; both pass against C-0157 and C-0158 with the built bytes).*

**What the passes cover** beyond VE-1's first ticks:

- every later mission phase of flights 1–20, with the base test, the depot base and the helipad base;
- kills by the player's volleys (shields, armour, the five explosions, kill credit, distance flash) and by mission ships;
- escapes through exit tiles, spawns and respawns;
- all nine E1 transmissions except 7 (1–6, 8 and 9);
- the flight-17 base-attack logic and cut-scene request, the flight-18 bomb timer, the flight-19 vehicle switch and shield cap, and the flight-20 lander events;
- enemy fire on the player (armour and shields every tick) and the mines of flight 7;
- both infra-red flights and the helicopter flights (late draw);
- the teardown outcome tests of flights 3, 5, 12, 16 and 17.

**Not covered:**

- deaths and the end sequence (no plan loses the player);
- the Esc menu, pause and F5 map during a flight;
- missiles and Tab (the plans never select missiles), the afterburner (never pressed), the joystick;
- any flight that is not the first of a program run (every capture is a direct start);
- flight 21.

**Oracle checks.** Oracle 2 ran VE-1's frame-keyed scenario `ve1-e1f3-firstticks` (the scenario of C-0018) in the VE-2 rig, and all 3,010 output files are byte-identical to the oracle 1 run: `frames.tsv`, `int21.tsv`, `pit.tsv`, `events.tsv`, `io_sites.tsv` and every dump. A repeat of the flight-1 tick-keyed run gives 63 files byte-identical to C-0051.

## VE-5 (2026-11-06): two flights in one run, flights reached through the front end, missiles and the afterburner

Oracle build 3 (watcher generations, `re-work\dosbox\ORACLE.md` §5b; `re-work\dosbox\bin\oracle-3`, SHA-256 `67f8d9dd…`) keys each flight of a program run to its own `tickwatch`, and the second flight's dumps carry `_g2`. It is backward compatible: in a rig whose mount path is as long as VE-2's, C-0058's scenario gives all 44 output files of C-0058 byte for byte, and C-0156's scenario all 79 of C-0156 (`re-work\ve5\compat.py`). VE-5's rig and scripts: `re-work\ve5` (`run1.py` one flight, `run2.py` two flights of a block, `register.py`).

**Harness.** `full_flight::check_run` compares a run's flights in order: the first from program start, each later one with the session the flight before it left (`Flight::session`), its muzzle bytes from its own tick-1 record (`Muzzles::Capture`), its records from its generation. `tdtool` flies a flight that only follows another in its block after the predecessor's replay (`flight_setup::predecessor`: E1 21 after 19, E3 7 after 6, E3 21 after 11), for `plan`, `replay --verify` and `--oracle-schedule`; the L4 tests do the same (`success_replay::check_after`). The projectile arrays are dumped through a 64 KB window that holds them wherever the heap put them.

| Replay | Capture | Start | Ticks | Result | What it covers |
|---|---|---|---:|---|---|
| `e1f03.tdr` | C-0203 | `FILE0001.EXE` at mission 3, Enter, the story played (title and hangar cards, dialogue 11 with Space: C-0160's keys) | 1,651 | **PASS** | a flight reached through the front end: session from program start (RNG, class tables, `Session.weapon` 0), the muzzle bytes the screens left (OB-017). The test flies the capture's bytes; `td-flow/tests/front_end_state.rs` builds the port's bytes from its own screens and matches them against all 1,652 records of C-0203 |
| `e1f08-weapons.tdr` | C-0205 | direct start | 387 | **PASS** | `tdtool plan 8 --weapons`: one Tab to the missile weapon, six missiles of kind 4, three afterburner bursts, the comm viewer at 318; every burst starts in the port's tick |
| `e1f15-weapons.tdr` | C-0206 | direct start | 661 | **PASS** | class 3 (helicopter): one Tab, all twenty missiles of kind 5 |
| `e1f19.tdr`, then `e1f21.tdr` | C-0200 | direct start at mission 19; after flight 19 the debrief times out (10 s), Esc 60 frames after `voffice.scr` opens jumps to flight 21 | 287 + 58 | flight 19 **PASS**; flight 21 **diverges at setup** (D6, test ignored) | block 19 in one run; flight 21 has no input and ends itself through the end sequence (58 ticks); its session (RNG seed after flight 19 and the setup draws, class tables, `g_Weapon`, the player) matches at setup; the slots do not (D6) |

**Not covered (E1):** E1 flight 1 through `td1.exe` (the C-0002 path: no plan found that is neutral to both muzzle worlds; `--dual` not tried for lack of time), deaths in a later flight of a run, retries.

## Divergences

| # | Flight | Tick | Field | Ours | Original | Events around it | Spec | Resolution |
|---|---:|---:|---|---|---|---|---|---|
| D1 | 20 (trial run) | 14 | projectile slot 38 (owner 1, kind 2, heading 5, life 2) | freed | alive at (50, 3), not moving | slot 1's third bomb of the tick takes heading `Random(9)` (mission-logic §5.20) | weapons.md rev 3 §2 ("only 1, 2, 3, 4, 6, 7, 8, 9 move, anything above 9 deletes the shot") and §4.3 step 8 | **Fixed** in commit "fix: shots on headings 0 and 5 stay in place…". td-sim freed every non-principal heading; now only headings above 9 are freed. e1f20's checkpoints were re-recorded with the same inputs; C-0090 passes all 424 ticks |
| D2 | 14 | 163 | slot 13 record +415h (attackMode), +41Fh (targetSel) | 01, 00 | 00, FFh | t157 phase 2 and comm pending. t162 transmission 7 → phase 100, markers 0, then slots 1–6 = 32h. In tick 162 slot 13's hold schedule ends its attack, on both sides. Slot 13 at (151, 130) and the view at (147, 127) are within the 30 of `engage 13 → player`. In phase 2 the original re-ordered slot 13 every tick; afterwards it never does (ticks 163–301, always in range), nor slot 9 after tick 222 (`engage 9 → player, 25`) | mission-logic.md rev 2 §5.14 lists the engagement rules with no condition | U-390 resolved by RE-15 (mission-logic rev 3 §5.14: the five attacks on the player run only while phase < 100). **Fixed** by IM-18; `flight_14_full_matches_oracle` re-enabled and passing |
| D3 | 19 | setup | `g_Weapon` (DS:1CB8) | 1 | 0 | direct start, class 0 (truck) | weapons.md rev 3 §3.2 (classes 0 and 1 keep the previous flight's weapon); the program-start value is not specified | **BUG-2**, U-391. **Fixed** by IM-18 (weapons rev 4 §3.2: `Session::weapon`); the test copies nothing |
| D4 | 17 (trial run, test harness) | 107 | `g_HitFlash` | 17 | 0 | Amiel killed in step 6 (distance flash); cut-scene requested in step 18 | hit-flash.md rev 2 §4.2: every level of a flight fade zeroes it | Host behaviour. **Fixed** by IM-18: td2192 runs the cut-scene (BUG-3) and td-sim zeroes the hit flash when it reports the request (every viewer of the flight sees the same state) |
| D6 | 21 after 19 (C-0200) | setup | every slot: tile, sub offsets, heading, target, speeds, record path bytes | flight 19's end states (td-sim keeps the session's ships) | tiles such as (0, 112), (69, 164), (145, 64), offsets (16, 16), heading 6, speeds 0, path bytes `70 93 70 35 …` | flight 19 PASS (287 ticks); the RNG seed, classes, armour and shields of flight 21's setup match | mission-ships §4.1 steps 3–4 run for every flight, but flight 21 loads no mission file: the arrays they copy do not hold m19.td's paths; mission-setup row 21 "slots keep the previous flight's data" | **U-500** (RE: where the path arrays live and what fills them in flight 21). Test `flights_19_and_21_in_one_run_match_oracle` ignored |
| D7 | 21 after 19 (C-0200); E3 7 after 6, 21 after 11 (C-0201, C-0202) | setup | `g_PlayerTurnDir` (DS:1A5A); slot turn directions (DS:3B1A − 1 + s) | 0 | the previous flight's last values (2 after E1 flight 19) | — | flight-model §2 "starts at 0", mission-ships §2 "0 at setup"; mission-ships §4.1 step 4 writes no turn | **U-501.** Slots: **fixed** (`fix:` commit, `Ships::load_mission` keeps them; E3 flight 21 needed it). Player: **copied** by the harness at the setup of a run's second flight (td-sim needs a `Session` field, an IM task); from a principal heading the next steering input that changes the target sets it anyway |
| D5 | 8, 15, 16 (trial runs, first plans) | — | the flight ends later in the original | — | — | two-digit-heading volleys: the player's while turning, and in flight 16 an enemy's | U-160 (port policy: offset (0, 0) past the muzzle table) | Not a td-sim bug: with the capture's DGROUP bytes td-sim matches the original exactly. Evidence added to U-160; the plans were made neutral to it (see Method) |

## Open questions for the coordinator

1. **U-390**: flight 14 after the transmission (D2).
2. **U-391 / BUG-2**: `g_Weapon` at program start for classes 0 and 1 (D3).
3. **hit-flash.md rev 2 §4.2** says the comm viewer ends any hit flash ("the computer menu, comm viewer and other faded screens"). C-0058 shows `g_HitFlash` = 10 after the transmission screen of tick 318, and flight-screens §4.2 describes no fade for that screen. The sentence should name only the screens that fade: the Esc menu and the flight-17 cut-scene. td2192 already behaves that way.
4. **U-160 policy.** The (0, 0) offsets change the course and the end tick of flights in which the player fires while turning (D5). The original's offsets are the DGROUP bytes from DS:035C on. The bytes in a capture's first tick reproduced every two-digit volley of these 20 flights.
5. **BUG-3**: td2192 does not show the flight-17 cut-scene or the flight-19 V.C.I. screen.

**Answers (RE-15, 2026-11-06):**

1. U-390: the five rules "slots 8, 9, 10, 12, 13 attack the player" of the flight-14 block run only while phase < 100 (`2404:501F`): [mission-logic](../spec/sim/mission-logic.md) rev 3 §5.14. With that gate `flight_14_full_matches_oracle` can be re-enabled.
2. U-391 / BUG-2: `g_Weapon` is 0 at program start; only flight setup and Tab write it (classes 2..8); the session carries it from flight to flight: [weapons](../spec/sim/weapons.md) rev 4 §3.2. HUD icon for weapon 0: OB-090.
3. hit-flash §4.2: corrected in rev 3 (main session) and rev 4 (RE-15: the flight-19 V.C.I. screen fades too).
4. U-160: the bytes past the muzzle table are specified in [weapons](../spec/sim/weapons.md) rev 4 §3.3. They never change during a flight and are the same in every direct start, but differ after a played story (C-0002, C-0160). The replays are neutral only for direct-start bytes: with the bytes of C-0002 (start through `td1.exe`) or C-0160 (mission 3 with its story), 10 of the 20 replays take another course (flights 1, 4, 5, 8, 9, 10, 11, 16, 17, 18). Policy proposal: *preserve* (OB-017).
5. BUG-3: both screens are specified in [flight-screens](../spec/flow/flight-screens.md) rev 4 §4.10.

**Open questions (VE-5):** U-500 (D6), U-501 (D7, spec and `Session`), and for E2 U-502/BUG-11 ([M10-playthrough](M10-playthrough.md)).

## Reproducing

- Plan: `cargo run --release -p tdtool -- plan N [--alt-muzzles BYTES] [--beam 40]` writes `tests/replays/e1fNN.tdr`.
- Check: `cargo run --release -p tdtool -- replay tests/replays/e1fNN.tdr --verify [--alt-muzzles BYTES]`.
- Oracle run: `python -I re-work\ve2\run_plan.py N <replay> <out>`. It calls `tdtool replay --oracle-schedule` and uses the private rig and the oracle 2 build; `re-work\ve2\register.py` registers the run. `re-work\ve2\extract_muzzles.py` writes a capture's muzzle bytes, which are game-derived and stay in re-work.
- Compare: `cargo test --release -p td-sim --test oracle_e1_full -- --nocapture` with `TD2192_DATA` and `TD2192_ORACLE` set.
- VE-5: `tdtool plan N --weapons --suffix=-weapons`; `tdtool plan 21 --idle --suffix=""` (after `e1f19.tdr`); `python -I re-work\ve5\run1.py <rig> <name> 1 N <tdr> [direct|e1-story3]`, `run2.py <rig> <name> 1 19 e1f19.tdr 21 e1f21.tdr`; `register.py`.
