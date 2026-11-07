# Mission ships: setup, AI, movement, firing and removal

| | |
|---|---|
| **Spec ID** | `sim/mission-ships` |
| **Revision** | 7 |
| **Status** | Draft |
| **Fidelity tier** | T1 (positions, waypoints, AI state, fire requests and RNG use per tick) · T2 for drawing |
| **Confidence** | **CONFIRMED** for §4.1–§4.6: a model written from this text reproduces 3,342 of 3,343 slot-ticks of capture C-0009 byte for byte (slot arrays and the whole 1,065-byte ship record), the one exception being a ship destroyed by a projectile (step 6, [weapons](weapons.md)); shot spawns match in 49 of 51 events (§6). LIKELY for the paths the capture does not exercise (mode +422h, flights 7 and 20 specials, map edges) |
| **Tasks** | M8-01, M8-03 (feeds M8-04, M7-02) |
| **Unknowns** | U-038, U-039, U-040, U-042, U-045, U-047 (resolved here); U-160, U-161, U-162 (new); U-500 and the slots' part of U-501 (resolved here, RE-18) |

## 1. Summary

A flight has 20 mission-ship slots. Each slot has a class (FFh = no ship), a position (tile + pixel offset, like the player), four biased speed components, a heading, and a 1,065-byte **ship record** that holds its path (up to 500 tile steps from the mission file), its **waypoint** (the tile it is flying to, always at most one tile step from where it was) and its AI state.

Every tick `AI_UpdateShips` (`3292:0011`, tick-order step 10) regenerates shields, removes ships that left or were destroyed, and then for each live ship: when the ship's tile equals its waypoint it asks the AI (`AI_ChooseWaypoint`, `34E6:3449`) for the next waypoint; it turns one heading step toward a target heading, thrusts along each axis toward the waypoint (same speed model as the player, braking with the full thrust step), applies drift and drag with tile carry, removes ships at the map edge, and draws the ship (sprite = class code). The AI has four modes: **path following** (the default), **attack** (ordered by the mission script through `AI_SetTarget`), **detour** (a right- or left-hand wall follower used when the direct step is blocked by solid map cells) and **return** (fly back to the point where the ship left its path). In attack mode the ship raises **fire requests** along the row or column toward its target; the main loop turns them into shots every second tick (tick-order step 31). There is no body collision between ships or with the player.

## 2. Definitions and units

| Term | Original | Type | Notes |
|---|---|---|---|
| Slot s | 1..20 | — | Index of every array below and of `record[s]` |
| Class | `g_SlotClass` DS:3A7A − 1 + s | u8 | Class code ([structures §6](../../re/structures.md)); FFh = empty, gone or destroyed |
| Tile X / Y | DS:3B2E − 1 + s / DS:3B42 − 1 + s | u8 | Map column / row; map cell (x, y) is byte `x × 201 + y` of `td.map` |
| Offset X / Y | DS:3ACA + 2(s − 1) / DS:3AF2 + 2(s − 1) | i16 | Pixels; world pixel = tile × 32 + offset; kept in 16..48 by the carry |
| Speed up / down / left / right | DS:3B6A + 4(s − 1) + 0 / 1 / 2 / 3 | u8 | Biased by 16 (16 = still), capped at 42; 0 at setup |
| Heading | `g_SlotHeading` DS:3AA2 − 1 + s | u8 | Keypad code ([structures §7](../../re/structures.md)); 6 at setup |
| Target heading | DS:3AB6 − 1 + s | u8 | Heading the ship turns toward; 6 at setup |
| Turn direction | DS:3B1A − 1 + s | u8 | 1 = counter-clockwise (previous ring entry), 2 = clockwise (next); 0 at program start, **never reset by setup**: a flight starts with the values the previous flight of the program run left (U-501; only §4.2.2 writes it) |
| Goal X / Y | DS:3BBA − 1 + s / DS:3BCE − 1 + s | u8 | Copy of the waypoint used by the thrust step |
| Marker | `g_SlotMarker` DS:3958 − 1 + s | u8 | 32h or 39h: draw a bracket around the ship (§4.7); set per flight ([mission-setup](mission-setup.md)) |
| Fire request | DS:3BEC + 9s + e, e = 1..9 | u8 | 1 = fire; the AI sets e = 2, 4, 6 or 8 |
| Thrust flags | DS:3BE3 up, DS:3BE5 right, DS:3BE7 left, DS:3BE9 down | u8 | Set by the thrust step of the current ship; all of DS:3BE2..3BEA are cleared after each live ship (and at tick start) |
| Player tile | DS:9766 / DS:9767 | u8 | (view X + 5, view Y + 3), written at the start of `AI_UpdateShips` |
| AI range | DS:976B | u8 | 3 (set once by the `34E6` unit initialisation) |
| Detour step | DS:976C / DS:976E | i16 | (dx, dy) returned by the detour step to `AI_ChooseWaypoint` |
| Blocked(x, y) | `34E6:02AD` | bool | True if any of the four map cells (x, y), (x + 1, y), (x, y + 1), (x + 1, y + 1) has tile attribute ≠ 2 ([structures §8](../../re/structures.md), the same table as the player's collision). Coordinates are bytes (x − 1 at x = 0 is 255) |
| D(s) | | i16 | Squared tile distance between the waypoint and the target position: `(tx − wx)² + (ty − wy)²` computed in 16 bits and wrapped to a signed 16-bit value; the original compares `Sqrt(|D|)` (Real48) with small integers |
| sign(a → b) | | i16 | 0 if a = b, +1 if a < b, −1 if a > b |

## 3. Data

### 3.1 Ship record (`record[s]` at DS:4432 + (s − 1) × 1065)

Only the fields below are used. Everything else stays 0 (BSS) for the whole session. "AI" = only `34E6` reads or writes it.

| Offset | Type | Our name | Meaning |
|---:|---|---|---|
| +000h | u8 | fixedDone | Cleared by the dead fixed-point mode (§4.3.9) |
| +001h, +002h | u8 | waypointX, waypointY | Tile the ship is flying to |
| +003h, +004h | u8 | prevX, prevY | Waypoint before the last waypoint step |
| +005h | u8 | blockedY | Direct steps stopped on the Y axis (AI) |
| +006h | u8 | blockedX | Direct steps stopped on the X axis (AI) |
| +007h | i16 | stepX | Direct step on X: −1, 0, +1 (AI) |
| +009h | i16 | stepY | Direct step on Y (AI) |
| +00Bh | u16 | pathCursor | Index k of the last path step reached. It is also "path step 0" (path step k lives at +00Bh + 2k) |
| +00Dh + 2(k − 1) | u8 × 2 | path[k] | (x, y) of path step k = 1..500 (copied from the mission file; [structures §2](../../re/structures.md)) |
| +3F5h | u8 | moveDir | Direction of the last waypoint step: 2, 4, 6 or 8 (vertical wins); also the facing of the wall follower. 0 after a record reset |
| +3F6h | u8 | aggression | Fire-budget unit, set per flight ([mission-setup](mission-setup.md)); 0 = never fires and disengages at once |
| +3F8h | i16 | shields | |
| +3FAh | i16 | armour | |
| +3FCh | u8 | holdFlag | Set by the in-line check (§4.3.4); selects the hold schedule |
| +3FDh.. | u8 | holdList | Read by the hold schedule; never written (all 0) |
| +411h | u8 | holdIndex | |
| +412h | u8 | holdCount | |
| +413h | u8 | nearLimit | Always 0 |
| +414h | u8 | nearFlag | Always 0 (written, never read) |
| +415h | u8 | attackMode | 1 = attack the target |
| +416h | u16 | attackCounter | Attack budget counter |
| +418h | u8 | followerFlag | Written, never read |
| +419h | u8 | handBit | Random 0/1 per slot; chooses the wall follower |
| +41Ah | u8 | detourSteps | Waypoint steps taken in detour mode |
| +41Bh | u8 | directMode | 1 = step straight toward the target; 0 = detour |
| +41Ch | u8 | inLine | Target in line and in range (§4.3.4) |
| +41Dh | u8 | outOfRange | Target more than 11 tiles away |
| +41Eh | u8 | holdTarget | Never written (0) |
| +41Fh | u8 | targetSel | 0 = player, 1..20 = that slot's waypoint, FFh = return point, C8h = fixed point |
| +420h, +421h | u8 | targetX, targetY | Position of the target (§4.3.2) |
| +422h | u8 | fixedMode | Never set in `file0001` (0) |
| +423h | u8 | fixedReached | Dead |
| +424h, +425h | u8 | fixedX, fixedY | Never set (0) |
| +426h, +427h | u8 | returnX, returnY | Waypoint the ship last left while following its path |
| +428h | u8 | returnMode | 1 = fly to the return point |

`+3F3h/+3F4h` are path step 500. Path step 501 would be (+3F5h, +3F6h) = (moveDir, aggression) (§5).

### 3.2 Constants

| Constant | Value | Where |
|---|---|---|
| Speed cap | 42 (2Ah) | `3292:0011` |
| In-line range | D < 9 (Sqrt(D) < DS:976B = 3) | `34E6:0564`; C-0009 has two D = 9 cases, both "not in line" |
| Out-of-range | Sqrt(D) > 3 + 8 = 11 | `34E6:06A1` |
| Detour length | 5 waypoint steps (detourSteps counts 0..4) | `34E6:0E84` |
| Attack budget | 15 × aggression | `34E6:0DCE` |
| Fire chance | 5 × aggression > 100 × Random + 1 | `34E6:0A5D` (Real48 constants 100 and 1) |
| Exit marker | waypoint (50, 0) or (50, 1) | `3292:0011` pass 1 |

## 4. Behaviour

### 4.1 Setup (flight start, `Sim_FlyMission`)

In this order ([tick-order §4](tick-order.md)):

1. `AI_ResetShipRecords` (`3292:0000` → `34E6:0000`, at `0E8B:B8BA`, first thing in setup). For s = 1..20: clear path steps 1..255 of the record, and the fields +000h..+00Ch, +3F5h, +3F6h, +3F8h, +3FCh, +412h..+417h, +41Ch, +41Dh, +41Fh..+428h to 0; then **draw `Random(2)`** and set handBit := 1 if it returned 1, else 0. Fields +411h, +418h, +41Ah, +41Bh, +41Eh are *not* cleared (they keep values from the previous flight). The same routine also runs once at program start (unit initialisation, before `RandSeed` is ever changed), which consumes the first 20 `Random(2)` draws of the session. (RNG: [bp7-runtime-semantics](../core/bp7-runtime-semantics.md).)
2. Load `m<n>.td` (not in flight 21): classes and path arrays ([structures §2](../../re/structures.md)); slot 2's FFh path bytes become 0. Flight 21 loads nothing, so steps 3 and 4 read whatever the path arrays' memory holds: §4.1.1.
3. For s = 1..20 (every slot, including class FFh): shields := `k_ClassShields[class]`, armour := 2 × `k_ClassArmour[class]` (class FFh reads table bytes past the end: DS:03D1 and DS:03C7). Copy path steps k = 1..500 into the record. waypoint := path[1]; returnX/Y := waypoint.
4. For s = 1..20: tile := path[1]; heading := 6; target heading := 6; offset X := offset Y := 16; all four speeds := 0. (Also DS:3A3E − 1 + s := 1, never read.)
5. Per-flight setup `Msn_Setup` (`0E8B:60D8`): player class, slot classes, markers, aggression, armour, class-table patches ([mission-setup](mission-setup.md)).
6. Start view, palette, sounds (not ship-related), then **for s = 1..20 (all slots): `AI_ChooseWaypoint(s)`** (`0E8B:CC63..CC79`). In path mode this sets pathCursor := 1 and prevX/Y := returnX/Y := path[1] without moving the waypoint (§4.3.5).

Classes changed by setup take effect for shield regeneration (cap = new class) but not for the armour and shields already set in step 3: in flight 1, slots 14–17 become class 7 with 0 shields (their file class was 1) and regenerate to 25 in the first 25 ticks (C-0009).

Setup writes no turn direction (U-501): DS:3B1A − 1 + s keeps the previous flight's values (0 only in the first flight of a program run); the only writer is §4.2.2. From the setup heading 6 every target change of §4.2.2 writes it, so the carried value changes a ship's course only where mission logic sets a target heading without it (E2 flight 13's `Random(9)` target heading of slot 2, [e2-mission-logic §5.13](e2-mission-logic.md)); otherwise it is state that a byte comparison sees (C-0202: E3 flight 21 after flight 11, every slot 1..15 holds 1 or 2 at setup). Port: keep the 20 values in the session.

#### 4.1.1 Flight 21's path arrays (E1, U-500)

Flight 21 has no mission file, yet setup steps 3 and 4 copy the two path arrays into the records and place every slot (class FFh or not) on its path[1]. Those arrays are heap blocks, and their contents at flight 21 are left over from flight 19 and the screens between the two flights. The rule below reproduces both captured cases byte for byte (C-0200, C-0300: all 20,040 bytes except slot 20's Y steps 482..500, which no live slot uses).

**Heap layout.** `Sim_FlyMission` marks the heap top at entry (`BP_Mark` into DS:16EA, after freeing the music buffer) and allocates its buffers from there in a fixed order, so every flight of a program run puts them at the same offsets from that mark M: the title font (975 bytes), the eight projectile arrays, `g_FacFrames`, `g_WeaponIcons`, then **path X at M + 9,328 and path Y at M + 19,352** (10,020 bytes each, `array[1..20, 0..500]`), then the rotation sets and the sprite bank. At the flight's end `BP_Release` gives everything back to M. The front end then uses the same memory from M, in last-in first-out order:

| Writer | Where (offset from M) | What it writes |
|---|---|---|
| `Snd_PlayMusic(n)` (only with music on; `0E8B:07ED` frees the old song, `WORX_LoadMusic` `2C37:0273` allocates and reads) | 0 .. size − 1 | the whole CMF file; m := its size |
| every screen that loads the small font: scene cards, dialogue scenes, the debrief (`GetMem(975)` into `g_Font5`) | r(m) .. r(m) + 949 | `td.lt2` (950 bytes; the last 25 bytes of the block are not written) |
| the same screen at its end (`BP_FreeMem` of the font block) | r(m) + 4 .. r(m) + 7 | 00 00 3D 00 (the freed block's rounded size, 976 bytes, as BP7's free-list size field) |
| every portrait load (`Dlg_LoadPortrait` `051A:1618`: dialogue speaker lines with face ≠ 200, [dialogue-player §4.4](../narrative/dialogue-player.md); the debrief's face 1 on the right side, `051A:70E6`) | r(m) + 976 .. r(m) + 16,975 | record face − 1 of `newfaces.td`: each pixel p becomes 0 if p ≥ 112, then + 112 on the right side (slot 2) |
| the end of that portrait load (`BP_FreeMem`) | r(m) + 980 .. r(m) + 983 | 00 00 E8 03 (the block's size, 16,000 bytes, as the free-list size field) |
| a flight's mission-file load | 9,329 .. 19,328 and 19,353 .. 29,352 | the X and Y blocks of `m<n>.td` (element (1, 1) on), then slot 2's FFh → 0 |

r(m) rounds m up to a multiple of 8 (BP7 allocates in 8-byte units); m = 0 while no song is loaded (music off). Freeing a block writes only its size field (bytes 4..7) when the block is the top of the heap, which is always the case here (`BP_HeapFree` `3A9A:041E`).

**Port rule.** Keep a 29,372-byte image H of the heap from M (only offsets 9,328 and up matter) and apply the writers above in program order from flight 19's mission-file load to flight 21's setup. In block 19 ([episode-program-flow §4.4](../flow/episode-program-flow.md)) the screens between the two flights are always M8, the debrief, M5, `C:voffice`, then, unless the skip flag jumps to flight 21 earlier, D88, M3, `C:hangar` and D89. Flight 21's path step k of slot s is then H[9,328 + 501 (s − 1) + k] (X) and H[19,352 + 501 (s − 1) + k] (Y); everything else in setup is as for any flight. Bytes that none of these writers touched (slot 20's Y steps 482..500 with music on; more with music off) keep what the heap held before flight 19: never used by a live slot in the captured cases, port: 0.

| # | Path between the flights | Path X / Y sources (offsets from M) | Step-1 tiles of slots 1..10 (flight 21) |
|---|---|---|---|
| 1 | C-0200: debrief times out, Esc at `C:voffice` | X: `vulture.cmf` bytes 9,328..12,298, the debrief portrait, the `C:voffice` font block and its size field, the debrief portrait again; Y: the debrief portrait (to 21,271), then `m19.td`'s Y block | (0, 112), (0, 137), (0, 164), (69, 164), (145, 64), (0, 59), (0, 108), (247, 88), (133, 88), (130, 75) |
| 2 | C-0300: no Esc (D88: faces 24 left, 11 right; D89: faces 1 left, 2 right) | X: D89's last portrait (face 2, right) at 2,960..18,959 (`hangar.cmf` is 1,980 bytes, so r(m) = 1,984); Y: D88's last portrait (face 11, right; `vulture.cmf` playing, r(m) = 12,304) at 13,280..29,279, then `m19.td` | (118, 112), (112, 112), (112, 112), (122, 221), (112, 220), (133, 138), (134, 112), (112, 112), (120, 112), (112, 112) |
| 3 | C-0323 (C-0322): as C-0300 but **music off** (`td.cfg` 01 02 02: no song is ever loaded, m = 0) | X: D89's last portrait (face 2, right) at 976..16,975, i.e. path X of slots 1..16 (slot 16 in part); slots 17..20 keep `m19.td`'s X; Y: `m19.td`'s Y block whole (nothing reaches offset 19,352) | (120, 80), (112, 86), (134, 79), (133, 67), (112, 64), (112, 59), (112, 108), (112, 88), (112, 88), (112, 75) |

So the ships of flight 21 start wherever the bytes of a portrait, a song or a font put them, mostly at tile X 0 or 112..134; slots outside the map are removed by the edge test of §4.2.3 as in any flight. Classes, armour and shields are as for any flight (flight 19's classes at its end, then flight 21's `Msn_Setup` row).

### 4.2 Per tick (`AI_UpdateShips(flight)`, `3292:0011`)

```text
playerTileX := low byte(viewTileX + 5);  playerTileY := low byte(viewTileY + 3)       (DS:9766/9767)

pass 1, for s = 1..20 with class(s) != FFh:
    if shields(s) < k_ClassShields[class(s)]: shields(s) += 1
    if (waypointX = 50 and waypointY < 2) or armour(s) < 1: class(s) := FFh

pass 2, for s = 1..20 with class(s) != FFh (tested once, before the body):
    (a) tile adjustment through the pointer at DS:3A32: no effect, see 5 (OB-013)
    (b) if (waypointX, waypointY) = (tileX, tileY) and not (flight = 20 and s <= 5):
            AI_ChooseWaypoint(s)                                             (4.3)
    (c) goalX := waypointX; goalY := waypointY
    (d) turn: if target heading != heading: find heading in the ring k_HeadingRing[1..16]
            (DS:02AD..02BC); if found at index i: heading := ring[i - 1] when turn = 1,
            ring[i + 1] when turn = 2 (ring[0] = 01h, ring[17] = 0Eh wrap); not found: no turn
    (e) if flight = 20 and s <= 5: heading := 8; offsetY += 2          (no thrust, no new target heading)
        else: thrust toward the goal (4.2.1), then choose the target heading (4.2.2)
    (f) every speed component > 42 := 42
    (g) drift, drag, carry and edge removal (4.2.3)
    (h) flight 7: pin slots 2..12 (4.2.4)
    (i) draw the ship (4.7)
    (j) clear the nine bytes DS:3BE2..3BEA
```

Because (b) happens before the turn and the thrust, a ship reacts to a new waypoint in the same tick; because (d) uses the target heading chosen in the previous tick, a new target heading takes effect one tick later (C-0009 slot 1: target heading 2 is set in tick 1, the heading steps 6 → 24h → 3 → 20h → 2 in ticks 2–5).

#### 4.2.1 Thrust

`max := k_ClassMaxSpeed[class]`, `acc := k_ClassAccel[class]` (both read every tick: setup patches apply, [mission-setup](mission-setup.md)). U, D, L, R are the four speed bytes; arithmetic is 8-bit unsigned (a brake can leave a component as low as 13, which counts as still and resumes from there). The four blocks run in this order; each tests the goal against the ship tile **before** this tick's movement:

```text
if goalY < tileY and U < max:                    (target above)
    U += acc
    if D > 16 and goalY <= tileY: U -= acc        (original bug: brakes U, not D; OB-014)
    if L > 16 and tileX <= goalX: L -= acc
    if R > 16 and goalX <= tileX: R -= acc
    flagUp := 1
if tileY < goalY and D < max:                    (target below)
    D += acc
    if U > 16 and tileY <= goalY: U -= acc
    if L > 16 and tileX <= goalX: L -= acc
    if R > 16 and goalX <= tileX: R -= acc
    flagDown := 1
if goalX < tileX and L < max:                    (target left)
    if U > 16 and tileY <= goalY: U -= acc
    if D > 16 and goalY <= tileY: D -= acc
    if R > 16 and goalX <= tileX: R -= acc
    L += acc
    flagLeft := 1
if tileX < goalX and R < max:                    (target right)
    if U > 16 and tileY <= goalY: U -= acc
    if D > 16 and goalY <= tileY: D -= acc
    if L > 16 and tileX <= goalX: L -= acc
    R += acc
    flagRight := 1
```

A component that is at `max` gets no thrust and its block does not run (no braking, no flag). With the goal on the ship's row (column) both vertical (horizontal) components are braked by the horizontal (vertical) blocks.

#### 4.2.2 Target heading and turn direction

`H` is the current heading. Only single-digit headings are listed: when `H` is a two-digit code the turn direction keeps its previous value. "cw" = turn := 2, "ccw" = turn := 1.

| Flags set this tick | Target heading | cw when H in | ccw when H in | Condition |
|---|---|---|---|---|
| up and right | 9 | 7, 8, 4, 1 | 6, 2, 3 | — |
| up and left | 7 | 2, 4, 1 | 9, 6, 8 | — |
| up only | 8 | 7, 4, 1 | 9, 6, 3, 2 | only if H ≠ 8 (else nothing changes) |
| down and right | 3 | 9, 8, 6, 7 | 1, 2, 4 | — |
| down and left | 1 | 2, 3, 6, 9 | 8, 7, 4 | — |
| down only | 2 | 9, 6, 3 | 7, 4, 1, 8 | only if H ≠ 2 |
| left only | 4 | 3, 2, 1 | 9, 8, 7, 6 | only if H ≠ 4 |
| right only | 6 | 9, 8, 7, 4 | 3, 2, 1 | only if H ≠ 6 |
| none | unchanged | | | |

(The original evaluates the "cw" set first and the "ccw" set second; heading 9 appears in both sets for target 7 and ends as ccw.)

#### 4.2.3 Drift, drag, carry and edge removal

Offsets are i16, tiles u8. In this order:

```text
while offsetX > 80 or offsetX < -16: (offsetX > 80: offsetX -= 32); (offsetX < -16: offsetX += 32)
if U > 16: offsetY += 16 - U;  U -= 1
if offsetY < 16 and tileY > 1: tileY -= 1; offsetY += 32
if offsetY < 16 and tileY = 1: class := FFh
if D > 16: offsetY += D - 16;  D -= 1
if offsetY > 48 and tileY < 199: tileY += 1; offsetY -= 32
if tileY = 200 or tileX = 200 or tileX = 0 or tileY = 0: class := FFh
if L > 16: offsetX += 16 - L;  L -= 1
if offsetX < 16 and tileX > 1: tileX -= 1; offsetX += 32
if offsetX < 16 and tileX = 1: class := FFh
if R > 16: offsetX += R - 16;  R -= 1
if offsetX > 48 and tileX < 199: tileX += 1; offsetX -= 32
if offsetX > 48 and tileX = 199: class := FFh
if offsetX > 48: offsetX -= 32                     (no tile change)
```

Drag: each component above 16 loses 1 after it moved the ship (as the player, [flight-model §4.2](flight-model.md)). A ship removed here still finishes steps (h)–(j) of this tick: if it is inside the view window it is drawn once more, with sprite number FFh (the class byte is passed unchanged; in practice the map edge is outside the view, which stays within tiles 6..183 × 7..190). There is **no map collision** for ships: they fly over solid tiles; only the AI keeps the waypoints on open cells (§4.3). Y never wraps at row 199 (offsetY grows), and there is no removal at row 199.

C-0009, slot 1 (class 0, max 24, acc 2), down speed per tick from setup: 0, 2, 4, … 16 (tick 8), then 17 (offset Y 16 → 18), 18 (→ 21), 19 (→ 25), 20 (→ 30), 21 (→ 36), 22 (→ 43), 23 (→ 51 → carry: tile Y 73 → 74, offset 19), 24 (→ 28), 23 (→ 36; no thrust at max) …

#### 4.2.4 Flight-specific rules

| Flight | Rule | Where |
|---|---|---|
| 7 | After each live ship's movement, slots 2..12 are reset to fixed tiles with offsets (16, 16), heading 8 and target heading 8: 2 (50, 182), 3 (50, 167), 4 (50, 123), 5 (79, 122), 6 (111, 122), 7 (151, 122), 8 (184, 122), 9 (13, 122), 10 (18, 124), 11 (50, 101), 12 (50, 36) (stationary objects; `mine.td` sprites). Their AI and speeds still run but their position never changes | `3292:0011` |
| 20 | Slots 1..5 never call the AI; heading := 8 and offset Y += 2 every tick (they sink 2 px per tick, carrying into the next row at 48) | `3292:0011` |
| 19 | After `AI_UpdateShips`, when the mission phase DS:32BA > 1: player shields capped at 10, and slot 19's tile and offsets are copied from DS:16EE/16F0/16F2/16F4 (set by mission logic) | `0E8B:D646..D675` |

### 4.3 AI (`AI_ChooseWaypoint(s)`, `34E6:3449`)

Runs at setup for every slot and in tick step (b) when the ship has reached its waypoint. All tests read the record as it is at that moment (earlier steps in the same call change later tests).

```text
ChooseWaypoint(s):
    TargetPosition(s)                                            4.3.2
    (if returnMode = 0: nearFlag := 0  — the "near" test compares with +413h = 0, never true)
    if holdFlag = 1:                                     HoldSchedule(s)       4.3.6
    if attackMode = 0 and returnMode = 0 and holdFlag = 0 and fixedMode = 0:  PathStep(s)   4.3.5
    if attackMode = 1 and directMode = 1 and fixedMode = 0:  AttackStep(s)         4.3.3
    elif attackMode = 1 and directMode = 0 and fixedMode = 0:  DetourMove(s)       4.3.7
    if returnMode = 1 and directMode = 1 and fixedMode = 0:  ReturnStep(s)         4.3.8
    elif returnMode = 1 and directMode = 0 and fixedMode = 0:  DetourMove(s)
    (fixedMode = 1 branches: dead, 4.3.9)
    SetMoveDir(s)                                                                4.3.1

DetourMove(s):  (dx, dy) := Detour(s)      (via DS:976C/976E)
                prev := waypoint; waypoint += (dx, dy)
```

Note that an attack step can switch the ship to return mode (budget exhausted, or the hold schedule) and the return branch then runs **in the same call**, so a waypoint can move twice in one call (C-0009 slot 19, tick 230).

#### 4.3.1 Waypoint steps and moveDir

Every waypoint change (except the path step's own write, which also records the return point) is: `prev := waypoint; waypoint := waypoint + (dx, dy)` with byte wrap. `SetMoveDir` (`34E6:0869`) at the end of every call: if waypoint = prev nothing changes; otherwise moveDir := 4 if waypointX < prevX, then 6 if prevX < waypointX, then 8 if waypointY < prevY, then 2 if prevY < waypointY (later tests overwrite earlier ones).

#### 4.3.2 Target position (`34E6:0396`)

| targetSel | (targetX, targetY) := |
|---|---|
| 0 | player tile (DS:9766, DS:9767) |
| FFh | (returnX, returnY) |
| C8h | (fixedX, fixedY) (never used) |
| anything else n | (waypointX, waypointY) of record[n] |

#### 4.3.3 Attack step (`34E6:2448`)

```text
AttackStep(s):
    InLineCheck(s)          inLine := holdFlag := (D < 9 and (waypointX = targetX or waypointY = targetY)) ? 1 : 0
    Budget(s)               if 15 * aggression < attackCounter:            (16-bit unsigned)
                                attackCounter := 0; detourSteps := 0; directMode := 1
                                returnMode := 1; targetSel := FFh
                            else attackCounter += 1
    FireDecision(s)         4.4.1
    TargetPosition(s)       (targetSel may have changed to FFh)
    if inLine = 0:
        detourSteps := 0
        DirectStep(s, attack = true)
    (inLine = 1: the waypoint does not move: the ship holds its position and fires)

DirectStep(s, attack):
    if blockedX = 0: stepX := sign(waypointX -> targetX)
    if blockedY = 0: stepY := sign(waypointY -> targetY)
    dx := 0; dy := 0
    if not Blocked(waypointX + stepX, waypointY): dx := stepX  else: stepX := 0; blockedX := 1
    if not Blocked(waypointX, waypointY + stepY): dy := stepY  else: stepY := 0; blockedY := 1
    if (blockedY = 1 and stepY = 0) or (blockedX = 1 and stepX = 0) or (attack and directMode = 0):
        (dx, dy) := Detour(s)                                              4.3.7
    prev := waypoint; waypoint += (dx, dy)
```

`InLineCheck` and the first range tests use the target position computed at the start of `ChooseWaypoint`, not the one recomputed after `Budget`.

#### 4.3.4 Distances

D is the 16-bit wrapped squared distance of §2. "D < 9" stands for the original `Sqrt(|D|) < 3` and "D > 121" for `Sqrt(|D|) > 11` (Real48). The two are equivalent for every D the original can produce if Real48 `Sqrt` is exact on 9 and 121 (U-161; the D = 9 boundary is confirmed by C-0009).

#### 4.3.5 Path step (`34E6:2FAF`)

```text
PathStep(s):
    returnMode := 0
    c := pathCursor;  (nx, ny) := path[c + 1]
    dx := sign(waypointX -> nx); dy := sign(waypointY -> ny)
    prev := waypoint; return := waypoint
    waypoint += (dx, dy)
    if waypoint = (nx, ny):
        pathCursor := c + 1
        if path[c + 2].x = 0: pathCursor := 1
```

The waypoint walks one tile per call (diagonally while both axes differ) toward the next path step, so path steps far apart (the files have jumps of up to 198 tiles) are flown in straight-ish tile steps. When the following step has x = 0 the cursor wraps to 1 and the ship flies back toward path[2] from where it is: **paths loop**. A path whose next step is (50, 0) or (50, 1) makes the ship walk its waypoint there; the ship is removed in pass 1 as soon as its waypoint equals the marker. Path steps are read through the cursor word, so path[0] is the cursor itself and path[501] is (moveDir, aggression) (§5).

#### 4.3.6 Hold schedule (`34E6:3275`)

Runs when holdFlag = 1 (set by an in-line check during an attack step). With the list bytes all 0 and holdTarget = 0 it reduces to:

```text
HoldSchedule(s):
    i := holdIndex
    if byte(+3FCh + i) < holdCount:            (i = 0 reads holdFlag itself, = 1)
        if byte(+3FDh + i) = 0:
            holdIndex := 1; returnMode := 1; directMode := 1; targetSel := FFh
            holdFlag := 0; holdCount := 0; attackMode := 0
        else:                                   (never in file0001: the list is empty)
            holdIndex += 1; attackMode := 1; targetSel := holdTarget; directMode := 1; returnMode := 0
    else:
        holdCount += 1; returnMode := 1; directMode := 1; targetSel := FFh
```

Effect: the first call after a ship got in line sets return mode while attack mode stays on, so in the same `ChooseWaypoint` the attack step (in-line test against the *old* target, fire decision) **and** the return step run; the next call with holdFlag still 1 ends the attack and leaves the ship in return mode. C-0009 slot 19, ticks 110–115: in line at (105, 107) east of the player, fire request 6 (counter 118 → 169), then return toward (92, 111) while the script keeps re-ordering the attack.

#### 4.3.7 Detour (`34E6:23B2`) and the wall followers

```text
Detour(s) -> (dx, dy):
    if (handBit = 1 and returnMode = 0) or (handBit = 0 and returnMode = 1): (dx, dy) := FollowRight(s)
    else:                                                                  (dx, dy) := FollowLeft(s)
    directMode := 0
    if detourSteps < 4: detourSteps += 1  else: detourSteps := 0; directMode := 1
```

So a ship detours for five waypoint steps and then tries the direct step again. The followers work on the waypoint (x, y) and the facing d = moveDir; "ahead", "side" and the step use the cells next to the waypoint:

| d | Ahead cell | Step | Right-hand side | Left-hand side | Turn right to | Turn left to |
|---|---|---|---|---|---|---|
| 6 (E) | (x + 1, y) | (+1, 0) | (x, y + 1) | (x, y − 1) | 2, step (0, +1) | 8, step (0, −1) |
| 2 (S) | (x, y + 1) | (0, +1) | (x − 1, y) | (x + 1, y) | 4, step (−1, 0) | 6, step (+1, 0) |
| 4 (W) | (x − 1, y) | (−1, 0) | (x, y − 1) | (x, y + 1) | 8, step (0, −1) | 2, step (0, +1) |
| 8 (N) | (x, y − 1) | (0, −1) | (x + 1, y) | (x − 1, y) | 6, step (+1, 0) | 4, step (−1, 0) |

```text
FollowRight(s):                                   (FollowLeft: swap "Right"/"Left" everywhere)
    1. re-orient (34E6:1050 / 12C2): with d0 := moveDir, apply in order, each test on d0,
       the later ones overwriting:
          right hand: d0=6 & Blocked(x, y-1) -> 4;  d0=4 & Blocked(x, y+1) -> 6
                      d0=2 & Blocked(x+1, y) -> 8;  d0=8 & Blocked(x-1, y) -> 2
                      d0=6 & Blocked(x+1, y) -> 8;  d0=4 & Blocked(x-1, y) -> 2
                      d0=2 & Blocked(x, y+1) -> 6;  d0=8 & Blocked(x, y-1) -> 4
          left hand:  d0=6 & Blocked(x, y+1) -> 4;  d0=4 & Blocked(x, y-1) -> 6
                      d0=2 & Blocked(x-1, y) -> 8;  d0=8 & Blocked(x+1, y) -> 2
                      d0=6 & Blocked(x+1, y) -> 2;  d0=4 & Blocked(x-1, y) -> 8
                      d0=2 & Blocked(x, y+1) -> 4;  d0=8 & Blocked(x, y-1) -> 6
       moveDir := result; d := moveDir
    2. one action (dx = dy = 0 before it):
       if Blocked(side(d)) and not Blocked(ahead(d)):  (dx, dy) := step(d)            keep facing
       elif not Blocked(side'(d)):                     turn toward the hand; (dx, dy) := its step
       elif Blocked(ahead(d)):                         turn away from the hand; (dx, dy) := its step
       side'(d) = side(d), except for the RIGHT-hand follower facing south (d = 2), where it is the
       cell ahead (x, y + 1) (OB-015)
    3. release (34E6:153E), on the waypoint before the step:
       if waypointX <= targetX and d in {8, 2} and not Blocked(x + 1, y): blockedY := 0
       if targetX <= waypointX and d in {2, 8} and not Blocked(x - 1, y): blockedY := 0
       if waypointY <= targetY and d in {6, 4} and not Blocked(x, y - 1): blockedX := 0
       if targetY <= waypointY and d in {4, 6} and not Blocked(x, y + 1): blockedX := 0
       followerFlag := (blockedX or blockedY) ? 1 : 0                          (never read)
```

The three cases of step 2 always cover every situation when d is 2, 4, 6 or 8 (one of them applies). moveDir is 0 only for a ship whose waypoint has never moved; a detour cannot start before a waypoint step (an attack step moves or holds first), so the original's endless loop for an invalid facing cannot occur in practice; the port should treat it as "no step" (U-162).

C-0009 exercises both followers (about 120 right-hand and 220 left-hand calls).

#### 4.3.8 Return step (`34E6:280B`)

```text
ReturnStep(s):
    attackMode := 0
    TargetPosition(s)
    if waypoint = (targetX, targetY):
        returnMode := 0; targetSel := 0; nearFlag := 0           (back to path following next call)
    else:
        DirectStep(s, attack = false)                            (no "directMode = 0" clause)
```

The return point is the waypoint the ship last left in path mode, so after an attack the ship flies back to its path and resumes it at the same cursor.

#### 4.3.9 Dead code

Mode `fixedMode` (+422h) is never set by any code in `file0001` (only cleared), so `34E6:2C0D` (step toward (fixedX, fixedY)) and `34E6:04EC` never run, and the "hold list" branch of 4.3.6 never runs. A port need not implement them.

#### 4.3.10 Attack orders (`AI_SetTarget(slot, target)`, `34E6:3232`)

The only way into attack mode. Called by the mission logic (`2404`, 389 call sites; RE-6's mission-logic spec, in progress): `targetSel := target; attackMode := 1; directMode := 1`. It does not move the waypoint; the attack starts at the ship's next waypoint arrival. The scripts call it repeatedly, so `directMode` is often reset to 1 in the middle of a detour (C-0009: slot 19 ticks 34, 42, 49, 112, 116; the mission logic, not the AI, decides when). Mission logic also activates and removes ships by writing `g_SlotClass` directly, including `2404:05DC(slot, x, y)`: class := FFh when the ship's tile is (x, y) (in flight 1, slots 9–11 are removed at tick 1).

**When the target dies, hides or leaves the range (U-181).** Nothing in the AI ends an attack early. `TargetPosition` (`34E6:0396`) reads the target's record waypoint (or the player's tile for target 0) without looking at the target's class, and `AI_UpdateShips` no longer updates a ship whose class is FFh, so after the target is destroyed or hidden the attacker keeps flying to, holding at and firing toward the target's last waypoint. The mission rules that re-apply the order (`engage a → b, r` = `if Msn_NearLive(a, b, r) then AI_SetTarget(a, b)`, [mission-logic](mission-logic.md) §3.1) stop calling it once the target is dead or out of range, but stopping the calls does not cancel the attack. The attack ends only through the budget (§4.3.3: after 15 × aggression attack steps the ship switches to return mode, flies back to the point where it left its path and resumes the path at the same cursor, §4.3.8) or the hold schedule (§4.3.6). `AI_SetTarget` writes only `targetSel`, `attackMode` and `directMode`; it does not reset `attackCounter`, so re-applying the order every tick does not extend the budget. If the rule still holds when the budget has sent the ship home, the next order sets `attackMode` and `targetSel` again while `returnMode` is still 1: the next `ChooseWaypoint` then runs the attack step and the return step in the same call (§4.3), and the return step steps toward the attack target (`targetSel` is no longer FFh) until the waypoint reaches it. Static (`34E6:0396..04EB` has no class test; `34E6:3232`); the attack and return machinery is CONFIRMED by C-0009, the dead-target case is not captured.

### 4.4 Firing

#### 4.4.1 Fire decision (`34E6:0A5D`, inside the attack step)

```text
FireDecision(s):
    outOfRange := (D > 121) ? 1 : 0                          (target position from the start of the call)
    r := Random (BP7 real in [0, 1)) * 100 + 1                (always drawn: one RNG step per attack step)
    if not (5 * aggression > r and outOfRange = 0): return
    vBlocked := false
    if waypointX = targetX: for i = 0 .. |targetY - waypointY|: if Blocked(waypointX, waypointY + i): vBlocked := true
    hBlocked := false
    if waypointY = targetY: for i = 0 .. |targetX - waypointX|: if Blocked(waypointX + i, waypointY): hBlocked := true
    if not vBlocked and targetX in {waypointX - 1, waypointX, waypointX + 1}:
        RequestFire(s, 2 if waypointY < targetY else 8)
    if not hBlocked and targetY in {waypointY - 1, waypointY, waypointY + 1}:
        RequestFire(s, 6 if waypointX < targetX else 4)

RequestFire(s, e):  fireRequest[s][e] := 1;  attackCounter += aggression
```

The line-of-fire scan always goes toward increasing coordinates, even when the target is above or to the left (OB-016). With aggression ≥ 21 the random test always passes (the draw still happens); with 0 it never does.

#### 4.4.2 Spawning (tick-order step 30, `0E8B:F2D6..F4A6`) and clearing (step 31)

Every second tick (when `g_DrawPage` is 1 after the page toggle):

```text
for s = 1..20:
    for e = 2..8 with fireRequest[s][e] = 1:
        for m = 1..17:
            slot := the highest free projectile slot in 0..40 (scanned again for every m); none: skip m
            if shotTable[class(s)][m] != 0:
                life := 1; owner := s; tile := (tileX(s), tileY(s)); heading := heading(s)
                kind := shotTable[class(s)][m] - 1
                Snd_PlayEffect(kind = class(s), at the ship's tile, listener = view tile)
                offsetX := muzzle[heading(s)][m].dx + offsetX(s) - 16
                offsetY := muzzle[heading(s)][m].dy + offsetY(s) - 16
then (every tick) clear fireRequest[1..20][1..9]
```

The request entry only says "fire": the shot flies along the ship's **current heading**, not toward the target. A ship that has just turned (or is mid-turn) fires along that heading; when the heading is a two-digit code the muzzle bytes come from DS:035C + 34 × heading + 2m, far outside the table (other globals, OB-017), and the shot is deleted by its first projectile move ([weapons §4.3](weapons.md) step 8) after one hit test. Which globals those are, what they hold during a flight (constants on the four southern and western intermediate headings; the scene palette and the dialogue reader's last line, as the front end left them, on the four northern ones) and the port rule: [weapons §3.3](weapons.md) (RE-15, U-160). In C-0009, 8 of 51 enemy volleys left on two-digit headings; in VE-2's 20 whole flights, 230 enemy shots (and 292 player shots). A request raised on a tick where the page is 0 is lost. Muzzle tables, shot kinds, damage: [weapons](weapons.md).

### 4.5 Destruction and removal

A slot's class becomes FFh (the ship is gone for the rest of the flight unless mission logic writes a class again) when:

| Cause | Where |
|---|---|
| Armour < 1 at the start of a tick | pass 1 |
| Waypoint (50, 0) or (50, 1) (exit marker) | pass 1 |
| Destroyed by a shot (armour reaches 0) | `0E8B:0984`, [weapons §4.3](weapons.md) step 4 (C-0009: slot 1 at tick 254) |
| Map edge: tile Y reaches row 1 going up, row 200, column 0, 1 or 200, column 199 going right | §4.2.3 |
| Mission logic | `2404` (e.g. `2404:05DC`; RE-6) |
| Cheat / switch screens | `0E8B:9860` ship switching (RE-6) |

No ship–ship or ship–player contact damage or push exists (U-047): the only writers of ship and player armour/shields are the projectile routine, setup, shield regeneration, mission logic (`2404:2654`), the ship-switch panel (`0E8B:9860`), flight 19's cap and the cheat chords.

### 4.6 Records not modelled by the movement

A removed ship keeps its record and slot values (tile, offsets, speeds); nothing reads them except mission logic and the HUD (radar), which skip class FFh. `fireRequest` entries of a removed ship are cleared in step 32 of the same tick.

### 4.7 Drawing (T2, `3292:0011` step (i))

A live ship is drawn when `viewX − 1 < tileX < viewX + 10`, `viewY ≤ tileY ≤ viewY + 5`, (`tileX < viewX + 9` or offsetX < viewSubX), and `(tileY − viewY) × 32 + offsetY + 232 × drawPage > 32`. The sprite is drawn with the per-heading blit of [structures §7](../../re/structures.md) (`386E:024A` for heading 8 … `386E:172D` for 0Ch) at page position x = (tileX − viewX) × 32 + offsetX, y = (tileY − viewY) × 32 + offsetY, with **sprite number = class code** (U-042). These are the same routines and sprite numbering the player uses (`g_PlayerSprite` = class), so ship class c looks like player class c; which image each heading routine takes from `td.shp` and the rotation sets is the renderer's business (M7-01). Flights 7, 12, 15, 20 and 21 overwrite parts of the sprite data with `mine.td`, `td.hel`/`td.h2` and `lander.td` ([globals §6](../../re/globals.md)), so the same class codes show other images there; which slots, headings and ships: [flight-screens §4.9](../flow/flight-screens.md). Then, if marker = 32h or 39h, 20 single pixels form four corner brackets around the 32 × 32 cell (page y includes 232 × drawPage):

| Pixel (dx, dy) from (x, y) | Colour (32h) | Colour (39h) |
|---|---|---|
| (1, 1), (1, 30), (30, 1), (30, 30) | 5Ch | 66h |
| (2, 1), (1, 2), (1, 29), (2, 30), (29, 1), (30, 2), (30, 29), (29, 30) | 7Ah | 82h |
| (3, 1), (1, 3), (1, 28), (3, 30), (28, 1), (30, 3), (30, 28), (28, 30) | F9h | 98h |

Ships are drawn after the player ship (unless `g_PlayerDrawLate`) and before the overlay tiles and explosions ([tick-order](tick-order.md) steps 7–16).

## 5. Edge cases and original bugs

| Quirk | Original | Port policy |
|---|---|---|
| OB-013: tile adjustment through a nil pointer | Step (a) reads "tile attributes" from the far pointer DS:3A32, which is never assigned: it reads the real-mode interrupt vector table (byte at 0000:tile code). If the ship's cell reads ≠ 2 and a neighbour reads 2, the ship tile moves by one; if the cell reads 2 the waypoint is set to the ship tile. On the reference machine (C-0002, C-0009) no IVT byte 0..70 is 2, so nothing ever happens | **preserve as no-op** (proposed): treat every lookup as ≠ 2 |
| OB-014: up-thrust brakes the wrong component | Thrusting up while the down speed is above 16 adds and removes the thrust on the up component; the down speed only decays by drag | preserve |
| OB-015: right-hand follower facing south | Tests the cell ahead where it should test the right-hand cell: with the right side open and the way ahead blocked it turns left | preserve |
| OB-016: line-of-fire scan direction | Scans only toward increasing x / y; a wall between the ship and a target above or to the left is not seen, and a wall behind the ship can block | preserve |
| OB-017: two-digit headings read past the muzzle table | Muzzle offsets for 0Ch, 0Eh, 20h, 24h, 4Ah, 4Eh, 60h, 62h come from other globals (DS:04F6.. System constants, `g_SbIrq`, `g_MemAvailText`; 053A.. and 079E.. always 0; 0826.. menu palette; 0D32.., 0DBA.., 101E.. scene palette, title-font pointer, dialogue reader; 1062.. dialogue text buffer) | **preserve** (proposed, [weapons §3.3](weapons.md)): U-160 resolved; the 272 bytes are built at flight start from front-end state and three reference-machine constants |
| Path step 501 | A 500-step path with no 0 terminator reads path[501] = (moveDir, aggression) and flies toward it (m3 slot 2; m9 slot 18 ends at the map edge instead) | preserve (OB-144) |
| Slot 20, path steps 482..500 | Heap contents (OB-008): the path buffers' bytes at M + 19,329.. and M + 29,353.. (§4.1.1), which only a front-end portrait or song can have written; 0 in a direct start under DOSBox | OB-008 |
| Flight 21's paths come from the front end's leftovers | §4.1.1 (U-500): songs, fonts and portraits shown between flights 19 and 21, over `m19.td`'s arrays | Preserve (T1, OB-140): the port builds them by the rule of §4.1.1 |
| Turn directions carried across flights | Setup does not reset them (U-501) | Preserve (session state) |
| Class FFh at setup | Armour and shields read table bytes past `k_ClassArmour`/`k_ClassShields` (DS:03C7, DS:03D1) | preserve (only visible if mission logic revives the slot; OB-144) |
| Unit-init `Random(2)` draws | 20 draws at program start and 20 at every flight start | preserve (RNG stream) |
| Record fields not cleared between flights | +411h, +41Ah, +41Bh, +418h keep the previous flight's values | preserve (OB-144) |
| Ships removed at the map edge are drawn once more | Removal happens inside the movement; drawing follows | preserve (OB-144) |
| Speed components below 16 after a brake | 13..15 behave as 16 (no drift, no drag) and resume from there | preserve |

## 6. Test vectors (capture C-0009, E1 flight 1, no input)

Sampling: ship state "after tick t" is the **last** frame whose tick counter DS:1718 = t. (The first frame with counter t usually precedes `AI_UpdateShips` of tick t: in C-0009 the ship arrays change in the frame after the counter increments in 293 of 311 ticks.)

| # | Check | Result |
|---|---|---|
| 1 | One-tick prediction of all live slots (slot arrays + full records) for ticks 2–310, with an `AI_SetTarget(s, 0)` hypothesis per slot for ticks where the script ordered an attack | 3,342 / 3,343 slot-ticks identical; the exception is slot 1 destroyed by a shot at tick 254 |
| 2 | Slot 1 (class 0) from setup: path[1] = (124, 73), cursor 0 → tick 1: waypoint (124, 74), prev (124, 73), cursor 2, moveDir 2, target heading 2, turn 2 | as §4.2.3 vector: tile Y 73 → 74 at tick 15, waypoint (124, 75) and cursor 3 at tick 16, (124, 76) / 4 at tick 20, (124, 77) / 5 at tick 24 |
| 3 | Slot 19 (class 7, aggression 50, handBit 1): attack ordered at tick 34 at waypoint (93, 111), player tile (107, 107) | tick 34: waypoint (94, 110), stepX 1, stepY −1, attackCounter 1; tick 41: blockedY := 1 → right-hand detour, waypoint (93, 110), moveDir 4, detourSteps 1 |
| 4 | Slot 19 in line: tick 110 waypoint (105, 107), target (107, 107) (D = 4, same row) | waypoint unchanged, fire request 6, attackCounter 118 → 169; tick 111: holdCount 1, returnMode 1, waypoint → (104, 108) toward return point (92, 111), second request 6 (counter 220) |
| 5 | Enemy shot spawns (51 volleys by slots 17–20) | life 1, owner, tile = ship tile, heading = ship heading, kind 1 (class 7), offset = muzzle + ship offset − 16: 49 / 51 settled exactly; the other 2 are two-digit-heading shots deleted before their offsets were dumped |
| 6 | Setup, flight 1 | classes 0, 0, FFh × 6, 0, 0, 7, 1, 1, 7 × 7 (slots 9–11 removed by mission logic at tick 1); armour 3 (slots 1–17), 1000 (12, 13), 50 (18–20); aggression 100 (8), 50 (11, 14–20); handBits 1 1 1 0 0 0 0 0 1 0 0 0 1 0 1 1 0 1 1 0 |

Scratch models used for these checks are outside the repo (RE work area); the numbers above are the facts an implementation must reproduce.

## 7. Open questions

- [x] U-160: what the port does for two-digit-heading muzzle offsets (OB-017): [weapons §3.3](weapons.md) (RE-15; policy *preserve* proposed).
- [ ] U-161: confirm Real48 `Sqrt` is exact for 121 (out-of-range boundary); D = 9 is confirmed.
- [ ] U-162: policy for a detour with moveDir ∉ {2, 4, 6, 8} (endless loop in the original; not reachable in E1 as far as known).
- [ ] When and why the mission logic calls `AI_SetTarget` and writes slot classes: RE-6's mission-logic spec (`2404`).
- [ ] VE: a capture of flights 7 and 20 (pins, sinking landers) and of a ship leaving at the map edge.

## 8. Provenance

| Claim | Address / capture | Method | Confidence |
|---|---|---|---|
| Setup order, record and slot initialisation | `0E8B:B8BA`, `0E8B:C25F..C464`, `0E8B:CC63..CC79`, `34E6:0000`, `34E6:3836` | static + C-0009 tick 0/1 | CONFIRMED |
| Pass 1, pass 2, thrust, heading target, drift, carry, edge removal | `3292:0011` | static + C-0009 (3,342 slot-ticks) | CONFIRMED |
| DS:3A32 never assigned; IVT bytes 0..70 ≠ 2 under DOSBox | all segment listings; C-0002 0000:0000 dump; C-0009 | static + capture | CONFIRMED |
| AI state machine and helpers | `34E6:0396..3449` | static + C-0009 | CONFIRMED (paths exercised: path, attack, in-line hold, schedule, both followers, return, budget disengage) |
| Mode +422h dead | writers of record +422h/+424h/+425h: only `34E6:0000` | static | CONFIRMED |
| Fire decision and requests | `34E6:0A5D`, `34E6:09B6` | static + C-0009 (fire requests 2, 4, 6, 8) | CONFIRMED |
| Shot spawn | `0E8B:F2D6..F4A6` | static + C-0009 projectile dumps | CONFIRMED |
| No body collision | writers of DS:398E/398F and record +3F8h/+3FAh in all segments | static | CONFIRMED |
| Sprite = class code, per-heading blits, markers | `3292:0011` draw section | static | LIKELY (not pixel-checked) |
| Flight 7 pins, flight 20 sinking, flight 19 copy | `3292:0011`, `0E8B:D646..D675` | static | LIKELY |
| Turn directions: only writer `AI_UpdateShips`'s steering; none at setup | `3292:0A27..0D68` (16 writes), reads `3292:039A`, `03B5` | static (operand scan of every listing); C-0200, C-0202 | CONFIRMED |
| Flight heap layout and the flight-21 path rule (§4.1.1) | `0E8B:B8A4..BA68` (free music, `BP_Mark`, 17 `GetMem`s), `0E8B:F9C1..F9DE` (release); `051A:701A`, `051A:92CC`, `051A:59CA`, `051A:1EB5` (975-byte font blocks), `051A:163E`, `051A:17E0..182C` (portrait block and pixel rule), `051A:70E6..70F1` (debrief face 1, right), `0E8B:07ED`, `2C37:0273` (music), `3A9A:041E..04DB` (free writes the size field) | static; C-0200 and C-0300 path buffers (5800:0000 window), model = capture in all 20,001 bytes it defines | CONFIRMED |

## 9. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-02 | RE-2 (Archaeologist + Spec author) | Partial draft |
| 2 | 2026-11-02 | RE-5 (Archaeologist + Spec author + Verifier) | Complete: setup, AI state machine (U-045), detour followers, firing, removal, drawing (U-042), DS:3A32 nil pointer, test vectors from C-0009 |
| 3 | 2026-11-05 | RE-10 (Spec author, U-252) | U-252 corrections: `34E6:3232` is named `AI_SetTarget` (as in mission-logic; was `AI_OrderAttack`); volley spawning is tick-order step 30 and clearing step 31 (were 31/32) |
| 4 | 2026-11-06 | RE-14 (Archaeologist + Spec author) | §4.3.10: what an attacker does when its target dies, hides or leaves the range (U-181): it keeps attacking the last waypoint until its budget runs out; re-applied orders do not reset the budget |
| 5 | 2026-11-06 | RE-15 (Archaeologist + Spec author) | §4.4.2 and OB-017: the two-digit muzzle bytes are specified in weapons rev 4 §3.3 (U-160 resolved, policy *preserve* proposed) |
| 6 | 2026-11-06 | RE-18 (Archaeologist + Spec author) | §2 and §4.1: slot turn directions are not reset at setup, 0 only at program start (U-501, slots' part). §4.1.1 new: flight 21's path arrays, the flight heap layout and the front-end writers that fill them, port rule and two vectors (U-500; C-0200, new capture C-0300). §5 three rows (OB-140), §8 two rows |
| 7 | 2026-11-06 | RE-19 (Archaeologist) | §4.1.1 vector 3: music off confirmed (U-540): no song on the heap, so the fonts and portraits start at the heap mark; flight 21 of C-0323 (C-0300 with `td.cfg` music off) matches the rule in all 58 ticks (scratch run of `oracle_e1_full`'s block-19 check with the music-off heap uses) |
