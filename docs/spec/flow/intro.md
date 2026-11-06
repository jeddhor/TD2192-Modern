# Intro sequence (`file0000.exe`)

| | |
|---|---|
| **Spec ID** | `flow/intro` |
| **Revision** | 2 |
| **Status** | Draft |
| **Fidelity tier** | T0 for every drawn frame (layout, glyphs, tile grid, sprite blit, captions, palette values); T1 for the flyover state per step (scroll, hoverskid path, `Random` stream); T2 for wall-clock timing (fade speed, delays, step rate) and audio |
| **Confidence** | CONFIRMED for the drawn frames and the flyover state: 1,792 framebuffer dumps of C-0047 match a model of this spec pixel for pixel (every date, narration, flyover, `intro1` and `intro2` frame that is not a loading frame), and 903 per-frame RAM dumps match the state rules of §4.6. LIKELY for the sound-effect schedule (static only) |
| **Tasks** | M1-10 (RE/SP), M5-08 (IM), M11-05 (credits text) |
| **Unknowns** | U-036 (the five intro effects), U-370 |

## 1. Summary

The intro is a separate program, `file0000.exe`, that the launcher runs once before Episode 1 ([launcher](launcher.md) §4.3). It has five parts:

1. a **date card**: black screen, one red line that fades in and out;
2. the planet still **`intro0.scr`** with three narration cards (cyan text that fades in, holds, fades out);
3. the **flyover**: a vertically scrolling desert road (`intro.map` drawn with `td.blk` and `td.dun` tiles) with the hoverskid (sprite 4 of `td.shp`) weaving on it. The **credits** appear as one caption at a time, 1,618 steps at one step per BIOS tick, about 84 s;
4. the hangar still **`intro1.scr`** with two narration lines;
5. the explosion still **`intro2.scr`** with two lines in dark text that fades in.

Music: `intro1.cmf` from the date card until the flyover, `intro.cmf` once, then `intro2.cmf`, which the program restarts whenever it stops. Five sound effects play during the flyover on a Sound Blaster. Esc at a key check (after every narration card and after the flyover) ends the intro at once. Any other key cuts short the current hold or the flyover. Then the program fades to black, sets text mode 3 and exits.

The intro reuses Episode 1's machinery: the same unchained VGA layout with 352-pixel rows, the same `td.lt2` small font with the same advance table, and the same DOS-clock tick limiter. Its routines are separate copies, and they differ in places noted below.

## 2. Definitions and units

| Term / quantity | Type in original | Unit | Range | Notes |
|---|---|---|---|---|
| VGA memory | 4 planes × 65,536 bytes | | | Unchained mode 13h with 88-byte rows (352 pixels). Pixel (X, Y) is byte `(Y × 88 + X div 4) mod 65536` of plane `X mod 4`. All offsets wrap at 64 KB |
| VGA row | Integer | 88 bytes | 0..744 | Row R starts at byte R × 88 |
| Start address | Word | byte | | CRTC start; the screen shows 320 × 200 pixels from it: screen pixel (x, y) = VGA byte `start + y × 88 + x div 4`, plane `x mod 4` (no pel panning) |
| Page p | 0 or 1 | | | Flyover double buffer. Page p's origin is VGA row 264 × p; its tile grid starts 32 rows below the origin (row 32 + 264p) |
| Step s | Word | | 1..1618 | One pass of the flyover loop (`DS:0190`); set to 10000 to end the loop |
| Fade level | Integer | | 0..64 | Full-palette fade: DAC = palette × level div 64 |
| Timestamp T | Word, wraps | 1/100 s | 0..65535 | `((h × 60 + m) × 60 + s) × 100 + hs` of DOS Get Time, every product truncated to 16 bits (as [timing](../core/timing.md) §2) |
| Half-second | | 50 T units | | Unit of the narration holds |
| Colour fade | | | | One DAC entry stepped 0..63 or 63..0, one level per retrace wait (§4.3) |
| Text pen (x, y) | Integer | pixel | | As [render/text](../render/text.md) §2: glyph pixel (col c, row r) goes to (x + 1 + c, y + 1 + r) |

## 3. Data

### 3.1 Files, in load order

| File | Read | Destination | Notes |
|---|---|---|---|
| `td.cfg` | bytes 0 and 1 | music, sound-effects settings | §4.1 (the third byte is not read) |
| `td.lt2` | 950 bytes | font block (heap) | Same font as Episode 1 ([render/text](../render/text.md) §3.1) |
| `td.shp` | 20,480 bytes | sprite block (heap, 62,464 bytes reserved) | Only record 4 (the hoverskid) is drawn |
| `intro.map` | 40,000 bytes | `DS:019A` | **20 columns × 2,000 rows, column-major** (§3.2) |
| `td.blk` | 232 rows × 352 bytes = 81,664 bytes | VGA rows 528..759 | Same rule as Episode 1 ([FORMATS](../../re/FORMATS.md) §4): row count `Round(FileSize / 352) − 2 + 1`. Tile t lands at VGA byte `(B580h + 256 t) mod 65536`. Rows from 745 on (from tile 74's pixel row 19 to the end of tile 79) wrap to the start of VGA memory, where `intro0.scr` later overwrites them; `intro.map` uses no tile above 73 |
| `td.dun` | 96 × 32 bytes | VGA row 734, x 192.. | Written as one 3,072-pixel line, which is linear in VGA memory: `td.dun` bytes 0..3071 go to bytes `FC80h + k div 4`, plane `k mod 4`, that is **tile slots 71, 72, 73**, replacing `td.blk`'s tiles 71..73. They are the desert-dune tiles of `intro.map` |
| `whoosh.voc`, `pulse.voc`, `ric3.voc`, `scratch.voc`, `home.voc` | whole | heap | Only when sound is not overridden (§4.1). U-036 |
| `intro.cmf`, `intro1.cmf`, `intro2.cmf` | whole | heap | Same condition |
| `intro0.scr` | palette + 200 rows | working palette `DS:AD1E`; VGA rows 0..199 | |
| `td.pal` | 768 | working palette | Flyover palette |
| `intro1.scr` | palette + 200 rows; then bytes 58,368..60,607 again | working palette; VGA rows 32..231; strip buffer | The strip is image rows 180..186 (7 × 320) |
| `intro2.scr` | palette + 200 rows; then bytes 29,568..31,807 again | working palette; VGA rows 232..431; strip buffer | The strip is image rows 90..96 |

### 3.2 `intro.map`

The program indexes the map as a Pascal `array[1..20, 1..2000] of Byte`: element (i, j) is file byte `(i − 1) × 2000 + (j − 1)`. i is the screen's horizontal axis and j the vertical one, so the file is column-major like the episode maps ([FORMATS](../../re/FORMATS.md) §5), but 20 tiles wide and 2,000 tall. Columns 8 and 11 hold road-edge tiles (56, 57), columns 9 and 10 the road (49, 50), the rest desert (15, 16, 71, 72, 73, and 14 in places). The flyover shows columns 5..14 (the 11th drawn column, 15, lies off screen) and rows 2..848.

### 3.3 Exe resources ([file0000.toml](../../re/exe-resources/file0000.toml))

| Id | Count | Use |
|---|---:|---|
| `intro.narration.01` | 5 | [0] date line; [1], [2] one line each; [3], [4] the third card's two lines |
| `intro.credits` | 25 | Flyover captions [0]..[24] (§3.5). Two other caption strings are blank and are not in the resource: two spaces (file offset 0x135A) and one space (0x1503); a blank caption draws nothing |
| `intro.narration.02` | 2 | `intro1.scr` lines |
| `intro.narration.03` | 2 | `intro2.scr` lines |

The strings use only lower case, digits, space, `,` `-` `.` `:` and `^`. `^` is drawn as a raised `2` (§4.4).

### 3.4 Palette entries the program changes

| Entry | Where | Value |
|---|---|---|
| 52, 92, 195, 221 | `intro0.scr` palette buffer | set to (0, 0, 0) after loading, so they stay black through both full fades. 52, 92 and 195 are the narration inks; 221 is never used. No `intro0.scr` pixel uses these entries |
| 52 | date card | (k, 0, 0), red |
| 52, 92, 195 | narration cards | (0, k, k), cyan |
| 46 | flyover, after the fade-in (DAC only) | (0, 63, 63): caption ink |
| 45 | `intro1.scr`, after the fade-in (DAC only) | (0, 0, 0), then (0, k, k) per line. Not used by the image |
| 140, 141 | `intro2.scr`, after the fade-in (DAC only) | (63, 63, 63), then (k, k, k) per line. Not used by the image |

### 3.5 Flyover caption table

The caption is chosen from scratch at every step: start with a blank, then for each row in order, if s > threshold, the caption becomes that row's text (so the last threshold passed wins).

| s > | Caption | | s > | Caption |
|---:|---|---|---:|---|
| (none) | blank | | 1000 | credits[5] |
| 70 | credits[0] | | 1050 | blank |
| 150 | blank | | 1100 | credits[12] |
| 190 | credits[1] | | 1150 | credits[13] |
| 270 | blank | | 1200 | blank |
| 300 | credits[2] | | 1250 | credits[14] |
| 400 | blank | | 1300 | credits[15] |
| 440 | credits[3] | | 1310 | credits[16] |
| 500 | credits[4] | | 1320 | credits[17] |
| 550 | credits[5] | | 1330 | credits[18] |
| 600 | credits[6] | | 1340 | credits[19] |
| 650 | blank | | 1350 | credits[20] |
| 700 | credits[7] | | 1360 | credits[21] |
| 750 | credits[4] | | 1370 | credits[22] |
| 790 | credits[8] | | 1380 | credits[23] (15 spaces: blank) |
| 830 | credits[9] | | 1420 | credits[24] |
| 870 | credits[10] | | 1470 | credits[4] |
| 910 | blank | | 1530 | blank (one space) |
| 950 | credits[11] | | | |

Where the caption is drawn depends on s. Outside the windows below, nothing is drawn.

| Condition | Centre cx | Ink pen row Y (screen) |
|---|---:|---:|
| s < 400 or s > 1420 | 160 | 84 |
| 440 < s < 650 | 250 | 134 |
| 700 < s < 900 | 80 | 44 |
| 950 < s < 1050 | 250 | 24 |
| 1100 < s < 1200 | 80 | 54 |
| 1250 < s < 1380 | 250 | 114 |

(The code stores the rows as 100, 150, 60, 40, 70, 130 relative to the scrolled page; the screen row is 16 less, §4.6.)

### 3.6 Sound-effect schedule (U-036)

All effects need: sound not overridden and effects setting 2 (Sound Blaster). Rules 1..7 also need a Sound Blaster that answered at start-up for their `PlayVoc` ("card present"); rule 8 does not check it. "Volume v" is `WORX_SetVoiceVolume(v, v)` (SB Pro mixer voice level v/15 left and right); some volume calls happen even without a card. The checks run in this order every step, after the tile grid is drawn:

| # | When | Action |
|---:|---|---|
| 1 | s = 1 (card present) | play `whoosh`; volume 12 |
| 2 | s = 150 (card present) | play `scratch`; volume 12 |
| 3 | s = 155 | play `ric3` (card present); volume 13 |
| 4 | 155 < s < 240, no effect playing, flag clear | set flag; play `scratch` (card present); volume 12 |
| 5 | s = 240 | play `scratch` (card present); volume 12; clear flag |
| 6 | s = 245 (card present) | play `home`; volume 13 |
| 7 | 265 < s < 330, no effect playing, flag clear | set flag; play `scratch` (card present); volume 12 |
| 8 | any step, no effect playing | volume 10; play `pulse` |

Starting an effect stops the one playing ([voc-and-adpcm](../audio/voc-and-adpcm.md) §2.1). So `pulse` fills every silence and is cut by the others. The flag (`DS:0069`) starts clear. Rule 4 fires once, on the first step after `ric3` (or the `scratch` of rule 2) has finished. Rule 7 fires once, on the first silent step after s = 265. Rule 8 runs on the same step after rules 4 and 7, so on that step the new `scratch` is already playing and `pulse` waits.

### 3.7 Constants

| Constant | Value | Provenance |
|---|---|---|
| Map view origin (column i, initial row j) | i = c + 5, j = r + v194; v194 starts at 2 | `file0000:0000:1270..1276`, `1B86..1BC7` |
| Horizontal offset v196 | 16 (constant), so the start address gets `Round(16 / 4) − 4` = 0 | `0000:127C`, `1BE1..1C1C` |
| Vertical offset v198 | starts at 16; the screen shows the grid from row v198 − 16 | `0000:1282`, `1C0C..1C1B` |
| Hoverskid sprite and x | `td.shp` record 4, x = 142 (drawn at the 4-pixel boundary 140) | `0000:1F75..1FA1` |
| Hoverskid y | starts at −32 (`DS:AA40`) | `0000:134B` |
| Draw limit | sprite drawn only while y ≤ 232 (signed) | `0000:1F6D` |
| End | y ≥ 292 sets s := 10000 | `0000:2837..283F` |
| Fade-out pause after the flyover | `Crt.Delay(100)` (100 ms busy loop) | `0000:2927` |
| Narration holds | date 4, `intro0` 11 each, `intro1` 10 each, `intro2` 11 each (half-seconds) | `0000:17F2`, `18E9`, `199F`, `1A76`, `2ABC`, `2BB7`, `2DC7`, `2EC6` |

## 4. Behaviour

### 4.1 Start-up and sound configuration (`file0000:0000:0FCA..1266`)

```text
override := first parameter is /PC, /pc, /NOSOUND or /nosound
if override: write a one-line debug message in text mode (not ported)
if not override: WORX_Init; card := WORX_SbInit (−1 = none) else card := −1
if td.cfg opens and not override: music := td.cfg[0]; sfx := td.cfg[1]
else: sfx := 2 if card ≠ −1 else 3; music := WORX_DetectAdLib + 1 (0 if override)
first parameter /pc or /PC  -> music 1, sfx 3
                /nosound or /NOSOUND -> music 1, sfx 1
                /sb or /SB  -> music 2, sfx 2
                /a or /A    -> music 2, sfx 3
```

Music plays only when `music = 2` and the sound is not overridden. Effects need `sfx = 2` (there is no PC-speaker path in the intro). After the files of §3.1 are open: if music = 2, `WORX_SetFmVolume(10, 10)`; if sfx = 2, `WORX_SetVoiceVolume(12, 12)` and `WORX_SetTimerStreaming(1)`; `Crt.NoSound`. The VOCs and CMFs are loaded only when not overridden.

The video mode is set up once: memory cleared, mode 13h, all 256 DAC entries black, unchained, 88-byte rows, split screen off, start address `0B00h` (`0957:02BC`, `0957:0266(400)`, `0957:0343`).

### 4.2 Primitives

| Primitive | Behaviour | Address |
|---|---|---|
| Wait retrace | loop until VGA status bit 3 is set. **Returns at once while a retrace is in progress** | `0000:0000` |
| Full fade in / out | for level 0..64 (out: 64..0): wait retrace, then DAC entries 1..255 := working palette × level div 64 (integer, truncating). Entry 0 is never touched. 65 levels | `0000:0016`, `0000:00AF` |
| Colour fade | for k in the given order: wait retrace, set one DAC entry | inline |
| Hold(n) | record T₀ (DOS time, done by the caller just before); then repeat: T := now; return if T > T₀ + 50n (unsigned 16-bit); return if T < T₀; return if a key is waiting (`KeyPressed`, the key is not read) | `0000:0A6D` |
| Key check | if a key is waiting, read it into `DS:008E`; if it is Esc, go to the exit (§4.9); if it is 0, read once more (extended key). Without a waiting key `DS:008E` keeps its value | inline, after every card |
| Text A / Text B | §4.4 | `0000:0302`, `0000:06BC` |
| Tile, sprite | §4.5 | `0957:01D5`, `0957:0000` |
| Put pixel | plane X mod 4, byte `(Y × 88 + X div 4) mod 65536` | `0957:0375` |
| Set start | wait until display is enabled, write the CRTC start address, wait for the retrace bit | `0957:0343` |
| Clear page | 232 rows × 88 bytes to 0 from `0B00h` (page 0) or `5AC0h` (page 1) | `0957:0170` |

### 4.3 Timing at preset P1 (T2)

Retrace waits, holds and the flyover limiter are what pace the intro. The result depends on CPU speed and on the BIOS tick rate, which WORX changes ([timing](../core/timing.md) §3: about 19.20 Hz while a song is playing, 18.52 Hz with WORX installed and no song, 18.21 Hz without WORX). Measured at preset P1 (C-0047, 70.086 frames/s):

| Item | P1 value |
|---|---|
| Full fade | 1 level per frame: 65 frames |
| Colour fade (64 levels) | about 10 levels per frame: 7 frames (the wait returns at once inside a retrace) |
| Hold(n) | n × 0.5 s of DOS time. While a song plays the DOS clock runs 19.20 / 18.21 fast: Hold(4) = 132 frames, Hold(11) = 366, Hold(10) = 334 |
| Flyover step | one BIOS tick: 3 or 4 frames (19.2 per second) |

Port policy (Classic): full fades at 1 level per 70 Hz frame; colour fades at 10 levels per frame; holds in DOS-time units at the current BIOS tick rate; the flyover at the BIOS tick rate ([timing](../core/timing.md)). The 16-bit timestamp wrap (OB-034) is not reproduced.

### 4.4 Text (`Text B` `0000:06BC`, `Text A` `0000:0302`)

Both are the small-font string routine of [render/text](../render/text.md) §4.2, **transparent**, with the same glyph table (§3.1 there) and advance table (§3.2 there), with these differences:

- **`:`** advances 2 and is code-drawn as pixels (0, 1) and (0, 3). The intro has no code-drawn `?` or `!` (they do not occur in its text).
- **`^`** is replaced by `2` and drawn 4 rows higher (pen y − 4): a superscript.
- No glyph is looked up for space, `,` `.` `-` `:` `"`. The other code-drawn marks are as in text.md §3.3.
- **Text B** is text.md's rule exactly: pen offset starts at 1; `x` and `y` take one pixel off before drawing; after each character, offset += advance.
- **Text A** adds one pixel of letter spacing: pen offset starts at 1; before drawing each character other than `x` or `y`, offset += 1; after it, offset += advance. Every character therefore sits one pixel further right than in Text B for each character before it, plus one.

Calls (pen x, pen y, ink):

| Text | Routine | Pen |
|---|---|---|
| Date line | B | (160 − 5 × (len div 2), VGA row 280), ink 52; shown at screen row 80 |
| `intro0` line 1 | B | (20, 180), ink 52 |
| `intro0` line 2 | B | (27, 180), ink 92 |
| `intro0` line 3 | B | (12, 180) and (55, 190), ink 195 |
| Flyover caption | A | black copy at (cx − 1 − 6 × (len div 2), Y + 1), then ink 46 at (cx − 6 × (len div 2), Y), screen coordinates from §3.5 |
| `intro1` lines | B | (8, 212) VGA row, screen row 180, ink 45 |
| `intro2` line 1 | B | (35, 322) VGA row, screen row 90, ink 140 |
| `intro2` line 2 | B | (17, 322), ink 141 |

`len` is the string length including leading spaces. Narration text is never erased in `intro0`: each card's line is drawn over the previous one in a new ink, and the old ink is black by then.

### 4.5 Tiles and the hoverskid blit

**Tile.** A grid cell (c, r) on page p is the 32 × 32 tile copied, with latched writes, from VGA byte `(B580h + 256 × tile) mod 65536` (8 bytes per row, 32 rows) to page p's grid position: VGA byte `0B00h + 5AC0h·p + r × 0B00h + 8c`. That is VGA row 32 + 264p + 32r, x = 32c. Tile pixel (tx, ty) is `td.blk` byte `1024 × tile + 32 ty + tx` (or `td.dun` for tiles 71..73).

**Hoverskid** (`0957:0000(page, sprite, y, x)`). The routine draws 32 rows of 32 pixels to VGA rows `232p + (y mod 256) + dy` (dy = 0..31) at pixels `(x and FFFCh) + dx` (dx = 0..31). Only the low byte of y is used, and x is truncated to a multiple of 4. Each destination pixel takes the source byte

```text
src(dx, dy) = sprite × 1024 + (31 − dy) × 32 + (dx + 1)
```

and writes it unless it is 0. The sprite is therefore **flipped vertically** (nose down) **and shifted one pixel left**: source column 0 is never drawn, and destination column 31 takes byte `(32 − dy) × 32`, the first pixel of the source row below. For dy = 0 that is the first byte of the next sprite record (record 5, row 0, column 0). Both effects show on screen (OB-059).

### 4.6 The flyover (`0000:1B6B..2921`)

Setup, after the `intro0` part (§4.8):

```text
load td.pal into the working palette
if music: WORX_StopMusic; WORX_SetMusicLoop(0); play intro.cmf; musicState := 1
page := 0; v194 := 2; v198 := 16; y := −32; steer := 0; speed := 0; s := 0; flag := 0
draw the grid on page 0, columns c = 0..9 only (rows r = 0..7)
start := 0B00h (page 0, v198 16); full fade in; DAC 46 := (0, 63, 63)
T_prev := DOS time
```

Each step:

```text
1. if not override and no music is playing (WORX fn 0Eh = 0) and music = 2:
       WORX_StopMusic; play intro2.cmf; musicState := 3
2. s := s + 1
3. draw the grid on page `page`: c = 0..10, r = 0..7, tile = map[c + 5, r + v194]
4. sound effects (§3.6)
5. if y ≤ 232: hoverskid(page, 4, y + v198 + 32 × page, 142)
6. caption := §3.5 text for s; if s is in a caption window:
       Text A in colour 0 at (cx − 1 − 6 (len div 2), row + 1 + v198 + 264 page)    (VGA rows)
       Text A in colour 46 at (cx − 6 (len div 2), row + v198 + 264 page)
   where row is 100, 150, 60, 40, 70 or 130 per the window
7. wait: repeat T := DOS time until T > T_prev + 4 (unsigned) or T < T_prev; T_prev := T
8. set start := 0B00h + 5AC0h × page + (v198 − 16) × 88          (shows this page)
9. page := 1 − page
10. motion (in this order):
       if 19 < s ≤ 37:  y := y + (37 − s);  v198 := v198 + (s − 20)
       if 37 < s < 1600: v198 := v198 + 17
       if s ≥ 1600:      if s ≤ 1617: v198 := v198 + (1617 − s)
                         y := y + (s − 1600); if y ≥ 292: s := 10000
       if 37 < s < 1605:
           steer := 1 − Random(3)                       (one Random call per step)
           if y < 65: steer := 1;  if y > 145: steer := −1
           speed := speed + steer
           if speed > 5: speed := 2;  if speed < −5: speed := −2
           y := y + speed
       if v198 ≥ 48: v194 := v194 + 1; v198 := v198 − 32
11. if no key is waiting and s ≠ 10000: next step
    otherwise: key check (Esc -> exit, §4.9); leave the loop
```

`Random` is the BP7 generator of [bp7-runtime-semantics](../core/bp7-runtime-semantics.md) §4.1. The intro never calls `Randomize`, so `RandSeed` starts at 0 and the hoverskid's path is the same on every run. The 1,567 calls (steps 38..1604) are the only use of the generator.

Seen on screen, the frame shown after step s is page p(s) = (s + 1) mod 2:

- the grid pixel rows v198 − 16 .. v198 + 183 of the 352 × 256 grid, so screen pixel (x, y) shows tile map[x div 32 + 5, (y + v198 − 16) div 32 + v194] at pixel (x mod 32, (y + v198 − 16) mod 32);
- the hoverskid at screen (140, y − 16) when its VGA row does not wrap (y + v198 + 32p < 256);
- the caption at the screen rows of §3.5.

The scroll accelerates over steps 20..37 (v198 gains 0, 1, ..., 17 px) while the hoverskid swoops in from above the screen (y gains 17, 16, ..., 0, ending at 121). It then cruises at 17 px per step with the random weave inside y 65..145. From step 1600 the scroll slows to a stop (17, 16, ..., 0 px over steps 1600..1617) while the hoverskid accelerates down and out of the screen. The loop ends after step 1618.

The memory model matters at the end of the flyover. When `y + v198 + 32` reaches 256 on a page-1 step, the hoverskid is written 256 rows too high, into page 0's grid area. Page 0 is on screen during that step's wait, so a fragment of the hoverskid flashes near the bottom of the screen (C-0047 f7495..7498, step 1612; OB-058). A port that models the 4 × 64 KB VGA memory with these addresses reproduces this. One that draws in screen space must special-case it.

After the loop (normal end or a non-Esc key): full fade out; `Crt.Delay(100)`; if music = 2 and musicState ≠ 3 and `intro.cmf` is loaded: musicState := 3, `WORX_StopMusic`, play `intro2.cmf` (so `intro2` is playing when the flyover was cut short).

### 4.7 Date card (`0000:1662..1849`)

```text
load intro0.scr: palette -> working palette; entries 52, 92, 195, 221 := 0; pixels -> VGA rows 0..199
start := 44C0h (VGA row 200, which is black)
draw the date line (intro.narration.01[0]) with Text B in ink 52 (§4.4)
if music: musicState := 2; play intro1.cmf (loop flag as the driver default: on)
colour fade DAC 52 := (k, 0, 0) for k = 0..63
T₀ := now; Hold(4)
colour fade DAC 52 := (k, 0, 0) for k = 63..0
key := ' '; key check
```

### 4.8 Narration cards on `intro0.scr` (`0000:1849..1ACA`)

```text
start := 0 (the planet image); full fade in; DAC 52, 92, 195, 221 := 0
for card in (line 1: ink 52) (line 2: ink 92) (lines 3 and 4: ink 195):
    draw its line(s) (§4.4)
    colour fade DAC ink := (0, k, k), k = 0..63
    T₀ := now; Hold(11)
    colour fade DAC ink := (0, k, k), k = 63..0
    key check
full fade out
```

### 4.9 `intro1`, `intro2` and the exit (`0000:2964..2F8E`)

```text
intro1:
    clear page 0; start := 0B00h
    intro1.scr: palette -> working palette; pixels -> VGA rows 32..231; strip := image rows 180..186
    full fade in; DAC 45 := 0
    line 1 (Text B, (8, 212), ink 45); fade (0,k,k) up; T₀ := now; Hold(10); fade down; key check
    restore the strip to VGA rows 212..218 (x 0..319)
    line 2; fade up; Hold(10); fade down; restore the strip; key check
    full fade out
intro2:
    clear page 1; start := 4FC0h (VGA row 232)
    intro2.scr: palette; pixels -> VGA rows 232..431; strip := image rows 90..96
    full fade in; DAC 140 := DAC 141 := (63, 63, 63)
    line 1 (Text B, (35, 322), ink 140); fade DAC 140 := (k,k,k), k = 63..0 (dark text appears)
    T₀ := now; Hold(11); fade k = 0..63 (text vanishes); restore the strip to rows 322..328; key check
    line 2 ((17, 322), ink 141); fade 63..0; Hold(11); fade 0..63; restore the strip
exit (also the target of every Esc):
    full fade out (from the working palette)
    if a key is waiting: read it; if it was 0 read one more
    if music: WORX_StopMusic
    if not override: WORX_SbShutdown; WORX_Shutdown
    text mode 3; Halt(0)
```

The final key read swallows one key typed during the last fade. In C-0002, C-0049 and C-0050 the second of two Esc presses is consumed there, not by `file0001`.

### 4.10 Skipping

| Key at... | Effect |
|---|---|
| During a hold | the hold ends at once (`KeyPressed`); the fade-out runs; the key check then reads the key |
| During a fade | the next hold ends at once, as above |
| Esc at a key check | exit (§4.9): full fade out from the current working palette, text mode |
| Other key at a key check | consumed; the next card starts |
| Any key during the flyover | the loop ends after the current step; Esc exits; another key goes on to `intro1` |

There are no key checks inside the `intro1` and `intro2` fades. A key pressed there takes effect at the next key check.

## 5. Edge cases and original bugs

| Case | Original | Policy |
|---|---|---|
| Hoverskid y uses only the low byte (OB-058) | at the exit a fragment flashes at the bottom of the screen for one step (C-0047 f7495..7498) | preserve (Classic); fix-enhanced |
| Sprite blit flips vertically and reads one column to the right (OB-059) | the hoverskid's left column is missing and its right column shows the row below (the first pixel of record 5 on the top row) | preserve (Classic) |
| `td.blk` rows 745..759 wrap to VGA offset 0 and are later overwritten by `intro0.scr`; tile 79's last 8 rows are never loaded | tiles 74..79 are garbage, but `intro.map` uses none above 73 | none needed |
| Colour fades run about 10 levels per frame at P1 (retrace wait returns at once inside a retrace) | CPU-speed dependent | port uses the P1 rate (§4.3) |
| Holds and the limiter use the 16-bit timestamp (OB-034) | a hold can end at once near a wrap | fix-all (frame counting) |
| After the flyover the program checks the `intro.cmf` pointer but starts `intro2.cmf` | harmless (all three are loaded together) | n/a |
| Esc during the date card or a card hold | the exit's full fade out starts from the working palette (`intro0.scr` with the four inks zeroed), so no colour flashes | preserve |
| `td.cfg` missing | defaults from detection (§4.1); the intro never writes `td.cfg` | n/a |

## 6. Test vectors

| # | Setup | Expected | Evidence |
|---|---|---|---|
| 1 | No input, P1 | file opens: `intro0.scr` f114, `td.pal` f1550, `intro1.scr` f7593, `intro2.scr` f8438; exit f9353 | C-0047 `int21.tsv` (= C-0001) |
| 2 | Date card | DAC 52 = (9,0,0) f137 … (63,0,0) f143; fade out f275..281 (54, 44, …, 4, 0) | C-0047 fb palettes |
| 3 | Full fades | 255 DAC writes per frame: f282..345, f1486..1549, f1553..1616, f7521..7585, f7615..7678, f8374..8437, f8460..8523, f9287..9350 | C-0047 `frames.tsv` |
| 4 | Flyover state after step s (v194, v198, y, steer, speed, RandSeed) | s = 20: (2, 16, −15, 0, 0, 0); 21: (2, 17, 1, …); 37: (6, 41, 121, 0, 0, 0); 38: (7, 26, 122, 1, 1, 1); 39: (7, 43, 124, 1, 2, 134775814); 40: (8, 28, 125, −1, 1, 3698175007); 100: (40, 24, 131, 0, −1, 2127203315); 500: (252, 40, 126, 0, 1, 2815784867); 1000: (518, 28, 87, 0, −1, 2302979935); 1604: (838, 46, 135, 0, 1, 1689385875); 1617: (841, 28, 278, …); after 1618: y 296, s = 10000 | C-0047 RAM (903 dumps, all match) |
| 5 | Step cadence | steps 1..1618 at frames 1618..7517 (3 or 4 frames per step) | C-0047 RAM `DS:0190` |
| 6 | Frame content | every flyover frame in f1600..2050, f3200..3300, f7300..7593 equals the page of step s − 1 (s = value at that frame), 0 differing pixels; f7495..7498 also include step 1612's wrapped sprite | C-0047 fb |
| 7 | Cards | date f135..281, `intro0` lines f282..1546, `intro1` lines f7681..8371, `intro2` lines f8526..9281: 0 differing pixels against §4.4/§4.7..4.9 | C-0047 fb |
| 8 | Esc, Esc at file0000 +300/+400 (P1) | the first Esc ends the first narration hold; exit at f426; the second Esc is swallowed by the exit's key read | C-0049, C-0050 `int21.tsv` |

## 7. Presentation and audio hooks

| When | Music | Effects |
|---|---|---|
| Date card start | `intro1.cmf` (driver default loop) | |
| Flyover setup | stop; loop off; `intro.cmf` | |
| Every flyover step, when no song plays | `intro2.cmf` restarted | §3.6 |
| After the flyover | `intro2.cmf` if it is not already the song | |
| Exit | stop | |

The CMF playback itself is [audio/cmf](../audio/cmf.md) (§6.2 C1 checks this exact sequence against C-0001).

## 8. Open questions

- [ ] Q1 (U-370): the WORX "music playing" function (fn 0Eh, CS:316A) is used only here. Does it read 0 for the step or two between the end of `intro.cmf` and the restart, so that `intro2` starts on the next step (cmf §6.2 C1 says 2 ticks)? The port can poll its own sequencer.
- [ ] Q2: sound effects were not checked against a recording (the effect schedule is static). A VE pass with C-0001's WAV should confirm the onsets at steps 1, 150, 155, 240, 245 (s/19.2 s after the flyover start).

## 9. Provenance

| Claim | Address / capture | Method | Confidence |
|---|---|---|---|
| Start-up, switches, `td.cfg`, sound flags | `file0000:0000:0FCA..1266`, `14B3..1593` | static | LIKELY |
| Asset loads, VGA layout, `td.dun` as tiles 71..73 | `0000:1266..165D` | static + C-0047 frames | CONFIRMED |
| `intro.map` 20 × 2000 column-major | `0000:1B86..1BC7` (stride 2000) + data | static + C-0047 frames | CONFIRMED |
| Fades, colour fades, holds, key checks | `0000:0000`, `0016`, `00AF`, `0A6D`; inline | static + C-0047 palettes / `frames.tsv` | CONFIRMED |
| Text A / B, glyphs | `0000:0302`, `06BC`, `0149` | static + C-0047 frames | CONFIRMED |
| Sprite blit, tile blit | `0957:0000`, `0957:01D5` | static + C-0047 frames | CONFIRMED |
| Flyover loop, motion, captions, limiter | `0000:1C91..28F5` | static + C-0047 RAM and frames | CONFIRMED |
| Sound-effect schedule | `0000:1D43..1F6D` | static | LIKELY |
| `intro1`, `intro2`, exit | `0000:2964..2F8E` | static + C-0047 frames | CONFIRMED |
| Esc handling | inline key checks; `0000:2F36..2F8E` | static + C-0049, C-0050 | CONFIRMED |

## 10. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-05 | RE-13 (Archaeologist + Spec author + Verifier) | Initial: whole intro, verified against C-0047 (1,792 frames, 903 RAM dumps) |
| 2 | 2026-11-06 | main session | §4.5 tile destination: `r × 0B00h`, not `(r + p) × 0B00h`, which matches the stated row 32 + 264p + 32r and the frames (found by IM-15) |
