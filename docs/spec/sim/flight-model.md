# Player flight model

| | |
|---|---|
| **Spec ID** | `sim/flight-model` |
| **Revision** | 2 |
| **Status** | Draft |
| **Fidelity tier** | T1 |
| **Confidence** | **CONFIRMED** for §4.2 drift and drag, §4.3 horizontal collision and bounce, §4.4 tile carry inside the map, §4.5 thrust and braking (classes 4 and 0, keyboard), §4.6 turn step and the side-block steering cases: the rules reproduce every tick of captures C-0007 and C-0008 (215 ticks, 0 mismatches, §6). LIKELY (static only) for vertical collision, map limits, the up/down and diagonal steering cases, the `full` predicates (joystick) and §4.7 |
| **Tasks** | M6-01 (draft from M1-08 findings) |
| **Unknowns** | U-002 and U-003 (resolved), U-009 (partial), U-024 (partial), U-043 |

## 1. Summary

The player's ship does not have a position of its own: it sits at the centre of the playfield and **the view moves**. The view position is a map tile plus a pixel offset per axis. Movement is driven by **four independent speed bytes** (up, down, left, right), each biased by 16: a component moves the view by `speed − 16` pixels per tick while it is above 16, and loses 1 per tick (drag). The arrow keys (keypad digits, grey arrows or joystick) add the class's thrust step to the matching component, up to the class maximum, and brake the other three. The ship's **heading** is one of 16 discrete directions and turns one step per tick toward the direction of the pressed keys; the heading only selects the sprite and the afterburner direction, it does not steer the movement. Hitting a solid map tile swaps the two opposite components (a bounce). There is no floating point in this model.

## 2. Definitions and units

| Quantity | Original | Type | Unit / range | Notes |
|---|---|---|---|---|
| View tile X, Y | DS:3A36, DS:3A38 | i16 | tiles; X 6..183, Y 7..190 | Column/row of the map (column-major map, `structures.md` §1) |
| View offset X, Y | DS:3A3A, DS:3A3C | i16 | pixels; kept in 16..48 between ticks | Playfield top-left world pixel = tile × 32 + offset − 16 |
| Speed components | DS:3995 up, 3996 down, 3997 left, 3998 right | u8 | biased: 0..16 = still, s > 16 moves s − 16 px/tick | Reset to 0 at flight start |
| Heading | DS:1A56 | u8 | keypad code (`structures.md` §7) | Starts at 8 (up) |
| Target heading | DS:1CBB | u8 | keypad code (principal directions only) | Starts at 8 |
| Turn direction | DS:1A5A | u8 | 0 none, 1 counter-clockwise, 2 clockwise | Starts at 0 |
| Class | DS:1A57 | u8 | 0..8 | Per flight (`structures.md` §6) |
| `max[c]`, `accel[c]` | DS:02BE + c, DS:02DC + c | u8 | raw (biased) speed, speed units per tick | §3 |
| Armour, shields | DS:398E, DS:398F | i8 | points | From DS:02C8 + c and DS:02D2 + c |

Directions: up = decreasing Y, left = decreasing X. One tick ≈ 1/19 s ([timing](../core/timing.md)).

## 3. Data

| Class c | `max[c]` | top speed (px/tick) | `accel[c]` | armour | shields |
|---:|---:|---:|---:|---:|---:|
| 0 | 24 | 8 | 2 | 31 | 0 |
| 1 | 28 | 12 | 4 | 16 | 0 |
| 2 | 32 | 16 | 4 | 15 | 18 |
| 3 | 28 | 12 | 4 | 20 | 20 |
| 4 | 28 | 12 | 4 | 25 | 25 |
| 5 | 28 | 12 | 4 | 25 | 20 |
| 6 | 24 | 8 | 2 | 31 | 31 |
| 7 | 28 | 12 | 4 | 25 | 25 |
| 8 | 32 | 16 | 4 | 20 | 25 |

"Top speed" is the nominal `max − 16`; the real steady state is higher (§4.2, test vector 1). Start values: view (102, 104) with offsets (32, 16) by default (`0E8B:C62B..C647`); per-flight overrides in `0E8B:60D8` and for flights 13–14 (177, 153).

Input predicates used below (all read from the key table DS:1A5C + scan code, `globals.md` §3; joystick X = DS:1F67 with 1 = left and −1 = right, Y = DS:1F68 with 1 = up and −1 = down):

| Name | True when |
|---|---|
| U | keypad 8 down, or joystick Y = 1, or grey Up down |
| D | keypad 2 down, or joystick Y = −1, or grey Down down |
| L | keypad 4 down, or joystick X = 1, or grey Left down |
| R | keypad 6 down, or joystick X = −1, or grey Right down |
| Ufull | keypad 8 and grey Up down **and** joystick Y < 0 |
| Dfull | keypad 2 and grey Down down **and** joystick Y ≥ 1 |
| Lfull | keypad 4 and grey Left down **and** joystick X < 0 |
| Rfull | keypad 6 and grey Right down **and** joystick X ≥ 1 |

The `full` predicates pair each key with the **opposite** joystick direction (an original bug). With the joystick centred they are always false.

## 4. Behaviour

All steps run in the tick tail after the page flip, in this order ([tick-order](tick-order.md) steps 27–30), except the turn step (§4.3), which runs at the top of the next tick.

### 4.1 Shields and destruction (step 27)

```text
if shields < 1: shields := 0
if armour < 1 and not mission over: mission over := 1, end ticks := 1   (player destroyed)
if shields < shields_table[class]: shields := shields + 1              (signed byte compare)
```

### 4.2 Drift and drag (step 28, first part)

In this order, using wrapping 16-bit arithmetic on the offsets and u8 on the speeds:

```text
if up    > 16: offsetY := offsetY − (up − 16);    up    := up − 1
if down  > 16: offsetY := offsetY + (down − 16);  down  := down − 1
if left  > 16: offsetX := offsetX − (left − 16);  left  := left − 1
if right > 16: offsetX := offsetX + (right − 16); right := right − 1
```

A component at 16 or below never changes by itself, so a ship that was only tapped (speed 4, 8, 12, 16) stays still with that value stored.

### 4.3 Map collision (step 28, second part; skipped while DS:1703 ≠ 0)

`solid(dx, dy)` means `tile_attr[map[viewX + dx][viewY + dy]] ≠ 2` (`structures.md` §8). Tests in this order; each uses the offsets as updated so far:

```text
if offsetX < 32 and (solid(4,3) or (solid(4,2) and offsetY < 40)):  offsetX := 32; swap(left, right)
if offsetX > 32 and (solid(6,3) or (solid(6,2) and offsetY < 40)):  offsetX := 32; swap(left, right)
if offsetY < 48 and (solid(5,2) or (solid(6,2) and offsetX > 32) or (solid(4,2) and offsetX < 32)):
                                                                     offsetY := 48; swap(up, down)
if offsetY > 48 and (solid(5,4) or (solid(4,4) and offsetX < 32) or (solid(6,4) and offsetX > 32)):
                                                                     offsetY := 48; swap(up, down)
```

The swap turns the speed that carried the ship into the wall into speed away from it: a bounce with no loss.

### 4.4 Tile carry and map limits (step 28, third part)

```text
if offsetY < 16 and viewY > 7:    viewY -= 1; offsetY += 32
if offsetY < 16 and viewY = 7:    offsetY += 32; edge alert            (pushed back one tile)
if offsetY > 48 and viewY < 190:  viewY += 1; offsetY -= 32
if offsetY > 48 and viewY = 190:  offsetY -= 32; edge alert
same for X with limits 6 and 183 (offsetX, viewX)
edge alert: if DS:32BC = 0 then DS:1E2E := 1   (DS:32BC = HUD ticker position; 0E8B:7FDF then posts the
            leave-jurisdiction message to the ticker)
```

The direction of each carry is recorded in DS:1CC8..1CCB (`globals.md` §4).

### 4.5 Thrust (step 30)

Only while not mission over. Blocks in this order; each block reads the speeds as left by the previous one:

```text
half := round_half_up(accel[class] / 2)            # 2 -> 1, 4 -> 2 (Real48 Round of accel / 2.0)
if U and up < max[class]:
    up += accel[class]
    if down  > 16 and not Dfull: down  -= half
    if left  > 16 and not Lfull: left  -= half
    if right > 16 and not Rfull: right -= half
if D and down < max[class]:
    down += accel[class]
    if up    > 16 and not Ufull: up    -= half
    if left  > 16 and not Lfull: left  -= half
    if right > 16 and not Rfull: right -= half
if L and left < max[class]:
    (brake up, down, right as above, each unless its full predicate)
    left += accel[class]
if R and right < max[class]:
    (brake up, down, left as above)
    right += accel[class]
```

The limit test is `speed < max` before adding, so a component can end up to `accel − 1` above `max` (for example 26 + 4 = 30 > 28).

### 4.6 Steering: target heading and turn direction (step 30, after thrust)

Principal headings: 8 up, 9 up-right, 6 right, 3 down-right, 2 down, 1 down-left, 4 left, 7 up-left. "cw/ccw if h ∈ S" sets the turn direction only when the current heading h is in S (otherwise the previous turn direction is kept).

```text
if U and not over:
    if R and not Lfull:                  cw if h in {7,8,4};  ccw if h in {6,2,3};  if h != 9: target := 9
    elif (not L) or Rfull:
        if h != 8:                       ccw if h in {9,6,3}; cw if h in {7,4,1};   if h != 2: target := 8
        go on to the D block below
    else:                                cw if h in {2,4,1};  ccw if h in {9,6,8};  if h != 7: target := 7
D block (reached when the U test failed, or from the up-only case):
if D and not over:
    if R and not Lfull:                  cw if h in {9,8,6};  ccw if h in {1,2,4};  if h != 3: target := 3
    elif L and not Rfull:                cw if h in {2,3,6};  ccw if h in {8,7,4};  if h != 1: target := 1
    elif Lfull or Rfull or h = 2:        go to the side block
    else:                                ccw if h in {7,4,1}; cw if h in {9,6,3};   if h != 8: target := 2
side block (reached when the D test failed, or from above):
if L and not over and h != 4:            ccw if h in {9,8,7}; cw if h in {3,2,1};   if h != 6: target := 4
elif R and not over and h != 6:          cw if h in {9,8,7};  ccw if h in {3,2,1};  if h != 4: target := 6
```

Notes: pressing the direction exactly opposite the current heading does not change the target (the ship keeps its heading); the target persists after the keys are released, so the turn completes; for an intermediate heading the turn direction is left as it was.

**Turn step** (top of the next tick, [tick-order](tick-order.md) step 3): if target ≠ heading, find the heading in the 16-entry ring (DS:02AD..02BC); if the turn direction is 1 take the previous ring entry, if 2 the next one (the ring wraps through DS:02AC and DS:02BD). One step (22.5°) per tick.

### 4.7 Afterburner burst (classes 2 and 5; tick-order step 5, top of the tick)

```text
if (keypad + or Ins or joystick button 1 is down) and class in {2, 5} and not mission over:
    if burst_frame = 0 and heading < 10:              # principal headings only
        burst_frame := 1; burst_heading := heading
        start a sound (kind 0, source FFFFh)
    clear the key bytes of keypad + and Ins            # one burst per key press
if burst_frame != 0:
    heading := burst_heading                           # the burst locks the heading
    if burst_frame = 2: every component along burst_heading := 34   (18 px/tick)
    if burst_frame = 3: every component along burst_heading := 44   (28 px/tick)
    draw flame sprite 9 + burst_frame at a heading-dependent offset (principal headings)
    burst_frame += 1; if burst_frame > 5: burst_frame := 0
```

Components along a heading: left for 4, 1, 7; right for 6, 9, 3; up for 8, 7, 9; down for 2, 1, 3 (diagonals set two components). Drag (§4.2) then slows the ship by 1 per tick. The hint shown before flight 6 describes the afterburner as short bursts on the numeric keypad.

## 5. Edge cases and original bugs

| Quirk | Original | Port policy (proposal) |
|---|---|---|
| Dead zone: speeds 1..16 do nothing | Four thrust ticks before the first pixel of movement (class 4) | Preserve (T1) |
| Overshoot of `max` by up to `accel − 1` | Steady state oscillates above the nominal top speed | Preserve |
| `full` predicates pair keys with the opposite joystick direction | Braking can be suppressed only by an odd key + joystick combination | Preserve (harmless with a centred joystick) |
| Map edge pushes the view back a whole tile | 32-pixel jump at the jurisdiction limit | Preserve |
| Opposite key does not reverse the heading | The ship keeps facing the old way while thrusting backwards | Preserve |
| Heading does not steer movement | Movement is 8-way by component | Preserve (it is the game's feel) |

## 6. Test vectors

Vectors 1 and 2 were first derived from this model and then **observed exactly** in C-0008 (rightward and leftward instead of upward, which is the same rule on another component; the start cell is boxed in vertically). Method: DGROUP dumped every video frame; the state of tick t is read from the first frame whose tick counter (DS:1718) is t, which lies after the physics of tick t − 1 and before that of tick t. A rule-by-rule model of §4.2–§4.6 driven by the recorded key bytes reproduces every observed tick of C-0007 (99 ticks, including repeated wall bounces and a class change) and C-0008 (116 ticks).

| # | Setup | Expected per tick (moved px / total px / up speed after the tick) | Evidence |
|---|---|---|---|
| 1 | Class 4, open street, hold one thrust direction from rest for 14 ticks, then release | t1..5: 0/0/4, 0/0/8, 0/0/12, 0/0/16, 0/0/20; t6: 4/4/23; t7: 7/11/26; t8: 10/21/29; t9: 13/34/28; t10: 12/46/31; t11: 15/61/30; t12: 14/75/29; t13: 13/88/28; t14: 12/100/31; released: t15: 15/115/30, t16: 14, t17: 13, … down to 1 px at t29, then 0 (speed stays 16) | C-0008 keypad 6, physics ticks 40..53 (world X 3296 → 3516) |
| 2 | Class 0 (accel 2, max 24), hold from rest for 22 ticks, then release | speeds 2, 4, …, 18 (t1..9, no movement); moved px from t10: 2, 3, 4, 5, 6, 7, 8, 9, 8, 9, 8, 9, 8 (speed alternating 25/24); released at speed 24: 9, 8, 7, …, 1, then 0 | C-0008 keypad 4 after the M+K+A+R chord, physics ticks 80..101 |
| 3 | C-0002 at flight +1 s and +2 s, no input | speeds 0, heading 8, target 8, turn direction 0, view (102, 104, 32, 16), armour 25, shields 25 (class 4) | C-0002 RAM |
| 4 | Class 4 at the start position, hold right 14 ticks | the first move (+4 px at the 6th tick) takes offset X to 36 > 32 with cell (108, 106) solid and offset Y 16 < 40: offset X := 32 and left/right swap, then the thrust brakes the new left component by 2 and adds 4 to right; per tick (left, right) = (17, 4), (4, 20), (17, 8), (8, 20), (17, 12), (12, 20), (17, 16), (16, 20), (17, 20); after release (19, 16), (16, 18), (17, 16), (16, 16); the view never moves | C-0007, physics ticks 22..39 |
| 5 | Class 0 at the start position, hold left (heading 6) | the target heading stays 6 (opposite key ignored); left = 18 is bounced by cell (106, 106) into right = 17, then braked by 1 and left gets +2: (18, 16) every tick | C-0007, physics ticks 74..101 |
| 6 | Heading 8, hold right | heading 8 → 98 → 9 → 96 → 6, one ring step per tick, turn direction 2 (clockwise), target 6 | C-0007 ticks 22..26 |
| 7 | Hold down 6 ticks from (102, 104, 32, 16) | down speed 4, 8, 12, 16, 20, 23; offset Y then 20, 27, 33, 38, 42, 45, 47, 48 (no carry) | C-0008 ticks 22..34 |

## 7. Presentation and audio hooks

- The player sprite for heading h is drawn with the h-specific blit (`structures.md` §7) at page position (128 + 4·round(offsetX / 4), offsetY + 84) of the back page (step 7 of the tick). The 4-pixel rounding comes from the planar blit.
- The afterburner burst plays a sound through `1E2A:006A` with kind 0.
- The edge alert drives the jurisdiction warning (`0E8B:7FDF`).

## 8. Open questions

- [ ] Q1 (U-043): Space (fire) and Alt (infra-red) in `0E8B:B1EC`.
- [ ] Q2 (U-009): collision for mission ships and projectiles; whether DS:1703 (late draw) also means "no collision" in every flight that sets it.
- [x] Q3 (VE): test vectors 1 and 2 captured (C-0008) and match.
- [ ] Q4 (VE): capture vertical collision, a map limit, an up/down + side diagonal and the afterburner burst (class 2 flight) to raise the rest of the spec to CONFIRMED.

## 9. Provenance

| Claim | Address / capture | Method | Confidence |
|---|---|---|---|
| Shields/armour step | `0E8B:E540..E5A0` | static | LIKELY |
| Drift and drag | `0E8B:E5A0..E640` | static | LIKELY |
| Collision tests and bounce | `0E8B:E640..E8xx` | static | LIKELY |
| Carry and limits | `0E8B:E8xx..EA0x` | static | LIKELY |
| Thrust, half-accel braking | `0E8B:EA2B..EDxx` (`Round(accel / 2.0)` at `0E8B:EA9D..EACA`) | static | LIKELY |
| Steering | `0E8B:EDxx..F2xx` | static | LIKELY |
| Turn step and ring | `0E8B:CCE3..CD22`, DS:02AC..02BD | static | CONFIRMED |
| Afterburner burst | `0E8B:CD78..D160` | static + hint text before flight 6 | LIKELY |
| Class tables | DS:02BE..02E4 | static (file bytes) + C-0002 (class 4 armour/shields) | CONFIRMED |
| Start values | `0E8B:C4E6..C647`; C-0002 | static + capture | CONFIRMED |
| Drift, drag, horizontal collision, carry, thrust, braking, turn | C-0007 (99 ticks), C-0008 (116 ticks), every tick reproduced | dynamic | CONFIRMED |

## 10. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-02 | RE-2 (Archaeologist + Spec author) | Initial draft from M1-08 findings |
| 2 | 2026-11-02 | RE-2 (Verifier) | No rule changes. Test vectors 1 and 2 observed in C-0008; collision, braking, class change and steering cases observed in C-0007 (vectors 4–7); confidence raised per section; edge-alert note names DS:32BC as the HUD ticker position |
