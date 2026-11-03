# Mission ships: movement along paths

| | |
|---|---|
| **Spec ID** | `sim/mission-ships` |
| **Revision** | 1 |
| **Status** | Draft (partial) |
| **Fidelity tier** | T1 |
| **Confidence** | LIKELY for the per-tick movement step (`3292:0011`, read in full for movement); PARTIAL for waypoint choice (`34E6:3449` is an AI state machine, only its outline is known) |
| **Tasks** | M6-01 follow-up, M8-01, M8-03 |
| **Unknowns** | U-039 (partial), U-045 |

## 1. Summary

The 20 mission ships move with **the same four-component speed model as the player** ([flight-model](flight-model.md)): biased speed bytes up/down/left/right, drag of 1 per tick, class maximum speed and thrust step from the class tables. Each ship steers toward a **target tile** (its current waypoint) by thrusting along the axes where the target lies, and turns its heading one ring step per tick toward a target heading. When a ship reaches its waypoint tile, `34E6:3449` chooses the next waypoint: in the simplest mode that is the next step of the ship's mission path; other modes (attack, chase, evade) compute the waypoint from other ships or the player (U-045). Ships leave the game (class := FFh) when destroyed, when they reach the map edge, or when their waypoint is the exit marker (50, 0) or (50, 1).

## 2. Definitions

| Item | Where | Notes |
|---|---|---|
| Ship slot s | 1..20 | Arrays of structures §4, record of structures §3 |
| Speeds | DS:3B6A + 4(s − 1) + {0 up, 1 down, 2 left, 3 right} | u8, biased by 16, capped at 42 |
| Target tile | DS:3BBA − 1 + s (X), DS:3BCE − 1 + s (Y) | Copied from the record's waypoint (+001h/+002h) every tick |
| Heading / target heading / turn direction | `g_SlotHeading` DS:3AA2, `g_SlotHeading2` DS:3AB6, `g_SlotState` DS:3B1A | Same ring and rules as the player's turn step |
| Shields / armour | record +3F8h / +3FAh (i16) | |

## 3. Behaviour (`3292:0011(flight)`, tick-order step 10)

```text
pass 1, for s = 1..20 with class != FFh:
    if shields(s) < shields_table[class]: shields(s) += 1
    if waypoint(s) = (50, 0 or 1) or armour(s) < 1: class(s) := FFh          (leaves / destroyed)
pass 2, for s = 1..20 with class != FFh:
    (map-dependent adjustment of the ship's tile, 3292:0011 lines for DS:3A32; U-039)
    if waypoint(s) = ship tile and not (flight 20 and s <= 5): choose the next waypoint (34E6:3449)
    target := waypoint(s)
    if target heading != heading: one ring step in the turn direction (as the player)
    flight 20, s <= 5: heading := 8; offsetY(s) += 2 (scripted drift); no thrust
    otherwise, thrust toward the target, one block per axis, in the order up, down, left, right:
        up block (target row above the ship) and up < max[class]:
            up += accel[class]
            if down > 16 and target row <= ship row:      up −= accel   (original: subtracts from up)
            if left > 16 and ship column <= target column: left −= accel
            if right > 16 and target column <= ship column: right −= accel
            set per-tick flag DS:3BE3
        (down, left and right blocks: same pattern for the other axes; turn direction and target
         heading are set from the direction of travel)
    cap every speed component at 42
    normalise offsetX into −16..80 by ±32 steps
    drift and drag, axis by axis in the order up, down, left, right (as the player, 4.2), with carry:
        offsetY < 16: tile Y −= 1, offsetY += 32 (tile Y = 1: the ship leaves)
        offsetY > 48: tile Y += 1, offsetY −= 32 (while tile Y < 199)
        offsetX < 16: tile X −= 1, offsetX += 32 (tile X = 1: the ship leaves)
        offsetX > 48: tile X += 1, offsetX −= 32 (while tile X < 199; tile X = 199: leaves)
        any tile coordinate of 0 or 200: the ship leaves
    draw the ship sprite for its heading; flight-specific overlays (flight 7 pins slot 2 to (50, 182))
```

Differences from the player: braking subtracts the full thrust step (not half), the up block's "brake down" subtracts from the up component (so thrusting up while still moving down gains nothing until the down component has decayed), there is no map collision bounce in this routine (map handling is the unread adjustment above), and the speed cap is 42 instead of the class maximum (thrust stops at the class maximum, but bursts can exceed it).

### 3.1 Waypoint choice (`34E6:3449`, partial)

`34E6:3449(s)` runs for every slot at flight start and whenever a ship reaches its waypoint. It always calls `34E6:0396` first and `34E6:0869` last. In between, flags in the ship record select one behaviour:

| Record flags | Called | Reading |
|---|---|---|
| +428h = 0 | `34E6:0776` | path following: next step of the mission path (cursor +00Bh) |
| +3FCh = 1 | `34E6:3275` | ? |
| +415h, +428h, +3FCh, +422h all 0 | `34E6:2FAF` | ? |
| +415h = 1, +41Bh = 1, +422h = 0 | `34E6:2448` | ? (target in sight) |
| +415h = 1, +41Bh = 0, +422h = 0 | `34E6:23B2` | waypoint += (dx, dy) from DS:976C/976E; old waypoint saved at +003h |
| +428h = 1 with +41Bh = 1 / 0 | `34E6:280B` / `34E6:23B2` | as above |
| +422h = 1 with +41Bh = 1 / 0 | `34E6:2C0D` / `34E6:23B2` | as above |

C-0002 shows path following for mission 1: the cursor advances by one per waypoint and a class-0 ship covers one tile per about four ticks (8 px/tick). The meaning of the flags and of the routines is U-045 (M8-03).

## 4. Edge cases and original bugs

| Quirk | Original | Port policy |
|---|---|---|
| Up-block brake writes the up component | Ships reverse slowly | Preserve (T1) |
| Exit marker waypoint (50, 0/1) | Ship removed at the start of the next tick | Preserve |
| Flight 20 slots 1..5 | Scripted downward drift, never reach waypoints | Preserve |

## 5. Test vectors

To be captured: per-frame dumps of DS:3A52..3BFF and the ship records (DS:4432..9765) during E1M1 without input (VE, §6 Q1).

## 6. Open questions

- [ ] Q1 (VE): capture mission-1 ship movement and compare with §3.
- [ ] Q2 (U-039): the DS:3A32 map adjustment at the start of pass 2 and the down/left/right blocks in detail.
- [ ] Q3 (U-045): behaviours selected in `34E6:3449`.

## 7. Provenance

| Claim | Address | Method | Confidence |
|---|---|---|---|
| Shield regeneration, exit marker, destruction removal | `3292:0011` pass 1 | static | LIKELY |
| Thrust, cap 42, drift, carry, edge removal | `3292:0011` (`3292:041A..`) | static | LIKELY |
| Up-block brake quirk | `3292:045E..04B0` | static (listing) | CONFIRMED |
| Waypoint choice outline | `34E6:3449` | static | PARTIAL |

## 8. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-02 | RE-2 (Archaeologist + Spec author) | Partial draft |
