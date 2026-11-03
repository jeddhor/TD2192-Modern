# In-flight display and HUD (draft)

| | |
|---|---|
| **Spec ID** | `render/hud` |
| **Revision** | 1 (draft) |
| **Status** | Draft |
| **Fidelity tier** | T0 for geometry and element positions; T1 for update order |
| **Confidence** | CONFIRMED for the display geometry and the HUD bar position (static + oracle); LIKELY for the element positions listed in §3.3 (static only); gauges and counters not yet analysed |
| **Tasks** | M2-09, M6-07 |
| **Unknowns** | U-025 (resolved), U-033 |

## 1. Summary

In flight the original shows a **320×192** picture: a scrolling **320×160 playfield** on top and the **320×32 HUD bar** (`td.win`) below it. It does this with VGA hardware alone. The CRTC's vertical display end is cut to 192 lines, and the line-compare (split-screen) register makes the bottom 32 lines always show video-memory rows 0..31, where `td.win` was plotted once at mission start. The playfield is drawn into one of two 352×232 off-screen pages, and the visible window is chosen with the display start address, so scrolling costs nothing per pixel. HUD elements (radar, weapon icon, transmission indicator, message ticker) are drawn straight into VGA rows 0..31 and therefore appear in the bar at once, independent of the page flip.

## 2. Definitions and units

| Term / quantity | Type in original | Unit | Range | Notes |
|---|---|---|---|---|
| VGA row | — | 88 bytes = 352 px | 0..743 | Unchained mode 13h, CRTC offset 44 ([asset-load-sites.md §1.1](../../re/asset-load-sites.md#11-vga-memory-layout-unchained-mode-13h-352-pixel-rows)) |
| HUD coordinates (hx, hy) | — | px | 0..319, 0..31 | VGA rows 0..31; on screen at (hx, 160 + hy) |
| Page | Byte DS:3A30 | — | 0, 1 | Page 0 at VGA 0B00h (row 32), page 1 at 5AC0h (row 264); each 352 × 232 |
| Scroll x, y | Word DS:3A3A, DS:3A3C | px | start 32, 16 | Fine scroll inside the page |
| View tile | Word DS:3A36, DS:3A38 | tile | start 102, 104 | Map tile of the view's top-left corner (agrees with the oracle frame match on main) |

## 3. Data

### 3.1 Display set-up (`Sim_FlyMission`, `file0001:0E8B:B880`)

| Register / value | Effect | Provenance |
|---|---|---|
| CRTC 12h := 7Fh (vertical display end 17Fh = 383) | 384 scan lines = **192 rows** shown | `0E8B:CC40` |
| Line compare := 13Fh (319) | Scan lines 0..319 (rows 0..159) show the page; from row 160 the display restarts at VGA address 0 | `Gfx_SetSplitLine` calls in `0E8B:B880` |
| CRTC 13h = 2Ch | 88-byte rows | `Gfx_InitUnchained` |
| Start address | page base + (scroll y − 16) × 88 + Round(scroll x / 4) − 4 | `0E8B:7E91`, `0E8B:7ED1` (restoring the view after the computer menu) |
| No attribute-controller writes | horizontal scroll moves in 4-pixel steps (no pixel panning) | port I/O scan (`FindPortIO`) |

Menus and scenes set the split line to 400 (off) and the display end back to normal.

### 3.2 `td.win` (HUD bar)

320×32 bytes, read one byte at a time and plotted to VGA (x, y) = (column, row), rows 0..31, before the first frame. The palette is `td.pal`. The bar is never redrawn as a whole during the mission; elements overwrite parts of it.

### 3.3 HUD elements (HUD coordinates)

| Element | Area (hx, hy, w × h) | Source | Update | Provenance | Conf. |
|---|---|---|---|---|---|
| Radar | (148, 6), 20 × 20 | One pixel per map tile: colour = `k_TileRadarColour[tile]` (DS:0264) for map tiles x = view x − 6 .. + 13, y = view y − 6 .. + 13 (drawn via a 28-wide scratch buffer), then ship markers | per tick (`0E8B:8E87` → `386E:0000`) | static | LIKELY |
| Weapon icon | (179, 7), 36 × 18 | `td.wea` icon *i*; *i* from the ship class byte DS:1A57: 2, 3, 8 → 1; 4, 5, 7 → 2; 6 → 3 (0, 4, 5 drawn elsewhere or unused) | at mission start and when the ship changes | `0E8B:B880` after `td.win` | LIKELY |
| Transmission indicator ("HIT F6") | (220, 2), 28 × 28 | `td.fac` frame *f* (784 bytes each; plotted as 4 planes × 7 bytes × 28 rows) | While a transmission is pending: a 7-step cycle showing frames 1, 2, 3, 4, 3, 2, 1, one step per call; frame 0 (blank) when it ends | `0E8B:7FDF` → `386E:012B` | LIKELY |
| Message ticker | (256, 13), 56 × 5 | Objective text pre-drawn once with the small font (colour 37) at off-screen VGA row 735 (glyph rows 736..740); a 56-px window of it is latch-copied into the bar | Window offset steps 4 px at a time from 4 to 312 px, on page-0 ticks only (every other tick), then the ticker stops | `0E8B:60D8`, `0E8B:7FDF` → `0E8B:0000` | LIKELY |
| Shield and hull bars, missile count, status boxes | inside the `td.win` artwork | — | — | not analysed | — |

## 4. Behaviour

### 4.1 Mission start (order)

1. `Gfx_InitUnchained`; load the data files ([asset-load-sites.md §5](../../re/asset-load-sites.md#5-flight-sim_flymission-0e8bb880-once-per-mission)), tiles into VGA rows 496..729.
2. Set all DAC entries to black; plot `td.win` into rows 0..31; show page 1 (start address 5AC0h) with the split at 13Fh; set the display end to 192 rows (`0E8B:CC40`).
3. Per-mission set-up (`0E8B:60D8`): objective text into row 735, mission-specific ship state.
4. Draw the weapon icon; enter the tick loop (U-033).

### 4.2 Each tick (known parts)

- The playfield is drawn into the hidden page, then shown with `Gfx_FlipPage` (start address, then wait for retrace) and the page index toggles.
- `0E8B:7FDF` (from `0E8B:8E87`) advances the message ticker (page-0 ticks only) and the transmission indicator.
- The radar is rebuilt from the map around the view (`0E8B:8E87`).

## 5. Edge cases and original bugs

- The split-screen trick means the HUD is unaffected by page flips and scrolling. A port that composes a 320×192 frame must draw the HUD after the playfield every frame.
- The ticker only advances on page-0 ticks, so it runs at half the tick rate.

## 6. Test vectors

| # | Setup / input | Expected output | Evidence |
|---|---|---|---|
| 1 | E1 mission 1, first flight frame | Display 320×192; rows 160..191 equal `td.win` except where elements are drawn (85 % of pixels match on main) | C-0002 f3473.., main session |
| 2 | Same | The oracle shows tile (103, 104) starting at screen x 16. Consistent with a page whose first column is view tile x 102 and a start-address offset of Round(32 / 4) − 4 = 4 bytes = 16 px (start values DS:3A36 = 102, DS:3A3A = 32). To be confirmed with a RAM dump | C-0002 f3500 (main session), static start values |

## 7. Presentation and audio hooks

The transmission indicator pairs with the incoming-transmission sound (`income.voc`, M1-02).

## 8. Open questions

- [ ] Gauges, missile counter and status boxes: routines and positions (M6-07).
- [ ] Radar ship markers and their colours.
- [ ] What starts and stops the transmission indicator (DS:329C) and when the ticker is restarted (DS:32BC, DS:1E2E).
- [ ] U-033: order of work inside the tick.

## 9. Provenance

| Claim | Address / capture | Method | Confidence |
|---|---|---|---|
| 192-row display, split at 13Fh, HUD = VGA rows 0..31 | `file0001:0E8B:CC40`, `0E8B:B880`, `386E:19A0`; C-0002 | static + capture | CONFIRMED |
| Pages and start-address formula | `386E:189C`, `386E:191C`, `0E8B:7E91` | static | LIKELY |
| Radar | `0E8B:8E87`, `386E:0000` | static | LIKELY |
| Weapon icon | `0E8B:B880` | static + file decode | LIKELY |
| Indicator | `0E8B:7FDF`, `386E:012B` | static | LIKELY |
| Ticker | `0E8B:60D8`, `0E8B:7FDF`, `0E8B:0000` | static | LIKELY |

## 10. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-02 | RE-3 | Draft from static analysis (M2-09) |
