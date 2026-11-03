# Asset load sites (M1-05)

For every data file that `file0001.exe` (Episode 1) opens: the routine that loads it, how it reads the file (sizes, offsets, order), where the bytes go, and what uses them. Addresses use ADG §7.8 notation (`file0001:SSSS:OOOO`, image-relative). Sizes are bytes unless marked. Confidence is per row: **CONFIRMED** means static analysis agrees with the INT 21h log of capture C-0002 (or with a decode of the file), **LIKELY** means static analysis only.

Sources: static analysis of the private Ghidra copy (RE-3, 2026-11-02); C-0002 `int21.tsv` (E1 power-on to mission 1 flight); decodes of the original files. Episode 2/3 notes are at the end (§7).

## 1. How the game reads files

- Everything goes through the BP7 System unit: `Assign` (`3A9A:0991`), `Reset(F, 1)` (`3A9A:09CC`, record size 1), `BlockRead` (`3A9A:0AB7`), `Seek` (`3A9A:0B1F`), `Close` (`3A9A:0A4D`). Text files (`dialogue.dat`, `safedial.dat`, `td.stt`) use the text-file routines (`Assign`/`Reset` at `3A9A:0549`/`05C7`, `Read(ch)`, `Read(s)`, `ReadLn`).
- **Correction to the symbol table:** `3A9A:19D7` is `FilePos` and `3A9A:19EE` is `FileSize` (they were named the other way round). The helper `3A9A:1A1E` returns the current position in DX:AX and the size in BX:CX; `19D7` returns DX:AX, `19EE` returns BX:CX.
- Almost every loader runs with `{$I+}` (`BP_IOCheck` after every call), so a missing or short file is a runtime error (error 2 or 100), not a recoverable condition. Exceptions: `tdstatus.dat`, `td.cfg` and `dialogue.dat` are probed with `IOResult` first (§2).
- Graphics are not kept in RAM: most loaders read a row (or a byte) at a time and plot it straight into **VGA memory** with the plane-select pixel writer `386E:1A97` (`Gfx_PutPixel(x, y, colour)`). Off-screen VGA memory doubles as the sprite/tile/menu atlas.
- Palettes go into one of three 768-byte RAM buffers and reach the DAC later through a fade: **`g_Palette` DS:0D2E** (scenes, dialogue, story cards), **`g_MenuPalette` DS:07E0** (main menu, help), **`g_FlightPalette` DS:13E6** (flight).

### 1.1 VGA memory layout (unchained mode 13h, 352-pixel rows)

`Gfx_InitUnchained` (`386E:19EF`) clears video memory, sets mode 13h, sets all 256 DAC entries to black, turns chain-4 off and sets the CRTC offset to 44 words, so **every VGA row is 88 bytes = 352 pixels** (320 visible). A pixel (x, y) is at address `y × 88 + x / 4`, plane `x mod 4`. The rest of the engine assumes this pitch (`1A97`, `0269`, `191C`, `189C`).

| VGA rows | Address | Use in flight | Use in menus and scenes |
|---|---|---|---|
| 0..31 | 0000..0AFF | HUD bar (`td.win`), shown under the split line | Visible screen (rows 0..199) |
| 32..263 | 0B00..5ABF | Playfield page 0 (352 × 232) | |
| 200..399 | 44C0..897F | (inside the pages) | Second screen (`javelin.td`, `city.td`, help page) |
| 264..495 | 5AC0..AA7F | Playfield page 1 (352 × 232) | |
| 400..619 | 8980..D4FF | | Menu atlas copied from `td.men` (§3, `td.men`) |
| 496..729 | AA80..FA7F | Tileset (`td.blk`), 256 bytes per tile | |

## 2. Start-up and configuration

| File | Routine | Reads | Destination | Consumer | Conf. |
|---|---|---|---|---|---|
| `tdstatus.dat` | main body `0000:45E1` | Probe with `Reset` + `IOResult`; if missing, create it and write one zero byte. If size > 0: 1 byte → DS:0752 (valid flag); if non-zero, 13 more bytes: 2 → DS:0D2A, 1 → DS:0D29, 1 → DS:0754, 2 → DS:1F5C, 2 → DS:1F5A, 2 → DS:1F58, 2 → DS:1F56, 1 → DS:0D28 (script choice). Then `Rewrite` (truncates the file to 0 bytes). | Globals | Resumes state after a round trip through `td1.exe`; `0000:3381` writes the same 14-byte record (U-004) | CONFIRMED (C-0002: 0/1/0-byte opens) |
| `td.cfg` | main body `0000:49C1` | Probe; then 3 × 1 byte → DS:12C6, DS:12C7, DS:12C8. Written back (3 bytes) by the configuration screen at `0000:5044`. | `g_CfgMusic` (2 = CMF music on; tested by `0E8B:07ED`), `g_CfgSfx`, `g_CfgController` (2 = joystick; tested before every joystick read) | Config screen, sound start-up, input | CONFIRMED |
| `dialogue.dat` | main body `0000:5100` | Existence probe only (`Reset`, `Close`, `IOResult`) | — | If present: script-choice screen `051A:527F`; if absent: DS:0D28 := 2 (edited script) and the notice screen `051A:56D5` | CONFIRMED |

## 3. Menus, help, saves

| File | Routine | Reads | Destination | Consumer | Conf. |
|---|---|---|---|---|---|
| `td.men` | `0000:2E88` (main menu set-up) | Seek 42,880: 32 rows × 320 → VGA rows 400..431. Seek 80,000: 128 rows × 320 → VGA rows 432..559; then 20 rows × 100 → (x 0..99, rows 560..579); 20 × 100 → (x 100..199, rows 560..579); 40 × 100 → (x 0..99, rows 580..619). Reopen, seek 16,000: 200 rows × 320 → VGA rows 0..199 (the visible menu). | VGA (on- and off-screen) | Menu loop `0000:3381`; boxes copied by `0000:330E` | CONFIRMED (C-0002: 59,200 bytes in 240 reads + 64,000 in 200 reads) |
| `td.men` | `0E8B:794C` (in-flight computer menu) | From offset 0: 100 rows × 160 → page-relative (80..239, 50..149) of the current flight page | VGA | Pause menu; palette entries 107..110 are the four item colours | LIKELY |
| `tdmen.pal` | `0000:2E88`, `0000:3381` | 768 → `g_MenuPalette` | RAM | Menu fades | CONFIRMED |
| `td.hlp` | `0000:02AF` (from the menu) | 768 → `g_MenuPalette`; 200 rows × 320 → VGA rows 200..399, shown (start address 44C0h), wait for a key; second 200 rows → same place, shown | VGA | Help, two pages | CONFIRMED (static); file decode agrees |
| `td.hlp` | `0E8B:6EDF` (in flight) | 768 → `g_FlightPalette`; page 1 and page 2 as above into the current flight page | VGA | In-flight help | LIKELY |
| `td.stt` | `051A:76EA` (restore), `051A:7EEA` (save) | Text file, 10 lines → 10 slot names | Stack | Save/restore screen; `7EEA` rewrites it | LIKELY |
| `td.sav` | `051A:76EA`, `051A:7EEA` | For slot *s* = 1..10: seek (*s* − 1) × 3, read 1 byte (mission number) + 2 bytes (word). A mission byte of 0 or > 20 marks the slot empty. Bytes 30..69 are never read. `7EEA` writes the 3 bytes of the chosen slot. | Stack | Save/restore screen | LIKELY |
| `td.sss` | `051A:76EA`, `051A:7EEA` | 19 rows × 320, one byte per read; every pixel is plotted 10 times, at rows 10 + 19·k (k = 0..9) | VGA | Ten slot bars stacked from row 10 to 199 | LIKELY |

## 4. Story, scenes and dialogue

All scene routines live in unit `051A`. Unless stated otherwise each one: calls `Gfx_InitUnchained`, start address 0, split line off (400); loads `td.lt2` (950 bytes into a 975-byte heap block, `g_Font5` far pointer at DS:9770); loads the image; draws caption lines with `Txt_DrawString` (`051A:0B9F`, [text.md](../spec/render/text.md)); fades in (`051A:0016`), waits (`051A:1AA3`, seconds or a key), fades out (`051A:013F`).

| File | Routine | Reads | Drawn at | Wait | Conf. |
|---|---|---|---|---|---|
| `td.lt2` | every text screen (about 30 sites); `051A:1DA1` is a one-shot "load, draw one string, free" helper | 950 bytes → 975-byte block (glyph 38 stays uninitialised) | — | — | CONFIRMED (C-0002: 950 bytes, 1 read, many times) |
| `*.scr` scenes: `hangar`, `vlab`, `intro2`, `shooting`, `vhang`, `marauder`, `briefing`, `tdlab`, `velsroom`, `helihangar` (→ `helihang.scr`), `eggbase`, `soffice`, `sbar`, `voffice` | `051A:1E5A`, `2055`, `22BE`, `248A`, `3109`, `3371`, `35B0`, `380C`, `3A6B`, `3CC0`, `3E8E`, `4A66`, `4D66`, `596F` | 768 → `g_Palette`; 200 rows × 320, each row plotted at **x = 1..320** (shifted right one pixel) | Full screen, rows 0..199; screen column 0 stays index 0 | 10 s (`eggbase` 15 s) | CONFIRMED for `hangar.scr` (C-0002: 64,768 bytes in 201 reads; frame 1630 equals the file shifted right by one pixel on every compared pixel, column 0 black) |
| `javelin.td`, `city.td` | `051A:5EE9`, `051A:5FC7` | 768 → `g_Palette`; start address 44C0h; 200 rows × 320 → VGA rows 200..399, x 0..319 | Second screen | caller | LIKELY |
| `secret.pic` | `051A:2B26` (hint screens) | 768 → `g_Palette`; 32 rows × 320 at x = 1..320, rows 0..31 | Banner; hint text typed below it | key | CONFIRMED (C-0002: 11,008 bytes in 33 reads) |
| `td1-16.pic` | `051A:4113` variant 1 | 768 → `g_Palette`; 170 rows × 150 | x 86..235, y 0..169 | 25 s | LIKELY |
| `td1-14.pic` | `051A:4113` variant 3 | 768 → `g_Palette`; 200 rows × 200 | x 1..200, y 0..199 | 25 s | LIKELY |
| `td1-13.pic` | `051A:4113` variant 4 | 768 → `g_Palette`; 86 rows × 202 | x 44..245, y 39..124 | 25 s | LIKELY |
| `td1-20.pic` | `051A:4113` variant 2 | Skips 768 bytes (read one at a time and discarded: **the file's palette is not used**; `td.pal`, loaded first, stays); 100 rows × 320 | x 0..319, y 50..149 | 25 s | LIKELY |
| `td.pal` | `051A:4113` (before the variants) and the text screens `62FD`, `6653`, `6A33`, `6FF2`, `74B2` | 768 → `g_Palette` | — | — | CONFIRMED |
| `td1-18.pic` | `051A:471B` | 768 → `g_Palette`; 104 rows × 170 | x 76..245, y 20..123 | 15 s | LIKELY |
| `td1-15.pic` | `051A:4979` | 768 → `g_Palette`; 121 rows × 255 | x 32..286, y 30..150 | 10 s | LIKELY |
| `pic4.td` | `051A:4F5A` | 768 → `g_Palette`; 150 rows × 100 | x 11..110, y 16..165 | 10 s | LIKELY |
| `td-let.cin` | `051A:527F` (script choice), `051A:5852` (mission title card) | **2,496 bytes = 39 glyphs** into a 2,560-byte block (the file's 40th glyph, `:`, is never loaded) | — | `5852`: 5 s | CONFIRMED (C-0002: 2,496 bytes, 1 read) |
| `newfaces.td` | `051A:1618` (`Dlg_LoadPortrait`) | Seek (face − 1) × 16,768 (done as face − 1 relative seeks); 768 → stack; 16,000 → 16,000-byte heap block | Left (x 0..159) or right (x 160..319) half of rows 0..99; palette entries 0..111 or 112..223 ([dialogue-player.md](../spec/narrative/dialogue-player.md)) | — | CONFIRMED (C-0002: 16,768 bytes in 2 reads per face) |
| `dialogue.dat` / `safedial.dat` | `051A:92AF` (`Dlg_PlayScene`) | Text file. Chosen by DS:0D28 (1 = uncut, 2 = edited). Read from the start, line by line, until `06 <id>` matches; then played until a line starting with `d` | — | — | CONFIRMED (C-0002: text-file reads of 128 bytes) |

Text-only screens (`td.lt2` + `td.pal`, no image file): assignment orders `051A:62FD`, enemy dossier `051A:6653` (portrait 6 on the right), retry prompt `051A:6A33`, debriefing `051A:6FF2` (portrait 1 on the right), player dossier `051A:74B2` (portrait 1 on the right), background story `051A:8EAA`, script-choice warning `051A:527F` (with `td-let.cin`), missing-dialogue notice `051A:56D5`.

## 5. Flight (`Sim_FlyMission`, `0E8B:B880`, once per mission)

Heap blocks are allocated first (sizes in parentheses). Then, in this order:

| # | File | Reads | Destination | Notes | Conf. |
|---|---|---|---|---|---|
| 1 | `td.pal` | 768 | `g_FlightPalette` DS:13E6 | Sent to the DAC immediately, again after set-up | CONFIRMED |
| 2 | `td.sc3` | **14,400 = cells 0..8 only** | block DS:9774 (14,400) | 22.5° rotation set for sprites 0..8 | CONFIRMED (C-0002: 14,400) |
| 3 | `td.lt2` | 950 | `g_Font5` (975) | HUD and message text | CONFIRMED |
| 4 | `td.sc2` | 14,400 = cells 0..8 | block DS:9778 (14,400) | 67.5° set for sprites 0..8 | CONFIRMED |
| 5 | `td.fac` | 3,920 = 5 × 28 × 28 | block DS:9B86 (3,920) | After loading, colour 38 is replaced by 39 in frames 0..3 | CONFIRMED |
| 6 | `td.shp` | 20,480 = sprites 0..19 | block DS:9B82 (27,648 = 27 × 1,024) | Sprite bank slots 0..19 | CONFIRMED |
| 7 | `td.exp` | 7,168 = 7 sprites | DS:9B82 + 20,480 | Sprite bank slots 20..26 | CONFIRMED |
| 8 | `td.wea` | 3,888 = 6 × 648 | block DS:18BE (3,888) | Weapon icons 36 × 18 for the HUD ([hud.md](../spec/render/hud.md)) | CONFIRMED |
| 9 | `td.shr` | 32,000 = cells 0..19 | block DS:977C (32,000) | 45° set for sprites 0..19 | CONFIRMED (C-0002: 32,000) |
| 10 | `mine.td` | 1,024 | sprite slot 3 | **Mission 7 only** | LIKELY |
| 11 | `lander.td` | 2,048 → slots 0..1; 3,072 → slots 4..6 | sprite bank | **Mission 20 only** | LIKELY |
| 12 | `td.hel` | 2,048 → slots 2..3 | sprite bank | **Missions 12, 15, 21 only** | LIKELY |
| 13 | `td.h2` | 3 × 3,200: → `td.sc2` block cells 2..3, `td.shr` block cells 2..3, `td.sc3` block cells 2..3 | rotation sets | Missions 12, 15, 21: pre-rotated helicopter frames for slots 2..3 (67.5°, 45°, 22.5° in that order) | LIKELY |
| 14 | `m<n>.td` | 20 → DS:3A52, 20 → DS:3A8E, 20 → DS:3A7A (slot class codes), 20 → DS:3A66, 10,000 → X-path block (DS:3BEC + 1), 10,000 → Y-path block (DS:3BF0 + 1) | globals and two 10,020-byte blocks (20 × 501) | Name built as `m` + decimal *n* + `.td`. **Not loaded for mission 21.** The path blocks are indexed with a **stride of 501 per slot**, not 500 ([FORMATS §7](FORMATS.md#7-missions-mntd-mntd2-mntd3)). After loading, `FFh` bytes in slot 2's path (block indices 502..1001) are set to 0 | CONFIRMED (C-0002: 20,080 bytes in 6 reads) |
| 15 | `td.map` | 40,401 | far block DS:442E | City map, **column-major**: tile (x, y) is byte `x × 201 + y`. The map overview `0E8B:A714` plots byte `x·201 + y` at screen (x, 63 + y); the radar `0E8B:8E87` reads `(px + i)·201 + (py + j)` around the player. Tile → radar colour table at DS:0264 | CONFIRMED (static + oracle frame match on main, 5fcb0df) |
| 16 | `td.blk` | `FileSize` / 352 rounded, minus 2, plus 1 rows of 352 bytes (E1: **232 rows = 81,664 bytes; the last 256 bytes are never read**) | VGA rows 496.., i.e. tile *t* at VGA address AA80h + 256·*t* | Tile blitter `386E:191C` copies 8 bytes × 32 rows per tile with latched writes | CONFIRMED (C-0002: 81,664 bytes in 232 reads, 3 seeks for `FileSize`) |
| 17 | `td.win` | 10,240, **one byte per read** | VGA rows 0..31 | HUD bar, displayed below the split line | CONFIRMED (C-0002: 10,240 reads) |

Loaded later, during flight:

| File | Routine | Reads | Destination / use | Conf. |
|---|---|---|---|---|
| `td.cmm` | `0E8B:5435` (F6 transmission screen) | 32 × 1,024 (4 rows × 8 columns of 32 × 32 cells) through sprite slot 0 | Drawn as a 256 × 128 panel at page-relative (16.., 32..) | LIKELY |
| `td1.ccm` | `0E8B:51FE`, `0E8B:530B` | Seek *i* × 1,024, read 1,024 (comm portrait *i*) into sprite slot 0, draw; then re-read the first 1,024 bytes of `td.shp` to restore slot 0 | Two 32 × 32 portraits on the transmission panel | LIKELY |
| `td.shp` | `0E8B:7374` (ship library screen) | 9 × 1,024 (sprites 0..8) one at a time into slot 0, drawn at fixed positions; then slot 0 is replaced by a generated pattern; finally the first 1,024 bytes are re-read | Ship library | LIKELY |
| `td1-17.pic` | `0E8B:9860` (mission event handler) | 90 rows × 320, one byte per read, no palette (current `td.pal`) | Page rows 67..156 | LIKELY |
| `td.joy` | `0E8B:AF1F` (joystick calibration) | 40 rows × 40, one byte per read | (140..179, 70..109) | LIKELY |

## 6. Sound

| Files | Routine | Reads | Notes | Conf. |
|---|---|---|---|---|
| 14 `.CMF` songs | `0E8B:07ED` (`Snd_PlayMusic(cue)`) → WORX `2C37:0273` (size via `2C37:009F`, whole file into one heap block via `2C37:0066`) | Whole file | Only when `g_CfgMusic` = 2. Cue → file: 1 `td01`, 2 `td02`, 3 `hangar`, 4 `satair`, 5 `vulture`, 6 `mood`, 7 `menu`, 8 `low`, 9 `climax`, 10 `mood2`, 11 `moodmaj`, 12 `death`, 13 `mood3`, 14 `td03` (names at `0E8B:0761..07E4`) | CONFIRMED (C-0002: whole-file reads of `menu`, `hangar`, `low`, `td01`) |
| 10 `.VOC` effects | `1E2A:06C6` (sound start-up; mode 2 = Sound Blaster) → WORX `2C37:010A` | Whole file each | `missile`, `hornet`, `afterb`, `income`, `boomboom`, `zeus`, `vii`, `vulturei`, `stiletto`, `punyexp` (names at `1E2A:0655..06BA`). Mode 3 (PC speaker) installs an INT 1Ch handler instead. Freed by `1E2A:0810` | CONFIRMED (C-0002) |

## 7. Files that file0001 never opens

`td.ltr`, `td.sel`, `td.dun` (intro only), `td1-19.pic`, `td2.*`/`td3.*` data, `ric2.voc`, `poink.voc`, `home.voc`, `ric3.voc`, `whoosh.voc`, `pulse.voc`, `scratch.voc`, `intro*.cmf`, `intro*.scr` except `intro2.scr`. `td.hlp` and `td.sav`/`td.stt` are E1 names; E2/E3 use their own (`td2.sav`, `td3.stt`, ...).

## 8. Episodes 2 and 3

- **Map and tileset choice (U-006).** `td2.exe` flight set-up (`td2:0E96:A76E`) loads `td.map` + `td.blk` when the byte DS:1302 is 1 and `td2.map` + `td2.blk` when it is 2. `td3.exe` (`td3:0C5D:A9CC`) loads `td3.map` when DS:12FE is 1, but `td4.map` whenever the mission number (DS:31DD) is greater than 6; it always loads `td3.blk`. What sets DS:1302 / DS:12FE per mission is not traced yet.
- **Tileset loader** is the same in E2/E3: rows of 352 bytes, `Round(FileSize / 352) − 2 + 1` of them. For the 82,784-byte `td2.blk`/`td3.blk` that is **234 rows = 82,368 bytes**: all 80 tiles plus the first 448 of the 864 trailing bytes go to VGA rows 496..729; the last 416 bytes are never read. No map uses a tile index above 70, so the trailing bytes are never drawn (U-013).
- **Dialogue** (`td2:04EC:96A4`, `td3:04A0:7795`): same player as E1, with different face-code remaps ([dialogue-player.md](../spec/narrative/dialogue-player.md) §3.3).
- The other E2/E3 loaders have not been traced yet (follow-up for M1-09).

## 9. Revision history

| Date | Change |
|---|---|
| 2026-11-02 | First version (RE-3, M1-05): all E1 load sites, VGA memory layout, FilePos/FileSize correction |
