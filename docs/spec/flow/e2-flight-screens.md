# Episode 2 and 3 in-flight screens: transmissions and ship library

| | |
|---|---|
| **Spec ID** | `flow/e2-flight-screens` |
| **Revision** | 1 |
| **Status** | Draft |
| **Fidelity tier** | As [flight-screens](flight-screens.md): T0 for the screen images (positions, inks, which strings), T1 for the state the screens change, T2 for typing speed |
| **Confidence** | CONFIRMED (static) for every table below: each was read from our own listing of the loader or library routine by a script that reproduces E1's [flight-screens §3.2 and §3.3](flight-screens.md) tables exactly when run on `file0001`. E2 transmission 4 and the E2 library also CONFIRMED by capture (C-0044, §6.1); the other transmissions and all of E3 are static only |
| **Tasks** | M10-44.b (E2), M10-02 / M10-45 (E3) |
| **Unknowns** | U-310, U-313 (resolved here) |

This is a **delta spec**. Implement [flight-screens](flight-screens.md) §3.2, §3.3, §4.1.2 and §4.2 (Episode 1) and change only what this document says. Addresses are `td2:` (E2, DGROUP `36B6`) or `td3:` (E3, DGROUP `32C5`), image-relative (ADG §7.8). Text is named by resource id ([td2.toml](../../re/exe-resources/td2.toml), [td3.toml](../../re/exe-resources/td3.toml)), never quoted.

## 1. Summary

The E2 and E3 transmission screen (F6) and ship library (Esc menu, item 1) are E1's routines with new data. The comm screen code is instruction-for-instruction E1's except for the dispatch to the comm text routines (E2: seven loaders, E3: four, for screens 1, 2, 3 and 5); each loader fills the same table (two headers, up to 30 lines, a speaker byte per line, two portrait numbers) from its own string run. The ship library draws the same nine `td.shp` records at the same places with E1's pens and inks, but every episode has its own text table (31 strings in E2 and E3) and its own rule for which entries are hatched.

## 2. Definitions and units

As [flight-screens §2](flight-screens.md). Additional terms:

| Term | Meaning |
|---|---|
| Run | The string resource a loader reads, as listed per transmission in §3.1. `j` is the 0-based index into it |
| Line k | Slot k = 1.. of the transmission table; k = −1 is header A, k = 0 is header B |
| `P`, `E` | The prompt marker and the end marker: lines whose content equals `e2.comm.prompts[0]` / `[1]` (E3: `e3.comm.prompts[0]` / `[1]`). Each run also holds its own copies of the two markers, so `P`/`E` below stand for a `j` whose string is a marker |
| Speaker byte | 1 = the line is typed in box A; **any other value** (2, or a stale 0) = box B (the comm screen tests "= 1") |

## 3. Data

### 3.1 E2 transmissions (`td2:2555:*`, dispatched by `UI_CommScreen` `0E96:5401`)

Table layout in DGROUP: header A DS:31EC, header B DS:321F, line k at DS:321F + 51k (`string[50]`), speaker byte k at DS:384B + k, portrait A DS:386A, portrait B DS:386B (`td2.ccm` records). Every loader writes header A := `j` 0 and header B := `j` 1, portrait B := 0, and nothing else outside the slots listed.

| n | Loader | Run | Portrait A / B | Lines k = 1.. as `j` | Speaker of lines |
|---:|---|---|---|---|---|
| 1 | `2555:01D4` | `e2.comms.01` (19) | 1 / 0 | 2 3 P 5 6 7 P 8 9 10 11 12 13 P 8 14 15 P 15 16 17 E | A 1..4, B 5..8, A 9..19, B 20..22 |
| 2 | `2555:0525` | `e2.comms.02` (10) | 2 / 0 | 2 3 4 5 P 7 8 E | A 1..6, B 7..8 (also bytes 9, 10 := B; no line there) |
| 3 | `2555:0774` | `e2.comms.03` (14) | 3 / 0 | 2 3 4 5 6 P 8 9 10 11 12 E | A 1..7, B 8..12 |
| 4 | `2555:0B7E` | `e2.comms.04` (25) | 4 / 0 | 2 3 4 5 P 7 8 9 P 10 11 12 13 14 P 15 16 17 18 19 P 15 20 21 22 23 E | A 1..6, B 7..10, A 11..22, B 23..27 |
| 5 | `2555:0F93` | `e2.comms.05` (15) | 5 / 0 | 2 3 4 P 6 7 8 4 P 9 10 11 4 P 6 12 13 4 P 9 E | A 1..5, B 6..10, A 11..15, B 16..20; the speaker byte of line 21 (E) is not written (§5) |
| 6 | `2555:1325` | `e2.comms.06` (10) followed by `e2.comms.07` (2), one run of 12: `j` 10, 11 are `.07[0]`, `.07[1]` | 6 / 0 | 2 3 4 5 6 7 8 9 10 E | A 1..5, B 6..10 (also byte 11 := B) |
| 7 | `2555:158D` | `e2.comms.08` (15) | **5** / 0 | 2 3 4 5 P 7 8 9 10 5 P 11 12 13 5 E | A 1..6, B 7..12, A 13..16 |

Marker indices: n = 1: P = 4, E = 18; n = 2: P = 6, E = 9; n = 3: P = 7, E = 13; n = 4: P = 6, E = 24; n = 5: P = 5, E = 14; n = 6: E = 11 (no prompt); n = 7: P = 6, E = 14. Every run's last string is its end marker, and every string of every run is used.

Strings that consist only of spaces (one or more) are typed like any other line: a single space is a blank line; a longer all-space string typed right after a prompt (for example n = 1, `j` 5 and 8) overwrites the prompt marker in that box before the box scrolls ([flight-screens §4.2](flight-screens.md)).

Which flight and phase show which n: [episode-deltas §6](../../re/episode-deltas.md#6-hud-and-flight-ui), [sim/e2-mission-logic](../sim/e2-mission-logic.md) §4.5.

### 3.2 E3 transmissions (`td3:2215:*`, dispatched by `UI_CommScreen` `0C5D:540E`)

Table layout: header A DS:31E8, header B DS:321B, line k at DS:321B + 51k, speaker byte k at DS:3847 + k, portrait A DS:3866, portrait B DS:3867 (`td3.ccm` records). Headers `j` 0 and 1, portrait B := 0 as in E2. The screen numbers are those passed to `UI_CommScreen` ([sim/e3-mission-logic §4.5](../sim/e3-mission-logic.md)); there is no screen 4.

| Screen n | Loader | Run | Portrait A / B | Lines k = 1.. as `j` | Speaker of lines |
|---:|---|---|---|---|---|
| 1 | `2215:0135` | `e3.comms.01` (13) | 1 / 0 | 2 P 4 5 6 7 8 P 9 10 11 E | A 1..3, B 4..9, A 10..12 |
| 2 | `2215:042A` | `e3.comms.02` (16) | 2 / 0 | 2 3 4 5 P 7 8 9 10 P 7 11 12 13 14 E | A 1..6, B 7..11, A 12..16 |
| 3 | `2215:0680` | `e3.comms.03` (7) | 3 / 0 | 2 3 4 5 E | A 1..5 |
| 5 | `2215:093D` | `e3.comms.04` (21) | 4 / 0 | 2 3 4 P 6 7 8 9 4 P 10 11 4 P 6 12 13 14 15 P 16 17 18 P 16 19 18 E | A 1..5, B 6..10, A 11..15, B 16..21, A 22..25, B 26..28 |

Marker indices: screen 1: P = 3, E = 12; screen 2: P = 6, E = 15; screen 3: E = 6 (no prompt); screen 5: P = 5, E = 20.

### 3.3 Ship library text tables (`UI_ShipLibrary`: E2 `td2:0E96:7653`, E3 `td3:0C5D:7337`)

Sprites: `td.shp` records 0..8 at the screen positions of [flight-screens §3.2](flight-screens.md) (unchanged in both episodes). Text: `e2.ship_library[i]` / `e3.ship_library[i]` drawn opaque (background 0) at pen (x, y), screen coordinates; the code passes y + 32 + 232 × page as in E1. Inks are `td.pal` entries.

| Sprite | Screen (x, y) | Text pen x | Text lines (y : index, ink) | E2 | E3 |
|---:|---|---:|---|---|---|
| 0 | (4, 12) | 40 | 12 : 0, 96 · 18 : 1, 92 · 24 : 2, 92 · 32 : 3, 94 | always | always |
| 1 | (4, 49) | 40 | 49 : 4, 96 · 55 : 1, 92 · 61 : 5, 92 · 69 : 6, 94 | always | always |
| 7 | (4, 92) | 40 | 92 : 7, 96 · 98 : 8, 92 · 104 : 9, 92 · 112 : 10, 94 | always | always |
| 8 | (4, 123) | 40 | 123 : 11, 96 · 129 : 12, 92 · 135 : 13, 92 · 143 : 14, 94 | always (E1: flights ≥ 4) | always |
| 6 | (4, 152) | 40 | 154 : 15, 96 · 160 : 16, 92 · 166 : 17, 92 · 174 : 3, 94 | text only in flights ≥ 15; hatched in flights < 15 | text, never hatched |
| 2 | (164, 28) | 200 | 28 : 18, 96 · 34 : 12, 92 · 40 : 19, 92 · 48 : 20, 94 | always | always |
| 3 | (164, 71) | 200 | 71 : 21, 96 · 77 : 22, 92 · 83 : 19, 92 · 91 : 23, 94 | always | always |
| 4 | (164, 109) | 200 | 109 : 24, 96 · 115 : 25, 92 · 121 : 19, 92 · 129 : 26, 94 | always | always |
| 5 | (164, 150) | 200 | 147 : 27, 96 · 153 : 28, 92 · 159 : 29, 92 · 167 : 30, 94 | text only in flights ≥ 11; hatched in flights < 11 | text, never hatched |

The text of sprites 6 and 5 starts 2 rows below (sprite 6) or 3 rows above (sprite 5) the sprite's top, unlike the other blocks, whose first line is level with the sprite. The ink-94 line of the sprite-6 block repeats index 3. Index 12 and 19 are shared lines, as in E1.

"Hatched" is E1's mask ([flight-screens §3.2](flight-screens.md): colour 1 where row + column is even, a one-pixel checkerboard over the 32 × 32 sprite), built in sprite slot 0 after the nine sprites. E2 draws it at (4, 152) when the flight number is below 15 and at (164, 150) when it is below 11, before any text. E3 builds no mask and hatches nothing.

## 4. Behaviour

### 4.1 Transmission screen

[flight-screens §4.2](flight-screens.md) unchanged, with the table of §3.1 or §3.2. Verified by comparing the instruction sequences of `file0001:0E8B:5435..56EA`, `td2:0E96:5401..56A0` and `td3:0C5D:540E..`: apart from call targets, DGROUP offsets and the dispatch (9, 7 and 4 tests) they are identical. In particular: headers in ink 45 at (92, 35) and (92, 87), lines in ink 40 typed at (92, 67) or (92, 119), markers in ink 43, six one-row scrolls after every normal line, Space waits after `P`, `E` typed in box A and then box B, then F6 or Esc closes. The `td.cmm` panel, the portrait positions and the reload of `td.shp` record 0 into sprite slot 0 after each portrait are E1's. E2 key bytes: Space DS:19BD, Esc DS:1985, F6 DS:19C4.

The line loop index is a byte that starts at 1 and stops only at an end marker. Every loader of both episodes writes an end marker, so the loop always ends inside the table.

### 4.2 Ship library

[flight-screens §4.1.2](flight-screens.md) unchanged except the hatch and text rules of §3.3. E2: fade out, clear the back page, nine records into sprite slot 0 and drawn, hatch mask built in slot 0 and drawn per §3.3, text, seek `td.shp` to 0 and re-read record 0 into slot 0, fade in, wait for any key byte 1..600, clear the key bytes, fade out (`0E96:7653..7CEB`).

## 5. Edge cases and original bugs

| Quirk | Original behaviour | Port policy (proposal) |
|---|---|---|
| E2 transmission 5, line 21 (the end marker) has no speaker byte written | The byte keeps the value of an earlier transmission (or 0 after start-up). The end marker is typed in both boxes regardless, so nothing visible changes | Preserve (no effect) |
| Speaker bytes beyond the last line (E2 n = 2: 9, 10; n = 6: 11) | Written but never read before the next loader | Preserve (no effect) |
| E2 transmission 7 shows portrait 5 | The loader writes record 5 (the same portrait as transmission 5), not 7 | Preserve (OB-149) |
| E2 library: text and hatch of sprites 5 and 6 depend on the flight number | Sprite 5 entry from flight 11, sprite 6 entry from flight 15; before that the sprite is drawn and hatched with no text | Preserve |

No new original bug. Two resource-map errors were fixed in the same change: `e2.comms.06` had count 2 (the 8 strings `td2:2555:11E3..12E9` that transmission 6 uses were in no resource) and `e3.comms.02` had count 4 (12 strings of screen 2 were missing).

## 6. Test vectors

| # | Setup | Expected | Evidence |
|---|---|---|---|
| 1 | Extraction script run on `file0001` loaders `2A15:*` and library `0E8B:7374` | Reproduces flight-screens §3.3 (all nine rows, speakers, portraits) and §3.2 (positions, indices, inks) exactly | static (RE-12) |
| 2 | String counts | Highest `j` used per run = count − 1: E2 18, 9, 13, 24, 14, 11 (06 + 07), 14; E3 12, 15, 6, 20 | static, `td2.exe`/`td3.exe` bytes |
| 3 | E2 flight 11, F6 in phase 1 (transmission 4, announced at the start) | Portrait A = `td2.ccm` record 4 at (48, 32), record 0 at (48, 96); four Space waits, then the end marker in both boxes; RAM DS:31EC.. holds the 27 lines and speaker bytes of §3.1 row 4 | C-0044 (§6.1) |
| 4 | E2 Esc menu, ship library in flight 11 | Sprites 0..8 at the E1 positions; sprite 6 hatched with no text; sprite 5 not hatched, with its four lines at y 147..171, x 200 | C-0044 (§6.1) |
| 5 | E3 library, any flight | All nine sprites unhatched, nine four-line blocks | static |

### 6.1 Captures (C-0044)

E2 flight 11 reached through a prepared `TD2.SAV` (slot 1 = mission 11) and the menu restore, Esc at the `sbar` card, F6 at flight start.

- **RAM** (DGROUP `3EDA`, dumps of DS:31E0..387F at frames 1674 and 3314): headers, all 27 lines (each equal to `e2.comms.04[j]` for the `j` of §3.1 row 4), the speaker bytes of lines 1..27 and portraits 4 / 0: 29 of 29 slots, 27 of 27 speaker bytes.
- **Screen** (fb every 25 frames, 320 × 192): `td2.ccm` records 4 and 0 at (48, 32) and (48, 96) with 0 differing pixels from frame 1550 to 3325. Ink-43 marker pixels: the prompt in box A (frames 1675..1850), in box B (1950..2200), in box A (2400..2550, 2750..2900), each erased by the all-space line typed after Space; box B fills with ink-40 lines exactly where the speaker table says B; the end marker appears in box A and then box B (frames 3050..3325, 197 ink-43 pixels each); F6 at frame 3325 closes the screen.
- **Ship library** (frame 4000, flight 11): all nine `td.shp` records at their positions with 0 differing pixels; sprite 6 covered by the colour-1 checkerboard (512 of 512 cells); sprite 5 unhatched; the 36 text lines of §3.3 rendered from `e2.ship_library` with [render/text](../render/text.md) match pixel for pixel (ink 96/92/94), and the sprite-6 block is absent.

## 7. Presentation and audio hooks

As E1: no sounds; music continues; typing paces on vertical retrace.

## 8. Open questions

None for the data. The E3 rows are static only (no E3 capture of a transmission or of the library).

## 9. Provenance

| Claim | Address / evidence | Method | Confidence |
|---|---|---|---|
| E2 tables (headers, lines, markers, speakers, portraits) | `td2:2555:01D4..1757`; DGROUP DS:31EC, 321F, 384B, 386A..386C | static (listing + exe strings) | CONFIRMED |
| E3 tables | `td3:2215:0135..0C34` (the body of `042A` after `047B` was disassembled on RE-12's project copy; Ghidra had stopped there); DS:31E8, 321B, 3847, 3866..3868 | static | CONFIRMED |
| Comm screen identical to E1 but for the dispatch | instruction-shape comparison of the three routines | static | CONFIRMED |
| Markers | E2 `0E96:53DC`, `53E6` (= `e2.comm.prompts`); E3 `e3.comm.prompts` | static | CONFIRMED |
| Library text table, hatch and block rules | E2 `0E96:7653..7CEB` (tests `789A`, `78B1`, `7A59`, `7BFC`); E3 `0C5D:7337..` | static | CONFIRMED |
| Script check against E1 | same extractor on `file0001` reproduces flight-screens §3.2/§3.3 | static | CONFIRMED |

## 10. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-05 | RE-12 (Archaeologist + Spec author) | Initial: E2 and E3 transmission tables (U-310), E2/E3 ship-library text tables (U-313); `e2.comms.06` and `e3.comms.02` counts fixed |
