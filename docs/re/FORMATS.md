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
| Confidence | LIKELY |
| Size | 128,768 = 768 + 2 × 64,000 |
| Hypothesis | Palette + two 320×200 pages. The first 768 bytes are a valid 6-bit palette (max 63). |
| To do | Render both pages; confirm page order and usage (M2-09, M5-06). |

## 4. Tileset (`.blk`)

| | |
|---|---|
| Confidence | **CONFIRMED** for `td.blk`; LIKELY for `td2.blk`/`td3.blk` |
| `td.blk` | 81,920 bytes = **80 tiles × 32×32** pixels. Tile *i* is at offset `i × 1024`, stored row-major. No header, no palette (drawn with `td.pal`). |
| `td2.blk`, `td3.blk` | 82,784 bytes = 81,920 + **864 trailing bytes** of unknown purpose (U-013) |
| Evidence | Contact sheet of `td.blk` shows coherent 32×32 city tiles: building blocks, hazard-striped roofs, roads, desert, a "TD" logo, arrows. |

## 5. Map (`.map`)

| | |
|---|---|
| Confidence | **CONFIRMED** for `td.map` (rendered with `td.blk`; coherent city) · LIKELY for the others |
| Episode maps | `td.map`, `td2.map`, `td3.map`, `td4.map`: 40,401 bytes = **201 × 201** tile indices, row-major (`index = y*201 + x`), 1 byte each |
| Intro map | `intro.map`: 40,000 bytes = **200 × 200** |
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
- The E1 city is surrounded by desert (tile indices 15/16 alternate pseudo-randomly as desert texture). An approach road enters from the west edge.
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
| Heading construction | 16 headings, 0 = nose up, increasing counter-clockwise in 22.5° steps: `h % 4` picks `[shp, sc3, shr, sc2]`, and the result is rotated CCW by `90° × (h / 4)` with exact pixel transforms. The 32×32 base is centred in a 40×40 frame (offset 4,4). Implemented in `td-render::headings`; the result is continuous through all 16 steps. |
| Still open | How the engine maps its internal angle to a frame index (U-003). |

### 6.3 Other sprite files (HYPOTHESIS)

| File | Bytes | Guess |
|---|---:|---|
| `mine.td` | 1,024 | One 32×32 sprite |
| `lander.td` | 5,120 | 5 × 32×32 |
| `td.exp` | 7,168 | 7 × 32×32 explosion frames |
| `td.hel` | 2,048 | 2 × 32×32 (helicopter / rotor frames?) |
| `td.h2` | 9,600 | Helicopter-related; 9,600 = 2 × 4,800 or 6 × 1,600 (40×40)? |

## 7. Missions (`m<N>.td`, `m<N>.td2`, `m<N>.td3`)

| | |
|---|---|
| Confidence | LIKELY (structure); field semantics PARTIAL |
| Files | E1 `m1..m20.td`, E2 `m1..m20.td2`, E3 `m1..m19.td3` + `m21.td3` (60 files) |
| Size | 20,080 bytes each |

Layout:

| Offset | Size | Content | Evidence |
|---:|---:|---|---|
| 0 | 40 | Reserved: zero in 59 of 60 files (`m18.td3` has `0x4C` at offset 10; U-014) | All files scanned |
| 40 | 20 | **Slot class code** per ship slot 0..19: values 0..8, `0xFF` = slot unused | All files scanned; value histogram 0:128, 1:37, 2:22, 3:29, 4:48, 5:4, 6:33, 7:243, 8:124, FF:552 |
| 60 | 20 | Reserved: zero in all files | All files scanned |
| 80 | 20 × 500 | **X path** for slot *s* at `80 + s*500`: 500 bytes of tile-grid X coordinates (1..199) | Smooth monotonic runs, values within map bounds |
| 10080 | 20 × 500 | **Y path** for slot *s* at `10080 + s*500` | Same as X |

Observations:
- Path runs are smooth (±1 per step) with plateaus, consistent with recorded or hand-drawn routes on the tile grid. Unused tail entries are 0.
- The first byte of some slots' X arrays differs from the run that follows (`0xFF`, `0x80`, `0x11`, `0x64`, `0x02`, `0x50`, ...). It may be a flag, a delay or a start marker (U-015).
- What the class codes mean is U-008. 7 and 8 are the most common; 1 appears in truck-heavy missions.
- The reserved areas may be runtime state in the original's in-memory record (the file is probably block-read straight into a record).

## 8. Dialogue scripts (`dialogue.dat`, `dialogu2.dat`, `dialogu3.dat`, `safedial.dat`, `safedia2.dat`, `safedia3.dat`)

| | |
|---|---|
| Confidence | PARTIAL |
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
d                                      a literal 'd' line follows each 04 (purpose unknown; U-016)
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
- **Scene 13 quirk:** its `06` line is the single byte `06`. The ID byte 0x0D doubles as the CR of the CR LF terminator (`06 0D 0A`); its neighbours are 12 and 14. The bytes 0x0A and 0x0D never occur inside any other line. How the original reader handles this is part of U-016; the port reads a bare `06` as ID 13.
- **Mid-segment speaker lines:** a `01` line can appear inside a segment to bring in a new speaker (for example a new character taking slot 2 part-way through a scene).
- **Speaker lines between segments** (after `04`, before the next `03`) occur in E3. They apply from the next segment on.
- **Portrait-only changes:** a 3-byte `01 slot face` line (E3: twice in `dialogu3.dat`, once in `safedia3.dat`) changes the portrait shown in a slot without changing colour or name. Face 200 appears when a character leaves, so it probably means "no portrait". Face 200 also appears in some header speaker lines.
- **Trailing spaces:** one `05` line in each E2 script carries two trailing spaces (`05 20 20`).
- No other line-leading codes occur. Every line starts with `01`–`06` or is the literal `d` after `04`. The file ends with CR LF.
- Speaker slots are only ever 1 and 2 (two speakers per scene). Portrait bytes range up to 200 (200 may mean "no portrait"; to confirm).
- **Continuation segments:** in E2 and E3, a `03` follows a `04` directly, with no `06` or `01` lines (for example around line 4,002 of `dialogu2.dat`). This is a second segment that reuses the previous scene's speakers and has no trailing `d`.
- Colour bytes can be any value, including control-range values (`0x14`, `0x16`, `0x17` ...). Parse lines positionally, not by scanning for printable text.
- The second byte of `02` lines is `01` in every one of the 21,115 lines. Its meaning (alignment? window?) is U-016.
- The speaker is identified by the **colour byte** matching a speaker definition's colour. Examples of colour bytes seen: `0x2E`, `0x39`, `0x2A` (these happen to be printable as `.`, `9`, `*`, so be careful when viewing the file as text).
- Blank padding lines (`02 01 <color> <space>`) frame each speech block.
- Edited files share the same scene structure. Line content differs.

## 9. Fonts

| File | Bytes | Finding | Confidence |
|---|---:|---|---|
| `td.lt2` | 950 | **5-pixel-wide glyphs**, stored as consecutive rows. 950 / 5 = 190 rows; probably **38 glyphs of 5×5** (26 letters + 10 digits + 2 symbols?). Pixel values `0xF7` (foreground) and `0x00`, with some `0x12` shading. | LIKELY |
| `td-let.cin` | 2,560 | **8×8 glyphs**, 40 glyphs (2,560 / 64). Large title lettering (letters, digits, punctuation). | LIKELY |
| `td.ltr` | 14,976 | Not 8 px wide (render incoherent). Geometry unknown. 14,976 = 2^7 × 117. | UNKNOWN (U-017) |

The character-to-glyph mapping, spacing and colour substitution come from the text-drawing routine (M2-08).

## 10. Portraits (`newfaces.td`, `.td2`, `.td3`)

| | |
|---|---|
| Confidence | **CONFIRMED** (decoded and rendered, 2026-11-02) |
| Layout | A sequence of records, each a **768-byte palette + 160×100 pixels** (16,768 bytes). E1: 26 faces (435,968 bytes); E2 and E3: 27 faces (452,736 bytes). No header. |
| Pixel use | Faces use palette indices 1..~112 (up to 147 in E2), each with its **own palette**. How a dialogue screen shows two faces plus a backdrop in one 256-colour palette is part of M5-04. |
| Dialogue link | Speaker face bytes 1..26/27 are 1-based portrait indices. Other values occur: 32, 33, 65, 78, 87, 89, 90, 126 and 200 (200 appears when a character leaves). These are special codes (U-019). |
| Code | `td-formats::portraits` |

## 11. Story stills (`td1-13.pic` .. `td1-20.pic`, `secret.pic`, `pic4.td`)

| | |
|---|---|
| Confidence | LIKELY (geometry from data: row-similarity peak + exact size fit, every image rendered and visually coherent). The loaders in the exe should confirm it (M1-05). |
| Layout | Optional 768-byte palette, then raw pixels. **No dimensions in the file.** |

| File | Size | Own palette | Content (brief) |
|---|---|---|---|
| `td1-13.pic` | 202×86 | yes | Two portraits |
| `td1-14.pic` | 200×200 | yes | Character dissolving (teleport) |
| `td1-15.pic` | 255×121 | yes | Shooting scene |
| `td1-16.pic` | 150×170 | yes | Character in a chair |
| `td1-17.pic` | 320×90 | **no** (draw with the current palette; correct with `td.pal`) | Cockpit/bridge scene |
| `td1-18.pic` | 170×104 | yes | Clone tube |
| `td1-19.pic` | 320×72 | **no** (correct with `td.pal`) | Ship over clouds |
| `td1-20.pic` | 320×100 | yes | Laser scene |
| `secret.pic` | 320×32 | yes | Hint banner |
| `pic4.td` | 100×150 | yes | Corridor/room |

Code: `td-formats::picture::{STILLS, Picture}`.

## 12. Raw UI and sprite graphics

Headerless 8bpp. Geometry from data analysis (row-similarity peak, exact size fit, visual check), drawn with `td.pal` unless noted. `td-formats::picture::CELL_SHEETS` holds the confirmed ones.

| File | Bytes | Geometry | Content | Confidence |
|---|---:|---|---|---|
| `td.win` | 10,240 | 320×32 | **In-flight HUD bar**: shielding/hull panel, a radar-like box, missiles display, status boxes. So the flight viewport is probably 320×168 (U-025). | LIKELY |
| `td.sss` | 6,080 | 320×19 | Save-slot bar | LIKELY |
| `td.fac` | 3,920 | 5 × 28×28 | "HIT F6" comm indicator (blank + 4 animation frames) | CONFIRMED (visual) |
| `td.joy` | 1,600 | 40×40 | Joystick icon (calibration screen) | CONFIRMED (visual) |
| `td.h2` | 9,600 | 6 × 40×40 | Helicopter rotor frames | CONFIRMED (visual) |
| `td.hel` | 2,048 | 2 × 32×32 | Helicopter bodies | CONFIRMED (visual) |
| `td.dun` | 3,072 | 3 × 32×32 | Desert/dune tiles (intro only) | CONFIRMED (visual) |
| `td1.ccm`, `td2.ccm`, `td3.ccm` | 9,216 / 7,168 / 5,120 | 9 / 7 / 5 × 32×32 | **Comm portraits** (small faces for in-flight transmissions) | CONFIRMED (visual) |
| `mine.td` | 1,024 | 32×32 | Mine | CONFIRMED (visual) |
| `lander.td` | 5,120 | 5 × 32×32 | Lander pieces (a large craft built from tiles?) | CONFIRMED (visual) |
| `td.exp` | 7,168 | 7 × 32×32 | Explosion frames | CONFIRMED (visual) |
| `td.men` | 144,000 | Mixed. The top ~380 rows at width 320 show the main menu panel, in-game options panels and confirmation boxes (quit confirmation, save/restore slot prompts). Uses `tdmen.pal`. The rest holds sub-images of other widths. | Menu graphics atlas | PARTIAL (needs blit sites, M2-09) |
| `td.cmm` | 32,768 | Unknown (no clean width) | Communication panel? | UNKNOWN |
| `td.sel` | 3,096 | Unknown | Selection UI | UNKNOWN |
| `td.wea` | 3,888 | Unknown (36×36 × 3?) | Weapon/HUD element? | HYPOTHESIS |

## 13. Configuration and saves

### 13.1 `td.cfg` (3 bytes), LIKELY

`[music, sfx, controller]`. The shipped value is `02 02 02`. Probably 1-based menu choices from the config screen: music {1 none, 2 SB/AdLib}, SFX {1 none, 2 SB, 3 PC speaker}, controller {1 keyboard, 2 keyboard + joystick}. Confirm in M2-12.

### 13.2 `td.sav`, `td2.sav`, `td3.sav` (70 bytes), HYPOTHESIS

Shipped `td.sav`: 10 × `02 0D 00`, then 40 × `0xFF`. Possibly 10 slots × 3 bytes (for example mission number / state) + a 40-byte area (10 × 4?) unused in the shipped file. The shipped `td2.sav` and `td3.sav` differ from `td.sav` and from each other; not yet examined. Real saves are needed to decode the format (create them in DOSBox).

### 13.3 `td.stt`, `td2.stt`, `td3.stt` (260 bytes), LIKELY

10 save names × 26 bytes: a space, then the name padded with spaces to 24 characters, then CR LF. The shipped files hold two alternating placeholder names.

### 13.4 `tdstatus.dat`

Not shipped; written at runtime. Presumably passes state between `td1.exe` and the episode program (U-004).

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
