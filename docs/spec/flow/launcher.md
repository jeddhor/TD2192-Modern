# Launcher (`td1.exe`) and the start-up configuration screen

| | |
|---|---|
| **Spec ID** | `flow/launcher` |
| **Revision** | 1 |
| **Status** | Draft |
| **Fidelity tier** | T3 for the launcher process itself: the port has no launcher, separate programs, `tdstatus.dat`, memory check or text-mode screens. They are replaced by the port's own shell (§4.10). T1 for what the player sees in order: the intro once before Episode 1; menu items 6 and 7 return to the menu with the game state kept |
| **Confidence** | CONFIRMED. `td1.exe`'s whole main program was read statically (`td1:0000:0208..06A5`, about 1.2 KB). Every path was observed in the oracle: intro → episode (C-0001, C-0002), status 2 → `aboutus.exe` → episode again (C-0049), status 1 → `order.exe` → episode again → quit → `order2.com` → exit (C-0050), and the configuration screen with `td.cfg` missing (C-0048) |
| **Tasks** | M1-10 |
| **Unknowns** | U-004 (launcher side, resolved here), U-005 (resolved here) |

## 1. Summary

`TD1.EXE` is the program the player types (its help names it, and so does `helpme.doc`). It is a small Borland Pascal program with the WORX sound unit, and it shows nothing of its own. In order:

1. Print a help text and stop if the first parameter is `/?` or `?`.
2. Run `nuffmem.exe` (a free-memory check) and stop if it fails.
3. Run the intro `file0000.exe`.
4. Load `intro.cmf` into memory.
5. Create `tdstatus.dat` with one zero byte if it does not exist.
6. Run Episode 1, `file0001.exe`.
7. Loop for as long as Episode 1 leaves a non-zero status byte in `tdstatus.dat`: menu item 6 leaves 2 ("about"), item 7 leaves 1 ("ordering info"). For each, play `intro.cmf` on the AdLib if music is enabled, run `aboutus.exe` or `order.exe`, stop the music and run Episode 1 again. Episode 1 restores its game and menu from the rest of the file.
8. When Episode 1 ends normally (status 0), run `order2.com` (a text-mode advertisement for the three registered episodes), set the text colour to light grey, put the cursor on row 25 and exit.

The first command-line parameter is passed unchanged to the intro and to every run of Episode 1, which act on the sound switches, `/config` and `/m` themselves. The **text-mode configuration screen** belongs to `file0001.exe`, not to the launcher (§4.9). It appears when `td.cfg` is missing or does not match the detected hardware, or on `/config`, and it writes the three bytes of `td.cfg`. **Episodes 2 and 3 are not launched by `td1.exe`** or by any other program: the player runs `TD2.EXE` and `TD3.EXE` directly (§4.8, U-005).

## 2. Definitions and units

| Term | Meaning |
|---|---|
| Parameter | `ParamStr(1)`, the first word of the command line (string up to 255). Only this word is passed on; later words are dropped |
| Exec | DOS EXEC (INT 21h 4Bh) through the BP7 `Dos.Exec`, which waits for the child to end |
| Status | Byte 0 of `tdstatus.dat` |
| No-sound flag | `td1` `DS:025C`: 1 when the parameter is `/nosound`, `/NOSOUND`, `/PC` or `/pc` |

## 3. Data

### 3.1 `td1.exe` globals (DGROUP `0817`)

| Address | Type | Name | Meaning |
|---|---|---|---|
| `DS:0052` | FileRec | `g_StatusFile` | `tdstatus.dat`, record size 1 |
| `DS:00D2` | FileRec | `g_CfgFile` | `td.cfg`, record size 1 |
| `DS:0152` | far pointer | `g_IntroCmf` | `intro.cmf` in memory, nil when the no-sound flag is set |
| `DS:0156` | u8 | `g_Status` | Status byte read from `tdstatus.dat` |
| `DS:015A` | string[255] | `g_ChildParam` | Copy of `ParamStr(1)` |
| `DS:025C` | u8 | `g_NoSound` | No-sound flag |
| `DS:025D` | u8 | `g_CfgMusic` | Byte 0 of `td.cfg` (default 1 when the file is missing) |

### 3.2 `tdstatus.dat` (launcher side, U-004)

| Writer | Content |
|---|---|
| `td1` at start-up, only if the file does not open | 1 byte: 00 |
| `file0001` at every start | truncated to 0 bytes after reading ([episode-program-flow](episode-program-flow.md) §4.6) |
| `file0001` menu items 6 and 7 | 14 bytes: status 2 or 1, kills to date, mission, menu cursor, joystick calibration, script choice ([episode-program-flow](episode-program-flow.md) §4.6). C-0049: `02 0000 00 06 FFFF 0000 FFFF 0000 01` (new game not started, cursor 6, no joystick, script 1) |

`td1` reads only byte 0, and only when the file opens and its size is not 0. A missing, unreadable or empty file counts as status 0. Status 1 runs `order.exe`, status 2 runs `aboutus.exe`, and any other non-zero value runs no extra but still re-runs Episode 1.

### 3.3 `td.cfg` (3 bytes)

| Byte | Meaning | Values |
|---|---|---|
| 0 | music | 1 none, 2 Sound Blaster / AdLib FM |
| 1 | sound effects | 1 none, 2 Sound Blaster, 3 PC speaker |
| 2 | controller | 1 keyboard only, 2 keyboard and joystick |

Only `file0001` writes it, from the configuration screen (§4.9). Readers: `file0001` (all three bytes), `file0000` (bytes 0 and 1, [intro](intro.md) §4.1) and `td1` (byte 0, only to decide whether to play `intro.cmf` under the extras). E2 and E3 share the file ([episode-deltas](../../re/episode-deltas.md) §8.3).

## 4. Behaviour

### 4.1 Main program (`td1:0000:0208..06A5`)

```text
param := ParamStr(1)
if param = "/?" or param = "?":
    write the 8-line help text to standard output, then Halt      (not ported)
Exec("nuffmem.exe", ""); if DosExitCode ≠ 0: Halt                 (not ported, OB-002)
g_ChildParam := param
g_NoSound := param in {"/nosound", "/NOSOUND", "/PC", "/pc"}
Exec("file0000.exe", g_ChildParam)                                (the intro; its exit code is ignored)
if not g_NoSound:
    WORX_Init; g_IntroCmf := WORX_LoadMusic("intro.cmf"); WORX_Shutdown
if tdstatus.dat cannot be opened:
    create it and write one byte 00
Exec("file0001.exe", g_ChildParam)
loop:
    g_Status := 0
    if tdstatus.dat opens and its size ≠ 0: g_Status := byte 0
    if g_Status = 0: leave the loop
    if not g_NoSound: WORX_Init
    g_CfgMusic := 1; if td.cfg opens: g_CfgMusic := byte 0
    if not g_NoSound and g_CfgMusic = 2 and WORX_DetectAdLib = 1:
        WORX_SetFmVolume(10, 10); if g_IntroCmf ≠ nil: WORX_PlayCmf(g_IntroCmf)
    if g_Status = 1: Exec("order.exe", "")
    if g_Status = 2: Exec("aboutus.exe", "")
    if not g_NoSound: WORX_Shutdown                               (stops the music)
    Exec("file0001.exe", g_ChildParam)
Exec("order2.com", "")                                            (not ported)
TextColor(7); GotoXY(1, 25); Halt(0)
```

Notes:

- The launcher never initialises the Sound Blaster. It only uses the AdLib for `intro.cmf`.
- The launcher installs WORX only around the extras, so no song is playing and the PIT is at 18.2 Hz while the intro and the episode run under it (they install their own WORX).
- `intro.cmf` is loaded once, right after the intro, and stays in the launcher's heap while Episode 1 runs.
- The intro runs once per `TD1` start. It does not run again after an extra.
- A `tdstatus.dat` left over with a non-zero status (for example after a reset during an extra) is not touched at start-up. The next Episode 1 run restores that state and skips the script choice.

### 4.2 Help text

Eight lines (two blank lines between some): how to configure with `/pc`, `/a`, `/sb`, `/nosound`, the auto-detect menu with `/config`, and a hint to use `/pc` or `/nosound` if the system locks up. The text names the switches the episode programs understand; `td1` itself acts on none of them except the no-sound flag. Not ported, and not mapped as a resource (command-line text, [exe-resources README](../../re/exe-resources/README.md)).

### 4.3 Running the intro

`Exec("file0000.exe", param)`. The intro reads `td.cfg` and the same sound switches ([intro](intro.md) §4.1). It never writes `td.cfg` and ignores `/config`. Two Esc presses at P1 end it after about 6 s (C-0002, C-0049, C-0050: exit at frame 426).

### 4.4 How menu items 6 and 7 come back

Episode 1's menu item 6 (about) or 7 (ordering info) writes the 14-byte record with status 2 or 1 and halts. `td1` sees the status, plays `intro.cmf` (music on in `td.cfg`, AdLib present, no no-sound switch), runs the extra (a TFP presentation player, `aboutus.tfp` or `order.tfp`; Esc ends it), stops the music and starts Episode 1 again. Episode 1 reads the record, truncates the file, skips the configuration check, script choice and joystick calibration, and opens the menu on the same item with the same game. C-0049 and C-0050 show the whole round trip:

| Event | C-0049 (item 6) | C-0050 (item 7) |
|---|---|---|
| Episode 1 writes the record and halts | f1163 | f1182 |
| `td1` reads status | 2 (f1163) | 1 (f1182) |
| `td1` reads `td.cfg` byte 0, installs WORX, starts `intro.cmf` | f1165..1173 | f1184..1193 |
| Extra | `aboutus.exe` f1174..2584 | `order.exe` f1193..1553 |
| WORX removed, Episode 1 again | f2586 | f1555 |
| Menu restored on | item 6 | item 7 |

E2 and E3 show `aboutus.scr` and `order.scr` in-process for these items, with music 69 = `intro.cmf` ([episode-deltas](../../re/episode-deltas.md) §8.3).

### 4.5 Exit

When Episode 1 quits (item 8 and Y), it has truncated `tdstatus.dat` at its start and does not write it again, so the status is 0. `td1` runs `order2.com`, a TheDraw "COM screen" that writes a text page to the screen (an advertisement for episodes alpha, beta and gamma with prices and order addresses) and returns at once (C-0050: same frame). `td1` then sets the text colour to 7, moves the cursor to (1, 25) so that the DOS prompt appears below the page, and halts with exit code 0. Not ported (MDG NG3). E2 and E3 end with a similar built-in notice ([episode-deltas](../../re/episode-deltas.md) §8.3).

### 4.6 What the extras and helpers are

| Program | Role | Port |
|---|---|---|
| `nuffmem.exe` | free-memory check; a non-zero exit code stops the launcher | not ported (OB-002) |
| `aboutus.exe`, `order.exe` | TFP presentation players for `aboutus.tfp` / `order.tfp` | open decision D4 (TFP extras) |
| `order2.com` | text-mode advertisement shown at exit | not ported (NG3) |

### 4.7 Sound switches across the three programs

| Parameter | `td1` | intro | Episode 1 |
|---|---|---|---|
| `/nosound`, `/NOSOUND` | no WORX around extras | no WORX; music 1, effects 1 | no WORX; music 1, effects 1 |
| `/pc`, `/PC` | as above | no WORX; music 1, effects 3 | no WORX; music 1, effects 3 |
| `/sb`, `/SB` | — | music 2, effects 2 | music 2, effects 2 |
| `/a`, `/A` | — | music 2, effects 3 | music 2, effects 3 |
| `/config`, `/CONFIG` | — | — | configuration screen (unless resuming) |
| `/m`, `/M` | — | — | skip the memory gate (not ported) |

In Episode 1 every sound switch also sets the controller from joystick detection, and any of them makes the configuration screen unnecessary (§4.9). The intro has no `/config`.

### 4.8 Episodes 2 and 3 (U-005)

No program in the install starts `td2.exe` or `td3.exe`. `td1.exe` has seven `Exec` sites for six programs (`nuffmem.exe`, `file0000.exe`, `file0001.exe` twice, `order.exe`, `aboutus.exe`, `order2.com`), `file0001.exe`'s only `Exec` is the hostile `label.exe` path (never ported), and E2/E3 have no `Exec` at all ([episode-deltas](../../re/episode-deltas.md) §8.3). The player runs `TD2.EXE` or `TD3.EXE` from DOS. Each does its own `td.cfg` check (and configuration screen), script choice and joystick calibration, starts at mission 1 with 0 kills, and shows no intro. Nothing carries over from another episode.

### 4.9 The configuration screen (`file0001:0000:49C1..50BD`)

**When.** At start-up Episode 1 decides whether `g_CfgMusic` (`DS:12C6`) is 0:

```text
g_CfgMusic := 0
if td.cfg opens:
    read music, effects, controller into DS:12C6..12C8
    detected := (music: WORX_DetectAdLib + 1 (0 with /nosound or /pc),
                 effects: 2 if a Sound Blaster answered else 3,
                 controller: 2 if a joystick is present else 1)
    if not resuming (status 0) and any of the three bytes differs from detected: g_CfgMusic := 0
apply the sound switch of §4.7 (sets all three bytes, so g_CfgMusic ≠ 0)
if not resuming and the parameter is /CONFIG or /config: g_CfgMusic := 0
if g_CfgMusic = 0: run the configuration screen
```

So the screen appears when `td.cfg` is missing, when the hardware no longer matches it (the shipped file says `02 02 02`, joystick included, which is why the oracle shows the screen in C-0001 and C-0002: its DOSBox has no joystick), or on `/config`. A sound switch suppresses it, and so does resuming from an extra.

**Defaults.** The screen detects again and starts from: music `WORX_DetectAdLib + 1`; effects **2 if a Sound Blaster answered, else 1** (not 3, unlike the check above); controller 2 if a joystick is present, else 1. If `MemAvail` is below 150,000 bytes, it forces effects to 1 and shows the low-memory notice `e1.startup[0]` at (27, 12) with a beep and a 5 s pause (not ported). C-0048 (no `td.cfg`, Sound Blaster and AdLib, no joystick): 2, 2, 1.

**Screen.** 80 × 25 text mode, cleared, then a 609-byte TheDraw-crunched page from `DS:0002` is unpacked into text memory. It holds the boxed title, the labels MUSIC / SOUND EFFECTS / CONTROL, the key help line, a credit line and the switch hints (C-0001 f9450, C-0048). The values are written over it at column 27 of rows 10 (music), 12 (effects) and 14 (controller), and the start item (`e1.startup[8]`) at column 28 of row 16, all 1-based. The selected field is blue (1) on light grey (7); the other values are white (15) on black and the start item is light cyan (11). Value texts are `e1.startup[1..8]` ([file0001.toml](../../re/exe-resources/file0001.toml)): music [1] none / [2] Sound Blaster-AdLib; effects [3] none / [4] Sound Blaster / [5] PC speaker; controller [6] keyboard / [7] keyboard and joystick; [8] the start item.

**Keys.** The cursor starts on field 1. One key per pass; the key bytes are reset before each read.

| Key | Effect |
|---|---|
| Space | change the current field: music 1 ↔ 2 (2 only if AdLib was detected); effects 1 → 2 (or 3 without a Sound Blaster) → 3 → 1; controller 1 ↔ 2 (2 only with a joystick). Space on field 4 accepts |
| Down arrow, `2`, Enter | next field, 4 wraps to 1. Enter on field 4 accepts |
| Up arrow, `8` | previous field, 1 wraps to 4 |
| Esc | accepts at once (it does **not** cancel) |

**Accept.** Rewrite `td.cfg` with the three working values and copy them into `DS:12C6..12C8`. C-0048: Space on music (2 → 1), Down, Space on effects (2 → 3), Esc → `td.cfg` = `01 03 01`. Afterwards a controller of 2 is checked once more against the joystick and dropped to 1 if none is present.

### 4.10 Port shell (proposal for M11)

The port is one program, so none of this is needed as such:

| Original | Port |
|---|---|
| `TD1` → intro → Episode 1 | Starting Episode 1 plays the intro first (Esc skips it, as in §4.3). E2 and E3 start without it. An Enhanced option may play the intro only on first launch |
| Configuration screen and `td.cfg` | the port's settings screens (M11-03). `td.cfg` is neither read nor written |
| Menu items 6 and 7 | in-process, as E2/E3 do: show `aboutus.scr` / `order.scr` with `intro.cmf` and return to the menu with the state unchanged. Running the TFP extras instead is decision D4 |
| `tdstatus.dat` | not needed: the state stays in memory |
| `nuffmem`, help text, `order2.com`, memory gate | not ported |

## 5. Edge cases and original bugs

| Case | Original | Policy |
|---|---|---|
| Only `ParamStr(1)` is passed on | `TD1 /sb /m` loses `/m` | n/a (T3) |
| Esc on the configuration screen | accepts and writes `td.cfg` | n/a (T3) |
| Defaults without a Sound Blaster | the screen offers "no effects" (1), while the start-up check would have accepted 3 | n/a (T3) |
| Status other than 0, 1, 2 | no extra, Episode 1 re-runs and truncates the file | n/a |

## 6. Test vectors

| # | Setup | Expected | Evidence |
|---|---|---|---|
| 1 | Pristine install, no input | `nuffmem` f8, intro f8..f9353, `intro.cmf` loaded, `tdstatus.dat` created with `00`, `file0001` f9357; configuration screen (joystick mismatch) | C-0001 `int21.tsv`, fb f9450 |
| 2 | `td.cfg` deleted | intro and episode both fail to open it; screen defaults 2, 2, 1; after Space, Down, Space, Esc: `td.cfg` = `01 03 01` | C-0048 (RAM 4FEB:060A..060D after each key; `int21.tsv` writes) |
| 3 | Menu item 6 | record `02 0000 00 06 FFFF 0000 FFFF 0000 01`; `aboutus.exe`; Episode 1 again with cursor 6 | C-0049 |
| 4 | Menu item 7, then quit | status 1 → `order.exe` → Episode 1 (cursor 7) → quit → `tdstatus.dat` size 0 → `order2.com` → `td1` exit code 0 at f2035 | C-0050 |

## 7. Presentation and audio hooks

`intro.cmf` plays under the extras (FM volume 10, 10) and stops when the extra ends. The launcher draws nothing itself.

## 8. Open questions

None for the port. The TFP extras are decision D4.

## 9. Provenance

| Claim | Address / capture | Method | Confidence |
|---|---|---|---|
| Main program, help, nuffmem, Exec sequence, loop, exit | `td1:0000:0208..06A5` | static + C-0001, C-0049, C-0050 | CONFIRMED |
| WORX wrappers in `td1` (smart-linked: Init `00CD:0238`, LoadMusic `0160`, PlayCmf `020B`, DetectAdLib `0141`, SetFmVolume `010A`, Shutdown `0055`) | driver function numbers 26h, 17h/18h, 1Eh, 23h, 22h | static | CONFIRMED |
| `Dos.Exec`, `DosExitCode` | `td1:070D:0000`, `070D:0094` | static | CONFIRMED |
| Configuration decision and screen | `file0001:0000:49C1..50BD` | static + C-0048 | CONFIRMED |
| No program starts E2/E3 | all `Exec` sites of the five game programs | static | CONFIRMED |

## 10. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-05 | RE-13 (Archaeologist + Spec author + Verifier) | Initial: launcher flow, `tdstatus.dat` launcher side (U-004), E2/E3 launch (U-005), configuration screen, port shell proposal |
