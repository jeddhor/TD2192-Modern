# Parity: M9 Episode 1 playthrough, whole flights to success (2026-11-06, in progress)

Verifier: VE-2 (M9-21, the success replays of M9-01..M9-17, the replay check of M6-02). Implementation under test: `crates/td-sim` at the branch head (`flight/logic.rs` = [mission-logic.md](../spec/sim/mission-logic.md) rev 2, `setup.rs` = [mission-setup.md](../spec/sim/mission-setup.md) rev 1, `ships.rs` = [mission-ships.md](../spec/sim/mission-ships.md) rev 2, `shots.rs` = [weapons.md](../spec/sim/weapons.md) rev 3, tick order = [tick-order.md](../spec/sim/tick-order.md) rev 1). VE-1's report for the first ~387 ticks is [M8-missions.md](M8-missions.md).

## Method

**Plans.** `tdtool plan N` searches td-sim for inputs that complete flight N with success (`crates/tdtool/src/plan/`). It is a beam search: every 3 ticks each of the 24 kept flights is cloned once per action (one of the 8 keypad directions, nothing, or an F6 tap with every key released when a transmission waits) and run for 3 ticks. Space is held by a rule whenever a ship other than a friend (marker 39h) is first on a clear line of fire along the current heading. Clones are scored by mission phase, the number and hit points of live targets (marker 32h), the grid distance to the nearest cell with a line of fire on a target (or to the base once the phase is 100), a bonus while a target is in the line of fire, and the damage the player took. A clone whose player is destroyed or whose mission is lost is dropped. Only keypad keys, Space and F6 are used; the afterburner keys are never pressed. The flight is started as the first of a program run (`Session::new()`), with the 342-byte muzzle table of the executable (the world the game uses).

**Replays.** The plan is written as `tests/replays/e1fNN.tdr` (TDR v2, our data): one `TickControls` word per tick, a state hash every 100 ticks. `tdtool replay FILE --verify` re-runs it and checks every hash; `crates/td-sim/tests/replays_e1.rs` (L4, `TD2192_DATA`) does the same and checks that the flight ends with success on the last tick and not earlier. Replay semantics ([`td_sim::replay::step`](../../crates/td-sim/src/replay.rs)): a modal screen the tick opens is closed at once with no input.

**Oracle input: the tick rule.** The DOSBox-X oracle build 2 (VE-2 private build, `re-work\ve2\bin\oracle-2`; `re-work\dosbox\ORACLE.md` §5a) watches `g_MissionTicks` (DS:1718) at every emulated millisecond. `tdtool replay FILE --oracle-schedule` turns the replay into `tick:T down|up <key>` lines. The exact rule:

- The game increments the counter at step 1 of tick T, right after tick T − 1's input steps 23–33, which follow a vertical-retrace wait (tick-order rev 1 §4).
- The oracle sees the new value within 1 ms (20,000 cycles at preset P1) and injects the transitions of `tick:T` in that millisecond; each scan-code byte reaches the INT 09h handler about 0.3 ms later. The game is then in steps 2–4 (drawing the 66-tile window takes several ms).
- Tick T's input steps run after the tick wait and a retrace wait, at least ~12 ms later, so they see every transition of `tick:T`; tick T − 1's had already run. So port tick T gets: keys down = the keys down after `tick:T`, keys made = the keys that went down at `tick:T`.
- Exception, not used by the plans: the afterburner keys are also read at step 5, early in the tick.
- Typematic repeat (DOSBox-X repeats the last key pressed after 500 ms, then every 33 ms) sets a byte the game has cleared, which a replay word cannot express. The plans hold the keys the game clears (afterburner, Tab, Alt, F6) for at most 5 ticks and never select missiles, so Space is never cleared.
- **Comm viewer.** While the transmission screen is open the counter stays at the tick T that opened it. The schedule presses Space and F6 alternately every 15 frames from 20 frames after T with `intick:T+N` lines, which run only while the counter still shows T: Space answers each prompt, the first F6 after the last prompt closes the screen, and the presses that come later are skipped. No key reaches a flight tick, as in the replay semantics.

The rule was checked by the comparison itself: the test compares the port's key table after every tick with the original's key bytes (the record of tick t + 1 is taken before tick t + 1's transitions), and every tick of the flights below matches.

**Captures.** One oracle run per flight with VE-1's direct start (`re-work\ve2\run_plan.py`, private rig `re-work\ve2\dosbox`, preset P1, cycles 20000): the game copy is reset to the install, `tdstatus.dat` = status 1, kills 0, mission N, menu cursor 3, script 1; `FILE0001.EXE` with `-KeepState`; Enter at the menu, Esc at the mission card; the watcher is armed one frame after `mN.td` opens. Per-tick packed dumps of DGROUP (4467:0000, 0x9DF0) and of the projectile heap arrays (5C4E:0000, 0x240), a stall record 150 ms after the last change, a DGROUP dump 240 frames after the last planned tick (teardown and outcome), a framebuffer every 70 frames.

**Test.** `crates/td-sim/tests/oracle_e1_full.rs` (L5, `TD2192_DATA` + `TD2192_ORACLE`), one test per flight; it finds the capture by its scenario's first line (`# ve2-e1fN-plan`) and skips flights without a replay or a capture.

- **Sampling.** The record of value t + 1 is taken within 1 ms after the counter became t + 1: the state after tick t, plus tick t + 1's counter, per-tick flags and turn step (the heading may be one turn step ahead). The last tick is compared with the stall record. No frame has to be guessed, no record is torn, and no field needs a second frame.
- **Setup** is compared with the record of tick 1 (the setup `AI_ChooseWaypoint` pass has run, so the whole record is compared).
- **Each tick:** the `Random` seed, draw page, phase, comm pending, succeeded, lost, kills, home marker, the 20 markers, ticker, comm indicator, hit flash; the player's view, four speeds, heading (or its next turn step), target heading, turn, armour, shields, weapon, missiles, cooldown, mission over, end ticks; the key bytes of all 20 flight keys; arrays and the whole 1,065-byte record of every slot live on either side; the 51 projectile slots.
- **End:** the original's last tick is the replay's last tick; outcome (DS:12CF), succeeded and lost after teardown equal the port's.
- **World.** The muzzle bytes past the table (U-160) are taken from the capture's tick-1 record, as VE-1 did; the replays themselves were planned with the 342-byte table.

## Results

(in progress)

## Divergences

(none so far)
