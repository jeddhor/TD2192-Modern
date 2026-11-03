# Weapons, projectiles and damage

| | |
|---|---|
| **Spec ID** | `sim/weapons` |
| **Revision** | 1 |
| **Status** | Draft |
| **Fidelity tier** | T1 (projectiles, hits, damage, kills and the RNG calls of hit explosions) · T2 for the drawing of shots |
| **Confidence** | LIKELY (static reading of `0E8B:B1EC`, `0E8B:0984` and the enemy-fire step of `Sim_FlyMission`). No capture yet (§8) |
| **Tasks** | M6-01 (follow-up), feeds M7-02, M7-03, M7-05 |
| **Unknowns** | U-041 and U-043 (resolved here), U-040, U-061 |

## 1. Summary

All shots, of the player and of the 20 mission ships, live in one pool of 51 projectile slots. A shot has a map tile, a pixel offset, a heading, a kind and a life counter. **Shots move one whole tile (32 px) per tick** along one of the eight principal directions and die after 25 ticks, when they leave the map, or when they enter a solid tile. Each tick every shot is tested against the 20 mission ships and against the player with a square hit box of ±19 px. Damage depends only on the shot kind; shields absorb first and the rest goes to armour. A ship whose armour reaches 0 is destroyed: its class code becomes FFh and, if the player fired, the mission kill counter increases. The player fires with **Space** (primary weapon every second tick while held, or one missile per press) and switches between primary and missiles with **Tab**.

## 2. Definitions and units

| Term | Original | Type | Notes |
|---|---|---|---|
| Projectile slot | index 0..50 into the eight heap arrays (structures §5) | — | Slot free when tile X = FFh |
| Tile X, Y | DS:3986 / DS:398A arrays | u8 | Map column / row |
| Offset X, Y | DS:3976 / DS:397A arrays | i16 | Pixels added to tile × 32 (not kept inside 0..31) |
| Heading | DS:397E array | u8 | Keypad code; only 1, 2, 3, 4, 6, 7, 8, 9 move, anything above 9 deletes the shot |
| Kind | DS:3982 array | u8 | 0..3 cannon/pulser classes, 4 and 5 missiles, 6 homing missile |
| Life / target | DS:3972 array | u8 | Life counter for kinds 0..5; **target slot** for kind 6 |
| Owner | DS:396E array | u8 | 0 = player, 1..20 = mission slot |
| Weapon | `g_Weapon` DS:1CB8 | u8 | Player's selected weapon 1..6 (§3.2) |
| Missiles | `g_Missiles` DS:1CB9 | u8 | Missiles left (HUD bar, max shown 26) |
| Missile cooldown | `g_MissileCooldown` DS:1CBA | u8 | Set to 4 on a launch, −1 per tick (tick-order step 34) |
| Difficulty | DS:13DE | u8 | Multiplier of damage to the player; 1 at flight start (cheat chords change it, globals §5) |

World pixel of a shot = tile × 32 + offset (per axis). World pixel of a mission ship = slot tile × 32 + slot offset (DS:3ACA/3AF2). The player's hit reference = `((viewX + 5) × 32 + viewSubX, (viewY + 3) × 32 + viewSubY)`.

## 3. Data

### 3.1 Shot table (DS:02E5 + 17 × class + m, m = 1..17) and damage

| Class | Muzzles m with an entry | Entry (kind + 1) | Kind | Shots per volley | Damage to a ship per shot |
|---:|---|---:|---:|---:|---:|
| 0, 1 | none | — | — | 0 | — |
| 2 | 1, 2, 4, 5 | 1 | 0 | 4 | 5 |
| 3 | 8, 9 | 1 | 0 | 2 | 5 |
| 4 | 6, 7 | 2 | 1 | 2 | 10 |
| 5 | 3 | 2 | 1 | 1 | 10 |
| 6 | 10, 11 | 3 | 2 | 2 | 17 |
| 7 | 12, 13 | 2 | 1 | 2 | 10 |
| 8 | 14, 15, 16, 17 | 1 | 0 | 4 | 5 |

Damage to a ship by kind k: `(k + 1)² + 4 + 2k` → k = 0: 5, 1: 10, 2: 17, 3: 26, 4: 37, 5: 50; kind 6 destroys outright. Damage to the player: `2 × (k + 1) × difficulty` → with difficulty 1: kind 0: 2, 1: 4, 2: 6.

The volleys agree with the class chart's armament lines (truck and transport unarmed, four light pulsers for classes 2 and 8, two weapons for 4 and 7), which supports the class names in structures §6.

Muzzle offsets (dx, dy) per heading h and muzzle m are signed bytes at DS:035C + 34h + 2m (h = 1..9; heading 5 is unused). For example heading 8, muzzles 6 and 7: (7, 11) and (22, 11); heading 6, muzzles 1, 2, 4, 5: (16, 25), (16, 9), (31, 19), (31, 15). Two-digit headings index beyond the table (U-040).

### 3.2 Player weapons per class (set at flight start, `0E8B:C751..C86C`)

| Class | Primary weapon | Missile weapon (Tab) | Missiles at start |
|---:|---:|---:|---:|
| 0, 1 | (unchanged from the previous flight) | none | 0 |
| 2 | 1 | 4 (kind 4) | 6 |
| 3 | 1 | 5 (kind 5) | 20 |
| 4 | 2 | 4 (kind 4) | 4 |
| 5 | 2 | 6 (homing) | 1 |
| 6 | 3 | 5 (kind 5) | 6 |
| 7 | 2 | 2 (same as primary) | 0 |
| 8 | 1 | 1 (same as primary) | 0 |

Weapons 1..3 mean "fire the class volley" (the weapon number only selects the HUD icon from `td.wea`); weapons 4..6 fire one missile of kind 4, 5 or 6.

## 4. Behaviour

### 4.1 Player input (tick-order steps 33–34, `0E8B:B1EC` and `0E8B:F4F8..F6A7`)

```text
Tab (scan 0Fh) pressed, or the missile weapon selected (weapon > 3) and no missiles left:
    clear the Tab byte
    if weapon > 3 and class > 1:            weapon := primary weapon of the class (3.2)
    elif weapon <= 3 and class > 1 and missiles > 0: weapon := missile weapon of the class
    redraw the weapon icon
M (scan 32h) and the HUD ticker idle:        restart the ticker (replays the current message)
Alt or right Alt, palette mode 1 -> 2; or palette mode 2 -> 1 (never from mode 0; not after the
    mission is over): infra-red view on/off (palette only, no game effect); clears the Alt bytes
Space or joystick button 2:
    if weapon > 3 and missiles > 0 and cooldown = 0 and not over:   launch one missile (4.2)
    if weapon <= 3 and not over and the back page is 1 (every second tick):  fire a volley (4.2)
```

Holding Space fires a volley every second tick (the Space byte is not cleared); a missile clears the Space byte, so each missile needs a new key press (or the keyboard's auto-repeat) and at least 4 ticks.

### 4.2 Spawning

```text
player volley:  for m = 1..17 with shot_table[class][m] != 0:
    slot := highest free slot in 8..48 (none -> skip this muzzle)
    life := 1; tile := (viewX + 4, viewY + 3); owner := 0; heading := player heading
    kind := shot_table[class][m] − 1
    offsetX := 4 × round(viewSubX / 4) + muzzle[heading][m].dx
    offsetY := viewSubY − 28 + muzzle[heading][m].dy
    sound: kind = class (or 2 while DS:1703 is set), at the view
player missile: slot := lowest free slot in 40..50 (none: the original writes slot 200, out of the
    array; port: skip, see 5)
    life := 0; kind := weapon (4, 5 or 6); tile := (viewX + 4, viewY + 3); owner := 0
    heading := player heading; offsetX := viewSubX − 2; offsetY := viewSubY − 12
    if weapon = 6: target := the live mission slot (class != FFh) with the smallest squared tile
        distance to (viewX + 4, viewY + 3), ties to the lower slot; stored in the life field.
        No live ship: target stays 0 and the missile is downgraded to kind 0
    missiles −= 1; cooldown := 4; sound kind 10
mission ship volley: see tick-order step 31 (highest free slot in 0..40, life 1, tile = ship tile,
    offset = muzzle + ship offset − 16, kind = shot table entry − 1, owner = slot)
```

### 4.3 Per-tick update (`0E8B:0984`, tick-order step 6)

For each projectile slot i = 0..50 in order, if its tile X ≠ FFh:

```text
1. if tile X or tile Y < 2 or > 199: free the slot
2. if kind != 6: life += 1; then, unless (DS:1703 set and owner = 0), test the map:
     heading with a vertical part (8, 2, 9, 1, 3, 7):
         solid(tileX, tileY) and offsetX < 17  -> free
         solid(tileX + 1, tileY) and offsetX > 16 -> free
     heading with a horizontal part (4, 6, 9, 1, 3, 7):
         solid(tileX + 1, tileY − 1) and offsetY < 17 -> free
         solid(tileX + 1, tileY) and offsetY > 16 -> free
3. kind 6: for each live mission slot s != owner with |ship − shot| < 40 px on both axes:
     free the slot; mission kills += 1; white flash; ship armour := 0, shields := 0;
     explosion at the ship
4. unless (flight 20 and owner = 1): for each live mission slot s = 1..20, s != owner, with
   |ship − shot| < 20 px on both axes:
     free the slot; hit sound (kind 9, at the ship)
     d := damage(kind) (3.1)
     if d < shields(s): shields(s) −= d
     else: armour(s) := armour(s) − d + shields(s); shields(s) := 0
     if armour(s) < 1: destruction sound (kind 100); distance flash (0E8B:06C5); armour(s) := 0
     if kind = 6: white flash; armour(s) := 0; shields(s) := 0
     if shields(s) > 60 or < 0: shields(s) := 0
     if armour(s) = 0:
         kind[s] := 1                     (original bug: writes the projectile kind array at index s)
         if owner = 0: mission kills += 1
         class(s) := FFh; five explosions around the ship (fixed offsets)
     else: one small explosion at ship offset + Random(16) on each axis (X first, then Y)
5. if owner != 0 and DS:1703 = 0 and |player reference − shot| < 20 px on both axes:
     free the slot; hit sound (kind 9)
     if difficulty != 0: apply 2 × (kind + 1) × difficulty to shields, then armour, as in step 4
     if armour < 0: armour := 0;  if shields > 60 or < 0: shields := 0
     explosion at the player: offset view + Random(8) − 8 on each axis (X first, then Y)
6. kind 6: heading := principal direction from the shot's tile toward the target slot's tile
     (both axes differ: 3, 9, 1 or 7; same column: 2 if the target is below, else 8;
      same row: 6 if the target is to the right, else 4; DS:1CBC keeps the last value)
   draw the shot (kinds 0..3 as short lines in colours 3Ch/3Dh/35h/BEh, kinds 4..6 as sprites)
7. if life > 25: free the slot
8. if still alive: move one tile along the heading (8: y−1, 2: y+1, 4: x−1, 6: x+1, 9: y−1 x+1,
   7: y−1 x−1, 1: y+1 x−1, 3: y+1 x+1); a heading above 9 frees the slot
```

The tests in steps 3–5 use the positions before this tick's move; a shot can therefore pass through a target that is less than 20 px wide in its path between two tile steps.

## 5. Edge cases and original bugs

| Quirk | Original | Port policy (proposal) |
|---|---|---|
| Missile with no free slot | Writes slot 200 (outside the 51-byte arrays) | Fix (skip the launch); undefined in the original |
| Homing launch writes life 200 into slot 40 | Slot 40 (if in use) dies next tick | Preserve |
| Destroyed ship sets projectile kind[s] := 1 | Changes the kind of projectile slot s | Preserve (T1) |
| Homing missile with target 0 reads slot-0 arrays (DS:3B2D/3B41) | Steers toward garbage | Cannot happen (downgraded to kind 0) unless the target dies; then it keeps homing on the dead ship's last tile |
| Two-digit headings | Shot deleted on its first move | Preserve |
| Tile-step movement | Shots jump 32 px per tick | Preserve |
| Flight 20, owner 1 | Slot 1's shots never hit ships | Preserve (mission rule) |

## 6. Test vectors

To be captured (§8). Static examples:

| # | Setup | Expected |
|---|---|---|
| 1 | Class 2 player, heading 8, Space held for 4 ticks | 2 volleys of 4 shots (every second tick), kinds 0, slots 48, 47, 46, 45 first |
| 2 | Kind 1 shot hits a class 7 ship with shields 25 | shields 15, armour unchanged |
| 3 | Kind 0 shot hits the player (difficulty 1, shields 1, armour 25) | shields 0, armour 24 |

## 7. Presentation and audio hooks

Sounds through `1E2A:006A`: player volley (kind = class, or 2), missile (10), hit (9), destruction (100), enemy volleys (kind = shooter class). Flashes: white (kind 6 hits), red distance flash `0E8B:06C5` = `max(flash, 20 − round(sqrt(distance)))` with Real48 `Sqrt` (presentation only).

## 8. Open questions

- [ ] Q1 (VE): capture a volley, a missile launch and a ship kill in E1M1 (class 4) with per-frame dumps of the projectile arrays (heap; follow the far pointers at DS:396E..398A) and the ship records.
- [ ] Q2: the five destruction-explosion offsets and the `06C5` distance formula (presentation).
- [ ] Q3 (U-040): what the original reads for two-digit headings in the muzzle table.

## 9. Provenance

| Claim | Address | Method | Confidence |
|---|---|---|---|
| Player input, Tab, M, Alt, Space | `0E8B:B1EC..B78x`, `0E8B:F4F8..F6A7` | static | LIKELY |
| Weapons and missiles per class | `0E8B:C751..C86C` | static | CONFIRMED (table code) |
| Shot table and muzzle table | DS:02E5, DS:035C (file bytes) | static | CONFIRMED (values) |
| Update order and movement | `0E8B:0984..1AxX` | static | LIKELY |
| Damage formulas | `0E8B:0984` (ships), same routine (player) | static | LIKELY |
| Enemy volleys | tick-order step 31 | static | LIKELY |

## 10. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-02 | RE-2 (Archaeologist + Spec author) | Initial |
