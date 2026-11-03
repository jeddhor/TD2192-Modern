# In-flight modal screens and flight presentation details

| | |
|---|---|
| **Spec ID** | `flow/flight-screens` |
| **Revision** | 1 |
| **Status** | Draft |
| **Fidelity tier** | T0 for the screen images (layouts, colours, palette formulas); T1 for the state the screens change (abort flag, mission phase, key bytes, `Random` calls); T2 for timing (fade and typing speed) |
| **Confidence** | LIKELY overall (static analysis of every routine named below). CONFIRMED by capture where §6 says so: computer menu, ship library, help, pause, F5 map (C-0012), transmission screen (C-0013), palette modes 1 and 2 and the IR notice (C-0014) |
| **Tasks** | E1 screens (RE-8); feeds M6-07, M7, M8, M9 |
| **Unknowns** | U-220..U-226 (new); U-122, U-124 answered in [screens.md](screens.md) |

Related specs, not repeated here: tick order [sim/tick-order](../sim/tick-order.md) (where each screen is called), comm phases [sim/mission-logic](../sim/mission-logic.md) §4.5–4.6, HUD [render/hud](../render/hud.md), text drawing [render/text](../render/text.md), weapons [sim/weapons](../sim/weapons.md), afterburner [sim/flight-model](../sim/flight-model.md) §4.7, camera [sim/camera-viewport](../sim/camera-viewport.md).

## 1. Summary

During a flight the original opens four modal screens from inside the tick loop: the **Esc computer menu** (four items: return, ship library, help, quit; quit sets `g_MissionAborted`), the **F6 transmission screen** (a 256×128 panel over the playfield with two portraits and two scrolling text boxes, shown only when the flight and mission phase match), the **F5 tactical map** (the whole 200×200 map as one pixel per tile, live marked ships, and five info lines for a selected target; Tab cycles targets) and the **pause** (a text line over the playfield until any key). None of them advances the simulation: ticks stop while they are open. The F5 screen is **not** a ship-switch screen (earlier name `UI_ShipSwitchScreen`); it changes no game state. This spec also gives the **infra-red palette modes** 1 and 2 (formulas, the start-of-flight notice, and how the per-tick hit and white flashes behave in each mode) and the presentation details left open by the flight specs: afterburner flame positions, explosion spawning and drawing, and how shots are drawn.

## 2. Definitions and units

| Term | Meaning |
|---|---|
| VGA row | Unchained mode 13h row of 352 pixels (88 bytes). Rows 0..31 hold the HUD bar; flight page 0 is rows 32..263, page 1 rows 264..495 ([render/hud](../render/hud.md) §2) |
| Page row | VGA row − 32 − 232 × page |
| Sprite y | The y argument of `Gfx_DrawSprite32` (`386E:024A`) and the per-heading blits: page row + 32 (the blitter adds 232 rows for page 1). Its x is rounded down to a multiple of 4 (planar blit). Pixel value 0 is transparent. Only the low byte of y is used (y ≤ 255) |
| Text y | The y argument of the flight text routines (`0E8B:4947` transparent, `0E8B:45EF` opaque, `0E8B:4C9F` typed): an absolute VGA row. Callers add 232 × page themselves when they mean the back page |
| R | `4 × Round(g_ViewSubX / 4)` (Real48 `Round`, halves up), the horizontal scroll quantum ([camera-viewport](../sim/camera-viewport.md) §3) |
| Flight view | The visible playfield shows page pixels from (R − 16, viewSubY − 16). A page pixel (px, pr) is at screen (px − R + 16, pr − viewSubY + 16) |
| Key byte | `g_KeyDown[sc]` DS:1A5C + scan code (+100h for E0 keys), 1 while held ([globals](../../re/globals.md) §3). Flight screens poll these bytes, except the F5 map, which reads the BIOS keyboard |
| Flight palette | `g_FlightPalette` DS:13E6 (768 bytes, 6-bit, `td.pal` as patched by the flight). `SetDac(i, r, g, b)` = `Gfx_SetDacEntry` (`386E:1901`): DAC only |
| Font background | Byte 650 of the loaded `td.lt2` block (`g_Font5`, DS:9770). In flight it is the file's value **178** (C-0012 RAM); the menu and the F5 map set it to 0 while they run and restore it |
| `FadeOut`, `FadeIn` | `Gfx_FadeOutFlight` `0E8B:0426`, `Gfx_FadeInFlight` `0E8B:01F6` (§4.6.3) |

## 3. Data

### 3.1 Esc computer menu (`td.men` bytes 0..15,999, 160×100)

| Cursor | Item | Palette entry of its letters | Action |
|---:|---|---:|---|
| 0 | Return to game | 107 | leave the menu |
| 1 | Ship library | 108 | §4.1.2, then the menu again |
| 2 | Help | 109 | §4.1.3, then the menu again |
| 3 | Quit | 110 | `g_MissionAborted` := 1, leave the menu |

Item letter boxes in the image (CONFIRMED, file): entry 107 rows 30..36, 108 rows 40..46, 109 rows 50..56, 110 rows 70..76 of the 160×100 image.

### 3.2 Ship library layout (`0E8B:7374`)

Sprites are records 0..8 of `td.shp` (1,024 bytes each, read in file order), drawn transparent at these **screen** positions; the text lines are `ship_library[i]` drawn opaque (background 0) at pen (x, y):

| Sprite | Screen (x, y) | Text pen x | Text lines (y : index, ink) |
|---:|---|---:|---|
| 0 | (4, 12) | 40 | 12 : 0, 96 · 18 : 1, 92 · 24 : 2, 92 · 32 : 3, 94 |
| 1 | (4, 49) | 40 | 49 : 4, 96 · 55 : 1, 92 · 61 : 5, 92 · 69 : 6, 94 |
| 7 | (4, 92) | 40 | 92 : 7, 96 · 98 : 8, 92 · 104 : 9, 92 · 112 : 10, 94 |
| 8 | (4, 123) | 40 | only in flights ≥ 4: 123 : 11, 96 · 129 : 12, 92 · 135 : 13, 92 · 143 : 14, 94 |
| 2 | (164, 28) | 200 | 28 : 15, 96 · 34 : 12, 92 · 40 : 16, 92 · 48 : 17, 94 |
| 3 | (164, 71) | 200 | 71 : 18, 96 · 77 : 19, 92 · 83 : 16, 92 · 91 : 20, 94 |
| 4 | (164, 109) | 200 | 109 : 21, 96 · 115 : 22, 92 · 121 : 16, 92 · 129 : 23, 94 |
| 5 | (164, 150) | — | hatched, no text |
| 6 | (4, 152) | — | hatched, no text |

"Hatched": after all nine sprites are drawn, a 32×32 mask whose pixel (column c, row r) is colour 1 when r + c is even and transparent otherwise (a one-pixel checkerboard) is drawn over sprites 6 and 5, and over sprite 8 when the flight number is below 4. Colours 92, 94, 96 are `td.pal` entries (palette mode is 0 inside the menu).

### 3.3 Transmissions (F6), loaded by `2A15:01EC`, `0593`, `0A02`, `0ED8`, `1273`, `1591`, `18BE`, `1D76`, `20DD` for n = 1..9

Each loader fills a table of 51-byte ShortStrings: slot −1 = header A (DS:32C4), slot 0 = header B (DS:32F7), slots 1.. = lines (DS:32F7 + 51k); a speaker byte per line (DS:3923 + k: 1 = box A, 2 = box B); and the two portraits (DS:3942 = record of `td1.ccm` for box A, DS:3943 for box B). Slot k takes the string `e1.comms.0n[j]` (resource map) listed below. Two string values are **markers**, recognised by content: `comm.prompts[0]` (the "press Space" prompt) and `comm.prompts[1]` (the closing line). Where a slot is written twice (n = 1, slot 18), the second write counts.

| n | Portraits A / B | Header A, B | Lines k = 1.. as `j` (`P` = prompt marker, `E` = end marker) | Speaker of lines |
|---:|---|---|---|---|
| 1 | 1 / 0 | 0, 1 | 2 3 4 5 6 P 8 9 10 11 12 P 13 14 14 15 14 E | A for 1..15, B for 16..18 |
| 2 | 2 / 0 | 0, 1 | 2 3 4 5 P 7 8 9 P 10 11 12 13 14 15 15 E | A 1..6, B 7..10, A 11..13, B 14..15, A 16; 17 is not written (keeps an earlier value, §5) |
| 3 | 3 / 0 | 0, 1 | 2 P 4 5 6 7 P 8 9 10 11 12 P 13 14 15 13 P 13 16 17 18 19 P 4 20 21 E | A 1..3, B 4..8, A 9..19, B 20..25, A 26..28 |
| 4 | 3 / 0 | 0, 1 | 2 3 4 P 6 7 8 8 9 10 11 12 P 13 14 15 16 17 P 13 E (indices into the 19-string `e1.comms.04`) | A 1..5, B 6..8, A 9..14, B 15..21 |
| 5 | 4 / 0 | 0, 1 | 2 3 4 5 P 7 8 9 10 11 P 12 E | A 1..6, B 7..13 |
| 6 | 5 / 0 | 0, 1 | 2 3 4 5 P 7 8 9 10 11 P 12 13 14 15 E | A 1..6, B 7..12, A 13..16 |
| 7 | 6 / 0 | 0, 1 | 2 3 P 5 6 P 7 8 9 10 P 11 12 10 P 13 14 15 E | A 1..3, B 4..6, A 7..11, B 12..15, A 16..19 |
| 8 | 7 / 0 | 0, 1 | 2 3 4 5 P 7 8 9 10 P 11 12 13 14 15 P 16 17 18 19 20 21 22 23 E | A 1..21, B 21..25 (B wins for 21) |
| 9 | 0 / 8 | 0, 1 | 2 3 4 5 P 7 8 9 10 E | B 1..6, A 7..10 |

`j` for a marker is the marker's own index in the transmission's string run (for example n = 1: `P` = j 7, `E` = j 16); the screen only compares contents. Single-space strings (n = 1, j 14; n = 2, j 15; n = 3, j 21) are blank lines. All strings are typed from the exe at run time (ADR-0003).

### 3.4 F5 map: target entries and info lines

Entries 0..31 in three DGROUP arrays: selectable flag DS:1F74 + e, map X DS:1F94 + e, map Y DS:1FB4 + e; and five 31-byte strings (`string[30]`) per entry at DS:1FD4 + 155e + 31l, l = 0..4. Entry 0 is the player, entries 1..20 the mission slots, 21..31 fixed points. At flight start `Msn_LoadTargetInfo` (`1EDE:0E6A`, called by `Msn_Setup` `0E8B:60D8`) clears the flags of entries 1..30 and then, per flight, writes the lines below (indices into `e1.target_info`, each truncated to 30 characters) and the fixed points. Entries not listed keep whatever an earlier flight of the same program run left (zero-length strings after start-up). Flights 14 and 21 write nothing.

| Flight | Entry: lines l = 0..4 | Fixed points (flag 1) |
|---:|---|---|
| 1 | 0: 2 3 4 1 1; 1..2: 5 6 7 8 9; 3..4: 5 6 7 10 9; 5..7, 9..10: 5 6 7 11 12; 8: 13 6 7 14 15; 11: 13 6 7 16 1; 12..20: 17 18 7 1 1; 21: 0 1 1 1 1 | 21 at (107, 104) |
| 2 | 0: 19 3 4 1 1; 1: 13 6 7 20 1; 2: 5 3 21 22 23; 3..4: 5 6 7 24 1; 5: 13 6 7 25 1; 6..7: 13 6 7 26 1; 21: 0 1 1 1 1 | 21 at (107, 104) |
| 3 | 0: 19 3 4 1 1; 1: 19 3 27 1 1; 2..3, 6..7, 12: 13 6 7 26 1; 4: 19 3 28 29 30; 5: 31 6 7 32 33; 8: 31 6 7 32 34; 14..15: 17 18 7 1 1; 21: 0 1 1 1 1 | 21 at (107, 104) |
| 4 | 0: 19 3 4 1 1; 1: 19 3 35 1 1; 2..3: 13 6 7 26 1; 4..5, 7: 36 6 7 26 1; 6: 5 37 7 1 1; 8: 17 18 7 38 1; 9: 17 18 7 39 1; 21: 0 1 1 1 1 | 21 at (107, 104) |
| 5 | 0: 19 3 4 1 1; 1: 17 3 40 41 42; 3..5, 7: 13 6 7 26 1; 21: 0 1 1 1 1 | 21 at (107, 104) |
| 6 | 0: 43 3 4 1 1; 3: 36 6 44 26 1; 5, 7..9: 13 6 7 26 1; 21: 0 1 1 1 1 | 21 at (107, 104) |
| 7 | 0: 43 3 4 1 1; 1: 45 6 46 47 48; 2..12: 49 50 1 51 52; 13..14: 36 6 46 26 1; 21: 0 1 1 1 1 | 21 at (107, 104) |
| 8 | 0: 43 3 4 1 1; 1: 43 3 35 1 1; 3..6: 13 53 7 1 1; 8..9: 36 6 44 26 1; 12..13: 13 6 7 26 1; 15: 54 6 7 55 56; 16..18: 54 6 7 57 56; 21: 0 1 1 1 1 | 21 at (107, 104) |
| 9 | 0: 58 3 4 1 1; 1..4: 59 6 7 60 61; 5..12: 13 6 7 62 63; 13..18: 13 53 7 1 1; 21: 0 1 1 1 1 | 21 at (107, 104) |
| 10 | 0: 58 3 4 1 1; 1: 19 3 35 1 1; 3..5, 14..16: 17 53 7 64 65; 7..11: 13 6 7 66 1; 12: 36 6 67 68 69; 21: 0 1 1 1 1 | 21 at (107, 104) |
| 11 | 0: 43 3 4 1 1; 1..3: 43 72 7 73 74; 4: 19 3 70 1 1; 5: 19 3 71 1 1; 7: 36 6 67 1 1; 8..9, 17..18: 13 6 7 75 1; 11..15: 17 18 7 76 1; 21: 0 1 1 1 1 | 21 at (107, 104) |
| 12 | 0: 78 3 4 1 1; 1..5: 5 79 7 80 81; 7..11: 13 6 7 82 83; 12..13: 36 6 7 82 83; 15: 19 3 70 1 1; 16: 19 3 71 1 1; 21: 77 1 1 1 1 | 21 at (107, 102) |
| 13 | 0: 36 6 4 1 1; 1..2: 13 6 7 1 1; 8..9: 43 3 7 66 1; 11..13: 19 3 7 66 1; 15..17: 85 3 7 66 1; 21: 84 1 1 1 1 | 21 at (180, 155) |
| 14 | — (F5 is disabled in flight 14) | — |
| 15 | 0: 78 3 4 1 1; 1..2: 17 6 7 86 1; 3..5, 10..12: 87 6 7 88 89; 8..9: 54 6 7 86 1; 15: 19 3 90 1 1; 16: 19 3 91 1 1; 21: 77 1 1 1 1 | 21 at (107, 102) |
| 16 | 0: 19 3 4 1 1; 1..3, 8..10: 92 6 7 93 94; 4..5, 11..13: 95 6 7 96 97; 6: 87 6 7 96 97; 17: 85 3 90 1 1; 21: 0 1 1 1 1 | 21 at (107, 104) |
| 17 | 0: 43 3 4 1 1; 1: 19 3 98 99 1; 3: 13 6 46 1 1; 5..6, 8, 12..14: 13 6 46 100 101; 9..10: 36 6 46 100 101; 21: 0 1 1 1 1 | 21 at (107, 104) |
| 18 | 0: 43 3 4 1 1; 1: 43 3 90 1 1; 3, 7: 104 6 46 102 103; 4..6, 8..9, 12..14, 16: 13 6 46 102 103; 10..11, 15, 17..18: 105 6 46 102 103; 21: 0 1 1 1 1 | 21 at (107, 104) |
| 19 | 0: 5 6 4 1 1; 1..4, 9..10: 36 6 46 109 1; 5..8: 13 6 46 109 1; 19: 5 3 110 111 112; 21: 0 1 1 1 1; 22: 106 1 1 107 108 | 21 at (107, 104); 22 at (125, 73) |
| 20 | 0: 43 3 4 1 1; 1..5: 113 6 46 114 115; 7..8: 36 6 46 116 117; 21: 0 1 1 1 1 | 21 at (107, 104) |
| 21 | — | — |

Flight 19's `Msn_UpdateEvents` later overwrites entry 0's lines ([mission-logic](../sim/mission-logic.md) §5.19).

### 3.5 Constants

| Constant | Value | Provenance |
|---|---|---|
| Menu image position | (80, 50) of the back page, shown from the page origin | `0E8B:79E5..7A0C` |
| Menu item fade | DAC entry 107 + cursor: (k, 0, 0), k = 32..63 up, 63..32 down, one step per retrace start | `0E8B:7B1E..7B4A`, `7DD6..7E02` |
| Menu poll delay | `Crt.Delay(100)` per poll | `0E8B:7B4C` |
| Comm panel | `td.cmm`, 4 rows × 8 cells, screen (32 + 32·col, 16 + 32·row) | `0E8B:5459..5503` |
| Comm portraits | A at screen (48, 32), B at (48, 96) | `0E8B:51FE`, `0E8B:530B` |
| Comm text | header A pen (92, 35), header B (92, 87), ink 45 opaque; line A typed at (92, 67), line B at (92, 119), ink 40 (lines) or 43 (markers) | `0E8B:5062`, `50C3`, `5124`, `5188` |
| Comm scroll boxes | A: screen rows 42..72, B: rows 94..124, x 92..271 (45 bytes) | `0E8B:0061`, `0E8B:012A` |
| Pause / IR notice pen | screen (96, 76); ink 37 (`0x25`) | `0E8B:E147..E18D`, `E21F..E4AD` |
| F5 map | tile (x, y), x, y = 1..200, at screen (x, y − 1); text pen x 205 | `0E8B:A798..A7EF` |

## 4. Behaviour

### 4.1 Esc computer menu (`UI_FlightMenu` `0E8B:794C`, tick-order step 25)

Called when the Esc key byte is 1 and the mission is not over, after the page flip and the page toggle of the tick (so `g_DrawPage` is the page **not** on screen).

```text
FadeOut (palette mode as it is)
saved_bg := font background; font background := 0
DS:16E6 := 0                                  (U-223)
saved_mode := g_PaletteMode; g_PaletteMode := 0
cursor := 0
clear the back page (all 232 rows, colour 0)
copy td.men bytes 0..15999 (160 x 100) to back-page pixels (80 + j, 50 + i)
split line := 400 (HUD band off; the display still ends after 192 rows)
for e = 107..110: save g_FlightPalette[e]; g_FlightPalette[e] := (32, 0, 0)
show the back page from its origin (start address = page base)
FadeIn (mode 0)
clear key bytes 1..600; joystick Y := 0
repeat                                         (outer loop)
    changed := false; done := false; shown := cursor
    for k = 32..63: wait for retrace start; SetDac(107 + cursor, k, 0, 0)
    repeat                                     (inner loop)
        Crt.Delay(100); read the joystick if the controller is the joystick
        if Up (byte 48h or E0 48h) or joystick up:
            cursor := cursor - 1, 0 -> 3; clear the bytes 30h and E0 48h; changed := true
        if Down (byte 50h or E0 50h) or joystick down:
            cursor := cursor + 1, 3 -> 0; clear the bytes 32h and E0 50h; changed := true
        if Esc byte: done := true; changed := true; cursor := 1
        if Space, Enter, keypad Enter, or joystick button 1 or 2:
            cursor 3: g_MissionAborted := 1; done := true; changed := true
            cursor 2: changed := true; FadeOut; UI_FlightHelp(back page) (4.1.3); redraw the menu image; FadeIn
            cursor 1: changed := true; UI_ShipLibrary(back page) (4.1.2); redraw the menu image; FadeIn
            cursor 0: done := true; changed := true
    until changed
    for k = 63 down to 32: SetDac(107 + shown, k, 0, 0); wait for retrace start
until done
if not g_MissionAborted:
    FadeOut (mode 0)
    g_PaletteMode := saved_mode
    restore g_FlightPalette[107..110]
    split line := 13Fh; show the OTHER page at the current scroll
        (start = base of page (1 - g_DrawPage) + Round(viewSubX / 4) - 4 + (viewSubY - 16) x 88;
         0E8B:7E91 for g_DrawPage = 0, 0E8B:7ED1 for g_DrawPage = 1)
    FadeIn (restored palette mode)
Esc byte := 0; font background := saved_bg
```

- The screen shows the 352×232 back page from its origin with the display end still at 192 rows, so the visible area is 320×192: the menu at (80..239, 50..149) on black. The HUD is not visible while the menu is open (split line off). C-0012 frame 3844 matches the `td.men` image at (80, 50) with 0 differing pixels and DAC 107..110 = (32,0,0), (63,0,0), (32,0,0), (32,0,0) with the cursor on 1.
- **Restoring the view:** the menu overwrote only the back page and VGA rows 0..31 were never touched, so showing the other page (the frame shown before Esc) with the split line back at row 160 restores the playfield and the HUD exactly. The back page is redrawn completely by the next tick. Palette mode 1 or 2 is restored through `FadeIn`, which uses the mode-1 fade formula (§4.6.3), not the toggle formula (OB-029).
- **Abort:** with Quit the routine returns without fading back in: the screen stays black; the tick then leaves the loop (tick-order step 35) and the flight's teardown runs. `g_MissionAborted` is read by the campaign ([episode-program-flow](episode-program-flow.md) §4.3).
- **Keys:** Up and Down are read from the flight key bytes, so a held key repeats every poll (about 7 video frames, `Crt.Delay(100)`) plus the 2 × 32-retrace highlight fades. The code clears the wrong bytes after a move (30h, 32h instead of 48h, 50h; OB-030), so the keypad 8/2 keys (no E0 prefix) keep repeating while held; the grey arrow keys are cleared and move once per make code. The Esc byte that opened the menu was cleared on entry; a second Esc press closes the menu like "return to game" (it sets cursor 1, which matters only if a select key is down in the same poll: then the ship library opens first).
- **Joystick:** up/down as the arrows; either button selects.

#### 4.1.2 Ship library (`UI_ShipLibrary` `0E8B:7374`)

```text
FadeOut; clear the back page
for r = 0..8: read td.shp record r into sprite slot 0 and draw it at the position of 3.2
build the hatch mask in sprite slot 0; draw it over sprites 6 and 5, and over 8 when flight < 4
draw the text lines of 3.2 (ship_library, opaque, background 0)
re-read td.shp record 0 into sprite slot 0 (restores the bank)
FadeIn (mode 0)
wait until any key byte 1..600 is 1; clear key bytes 1..600
FadeOut
```

The screen is shown from the back-page origin (set by the menu), so screen = page coordinates. C-0012 frame 4184: all seven drawn sprites match `td.shp` records at the 3.2 positions with 0 differing pixels; 561 colour-1 hatch pixels over sprite 6.

#### 4.1.3 Help in flight (`UI_FlightHelp` `0E8B:6EDF`)

```text
clear the back page; save g_FlightPalette (all 768 bytes)
open td.hlp; read 768 bytes into g_FlightPalette
page 1: 200 rows x 320 bytes to back-page rows 0..199 (only rows 0..191 are on screen)
show the back page from its origin; FadeIn (mode 0)
wait until any key byte is 1; clear key bytes 1..600; FadeOut
page 2: the next 64,000 bytes, same way; FadeIn; wait; clear key bytes; FadeOut
restore g_FlightPalette; close td.hlp
```

The bottom 8 rows of each help page are not visible in flight (192-row display; the front-end help `0000:02AF` shows all 200). C-0012 frames 4824 and 5124 show pages 1 and 2. The select key that opened help may still be down when page 1 appears: the wait ends at once if it is (no debounce).

### 4.2 F6 transmission screen

Request and phase logic: [mission-logic](../sim/mission-logic.md) §4.6 (`UI_CommViewer` `0E8B:80D3` calls `UI_CommScreen(n)` `0E8B:5435` only for the listed (flight, phase) pairs, then sets the phase and clears comm pending). The screen itself:

```text
(called at tick-order step 15; the back page is page 0, and this tick's tiles, ships and explosions
 are already in it)
for cell = 0..31: read td.cmm cell (1,024 bytes) into sprite slot 0;
    draw it transparent at screen (32 + 32 (cell mod 8), 16 + 32 (cell div 8))
show page 0 at the current scroll (the HUD band stays)
load transmission n (3.3)
draw td1.ccm record A at screen (48, 32) and record B at (48, 96), transparent;
    reload td.shp record 0 into sprite slot 0 after each
draw header A at pen (92, 35) and header B at (92, 87): opaque, ink 45, background = font background (178)
k := 1; finished := false
repeat
    s := line k; side := speaker byte k
    if s is neither marker:
        type s (Txt_FlightTypeString: opaque, block cursor, one character per retrace) in ink 40
            at (92, 67) for side 1, else (92, 119)
        scroll that box up 6 times by one row (no delay between them)
        k := k + 1
    if s = prompt marker:
        type s in ink 43 at the side's line position (not scrolled; the next line typed there covers it)
        wait until the Space key byte is 1 (busy loop on the key byte); Space byte := 0; k := k + 1
    if s = end marker:
        type s in ink 43 in box A and then in box B
        wait until the F6 or the Esc key byte is 1; clear both bytes; finished := true
until finished
```

- **Scroll box:** a latched copy of 30 rows × 180 pixels one row up (box A rows 43..72 to 42..71, box B rows 95..124 to 94..123, screen x 92..271), then the bottom row (72 or 124) is filled with the font background. Six scrolls move a typed line from rows 68..72 up by 6 pixels, so box A shows five lines.
- **Text background:** the opaque glyph cells and the scroll fill use font background byte 650 = 178 (`td.pal` entry 178), not the panel's colour. Ink 40, 43 and 45 are `td.pal` entries, transformed by the current palette mode (U-224).
- **No ticks run** while the screen is open; music continues (interrupt-driven). After the return, steps 16..35 of the same tick run and step 22 shows page 0 again, so the panel stays visible for one more frame; the next tick draws page 1 in full and the panel is gone.
- The Space wait does not read the joystick buttons: a joystick player needs the keyboard (OB-031).

### 4.3 F5 tactical map (`UI_TacticalMap` `0E8B:A714`, tick-order step 29)

Called when the F5 key byte is 1 and the flight is not 14 (also after the mission is over). It changes no game state.

```text
DS:16E7 := 1; DS:16E8 := 1
restore the BIOS INT 09h handler (the flight key bytes stop changing); clear key bytes 1..600
F5 byte := 0; saved_bg := font background; font background := 0
CRTC 12h := 8Fh (400 scan lines = 200 rows); clear page 0 and page 1; split line := 400
display start := 1600h (VGA row 64 at screen row 0)
for y = 1..200, x = 1..200: pixel (x, y - 1) := k_TileRadarColour[map(x, y)]      (hud.md 3.4)
draw map_prompts[0] at pen (205, 165) and map_prompts[1] at (205, 185), opaque, ink 92
sel := 0
entry 0 := (viewX + 5, viewY + 3), flag 1
for s = 1..20: entry s := (slot tile X, slot tile Y); flag := (class != FFh and marker != 0)
repeat
    pixel (viewX + 5, viewY + 2) := 46h                        (player)
    for s = 1..20 with class != FFh and marker != 0: pixel (tileX(s), tileY(s) - 1) := class(s) + 4Eh
    if the last key was Tab:
        restore the 8 cross pixels of the old selection from the map colours (cross shape below)
        sel := the lowest e > sel (e <= 30) with flag 1; if none, the lowest e >= 0 with flag 1
    if sel = 0 or flag[sel] = 1:
        draw the cross in colour 5Ch around entry sel
        draw its five lines opaque in ink 92 at pen x 205, y = 15, 25, 35, 105, 115
    wait for a key (BIOS); key := ReadKey; if 0, ext := ReadKey
until key = Esc or ext = 3Fh (F5)
F5 byte := 0; clear page 0; split line := 13Fh; CRTC 12h := 7Fh; font background := saved_bg
re-install the flight INT 09h handler (0E8B:0653)
```

- **Cross** around entry (X, Y) (screen coordinates, the entry's pixel at (X, Y − 1)): (X − 3, Y − 1), (X − 2, Y − 1), (X + 2, Y − 1), (X + 3, Y − 1), (X, Y − 4), (X, Y − 3), (X, Y + 1), (X, Y + 2): a plus with a 3-pixel hole. Ship dots under an erased arm are not redrawn until the next key.
- Positions are snapshots taken on entry; nothing moves while the map is open.
- **Return:** the display start stays at VGA row 64 until the next tick's page flip, so for the rest of the tick the playfield shows black (page 0 was cleared); the HUD band is intact. The next tick redraws everything.
- **Keys:** read through the BIOS (`Crt.ReadKey`), so the typematic repeat of a still-held F5 can close the map at once (about 0.5 s after the press). The extended-key byte DS:16E8 is only updated by extended keys, so it keeps 3Fh-free values from earlier keys; any other key (not Tab, not Esc, not F5) just redraws.
- C-0012 frames 6024 and 6104: the 200×200 map matches the radar colours of `td.map` with 11 differing pixels (player dot, two ship dots, cross); info lines at screen rows 16..20, 26..30, 36..40, 106..110, 116..120, prompts at 166..170 and 186..190, ink 92; the entries in RAM (C-0012 frame 6004) equal the flight-1 row of 3.4.

### 4.4 Pause (tick-order step 23, inline `0E8B:E215..E4FD`)

Right after the page flip (step 22), if the P key byte (19h) is 1:

```text
on the page just shown (g_DrawPage, not yet toggled), at text y = 232 x page + 32 + 60 + viewSubY:
    draw flight.prompts[1] in ink 0 at x offsets R + 81, R + 79 (y + 0), R + 81, R + 79 (y + 1),
        R + 81, R + 79 (y - 1), R + 80 (y + 1), R + 80 (y - 1)
    then in ink 37 at x = R + 80, y + 0                      -> screen pen (96, 76), black outline
clear key bytes 1..600
repeat: scan key bytes 1..600; if any is 1: latch := 1  until latch = 1
latch := 0; P byte := 0
```

- The key that ends the pause stays down in the key table, so it acts in the same tick: Esc opens the computer menu (step 25), Space fires, F6 requests a comm, and so on.
- Ticks stop; `g_TickPrevTime` keeps its old value, so the first tick after a pause does not wait.
- C-0012 frame 5724: ink-37 pixels at x 102..242, rows 77..81 (the string starts with one space); DAC 37 = `td.pal` 37 in mode 0.

### 4.5 Infra-red palette modes (`g_PaletteMode` DS:1704)

#### 4.5.1 Formulas

`P[i] = (R, G, B)` is `g_FlightPalette[i]` (never changed by the modes); `Round` is Real48 `Round` of an exact quotient (halves up); `>>` truncates.

| Where | Mode | DAC entry i (i = 0..255 unless stated) |
|---|---|---|
| Flight start `0E8B:CAD2..CBAA` (after the first `FadeIn`), and Alt from mode 2 (`0E8B:B2B8..B390`) | 1 | (Round(R / 8), Round(G / 8), Round(B / 4)); then entry 37 := P[255] |
| Alt from mode 1 (`0E8B:B222..B291`) | 2 | (min(63, Round(R × 1.5)), 0, 0); entry 37 not special |
| `FadeIn` / `FadeOut` target (`0E8B:01F6`, `0E8B:0426`) | 1 | (Round(R / 8), Round(G / 8), Round(B / 8)) — B / 8, entry 37 not special |
| `FadeIn` / `FadeOut` target | 2 | (min(63, Round(R × 1.5)), 0, 0) |
| Hit flash ended (step 14) | 1 | (R >> 3, G >> 3, B >> 3) |
| Hit flash ended (step 14) | 2 | (min(63, (3R) >> 1), 0, 0) |
| White flash, last frame (step 14) | 1 | as flight start (B / 4, entry 37 := P[255]) |
| White flash, last frame (step 14) | 2 | nothing (OB-032) |

So the exact IR picture depends on how mode 1 or 2 was entered last (OB-029). Alt toggles only between 1 and 2 (a flight that starts in mode 0 never enters IR; E1 flight 8 starts in mode 1, [mission-setup](../sim/mission-setup.md)); it is ignored when the mission is over, and it zeroes `g_HitFlash`.

Entry 37 is the ink of the IR notice and of the pause line; in mode 1 it keeps the **normal** colour of entry 255, so the text stays readable.

#### 4.5.2 IR notice (tick-order step 21)

After the tick wait, if `g_PaletteMode = 1` and `g_MissionTicks` (u32) ≤ 200: draw `flight.prompts[0]` transparent in ink 37 at text (R + 80, 232 × page + 32 + 60 + viewSubY) of the back page, i.e. screen pen (96, 76), just before that page is shown. Ticks 1..200 show it (about 10.4 s at 19.2 ticks/s); mode 2 hides it.

#### 4.5.3 Flight fades

`FadeIn`: build the target palette from `g_FlightPalette` for the current mode (table above; mode 0 = P itself) for entries 1..255; then for s = 0..16: wait until the VGA status bit 3 is set (returns at once inside a retrace), set DAC 1..255 to (Round(r·s/16), Round(g·s/16), Round(b·s/16)) of the target, and zero `g_HitFlash`. `FadeOut` is the reverse (s = 16..0) over entries 0..255. DS:1829 is cleared on every step.

#### 4.5.4 Palette effects per tick (tick-order step 14, `0E8B:DA6B..E019`)

```text
if g_HitFlash > 0 and mode = 0:
    g_HitFlash -= 2; if g_HitFlash > 30 (u16 compare, so also after going below 0): g_HitFlash := 0
        (a flash above 32 ends at once; the starters never set more than 20)
    if g_HitFlash = 0: DAC i := P[i] for i = 0..255
    else for i = 0..45 except 36 and 38: DAC i := (min(63, R + f), min(63, G + f), B)   (f = g_HitFlash)
if g_HitFlash > 0 and mode = 1:
    same with -= 3; zero: DAC i := (R >> 3, G >> 3, B >> 3) for all i
    else for i = 0..45 except 36, 38: (min(63, (R >> 3) + f), min(63, (G >> 3) + f), B >> 3)
if g_HitFlash > 0 and mode = 2:
    same with -= 2; zero: DAC i := (min(63, (3R) >> 1), 0, 0) for all i
    else for i = 0..45 except 36, 38: (min(63, ((3R) >> 1) + f), 0, 0)
if g_FlashFrames > 0: g_HitFlash := 0
g_FlashFrames = 4: DAC i := (63, 63, 63)          for all i
g_FlashFrames = 3: DAC i := (63, 63, B)
g_FlashFrames = 2: DAC i := (63, B, B)            (B of P[i] in both places)
g_FlashFrames = 1: mode 0: DAC i := P[i]; mode 1: as flight start; mode 2: nothing
if g_FlashFrames > 0: g_FlashFrames -= 1
```

The exclusion list in the code also names entries 85h, 86h, C1h and 90h, which the "i ≤ 45" test already excludes (U-225).

### 4.6 Afterburner flame (flight-model §4.7, `0E8B:CD78..D160`)

When a burst frame f (1..5) is drawn, sprite bank slot 9 + f (`td.shp` 10..14) is drawn with the blit for the burst heading (the same transform as the player sprite for that heading, [structures](../../re/structures.md) §7) at:

| Heading | Sprite x | Sprite y | Screen top-left |
|---:|---|---|---|
| 8 | R + 128 | viewSubY + 116 | (144, 100) |
| 2 | R + 128 | viewSubY + 52 | (144, 36) |
| 6 | R + 96 | viewSubY + 84 | (112, 68) |
| 4 | R + 160 | viewSubY + 84 | (176, 68) |
| 7 | R + 152 | viewSubY + 108 | (168, 92) |
| 1 | R + 152 | viewSubY + 60 | (168, 44) |
| 9 | R + 104 | viewSubY + 108 | (120, 92) |
| 3 | R + 104 | viewSubY + 60 | (120, 44) |

The player sprite itself is at screen (144, 68); the flame sits one sprite behind the ship along the burst heading.

### 4.7 Explosions

**Slot choice.** Every explosion started by `Sim_ProjectilesAndHits` (`0E8B:0984`) takes the **lowest idle slot among 1..20** (frame = 8). If none is idle, the explosion is not started and **its `Random` calls are not made** (the calls sit inside the slot search; see [weapons](../sim/weapons.md) rev 3). Slots 21..30 are used only by the other starters (`2404:1426`, `0E8B:56ED`). A new explosion starts at **frame 1**.

| Event | Count | Tile | Offset (x, y) |
|---|---:|---|---|
| Ship destroyed (step 4) and kind-6 kill (step 3) | 5 | (X + 1, Y + 1), (X, Y), (X + 1, Y − 1), (X − 1, Y + 1), (X − 1, Y − 1), in this order, each in its own search | (16, 16) |
| Ship hit, not destroyed | 1 | ship tile | ship offset + `Random(16)` on X, then `Random(16)` on Y (stored as bytes) |
| Player hit | 1 | (viewX + 4, viewY + 3) | (viewSubX − 8 + `Random(8)`, viewSubY − 8 + `Random(8)`), X first |

X, Y = the destroyed ship's tile. Offsets are u8.

**Drawing (tick-order step 13, `0E8B:D872..DA66`).** For e = 1..30 with frame < 8: if viewX ≤ tileX ≤ viewX + 9, viewY ≤ tileY ≤ viewY + 5, not (tileX = viewX + 9 and offX ≥ viewSubX), and (page = 1 or (tileY − viewY) × 32 + offY > 32): draw sprite bank slot 19 + frame (frames 1..6 = `td.exp` 0..5; frame 0 would be `td.shp` 19) with the plain blit at sprite (x, y) = ((tileX − viewX) × 32 + offX, (tileY − viewY) × 32 + offY + 1) (the code passes x + 352, which adds one row). Then frame += 1, and 7 becomes 8. So an explosion shows six frames; `td.exp` frame 6 is never used. The page-dependent top test makes an explosion in the top tile row visible on page 1 frames only (OB-033).

### 4.8 Shots

Visibility and drawing per projectile slot, in step 6 after the hit tests (`0E8B:1785..3F9C`). Let X = (tileX − viewX) × 32 + offX and Y = (tileY − viewY) × 32 + offY.

**Kinds 0..2** (pixels, `Gfx_PutPixel` into the back page at column X + 16 + dx, VGA row 232 × page + Y + 16 + dy, i.e. screen (X + 32 − R + dx, Y − viewSubY + dy); the code passes x + 352, which adds the one row): drawn when the world pixel (tile × 32 + offset + 16) is within the drawn window on both axes, X + 16 ≥ 0, Y + 16 ≥ 34, X + 16 ≤ 352 and Y + 1 ≤ 261. Heading groups: 8/2 vertical, 4/6 horizontal, 9/1 and 7/3 diagonals.

| Kind | Heading | Pixels (dx, dy) colour |
|---:|---|---|
| 0 | 8, 2 | (0,−1) 3Ch, (0,0) 3Dh, (0,1) 3Ch |
| 0 | 4, 6 | (1,0) 3Ch, (0,0) 3Dh, (−1,0) 3Ch |
| 0 | 9, 1 | (1,−1) 3Ch, (0,0) 3Dh, (−1,1) 3Ch |
| 0 | 7, 3 | (1,1) 3Ch, (0,0) 3Dh, (−1,−1) 3Ch |
| 1 | 8, 2 | (0,−1) 32h, (0,0) 33h, (0,1) 33h, (0,2) 32h; (1,−1) 32h, (1,0) 33h, (1,1) 33h, (1,2) 33h |
| 1 | 4, 6 | (1,0) 32h, (0,0) 33h, (−1,0) 33h, (−2,0) 32h; (1,1) 32h, (0,1) 33h, (−1,1) 33h, (−2,1) 33h |
| 1 | 9, 1 | (1,−1) 32h, (0,0) 33h, (−1,1) 33h, (−2,2) 32h; (2,−1) 32h, (1,0) 33h, (0,1) 33h, (−1,2) 32h |
| 1 | 7, 3 | (1,1) 32h, (0,0) 33h, (−1,−1) 33h, (−2,−2) 32h; (2,1) 32h, (1,0) 33h, (0,−1) 33h, (−1,−2) 32h |
| 2 | 8, 2 | for dx = −1..2: (dx,−2) BEh, (dx,−1) 35h, (dx,0) 35h, (dx,1) 35h, (dx,2) BEh |
| 2 | 4, 6 | for dy = −1..2: (2,dy) BEh, (1,dy) 35h, (0,dy) 35h, (−1,dy) 35h, (−2,dy) BEh |
| 2 | 9, 1 | for k = 0..3: (2,k−3) BEh, (1,k−2) 35h, (0,k−1) 35h, (−1,k) 35h, (−2,k+1) BEh |
| 2 | 7, 3 | for k = 0..3: (2,k+1) BEh, (1,k) 35h, (0,k−1) 35h, (−1,k−2) 35h, (−2,k−3) BEh |

Kind 3 is never drawn (no class fires it). Shots on a two-digit heading (U-160) are not drawn.

**Kinds 4..6** (sprites): drawn when viewX ≤ tileX ≤ viewX + 8 and viewY + 2 ≤ tileY ≤ viewY + 5, with the heading's blit (8 plain, 2 `386E:0C02`, 4 `0926`, 6 `07BC`, 7 `0391`, 9 `04E9`, 1 `0A93`, 3 `0650`) at sprite (x, y) = (X, Y). Sprite slot: kind 4 → 16, kind 5 → 15, kind 6 → 9 (`td.shp` 16, 15, 9). Kind 6 is drawn after its heading update (weapons §4.3 step 6).

## 5. Edge cases and original bugs

| Quirk | Original behaviour | Port policy (proposal) |
|---|---|---|
| Menu clears the wrong key bytes (OB-030) | Keypad 8/2 repeat while held | Preserve (Classic) |
| IR palette differs by path (OB-029) | Mode 1 after a menu, help or comm fade uses B/8 and no entry-37 fix; after a hit flash ends, truncating formulas | Preserve |
| White flash in mode 2 (OB-032) | Last frame restores nothing: the screen stays (63, B, B) until the next hit flash or Alt | Preserve (Classic), fix-enhanced |
| Explosions at the top edge (OB-033) | Skipped on page 0 frames only: flicker at the top of the playfield | Preserve |
| Comm Space wait ignores the joystick (OB-031) | Joystick players must press Space | Preserve; Enhanced may accept a button |
| Speaker bytes not reset between transmissions | n = 2 lines 16, 17 use the speaker bytes left by an earlier transmission (or 0 = box B at start); they are blank lines | Preserve (blank lines; no visible effect) |
| F5 info lines not rewritten | Entries a flight does not write keep an earlier flight's lines; entry 0 in flight 21 shows flight 19's lines | Preserve |
| Menu Esc + select in one poll | Opens the ship library | Preserve |
| Pause resume key | Acts in the same tick (Esc opens the menu) | Preserve |

## 6. Test vectors

| # | Setup | Expected | Evidence |
|---|---|---|---|
| 1 | E1 flight 1, Esc, Down | `td.men` 160×100 at (80, 50), 0 differing pixels; DAC 107..110 = (32,0,0), (63,0,0), (32,0,0), (32,0,0); everything else black | C-0012 f3844 |
| 2 | Enter on "ship library" (flight 1) | sprites 0, 1, 2, 3, 4, 7 at 3.2 positions, 0 differing pixels; sprites 5, 6, 8 hatched; 561 colour-1 pixels over sprite 6 | C-0012 f4184 |
| 3 | Help, two pages | `td.hlp` pages 1 and 2, top 192 rows | C-0012 f4824, f5124 |
| 4 | P in flight 1 | ink 37 on rows 77..81, x 102..242, black outline | C-0012 f5724 |
| 5 | F5 in flight 1 | map pixels = radar colours except 11; text rows 16, 26, 36, 106, 116, 166, 186 (+0..4), ink 92; entry 0 lines 2 3 4 1 1, entries 1, 2 flag 1, entry 21 at (107, 104) | C-0012 f6024, RAM f6004 |
| 6 | Flight 1 RAM | font background byte 650 = 178 | C-0012 RAM f6004 (DS:329A) |
| 7 | Palette mode 1 at flight start, entry with P = (40, 20, 30) | DAC (5, 3, 8) (Round(40/8) = 5, Round(2.5) = 3, Round(7.5) = 8) | static |
| 8 | Mode 2 by Alt, R = 41 | DAC R = Round(61.5) = 62 | static |

### 6.1 Transmission screen and infra-red (C-0013, C-0014)

| # | Setup | Expected | Evidence |
|---|---|---|---|
| 9 | E1 flight 2, phase 1, F6 (transmission 1) | `td.cmm` panel cells at (32 + 32c, 16 + 32r): 0 of 12,113 non-zero panel pixels differ; `td1.ccm` records 1 and 0 at (48, 32) and (48, 96): 0 differences | C-0013 f584, f1184, f2384 |
| 10 | Same | header ink rows 36..40 and 88..92; typed line A rows 68..72, scrolled lines at 6-row steps above (42..54 ...); line B rows 120..124; box A background pixels colour 178 = `td.pal` 178 (3, 15, 3); DAC 40, 43, 45 = `td.pal` | C-0013 |
| 11 | Same, after both prompts | end marker in both boxes, ink 43; F6 closes; phase 2 | C-0013 (last panel frame, then flight) |
| 12 | E1 flight 8 start (mode 1) | DAC = (Round(R/8), Round(G/8), Round(B/4)) of `g_FlightPalette` for all entries but 37; DAC 37 = P[255]; the only other differences are entries the flight itself patches (36, 38, 46..49, ...) | C-0014 f522..f1222 |
| 13 | Same, ticks ≤ 200 | IR notice ink-37 pixels at rows 77..81, x 106..232 | C-0014 f522..f1222 |
| 14 | Alt (mode 2), then Alt (mode 1) | mode 2: (min(63, Round(1.5 R)), 0, 0); mode 1 again: as #12 | C-0014 f1322, f1422 |
| 15 | P in mode 1 | pause ink 37 rows 77..81, x 102..242, DAC 37 = P[255] | C-0014 f1522 |
| 16 | Esc menu in mode 1, then "return to game" | menu in mode 0 (normal palette); after the return DAC = (Round(R/8), Round(G/8), Round(B/8)), DAC 37 = (8, 6, 8) (not P[255]): OB-029 | C-0014 f1822, f1982 |

## 7. Presentation and audio hooks

No sounds are started by these screens. Music keeps playing (WORX is interrupt-driven) during all of them. Typing in the comm screen paces itself on vertical retrace like the front-end typing ([render/text](../render/text.md) §4.3).

## 8. Open questions

- U-221: transmissions 2..9 were checked statically only (C-0013 shows transmission 1); capture one more (for example flight 8, transmission 4, whose strings were mis-mapped before).
- U-222: afterburner flame, explosions and shots (§4.6–4.8) are static only; capture a class-2 flight with a burst and a kill (VE).
- U-220: F5 map. What does the typematic repeat of F5 do on real hardware (the BIOS reads it while the map is open)? Port: ignore key repeat for closing.
- U-223: what DS:16E6 (zeroed by the computer menu) is used for.
- U-224: are ink 40/43/45 and the font background (178) in the comm screen meant to follow the IR palette (they do in the code)?
- U-225: the code's extra exclusions 85h, 86h, C1h, 90h in the hit-flash loop are dead; was the intended range wider (the HUD lamp entries)?
- U-226: which flight-21 entries the F5 map shows (nothing is loaded for flight 21).

## 9. Provenance

| Claim | Address / capture | Method | Confidence |
|---|---|---|---|
| Computer menu | `file0001:0E8B:794C..7F2E` | static; C-0012 | CONFIRMED (layout, colours) / LIKELY (keys) |
| Ship library, help | `0E8B:7374..78FF`, `0E8B:6EDF..712x` | static; C-0012 | CONFIRMED (layout) |
| Comm screen | `0E8B:5435..56EA`, `0E8B:0061`, `012A`, `5032..51FD`, `51FE`, `530B`; loaders `2A15:01EC..2216` | static; C-0013 | CONFIRMED (layout, colours, transmission 1) / LIKELY (other transmissions) |
| F5 map | `0E8B:A714..AED0`; `1EDE:0E6A` (info lines; bytes `1EDE:0ED8..1419` read directly where the disassembly stops) | static; C-0012 (pixels and RAM) | CONFIRMED |
| Pause, IR notice | `0E8B:E12F..E4FD` | static; C-0012, C-0014 | CONFIRMED (pause) |
| Palette modes | `0E8B:01F6`, `0426`, `B1EC..B39A`, `CAD2..CBAA`, `DA6B..E019` | static; C-0014 | CONFIRMED (start, toggle, menu fade) / LIKELY (flash formulas) |
| Afterburner flame | `0E8B:CD78..D160` | static | LIKELY |
| Explosions | `0E8B:0D31..0F42`, `12xx..14D4`, `14E4..1580`, `16F9..177B`, `D872..DA66` | static | LIKELY |
| Shots | `0E8B:1785..3F9C` (bytes `1948..197F` read directly) | static | LIKELY |

## 10. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-02 | RE-8 (Archaeologist + Spec author) | Initial |
