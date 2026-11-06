# Original File Formats

The knowledge base for every data format in Traffic Department 2192. Each section has a **confidence** level (CONFIRMED / LIKELY / PARTIAL / HYPOTHESIS / UNKNOWN), its **evidence**, and **open questions** linked to [UNKNOWNS.md](UNKNOWNS.md).

Rules for this file:
- Facts only: sizes, offsets, layouts, value ranges, algorithms in our own words.
- **No game content.** Don't paste dialogue, story text or credits. Use placeholders such as `<text>`. A few-word anchor is acceptable when it is needed to locate something.
- Update the confidence column in [FILE_INVENTORY.md](FILE_INVENTORY.md) whenever a section here changes.

General conventions observed: little-endian; no headers or magic numbers on most game-specific formats (sizes and layouts are implied by the code); 8bpp indexed pixels; VGA 6-bit palettes.

---

## 1. Palette (`.pal`, and the prefix of many files)

| | |
|---|---|
| Confidence | **CONFIRMED** |
| Files | `td.pal`, `tdmen.pal` (768 bytes each); also the first 768 bytes of every `.scr`, `city.td`, `javelin.td`, `td.hlp`, `*.pic`, `newfaces.*` |
| Layout | 256 entries × (R, G, B), one byte each, range 0..63 (VGA DAC 6-bit) |
| Evidence | Max component value is exactly 63 in both `.pal` files and in the first 768 bytes of every full-screen file tested; renders with these palettes look correct. |

Conversion to 8-bit for presentation: `c8 = (c6 << 2) | (c6 >> 4)`. Keep 6-bit values internally for fades and effects.

## 2. Full-screen image (`.scr`, `city.td`, `javelin.td`)

| | |
|---|---|
| Confidence | **CONFIRMED** (rendered `hangar.scr`, `city.td`, `briefing.scr`, `intro0.scr`, `aboutus.scr`, `javelin.td`) |
| Size | 64,768 bytes |
| Layout | `[0..768)` palette (§1) · `[768..64768)` 320×200 pixels, row-major, top-left first, 1 byte per pixel |

## 3. Help screens (`td.hlp`)

| | |
|---|---|
| Confidence | **CONFIRMED** (loaders `file0001:0000:02AF` and `0E8B:6EDF`, M1-05) |
| Size | 128,768 = 768 + 2 × 64,000 |
| Layout | Palette, then page 1 (320×200), then page 2 (320×200), read row by row. The game shows page 1, waits for a key, then page 2. From the menu the palette goes to the menu palette buffer; in flight it replaces the flight palette until the help is closed. See [asset-load-sites.md](asset-load-sites.md) §3. |

## 4. Tileset (`.blk`)

| | |
|---|---|
| Confidence | **CONFIRMED** for `td.blk` (decode + loader + oracle); LIKELY for `td2.blk`/`td3.blk` (same loader code in `td2.exe`/`td3.exe`) |
| `td.blk` | 81,920 bytes = **80 tiles × 32×32** pixels. Tile *i* is at offset `i × 1024`, stored row-major. No header, no palette (drawn with `td.pal`). |
| `td2.blk`, `td3.blk` | 82,784 bytes = 81,920 + **864 trailing bytes** with no function: the loader reads only the first 448 of them, places them after tile 79 in video memory, and no map uses a tile index above 70, so they are never drawn (U-013, RESOLVED) |
| Loader | `Sim_FlyMission` (`file0001:0E8B:B880`, `td2:0E96:A76E`, `td3:0C5D:A9CC`) reads the file as rows of 352 bytes into off-screen VGA memory (unchained mode, 352-pixel rows) starting at row 496. Row count = `Round(FileSize / 352) − 2 + 1`: **232 rows (81,664 bytes) for `td.blk`, so the last 256 bytes (tile 79, pixel rows 24..31) are never loaded**; 234 rows (82,368 bytes) for `td2.blk`/`td3.blk`. Because 352 is a multiple of 32, tile *t* lands at VGA address `AA80h + 256·t` (8 bytes × 32 rows, four planes), and the tile blitter `386E:191C` copies it with latched writes. See [asset-load-sites.md](asset-load-sites.md) §5 |
| Port note | Tile 79's bottom 8 rows are blank in the original. E1 maps never use tile 79 (maximum index 70), so a port that loads all 1,024 bytes renders identically. |
| Evidence | Contact sheet of `td.blk` shows coherent 32×32 city tiles: building blocks, hazard-striped roofs, roads, desert, a "TD" logo, arrows. C-0002: 81,664 bytes in 232 reads plus the 3 seeks of `FileSize`. |

## 5. Map (`.map`)

| | |
|---|---|
| Confidence | **CONFIRMED** for `td.map` (rendered with `td.blk`; coherent city) · LIKELY for the others |
| Episode maps | `td.map`, `td2.map`, `td3.map`, `td4.map`: 40,401 bytes = **201 × 201** tile indices, **column-major** (`index = x*201 + y`), 1 byte each. CONFIRMED 2026-11-02 against oracle flight frame C-0002 f3500: with column-major indexing all 42 identifiable tiles in view match one map position (top-left visible tile (103, 104), tile-grid offset x 16 / y 0); with row-major indexing none does. The first survey's row-major render looked plausible only because a transposed city still looks like a city. |
| Intro map | `intro.map`: 40,000 bytes = **20 columns × 2,000 rows**, column-major (`index = x*2000 + y`), CONFIRMED 2026-11-05 (RE-13): the intro indexes it as `array[1..20, 1..2000]` (stride 2000, `file0000:0000:1B86..1BC7`) and C-0047's flyover frames match pixel for pixel. Columns 7 and 10 (0-based) are road edges (56, 57), 8 and 9 the road (49, 50). See [spec/flow/intro.md](../spec/flow/intro.md) §3.2 |
| World size | 201 × 32 = 6,432 pixels square |

Observed statistics:

| File | Distinct values | Max index | Most common indices |
|---|---:|---:|---|
| `td.map` | 65 | 70 | 49, 50, 16, 15, 61 |
| `td2.map` | 64 | 68 | 15, 16, 29, 28, 50 |
| `td3.map` | 41 | 40 | 25, 28, 37, 38, 31 |
| `td4.map` | 69 | 70 | 25, 28, 38, 37, 31 |
| `intro.map` | 10 | 73 | 15, 16, 72, 71, 73 |

Notes:
- The E1 city is surrounded by desert (tile indices 15/16 alternate pseudo-randomly as desert texture). An approach road enters from the north edge (the west edge in the first, transposed survey render).
- `intro.map` is not square: it is a 20-tile-wide road strip, 2,000 tiles long, column-major like the episode maps (confirmed above). The intro shows columns 4..13 (0-based). Its tiles 71..73 are `td.dun`, loaded over `td.blk`'s slots 71..73 (§12).
- In flight the playfield is 320×160 with the player's ship drawn at its centre (oracle C-0002), and the HUD bar `td.win` occupies rows 160–191 of a 320×192 display (U-025).
- `td3.map` and `td4.map` begin with near-identical bytes. Which episode uses which map (E3 has two settings: a mining station and Kolor) is U-006.
- Which tile indices are solid (collision) is U-009.

## 6. Sprites

### 6.1 Base sprite sheet (`td.shp`)

| | |
|---|---|
| Confidence | **CONFIRMED** (rendered) |
| Size | 20,480 = **20 sprites × 32×32**, row-major, consecutive |
| Orientation | Nose up (heading 0 = north, assumed) |
| Transparency | Index 0 renders as background in all sprites (LIKELY transparent) |

Slot contents as seen in the render (class mapping is U-008):

| Slots | Content |
|---|---|
| 0 | Boxy cargo vehicle (truck/container) |
| 1–9 | Nine distinct craft (several T.D. and Vulture designs) |
| 10 | Small spark/flash |
| 11–13 | Explosion frames (large) |
| 14 | Debris / smoke |
| 15 | Missile (red tip) |
| 16–19 | Projectiles / bolts (small, cyan, red, yellow) |

### 6.2 Rotation sets (`td.sc3`, `td.shr`, `td.sc2`)

| | |
|---|---|
| Confidence | **CONFIRMED** (measured overlap + visual check of the assembled 16-heading sheet, 2026-11-02) |
| Size | 38,400 bytes each = **24 cells × 40×40** |
| Content | Cells 0..19 are the same 20 sprites as `td.shp`, pre-rotated **counter-clockwise** by **22.5° (`td.sc3`)**, **45° (`td.shr`)** and **67.5° (`td.sc2`)**. Best overlap with the base sprites rotated CCW is at 20° / 40° / 65° in 5° steps; CW rotation matches much worse. **Cells 20..23 are blank** (all zero). |
| Heading construction | 16 headings, 0 = nose up, increasing counter-clockwise in 22.5° steps; `h % 4` picks `[shp, sc3, shr, sc2]`. **How the original turns or mirrors each cell is not "rotate by 90° × (h / 4)"**: E, S, NE, SE and four of the eight 22.5°/67.5° headings are mirror images, ten of the sixteen blits shift the image by one pixel, and the 40×40 cells are drawn with their top-left at the 32×32 sprite's corner (not centred). The exact per-heading rule is [flow/flight-screens §4.9](../spec/flow/flight-screens.md) (rev 3, U-360, CONFIRMED by C-0080). |
| Engine headings | Resolved (U-003): 16 discrete heading codes, one blit routine each ([structures §7](structures.md)). |

### 6.3 Other sprite files (LIKELY, from the loader `file0001:0E8B:B880`, M1-05)

In flight the game keeps one RAM **sprite bank of 27 slots × 32×32** (`td.shp` → slots 0..19, `td.exp` → 20..26) and three rotation blocks: `td.sc3` and `td.sc2` are read only up to **cell 8** (14,400 bytes each), `td.shr` up to cell 19 (32,000 bytes). Some missions overwrite slots:

| File | Bytes | Layout | Loaded into | When (E1 mission) |
|---|---:|---|---|---|
| `td.exp` | 7,168 | 7 × 32×32 explosion frames | slots 20..26 | always |
| `mine.td` | 1,024 | 1 × 32×32 | slot 3 | mission 7 |
| `lander.td` | 5,120 | 5 × 32×32 | first 2 → slots 0..1, last 3 → slots 4..6 | mission 20 |
| `td.hel` | 2,048 | 2 × 32×32 | slots 2..3 | missions 12, 15, 21 |
| `td.h2` | 9,600 | **3 × 2 × 40×40**: slots 2..3 pre-rotated by 67.5°, then 45°, then 22.5° | cells 2..3 of `td.sc2`, `td.shr`, `td.sc3` (in that order) | missions 12, 15, 21 |

So `td.h2` holds the rotated versions of the two `td.hel` sprites, in the same scheme as §6.2 (it is not a set of rotor frames).

## 7. Missions (`m<N>.td`, `m<N>.td2`, `m<N>.td3`)

| | |
|---|---|
| Confidence | LIKELY (structure); field semantics PARTIAL |
| Files | E1 `m1..m20.td`, E2 `m1..m20.td2`, E3 `m1..m19.td3` + `m21.td3` (60 files) |
| Size | 20,080 bytes each |

Layout:

| Offset | Size | Content | Evidence |
|---:|---:|---|---|
| 0 | 20 | Per-slot byte A, slots 1..20 (loaded to DS:3A52); zero in 59 of 60 files (`m18.td3` has `0x4C` at offset 10; U-014, U-038) | All files scanned; load `file0001:0E8B:C151` |
| 20 | 20 | Per-slot byte B (loaded to DS:3A8E); zero in all files (U-038) | Same |
| 40 | 20 | **Slot class code** per ship slot 0..19: values 0..8, `0xFF` = slot unused | All files scanned; value histogram 0:128, 1:37, 2:22, 3:29, 4:48, 5:4, 6:33, 7:243, 8:124, FF:552 |
| 60 | 20 | Per-slot byte D (loaded to DS:3A66); zero in all files (U-038) | All files scanned; load `0E8B:C19F` |
| 80 | 10,000 | **X paths**: elements (s, k) of an in-memory `array[1..20, 0..500]` (stride **501**), starting at element (1, 1). Slot s (1-based), step k is at `80 + (s−1)*501 + k − 1`. Tile-grid X (map column) | Load and use in `file0001:0E8B:C1A4..C3BF` (RE-2, [structures §2](structures.md#2-mission-file-mtd-20080-bytes-as-loaded)); C-0002 RAM |
| 10080 | 10,000 | **Y paths**, same layout: `10080 + (s−1)*501 + k − 1` | Same |

Observations:
- Path runs are smooth (±1 per step) with plateaus, consistent with recorded or hand-drawn routes on the tile grid. Unused tail entries are 0.
- **Correction (RE-2, 2026-11-02):** the earlier reading of a 500-byte stride (`80 + s*500`) shifts slot s by s − 1 bytes. The "odd first bytes" (`0xFF`, `0x80`, `0x11`, `0x64`, ...) are element k = 0 of each slot, which the game never uses: it copies steps k = 1..500 into the ship records. Slot 20 has file data only for k ≤ 481. The loader also replaces `0xFF` by 0 in steps 1..500 of slot 2 (both arrays). U-015 resolved.
- **Independent confirmation (RE-3, M1-05):** over all 60 mission files, steps of more than 2 tiles inside a path drop from 747 (stride 500) to 202 (stride 501). Whether anything ever reads element 0 is U-060. **Port:** `td-formats` implements the 501 stride (M2-11, 2026-11-02).
- Class codes index the ship-class tables directly (0..8 used, 9 empty, `0xFF` = no ship); numbers and names in [structures §6](structures.md#6-ship-classes-u-008) (U-008). The flight setup overrides some slots' codes per mission (`0E8B:60D8`).
- The reserved areas may be runtime state in the original's in-memory record (the file is probably block-read straight into a record).

## 8. Dialogue scripts (`dialogue.dat`, `dialogu2.dat`, `dialogu3.dat`, `safedial.dat`, `safedia2.dat`, `safedia3.dat`)

| | |
|---|---|
| Confidence | **CONFIRMED** (grammar and reader `file0001:051A:92AF`, M5-04 prerequisites, 2026-11-02); playback rules in [spec/narrative/dialogue-player.md](../spec/narrative/dialogue-player.md) |
| Encoding | ASCII lines terminated by CR LF. Each line starts with a control byte (< 0x20). Text is lowercase; `"` is used for an apostrophe. |
| Lines | 4,596 CR LF lines in `dialogue.dat` |

Grammar as observed (bytes shown in hex; `<...>` = placeholder):

```
06 <scene:u8>                         start of scene <scene> (1-based, sequential)
01 <slot:u8> <face:u8> <color:u8> <name>   speaker definition (slot 1..n, portrait index, text colour = palette index, display name)
...                                    (one 01-line per speaker)
03                                     end of scene header
02 01 <color:u8> <text>                text line in the colour of the speaker who says it
01 <slot> <face> <color> <name>        (inside a segment) a new or replacement speaker
05                                     page break (wait for Space)
04                                     end of scene
d                                      end of scene: the reader stops at the first line starting with 'd' (U-016)
```

Whole-file survey (all six files):

| File | Lines | `06` scenes | `03`/`04` segments | `02` lines | `05` breaks | Speaker lines |
|---|---:|---:|---:|---:|---:|---:|
| `dialogue.dat` | 4,597 | 103 | 103 | 3,100 | 760 | 324 |
| `safedial.dat` | 4,533 | 103 | 103 | 3,049 | 747 | 324 |
| `dialogu2.dat` | 6,012 | 125 | 126 | 4,126 | 1,032 | 351 |
| `safedia2.dat` | 5,881 | 125 | 126 | 4,025 | 1,002 | 351 |
| `dialogu3.dat` | 5,072 | 111 | 112 | 3,420 | 885 | 320 |
| `safedia3.dat` | 5,041 | 111 | 112 | 3,395 | 880 | 319 |

Notes:
- **Scene numbers are IDs, not a sequence.** E1 has 103 scenes with IDs from 1 to 157: 9 and 10 are absent, and 100, 105, 108, 109, 110, 156 and 157 sit among the lower IDs. The executables presumably look scenes up by ID. Uncut and edited files have identical ID lists.
- **Scene 13 quirk:** its `06` line is the single byte `06`. The ID byte 0x0D doubles as the CR of the CR LF terminator (`06 0D 0A`); its neighbours are 12 and 14. The bytes 0x0A and 0x0D never occur inside any other line. The original reads the ID with Pascal `Read(ch)`, which returns the CR (13) at the end of the line, so the bare `06` really is scene 13; the port reads it the same way.
- **Mid-segment speaker lines:** a `01` line can appear inside a segment to bring in a new speaker (for example a new character taking slot 2 part-way through a scene).
- **Speaker lines between segments** (after `04`, before the next `03`) occur in E3. They apply from the next segment on.
- **Portrait-only changes:** a 3-byte `01 slot face` line (E3: twice in `dialogu3.dat`, once in `safedia3.dat`) changes the portrait shown in a slot without changing colour or name. Face 200 appears when a character leaves, so it probably means "no portrait". Face 200 also appears in some header speaker lines.
- **Trailing spaces:** one `05` line in each E2 script carries two trailing spaces (`05 20 20`).
- No other line-leading codes occur. Every line starts with `01`–`06` or is the literal `d` after `04`. The file ends with CR LF.
- Speaker slots are only ever 1 and 2 (two speakers per scene). Face bytes are portrait numbers after a per-episode remap of a few codes; 200 means "load no portrait" (FORMATS §10).
- **Continuation segments:** in E2 and E3, a `03` follows a `04` directly, with no `06` or `01` lines (for example around line 4,002 of `dialogu2.dat`). This is a second segment of the same scene: the reader only stops at `d`, so it fades out at `04` and back in at the next `03`, keeping the speakers, portraits and text.
- Colour bytes can be any value, including control-range values (`0x14`, `0x16`, `0x17` ...). Parse lines positionally, not by scanning for printable text.
- The second byte of `02` lines is `01` in every one of the 21,115 lines. The reader reads it and **ignores it** (U-016, RESOLVED).
- The speaker is identified by the **colour byte**: a line whose colour equals slot 1's colour is slot 1; any other colour counts as slot 2. Examples of colour bytes seen: `0x2E`, `0x39`, `0x2A` (these happen to be printable as `.`, `9`, `*`, so be careful when viewing the file as text).
- Blank padding lines (`02 01 <color> <space>`) frame each speech block.
- Edited files share the same scene structure. Line content differs.

## 9. Fonts

| File | Bytes | Geometry | Glyph order | Confidence |
|---|---:|---|---|---|
| `td.lt2` | 950 | **38 × 5×5** | A–Z, dark block, bright block, 0–9 | **CONFIRMED** (decode + text routines `file0001:051A:088F`/`0B9F`) |
| `td-let.cin` | 2,560 | **40 × 8×8** | A–Z, 1–9, 0, `-`, `?`, `!`, `:` | **CONFIRMED** (decode + `051A:042A`); the game loads only the first **39 glyphs (2,496 bytes)**, so `:` is never available |
| `td.ltr` | 14,976 | **78 × 12×16** | A–Z, 1–9, 0, (blank), `-`, then blank cells | CONFIRMED (decoded). **Not opened by any game exe** (string scan of all five exes; no load site in `file0001`) |

Notes:
- Glyphs are stored consecutively, row-major. Index 0 is background. The files carry baked colours (white `0xF7` plus some coloured pixels), but **the game recolours every glyph**: `td.lt2` draws every non-zero pixel in the requested colour; `td-let.cin` draws only the pixels equal to `0xF7` in the requested colour and treats all others as transparent. The baked colours never reach the screen.
- **`td.lt2` is the font for all small text, including dialogue** (U-037, RESOLVED). Lower-case letters map to the glyphs; `.` `,` `"` (shown as an apostrophe) `?` `-` `!` are drawn by code as a few pixels; the advance is proportional (2 to 6 pixels per character). Full rules: [spec/render/text.md](../spec/render/text.md).
- `td.lt2` is read into a 975-byte block (39 glyphs), so glyph 38 would be uninitialised memory; no character maps to it.
- Character maps: `td.lt2`: `a`..`z` → 0..25, `{` → 26 (dark block), `|` → 27 (bright block, the typing cursor), `0`..`9` → 28..37. `td-let.cin` (after `UpCase`): `A`..`Z` → 0..25, `1`..`9` → 26..34, `0` → 35, `-` → 36, `?` → 37, `!` → 38, `:` → 39 (not loaded); space draws nothing; `.` and `,` are drawn by code. Both CONFIRMED.

## 10. Portraits (`newfaces.td`, `.td2`, `.td3`)

| | |
|---|---|
| Confidence | **CONFIRMED** (decoded and rendered, 2026-11-02) |
| Layout | A sequence of records, each a **768-byte palette + 160×100 pixels** (16,768 bytes). E1: 26 faces (435,968 bytes); E2 and E3: 27 faces (452,736 bytes). No header. |
| Pixel use | Faces use palette indices 1..~112 (up to 147 in E2), each with its **own palette**. The game uses **only palette entries 0..111 and pixel values 0..111** of each face: on load, every pixel value above 111 is replaced by 0 (CONFIRMED, `file0001:051A:1618`). The left face keeps entries 0..111; the right face is shifted to entries 112..223 (pixel + 112). There is no backdrop image on the dialogue screen. Details: [spec/narrative/dialogue-player.md](../spec/narrative/dialogue-player.md). |
| Animation | None: one still frame per face (U-019, RESOLVED). The non-speaking face is shown at half brightness. |
| Dialogue link | Speaker face bytes are 1-based portrait indices. A few codes stand in for faces whose numbers are awkward bytes in a text file; each episode remaps them before loading: **E1** 32 → 9, 65 → 10, 66 → 14, 78 → 13; **E2** 32 → 9, 33 → 10, 78 → 13; **E3** (applied in this order) 27 → 13, 28 → 27, 33 → 27, 89 → 16, 90 → 17, 87 → 18, 126 → 10, 32 → 9. **200 = do not load a portrait** (at the start of a scene that half stays black; mid-scene the previous portrait stays on screen). CONFIRMED (`file0001:051A:92AF`, `td2:04EC:96A4`, `td3:04A0:7795`) |
| Code | `td-formats::portraits` |

## 11. Story stills (`td1-13.pic` .. `td1-20.pic`, `secret.pic`, `pic4.td`)

| | |
|---|---|
| Confidence | **CONFIRMED** (data analysis and the loaders agree on every size, M1-05, 2026-11-02; `secret.pic` also by the C-0002 read pattern). `td1-19.pic` stays LIKELY: no E1 loader. |
| Layout | Optional 768-byte palette, then raw pixels, row by row. **No dimensions in the file**: each loader has its own width, height and screen position (U-018, RESOLVED: no compression). |

| File | Size | Own palette | Loader | Screen position (x, y of the top-left pixel) | Content (brief) |
|---|---|---|---|---|---|
| `td1-13.pic` | 202×86 | yes | `051A:4113` (4) | (44, 39) | Two portraits |
| `td1-14.pic` | 200×200 | yes | `051A:4113` (3) | (1, 0) | Character dissolving (teleport) |
| `td1-15.pic` | 255×121 | yes | `051A:4979` | (32, 30) | Shooting scene |
| `td1-16.pic` | 150×170 | yes | `051A:4113` (1) | (86, 0) | Character in a chair |
| `td1-17.pic` | 320×90 | **no** (current flight palette, `td.pal`) | `0E8B:9860` | (0, 67) of the flight page | Cockpit/bridge scene |
| `td1-18.pic` | 170×104 | yes | `051A:471B` | (76, 20) | Clone tube |
| `td1-19.pic` | 320×72 | **no** (correct with `td.pal`) | none in E1 | — | Ship over clouds |
| `td1-20.pic` | 320×100 | yes (byte-identical to `td.pal`); **the game skips it** and keeps `td.pal`, so the result is the same | `051A:4113` (2) | (0, 50) | Laser scene |
| `secret.pic` | 320×32 | yes | `051A:2B26` | (1, 0) | Hint banner |
| `pic4.td` | 100×150 | yes | `051A:4F5A` | (11, 16) | Corridor/room |

Full-screen `.scr` scenes are also drawn one pixel to the right (column 0 of the file at x = 1; the last column falls off screen); `javelin.td` and `city.td` are not. See [asset-load-sites.md](asset-load-sites.md) §4.

Code: `td-formats::picture::{STILLS, Picture}`.

## 12. Raw UI and sprite graphics

Headerless 8bpp. Geometry from data analysis (row-similarity peak, exact size fit, visual check), drawn with `td.pal` unless noted. `td-formats::picture::CELL_SHEETS` holds the confirmed ones.

| File | Bytes | Geometry | Content | Confidence |
|---|---:|---|---|---|
| `td.win` | 10,240 | 320×32 | **In-flight HUD bar**, plotted into VGA rows 0..31 and shown below the split line at screen rows **160..191** of a 192-line display; the playfield is 320×160 (U-025, RESOLVED). See [spec/render/hud.md](../spec/render/hud.md) | **CONFIRMED** (loader `0E8B:B880` + C-0002) |
| `td.sss` | 6,080 | 320×19 | Save-slot bar, drawn 10 times (rows 10, 29, ..., 181) on the save/restore screen | **CONFIRMED** (loader `051A:76EA`) |
| `td.fac` | 3,920 | 5 × 28×28 | "HIT F6" comm indicator (blank + 4 animation frames). On load, colour 38 is replaced by 39 in frames 0..3 | CONFIRMED (visual + loader) |
| `td.joy` | 1,600 | 40×40 | Joystick icon, drawn at (140, 70) on the calibration screen | CONFIRMED (visual + loader `0E8B:AF1F`) |
| `td.h2` | 9,600 | 3 × 2 × 40×40 | The two `td.hel` helicopter bodies pre-rotated by 67.5°, 45° and 22.5° (§6.3) | CONFIRMED (loader + visual) |
| `td.hel` | 2,048 | 2 × 32×32 | Helicopter bodies | CONFIRMED (visual) |
| `td.dun` | 3,072 | 3 × 32×32 | Desert/dune tiles (intro only). The intro writes them into VGA memory as **tile slots 71, 72, 73**, replacing `td.blk`'s tiles 71..73 for `intro.map` ([spec/flow/intro.md](../spec/flow/intro.md) §3.1) | CONFIRMED (visual + loader `file0000:0000:1593..1644` + C-0047) |
| `td1.ccm`, `td2.ccm`, `td3.ccm` | 9,216 / 7,168 / 5,120 | 9 / 7 / 5 × 32×32 | **Comm portraits** (small faces for in-flight transmissions) | CONFIRMED (visual) |
| `mine.td` | 1,024 | 32×32 | Mine | CONFIRMED (visual) |
| `lander.td` | 5,120 | 5 × 32×32 | Lander pieces (a large craft built from tiles?) | CONFIRMED (visual) |
| `td.exp` | 7,168 | 7 × 32×32 | Explosion frames | CONFIRMED (visual) |
| `td.men` | 144,000 | Five regions, see §12.1 | Menu graphics atlas | **CONFIRMED** (loaders `0000:2E88`, `0E8B:794C`; C-0002 read pattern) |
| `td.cmm` | 32,768 | **32 cells × 32×32**, forming a **256×128 panel** of 4 rows × 8 columns (cell *k* at column *k* mod 8, row *k* div 8) | F6 transmission panel: frame, two 32×32 portrait boxes, two text boxes | **CONFIRMED** (loader `0E8B:5435` + visual) |
| `td.sel` | 3,096 | Unknown | **Unused**: no exe contains the name (byte scan of all ten exes, 2026-11-02), so the port does not need it | UNKNOWN geometry; not needed |
| `td.wea` | 3,888 | **6 × 36×18** | Weapon icons for the HUD; one is drawn at HUD (179, 7) according to the ship class | **CONFIRMED** (loader `0E8B:B880` + visual) |

### 12.1 `td.men` regions (M2-09)

| Bytes | Geometry | Palette | Content | Used by `file0001` |
|---|---|---|---|---|
| 0..15,999 | 160×100 | `td.pal` (flight), entries 107..110 overridden as item colours | In-flight computer menu (four items) | `0E8B:794C`: drawn at (80, 50) of the flight page |
| 16,000..79,999 | 320×200 | `tdmen.pal` | Main menu screen | `0000:2E88`: full screen (VGA rows 0..199). Entries 224..233 are the item colours |
| 80,000..120,959 | 4 × 320×32 | `tdmen.pal` | Confirmation boxes: quit (Y/N); current game lost (Y/N); save 0–9; restore 0–9 | Copied to VGA rows 432..559; boxes 1 and 2 are shown by `0000:330E` over menu rows 84..115; boxes 3 and 4 are loaded but never shown in E1 |
| 120,960..124,959 | 2 × 100×20 | `tdmen.pal` | Left and right halves of a 200×20 "game not loaded" message | VGA (0..199, 560..579); not shown in E1 |
| 124,960..128,959 | 100×40 | `tdmen.pal` | "Game saved" (top 20 rows), "game loaded" (bottom 20 rows) | VGA (0..99, 580..619); not shown in E1 |
| 128,960..143,999 | — | — | Not image-like | Never read by `file0001` |

The main-menu loader also copies main-menu rows 84..115 (bytes 42,880..53,119) to VGA rows 400..431, to restore the menu after a confirmation box (`0000:330E` with box 0).

## 13. Configuration and saves

### 13.1 `td.cfg` (3 bytes), CONFIRMED

`[music, sfx, controller]`. The shipped value is `02 02 02`. 1-based choices of the configuration screen: music {1 none, 2 SB/AdLib}, SFX {1 none, 2 SB, 3 PC speaker}, controller {1 keyboard, 2 keyboard + joystick}. CONFIRMED 2026-11-05 (RE-13): `file0001`'s configuration screen writes exactly these values (C-0048: `01 03 01` after choosing no music, PC speaker, keyboard); the intro reads bytes 0..1 and `td1.exe` byte 0. When and how the screen chooses them: [spec/flow/launcher.md](../spec/flow/launcher.md) §4.9.

Static (M1-05, 2026-11-02): `file0001` reads the three bytes into DS:12C6 (music), DS:12C7 (effects) and DS:12C8 (controller) at start-up (`0000:49C1`) and writes them back from the configuration screen (`0000:5044`). Music plays only when byte 0 = 2 (`0E8B:07ED`); the joystick is read only when byte 2 = 2; the effects start-up `1E2A:06C6` loads the VOC files for 2 and installs a PC-speaker tick handler for 3. This agrees with the meanings above.

### 13.2 `td.sav`, `td2.sav`, `td3.sav` (70 bytes), CONFIRMED

Shipped `td.sav`: 10 × `02 0D 00`, then 40 × `0xFF`. The shipped `td2.sav` (10 × `02 0F 00`) and `td3.sav` (10 × `02 0E 00`) have the same layout, so every shipped slot is a used slot at mission 2.

Slot *s* (1..10) is the 3 bytes at offset (*s* − 1) × 3: the **mission number** to play next (u8; 0 or > 20 means "empty slot") and the **kills to date** (u16 little-endian, loaded into the i16 `g_KillsToDate`). A save writes **all ten slots** (bytes 0..29): the chosen slot gets (mission, kills), the others keep their kills word and their normalised mission byte (an empty slot's 21..255 becomes 0). Bytes 30..69 are never read or written by any E1 code. Static (`051A:76EA`, `051A:7EEA`) and oracle (C-0042, C-0043); full rules: [flow/save-screens](../spec/flow/save-screens.md) (U-011, U-301 resolved).

### 13.3 `td.stt`, `td2.stt`, `td3.stt` (260 bytes as shipped), CONFIRMED

Text file, 10 lines (CR LF), one name per slot, read with `ReadLn`. A name written by the save screen is 24 characters: a space, up to 20 typed characters (a–z, 0–9, space, `-`, `.`), then spaces. The save screen rewrites the whole file: other used slots' lines are written back as read, and empty slots' lines become the 19-character empty label, so the file is 260 bytes only while all slots are used (C-0042: 215 bytes). The shipped files hold two alternating placeholder names. [flow/save-screens](../spec/flow/save-screens.md).

### 13.4 `tdstatus.dat`

Not shipped; written at runtime. Passes state between `td1.exe` and the episode program (U-004, RESOLVED). `td1.exe` creates it with one 00 byte if it is missing and reads byte 0 after each Episode 1 run (0 or empty = quit, 1 = run `order.exe`, 2 = run `aboutus.exe`, then Episode 1 again): [spec/flow/launcher.md](../spec/flow/launcher.md) §3.2.

Static (M1-05): `file0001` start-up (`0000:45E1`) reads a 1-byte flag; if it is non-zero it reads 13 more bytes (word, byte, byte, word, word, word, word, byte; the last byte is the script choice, 1 = uncut, 2 = edited), then truncates the file to 0 bytes. `0000:3381` writes the same 14-byte record. The fields are resolved in [spec/flow/episode-program-flow.md](../spec/flow/episode-program-flow.md) §4.6 (RE-2).

## 14. Creative Music File (`.cmf`), standard format

| | |
|---|---|
| Confidence | **CONFIRMED** (header parse of all 17 files) |
| Magic | `CTMF`, version `0x0101` |

Header fields used:

| Offset | Size | Field |
|---:|---:|---|
| 0x00 | 4 | `CTMF` |
| 0x04 | 2 | Version (0x0101) |
| 0x06 | 2 | Instrument block offset (40 in all files) |
| 0x08 | 2 | Music data offset |
| 0x0A | 2 | Timing field A (*ticks per quarter note* in common documentation): **varies 23..192** |
| 0x0C | 2 | Timing field B (*ticks per second* in common documentation): **96 in every file** |
| 0x0E–0x13 | 6 | Title, composer and remarks offsets |
| 0x14 | 16 | Channel-in-use flags |
| 0x24 | 2 | Instrument count |
| 0x26 | 2 | Basic tempo |

Each instrument is 16 bytes of OPL2 register values. Music data is a MIDI-like event stream (delta times, running status, CMF-specific controllers 0x63–0x69).

| File | Instruments | Music offset | Field A (0x0A) | Field B (0x0C) |
|---|---:|---:|---:|---:|
| climax | 10 | 200 | 23 | 96 |
| death | 9 | 184 | 58 | 96 |
| hangar | 12 | 232 | 52 | 96 |
| intro | 9 | 184 | 48 | 96 |
| intro1 | 9 | 184 | 74 | 96 |
| intro2 | 9 | 184 | 58 | 96 |
| low | 9 | 184 | 58 | 96 |
| menu | 9 | 184 | 58 | 96 |
| mood | 10 | 200 | 52 | 96 |
| mood2 | 9 | 184 | 52 | 96 |
| mood3 | 9 | 184 | 52 | 96 |
| moodmaj | 3 | 88 | 48 | 96 |
| satair | 9 | 184 | 48 | 96 |
| td01 | 13 | 248 | 192 | 96 |
| td02 | 16 | 296 | 41 | 96 |
| td03 | 10 | 200 | 30 | 96 |
| vulture | 8 | 168 | 48 | 96 |

`mood2.cmf` and `mood3.cmf` are **byte-identical** (same SHA-256). They are presumably referenced separately by the code; keep both logical names.

**Playback timer (static, M1-06, 2026-11-02; U-007 still open):** when a song starts, WORX (`file0001:2C37:4791`) stores one header word to a variable (CS:3062), advances to the next word, and programs PIT channel 0 to `1234DCh div <that word>`. If the words are 0x0A then 0x0C (the likely reading), the timer runs at the *ticks-per-second* field (0x0C, 96 Hz in every shipped file) and field 0x0A feeds the tempo calculation at `2C37:48BB`. The divisor also changes the effective BIOS tick and so the game's tick rate ([timing.md](../spec/core/timing.md)).

Which field actually sets playback speed in WORX is **U-007**. Implement both interpretations behind a switch and compare durations against DOSBox captures.

## 15. Creative Voice File (`.voc`), standard format

| | |
|---|---|
| Confidence | **CONFIRMED** (header and first block of all 17 files) |
| Magic | `Creative Voice File` + `0x1A`; header length at offset 0x14 |
| Data | Block 1 (sound data): 3-byte length, time constant `tc`, codec byte. Rate = `1,000,000 / (256 − tc)` Hz. |
| Codecs | 0 = 8-bit unsigned PCM · 1 = 4-bit Creative ADPCM · 2 = 2.6-bit (3-bit) Creative ADPCM · 3 = 2-bit Creative ADPCM |

| File | Rate (Hz) | Codec | Block length |
|---|---:|---|---:|
| afterb | 6,024 | 4-bit ADPCM | 4,161 |
| boomboom | 6,250 | 4-bit ADPCM | 5,499 |
| home | 8,000 | 4-bit ADPCM | 16,118 |
| hornet | 4,000 | PCM | 4,411 |
| income | 8,000 | 4-bit ADPCM | 5,575 |
| missile | 6,410 | 4-bit ADPCM | 3,766 |
| poink | 4,000 | PCM | 4,411 |
| pulse | 4,000 | PCM | 899 |
| punyexp | 6,993 | 2.6-bit ADPCM | 1,144 |
| ric2 | 8,000 | PCM | 23,943 |
| ric3 | 12,048 | 4-bit ADPCM | 16,766 |
| scratch | 8,000 | 4-bit ADPCM | 597 |
| stiletto | 8,000 | 2-bit ADPCM | 723 |
| vii | 8,000 | 4-bit ADPCM | 1,462 |
| vulturei | 6,024 | 2-bit ADPCM | 333 |
| whoosh | 4,000 | PCM | 17,691 |
| zeus | 8,000 | 2.6-bit ADPCM | 1,829 |

Only the first block was examined. Check for further blocks (silence, repeat, markers) in M3-01. `hornet.voc` and `poink.voc` have identical sizes but different contents (different SHA-256).

Usage from the filename scan (FILE_INVENTORY "Referenced by"): `home`, `ric3`, `whoosh`, `scratch` and `pulse` are referenced only by the intro. `ric2` and `poink` are not referenced by name in any exe (built names, or unused). Everything else is referenced by all three episodes.

Probable meanings (HYPOTHESIS, to confirm from call sites in M1-02): `afterb` afterburner, `boomboom` big explosion, `punyexp` small explosion, `missile` missile launch, `pulse`/`vii`/`vulturei`/`stiletto`/`hornet`/`zeus` weapon sounds per ship type, `income` incoming transmission, `ric2`/`ric3` ricochets (or intro voice), `whoosh`/`scratch`/`pulse` intro effects, `poink` hit, `home` homing or homecoming.

## 16. TFP presentation (`aboutus.tfp`, `order.tfp`)

| | |
|---|---|
| Confidence | PARTIAL; out of scope unless D4 changes |
| Magic | `04 'T' 'F' 'P' 1A` |
| Content | Contains named resources (`screen`, `playback`, ...). Played by `aboutus.exe`/`order.exe`. |
