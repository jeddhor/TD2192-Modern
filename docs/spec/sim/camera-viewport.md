# Camera and viewport (flight)

| | |
|---|---|
| **Spec ID** | `sim/camera-viewport` |
| **Revision** | 1 |
| **Status** | Draft |
| **Fidelity tier** | T1 for the view state (it is the player's position); T2 for presentation |
| **Confidence** | CONFIRMED for the view-to-world relation (code + C-0002 RAM + the Verifier's tile match on frame 3500); LIKELY for the start-address formula |
| **Tasks** | M6-01 |
| **Unknowns** | U-025 (resolved by [render/hud.md](../render/hud.md)), U-046 |

Builds on [render/hud.md](../render/hud.md) (display set-up, split screen, HUD band) and [asset-load-sites §1.1](../../re/asset-load-sites.md) (VGA memory layout); this spec does not repeat them.

## 1. Summary

The camera is the view state `(viewTileX, viewTileY, viewSubX, viewSubY)` of [flight-model](flight-model.md) §2; it moves only through the player's movement. Each tick the game draws an 11 × 6 window of map tiles into the back page (352 × 232 pixels of unchained VGA memory) and then points the CRTC start address at the part of that page that should be visible, so the 320 × 160 playfield scrolls smoothly in steps of 4 pixels horizontally and 1 pixel vertically. The HUD band below it (rows 160..191) is the split-screen part described in hud.md.

## 2. Definitions

| Quantity | Value |
|---|---|
| Playfield | 320 × 160 pixels (screen rows 0..159) |
| Back page | `g_DrawPage` DS:3A30; page base 0B00h (page 0) or 5AC0h (page 1); 88 bytes per row |
| Tile window | map tiles (viewTileX + i, viewTileY + j), i = 0..10, j = 0..5, tile (i, j) at page pixel (32i, 32j) |
| World pixel | tile × 32 + offset |

## 3. Behaviour

```text
visible world rectangle (top-left, inclusive):
    left = viewTileX × 32 + 4 × round(viewSubX / 4) − 16
    top  = viewTileY × 32 + viewSubY − 16
    size 320 × 160
CRTC start address (byte address, 4 pixels per byte; Gfx_FlipPage 386E:1A6C, tick-order step 22):
    start = page_base(g_DrawPage) + round(viewSubX / 4) − 4 + (viewSubY − 16) × 88
    (round = Real48 Round of viewSubX / 4.0: halves round up, e.g. 18/4 = 4.5 -> 5)
```

- Because `viewSubX` and `viewSubY` stay in 16..48 between ticks, the visible window always lies inside the drawn 352 × 192 tile window (page rows 0..191).
- Horizontal scrolling is quantised to 4 pixels (the player sprite is placed with the same `4 × round(viewSubX / 4)`), vertical scrolling is per pixel.
- The view (and with it the player) is limited to tiles X 6..183 and Y 7..190 (flight-model §4.4), so the window never leaves the 201 × 201 map.
- Start values: (102, 104, 32, 16) in most flights → visible top-left (3280, 3328) (C-0002; Verifier tile match).

The player ship is drawn at page position (128 + 4 × round(viewSubX / 4), viewSubY + 84) as passed to the sprite blitter; the Verifier observed it centred on the playfield, at screen (160, 80). The blitter's vertical origin for sprites is U-046.

## 4. Edge cases

| Quirk | Original | Port policy |
|---|---|---|
| 4-pixel horizontal steps | Visible on slow horizontal drift | Classic: preserve; Enhanced may scroll per pixel (presentation only) |
| Display updates at the tick rate | ≈ 19 frames per second | See timing §5 |

## 5. Test vectors

| # | View state | Visible top-left (world px) | Start address (page 1) | Evidence |
|---|---|---|---|---|
| 1 | (102, 104, 32, 16) | (3280, 3328) | 5AC0h + 8 − 4 + 0 = 5AC4h | C-0002 RAM + Verifier match |
| 2 | (109, 104, 28, 48) | (3500, 3360) | 5AC0h + 7 − 4 + 32 × 88 = 65C3h | C-0008 state at tick 70 (address computed) |

## 6. Open questions

- [ ] Q1 (VE): log the CRTC start address (port 3D4h index 0Ch/0Dh) per frame in a C-0008-like run to confirm §3's formula; confirm the player sprite position (U-046).

## 7. Provenance

| Claim | Address / capture | Method | Confidence |
|---|---|---|---|
| Tile window | `0E8B:CD22..CD80` | static | CONFIRMED |
| Start address | `0E8B:E190..E215` | static | LIKELY |
| Visible rectangle | C-0002 RAM, Verifier frame 3500 | capture | CONFIRMED |
| Player sprite page position | `0E8B:D1A0..D1DD` | static | LIKELY |

## 8. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-02 | RE-2 (Spec author) | Initial |
