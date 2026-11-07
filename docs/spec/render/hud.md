# In-flight display and HUD

| | |
|---|---|
| **Spec ID** | `render/hud` |
| **Revision** | 4 |
| **Status** | Draft |
| **Fidelity tier** | T0 for geometry, element positions and colours; T1 for update order and for the two game rules found here (§4.3, flight 7) |
| **Confidence** | CONFIRMED for the display geometry, both bars, the missile column, the radar (map, ship markers, player), the compass lamps and the guidance dots: a model written from this spec reproduces C-0002 frame 3543 with 0 differing pixels (§6). LIKELY for the ticker window, the F6 indicator and the weapon icon (static) |
| **Tasks** | M2-09, M6-07 (RE part) |
| **Unknowns** | U-025 (resolved), U-033 (resolved), U-185, U-186 |

## 1. Summary

In flight the original shows a **320×192** picture: a scrolling **320×160 playfield** on top and the **320×32 HUD bar** (`td.win`) below it. It does this with VGA hardware alone. The CRTC's vertical display end is cut to 192 lines, and the line-compare (split-screen) register makes the bottom 32 lines always show video-memory rows 0..31, where `td.win` was plotted once at mission start. The playfield is drawn into one of two 352×232 off-screen pages, and the visible window is chosen with the display start address. HUD elements are drawn straight into VGA rows 0..31 and therefore appear in the bar at once, independent of the page flip. Once per tick `UI_DrawInstruments` (`0E8B:8E87`, tick-order step 19) draws, in this order: the **guidance line** (six dots around the player ship, in the playfield page) and the eight **compass lamps** (palette entries), the low-armour warning, the **armour** and **shield bars**, the **missile lamp** and **missile column**, the **radar** (map, ships, shots, player), and then the **message ticker** and the **F6 transmission indicator**. Several elements are animated only through palette entries; a port that composes RGB frames must apply those palette changes.

## 2. Definitions and units

| Term / quantity | Type in original | Unit | Range | Notes |
|---|---|---|---|---|
| VGA row | — | 88 bytes = 352 px | 0..743 | Unchained mode 13h, CRTC offset 44 ([asset-load-sites.md §1.1](../../re/asset-load-sites.md#11-vga-memory-layout-unchained-mode-13h-352-pixel-rows)) |
| HUD coordinates (hx, hy) | — | px | 0..319, 0..31 | VGA rows 0..31; on screen at (hx, 160 + hy) |
| Playfield coordinates (px, py) | — | px | 0..319, 0..159 | Screen pixel in the playfield |
| Page | Byte DS:3A30 | — | 0, 1 | Page 0 at VGA 0B00h (row 32), page 1 at 5AC0h (row 264); each 352 × 232 |
| Scroll x, y | Words DS:3A3A, DS:3A3C (`g_ViewSubX/Y`) | px | 16..48 | Fine scroll inside the page; start 32, 16 |
| View tile | Words DS:3A36, DS:3A38 (`g_ViewTileX/Y`) | tile | | Map tile of the view's top-left corner; start (102, 104) |
| Player tile (HUD) | — | tile | | (viewX + 5, viewY + 3): the tile the radar and the guidance treat as the player's |
| Palette mode | Byte DS:1704 (`g_PaletteMode`) | — | 0, 1, 2 | 0 normal, 1 infra-red, 2 the second IR mode; changes the bar colours (§3.4) |
| Flight palette | DS:13E6 (`g_FlightPalette`), 768 bytes | 6-bit | | Working copy of `td.pal`; fades read it. `Pal_SetEntry` (`0E8B:8242`) writes an entry there **and** to the DAC; `Pal_StoreEntry` (`0E8B:829A`) only there |
| DAC value | — | 6-bit | 0..63 | The VGA DAC keeps the low 6 bits: a stored 64 shows as 0 (§5) |

## 3. Data

### 3.1 Display set-up (`Sim_FlyMission`, `file0001:0E8B:B880`)

| Register / value | Effect | Provenance |
|---|---|---|
| CRTC 12h := 7Fh (vertical display end 17Fh = 383) | 384 scan lines = **192 rows** shown | `0E8B:CC40` |
| Line compare := 13Fh (319) | Scan lines 0..319 (rows 0..159) show the page; from row 160 the display restarts at VGA address 0 | `Gfx_SetSplitLine` calls in `0E8B:B880` |
| CRTC 13h = 2Ch | 88-byte rows | `Gfx_InitUnchained` |
| Start address | page base + (scroll y − 16) × 88 + Round(scroll x / 4) − 4 | `0E8B:7E91`, `0E8B:7ED1` |
| No attribute-controller writes | horizontal scroll moves in 4-pixel steps (no pixel panning) | port I/O scan (`FindPortIO`) |

Menus and scenes set the split line to 400 (off) and the display end back to normal. The playfield shows page pixels from (4 × Round(scroll x / 4) − 16, scroll y − 16) ([camera-viewport](../sim/camera-viewport.md)).

### 3.2 `td.win` (HUD bar)

320×32 bytes, read one byte at a time and plotted to VGA (x, y) = (column, row), rows 0..31, before the first frame. The palette is `td.pal`. The bar is never redrawn as a whole during the mission; elements overwrite parts of it. Some of its pixels use palette entries that the game animates (§3.4); their places in the artwork (from the file, CONFIRMED):

| Palette entry | Pixels | Bounding box (hx, hy) | Role |
|---:|---:|---|---|
| 85h | 36 | 74..79, 8..13 | compass lamp NW |
| 86h | 25 | 80..84, 8..12 | compass lamp N |
| C1h | 36 | 85..90, 8..13 | compass lamp NE |
| 2Fh | 20 | 74..78, 14..17 | compass lamp W |
| 31h | 20 | 86..90, 14..17 | compass lamp E |
| 2Eh | 36 | 74..79, 18..23 | compass lamp SW |
| 30h | 25 | 80..84, 19..23 | compass lamp S |
| 24h | 36 | 85..90, 18..23 | compass lamp SE |
| 26h | 59 | 115..131, 8..14 | missile lamp (also every second row of the missile column) |
| 90h | 1,229 | 5..63, 4..27 | background of both bars (also their empty segments) |

### 3.3 HUD elements (HUD coordinates unless stated)

| Element | Area | Source | Update | Conf. |
|---|---|---|---|---|
| Shield bar | x 6..63, y 11..14 | `g_PlayerShields` DS:398F (i8) | every tick (§4.4) | CONFIRMED |
| Armour bar | x 6..63, y 23..26 | `g_PlayerArmour` DS:398E (i8) | every tick (§4.4) | CONFIRMED |
| Compass lamps | palette entries of §3.2 | live marked mission ships and the home point | every tick (§4.3) | CONFIRMED |
| Missile lamp | palette entry 26h | `g_Missiles` DS:1CB9, `g_MissileCooldown` DS:1CBA | every tick (§4.5) | CONFIRMED |
| Missile column | x 100..103, y 2..29 | `g_Missiles` | every tick (§4.5) | CONFIRMED |
| Radar | x 148..167, y 6..25 (20 × 20) | `td.map` via `k_TileRadarColour`, ship and shot tiles | every tick (§4.6) | CONFIRMED |
| Weapon icon | x 179..214, y 7..24 (36 × 18) | `td.wea` icon number `g_Weapon` DS:1CB8 (1..6) | at flight start and when the weapon changes ([weapons](../sim/weapons.md) §4.1) | LIKELY |
| F6 transmission indicator | x 220..247, y 2..29 (28 × 28) | `td.fac` frame 0..4 | every tick while a transmission is pending (§4.8) | LIKELY |
| Message ticker | x 256..311, y 13..17 (56 × 5) | text strip in off-screen VGA rows 735..740 | every page-0 tick while running (§4.7) | LIKELY |
| Guidance line | playfield, 6 dots at radius 20..25 px around (160, 85) | first live marked mission ship, else the home point | every tick, drawn into the back page (§4.3) | CONFIRMED |

### 3.4 Colours

| Use | Palette entry | Notes |
|---|---|---|
| Bar segment, columns J = 0..6 | B0h (left pixel), 7Ah (right pixel) | all palette modes |
| Bar segment, J = 7..12 | 48h / 96h; palette mode 2: B0h / A5h | |
| Bar segment, J = 13..28 | B2h / A9h; palette mode 2: B0h / A5h | |
| Bar segment, empty | 90h | |
| Missile column rows | 26h and 27h alternating; 0 when empty | |
| Radar: map tile *t* | `k_TileRadarColour[t]` | table at DS:0264, resource `hud.radar_tile_colours` (71 bytes, tiles 0..70) |
| Radar: live mission ship | 55h | |
| Radar: shot (projectile slots 1..50) | 22h | slot 0 is never shown |
| Radar: player | 46h | |
| Guidance, marker 32h (target) | dots 0..5: B0h, A5h, F9h, 7Ah, 5Ch, 5Ch | |
| Guidance, marker 39h (friend) | B2h, A9h, 98h, 82h, 66h, 66h | |
| Guidance, marker 3Dh (home) | B3h, ABh, 9Ch, D1h, 50h, 50h; palette mode 2: all six in the home lamp's entry, whose DAC is set to (0, 0, 63) | |
| Guidance shadow | 0 | |

`k_TileRadarColour` replaces the two short tables that [globals](../../re/globals.md) §1 listed at DS:0295 and DS:02A2: they are its entries for tiles 49..59 and 62..70.

### 3.5 Guidance constants (Real48, bytes in memory order)

| Name | Bytes | Value ≈ | Provenance |
|---|---|---|---|
| `SLOPE_LO` | `7F 17 D9 CE F7 53` | 0.414 (the decimal literal; tan 22.5° is 0.41421) | `0E8B:8422` |
| `SLOPE_HI` | `82 D7 A3 70 3D 1A` | 2.41 (literal; tan 67.5° is 2.41421) | `0E8B:848D` |
| `HALF_PI` | `81 21 A2 DA 0F 49` | π/2 | `0E8B:8B97` |
| `THREE_HALF_PI` | `83 99 F9 E3 CB 16` | 3π/2 | `0E8B:8B86` |
| `PI` | `82 21 A2 DA 0F 49` | π | `0E8B:8B5B` |
| `FOUR` | `83 00 00 00 00 00` | 4 | `0E8B:8BBA` |

The negative slopes are the same bytes with the sign bit set ([real48](../core/real48.md) §2).

### 3.6 Home point (guidance target when no marked ship is live)

| Flight | Home tile | Provenance |
|---|---|---|
| default | (107, 104) | `0E8B:82DD` |
| 12, 15 | (107, 102) | |
| 13 | (180, 155) | |
| 19 while phase < 100 | (123, 72) | |

The home point's marker is element 21 of `g_SlotMarker` (DS:396C): 3Dh, or 32h in flight 19 until its phase-2 rule sets 3Dh ([mission-logic](../sim/mission-logic.md) §3.3).

## 4. Behaviour

### 4.1 Mission start (order)

1. `Gfx_InitUnchained`; load the data files ([asset-load-sites.md §5](../../re/asset-load-sites.md#5-flight-sim_flymission-0e8bb880-once-per-mission)), tiles into VGA rows 496..729.
2. Set all DAC entries to black; plot `td.win` into rows 0..31; show page 1 (start address 5AC0h) with the split at 13Fh; set the display end to 192 rows (`0E8B:CC40`).
3. Per-mission set-up (`0E8B:60D8`): objective text into the ticker strip, ticker := 1 (all flights except 20), mission-specific ship state.
4. Draw the weapon icon for the class's primary weapon (§4.9).
5. Armour and shields from the class tables. In the flight palette (not the DAC): the eight compass-lamp entries := (0, 0, 0); entry 26h := (0, 0, 0) when there are no missiles, else ((4 − cooldown) × 15, 5, 8); entry 90h := (0, 0, 0). Draw the missile column (§4.5).
6. Fade in from the flight palette; enter the tick loop.

### 4.2 Each tick (`UI_DrawInstruments`, tick-order step 19)

```text
guidance line and compass lamps            (4.3)
low-armour warning                         (4.4)
armour bar, then shield bar                (4.4)
missile lamp, then missile column          (4.5)
radar: build, then copy to the bar         (4.6)
ticker step, then F6 indicator             (4.7, 4.8)
```

Everything except the guidance dots goes straight to VGA rows 0..31, i.e. it is visible in the frame shown at the end of this tick. The guidance dots go into the back page and appear with the flip of the same tick.

### 4.3 Guidance line and compass lamps (`UI_DrawGuidanceLine`, `0E8B:82DD`)

Let P = (px, py) = (viewX + 5, viewY + 3). All differences are 16-bit signed integers; Real48 operations are those of [real48](../core/real48.md) and must be bit-exact.

**Sector of a point T = (tx, ty):**

```text
sector(T):
    if tx = px:  return N if ty < py else S
    q := Real(ty − py) / Real(tx − px)                   (Real48 divide of two LongInt conversions)
    s := E                                               (initial value)
    if q < SLOPE_LO and q > −SLOPE_LO:  s := E if tx > px else W
    if q ≥ SLOPE_LO and q ≤ SLOPE_HI:   s := SE if tx > px else NW
    if q > SLOPE_HI:                    s := N if ty < py else S
    if q < −SLOPE_HI:                   s := N if ty < py else S
    if q ≤ −SLOPE_LO and q ≥ −SLOPE_HI: s := NE if tx > px else SW
    return s
```

(Screen y grows downwards, so a positive q with tx > px is south-east.) The five tests are made in this order with Real48 compares; the ranges do not overlap, so exactly one applies.

**Lamps:**

```text
red[8] := 0; blue[8] := 0
for s = 1..20 with class(s) ≠ FFh and marker(s) ≠ 0:   red[sector(tile(s))] += 10
blue[sector(home)] += 63                                   (home point, 3.6)
for each sector d: if red[d] ≠ 0: red[d] += 20;  red[d] := min(red[d], 63)
for each lamp, in the order E, NE, N, NW, W, SW, S, SE:
    Pal_SetEntry(lamp entry, R = red[d], G = 0, B = blue[d])
```

So one ship gives red 30, two 40, three 50, four 60, five or more 63; the home direction is blue 63. Lamp entries (§3.2): E 31h, NE C1h, N 86h, NW 85h, W 2Fh, SW 2Eh, S 30h, SE 24h. The ship test includes every live marked slot, whatever its marker value.

**Target of the line:** the lowest slot *s* = 1..20 with marker(*s*) ≠ 0 and class(*s*) ≠ FFh; if there is none, the home point with marker element 21.

**Angle θ to target T:**

```text
dxp := px − tx;  dyp := py − ty          (written in the original as viewX − tx + 5, viewY − ty + 3)
if tx = px:
    θ := THREE_HALF_PI if viewY − ty + 2 > 0 else HALF_PI          (note the +2, not +3: §5)
else if dxp < 0:  θ := ArcTan(Real(dyp) / Real(dxp))
else:             θ := ArcTan(Real(dyp) / Real(dxp)) + PI           (Real48 add)
```

**Dots:**

```text
ox := 4 × Round(Real(viewSubX) / FOUR)                 (Real48 divide, Round = half away from zero)
for j = 0..5:  r := j + 20
    x[j] := Round(Cos(θ) × Real(r)) + 144 + ox          (page x)
    y[j] := Round(Sin(θ) × Real(r)) + 100 + viewSubY + 232 × page + 1     (VGA row)
for j = 0..5:  colour 0 at (x[j] + 1, y[j]), (x[j] + 1, y[j] + 1), (x[j], y[j] + 1)
then, by the target's marker (3.4):
    32h or 39h: dot j in its colour at (x[j], y[j]), j = 0..5
    3Dh: flight 7 only: if dead(2, 3, 4): mission phase := 100     (a game rule; mission-logic 5.7)
         palette mode 2: DAC of the home lamp entry := (0, 0, 63) (DAC only); all six dots in that entry
         otherwise: dot j in its colour
    any other marker: only the shadows
```

On screen the dots are centred on playfield pixel (160, 85): x = 160 + Round(cos θ · r), y = 85 + Round(sin θ · r). Shadows of later dots can be overwritten by earlier dots' colours only through the drawing order above (all shadows first, then all dots). Two dots can coincide; the later one wins.

### 4.4 Bars and the low-armour warning

```text
if armour ≤ 6:  Pal_SetEntry(90h, R = (8 − (missionTicks mod 8)) × 8, G = 0, B = 0)
for each bar (armour at rows 23..26 first, then shields at rows 11..14), value v:
    if v > 0:
        for J = 0 .. min(v, 6):   left pixel (2J + 6) := B0h, right pixel (2J + 7) := 7Ah
        if v > 6:  for J = 7 .. min(v, 12):  left := 48h, right := 96h    (mode 2: B0h, A5h)
        if v > 12: for J = 13 .. min(v, 28): left := B2h, right := A9h    (mode 2: B0h, A5h)
    if 0 ≤ v < 28:
        for J = (0 if v = 0 else v + 1) .. 28:  both pixels := 90h
    (every column J covers the bar's 4 rows)
```

A value v ≥ 1 lights v + 1 segments (J = 0..v), so the bar is full from 28 up. Negative values (impossible in play) draw nothing and clear nothing. When armour ≤ 6 the bar background and the empty segments (all colour 90h) pulse red in an 8-tick saw tooth; the entry is left as it is once armour is above 6 again.

### 4.5 Missile lamp and missile column

```text
if missiles = 0: Pal_SetEntry(26h, 0, 0, 0)
else:            Pal_SetEntry(26h, R = (4 − cooldown) × 15, G = 5, B = 8)
Fill4x2(x, y, c) paints the 4 × 2 block x..x+3, y..y+1 in colour c (386E:18D8, latched all-planes write)
if missiles > 26:  for i = 0..26: Fill4x2(100, 28 − i, 26h + (i mod 2))
if missiles = 0:   for i = 0..26: Fill4x2(100, 28 − i, 0)
if 1 ≤ missiles ≤ 26:
    for i = 0..missiles:              Fill4x2(100, 28 − i, 26h + (i mod 2))
    for i = missiles + 1 .. 26:       Fill4x2(100, 28 − i, 0)
```

Result: with *m* missiles (1 ≤ m ≤ 25) rows 29 − m .. 29 are lit (m + 1 rows), row *r* in colour 26h + ((29 − r) mod 2), and rows 2 .. 28 − m are 0. With 26 or more missiles all rows 2..29 are lit and row 2 has colour 26h. The lamp and the even rows glow red when a missile is ready, fade during the 4-tick cooldown, and go black with no missiles.

### 4.6 Radar

```text
buffer B: 28 × 28 bytes at DS:9780 (persistent between ticks)
for I = 3..23, J = 3..23:  B[I, J] := k_TileRadarColour[map(viewX + I − 9, viewY + J − 10)]
for s = 1..20 with class(s) ≠ FFh and viewX − 10 < tileX(s) < viewX + 17 and viewY − 10 < tileY(s) < viewY + 18:
    B[tileX(s) − viewX + 10, tileY(s) − viewY + 10] := 55h
for i = 1..50 with shotTileX(i) ≠ FFh, inside the same window:
    B[shotTileX(i) − viewX + 10, shotTileY(i) − viewY + 10] := 22h
B[14, 13] := 46h                                         (player)
copy B[3 + k, 4 + r] to HUD (148 + k, 6 + r) for k, r = 0..19     (Gfx_HudRadarBlit 386E:0000)
```

Equivalent per radar pixel (k, r): the map tile (viewX − 6 + k, viewY − 6 + r); a ship or shot at tile (tx, ty) is plotted at (tx − viewX + 7, ty − viewY + 6) when that is inside 0..19 — **one pixel east of its own map tile** (§5); the player at (11, 9), which is tile (viewX + 5, viewY + 3). Drawing order: map, ships in slot order, shots in slot order, player. Buffer cells outside the copied window are never displayed, so a port need not keep the buffer.

### 4.7 Message ticker

- **Strip.** Off-screen VGA rows 735..740 hold the text: messages are drawn with the small font, opaque, pen (0, 735), ink 37 or 42, background from `td.lt2` ([text](text.md) §4.1–4.2), glyph rows 736..740. Objectives and jurisdiction warnings use `Txt_FlightDrawStringOpaque` (`0E8B:45EF`); mission messages use the copy `2404:030A`/`2404:0000`, whose advance table gives 0 to the digits other than 6. No E1 message contains a digit, so both render as text.md. A new text overwrites the strip without clearing it first. The strip is VGA memory: a pixel at x ≥ 352 lands at (x − 352, y + 1). Every E1 message's trailing spaces reach past x = 352 (pen end 355..397), so their background cells are also written at the left of rows 737..741 (§5).
- **Window.** `UI_HudTickerBlit(pos)` (`0E8B:0000`) copies 14 bytes × 5 rows with latched writes from VGA byte (736 × 88 + pos) to HUD byte (13 × 88 + 64): strip pixels x = 4·pos .. 4·pos + 55, rows 736..740 → HUD (256..311, 13..17). Bytes past 87 in a row continue in the next row (pos ≥ 75).
- **Step** (`UI_HudTickerAndIndicator` `0E8B:7FDF`):

```text
if g_EdgeHit = 1:                                    (set by the player's move at the map limits)
    draw jurisdiction.warnings[1] if flight is 13 or 14 else [0]   (ink 37)
    ticker := 1; g_EdgeHit := 0
if ticker ≥ 1 and back page = 0:
    blit(ticker); ticker += 1; if ticker > 78: ticker := 0
```

The ticker moves on page-0 ticks only: a message takes 78 page-0 ticks (155 or 156 ticks) and the window then stays on its last position until the next message. Who posts messages and restarts the ticker: [mission-logic](../sim/mission-logic.md) §4.5, the M key ([weapons](../sim/weapons.md) §4.1).

### 4.8 F6 transmission indicator (same routine, after the ticker)

```text
if comm pending ≠ 0 and phase = 0:  play sound 200 (income) at the view (source FFFFh)
if comm pending = 1:
    phase := phase + 1 if phase < 7 else 1
    draw td.fac frame (phase if phase < 5 else 8 − phase)
if comm pending = 0 and phase ≠ 0:  phase := 0; draw frame 0
```

`phase` is `g_CommIndicatorPhase` DS:1705. The frame sequence is 1, 2, 3, 4, 3, 2, 1, 1, 2, … one step per tick; frame 0 is the blank lamp. `Gfx_HudFacFrame(f)` (`386E:012B`) writes the 784 bytes of frame *f* (7 bytes × 28 rows × 4 planes, direct writes) to HUD (220, 2). Comm pending is raised and cleared by mission logic and the comm viewer (mission-logic §4.5–4.6).

### 4.9 Weapon icon

`for row = 1..18, col = 1..36: pixel (178 + col, 6 + row) := td.wea byte (g_Weapon − 1) × 648 + (row − 1) × 36 + (col − 1)` (opaque), drawn at flight start and whenever Tab changes `g_Weapon` (`0E8B:F4F8..F6A7`). Weapons 1..3 are the primary-weapon icons and 4..6 the missile icons ([weapons](../sim/weapons.md) §3.2).

## 5. Edge cases and original bugs

| Quirk | Original behaviour | Port policy |
|---|---|---|
| Split screen | The HUD is unaffected by page flips and scrolling | A port composing a 320×192 frame draws the HUD after the playfield every frame |
| Ticker at half rate | Advances on page-0 ticks only | Preserve |
| Radar ships one pixel east | Ships and shots are plotted at buffer column tile − view + 10, the map at tile − view + 9 | Preserve (T0); OB-019 |
| Guidance, same column | Directly above/below uses `viewY − ty + 2 > 0`: a target exactly one row above the player tile gets the "down" angle | Preserve; OB-020 |
| Low-armour red 64 | When missionTicks mod 8 = 0 the entry is set to 64, which the 6-bit DAC shows as 0; the flight palette keeps 64 (fades read it) | Preserve the stored value; display `v & 63` (OB-142) |
| Low-armour colour after recovery | Entry 90h keeps its last red value when armour rises above 6 (cheat chord, flight-20 rule) | Preserve (OB-142) |
| Missile column with 26 vs 27+ missiles | Both light rows 2..29; row 2 keeps colour 26h | Preserve (OB-142) |
| Ticker strip wrap | Text past x = 351 continues one VGA row lower at x − 352; the trailing spaces of every message write background into rows 737..741 at x ≈ 0..46 | Preserve (emulate the strip as 352-pixel rows); U-185 asks whether any visible ink is hit |
| Dead code at the player's death | `0E8B:E555` clears a strip (x 8..131, y 12..13) when the word DS:18BA = 2; nothing writes DS:18BA, so it never runs | Not ported (U-186) |
| Flight-7 rule in the HUD routine | The guidance routine sets phase 100 in flight 7 | Implement it in the mission runtime, called at the same point of the tick (mission-logic §5.7) |
| OB-090: weapon icon 0 | With `g_Weapon` = 0 (a class-0/1 flight that is the first flight of the program run, [weapons](../sim/weapons.md) §3.2; in E1 only flight 19) the icon loop reads the 648 bytes **before** the `td.wea` block (heap memory, offset −648..−1). C-0069 (flight 19 started directly) shows a solid 36 × 18 block of colour B2h there from the first flight frame to the end, the vehicle switch included | Preserve as observed: fill the icon cell (x 179..214, y 7..24) with colour B2h when the weapon is 0 (LIKELY: heap contents, seen in one capture) |

## 6. Test vectors

| # | Setup / input | Expected output | Evidence |
|---|---|---|---|
| 1 | E1 mission 1, first flight frame | Display 320×192; rows 160..191 equal `td.win` except where elements are drawn | C-0002 f3473.. |
| 2 | C-0002 frame 3543 (RAM at the same frame: armour 25, shields 25, missiles 4, cooldown 0, palette mode 0, view (102, 104, 32, 16)) | Shield bar (232 px), armour bar (232 px), missile column (112 px), radar (400 px): **0 differing pixels** against the model of §4.4–4.6 | C-0002 `fb\f00003543`, `mem\f00003543`; RE-6 check script (scratch) |
| 3 | Same frame: live marked slots 1 and 2 at tiles (124, 75) and (125, 75), player tile (107, 107), home (107, 104) | DAC: NE lamp C1h = (40, 0, 0); N lamp 86h = (0, 0, 63); the other six lamps (0, 0, 0); missile lamp 26h = (60, 5, 8) | C-0002 f3543 palette |
| 4 | Same frame: guidance target slot 1 | Dots at playfield (169, 67) B0h, (170, 66) F9h (dots 1 and 2 coincide), (171, 65) 7Ah, (171, 64) 5Ch, (172, 63) 5Ch, with colour-0 shadows right and below | C-0002 f3543 |
| 5 | Radar pixel of a ship at (124, 75) with view (102, 104) | (tx − viewX + 7, ty − viewY + 6) = (29, −23): outside, not drawn; player dot at HUD (159, 15) colour 46h | static + C-0002 |
| 6 | `k_TileRadarColour` | 71 bytes at file offset 41154h; tiles 0..14 dark colours, 15..16 = E6h, 17..48 = E4h, 62..70 = 50h | file decode (`tdtool exe-res`) |

## 7. Presentation and audio hooks

The transmission indicator pairs with sound effect 200 (`income.voc`, [sfx-policy](../audio/sfx-policy.md)), started once per pending transmission.

## 8. Open questions

- [ ] U-185: does the ticker-strip wrap (§4.7) ever erase visible ink? (Static estimate: message `e1.flight_messages.01[23]` writes background up to strip x = 46, where its first letter starts.) Check a capture of that message.
- [ ] U-186: what DS:18BA was meant to be (the dead strip clear at the player's death).
- [ ] Palette mode 1 (infra-red) recolours the DAC at flight start (`0E8B:CAD2..CBAA`, Real48 divides); its formula is not specified here (flights 4 and 8 start in mode 1).

## 9. Provenance

| Claim | Address / capture | Method | Confidence |
|---|---|---|---|
| 192-row display, split at 13Fh, HUD = VGA rows 0..31 | `file0001:0E8B:CC40`, `0E8B:B880`, `386E:19A0`; C-0002 | static + capture | CONFIRMED |
| Pages and start-address formula | `386E:189C`, `386E:191C`, `0E8B:7E91` | static | LIKELY |
| Order inside `UI_DrawInstruments` | `0E8B:8E87..964D` | static | CONFIRMED (code) |
| Bars, low-armour pulse | `0E8B:8E8E..9322` (shields from `0E8B:90FF`) | static + C-0002 f3543 | CONFIRMED |
| Missile lamp and column, `Fill4x2` | `0E8B:9323..9428`, `386E:18D8` | static + C-0002 | CONFIRMED |
| Radar build and copy | `0E8B:9429..9644`, `386E:0000` | static + C-0002 (0 px) | CONFIRMED |
| `k_TileRadarColour` | DS:0264 | static + file | CONFIRMED |
| Guidance line and lamps | `0E8B:82DD..8E86` (listing for the Real48 constants) | static + C-0002 | CONFIRMED |
| Lamp places in `td.win` | `td.win` decode | file | CONFIRMED |
| Ticker, indicator | `0E8B:7FDF`, `0E8B:0000`, `386E:012B` | static | LIKELY |
| Weapon icon | `0E8B:B880` (two copies of the draw loop) | static | LIKELY |

## 10. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-02 | RE-3 | Draft from static analysis (M2-09) |
| 2 | 2026-11-02 | RE-6 (Archaeologist + Spec author) | M6-07 RE part: bars, low-armour pulse, missile lamp and column, compass lamps, guidance line maths, radar markers, ticker strip and window rules, F6 indicator rule, weapon icon source corrected (`g_Weapon`, not the class), `k_TileRadarColour`, test vectors from C-0002 |
| 3 | 2026-11-06 | RE-15 | §5: OB-090, the weapon icon for weapon 0 (flight 19 as the first flight of a run) shows colour B2h (C-0069) |
| 4 | 2026-11-06 | RE-18 (Spec author) | §5: the low-armour and missile-column quirks mirrored as OB-142 (no behaviour change) |
