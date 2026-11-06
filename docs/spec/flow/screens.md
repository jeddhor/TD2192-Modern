# Front-end screens of Episode 1 (layouts, keys, waits)

| | |
|---|---|
| **Spec ID** | `flow/screens` |
| **Revision** | 3 |
| **Status** | Draft |
| **Fidelity tier** | T0 for layouts, fonts, colours and resource ids; T1 for the `Random` call of the retry prompt and the skip rule; T2 for wait and fade timing (§2.1, §2.2: measured at P1) |
| **Confidence** | LIKELY (static analysis of each routine); main-menu pixels CONFIRMED against C-0002 f997 |
| **Tasks** | M5-03, M5-05, M5-07, M8-09, M9-18, M9-19 (resolves U-120..U-125) |
| **Unknowns** | U-120, U-121, U-122, U-123, U-124, U-125 (resolved here); U-227..U-229 (new) |

Fonts and drawing: [render/text](../render/text.md) (small font `td.lt2`, title font `td-let.cin`, typed text). Portrait loading `Dlg_LoadPortrait` (`051A:1618`): [narrative/dialogue-player](../narrative/dialogue-player.md). Flow and order of screens: [episode-program-flow](episode-program-flow.md). Text is always taken from the resource map by id (`docs/re/exe-resources/file0001.toml`).

## 1. Summary

Every front-end screen follows one pattern: re-initialise mode X (`Gfx_InitUnchained` `386E:19EF`, which clears video memory and sets **every DAC entry to black**), show VGA row 0 with the split line off, load `td.lt2` (and sometimes `td-let.cin`), load a palette into `g_Palette` (DS:0D2E) and change a few entries, draw, fade in (17 levels), wait, fade out. The tables below give each screen's palette changes, positions, inks, resource lines, waits and keys. Two answers matter for game state: the **retry prompt calls `Random(10)`** (it consumes one value of the gameplay RNG stream), and **Esc in the pre-flight story jumps straight to the flight only from steps that are followed by a skip test** (§3.12).

## 2. Common behaviour

### 2.1 Fades

`Gfx_FadeInScene` (`051A:0016`) sets DAC entries 1..255 to Round(c × s / 16) of `g_Palette` for s = 0..16 (Real48 `Round`), waiting before each level until the VGA status bit 3 is set (it returns at once if a retrace is in progress). `Gfx_FadeOutScene` (`051A:013F`) runs s = 16..0. DAC entry 0 is never faded.

**Frames per level (BUG-6).** A level costs one retrace wait plus, for each of the 765 components, a Real48 conversion, division and `Round`. Those Real48 operations are cheap on a zero operand and expensive otherwise, so the time per level depends on how many components of the palette being faded are non-zero, not on which screen it is. At preset P1 (cycles 20000):

| Palette being faded | Non-zero components | Frames per level | Fade (17 levels) | Evidence |
|---|---:|---:|---:|---|
| Nearly black: `g_Palette` untouched since program start (BSS zeros), or as the script-choice screen left it (it zeroes the buffer and sets 7 components) | ≤ 10 | 1 | 17–18 frames | script choice C-0002 f645..662, f780..797; title card after it C-0002 f1063..1080, f1409..1428; title card at a direct start C-0160 f342..359, f689..706 |
| A loaded picture palette (scene cards, stills, dialogue, debrief, orders, dossiers, retry, story so far, the menu's `tdmen.pal`) | ≈ 600–700 | 2 | 33–34 frames | menu C-0002 f835..868; hangar f1449..1482; title card after "story so far" (677 non-zero) C-0170 f1003..1036, f1364..1397 |

**Level phase (IM-21 observation, M12-14; U-510 for SP).** Level 0 multiplies every component by zero, so it is cheap: it takes one frame even with a picture palette. Each level starts at the first retrace after the previous level's last write. In the oracle a level of a picture palette writes from the retrace for about 1.5 frames, level 0 for 0.2 frame, so a level is first shown complete, counted from the frame in which the fade starts, at 2, 4, …, 34 (fade-in) or 3, 5, …, 33, 34 (fade-out); with a nearly black palette at 2, 3, …, 18. What follows the fade starts at the retrace after the last level, in the frame that shows it. Measured in `frames.tsv`: menu C-0002 f835 and f1027, C-0120 f272 and f435; flight start C-0002 f3439, C-0017 f458, C-0015 f631, C-0036 f660; Esc menu C-0012 f3555; flight 17 and 19 screens C-0067, C-0069; title cards C-0002 f1063 and f1409, C-0170 f1364. The same holds for the flight fades (flight-screens §4.5.3).

The palette buffer is shared: a screen that draws only a few inks (the mission title card sets only entry 85) fades whatever the previous screen left in `g_Palette`. So the **mission title card** fades at 1 frame per level only while `g_Palette` is still nearly black, which in Episode 1 means the first title card after program start when no picture palette has been loaded since (new game straight from the script choice, or from a direct start); after any scene, dialogue, story so far or flight debrief it fades at 2 frames per level like every other screen. The intermediate case does not occur in the shipped flows; the crossover lies between 10 and 677 non-zero components (an estimate from the measured level times: about 380). Port rule (Classic, T2): count the non-zero components among the entries the fade writes; fewer than 380 → 1 frame per level, otherwise 2. The same rule applies to the menu fades (`0000:0000`, `0000:0124`) and to the flight fades ([flight-screens §4.5.3](flight-screens.md#453-flight-fades)). On a faster machine every level would take 1 frame; P1 is the reference.

### 2.2 Waits (U-123)

| Routine | Ends when | Keys consumed |
|---|---|---|
| `Inp_WaitKeyOrButton` `051A:1A40` | `KeyPressed` (BIOS buffer not empty) or joystick button 1 or 2 down | one key (two bytes for an extended key); Esc sets `g_SkipStory` DS:1F4E |
| `Flow_WaitSecondsOrKey(n)` `051A:1AA3` | as above, or n seconds of DOS time | as above |

`Flow_WaitSecondsOrKey(n)`: start := DOS time as a **16-bit** centisecond count, `(h × 360000 + m × 6000 + s × 100 + cs) mod 65536`; loop (reading the joystick when present) while no key is in the BIOS buffer, no button is down, now < start + 100n (16-bit, the sum wraps) and start ≤ now. Consequences:

- Keys typed earlier and still in the BIOS buffer (for example during the fade-in, or during typing) end the wait at once. Nothing flushes the buffer except where stated (hint screen, main menu).
- Every 655.36 s of the day there is a window of n seconds in which start + 100n wraps past 65535: then the wait ends immediately (OB-034). A wait that straddles the wrap of `now` ends when `now` wraps (start ≤ now fails).
- The joystick is read through `Inp_JoystickAvailable`, independent of the configuration.
- **Clock (BUG-7).** "n seconds" is DOS time, which advances only with the BIOS tick: the wait ends at the first BIOS tick at which the DOS centisecond clock has moved by 100·n, i.e. after ceil(n × 18.2065) BIOS ticks or one less ([timing §4.4](../core/timing.md#44-clocks-outside-flight-bug-7)). With the Classic BIOS clock of 19.2015 Hz ([ADR-0007](../../adr/0007-classic-tick-rate.md), proposed) a wait of n seconds lasts n × 18.2065 / 19.2015 = 0.948 n wall seconds: 5 s → 4.74 s (332 frames), 10 s → 9.48 s (664 frames), 15 s → 14.2 s, 25 s → 23.7 s, 30 s → 28.4 s, 45 s → 42.7 s. Measured: 328 and 664 frames (C-0002, C-0170). The `Crt.Delay` pauses inside screens (debrief and dossier lines, save and restore screens) are real milliseconds and do not scale. Every timed wait and its clock: timing §4.4.

### 2.3 Text and palette conventions

Positions are pen positions as in text.md (first inked pixel at x + 2 for a normal letter, rows y + 1..y + 5). "Typed" = `Txt_TypeString` (`051A:1269`): opaque, background colour 0 (each screen sets font byte 650 to 0), one character per retrace bit. "Drawn" = `Txt_DrawString` (`051A:0B9F`), transparent. Entry n := (r, g, b) means `g_Palette[n]` is set before the fade-in; other entries keep the loaded palette.

## 3. Screens

### 3.1 Main menu (`UI_MainMenuLoop` `0000:3381`, loader `UI_LoadMainMenu` `0000:2E88`) — U-120

Loader: `tdmen.pal` into `g_MenuPalette` (DS:07E0); the `td.men` main-menu image (bytes 16,000..79,999) to VGA rows 0..199; split line 400; entries 224..233 := (16, 0, 0), then entry 227 (item 3, "next mission") := (0, 0, 0); the cursor's entry := (63, 0, 0), or (63, 63, 0) for cursor 3; fade in all entries (`Flow_MenuFadeIn` `0000:0000`, 17 levels as 2.1); then the cursor entry goes back to (16, 0, 0) (or (0, 0, 0) for item 3) **in the palette buffer only**; DAC 255 := (63, 63, 63); finally the free heap size (`MemAvail`, decimal) is drawn with `Txt_DrawStringOnce` at pen (10, 10) in **ink 0**. The ink-0 digits are invisible on black and show only where the image is not black: C-0002 f997 differs from `td.men` in exactly 5 pixels, (24, 11), (40, 11), (40, 13), (24, 15), (40, 15), all ink 0 over colour 181. A port cannot reproduce the MemAvail number; proposal: omit it (T3, 5 pixels).

Loop: the highlight step runs once when the loop starts, right after the loader's fade-in, and again after every change of cursor (C-0002 f868..872: the ramp of the start cursor, 11 steps per frame, after the fade-in f835..868):

| Step | Effect |
|---|---|
| Highlight | cursor ≠ 3: DAC (224 + cursor) := (k, 0, 0) for k = 16..63, one per retrace start. Cursor 3: DAC 227 := (k, k, 0) for k = 0..63 (yellow) |
| Wait | poll `KeyPressed`; when a key is there read it (and its extended byte) and **discard every further key in the buffer**; read the joystick if the controller is the joystick |
| Up | extended 48h, character `8`, or joystick up: cursor − 1, 0 → 8; skip 1 and 3 (to 0 and 2) while no game is in progress; joystick: `Crt.Delay(200)` |
| Down | extended 50h, character `2`, or joystick down: cursor + 1, 8 → 0; skip 1 and 3 (to 2 and 4) while no game is in progress |
| Esc | quit box (below); `Y` (upper-cased) confirms and sets cursor 8 |
| Select | Space, Enter, joystick button 1 or 2: the item's action ([episode-program-flow](episode-program-flow.md) §4.2) |
| Un-highlight | after any change: the previous item fades back: (k, 0, 0), k = 63..16, or (k, k, 0), k = 63..0 for item 3 |

So unselected items are dim red (16, 0, 0), item 3 is **invisible** unless selected, and item 1 ("save") is visible but skipped while no game is in progress (C-0002 f997: DAC 224..228 = (63,0,0), (16,0,0), (16,0,0), (0,0,0), (16,0,0)).

Confirmation boxes (Esc → box 1 "quit"; item 0 or 2 with a game in progress → box 2 "game lost"): the cursor entry is set to 63 in the buffer; all 256 DAC entries fade to half brightness, DAC = buffer × k / 32 (truncating) for k = 32..16, one level per retrace start; entries 192..209 are set back to full; `UI_ShowMenuBox(n)` copies box n over menu rows 84..115; wait for one key, upper-cased; box 0 restores the rows; fade back k = 16..32. `Y` confirms; any other key cancels.

### 3.2 Script choice (`UI_ScriptChoice` `051A:527F`) and missing-dialogue notice (`051A:56D5`)

Palette: all 256 entries := 0, then entry 1 := (10, 10, 10), 85 := (0, 33, 0), 92 := (40, 0, 0), 102 := (63, 0, 0), 103 := (63, 0, 0).

| Screen | Element | Position | Font, ink |
|---|---|---|---|
| Script choice | grey bands | rows 38..50 and 138..150, x 0..319, colour 1 | — |
| | `script_choice.warning[0]` | x = 160 − Round(width / 2), y 40 and again at y 140 | title font, 85 |
| | `warning[1..5]` | (66, 62), (54, 69), (42, 76), (38, 83), (66, 90) | drawn, 92 |
| | `warning[6]`, `[7]` | (66, 110), (90, 117) | drawn, 102 |
| Notice | `script_choice.missing_dialogue[0..5]` | (46, 70), (46, 77), (46, 84), (38, 91), (30, 98), (54, 105) | drawn, 92 |
| | `missing_dialogue[6]` | (66, 117) | drawn, 102 |

Keys: the script choice reads keys until the character is `1` or `2` (an extended key's second byte also counts, so Alt+N / Alt+M, extended 31h / 32h, select too; harmless); `1` → script 1, `2` → script 2. The notice ends on any key. Then fade out. C-0002 f710 shows entries 85, 92, 102 with these values.

### 3.3 Hint screens 1, 2, 6 (`Scene_HintScreen` `051A:2B26`) — U-121

Banner and colour as text.md §4.5 (`secret.pic`, entry 240 := (63, 10, 10)). After the fade-in the BIOS keyboard buffer is flushed, then lines of `e1.hints` are **typed** at pen x 10, ink 240:

| Hint (used before flight) | Lines `e1.hints[i]` at pen y |
|---|---|
| 1 (flight 1) | 8 @ 40, 9 @ 47, 10 @ 54, 11 @ 61, 12 @ 68, 13 @ 75, 14 @ 82, 15 @ 100, 16 @ 107, 17 @ 114, 18 @ 121, 19 @ 170, 7 @ 185 |
| 2 (flight 2) | 20 @ 40, 21 @ 47, 22 @ 54, 7 @ 170 |
| 6 (flight 6) | 0 @ 40, 1 @ 47, 2 @ 54, 3 @ 68, 4 @ 75, 5 @ 96, 6 @ 103, 7 @ 130 |

After each line (and once more before line 19 of hint 1) the screen tests for a key in the BIOS buffer or a joystick button; if there is one, the remaining lines are skipped. Then `Inp_WaitKeyOrButton` (which consumes that key; Esc sets the skip flag), fade out. Line 7 is the shared "press a key" line.

### 3.4 Assignment orders (`UI_AssignmentOrders` `051A:62FD`, block 2)

`td.pal`; entry 40 := (0, 63, 0). Frame: rectangle outlines (`Gfx_DrawRect` `051A:609D`, one pixel wide) (k, k)–(319 − k, 199 − k) in colour 5k for k = 0..5, colour 25 for k = 6..19, colour 5 (25 − k) for k = 20..25; then (275, 185)–(285, 190) colour 16 and (279, 187)–(281, 188) colour 40. Fade in, then `e1.assignment_orders[0..8]` typed at x 40, y 45, 51, 57, 69, 75, 81, 93, 99, 117, ink 40. `Flow_WaitSecondsOrKey(5)`, fade out.

### 3.5 Enemy dossier (`UI_EnemyDossier` `051A:6653`, block 5) and pilot dossier (`UI_PlayerDossier` `051A:74B2`, ending A)

`td.pal`; entry 40 := (0, 63, 0), entry 39 := (63, 0, 0); portrait: `Dlg_LoadPortrait(side 1, face f, name " ", colour 0)` (right half of rows 0..99, palette 112..223; the one-space name draws nothing); fade in; typed lines:

| Screen | Face | Lines (index @ x, y, ink) | Wait |
|---|---:|---|---|
| Enemy | 6 | `e1.dossier.enemy`: 0 @ 40,75; 1 @ 40,91; 2 @ 40,97; 3 @ 40,103; 4 @ 40,109; 5 @ 40,115; 6 @ 40,121; 7 @ 40,137; 8 @ 40,143, all ink 40; then 9 @ 40,109 ink 39 (over line 4); 10 @ 40,160 ink 40 | `Flow_WaitSecondsOrKey(10)` |
| Pilot | 1 | `e1.dossier.player`: 0 @ 20,45; delay 500; 1 @ 20,91; delay; 2 @ 20,97; delay; 3 @ 20,109; delay; then line 4 + the total kills (decimal) @ 20,121; `Crt.Delay(2000)`; 5 @ 20,109 ink 39 (over line 3); delay 500; 6 @ 20,160. Inks 40 except where stated | `Flow_WaitSecondsOrKey(25)` |

Delays are `Crt.Delay(ms)`.

### 3.6 Debriefing (`UI_Debrief(Kills, KillsToDate, Outcome)` `051A:6FF2`)

`td.pal`; entry 40 := (15, 55, 15), entry 39 := (0, 40, 0); portrait face 1 on the right as 3.5; fade in; typed at x 20:

| y | Text | Ink |
|---:|---|---:|
| 45 | `debrief.labels[0]` + mission number + `debrief.labels[1]` | 40 |
| 65 | `debrief.labels[2]` | 40 |
| | `Crt.Delay(500)` | |
| 75 | Outcome = 0: `debrief.labels[4]`; else `debrief.labels[3]` | 39 |
| 95 | `debrief.labels[5]` + Kills | 40 |
| | delay 500 | |
| 105 | `debrief.labels[6]` + (KillsToDate + Kills) | 40 |
| | delay 500 | |
| 130 | `debrief.note_label` | 40 |
| | delay 500 | |
| 140 | Outcome = 0: `debrief.failure_note`; else the rating (`debrief.cheater` or `debrief.tiers[t]`, thresholds in episode-program-flow §4.5) | 39 |

Numbers are `Str` of the integer (no padding). Then `Flow_WaitSecondsOrKey(10)`, fade out.

### 3.7 Retry prompt (`UI_RetryPrompt` `051A:6A33`) — U-124

```text
r := Random(10)                      (System Random, the gameplay RNG: one LCG step)
g_SkipStory := 0
mode X init; td.lt2; td.pal; entry 40 := (15, 55, 15); entry 39 := (0, 40, 0)
type retry.taunts[min(r, 9)] at (x_r, 90), ink 40, while the screen is still black
fade in
draw retry.taunts[10] at (86, 110) and retry.taunts[11] at (82, 117), ink 39
wait for a key; c := UpCase(key); if key = 0 read the extended byte
fade out
return c != Esc                       (any key but Esc flies again)
```

x_r for r = 0..9: 78, 46, 54, 82, 58, 46, 130, 30, 70, 42. Because the taunt is typed before the fade-in, it appears all at once. Because the prompt calls `Random`, every retry shifts the RNG sequence of the next flight ([core/bp7-runtime-semantics](../core/bp7-runtime-semantics.md): `RandSeed` starts at 0 and is never reseeded). The port must call its `BpRandom` once here.

### 3.8 Story so far (`Scene_BackgroundStory` `051A:8EAA`, menu item 5)

`g_SkipStory` := 0. Page 1: `Gfx_LoadJavelin` (`051A:5EE9`: `javelin.td` palette into `g_Palette`, image into VGA rows 200..399, display start row 200); entries 0..250 := each component × 6 div 9 (truncating); entry 254 G and B := 63 (R as loaded). Lines `e1.background_story[0..5]` drawn twice: shadow in ink 253 at (4, s + 1) and text in ink 254 at (5, s), screen y s = 15, 35, 55, 75, 95, 115. Fade in, `Flow_WaitSecondsOrKey(30)`, fade out. Page 2: `Gfx_LoadCity` (`city.td`, same way), same palette changes, lines 6..17 at screen y 10, 20, 30, 40, 50, 60, 80, 90, 100, 110, 130, 140 (shadow at (4, y + 1) in 253). Fade in, `Flow_WaitSecondsOrKey(45)`, fade out. Esc ends a page early but never skips page 2. (Screen y = VGA row − 200; the code passes VGA rows 215.. and 210...)

### 3.9 `td1-15.pic` (`Scene_StillTd1_15` `051A:4979`, block 18) — U-122

No split-screen or palette effect: a plain still. Mode X init, the file's 768-byte palette into `g_Palette`, then 121 rows of 255 bytes: row r, byte c is plotted at (33 + c, 30 + r) for c = 0..253, and (32, 30 + r) gets the byte before the row buffer (DS:0AE7; 0 in practice, as for the `.scr` scenes, OB-003); byte 254 of each row is never shown. Fade in, `Flow_WaitSecondsOrKey(10)`, fade out.

### 3.10 Other stills and cards

Unchanged from text.md §4.5 and asset-load-sites §4.

### 3.11 Help from the menu

`UI_MenuHelp` `0000:02AF`: `td.hlp` pages 1 and 2, each shown in full (200 rows) and held for a key; unchanged from the IM-1 port.

### 3.12 Esc in the pre-flight story — U-125

Every scene, card, dossier, hint and dialogue routine clears `g_SkipStory` when it starts (except the stills `051A:4113` and `051A:4979`) and the waits set it on Esc. The sequencer tests the flag **after most steps**; a set flag after a pre-flight step jumps to the flight call itself (past any remaining music change and hint), and after a post-flight step ends the block. Steps **without** a following test let the story continue, and the next routine clears the flag. Pre-flight steps without a test (all others have one; a test placed after a music change counts for the step before it):

| Block | Untested pre-flight steps |
|---:|---|
| 1 | D1 (so M8, H1 still run), H1 |
| 2 | D5 (so M8, H2 run), H2 |
| 3 | D11 |
| 4 | D16 |
| 5 | A (the enemy dossier; D19 then plays), D22 |
| 6 | D24 (so H6 runs), H6 |
| 7 | D27 |
| 8 | D31 |
| 9 | D35 |
| 10 | C:eggbase, D39 |
| 11 | D42 |
| 12 | D46 |
| 13 | D53 |
| 14 | D60 |
| 15 | D109 |
| 16 | D71 |
| 17 | D74 |
| 18 | D81 |
| 19 | C:soffice[0C45/04A1/0898], D87 |
| 20 | D95 |

So Esc on the mission-1 title or hangar card goes straight to flight 1 (no dialogue 1, no `low.cmf`, no hint), while Esc during dialogue 1 still shows hint 1 with `low` (C-0002 f2603 → f2788). Post-flight steps without a test: the last step of each block, and block 3 D13 (outcome branch), block 5 D3 / D156 branch, block 16 D72 / D105, block 18 P15, block 19 D89 (then flight 21; abort → return), block 20 P2 stills onwards (the episode-end card and endings have no tests). **Exception, block 19:** the tests after the post-flight steps that come before flight 21 (C:voffice, D88 and the `M3` C:hangar card, `0000:2A59..2A94`) do not end the block: a set flag jumps to the flight 21 call (`0000:2A9D`), skipping what is left before it. After flight 21 the tests end the block as usual. There is no debrief in block 20 ([episode-program-flow](episode-program-flow.md) §4.4). Block-by-block source: `0000:0F06..2E76`.

## 4. Edge cases and original bugs

| Quirk | Original | Port policy (proposal) |
|---|---|---|
| MemAvail digits in ink 0 on the main menu | 5 pixels in C-0002 | Omit (T3) |
| 16-bit DOS-time wait (OB-034) | A timed wait can end at once, about once every 11 minutes | Preserve in Classic is impossible without the clock; the port uses frame counts (T2 deviation) |
| Retry `Random(10)` | Consumes one RNG value per prompt | Preserve (T1) |
| `td1-15` first column | Byte before the buffer (black) | Preserve as black |
| Typed taunt before the fade-in | Appears at once | Preserve |

## 5. Test vectors

| # | Input | Expected | Evidence |
|---|---|---|---|
| 1 | Main menu after start-up, no game, cursor 0 | DAC 224..228 = (63,0,0), (16,0,0), (16,0,0), (0,0,0), (16,0,0); DAC 255 = (63,63,63); 5 ink-0 pixels at (24,11), (40,11), (40,13), (24,15), (40,15) | C-0002 f997 |
| 2 | Retry prompt with `RandSeed` = s | taunt index min(Random(10), 9); seed advanced by one LCG step | static (`051A:6A43`) |
| 3 | Esc on the mission 1 title card | next file opened is the flight's (`m1.td`), no `dialogue.dat` scene, no `secret.pic` | static (`0000:0F34`) |
| 4 | Debrief, Kills 3, KillsToDate 10, Outcome 1 | lines at y 45, 65, 75, 95, 105, 130, 140; rating tier for T = 13 | static |

## 6. Open questions

- U-227: what the stray byte DS:0AE7 holds (it is the first column of `td1-15` and of the `.scr` scenes; observed 0).
- U-228: hint screens: does the line-skip test see keys typed during the fade-in (the buffer is flushed only after the fade)? Static answer: no, the flush comes after the fade-in, so earlier keys are lost.
- U-229: the menu's `MemAvail` print and DAC 255 := white look like a leftover debug feature; is anything else drawn in ink 255 on the menu? (no other call found)
- U-302 (IM-7, M9-19): details the port had to assume: the box colour of the cursor entry on item 3, the pulse after a cancelled quit box, what item 5 reloads, the joystick delay after Down, and whether keys typed before a flight survive it. Also: §3.1 says "one per retrace start" for the pulse, but C-0002 f868..872 show 11 steps per frame at P1 (the wait returns at once while the retrace bit is set); the port uses 11.

## 7. Provenance

| Claim | Address | Method | Confidence |
|---|---|---|---|
| Main menu colours, keys, boxes | `0000:3381..424B`, `0000:2E88..30xx`, `0000:0000` | static; C-0002 f997 | CONFIRMED (colours, 5 pixels) / LIKELY (keys) |
| Waits | `051A:1A40`, `051A:1AA3` | static | LIKELY |
| Wait clock = DOS time; hold lengths | `051A:1AA3`; C-0002, C-0170 | static + capture | CONFIRMED |
| Fade frames per level follow the palette's non-zero count | `051A:0016`, `013F`; script choice zeroes `g_Palette` (`051A:54C1..54F3`); C-0002, C-0160, C-0170 | static + capture | CONFIRMED (the two measured classes); crossover estimated |
| Script choice, notice | `051A:527F`, `051A:56D5` | static; C-0002 f710 (palette) | LIKELY |
| Hint lines | `051A:2B26` | static; C-0002 f3332 (hint 1) | LIKELY |
| Orders, dossiers, debrief, retry | `051A:62FD`, `6653`, `74B2`, `6FF2`, `6A33` | static | LIKELY |
| Story so far | `051A:8EAA`, `5EE9`, `5FC7` | static | LIKELY |
| `td1-15` | `051A:4979` | static | LIKELY |
| Skip tests | `0000:0F06..2E76` | static (complete walk) | LIKELY; block 1 CONFIRMED by C-0002 |

## 8. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-02 | RE-8 (Archaeologist + Spec author) | Initial: U-120..U-125 |
| 2 | 2026-11-06 | RE-14 (Archaeologist + Spec author) | §3.12: in block 19 a skip before flight 21 jumps to flight 21, not to the block end (`0000:2A59..2A9D`; U-081) |
| 3 | 2026-11-06 | RE-16 (Archaeologist + Spec author) | §2.1: frames per fade level follow the non-zero count of the palette being faded (1 for a nearly black buffer, 2 for a picture palette at P1); the title card's fast fade only while `g_Palette` is still black (BUG-6; C-0160, C-0170). §2.2: the wait clock is DOS time, 0.948 n s with the ADR-0007 clock (BUG-7). §3.1: the highlight ramp also runs at loop entry |
