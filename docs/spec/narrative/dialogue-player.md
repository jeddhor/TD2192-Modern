# Dialogue player

| | |
|---|---|
| **Spec ID** | `narrative/dialogue-player` |
| **Revision** | 1 |
| **Status** | Draft |
| **Fidelity tier** | T0 for layout, palette and text; T1 for event order; T2 for fade and typing speed (CPU-bound in the original) |
| **Confidence** | CONFIRMED for E1 (static analysis of `file0001:051A:92AF` and helpers; three C-0002 frames checked). E2/E3: LIKELY (same routine, differences listed in §4.9) |
| **Tasks** | M5-04 (prerequisites), M2-10, M2-07 |
| **Unknowns** | U-016, U-019 (both resolved here), U-062, U-063 |

## 1. Summary

A dialogue scene is a black screen with two 160×100 portraits side by side at the top, each speaker's name under the portrait, and a scrolling text area below. There is **no backdrop image**. The two portraits share the 256-colour palette by splitting it: the left face uses entries 0..111, the right face entries 112..223, and each speaker's text colour gets its own entry from 240..253. While a line is typed, the speaker's portrait is at full brightness and the other at half brightness. Lines are typed one character at a time at the bottom of the text area and the area scrolls up 7 pixels per line. A `05` line waits for a key (Esc skips the rest of the scene). The scene fades in at `03`, fades out at `04`, and the reader stops at the line that starts with `d`.

## 2. Definitions and units

| Term / quantity | Type in original | Unit | Range | Notes |
|---|---|---|---|---|
| Scene ID | Byte | — | 1..157 (E1) | Second byte of a `06` line |
| Slot | Byte | — | 1, 2 | 1 = left, 2 = right |
| Face code | Byte | — | 1..27, special codes, 200 | Portrait record number (1-based) after the remap of §3.3 |
| Colour byte | Byte | `td.pal` index | 0..255 | Identifies the speaker in `02` lines and selects the text RGB |
| Text entry | Byte | palette index | 240..253 (E1) | Palette entry allocated for a speaker's colour (§4.4) |
| Lit flag `g_ScreenLit` | Byte | — | 0/1 | DS:102E. 1 after a fade-in, 0 after a fade-out. DAC writes made while it is 0 are deferred to the next fade-in |

## 3. Data

### 3.1 Script lines (see [FORMATS §8](../../re/FORMATS.md#8-dialogue-scripts-dialoguedat-dialogu2dat-dialogu3dat-safedialdat-safedia2dat-safedia3dat))

The reader takes the **first byte** of each line as the code (`Read(ch)`), reads any fields as further single characters, reads the rest as a string, then skips to the next line (`ReadLn`).

| Code | Fields read | Action |
|---|---|---|
| `06` | scene ID | (only during the search) |
| `01` | slot, face, colour, name (rest of line) | Speaker definition (§4.4) |
| `02` | second byte (**read and ignored**), colour, text | Show a line (§4.5) |
| `03` | — | Start of a segment: fade in (§4.6) |
| `04` | — | End of a segment: fade out |
| `05` | — | Page break: wait for a key (§4.7) |
| `d` | — | **End of scene**: stop reading (U-016) |

### 3.2 Screen layout

| Element | Position (pixels) | Notes |
|---|---|---|
| Left portrait | x 0..159, y 0..99 | Pixels as stored (0..111) |
| Right portrait | x 160..319, y 0..99 | Pixels + 112 |
| Name, slot *s* | x0 = (*s* − 1) × 160 + 79 − (length div 2) × 5, text pen (x0 + 1, 89), drawn over the portrait | Black copies first at (x0, 90), (x0, 89), (x0 + 1, 90); then the name in the speaker's text entry. Small font ([text.md](../render/text.md)) |
| Text area | rows 101..189, full width (80 VGA bytes = 320 px) | Row 100 is never touched |
| New line | typed with pen (2, 175), so glyph rows 176..180 and first inked column 4 | Then the area scrolls up 7 px |
| Prompt (scene 1 only, E1) | pen (1, 191), entry 254 = (32, 0, 0), black copies at x 0 and 2 | Drawn at every `03` of scene 1 |

### 3.3 Face codes

Before a portrait is loaded the face byte is remapped (in this order; first matching rule wins only where noted):

| Episode | Remap | Provenance |
|---|---|---|
| E1 | `N` (78) → 13, `A` (65) → 10, `B` (66) → 14, space (32) → 9 | `file0001:051A:92AF` |
| E2 | `N` (78) → 13, `!` (33) → 10, space (32) → 9 | `td2:04EC:96A4` |
| E3 | applied in sequence: 27 → 13, then 28 → 27, `!` (33) → 27, `Y` (89) → 16, `Z` (90) → 17, `W` (87) → 18, `~` (126) → 10, space (32) → 9 | `td3:04A0:7795` |

The codes stand in for face numbers that are awkward bytes in a text line (9 = TAB, 10 = LF, 13 = CR, 26 = end-of-file marker ...). **Face 200** means "load no portrait" (§4.4). Codes seen in the shipped scripts: E1 32, 65, 78, 200; E2 32, 33, 78, 200; E3 32, 33, 87, 89, 90, 126, 200. All of them are covered.

### 3.4 Constants

| Constant | Value | Provenance |
|---|---|---|
| Palette entries per face | 112 (0..111 left, 112..223 right) | `051A:1618` |
| Pixel values kept per face | 0..111 (larger values become 0) | `051A:1618` |
| Dim factor | each 6-bit component shifted right by 1 | `051A:1BF7` |
| First text entry | 250 (FAh); next entries 251, 252, 253, then 240..253 cyclically (E1; E2/E3 also use 254) | `051A:92AF` |
| Line pitch | 7 px (7 single-row scrolls per line) | `051A:1BF7` |
| Typed line pen | (2, 175) | `051A:1BF7` |
| Fade | 17 steps, value × *k* / 16, *k* = 0..16 (Real48, `Round`) | `051A:0016`, `051A:013F` |

## 4. Behaviour

### 4.1 Entry

`Dlg_PlayScene(id)` (`file0001:051A:92AF`) is called with a scene ID. The script file is `dialogue.dat` when the script choice (DS:0D28) is 1 and `safedial.dat` when it is 2.

### 4.2 Set-up and scene search

1. Allocate the small-font block, load `td.lt2`, and set its background byte to 0 (black).
2. Load `td.pal` into a private 768-byte buffer (used only for speaker colours).
3. Set the next text entry to 250 and clear the skip flag.
4. Open the script and read lines from the top: for each line read the code; if it is `06`, read the ID byte; then skip to the next line. Stop after the `06` line whose ID equals `id`. (For scene 13 the ID byte is the CR of the line ending, value 13: this works.)
5. `Gfx_InitUnchained`: clear video memory, set all DAC entries to black; split line off; display start 0.

### 4.3 Main loop

Read lines until one whose first byte is `d`, dispatching on the code (table §3.1). A `06` line met inside the loop only consumes its ID byte. Lines with other first bytes are ignored. When the loop ends (or Esc breaks it, §4.7), close the file and free the font.

### 4.4 Speaker definition (`01`)

1. Read slot, face and colour bytes and the name.
2. Remember the colour byte as that slot's **speaker colour** (slot 1 or "anything else" = slot 2).
3. Remap the face (§3.3).
4. Allocate a text entry *e* (the next entry, §3.4); copy `td.pal[colour]` into `g_Palette[e]`; record "colour byte → *e*". If the screen is lit, write *e* to the DAC now.
5. If face ≠ 200, **load the portrait** (`Dlg_LoadPortrait`, `051A:1618`) into side = slot − 1:
   1. read record face − 1 of `newfaces.td`/`.td2`/`.td3` (768-byte palette + 160×100 pixels);
   2. copy palette entries 0..111 of the record into `g_Palette` entries 0..111 (left) or 112..223 (right);
   3. replace every pixel value above 111 by 0; for the right side add 112 to every pixel;
   4. if the screen is lit, write DAC entries **0..223 at half brightness** (both faces dim);
   5. blit the 160×100 image to its half of rows 0..99;
   6. draw the name (§3.2) in entry *e*.

With face 200 nothing is drawn: at the start of a scene that half stays black and nameless; later in a scene the previous portrait and name stay on screen.

### 4.5 Line (`02`)

1. Read and ignore the second byte; read the colour byte and the text.
2. If the colour byte equals slot 1's speaker colour, the speaker is slot 1 and the ink is the entry mapped to that colour byte. **Otherwise** the speaker is slot 2 and (E1) the ink is the entry mapped to **slot 2's** colour byte. (E2/E3: the line's own mapped entry; §4.9.)
3. Highlight: write the speaker's face entries at full brightness and the other face's entries at half brightness (left face entries 1..111, right face entries 112..223; entry 0 is not touched). This happens at once, whatever the lit flag.
4. Type the text with `Txt_TypeString` at pen (2, 175) in the ink ([text.md §4.3](../render/text.md#43-typed-text-txt_typestring-file0001051a1269-flight-0e8b4c9f)).
5. Scroll the text area up 7 times by one pixel row: each step copies rows 102..189 one row up (rows 101..188), so row 189 is duplicated and row 100 never changes.

Padding lines (`02 01 <colour> ` with a single space) are typed like any other line, so each one costs a blank 7-pixel line. With the usual block (blank, text, blank) the finished text line sits at rows 162..166.

### 4.6 Segment start and end (`03`, `04`)

- `03`: in E1 scene 1 only, draw the prompt (§3.2). Then **fade in** (`Gfx_FadeInScene`, `051A:0016`): snapshot `g_Palette`; for *k* = 0..16: wait for the vertical-retrace bit, then write DAC entries 1..255 as Round(component × *k* / 16) each. Set the lit flag. Both faces are at full brightness after the fade, until the first line.
- `04`: **fade out** (`051A:013F`): clear the lit flag; the same with *k* = 16 down to 0.

Video memory is not cleared between segments: a continuation segment (`03` after `04` without `d`) fades back in over the same portraits and text.

### 4.7 Page break (`05`)

Skip to the next line, then wait until a key is pressed or a joystick button is down. Read the key (extended keys: both bytes). **Esc** sets the skip flag; then the player fades out and stops reading (the rest of the scene is skipped). Any other key continues. Keys pressed while text is typing stay in the BIOS buffer and are taken by the next wait.

### 4.8 Palette summary

| Entries | Content |
|---|---|
| 0 | Left face entry 0 (black in every face checked); never dimmed |
| 1..111 | Left face palette |
| 112..223 | Right face palette (record entries 0..111) |
| 224..239 | Unchanged from the previous screen (not visible: no pixel uses them) |
| 240..253 | Speaker text entries (250, 251 for the two header speakers) |
| 254 | Prompt colour (32, 0, 0) in scene 1 |
| 255 | Unchanged |

### 4.9 Episode 2 and 3 differences

- Different face remaps (§3.3).
- The text-entry allocator wraps after 254 instead of 253 (so 250..254, then 240..254).
- An `02` line whose colour byte is not slot 1's colour is drawn in **its own** mapped entry, highlighting slot 2.
- No prompt at `03`.

## 5. Edge cases and original bugs

1. **Unmatched line colours (E1).** `dialogue.dat` and `safedial.dat` have 4 `02` lines whose colour matches neither current speaker. E1 draws them in slot 2's colour with slot 2 highlighted. Preserve (T0).
2. **Face 200 mid-scene** leaves the old portrait and name on screen (§4.4). Preserve.
3. **Portrait-only speaker lines** (`01 slot face` with no colour or name; two in `dialogu3.dat`, one in `safedia3.dat`): the colour field reads the line's CR (13), so `td.pal[13]` is allocated and that slot's speaker colour becomes 13. Depending on how BP7's `Read` treats the CR, the name read may also swallow the next (padding) line (U-063). Port policy until U-063 is answered: keep the speaker colour change, do not swallow a line, draw no name.
4. **Text area overflow:** the area is 89 rows; lines older than 12 scroll off the top. Nothing else is cleared.
5. **Scene IDs not found:** the search runs to end of file; BP7 then returns end-of-file characters forever and the search never ends (the original hangs). Port policy: treat as an empty scene (fix-all).
6. **Typing and fade speed are CPU-bound.** A fade step waits for a retrace and then needs 765 Real48 operations; at the reference preset (P1, 20,000 cycles) each step takes 2 frames, so a fade lasts about 34 frames (C-0002 frames 2186..2219). Port: 17 steps, one per 2 frames at 70 Hz (T2).

## 6. Test vectors

| # | Setup / input | Expected output | Evidence |
|---|---|---|---|
| 1 | E1 scene 1 after the first line (dispatcher, colour 39h) | DAC 1..111 = face 1 palette >> 1; 112..223 = face 2 palette entries 0..111 (full); entry 250 = td.pal[2Eh] = (39, 39, 63); 251 = td.pal[39h] = (27, 63, 39) | C-0002 f2283 |
| 2 | Same scene after the second line (Velasquez, colour 2Eh) | 1..111 full face 1, 112..223 half face 2 | C-0002 f2433 |
| 3 | Portrait pixels | Left half rows 0..99 equal face 1 pixels (values > 111 → 0); right half equals face 2 pixels + 112, except where the names are drawn (rows 89..95) | C-0002 f2283/f2433/f2573: 98.5 % / 98.7 % of portrait pixels, the rest is the name rows |
| 4 | Names | Left name pen (50, 89) in entry 250; right name pen (215, 89) in entry 251 | C-0002 f2433, 0 differences ([text.md §6](../render/text.md#6-test-vectors)) |
| 5 | Newest finished text line after a padding line | rows 162..166, first inked column 4 (12 with two leading spaces) | C-0002 f2433 |
| 6 | Fade-in | 17 groups of 255 DAC writes, about 2 frames per group at P1 | C-0002 `frames.tsv` 2186..2219 |

## 7. Presentation and audio hooks

- Palette: fades at `03`/`04`; highlight changes at every `02`; DAC writes at `01` when lit.
- No music or sound changes inside the player (the caller chooses the music).
- Input: key or joystick button at `05`; Esc ends the scene.

## 8. Open questions

- [ ] U-062: frames per typed character (and per fade step) on reference hardware.
- [ ] U-063: BP7 `Read(ch)` / `Read(s)` behaviour at a line end, for the 3-byte speaker lines of E3 (§5 item 3).
- [ ] Who calls the player with which scene ID, and with what music, belongs to the episode flow (M1-04).

## 9. Provenance

| Claim | Address / capture | Method | Confidence |
|---|---|---|---|
| Line codes, search, `d` terminator, ignored second byte | `file0001:051A:92AF` | static | CONFIRMED |
| Portrait load, palette split, pixel clamp, name placement | `051A:1618` | static + C-0002 | CONFIRMED |
| Highlight (full/half) | `051A:1BF7` | static + C-0002 | CONFIRMED |
| Typing pen and 7-px scroll | `051A:1BF7`, `051A:0269`, `051A:1269` | static + C-0002 | CONFIRMED |
| Fades | `051A:0016`, `051A:013F` (Real48 divisor 16.0) | static + C-0002 frames.tsv | CONFIRMED (speed T2) |
| Page wait, Esc | `051A:1A40` | static | CONFIRMED |
| Face remaps | `051A:92AF`, `td2:04EC:96A4`, `td3:04A0:7795` | static | CONFIRMED |
| E2/E3 differences | `td2:04EC:96A4`, `td3:04A0:7795` | static (decompiler only) | LIKELY |

## 10. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-02 | RE-3 | Initial, from static analysis of all three episodes and C-0002 |
