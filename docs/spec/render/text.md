# Text drawing

| | |
|---|---|
| **Spec ID** | `render/text` |
| **Revision** | 1 |
| **Status** | Draft |
| **Fidelity tier** | T0 (pixel-exact glyph shapes, advances and positions) |
| **Confidence** | CONFIRMED (static analysis of every text routine in `file0001`; three oracle frames matched with 0 pixel differences, §6) |
| **Tasks** | M2-08, M5-01 |
| **Unknowns** | U-037 (resolved by this spec), U-061, U-062 |

## 1. Summary

Episode 1 draws all small text (dialogue, speaker names, captions, prompts, menus, HUD messages) with the 5×5 font **`td.lt2`**, and mission titles and a few headings with the 8×8 font **`td-let.cin`**. `td.ltr` is never used. Glyphs are always **recoloured**: the colours stored in the font files never reach the screen. Text is lower case in the data; the small font maps `a`..`z` and `0`..`9` to glyphs and draws six punctuation marks (`.` `,` `"` `?` `-` `!`) as a few pixels of code. The horizontal advance is **proportional**, from a fixed table (equal to the glyph's inked width plus one pixel). There is no word wrap: every caller passes lines that are already broken, and a line that is too long simply runs off the right edge. Dialogue text is "typed" one character at a time with a block cursor.

## 2. Definitions and units

| Term / quantity | Type in original | Unit | Range | Notes |
|---|---|---|---|---|
| Pen position (x, y) | Integer | pixel | x 0..351, y 0..~750 | Top-left of the glyph cell **minus one**: glyph pixel (col *c*, row *r*), 0-based, is plotted at (x + 1 + *c*, y + 1 + *r*) |
| String | ShortString | — | 0..255 chars | Pascal string; only lower case, digits, space and the six marks are supported |
| Colour | Byte | palette index | 0..255 | The ink. What it looks like depends on the palette entry the screen sets up (§4.6) |
| Background colour | Byte | palette index | | Byte 650 of the loaded `td.lt2` block (the first pixel of glyph 26, value 178 in the file). Callers that draw opaque text overwrite it with 0 first, so in practice it is black (0) |
| Screen | VGA memory, 88 bytes per row | | 352 × 744 | Unchained mode 13h with 352-pixel rows; only x 0..319 is visible ([asset-load-sites.md](../../re/asset-load-sites.md) §1.1) |

## 3. Data

### 3.1 Small font (`td.lt2`)

38 glyphs × 5×5 bytes, row-major, 0 = empty (FORMATS §9). Loaded into a 975-byte block (room for 39 glyphs; the 39th is never written).

| Character | Glyph index |
|---|---|
| `a`..`z` (61h..7Ah) | 0..25 (character − 61h) |
| `{` (7Bh) | 26: solid block (unused by text) |
| `\|` (7Ch) | 27: solid block, used as the typing cursor |
| `0`..`9` (30h..39h) | 28..37 (character + 4Dh − 61h) |
| anything else | character − 61h, i.e. **outside the glyph table** (§5) |

### 3.2 Small-font advance table

After a character is drawn, the pen moves right by:

| Advance | Characters |
|---:|---|
| 2 | `.` `!` |
| 3 | `,` `1` `"` |
| 4 | `a b c e f i l p r s t v ? -` and space |
| 5 | `d g h j k n o q u x y z 2 3 4 5 6 7 8 9` |
| 6 | `0 m w` |
| **0** | everything else (upper case, `:` `'` `(` `)` `/` ...) |

`x` and `y` are drawn **one pixel to the left** of the pen (their glyphs have an empty first column), then advance 5. For every supported character the advance equals the glyph's inked width plus one.

### 3.3 Code-drawn punctuation (small font)

These marks are not glyphs. They are drawn in the ink colour at these cell positions (column, row; 0-based, same origin as a glyph):

| Mark | Pixels | Advance |
|---|---|---:|
| `.` | (0, 4) | 2 |
| `,` | (0, 4), (1, 3) | 3 |
| `"` | (1, 0), (0, 1): looks like an apostrophe, which is how the data uses it | 3 |
| `?` | (0, 0), (1, 0), (2, 1), (1, 2), (1, 4) | 4 |
| `-` | (0, 2), (1, 2), (2, 2) | 4 |
| `!` | (0, 0), (0, 1), (0, 2), (0, 4) | 2 |
| space | nothing | 4 |

No other punctuation exists. Any other character draws no defined shape and does not advance (§5).

### 3.4 Title font (`td-let.cin`)

40 glyphs × 8×8 in the file; **only the first 39 are loaded** (2,496 bytes into a 2,560-byte block). Only pixels whose stored value is `F7h` are drawn; every other value is transparent.

The string is upper-cased first (`UpCase`: only `a`..`z` change), then:

| Character | Glyph index |
|---|---|
| `A`..`Z` | 0..25 |
| `1`..`9` | 26..34 |
| `0` | 35 |
| `-` | 36 |
| `?` | 37 |
| `!` | 38 |
| `:` | 39: **not loaded** (§5) |
| space | nothing drawn |
| `.` | 2×2 dot at cell rows 6..7, columns 2..3 (0-based) |
| `,` | cell pixels (2, 5), (3, 6), (2, 7), (3, 7) as (column, row) |

Title advance (per upper-cased character), applied in this order:

1. before drawing: `1` moves the pen 1 left; `M` and `W` move it 1 right;
2. after drawing: +6 for `C E Z O 3 5`, +7 for everything else (space included);
3. then: `1` −2, `J` +1, `M V W` +2.

Title width, used only for centring (`Txt_MeasureTitle`): the sum over characters of 7 (`1`: 6), then `M` +1, `C E Z O 3 5` −1, `1` −2, `J` +1, `M V W` +2. This is not exactly the drawn width (`1` and `M` differ by one pixel); keep the formula as it is.

| Constant | Value | Unit | Provenance |
|---|---|---|---|
| Glyph cell (small) | 5 × 5 | px | `file0001:051A:088F` |
| Glyph cell (title) | 8 × 8 | px | `051A:042A` |
| Loaded title glyphs | 39 | | `051A:5852` (BlockRead 2,496) |
| Opaque background colour | byte 650 of the font block | | `051A:088F`; set to 0 by `051A:92AF`, `051A:2B26`, `0E8B:AF1F`, `0E8B:794C` |

## 4. Behaviour

### 4.1 Draw one small glyph (`Txt_DrawGlyph`, `file0001:051A:088F`; flight copies `0E8B:40B2` opaque, `0E8B:43A6` transparent)

Inputs: pen (x, y), character, ink, opaque flag.

```text
for row in 0..4, col in 0..4 (row outer, col inner):
    p := glyph byte (row, col) of the character's glyph index (§3.1)
    if p ≠ 0 and the character is not one of: space . " ? , -
        plot (x+1+col, y+1+row) in ink
    else if opaque
        plot it in the background colour
    if opaque and the character is one of: space , . " ! ? -
        plot it in the background colour            (overwrites any ink just drawn)
if opaque and the character is m or w:
    plot (x+6, y+1..y+5) in the background colour   (clears the column right of the 5-wide glyph)
then draw the code punctuation of §3.3 in ink
```

### 4.2 Draw a small string (`Txt_DrawString`, `051A:0B9F`; opaque `051A:0F04`; flight `0E8B:4947`, `0E8B:45EF`)

```text
offset := 1
for each character ch:
    if ch is x or y: offset := offset − 1
    draw glyph ch at (x + offset, y)          (transparent or opaque per variant)
    offset := offset + advance(ch)            (§3.2)
```

So the first inked column of a string drawn at x is x + 2 (for a normal letter). Example: `Txt_DrawString(2, 175, "  the ...")` puts the `t` at x 12..14.

`Txt_DrawStringOnce` (`051A:1DA1`) loads `td.lt2`, draws one transparent string and frees the font.

### 4.3 Typed text (`Txt_TypeString`, `051A:1269`; flight `0E8B:4C9F`)

Opaque. For each character: draw it (as §4.2), advance, then draw at the new pen position either the **cursor** (glyph 27, a solid 5×5 block in the ink colour) or, after the last character, a space (which erases the cursor), then wait for `Gfx_WaitRetraceBit` (`051A:0000`: returns as soon as the VGA status bit 3 is set, so at most one frame and possibly no wait at all; U-062). The next character overwrites the cursor.

### 4.4 Title text (`Txt_DrawTitle`, `051A:042A`)

For each character (after upper-casing): if it is not a space, build an 8×8 mask from the glyph (pixels equal to `F7h`), or the `.`/`,` shape; plot every set pixel at (pen + 1 + col, y + 1 + row) in the ink colour; then advance (§3.4). Transparent: unset pixels are not touched.

### 4.5 Layout conventions used by the callers

No routine wraps text. Callers place lines explicitly:

| Use | Font | Position | Ink / effect | Provenance |
|---|---|---|---|---|
| Mission title card | title | x = 160 − Round(width / 2), y = 90 | palette entry 85, RGB passed by the caller | `051A:5852` |
| Scene captions (`.scr` scenes) | small | lines at (10, 170), (10, 180), (10, 190); each first drawn at (9, y + 1) in colour 0 as a shadow | entry 210 = (14, 63, 63) | `051A:1E5A` and its 13 siblings |
| Still-image captions | small | x = 160 − Round(5 × length / 2) (Real48 `Round`, halves away from zero), y = 170, 180, 190 | `td1-16`/`td1-20`: entry 74 = (14, 38, 63); `td1-18`: entry 174 = (63, 4, 63) | `051A:4113`, `051A:471B` |
| `td1-14` captions | small | (160, 30), (160, 40), (160, 50) | entry 255 = (63, 10, 10) | `051A:4113` |
| `td1-13` captions | small | x = 170 − Round(5 × length / 2), y = 140, 147, 154 | entry 255 = (63, 63, 10) | `051A:4113` |
| `pic4.td` captions | small | (150, 90), (150, 100), (150, 110) | entry 90 = (63, 5, 5) | `051A:4F5A` |
| Dialogue names | small | x0 = side × 160 + 79 − (length div 2) × 5; black at (x0, 90), (x0, 89), (x0 + 1, 90), then ink at (x0 + 1, 89) | speaker entry | `051A:1618` |
| Dialogue lines | small, typed | (2, 175), then scroll | speaker entry | `051A:1BF7` ([dialogue-player.md](../narrative/dialogue-player.md)) |
| Dialogue prompt (scene 1) | small | black at (0, 191), (1, 191), (2, 191), then entry 254 at (1, 191) | entry 254 = (32, 0, 0) | `051A:92AF` |
| Hint screens | small, typed | x = 10, y from 40 down | entry 240 = (63, 10, 10) | `051A:2B26` |

"Entry *n* = (r, g, b)" means the screen writes that 6-bit RGB into its palette buffer before the fade-in, so the ink colour is independent of the image's palette.

### 4.6 Colour model

The ink is a palette index. Glyph pixels are never taken from the font file (§3). Screens give text its colour by choosing a free palette entry and setting its RGB (table above); dialogue copies `td.pal[colour byte]` into an allocated entry. A port can therefore draw text as "solid ink index" and handle colour entirely in the palette.

## 5. Edge cases and original bugs

1. **Out-of-table characters (small font).** A character outside §3.1 (upper case, `:`, `'`, `(`, `)` ...) is looked up at glyph index *character − 61h*, which lies before the font block: `!` reads the 25 bytes 1,600 bytes before it. Any non-zero byte there is drawn in the ink colour. For `!` the code marks are then drawn on top. In C-0002 every `!` came out clean, so those bytes were zero (U-061). Other unsupported characters also advance 0, so they overprint the next character. **Port policy:** draw nothing for the lookup (equivalent to the observed result); keep the 0 advance.
2. **Title `:`** maps to glyph 39, which is never loaded; the 64 bytes come from uninitialised heap memory. **Port policy:** draw nothing. The `.` and `,` shapes also fall through to a glyph lookup far outside the block; the result observed is the plain dot. **Port policy:** draw only the shapes of §3.4.
3. **Pen beyond x = 351** wraps into the next VGA row (address = y × 88 + x div 4) and becomes visible at the left edge one pixel row lower. No shipped string is long enough to reach it on the screens analysed (the longest dialogue lines end before x = 320). **Port policy:** clip at the screen edge (T2 deviation, unobservable with the shipped data).
4. **Measured vs drawn title width** differ by a pixel for `1` and `M`; centring uses the measured width (§3.4). Preserve.
5. Glyph 38 of the small font (the 39th slot of the 975-byte block) is uninitialised; nothing maps to it.

Mirror items 1–3 in `original-bugs.md` when that catalogue is next updated.

## 6. Test vectors

All from C-0002 (preset P1), compared pixel by pixel with a scratch renderer written from this spec (RE work area, not committed). Strings are referred to by their location; see the resource maps for the text.

| # | Setup / input | Expected output | Evidence |
|---|---|---|---|
| 1 | Mission 1 title (ShortString at `file0001:0000:0478`, 27 chars), title font, entry 85 | Measured width 185; x = 160 − Round(92.5) = **67**, y = 90; 729 inked pixels, 0 differences | C-0002 f1162 |
| 2 | Speaker names of scene 1: left 13 chars, right 10 chars | x0 = 79 − 6 × 5 = 49 and 160 + 79 − 5 × 5 = 214; ink at (50, 89) and (215, 89); 194 ink pixels, 0 differences | C-0002 f2433 |
| 3 | Dialogue prompt (`051A:9265`, 73 chars) at (1, 191), entry 254 | 539 inked pixels, 0 differences | C-0002 f2433 |
| 4 | Second line of scene 1 (two leading spaces) drawn at (2, 175) and scrolled 14 px | inked columns start at x = 12 (`t`), 16 (`h`), 21 (`e`), 29 (`n`); the `"` pixels at (49, 162) and (48, 163); `.` at x = 98, row 166 | C-0002 f2433 (also matched by `td-render` `oracle_parity`) |

## 7. Presentation and audio hooks

None. Typed text paces itself on vertical retrace (§4.3); it makes no sound.

## 8. Open questions

- [ ] U-061: what lies before the font block in practice (only matters for exactness of unsupported characters).
- [ ] U-062: frames per typed character on hardware and in the oracle.
- [ ] Episode 2/3 copies of these routines were not compared byte for byte (the dialogue players use the same calls).

## 9. Provenance

| Claim | Address / capture | Method | Confidence |
|---|---|---|---|
| Glyph drawing, punctuation shapes, background byte | `file0001:051A:088F`, `0E8B:40B2`, `0E8B:43A6` | static | CONFIRMED |
| Advance table, x/y shift | `051A:0B9F`, `051A:0F04`, `051A:1269`, `0E8B:45EF`, `0E8B:4947` | static | CONFIRMED |
| Typed text and cursor | `051A:1269`, `051A:0000` | static | CONFIRMED (timing U-062) |
| Title font map, mask F7h, advance, measure | `051A:042A`, `051A:02D7`, `051A:5852` | static + capture | CONFIRMED |
| 39 title glyphs loaded | `051A:5852`, `051A:527F`; C-0002 int21 (2,496 bytes) | static + dynamic | CONFIRMED |
| Caption positions and colours | `051A:1E5A`, `051A:4113`, `051A:471B`, `051A:4F5A`, `051A:2B26` | static | LIKELY (only the dialogue and title screens were compared with captures) |
| `td.ltr` unused | byte scan of all exes | static | CONFIRMED |

## 10. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-02 | RE-3 | Initial, from static analysis of `file0001` and C-0002 frames |
