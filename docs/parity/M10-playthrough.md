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
- **Projectile arrays.** E2 and E3 allocate the projectile arrays on the heap at a place that depends on the path (music and scene sizes): E2 flight 1 `656C:0000`, flights 2–20 one of `6290`, `62BD`, `62CE`, `649B`, `656C`. The pre-run reads the eight far pointers (E2 DS:3896.., E3 DS:3892..) at tick 1, and the flight run dumps 0x240 bytes from that paragraph; the test takes the segment from the dump's file name.

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

All comparisons are exact (T1): every tick of the flight, the last one included (stall record), plus setup and the outcome after teardown, unless the row says otherwise. Slot-ticks count the slots live in the port after each compared tick.

**Episode 2**

| Flight | Capture | Plan | Ticks | Slot-ticks | Kills | Outcome | Comm viewer | Result |
|---:|---|---|---:|---:|---:|---:|---|---|
| 1 | C-0120 | dual | 1,079 | 9,876 | 11 | 1 | 824 | **PASS** |
| 2 | C-0121 | dual | 1,367 | 8,756 | 29 | 1 | — | **PASS** |
| 3 | C-0122 | strict | 1,719 | 3,892 | 12 | 1 | — | **PASS** |
| 4 | C-0123 | strict | 336 | 2,931 | 5 | 1 | — | **PASS** |
| 5 | C-0124 | strict | 631 | 4,128 | 8 | 1 | 162 | **PASS** |

**Episode 3**

| Flight | Capture | Plan | Ticks | Slot-ticks | Kills | Outcome | Comm viewer | Result |
|---:|---|---|---:|---:|---:|---:|---|---|
| 1 | C-0125 | strict | 921 | 2,591 | 16 | 1 | — | **PASS** |
| 2 | C-0126 | strict | 405 | 4,087 | 8 | 1 | — | **PASS** |
| 3 | C-0127 | strict | 982 | 4,901 | 15 | 1 | — | **PASS** |
| 4 | C-0128 | strict | 665 | 5,463 | 1 | 1 | — | **PASS** |
| 5 | C-0129 | strict | 854 | 4,950 | 4 | 1 | — | **PASS** |


## Divergences

DIVERGENCES

## Open questions for the coordinator

QUESTIONS

## Reproducing

- Pre-run (projectile segment and muzzle bytes): `python -I re-work\ve4\prerun.py <rig> <E> <N>`.
- Plan: `cargo run --release -p tdtool -- plan N --episode E --alt-muzzles re-work\ve4\muzzles\eEfN.bin [--dual]` writes `tests/replays/eEfNN.tdr`.
- Check: `cargo run --release -p tdtool -- replay tests/replays/eEfNN.tdr --verify [--alt-muzzles BYTES]`.
- Oracle run: `python -I re-work\ve4\run_plan.py <rig a|b|c> <E> <N>`; `re-work\ve4\register.py` registers it.
- Compare: `cargo test --release -p td-sim --test oracle_e2_full --test oracle_e3_full -- --nocapture` with `TD2192_DATA` and `TD2192_ORACLE` set.
