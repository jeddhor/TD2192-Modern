# Parity: M6 flight core and M7 weapons (2026-11-02, in progress)

## Player flight model (L5, automated)

Method: `crates/td-sim/tests/oracle_flight.rs` (needs `TD2192_DATA` and `TD2192_ORACLE`) replays the oracle runs **C-0007** and **C-0008** (E1 mission 1, preset P1, cycles 20000; DGROUP DS:1700..3A3F dumped every video frame). For every tick it feeds `td_sim::Flight` the eight arrow-key bytes the original's key table held, then compares our player state with the original's. The original's state after tick *t*'s physics is read from the first frame whose tick counter (DS:1718) is *t* + 1, and the key bytes are taken from that same frame. The frame before can still show a key that physics already saw released. Both captures switch class 4 → 0 with a cheat chord the port does not implement, so the harness copies that change (class, armour, shields) from the original.

Implementation: `crates/td-sim/src/flight/` from [flight-model.md](../spec/sim/flight-model.md) rev 2 and [tick-order.md](../spec/sim/tick-order.md) rev 1.

| # | Behaviour | Tier | Result | Notes |
|---|---|---|---|---|
| 1 | View tile and pixel offset, all four speed bytes, target heading, turn direction, every tick | T1 | **PASS (exact)** | All ticks of C-0007 (ticks 19..118) and C-0008 (19..135). They cover drift and drag, the dead zone, overshoot of the class maximum, half braking, horizontal wall bounces with the speed swap, tile carry, and class 4 and class 0 thrust |
| 2 | Heading and the turn ring | T1 | **PASS** | The original's frame may already include the next tick's turn step; the test accepts either side of that step and nothing else |
| 3 | Flight-model vectors 1–7 | T1 | **PASS** | Vectors 1–3 on a synthetic open map (`flight/tests.rs`). Vectors 4, 5 and 7 on the real E1 map (`tests/flight_e1.rs`), with the values the original produced |
| 4 | Vertical bounces, map limits, diagonal steering, joystick predicates, afterburner burst | T1 | not captured | Implemented from the spec (LIKELY parts of flight-model rev 2); unit tests only |

## Playfield picture (L5, automated)

Method: `crates/td-render/tests/flight_parity.rs` rebuilds C-0002 frames f3500 and f3543 (E1 mission 1 at rest; view (102, 104, 32, 16), class 4, heading 8) from `td.map`, `td.blk` and `td.shp` alone and compares rows 0..159 pixel for pixel. Implementation: `td_render::flight`, from [camera-viewport.md](../spec/sim/camera-viewport.md), [hud.md](../spec/render/hud.md) and [dynamic-observations.md](../re/dynamic-observations.md) §13.

| # | Behaviour | Tier | Result | Notes |
|---|---|---|---|---|
| 5 | 11 × 6 tile window, start address with 4-pixel horizontal steps | T0 | **PASS (exact)** | |
| 6 | Player sprite position (144, 68) and transparency | T0 | **PASS (exact)** | Resolves U-046 |
| 7 | Overpass tiles redrawn opaquely over the ships | T0 | **PASS (exact)** | Tiles 42h, 44h, 45h, 46h in window columns 3..6, rows 1..4 |
| 8 | Objects drawn after the overpass redraw | T0 | excluded | 15 pixels of a small moving object near (171, 64), drawn after step 12. This is M8 |
| 9 | HUD bar | T0 | partial | `td.win` is drawn; gauges, radar, weapon icon and ticker wait for hud.md rev 2 (M6-07) |

## Weapons (M7, L0 only so far)

Implementation: `crates/td-sim/src/flight/shots.rs`, from [weapons.md](../spec/sim/weapons.md) rev 1, which is a static spec. Spec vectors 1–3 pass: two volleys every second tick in slots 48..45 first; shields absorb 10 of 25; a player hit with shields 1 leaves armour 24. Further L0 tests cover destruction (the kill counter, the preserved projectile-kind bug), tile-step movement with the 25-tick life, and the missile cooldown and re-press rule. **No capture yet:** a Verifier run with a volley, a missile and a kill is needed (RE-2 handoff).

## Still to do for M6-09

- A human "feel" sign-off in the dev arena (`td2192 --dev-flight N`), on keyboard and on a game controller (M4-03).
- Captures for the parts still marked LIKELY (row 4) and for weapons.
