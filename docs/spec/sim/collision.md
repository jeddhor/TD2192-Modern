# Collision (index)

| | |
|---|---|
| **Spec ID** | `sim/collision` |
| **Revision** | 2 |
| **Status** | Draft |
| **Fidelity tier** | T1 |
| **Confidence** | per row below |
| **Tasks** | M6-01, M7-05 |
| **Unknowns** | U-009, U-039, U-047 |

Every collision rule the flight engine applies, with the spec section that defines it. All tests use the tile attribute table of [structures §8](../../re/structures.md) (`solid` = attribute ≠ 2) and world pixels (`tile × 32 + offset`).

| Pair | Rule (summary) | Defined in | Confidence |
|---|---|---|---|
| Player ↔ map | Fixed cells around the view (columns +4..+6, rows +2..+4) tested per axis after drift; offset snapped to 32 (X) or 48 (Y) and the two opposite speed components swapped (bounce) | [flight-model §4.3](flight-model.md) | CONFIRMED horizontally (C-0007), LIKELY vertically |
| Player ↔ map edge | View limited to tiles X 6..183, Y 7..190; pushed back one tile, edge alert | [flight-model §4.4](flight-model.md) | LIKELY |
| Projectile ↔ map | Before moving: the shot's tile or its right neighbour (and the cell above-right for horizontal headings) solid on the side of its offset → shot removed; not for the player's shots while DS:1703 is set | [weapons §4.3 step 2](weapons.md) | LIKELY |
| Projectile ↔ map edge | Tile X or Y outside 2..199 → removed | [weapons §4.3 step 1](weapons.md) | LIKELY |
| Projectile ↔ mission ship | \|dx\| < 20 and \|dy\| < 20 px (homing missile: < 40), shooter excluded, class ≠ FFh; one hit per shot | [weapons §4.3 steps 3–4](weapons.md) | LIKELY |
| Projectile ↔ player | \|dx\| < 20 and \|dy\| < 20 px against the player reference point; enemy shots only; not while DS:1703 is set | [weapons §4.3 step 5](weapons.md) | LIKELY |
| Mission ship ↔ map | None in the movement: ships fly over solid tiles. Path steps are not checked (ships follow the designers' paths). In attack and return mode the AI refuses a direct waypoint step on an axis when any of the 2 × 2 cells at the new waypoint is solid, and then detours with a wall follower. The "tile adjustment" at the start of pass 2 reads a nil pointer and does nothing on the oracle (OB-013) | [mission-ships §4.2.3, §4.3.3, §4.3.7](mission-ships.md) | CONFIRMED (C-0009) |
| Mission ship ↔ map edge | Ship removed: offset Y < 16 at row 1, offset X < 16 at column 1, offset X > 48 at column 199, or any tile coordinate 0 or 200 (no removal at row 199) | [mission-ships §4.2.3](mission-ships.md) | LIKELY |
| Player ↔ mission ship, ship ↔ ship | None: no code writes armour, shields or positions on contact; every writer of player and ship armour/shields is listed in mission-ships §4.5 | [mission-ships §4.5](mission-ships.md) (U-047 resolved) | CONFIRMED (static, exhaustive writer list) |

## Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-02 | RE-2 (Spec author) | Initial index |
| 2 | 2026-11-02 | RE-5 (Spec author) | Mission-ship rows from mission-ships rev 2; U-047 resolved |
