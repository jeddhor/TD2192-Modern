# Save and restore screens (Episode 1)

| | |
|---|---|
| **Spec ID** | `flow/save-screens` |
| **Revision** | 1 |
| **Status** | Draft |
| **Fidelity tier** | T0 for the screen image and palette; T1 for the bytes written to `td.sav`/`td.stt` and for the campaign state after a restore; T2 for fade and polling timing |
| **Confidence** | CONFIRMED (static analysis of both routines, and oracle runs C-0042 and C-0043: screen pixels, palette, file bytes before and after, DS:0D28..0D2B and the menu cursor) |
| **Tasks** | M5-05, M11 (feeds IM-7); resolves U-301 and U-011 |
| **Unknowns** | U-011, U-301 (resolved here) |

Related: the main menu that calls these screens, [screens §3.1](screens.md) (confirmation box 2, cursor rules); [episode-program-flow §4.2](episode-program-flow.md) (menu items 1 and 2); fonts, [render/text](../render/text.md); file layouts, [FORMATS §13.2–13.3](../../re/FORMATS.md#13-configuration-and-saves); E2/E3 differences, [episode-deltas §9.3](../../re/episode-deltas.md). Text comes from the resource map by id (`docs/re/exe-resources/file0001.toml`: `saves.restore`, `saves.save`).

## 1. Summary

Menu item 1 (*save game*, only while a game is in progress) calls `Sav_SaveScreen(Mission, Kills, Pal)` (`file0001:051A:7EEA`). Menu item 2 (*restore game*) calls `Sav_RestoreScreen(Pal)` (`051A:76EA`). Both screens:

- draw ten slot bars from `td.sss`, with a name from `td.stt` and a mission number and kill count from `td.sav` in each;
- fade in;
- let the player move a highlight with Up/Down;
- fade out.

A **saved game is exactly two values: the next mission to fly (`g_MissionNumber`, 1..20) and the kills to date (`g_KillsToDate`, i16)**.

- **Restore:** choosing a used slot loads both values and leaves. Enter on an empty slot does nothing; Esc leaves without changes.
- **Save:** choosing any slot (used or empty) starts a name entry that **cannot be cancelled**. Enter then rewrites the first 30 bytes of `td.sav` (all ten slots) and the whole of `td.stt`.

## 2. Definitions and units

| Term | Original | Type | Notes |
|---|---|---|---|
| Slot | 1..10 | — | Slot *s* is drawn in bar *s* (top to bottom) |
| Slot mission | byte *s* × 3 − 3 of `td.sav` | u8 | **Empty slot** when 0 or > 20; normalised to 0 in memory on load |
| Slot kills | bytes *s* × 3 − 2 .. *s* × 3 − 1 of `td.sav` | u16 LE | Kills to date at the time of saving. Read and written as raw 16 bits; **shown unsigned** (0..65535), loaded into the i16 `g_KillsToDate` |
| Slot name | line *s* of `td.stt` | string | Read with `ReadLn` into a `String[255]` |
| `Pal` | value parameter, 768 bytes | 6-bit RGB × 256 | The caller passes `g_MenuPalette` (DS:07E0) as the menu left it (§3.2) |
| Cursor | local, 1..10 | u8 | Starts at 1 on every entry |
| Pen (x, y) | | px | Pen positions as in [render/text §4.2](../render/text.md) (first inked column x + 2, rows y + 1..y + 5) |
| Row of slot *s* | y(s) = 19 *s* − 3 | px | 16, 35, 54, …, 187 |

## 3. Data

### 3.1 Files

| File | Access | Layout |
|---|---|---|
| `td.lt2` | read, 950 bytes, into a 975-byte block `g_Font5` (DS:9770); freed at exit | small font ([render/text](../render/text.md)); its byte 650 (the opaque background colour) is **178** in the shipped file and is not changed here |
| `td.stt` | text file. Both screens read 10 lines; the save screen rewrites the file (`Rewrite`, 10 × `WriteLn`) | one name per line, CR LF line ends; lines may have any length (§4.4) |
| `td.sav` | untyped file, record size 1. Both screens read slots 1..10 (seek (*s* − 1) × 3, read 1 + 2 bytes); the save screen writes slots 1..10 back the same way | bytes 0..29: ten 3-byte slots; bytes 30..69 are never read or written by any E1 code |
| `td.sss` | read one byte at a time, 6,080 bytes | 19 rows × 320 bytes; row r is plotted at screen rows 10 + r + 19k, k = 0..9 |

Shipped templates: `td.sav` = 10 × (02, 0D 00), then 40 × FFh. So every slot holds "mission 2, 13 kills" and the restore screen shows ten used slots. `td.stt` = ten 24-character lines. The E2 and E3 templates have the same layout (`td2.sav`: 10 × (2, 15); `td3.sav`: 10 × (2, 14)).

### 3.2 Palette and colours

`g_Palette` (DS:0D2E) := `Pal`. In practice that is `tdmen.pal` with the menu's changes to entries 224..233: (16, 0, 0) each, except 227 = (0, 0, 0). C-0042 and C-0043 confirm this; neither `td.sss` nor the text uses those entries. `td.sss` uses entries 0..28, 178, 242 and 243.

| Use | Entry | `tdmen.pal` value |
|---|---:|---|
| Column headings | 80 | (15, 63, 63) |
| Slot text, normal | 92 | (63, 3, 15) |
| Slot text, highlighted | 31 | (54, 56, 56) |
| Field background (the `td.sss` boxes, the save screen's clears and opaque text) | 178 | (3, 15, 3) |
| Rows 0..9 | 0 | (0, 0, 0) |

### 3.3 Layout

| Element | Pen (x, y) | Font, ink | Resource |
|---|---|---|---|
| Heading "name" | (45, 2) | small, transparent, 80 | `saves.restore[1]` / `saves.save[1]` |
| Heading "mission" | (200, 2) | same | `[2]` |
| Heading "kills" | (247, 2) | same | `[3]` |
| Slot *s* name | (43, y(s)) | small, transparent, 92 or 31 | name string; empty slots show `saves.restore[0]` / `saves.save[0]` |
| Slot *s* mission | (209, y(s)) | same, **only if the slot is used** | `Str(mission)`, decimal, no padding |
| Slot *s* kills | (250, y(s)) | same, **only if the slot is used** | `Str(kills as unsigned 16-bit)` |

The two screens use identical layouts and strings: `saves.save[0..3]` are copies of `saves.restore[0..3]`.

## 4. Behaviour

### 4.1 Common set-up (both screens, in this order)

```text
save screen only: flush the BIOS keyboard buffer (head := tail)
g_Palette := Pal
cursor := 1
load td.lt2 into a new 975-byte block
restore screen only: flush the BIOS keyboard buffer
names[1..10] := ReadLn × 10 from td.stt
kills[1..10] := 0; mission[1..10] := 0
for s = 1..10: read mission[s] (1 byte) and kills[s] (2 bytes) from td.sav at (s − 1) × 3
               if mission[s] = 0 or mission[s] > 20: mission[s] := 0; names[s] := empty label
plot colour 0 at every pixel of rows 0..9
draw the three headings (ink 80)
plot td.sss (rows 10..199, ten copies of the 19-row bar)
for s = 1..10: if mission[s] ≠ 0: draw mission and kills (ink 92); draw names[s] (ink 92)
Gfx_FadeInScene                       (screens §2.1: DAC 1..255 from black to g_Palette, 17 levels)
```

- There is no mode re-initialisation. The screen draws over the menu's page; VGA rows 0..199 are fully replaced.
- The menu has already faded to black (`Flow_MenuFadeOut`) before the call.
- Keys typed while the screen is drawn or fading in stay in the BIOS buffer and are handled by the loop. Keys typed earlier are flushed.
- A file that is missing, or a `td.sav` shorter than 30 bytes, stops the program with a Pascal I/O runtime error (OB-043).

### 4.2 Selection loop (both screens)

Each pass of a busy loop (no wait):

```text
key := 1; ext := 1                                  (sentinels: "no key")
if KeyPressed: key := ReadKey; ext := 0; if key = 0: ext := ReadKey
if controller = joystick (DS:12C8 = 2): read the joystick (g_JoyY, buttons)
if ext = 48h or key = '8' or g_JoyY = 1 (up):
    if g_JoyY ≠ 0: Crt.Delay(200)
    redraw slot cursor in ink 92;  cursor := cursor − 1, or 10 if cursor = 1
if ext = 50h or key = '2' or g_JoyY = −1 (down):
    if g_JoyY ≠ 0: Crt.Delay(200)
    redraw slot cursor in ink 92;  cursor := cursor + 1, or 1 if cursor = 10
if key = Space or key = Enter or joystick button 1 or 2 is down:
    restore screen: §4.3      save screen: §4.4
redraw slot cursor in ink 31                       (the highlight; every pass)
restore: leave when key = Esc        save: leave when key = Esc or key = Enter
```

- A "redraw" draws the slot's mission and kills (used slots only) and its name, transparent, at the positions of §3.3. Drawing the same glyphs again in a new ink recolours them.
- The first highlight appears in the first pass after the fade-in.
- Up is the Up arrow, or keypad 8 without NumLock (extended 48h), or the character `8`. Down is the Down arrow, keypad 2 (extended 50h) or `2`. Both wrap.
- When the loop ends: `Gfx_FadeOutScene` (screens §2.1), free the font block, return to the menu.

### 4.3 Restore: choosing a slot

```text
if mission[cursor] ≠ 0:
    g_MissionNumber (DS:0D29) := mission[cursor]
    g_KillsToDate (DS:0D2A) := kills[cursor]          (raw 16 bits into the i16)
    key := Esc                                         (the loop ends after this pass)
else: nothing                                          (empty slot: stay on the screen)
```

Back in the menu (`0000:3B1F`): if `g_MissionNumber` ≠ 0 the menu cursor becomes 3 (*next mission*, shown yellow), whether a slot was loaded or the player pressed Esc during a game in progress. Then the menu is reloaded. Nothing else changes: not the script choice, the configuration, `g_MissionKills` or any flight state. Mission number and kills to date are the whole campaign state carried between mission blocks ([episode-program-flow §4.2–4.3](episode-program-flow.md); the launcher hand-off record carries the same two values plus menu cursor, calibration and script choice). So after a restore, *next mission* plays block `g_MissionNumber` from its start, and the debrief adds that flight's kills to the restored kills to date.

The menu asks for confirmation (box 2, "current game will be lost", [screens §3.1](screens.md)) before calling the restore screen only when a game is in progress. Esc on the restore screen keeps the current game.

### 4.4 Save: choosing a slot and entering the name

Any slot can be chosen, used or empty, without confirmation:

```text
fill x 209..225 and x 250..266, rows y(c)..y(c)+6 with colour 178 (font byte 650)   (c = cursor)
names[c] := 31 spaces (saves.save[5])
mission[c] := Mission; kills[c] := Kills                (the parameters: g_MissionNumber, g_KillsToDate)
len := 0                                                (typed characters, buffer of 20)
repeat
    draw opaque, ink 31, background 178: Str(mission[c]) at (209, y(c)), Str(kills[c]) at (250, y(c)),
                                         names[c] at (43, y(c))
    wait until KeyPressed; key := ReadKey; if key = 0: ext := ReadKey
    if key = Backspace (8) and len > 0: len := len − 1
    if len < 20:
        if key is '-', '.' or Space: append key
        k := UpCase(key)
        if k is '0'..'9': append k
        if k is 'A'..'Z': append the lower-case letter
    names[c] := ' ' + the len typed characters, padded with spaces to 24 characters
    mission[c] := Mission; kills[c] := Kills
until key = Enter
write td.sav slots 1..10: (mission[s], kills[s] as 2 bytes LE) at (s − 1) × 3
rewrite td.stt: WriteLn(names[s]) for s = 1..10
redraw slot c transparent in ink 31
(the loop then ends because key = Enter)
```

Consequences:

- **Allowed characters:** a–z (typed in either case, stored in lower case), 0–9, space, `-` and `.`. Everything else is ignored, including **Esc**: once a slot is chosen the save always happens (OB-044). Maximum 20 characters; further characters are ignored, but Backspace still works. There is no visible cursor.
- The first draw (31 spaces, opaque) wipes the old name. Later draws are 24 characters wide. Every draw uses a background of 178, the colour of the `td.sss` box.
- **`td.sav` after a save:** all ten slots are rewritten.
  - The chosen slot becomes (Mission, Kills).
  - Every other slot keeps its kills word, and its mission byte becomes the normalised value: an empty slot's 21..255 becomes 0.
  - Bytes 30..69 are untouched.
- **`td.stt` after a save:**
  - The chosen line is `' ' + name` padded to 24 characters.
  - Each other used slot's line is written back exactly as read.
  - Each **empty** slot's line becomes the 19-character empty label `saves.save[0]`. The file therefore stops being 260 bytes once it holds empty slots (C-0042: 215 bytes).
- The game state is not changed by saving, and the menu cursor stays at 1.

### 4.5 Episodes 2 and 3

The same two routines, with their own files: E2 `td2:04EC:7B20` / `04EC:8322` on `td2.stt` / `td2.sav`; E3 `td3:04A0:5450` / `04A0:6413` on `td3.stt` / `td3.sav`; `td.sss` and `td.lt2` are shared.

- E2 moves the restore screen's keyboard flush to after the drawing, so keys typed during the drawing are lost.
- E3's restore screen flushes at the start and clears the joystick Y and button flags before the loop.
- Both save screens are identical to E1's.

Details: [episode-deltas §9.3](../../re/episode-deltas.md). C-0016 shows the E2 restore path with a prepared `td2.sav`.

## 5. Edge cases and original bugs

| Quirk | Original | Port policy (proposal) |
|---|---|---|
| Missing `td.sav` or `td.stt`, or `td.sav` shorter than 30 bytes (OB-043) | Runtime I/O error: the program stops | fix-all: treat missing slots as empty, and create the files on the first save |
| No cancel after choosing a slot on the save screen; Esc ignored in name entry (OB-044) | The slot is always overwritten | preserve (Classic); Enhanced may let Esc cancel |
| Save rewrites all slots; empty slots get mission byte 0 and the label as name | `td.stt` shrinks; kills words of empty slots survive | preserve (file compatibility) |
| Kills word ≥ 32768 | Shown unsigned, loaded as a negative i16; the debrief then gives the cheater rating ([episode-program-flow §4.5](episode-program-flow.md)) | preserve |
| Restore of an empty slot | Ignored; only Esc leaves an all-empty screen | preserve |
| Esc on the restore screen during a game | Game kept, menu cursor still moves to 3 | preserve |
| Highlight redrawn on every loop pass | Invisible | the port may draw once per change |

## 6. Test vectors

| # | Setup / input | Expected | Evidence |
|---|---|---|---|
| 1 | Save screen, `td.sav` with slots 1..10 empty (missions 0, 21, FFh, 0, …), cursor 1 | Rows 0..9 colour 0 except the headings in 80; rows 10..199 = `td.sss` except ink pixels (31 in slot 1, 92 elsewhere); every name is the empty label; no numbers; DAC = `tdmen.pal` except 224..233 | C-0042 f410 |
| 2 | Same, Down × 3, Enter, keys `r`, Shift+`e`, `.`, `-`, `7`, Space, `,`, `/`, `a`, `b`, Backspace, Enter (Mission 7, Kills 1234) | `td.sav` bytes 0..29: `00 00 00 00 11 11 00 22 22 07 D2 04` then 18 × `00`, bytes 30..69 unchanged (before: `00 00 00 15 11 11 FF 22 22 00 33 33 …`). `td.stt` = 10 lines: slot 4 `" re.-7 a"` + 16 spaces (24 characters), the 9 others the empty label (19 characters); 215 bytes | C-0042 `files\` |
| 3 | During test 2 after `b` | slot 4 shows 7, 1234 and the name in ink 31 with no other bar pixels changed | C-0042 f574, f587 |
| 4 | Restore screen after test 2, Enter on slot 1 (empty), Down × 3, Esc | No change; DS:0D29 = 7, DS:0D2A = 1234 before and after; menu cursor DS:0754 = 3 | C-0042 f1069, mem f1265 |
| 5 | Restore screen with the shipped files, DS:0D29 = 7, DS:0D2A = 1234; Down × 4, Enter | DS:0D29 = 2, DS:0D2A = 13; menu cursor 3 | C-0043 mem f196, f688 |
| 6 | Save screen (shipped files after test 5), Down, Enter, letters `a`..`y` (25), Esc, Backspace, `z`, Enter | `td.stt` line 2 = `" abcdefghijklmnopqrsz"` + 3 spaces; the other nine lines byte-identical; `td.sav` unchanged (all slots (2, 13)); cursor stays 1; the screen stays open after Esc | C-0043 `files\`, f1212, f1224 |

## 7. Presentation and audio hooks

There is no music change: the menu song keeps playing. There are no sound effects. The screens use the standard 17-level scene fade in and out ([screens §2.1](screens.md)).

## 8. Open questions

None. The purpose of `td.sav` bytes 30..69 is moot: no E1 code touches them (U-011).

## 9. Provenance

| Claim | Address / capture | Method | Confidence |
|---|---|---|---|
| Set-up, file reads, empty rule, layout, inks | `file0001:051A:76EA..7B68`, `051A:7EEA..8368` | static; C-0042, C-0043 frames | CONFIRMED |
| Selection keys, wrap, joystick, highlight | `051A:7B7A..7E66`, `051A:837A..8A6D` | static; C-0042, C-0043 | CONFIRMED (keyboard); LIKELY (joystick) |
| Restore sets mission and kills | `051A:7D89..7DB8`; menu `0000:3B1F..3B35` | static; C-0042, C-0043 memory | CONFIRMED |
| Name entry rules, 20-character limit, padding to 24 | `051A:858C..88BB` | static; C-0042, C-0043 files | CONFIRMED |
| All ten slots written, `td.stt` rewritten | `051A:88BE..89BF` | static; C-0042 files | CONFIRMED |
| Caller passes `g_MenuPalette`, Mission, Kills | `0000:3B12..3B7D` | static; C-0042 DAC | CONFIRMED |
| E2/E3 deltas | episode-deltas §9.3 | static (matcher) | LIKELY |

## 10. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-05 | RE-11 (Archaeologist + Spec author + Verifier) | Initial; resolves U-301 and U-011 |
