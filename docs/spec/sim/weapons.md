# Weapons, projectiles and damage

| | |
|---|---|
| **Spec ID** | `sim/weapons` |
| **Revision** | 4 |
| **Status** | Draft |
| **Fidelity tier** | T1 (projectiles, hits, damage, kills and the RNG calls of hit explosions; the two-digit muzzle bytes of §3.3) · T2 for the drawing of shots |
| **Confidence** | CONFIRMED for player volleys, missiles (kind 4), Tab, the cooldown and shot movement/expiry against map tiles (C-0011, main-session VE, two corrections in rev 2); CONFIRMED for the two-digit muzzle bytes of §3.3 (every writer read statically; 20 whole flights of VE-2 plus C-0002 and C-0160 reproduced byte for byte); LIKELY for hits, damage, homing and enemy volleys (static) |
| **Tasks** | M6-01 (follow-up), feeds M7-02, M7-03, M7-05; M9-11, M9-16 |
| **Unknowns** | U-041 and U-043 (resolved here), U-040 (resolved in mission-ships rev 2), U-061, U-160 and U-391 (resolved here, RE-15), U-440, U-441 |

## 1. Summary

All shots, of the player and of the 20 mission ships, live in one pool of 51 projectile slots. A shot has a map tile, a pixel offset, a heading, a kind and a life counter. **Shots move one whole tile (32 px) per tick** along one of the eight principal directions and die after 25 ticks, when they leave the map, or when they enter a solid tile. Each tick every shot is tested against the 20 mission ships and against the player with a square hit box of ±19 px. Damage depends only on the shot kind; shields absorb first and the rest goes to armour. A ship whose armour reaches 0 is destroyed: its class code becomes FFh and, if the player fired, the mission kill counter increases. The player fires with **Space** (primary weapon every second tick while held, or one missile per press) and switches between primary and missiles with **Tab**.

## 2. Definitions and units

| Term | Original | Type | Notes |
|---|---|---|---|
| Projectile slot | index 0..50 into the eight heap arrays (structures §5) | — | Slot free when tile X = FFh |
| Tile X, Y | DS:3986 / DS:398A arrays | u8 | Map column / row |
| Offset X, Y | DS:3976 / DS:397A arrays | i16 | Pixels added to tile × 32 (not kept inside 0..31) |
| Heading | DS:397E array | u8 | Keypad code; only 1, 2, 3, 4, 6, 7, 8, 9 move, anything above 9 deletes the shot |
| Kind | DS:3982 array | u8 | 0..3 cannon/pulser classes, 4 and 5 missiles, 6 homing missile |
| Life / target | DS:3972 array | u8 | Life counter for kinds 0..5; **target slot** for kind 6 |
| Owner | DS:396E array | u8 | 0 = player, 1..20 = mission slot |
| Weapon | `g_Weapon` DS:1CB8 | u8 | Player's selected weapon 1..6 (§3.2) |
| Missiles | `g_Missiles` DS:1CB9 | u8 | Missiles left (HUD bar, max shown 26) |
| Missile cooldown | `g_MissileCooldown` DS:1CBA | u8 | Set to 4 on a launch; −1 per tick **before** the fire keys are handled, so it reads 4, 3, 2, 1, 0 in the ticks after a launch and the next launch can come 4 ticks later (C-0011; rev 1 had the decrement after the launch) |
| Difficulty | DS:13DE | u8 | Multiplier of damage to the player; 1 at flight start (cheat chords change it, globals §5) |

World pixel of a shot = tile × 32 + offset (per axis). World pixel of a mission ship = slot tile × 32 + slot offset (DS:3ACA/3AF2). The player's hit reference = `((viewX + 5) × 32 + viewSubX, (viewY + 3) × 32 + viewSubY)`.

## 3. Data

### 3.1 Shot table (DS:02E5 + 17 × class + m, m = 1..17) and damage

| Class | Muzzles m with an entry | Entry (kind + 1) | Kind | Shots per volley | Damage to a ship per shot |
|---:|---|---:|---:|---:|---:|
| 0, 1 | none | — | — | 0 | — |
| 2 | 1, 2, 4, 5 | 1 | 0 | 4 | 5 |
| 3 | 8, 9 | 1 | 0 | 2 | 5 |
| 4 | 6, 7 | 2 | 1 | 2 | 10 |
| 5 | 3 | 2 | 1 | 1 | 10 |
| 6 | 10, 11 | 3 | 2 | 2 | 17 |
| 7 | 12, 13 | 2 | 1 | 2 | 10 |
| 8 | 14, 15, 16, 17 | 1 | 0 | 4 | 5 |

Damage to a ship by kind k: `(k + 1)² + 4 + 2k` → k = 0: 5, 1: 10, 2: 17, 3: 26, 4: 37, 5: 50; kind 6 destroys outright. Damage to the player: `2 × (k + 1) × difficulty` → with difficulty 1: kind 0: 2, 1: 4, 2: 6.

The volleys agree with the class chart's armament lines (truck and transport unarmed, four light pulsers for classes 2 and 8, two weapons for 4 and 7), which supports the class names in structures §6.

Muzzle offsets (dx, dy) per heading h and muzzle m are signed bytes at DS:035C + 34h + 2m (h = 1..9; heading 5 is unused). For example heading 8, muzzles 6 and 7: (7, 11) and (22, 11); heading 6, muzzles 1, 2, 4, 5: (16, 25), (16, 9), (31, 19), (31, 15). The address is computed for every heading the volley has: 34 × heading in 16 bits, plus 2m, plus 035Ch for dx and 035Dh for dy, each byte sign-extended. Two-digit headings therefore read other globals far past the table. Both the player and the mission ships fire on them (a volley uses the shooter's current heading, and a turning ship passes through the two-digit headings). §3.3 gives the exact bytes (OB-017, U-160).

### 3.2 Player weapons per class (set at flight start, `0E8B:C751..C86C`)

| Class | Primary weapon | Missile weapon (Tab) | Missiles at start |
|---:|---:|---:|---:|
| 0, 1 | (unchanged: see below) | none | 0 |
| 2 | 1 | 4 (kind 4) | 6 |
| 3 | 1 | 5 (kind 5) | 20 |
| 4 | 2 | 4 (kind 4) | 4 |
| 5 | 2 | 6 (homing) | 1 |
| 6 | 3 | 5 (kind 5) | 6 |
| 7 | 2 | 2 (same as primary) | 0 |
| 8 | 1 | 1 (same as primary) | 0 |

Weapons 1..3 mean "fire the class volley" (the weapon number only selects the HUD icon from `td.wea`); weapons 4..6 fire one missile of kind 4, 5 or 6.

**`g_Weapon` across flights (U-391, RE-15).** `g_Weapon` is an uninitialised global, so it is **0 at program start**. It has exactly two kinds of writers, both inside `Sim_FlyMission`: the flight setup above (`0E8B:C751..C7AA`, classes 2..8 only) and the Tab handler of §4.1 (`0E8B:F535..F645`, classes 2..8 only). Nothing else writes it: not the front end, not a save or restore, not `tdstatus.dat`, not the flight-19 vehicle switch (`0E8B:A185`, which sets the class to 8 and leaves the weapon alone). So for classes 0 and 1 the weapon is the value the previous flight **of the same program run** left, whatever that flight was:

- the flight before it in the campaign. In E1 the only class-0/1 player flight is 19, so normally that is flight 18 (class 2): weapon 1, or 4 if the player ended flight 18 with the missiles selected and missiles left;
- a retry of the same flight: the value that flight left (flight 19 never writes it, so it stays);
- **0** if no flight has been flown since the program started. This covers a direct start (C-0069), a saved game restored at mission 19 as the first flight, and a run started again by the launcher after "ordering info" or "about" (`td1.exe` runs the episode again: a new program).

The port's `Session` must carry `weapon: u8`, set to 0 when the program starts and copied back from the flight state at the end of every flight, whatever its outcome (success, death, abort or flight 21). td-sim's start value of 1 is BUG-2. What the weapon does with class 0 or 1: Space fires nothing (no shot-table entry, no missiles); with weapon > 3 the Tab rule of §4.1 clears the Tab byte every tick; after the flight-19 switch to class 8 the carried weapon stays in force (0 fires the class-8 volley, like 1–3). The HUD icon for weapon 0 reads before the icon block ([hud](../render/hud.md) §4.9, OB-090).

### 3.3 Muzzle bytes on two-digit headings (U-160, OB-017)

**Rule.** For a volley on heading h, the offsets of muzzle m are the signed bytes at DS:(035Ch + 34h + 2m) and DS:(035Dh + 34h + 2m) of the program's DGROUP at the moment of the volley (§3.1). For h = 1..9 that is the muzzle table. For the eight two-digit headings it is the following globals of E1 (m = 1..17 reads the 34 bytes from the first address, dx at even offsets from it, dy at odd):

| Heading | Bytes read | What is there (E1 `file0001`) | Value during a flight |
|---|---|---|---|
| 0Ch | DS:04F6..0517 | 04F6..0501: System typed constants `SegA000`, `SegB000`, `SegB800` (words A000h, B000h, B800h), `Test8086`, `Test8087`, `FileMode`, a padding byte, `k_CrLf` (0Dh 0Ah). 0502: `g_SbIrq` (word). 0504..: `g_MemAvailText` (`string[255]`, length byte then text) | constants; `g_SbIrq` and `g_MemAvailText` from start-up and the last main-menu load (below) |
| 0Eh | DS:053A..055B | `g_MemAvailText` characters 54..87 | always 0 |
| 20h | DS:079E..07BF | `g_File` (FileRec at DS:075A), name bytes 20..53 | always 0 |
| 24h | DS:0826..0847 | `g_MenuPalette` bytes 70..103 | bytes 70..103 of `tdmen.pal` |
| 4Ah | DS:0D32..0D53 | `g_Palette` (the scene palette, DS:0D2E) bytes 4..37 | scene palette (below) |
| 4Eh | DS:0DBA..0DDB | `g_Palette` bytes 140..173 | scene palette |
| 60h | DS:101E..103F | `g_Palette` bytes 752..767 (m = 1..8); `g_ScreenLit` DS:102E and DS:102F..1031 (m = 9, 10); `g_FontTitle` DS:1032..1035 (m = 11, 12); the dialogue reader's code byte DS:1036, DS:1037..1039, its field bytes DS:103A..103C, DS:103D, and the first two bytes of its text buffer DS:103E (m = 13..17) | scene palette, front-end state (below) |
| 62h | DS:1062..1083 | the dialogue reader's text buffer `g_DlgText` (DS:103E, `string[255]`), characters 36..69 | front-end state (below) |

**None of these bytes is written while a flight runs.** No routine of the flight unit `0E8B`, the mission units `2404`/`3292`/`34E6`, the transmission loaders `2A15` or the sound unit writes them, and the flight unit calls nothing in the front-end units `0000` and `051A` (static, the full xref of every operand in these ranges). VE-2's 20 E1 flights (C-0051..C-0069, C-0090: 17,563 ticks, every transmission screen, the flight-17 cut-scene, the flight-19 V.C.I. screen) keep all 272 bytes constant from tick 1 to the end. So the port computes them once, before the flight starts, from the state described below, and hands them to td-sim with the muzzle table (td-sim already takes the 3,402 DGROUP bytes from DS:035C as its muzzle world, `--alt-muzzles`).

**Where each value comes from.** Everything is set by the program start or by the front end before the flight.

1. **Machine and runtime constants.** `SegA000`/`SegB000`/`SegB800` are the Borland runtime's fixed segment constants. `Test8086` is 2 on any 386 or later (written once by the runtime's CPU test). `Test8087` keeps its initial 0 and `FileMode` its initial 2: no code writes either. The padding byte DS:04FF is 0. `g_SbIrq` is set once at start-up by the WORX Sound Blaster probe: the IRQ number (7 on the reference preset P1, SB16 at IRQ 7), or FFFFh with no card, `/nosound` or `/pc`. **Port: 7** (the reference machine, as OB-013 does for the IVT bytes).
2. **`g_MemAvailText`** is `Str(MemAvail)`, written each time the main menu is loaded (`UI_LoadMainMenu` `0000:2E88`; it is drawn in ink 0, so nobody sees it). On the reference machine it is `203352` (6 digits) after a direct start (C-0051..C-0090) and after a start through `td1.exe` (C-0002): the MZ header limits the memory the program takes beyond its image to 4A80h paragraphs, which both layouts can give in full, so the value does not depend on where DOS loads the program. Only the length byte and up to 6 digits are ever written; every byte after them stays 0. **Port: the length 6 and the six digits `203352`**, the rest 0. Only classes 3 and 6 read the digits (muzzles 8..11 on heading 0Ch); whether the value stays the same later in a long campaign is U-440.
3. **`g_File.Name`**: every name ever assigned to it (`td.men`, `tdmen.pal`, `tdstatus.dat`, `td.cfg`, `dialogue.dat`) is at most 12 characters long, so name bytes 20..53 are never written: 0.
4. **`g_MenuPalette`** is loaded from `tdmen.pal` by every main-menu load. The menu changes only entries 224..233 (the item highlights); the help screen saves and restores the whole buffer. A flight is always preceded by the main menu, so bytes 70..103 are `tdmen.pal` bytes 70..103.
5. **The scene palette `g_Palette`** (768 bytes, 0 at program start) is the palette buffer of the front-end screens: every scene card, still, hint screen, briefing, dossier, debrief, retry prompt and save or restore screen reads its 768-byte palette into it, the screens then change entries in it as their specs say (single entries: [text](../render/text.md) §4.5 "entry n = (r, g, b)" and [screens](../flow/screens.md); the story-so-far pages scale entries 0..250 by 6 div 9, [screens](../flow/screens.md) §3.8), and the dialogue player overwrites entries 0..111 and 112..223 with the two portraits, the speakers' text entries from 250 on, and entry 254 for the scene-1 prompt ([dialogue-player](../narrative/dialogue-player.md) §4.4, §4.6). The script-choice screen and the missing-dialogue notice clear it and set entry 1 to (10, 10, 10) and three other entries. Nothing resets it between screens or between flights, and the flight uses its own palette (`g_FlightPalette` DS:13E6), so the flight reads **whatever the last front-end screen before it left**. Neither the hit flash nor the infra-red modes nor the in-flight screens touch it. In-place writes that fall inside the three ranges read (bytes 4..37, 140..173, 752..767), besides the palette loads and the dialogue player: entry 1 (script choice, missing-dialogue notice), entry 12 (`velsroom` card, bytes 36..37), entries 0..250 (story so far: × 6 div 9), entry 250 (auditorium card, byte 752), entry 254 (story so far: bytes 763..764 := 63), entry 255 (`marauder`, `helihang`, `eggbase` cards, the `td1-13`/`td1-14` stills). After a direct start through `tdstatus.dat` with Esc at the mission card no screen has loaded it, so the three ranges are all 0 (C-0051..C-0090).
6. **`g_ScreenLit`** (1 after a scene fade-in, 0 after a scene fade-out) is 0 at the start of every flight observed (every pre-flight sequence ends with a fade-out; C-0002, C-0051..C-0090, C-0160). DS:102F..1031 and DS:1037..1039 and DS:103D are never written: 0.
7. **`g_FontTitle`** is the far pointer to the title-font block (`td-let.cin`), allocated anew by every text card (`Flow_TextCard` `051A:5852`; every mission block starts with one, the mission title) and by the script choice; freeing it leaves the pointer as it was. Its offset word is 0008h. Its segment word is the heap address of the block, which depends on where DOS loaded the program: on the reference machine it is DGROUP + 192Bh, that is **6916h** when the game is started through `td1.exe` (C-0002; DGROUP 4FEBh) and **5D92h** when `FILE0001.EXE` is started directly (C-0051..C-0090, C-0160; DGROUP 4467h). **Port: 6916h for a game played through the launcher flow, 5D92h for a direct start** (the oracle captures). Only class 6 (muzzle 11: offset (8, 0)) and class 7 (muzzle 12: the segment bytes) read it, on heading 60h.
8. **The dialogue reader** (`Dlg_PlayScene` `051A:92AF`) keeps its line state in DGROUP: DS:1036 = the code byte of the last line it read (during the search for the scene it holds the first byte of each line read and then the scene number; inside the scene the line codes of [dialogue-player](../narrative/dialogue-player.md) §3.1; an empty line reads as 13); DS:103A..103C = the fields of the last `01`, `02` or `06` line as stored (`01`: slot, face after the remap of §3.3 there, then DS:103C := the text entry allocated; `02`: the ignored byte, the colour; `06` inside a scene: its id at DS:103A); DS:103E = `g_DlgText`, the name of the last `01` line or the text of the last `02` line, read as `string[255]` (length byte, then the characters; a longer earlier string survives after the end of a shorter one). At the end of a scene played to its `d` line DS:1036 = 64h ('d'); when Esc ends a scene at a `05` wait it is 05h and the lines after that wait are never read. The flight reads this state as the last dialogue scene of the program run left it; 0 everywhere if no dialogue scene has been played (a direct start).

**Construction (for the Implementer).** td-sim's muzzle world is the byte vector of DGROUP from DS:035C (index = DS address − 035Ch, 3,402 bytes cover heading 98 + muzzle 17). Indices 0..341 are the muzzle table of the executable; outside the table only the 272 bytes below are ever read, and the rest may be anything. At each flight start fill them as follows (`scene` = the session's 768-byte scene palette, `dlg` = the reader's 256-byte text buffer, byte 0 = length):

| DS range | Bytes |
|---|---|
| 04F6..0501 | 00 A0 00 B0 00 B8 02 00 02 00 0D 0A |
| 0502..0503 | 07 00 (`g_SbIrq` of the reference machine) |
| 0504..0517 | 06 32 30 33 33 35 32, then 13 × 00 (`g_MemAvailText` = `203352`) |
| 053A..055B, 079E..07BF | 00 |
| 0826..0847 | `tdmen.pal` bytes 70..103 |
| 0D32..0D53 | `scene` bytes 4..37 |
| 0DBA..0DDB | `scene` bytes 140..173 |
| 101E..102D | `scene` bytes 752..767 |
| 102E..1031 | lit flag, 00, 00, 00 |
| 1032..1035 | 08 00, then the title-font segment low, high (6916h through the launcher flow, 5D92h after a direct start) |
| 1036..103F | reader code byte, 00, 00, 00, the three field bytes, 00, `dlg` bytes 0 and 1 |
| 1062..1083 | `dlg` bytes 36..69 |

The session keeps `scene`, the lit flag, the reader's code byte, field bytes and `dlg` from program start (all 0) for the whole program run, updated exactly as items 5–8 say; a direct start (tdstatus resume, Esc at the mission card) leaves them 0.

**Which shooters read what.** Only muzzles with a shot-table entry fire (§3.1). E1 player classes 2, 3, 4, 8 (and 0 in flight 19, which has none); E1 mission ships of classes 2..8.

| Class (muzzles) | 0Ch | 0Eh, 20h | 24h | 4Ah, 4Eh | 60h | 62h |
|---|---|---|---|---|---|---|
| 2 (1, 2, 4, 5) | (0, −96), (0, −80), (2, 0), (2, 0) | (0, 0) | `tdmen.pal` | scene palette | scene palette | text |
| 3 (8, 9) | (6, 50), (48, 51): length and digits of `g_MemAvailText` | (0, 0) | `tdmen.pal` | scene palette | scene palette; `g_ScreenLit`, 0 | text |
| 4 (6, 7) | (13, 10), (`g_SbIrq` low, high) = (7, 0) | (0, 0) | `tdmen.pal` | scene palette | scene palette | text |
| 5 (3) | (0, −72) | (0, 0) | `tdmen.pal` | scene palette | scene palette | text |
| 6 (10, 11) | (51, 53), (50, 0): digits | (0, 0) | `tdmen.pal` | scene palette | 0; (8, 0) | text |
| 7 (12, 13) | (0, 0), (0, 0) | (0, 0) | `tdmen.pal` | scene palette | (`g_FontTitle` segment low, high), (DS:1036, 0) | text |
| 8 (14..17) | (0, 0) × 4 | (0, 0) | `tdmen.pal` | scene palette | (0, 0), (DS:103A, DS:103B), (DS:103C, 0), (length, first character of `g_DlgText`) | text |

So the four southern and western intermediate headings (0Ch, 0Eh, 20h, 24h) read constants on the reference machine, and the four northern ones (4Ah, 4Eh, 60h, 62h) read the scene palette and the dialogue state that the front end left.

**Test vectors.** (a) Direct start, any E1 flight (C-0051..C-0069, C-0090): all 272 bytes identical in all 20 captures and constant in every tick; 4Ah, 4Eh and 62h all 0; 60h is 0 except DS:1032..1035 = 08 00 92 5D. (b) Start through `td1.exe`, mission 1 played with Space, Space, Esc in dialogue 1 and the hint screen (C-0002): the scene palette is `secret.pic`'s palette with entry 240 := (63, 10, 10) (the hint screen), DS:1032..1035 = 08 00 16 69, and DS:1036..113D equal a model of the reader that plays dialogue 1 from its start and stops at the third `05` wait, byte for byte (DS:1036 = 05h). (c) C-0160 (mission 3 started directly with the story played to the end): see [§6](#6-test-vectors) vector 7. (d) A class-7 volley on heading 60h in a direct start leaves at offsets (−110 + subX − 16, 93 + subY − 16) for muzzle 12; through `td1.exe`, (22 + subX − 16, 105 + subY − 16).

**How much it matters.** A two-digit volley lives for exactly one hit test (§4.3 steps 3–5, then step 8 deletes it), but with these offsets that test happens up to about 4 tiles away from the shooter. In VE-2's 20 replays (17,563 ticks) the player fired 292 shots and the mission ships 230 shots on two-digit headings. With the offsets of policy (0, 0) instead of the original's, VE-2's first plans for flights 2, 3, 6, 8, 15 and 16 took another course, and flights 8, 15 and 16 ended later. With the bytes of the start through `td1.exe` (C-0002) instead of a direct start's, 10 of the 20 replays (flights 1, 4, 5, 8, 9, 10, 11, 16, 17, 18; all planned so that the direct start's bytes and (0, 0) give the same flight) take another course with the same inputs. Example: flight 1, tick 117: the player's volley on heading 62h kills truck 3 at (124, 79) after a direct start and misses after the start through `td1.exe` (`tdtool replay --alt-muzzles`). The bytes therefore change outcomes often, and they differ between a direct start and a campaign.

**Port policy (OB-017): preserve** (proposed; the coordinator or the user decides). The bytes are a deterministic function of three reference-machine constants (`g_SbIrq` 7, `g_MemAvailText` `203352`, the `g_FontTitle` segment 6916h or 5D92h) and of front-end state the port already has to model exactly for its screens: the scene palette, the main-menu palette and the dialogue reader's last line. The port keeps a small "legacy front-end state" in its session for the whole program run (the 768-byte scene palette exactly as the front-end screens and the dialogue player leave it, the lit flag, the reader's code byte, field bytes and 256-byte text buffer, and the launch kind) and builds the 272 bytes from it at every flight start. Two weaker alternatives, if that is judged too costly: (B) a fixed block equal to the direct-start bytes of test vector (a): exact for every direct start and for headings 0Ch, 0Eh, 20h, 24h always, wrong in campaigns on the four northern headings; (C) offset (0, 0) for all two-digit headings: wrong on all eight (10 of 20 replays and 6 of VE-2's first 20 plans would change). E2 and E3 read their own DGROUP from DS:04BA (U-441); this section is E1 only.

## 4. Behaviour

### 4.1 Player input (tick-order steps 33–34, `0E8B:B1EC` and `0E8B:F4F8..F6A7`)

```text
Tab (scan 0Fh) pressed, or the missile weapon selected (weapon > 3) and no missiles left:
    clear the Tab byte
    if weapon > 3 and class > 1:            weapon := primary weapon of the class (3.2)
    elif weapon <= 3 and class > 1 and missiles > 0: weapon := missile weapon of the class
    redraw the weapon icon
M (scan 32h) and the HUD ticker idle:        restart the ticker (replays the current message)
Alt or right Alt, palette mode 1 -> 2; or palette mode 2 -> 1 (never from mode 0; not after the
    mission is over): infra-red view on/off (palette only, no game effect); clears the Alt bytes
Space or joystick button 2:
    if weapon > 3 and missiles > 0 and cooldown = 0 and not over:   launch one missile (4.2)
    if weapon <= 3 and not over and the back page is 1 (every second tick):  fire a volley (4.2)
```

Holding Space fires a volley every second tick (the Space byte is not cleared); a missile clears the Space byte, so each missile needs a new key press (or the keyboard's auto-repeat) and at least 4 ticks.

### 4.2 Spawning

```text
player volley:  for m = 1..17 with shot_table[class][m] != 0:
    slot := highest free slot in 8..48 (none -> skip this muzzle)
    life := 1; tile := (viewX + 4, viewY + 3); owner := 0; heading := player heading
    kind := shot_table[class][m] − 1
    offsetX := 4 × round(viewSubX / 4) − 16 + muzzle[heading][m].dx      (rev 2: the −16 is observed, C-0011)
    offsetY := viewSubY − 28 + muzzle[heading][m].dy
    sound: kind = class (or 2 while DS:1703 is set), at the view
player missile: slot := lowest free slot in 40..50 (none: the original writes slot 200, out of the
    array; port: skip, see 5)
    life := 0; kind := weapon (4, 5 or 6); tile := (viewX + 4, viewY + 3); owner := 0
    heading := player heading; offsetX := viewSubX − 2; offsetY := viewSubY − 12
    if weapon = 6: target := the live mission slot (class != FFh) with the smallest squared tile
        distance to (viewX + 4, viewY + 3), ties to the lower slot; stored in the life field.
        No live ship: target stays 0 and the missile is downgraded to kind 0
    missiles −= 1; cooldown := 4; sound kind 10
mission ship volley: see tick-order step 31 (highest free slot in 0..40, life 1, tile = ship tile,
    offset = muzzle + ship offset − 16, kind = shot table entry − 1, owner = slot)
```

### 4.3 Per-tick update (`0E8B:0984`, tick-order step 6)

For each projectile slot i = 0..50 in order, if its tile X ≠ FFh:

```text
1. if tile X or tile Y < 2 or > 199: free the slot
2. if kind != 6: life += 1; then, unless (DS:1703 set and owner = 0), test the map:
     heading with a vertical part (8, 2, 9, 1, 3, 7):
         solid(tileX, tileY) and offsetX < 17  -> free
         solid(tileX + 1, tileY) and offsetX > 16 -> free
     heading with a horizontal part (4, 6, 9, 1, 3, 7):
         solid(tileX + 1, tileY − 1) and offsetY < 17 -> free
         solid(tileX + 1, tileY) and offsetY > 16 -> free
3. kind 6: for each live mission slot s != owner with |ship − shot| < 40 px on both axes:
     free the slot; mission kills += 1; white flash; ship armour := 0, shields := 0;
     explosion at the ship
4. unless (flight 20 and owner = 1): for each live mission slot s = 1..20, s != owner, with
   |ship − shot| < 20 px on both axes:
     free the slot; hit sound (kind 9, at the ship)
     d := damage(kind) (3.1)
     if d < shields(s): shields(s) −= d
     else: armour(s) := armour(s) − d + shields(s); shields(s) := 0
     if armour(s) < 1: destruction sound (kind 100); distance flash (0E8B:06C5); armour(s) := 0
     if kind = 6: white flash; armour(s) := 0; shields(s) := 0
     if shields(s) > 60 or < 0: shields(s) := 0
     if armour(s) = 0:
         kind[s] := 1                     (original bug: writes the projectile kind array at index s)
         if owner = 0: mission kills += 1
         class(s) := FFh; five explosions around the ship (tiles and order: flow/flight-screens §4.7)
     else: one small explosion at ship offset + Random(16) on each axis (X first, then Y);
           only if an explosion slot 1..20 is idle: with all twenty busy, neither Random call is made
5. if owner != 0 and DS:1703 = 0 and |player reference − shot| < 20 px on both axes:
     free the slot; hit sound (kind 9)
     if difficulty != 0: apply 2 × (kind + 1) × difficulty to shields, then armour, as in step 4
     if armour < 0: armour := 0;  if shields > 60 or < 0: shields := 0
     explosion at the player: offset view + Random(8) − 8 on each axis (X first, then Y);
           again only if an explosion slot 1..20 is idle (no Random calls otherwise)
6. kind 6: heading := principal direction from the shot's tile toward the target slot's tile
     (both axes differ: 3, 9, 1 or 7; same column: 2 if the target is below, else 8;
      same row: 6 if the target is to the right, else 4; DS:1CBC keeps the last value)
   draw the shot (kinds 0..3 as short lines in colours 3Ch/3Dh/35h/BEh, kinds 4..6 as sprites)
7. if life > 25: free the slot
8. if still alive: move one tile along the heading (8: y−1, 2: y+1, 4: x−1, 6: x+1, 9: y−1 x+1,
   7: y−1 x−1, 1: y+1 x−1, 3: y+1 x+1); a heading above 9 frees the slot
```

The tests in steps 3–5 use the positions before this tick's move; a shot can therefore pass through a target that is less than 20 px wide in its path between two tile steps.

## 5. Edge cases and original bugs

| Quirk | Original | Port policy (proposal) |
|---|---|---|
| Missile with no free slot | Writes slot 200 (outside the 51-byte arrays) | Fix (skip the launch); undefined in the original |
| Homing launch writes life 200 into slot 40 | Slot 40 (if in use) dies next tick | Preserve |
| Destroyed ship sets projectile kind[s] := 1 | Changes the kind of projectile slot s | Preserve (T1) |
| Homing missile with target 0 reads slot-0 arrays (DS:3B2D/3B41) | Steers toward garbage | Cannot happen (downgraded to kind 0) unless the target dies; then it keeps homing on the dead ship's last tile |
| Two-digit headings | Shot deleted on its first move | Preserve |
| OB-017: two-digit headings read past the muzzle table | Offsets from other globals: constants, the scene palette and the dialogue reader's last line (§3.3) | **Preserve** (proposed, §3.3): the port builds the 272 bytes at flight start from its front-end state and three reference-machine constants |
| `g_Weapon` for classes 0 and 1 | Kept from the previous flight of the program run; 0 at program start (§3.2) | Preserve: the session carries it (BUG-2) |
| Tile-step movement | Shots jump 32 px per tick | Preserve |
| Flight 20, owner 1 | Slot 1's shots never hit ships | Preserve (mission rule) |

## 6. Test vectors

To be captured (§8). Static examples:

| # | Setup | Expected |
|---|---|---|
| 1 | Class 2 player, heading 8, Space held for 4 ticks | 2 volleys of 4 shots (every second tick), kinds 0, slots 48, 47, 46, 45 first |
| 2 | Kind 1 shot hits a class 7 ship with shields 25 | shields 15, armour unchanged |
| 3 | Kind 0 shot hits the player (difficulty 1, shields 1, armour 25) | shields 0, armour 24 |
| 4 | C-0011: class 4, heading 8, view sub (32, 16), Space held from tick 25 | volleys at ticks 26, 28, 30, … (back page 1 only) into slots 48/47 then 46/45; offsets (23, −1) and (38, −1) (muzzles 6, 7: dx 7 and 22); each shot moves one tile up and is freed by the map test two updates later |
| 5 | C-0011: Tab, then Space taps (weapon 4, 4 missiles) | missile in slot 40 at offsets (30, 4), missiles 4 → 3, cooldown 4, 3, 2, 1, 0; the held Space byte is cleared by the launch |
| 6 | C-0011: heading 6, Space held | volleys at offsets (36, −3) and (36, 12) (muzzles 6, 7 of heading 6: (20, 9), (20, 24)) |
| 7 | C-0160: mission 3 started directly, title card, hangar card and dialogue 11 played to its `d` line, then flight 3 | the 272 bytes of §3.3 constant over ticks 1..80; scene palette = `hangar.scr`'s palette with entries 0..111 and 112..223 replaced by the two portraits of dialogue 11 and entries 250, 251 := `td.pal` of the two speaker colours (all 768 bytes match); DS:1036..113D match a model of the reader that plays dialogue 11 to its end (DS:1036 = 64h); DS:1032..1035 = 08 00 92 5D; heading 0Ch, 0Eh, 20h, 24h bytes as in every direct start |
| 8 | `g_Weapon` at flight 19 started directly (C-0069) | 0 after setup and to the end of the flight, the switch to class 8 included |

## 7. Presentation and audio hooks

Sounds through `1E2A:006A`: player volley (kind = class, or 2), missile (10), hit (9), destruction (100), enemy volleys (kind = shooter class). Flashes: white (kind 6 hits), red distance flash `0E8B:06C5` = `max(flash, 20 − round(sqrt(distance)))` with Real48 `Sqrt` (presentation only). Full rules for the distance flash (argument order, distance from the player's tile, 16-bit square, decay in step 14, all callers): [sim/hit-flash](hit-flash.md).

## 8. Open questions

- [ ] Q1 (VE): capture a volley, a missile launch and a ship kill in E1M1 (class 4) with per-frame dumps of the projectile arrays (heap; follow the far pointers at DS:396E..398A) and the ship records.
- [x] Q2: explosion tiles, offsets, slot choice and drawing: [flow/flight-screens](../flow/flight-screens.md) §4.7 (RE-8); shot drawing §4.8. The `06C5` distance formula is in §7.
- [x] Q3 (U-040): answered in [mission-ships §4.4.2](mission-ships.md): bytes at DS:035C + 34 × heading + 2m (other globals); port policy U-160.
- [x] Q4 (U-160): the exact bytes and where they come from, §3.3 (RE-15). The policy itself (preserve, proposed) is the coordinator's or the user's to accept.
- [ ] Q5 (U-440, VE): in a long campaign, do `g_MemAvailText` and the `g_FontTitle` segment stay at their first values (they would change only if the heap were left in another state at a later main-menu load or mission title)? Capture a campaign through at least two flights.
- [ ] Q6 (U-441, RE): the same mapping for E2 and E3 (muzzle table DS:04BA; the eight ranges are E1's + 15Eh but hold other globals; in E3 heading 62h muzzles 16 and 17 read DS:11DE and DS:11E0, which change during a flight, C-0036).

## 9. Provenance

| Claim | Address | Method | Confidence |
|---|---|---|---|
| Player input, Tab, M, Alt, Space | `0E8B:B1EC..B78x`, `0E8B:F4F8..F6A7` | static | LIKELY |
| Weapons and missiles per class | `0E8B:C751..C86C` | static | CONFIRMED (table code) |
| Shot table and muzzle table | DS:02E5, DS:035C (file bytes) | static | CONFIRMED (values) |
| Update order and movement | `0E8B:0984..1AxX` | static | LIKELY |
| Damage formulas | `0E8B:0984` (ships), same routine (player) | static | LIKELY |
| Enemy volleys | tick-order step 31 | static | LIKELY |
| Muzzle address 34 × heading + 2m + 035Ch/035Dh, sign-extended, for every heading | `0E8B:F429..F466` (ships), `0E8B:B72B..B756` (player) | static; VE-2 C-0051..C-0090 (with the capture's DGROUP bytes td-sim matches every two-digit volley) | CONFIRMED |
| Contents and writers of the eight two-digit ranges (§3.3) | `0000:2E88` (menu palette, `g_MemAvailText`), `0000:02AF`, `0000:4526` (`g_SbIrq`), `3A9A:00B1` (`Test8086`), every scene loader of `051A` (`g_Palette`), `051A:5852`, `051A:527F` (`g_FontTitle`), `051A:92AF` (dialogue reader), all `Assign` calls on DS:075A | static (operand xref of every code listing); C-0002, C-0051..C-0090, C-0160 | CONFIRMED |
| No writer during a flight | no flight-time routine writes the ranges; `0E8B` never calls `0000`/`051A` | static; 17,563 ticks of C-0051..C-0090, 80 of C-0160 | CONFIRMED |
| `g_Weapon` writers and start value | `0E8B:C751..C7AA`, `0E8B:F535..F645` (only writers); BSS | static; C-0069 | CONFIRMED |

## 10. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-02 | RE-2 (Archaeologist + Spec author) | Initial |
| 2 | 2026-11-02 | main session (Verifier) | Capture C-0011 (`wpn-e1m1.txt`); L5 test `crates/td-sim/tests/oracle_weapons.rs` matches every frame. Corrections: volley offset X has a −16; the cooldown decrements before the fire keys. Vectors 4–6 |
| 3 | 2026-11-02 | RE-8 | The `Random` calls of the hit explosions run only when an explosion slot 1..20 is idle (T1: RNG order); explosion and shot presentation moved to flow/flight-screens §4.7–4.8 |
| 4 | 2026-11-06 | RE-15 (Archaeologist + Spec author) | §3.3 new: the exact muzzle bytes of the eight two-digit headings (U-160, OB-017), their sources, constancy during a flight, direct start vs campaign, how often they matter, policy proposal *preserve*. §3.2: `g_Weapon` is 0 at program start and carried from flight to flight for classes 0 and 1 (U-391, BUG-2). §3.1: the address rule applies to every heading and to the player too. Vectors 7, 8; capture C-0160 |
