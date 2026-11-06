# Episode deltas: `td2.exe` (E2) and `td3.exe` (E3) against `file0001.exe` (E1)

| | |
|---|---|
| **Document** | RE knowledge base, cross-episode deltas |
| **Revision** | 3 (2026-11-05) |
| **Task** | M1-09 (RE-7, Archaeologist + Spec author) |
| **Read with** | The E1 specs under [docs/spec/](../spec/), [EXECUTABLES.md §1.1](EXECUTABLES.md#11-ghidra-image-base-segments-and-dgroup-m0-08-2026-11-02), symbol tables [td2.csv](symbols/td2.csv), [td3.csv](symbols/td3.csv) |
| **Confidence** | Unit and function identity: CONFIRMED (instruction-level comparison of every function, §2). Behavioural deltas: CONFIRMED where a row says so (static reading of the instruction stream), otherwise as stated |

## 1. How to use this document

E2 and E3 are separate Borland Pascal 7 programs built from the same units as E1. Most of the engine is the same code. An Implementer porting E2 or E3 (M10) should:

1. implement the E1 spec;
2. apply the deltas listed here for the episode, section by section;
3. take every per-mission rule (setup overrides, mission logic, comm and message tables, outcome tests) from the M10 mission specs, which this document only outlines (§10).

Where a section says **identical**, the E1 spec applies unchanged. Addresses use `<exe>:<seg>:<off>` (ADG §7.8). DGROUP is `file0001:3C43`, `td2:36B6`, `td3:32C5`. "E1 DS:x / E2 DS:y" names the same variable in each program. Flight numbers are the argument of `Sim_FlyMission` (`g_FlightNumber`); mission numbers are the campaign counter (`g_MissionNumber`). They differ in E3 (§8.1).

## 2. Method and match statistics

`tools/ghidra/ExportInstructionTable.java` writes every instruction (bytes, Ghidra instruction mask, references) of the three programs. `tools/scripts/match-episodes.py` then works unit by unit, because BP7 compiles each unit into one code segment and the units are in the same order in all three exes. It pairs functions by same offset (units of equal size), exact match with address operands ignored, opcode skeleton, call graph, call site, address order and similarity. Each pair is classified by walking the E1 instructions over the target image:

- **IDENT**: every instruction is equal, or differs only in an operand that is an address that corresponds. That covers relocated segment words, call or jump targets that map to each other, Pascal string literals with the same text, and DGROUP offsets that follow the DS offset map (§3.2).
- **CONST**: same instruction skeleton, but some operand differs for no address reason.
- **CHANGED**: the skeleton differs. Similarity is the difflib ratio of the skeleton token streams.

[Appendix A](#appendix-a-every-non-identical-routine) lists every non-identical routine with a one-line description. Function counts are Ghidra functions. Some GONE/NEW entries are Ghidra artefacts: a body split in two (E2 `0E96:1874`, E3 `0C5D:DA99`) or a routine moved to another unit (§9.1).

| | E1 functions | Target functions | Matched | IDENT | CONST | CHANGED | E1 only | Target only |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `td2.exe` | 395 | 394 | 380 | 316 | 9 | 55 | 15 | 19 |
| `td3.exe` | 395 | 384 | 374 | 311 | 10 | 53 | 21 | 15 |

Per unit (E1 segment / E2 / E3; unit roles from [bp7-runtime-semantics](../spec/core/bp7-runtime-semantics.md)):

| Unit | E1 / E2 / E3 seg | Size E1 / E2 / E3 | E2: ident / const / changed / E1-only / new | E3: ident / const / changed / E1-only / new |
|---|---|---|---|---|
| Main program | 0000 / 0000 / 0000 | 51A0 / 4EC0 / 4A00 | 3 / 0 / 6 / 0 / 0 | 3 / 0 / 6 / 0 / 0 |
| Scenes, text, dialogue | 051A / 04EC / 04A0 | 9710 / 9AA0 / 7BD0 | 14 / 3 / 25 / 8 / 10 | 13 / 3 / 25 / 9 / 3 |
| Flight | 0E8B / 0E96 / 0C5D | F9F0 / EBB0 / EC10 | 26 / 2 / 12 / 2 / 3 | 24 / 2 / 14 / 2 / 3 |
| Sound effects | 1E2A / 1D51 / 1B1E | 950 (all) | 5 / 0 / 0 | 4 / 1 / 0 |
| Joystick | 1EBF / 1DE6 / 1BB3 | 1F0 / 250 / 250 | 2 / 0 / 0 / 0 / 1 | 2 / 0 / 0 / 0 / 1 |
| Message tables | 1EDE / 1E0B / 1BD8 | 5260 / 42B0 / 4130 | 1 / 0 / 1 / 0 / 2 | 1 / 0 / 1 / 0 / 4 |
| Mission logic | 2404 / 2236 / 1FEB | 6110 / 31F0 / 22A0 | 4 / 0 / 3 / 2 / 0 | 3 / 0 / 3 / 3 / 0 |
| Comm texts | 2A15 / 2555 / 2215 | 2220 / 1760 / C50 | 1 / 0 / 7 / 2 / 0 | 1 / 0 / 3 / 6 / 1 |
| WORX | 2C37 / 26CB / 22DA | 65B0 (all) | 59 / 0 / 0 | 59 / 0 / 0 |
| Mission-ship movement | 3292 / 2D26 / 2935 | 2540 / 23D0 / 23D0 | 1 / 0 / 1 | 1 / 0 / 1 |
| AI and waypoints | 34E6 / 2F63 / 2B72 | 3880 (all) | 27 / 1 / 0 | 27 / 1 / 0 |
| Flight graphics | 386E / 32EB / 2EFA | 1AE0 (all) | 27 / 0 / 0 | 27 / 0 / 0 |
| Crt | 3A1C / 3499 / 30A8 | 620 (all) | 26 / 0 / 0 | 26 / 0 / 0 |
| Dos | 3A7E / 34FB / 310A | 140 / A0 / A0 | 6 / 0 / 0 / 1 / 0 | 6 / 0 / 0 / 1 / 0 |
| String helper | 3A92 / 3505 / 3114 | 80 (all) | 1 / 0 / 0 | 1 / 0 / 0 |
| System | 3A9A / 350D / 311C | 1A90 (all) | 113 / 3 / 0 | 113 / 3 / 0 |

The CONST entries in the shared units are DGROUP offsets that the operand map could not prove (System's typed constants, one AI-unit init store, one sound-unit variable). They are reviewed in §3.1 and are addresses.

## 3. Shared code and data layout

### 3.1 Identical units

Seven units are **code-identical in all three episodes** once relocations and DGROUP offsets are accounted for. The check walked every E1 instruction of the unit at the same offset in E2/E3 and compared the remaining bytes raw (`match-episodes.py`, `unit_verdicts`). The identical units are:

- **System** (`file0001:3A9A`, `td2:350D`, `td3:311C`). The only differences are the offsets of System's typed constants (+15Eh) and variables (−DEh in E2, −E2h in E3). This answers [real48.md](../spec/core/real48.md) Q3 and [bp7-runtime-semantics](../spec/core/bp7-runtime-semantics.md): the Real48 routines `:1002..:17D6`, `Random` and the heap are the same.
- **WORX** (`2C37` / `26CB` / `22DA`): byte-identical. [cmf.md](../spec/audio/cmf.md) and [voc-and-adpcm.md](../spec/audio/voc-and-adpcm.md) apply unchanged.
- **Crt** (`3A1C` / `3499` / `30A8`). The 54 raw-byte differences are DGROUP offsets inside code that Ghidra did not disassemble in E1. All of them are −D8h/−DCh or +15Eh.
- **Sound effects** (`1E2A` / `1D51` / `1B1E`). In E3 one operand (`td3:1B1E:00FC`) is the DS offset of the byte after `g_SfxPriority`. The effect files, ids, priorities and PC-speaker sequences are unchanged, so [sfx-policy.md](../spec/audio/sfx-policy.md) applies unchanged (Q3).
- **Flight graphics** (`386E` / `32EB` / `2EFA`).
- **AI and waypoints** (`34E6` / `2F63` / `2B72`). This includes `AI_ChooseWaypoint` and the line-of-fire scan.
- **String helper** (`3A92` / `3505` / `3114`): the text-screen uncrunch routine (§11).

The **Dos** unit lost `Exec` (E1 `3A7E:009D`). E2 and E3 contain no `label.exe` code or strings and no other way to run a program (OB-001 does not apply to them). The hardware-access picture ([hardware-access.md](hardware-access.md)) is therefore the same for E2/E3. Ports, DAC, retrace, PIT and INT 09h are reached through the identical units and the changed flight unit, and the flight unit's port access did not change (§4).

### 3.2 DGROUP offset map

Variables moved between episodes, but not by one constant. [td2.csv](symbols/td2.csv) and [td3.csv](symbols/td3.csv) carry every named E1 global with its E2/E3 address. The comment says how the address was found: aligned references "n/m", the operand map, or "between" two referenced neighbours (HYPOTHESIS). For an unnamed E1 global, use the region delta below, then check it against a listing.

| E1 DS range | Contents | E2 delta | E3 delta |
|---|---|---:|---:|
| 0000..0262 | Configuration text screen | 0 | 0 |
| 0264..02AB | `k_TileRadarColour` | +106h (→ 036A, 80 entries) | +106h (→ 036A) |
| 02AC..0501 | Heading ring, class tables, shot table, muzzle offsets, System typed constants | +15Eh | +15Eh |
| 0502 | First variable (`g_SbIrq`) | +160h | +160h |
| 0606..07E4 | Menu and main-program variables | +60h | +60h |
| 0C28..0D2A | Campaign state (`g_ScriptChoice`, `g_MissionNumber`, `g_KillsToDate`) | −E0h | −E0h |
| 0D2D..0E77, 102E..123E | Palettes, fonts, scene buffers | mixed (+1B0h, −3E0h) | mixed |
| 12BE..12D4 | Configuration and outcome flags | −E0h / −DCh | −E0h |
| 13E5..1706 | Flight palette, sprites, flags | −E2h | −E6h |
| 1708..18BE | Tick timers, burst state | −DCh | −E0h |
| 18C2..9B9C | All unit variables (flight, sound, joystick, mission, AI, graphics, WORX) | −D8h | −DCh |
| 9BA4..9DF0 | System variables | −DEh | −E2h |

## 4. Flight (`Sim_FlyMission`)

E1 `0E8B:B880` ↔ E2 `0E96:A76E` ↔ E3 `0C5D:A9CC..EC03`. In E3 Ghidra splits the body at `0C5D:DA99` in the middle of a statement; it is one routine. The E3 loop runs from `0C5D:BB20` to its back edge at `0C5D:EA39`.

### 4.1 Tick order

Steps follow [tick-order.md](../spec/sim/tick-order.md) §3. "Identical" means the same instructions modulo addresses. All rows are CONFIRMED (static).

| # | Step | E2 (`td2:0E96:`) | E3 (`td3:0C5D:`) |
|---|---|---|---|
| 1 | Tick counter | BCC8..BCDB. **Changed:** no flight-21 end after 25 ticks | BB20..BB33. Same change as E2 |
| 2 | Clear per-tick flags | identical | identical |
| 3 | Turn step | identical | identical |
| 4 | 11 × 6 tiles | identical | identical |
| 5 | Afterburner burst | BDA6..C18B identical. Two stores move after the sound call; no effect | BC0D..BFF1. **Changed:** flight 16 with mission phase ≥ 2 cannot start a burst |
| 6 | Projectiles (§5.1) | call at C18C | call at BFF2 |
| 7 | Player draw | identical | identical |
| 8–9 | Mission logic A/B | C65D / C662 | C4C3 / C4C8 |
| 10 | Mission ships (§5.2) | C66B. **Removed:** the flight-19 block after the call (shield cap 10, slot-19 pin; E1 `0E8B:D646..D678`) | C4D1. Same removal |
| 11 | End-sequence step | identical | E3 routine stores `g_PlayerDied := 1` on every call (`0C5D:59BD`). This is dead, because teardown recomputes the flag |
| 12 | Overlay tiles | **Changed:** scans the whole 11 × 6 window instead of columns 3..6 × rows 1..4. Codes per map set in §4.4 | **Changed:** whole window; codes in §4.4 |
| 13 | Explosions | identical | identical |
| 14 | Palette effects | **Reordered** (see below). The hit-flash block is now the near procedure `Gfx_HitFlashStep` (`0E96:6D53`), an exact copy of E1's inline block `0E8B:DA6B..DE29` | As E2 (`0C5D:A55B`). **New:** flight 16 with phase ≥ 5 → hit flash := 0 before the call |
| 15 | Comm viewer | identical | **Changed:** flight 16 with phase = 2 sets the comm request every tick |
| 16 | Late player draw | identical | The "end ticks < 15" test is gone. This is dead code: E3 never sets late draw |
| 17 | Joystick | identical | **Changed:** read whenever a joystick is detected, without the `g_CfgController = 2` test |
| 18 | E1 `0E8B:9860` (E1 per-flight scripted events) | **Removed** | **Removed** |
| 19 | Instruments | identical routine | identical routine |
| 25 → | WORX idle poll | **Moved** to just before the tick wait (`CDEE..CE05`, after the instruments), with an extra `g_NoWorx = 0` guard | **Moved**, same (`CC82..CC99`) |
| 20–23 | Limiter, IR notice, flip, pause | identical | identical |
| 24 | Esc menu | identical | identical call (the E3 menu routine changed, §6) |
| new | Per-flight events | `Msn_E2FlightEvents` (`0E96:99B8`), called at D2D4 | `Msn_E3TimedEvents` (`0C5D:959E`), called at D168 |
| 26 | Shields, destruction | identical | identical |
| 27 | Drift, collision, carry, edges | identical | **Changed:** hazard tiles (§4.2) |
| 28 | F5 ship switch | **Changed:** allowed in every flight (E1 excluded flight 14) | **Changed:** blocked only in flight 16 with phase ≥ 5 |
| 29 | Thrust, steering | identical | identical |
| 30 | AI fire requests, Tab | identical | identical |
| 31 | `Sim_PlayerSpecials` | **Inlined** at `E447..E9E0`, with E1 logic except the infra-red bug (OB-023) | **Inlined** at `E461..E9FD`, E1 logic |
| 32 | Cheat chords | Only the chord that sets armour 64 and missiles 30 remains (`E9E1..E9FB`). The other four chords and the difficulty byte are gone (LIKELY) | Same (`E9FE..EA18`) |
| — | Flight-20 rule "view beyond (170, 170) → armour 64" | kept (`EA00..EA17`) | **removed** |
| 33 | End-of-flight test | identical | identical |

**Palette-effect order.** E1 order:

1. hit-flash decay;
2. if flash frames > 0, hit flash := 0;
3. flash-frame DAC writes (frames 4..1);
4. decrement.

E2/E3 order:

1. flash-frame DAC writes;
2. the cancel;
3. `Gfx_HitFlashStep`;
4. decrement.

For flash frames 1..4 the final DAC is the same in both orders, so no visible difference is expected (LIKELY; a capture would confirm it).

**Per-flight event routines.** These are new. Their rules belong in the M10 mission specs; summaries follow.

- **E2 `0E96:99B8`** handles flights 17 and 18. Its trigger is E2 DS:31DE (`g_CutsceneRequest`, the analogue of E1 DS:32B6), which mission logic sets at `td2:2236:2D1B` and `2236:2F15`.
  - **Flight 18.** A counter (E2 DS:11E8, cleared at flight start) advances while the trigger is set. Each step spawns a debris explosion: the first idle slot among 1..20, sound 9 from tile (90, 29), sub-pixel X := `Random(48)`, then Y := `Random(48)`, tile X := 91 + `Random(2)`, tile Y := 28 + `Random(2)`, in this RNG order, frame 1. At count 5 a ticker message is posted. At count ≥ 150 the routine sets `g_MissionLost` := 1, plays sound 100 at (90, 29), sets hit flash := 0 and sets all 256 DAC entries and the palette copy to 63 (white-out).
  - **First tick of flight 18.** Both counter branches run, so two debris explosions spawn.
  - **Flight 17.** A set trigger means mission lost.
- **E3 `0C5D:959E`** handles timed events. "Tick" is `g_MissionTicks`.

| Flight | When | Effect |
|---|---|---|
| 5 | phase < 100 and slot 2 class = FFh | mission lost |
| 10 | tick 10 / tick 300 | phase := 2 and slot 4 class := 2 / phase := 3 |
| 12 | tick 6480 | Map patch (hazard tiles, §4.2; rev 3: it writes back the `td4.map` bytes that `Msn_Setup` case 12 replaced with tile 26h at flight start, [sim/e3-mission-logic §4.6.3](../spec/sim/e3-mission-logic.md)): cells x 40, y 121..123 and 131..133, and x 66, y 121..123 and 128..130 := 44h/45h/46h. Strips x 53..55 at y 114/118/137, x 62..64 at y 114 and x 46..48 at y 118 := 41h/42h/43h |
| 14 | ticks 25 / 27 / 75 / 175 | slots 6–8 class := 8 and slots 9–11 class := 7 / phase := 2 / slot 11 armour := 0 / slot 8 armour := 0 |
| 16 | phase = 3 | Five-step magenta fade of DAC 1..255, one retrace wait per step, ending on the half-tint step. Then every class 0..8 gets max speed := 0 and accel := 0, class 2 gets max speed 20 and accel 4, and phase := 5. These are typed-constant patches that persist (OB-028) |
| 18 | tick 2160 | phase := 2; E3 DS:1EAD := 1: the flag of F5 target-information entry 21, which offers the point (28, 33) on the F5 map (U-202 resolved, rev 3) |
| 21 | tick 10 | phase := 2 |

### 4.2 Flight model

- **E2:** the player physics are **identical** to [flight-model.md](../spec/sim/flight-model.md). That covers the turn step, afterburner burst, shields and destruction, drift and drag, map collision, tile carry, map limits, thrust and steering. The class tables are byte-identical (§11). E2 flight 8 sets late draw (`td2:0E96:65EC` in `Msn_Setup`), so map collision is skipped in that flight, as in E1 flights 12, 15 and 21. CONFIRMED.
- **E3:** identical, with these changes (CONFIRMED):
  1. **Hazard tiles.** These checks are inserted after drift/drag and before map collision (`0C5D:D27A..D3CB`). They apply in every flight except 14, 17 and 19, whatever the class, and also after the mission is over. With the map read at (viewX + dx, viewY + dy):
     ```text
     hazard(dx, dy) := 41h <= map[viewX+dx][viewY+dy] <= 46h
     if hazard(5,2) or (hazard(4,2) and offsetX <= 32) or (hazard(5,2) and offsetY <= 32) or hazard(4,3):
         play sound 9 at the player; shields := 0; armour := armour - 17   (signed byte)
     ```
     - The third clause repeats the first, as read.
     - Destruction is detected at the next tick's step 26.
     - Only `td4.map` contains tiles 41h..46h (37 cells); no `td3.map` flight ever has them: flight 12 flies `td4.map`, and its patch restores cells that its own setup switched off (rev 3). Capture C-0041 confirms the test, and that the damage continues through the end sequence ([sim/e3-mission-ships §5.1](../spec/sim/e3-mission-ships.md))
  2. Flight 16: no new afterburner burst while phase ≥ 2.
  3. The joystick is read whenever one is present, so a connected stick steers even when the keyboard is configured.
  4. Late draw is never set, so map collision is always active.
  5. The flight-16 class-table patch (§4.1).

### 4.3 Setup

| Item | E1 | E2 | E3 |
|---|---|---|---|
| Map and tileset (U-006) | `td.map`, `td.blk` | `g_MapSet` (E2 DS:1302) := 1 for flights 1–8 → `td.map` + `td.blk`; := 2 for flights 9–20 → `td2.map` + `td2.blk`, and the 80-byte table at DS:03BA is copied over `k_TileRadarColour` (DS:036A). That copy is never undone (OB-024) | E3 DS:12FE := 1 always. Flights 1–6 → `td3.map`; flights ≥ 7 (`g_FlightNumber` > 6) → `td4.map`; `td3.blk` always |
| Tile attributes | [structures.md §8](structures.md#8-tile-attributes-u-009) | §4.4 | §4.4 |
| Mission file | `m<n>.td`, not for flight 21 | `m<n>.td2`, always | `m<n>.td3`, always |
| Extra sprites | `mine.td` (7), `lander.td` (20), `td.hel` + `td.h2` (12, 15, 21) | `td.hel` + `td.h2` for flight 8 only | none |
| Infra-red at start | flights 4 and 8 | flights 3 and 10 (also set in E2 `Msn_Setup`) | never. Alt only toggles between modes 1 and 2, so infra-red is unreachable in E3 |
| Flight song | flight mod 3: 1 → `td01`, 2 → `td03`, 0 → `td02` | same rule. **New:** if the song buffer is still nil after the request, a second request tries the rotated choice (mod 3 = 2 → `td02`, 1 → `td03`, 0 → `td01`) | same as E2 |
| Start view (view tile; offsets 32, 16) | [mission-setup.md](../spec/sim/mission-setup.md) | default (102, 104). After setup: flights 5–6 → (179, 33), 9–20 → (87, 28), 7 → (120, 70), late draw (flight 8) → (102, 100). E1's flights 13/14 override is gone | Set before `Msn_Setup`, with no later override: default (167, 66); flights ≥ 7 → (17, 164); 21 → (160, 75); 14, 17, 19 → (88, 76); 7 → (185, 32) |
| Player armour | from class | flight 7: armour := 5 (`td2:0E96:B986`) | flight 7: armour := 5 (test `td3:0C5D:B90E`, store `B915`) |
| Missiles, class 5 | 1 | 4 | 4; flight 12 with class 2: 10 |
| `AI_ChooseWaypoint` for slots 1..20 at setup (E1 `0E8B:CC63..CC7D`) | yes | **absent** | **absent** |
| Difficulty byte := 1, music buffer freed at entry | yes | no | no |

The missing setup loop matters for M10. In E2/E3 each ship's first waypoint is chosen by `AI_UpdateShips` when it first arrives, not before the first tick. That changes the first ticks and the RNG call order (U-205).

`Msn_Setup` (E2 `0E96:60F9`, E3 `0C5D:6149`) is called at `0E96:B6E1` / `0C5D:B66E`, which looks earlier than E1. Only the HUD frame draw moved: E1 drew `td.win` pixel by pixel, and E2/E3 draw it from a buffer after `Msn_Setup`. Relative to game state the order is the same: start-view defaults and projectile clearing before, weapons, missiles, armour and shields from the class tables after. No behavioural change. CONFIRMED.

### 4.4 Tile attributes and overlay tiles (U-009)

Attribute 1 = solid, 2 = open. Index ranges are hexadecimal. Entries above the highest listed index are not initialised, and no map uses them. The highest index in each map is: `td.map` 70, `td2.map` 68, `td3.map` 40, `td4.map` 70.

| Table | Solid | Open | Open share |
|---|---|---|---|
| E1 (`td.map`) | 00–0E, 11–30, 34–37, 3E–44, 46 | 0F–10, 31–33, 38–3D, 45 | 66% |
| E2 map set 1 (`td.map`) | as E1 | as E1, **plus 47–51** | 66.3% |
| E2 map set 2 (`td2.map`) | 00–0A, 11–1B, 2A–2D, 30, 33–39, 3B | 0B–10, 1C–29, 2E–2F, 31–32, 3A, 3C–4B | 74.8% |
| E3 (`td3.map`, `td4.map`) | 00–0F, 17–21, 23–24, 2D–40 | 10–16, 22, 25–2C, 41–46 | 25.5% / 34.3% |

The E2 set-2 fill has overlapping loops: 2E–32 are set open and then 30 solid; 33–3B are set solid and then 3A open. The table above is the end result.

Overlay tiles, redrawn over ships (step 12):

| Map | Overlay rule |
|---|---|
| E1 and E2 set 1 | codes 42h, 44h, 45h, 46h, plus cells x 7Ah..7Fh at y 47h |
| E2 set 2 | codes 1Eh, 1Fh, 20h and 33h..3Bh |
| E3 | codes 21h..24h. Flight 8 adds cells (21..23, 165..166) and (108..110, 113..115); flight 10 adds (21..23, 165..166) and (41..42, 57..58) |

### 4.5 End of flight

The loop-exit test is identical: aborted, end ticks = 35, lost, or succeeded. Teardown:

- E2 adds `Crt` text mode 3 after restoring INT 09h.
- E3 restores INT 09h after the outcome tests.
- Both episodes then set `g_MissionOutcome` := 1 and apply per-flight position tests (CONFIRMED). These belong in the mission specs:

| Episode | Outcome tests (slot numbers 1-based) |
|---|---|
| E2 | Flight 3: outcome 1 only if slot 1 is at (84, 135). Flight 8: 0 unless some slot 2..11 has X = 1. Flight 9: 0 if slot 5 is at (177, 169). Flight 10: 0 if slot 2 is at (151, 199). Flight 11: 0 if slot 2, 6 or 11 has X = 177. Flight 16: the test "slot 2 X = 177 and slot 2 X = 168" can never hold, so outcome is always 1 (OB-025). Flight 17: 0 unless slots 2 and 3 both have X = 1 |
| E3 | Flight 2: 0 if any slot 2..7 has Y = 199. Flight 3: 0 if any slot 2..4 has Y = 199. Flight 4: 0 if any slot 5..7 has X = 199. Flight 5: 0 unless slot 2 has Y = 199. Flight 8: 0 unless slots 1–3 are all at (22, 166). Flight 9: 0 if any slot 1..3 has Y = 199. Flight 10: 0 unless slot 5 is at (22, 166). Flight 12: 0 if any slot 1..6 is at (93, 76) |

## 5. Weapons and mission ships

### 5.1 Projectiles (`Sim_ProjectilesAndHits`)

E1 `0E8B:0984` ↔ E2 `0E96:09A9` (Ghidra splits the body; the tail at `0E96:1874` is skeleton-identical to E1 `0E8B:1888..40B1`) ↔ E3 `0C5D:09A7`.

These are unchanged in E2 and E3:

- the 51 slots, map tests and movement;
- the ±20 px hit box and the ±40 px homing radius;
- the ship damage formula, homing steering and all shot drawing;
- every spawn site (player volley, missile, mission-ship volleys; E1 `0E8B:F2D6..F6A7`): slot ranges and muzzle use are the same.

Changes against [weapons.md](../spec/sim/weapons.md) §4.3:

| # | E1 | E2 | E3 | Change | Conf. |
|---|---|---|---|---|---|
| P1 | `0E8B:0F4C..0F62` | removed | removed | Drop the step-4 rule "flight 20 and owner 1: no ship hits". No flight-number cases remain in the routine | CONFIRMED |
| P2 | `0E8B:1653..16C6` | `0E96:1637..16A1` | `0C5D:1644..16AE` | Player hit: damage is always 2 × (kind + 1). There is no difficulty multiplier and no "difficulty 0: no damage" case. This equals E1 at difficulty 1. Shields-then-armour order, clamps and the explosion are unchanged | CONFIRMED |
| P3 | `0E8B:12C4..12CB` | removed | removed | On a kill in step 4, class := FFh is **no longer written**. The kill count, the projectile-kind := 1 quirk and the five explosions stay. The wreck is removed later in the same tick by the AI pass (armour < 1) | CONFIRMED |
| P3a | — | — | — | Consequence of P3: until step 10 the wreck still counts as live, so a later projectile in the same tick can hit it again. That repeats the sound, distance flash and explosions, and counts another kill for the player (OB-026) | LIKELY |
| P4 | `0E8B:0CFB..0D2D` | `0E96:0D20..0D46` | as E1 | **E2 only:** a homing kill (step 3) plays destruction sound 100 at the ship's tile first, and the projectile-kind := 1 store is gone | CONFIRMED |
| P5 | `0E8B:1207..122D` | removed | kept | **E2 only:** the second sound 100 of the kind-6 branch in step 4 is gone | CONFIRMED |
| P6 | `0E8B:119E..11EF` | reordered | reordered | Order of sound, flash and armour := 0 when armour < 1. E1: sound, flash, armour. E2: flash, sound, armour. E3: armour, flash, sound. The flash uses no RNG, so there is no simulation effect | CONFIRMED |

### 5.2 Mission-ship movement (`AI_UpdateShips`)

E1 `3292:0011` ↔ E2 `2D26:0011` ↔ E3 `2935:0011`. E2 and E3 are identical to each other. Against [mission-ships.md](../spec/sim/mission-ships.md) every per-flight case is **removed** and none is added. The flight argument is still passed but never read. All rows are CONFIRMED.

| Change | E1 |
|---|---|
| Removed: flight 20, slots 1..5 skip the AI call | `3292:0332..033C` |
| Removed: flight 20 heading := 8 and Y offset += 2 (sinking) | `3292:03D5..03F5`, `0D77..0D89` |
| Removed: flight 7 pinning of slots 2..12 to fixed tiles | `3292:113A..129E` |
| Removed (in `Sim_FlyMission`): flight-19 shield cap and slot-19 pin (mission-ships §4.2.4) | `0E8B:D646..D675` |
| **New prologue:** before pass 1, for s = 1..20, ship record field +000h := 1 if class(s) ≠ FFh else 0 (`2D26:001F..0053`, `2935:001F..0053`). No reader found (U-201) | — |

The AI unit (`34E6`: waypoint choice, wall following, line of fire) is identical, so the OB-013..OB-017 quirks carry over.

## 6. HUD and flight UI

`UI_DrawInstruments` (gauges, radar, lamps) and the palette helpers `Pal_SetEntry`/`Pal_StoreEntry` are **identical**. [hud.md](../spec/render/hud.md) applies with these changes (all CONFIRMED unless marked):

**Guidance home point** (`UI_DrawGuidanceLine`: E1 `0E8B:82DD`, E2 `0E96:8587`, E3 `0C5D:81C1`). The home-point table is per episode, and E1's phase-dependent flight-19 rule is gone.

| E2 flight | Home tile | E3 flight | Home tile |
|---|---|---|---|
| 1–4, 7 (default) | (107, 104) | 1–5 | (172, 68) |
| 5 | (189, 37) | 6 | (1, 110) |
| 6 | (125, 68) | 7–10, 12, 14–17, 19, 21 | (22, 166) |
| 8 | (1, viewY + 2), recomputed every tick | 11 | (166, 78) |
| 9–19, 21 | (92, 28) | 13 | (41, 46) |
| 20 | (low byte of viewX + 5, 0), straight up from the player | 18 | (28, 33) |

Other changes in this routine:

- Marker 39h dots: in palette mode 2 (infra-red) all six dots are drawn in colour 90h (E2 `0E96:9012..90C2`, E3 `0C5D:8C52..8D02`).
- Marker 3Dh: the flight-7 rule "slots 2, 3, 4 dead → phase := 100" is gone.

**Ticker and indicator** (`UI_HudTickerAndIndicator`: E2 `0E96:8330`, E3 `0C5D:7FD0`). An edge hit always posts one jurisdiction warning (E2 string `CS:82D8`, E3 `CS:7F78`); E1 chose between two texts by flight 13/14. Ticker stepping and the F6 indicator are unchanged.

**Comm viewer** (`UI_CommViewer`: E2 `0E96:83F9`, E3 `0C5D:8099`). The rule is E1's: when flight = F and phase = P, show comm screen n, clear the comm request and set phase := P′. E1's flight-14 marker case is gone. The table:

| Episode | (flight, phase) → screen n, new phase |
|---|---|
| E2 | (1,2)→1,3 · (5,2)→2,3 · (7,2)→3,100 · (11,1)→4,2 · (19,2)→5,3 · (15,2)→6,3 · (16,2)→7,3 |
| E3 | code order (rev 3): (8,2)→1,3 · (16,2)→3,3 · (19,2)→5,3 · (15,2)→2,3. In flight 16 the request is also forced every tick in phase 2 ([sim/e3-mission-logic §4.5](../spec/sim/e3-mission-logic.md)) |

**Comm screen** (`UI_CommScreen`: E2 `0E96:5401`, E3 `0C5D:540E`). Only the dispatch to the comm text routines differs:

- E2: n = 1..7, unit `2555`: `01D4`, `0525`, `0774`, `0B7E`, `0F93`, `1325`, `158D`.
- E3: n = 1, 2, 3 and 5 only, unit `2215`: `0135`, `042A`, `0680`, `093D`. There is no comm 4.

The comm portraits come from `td2.ccm` / `td3.ccm` (E1 `td1.ccm`; `UI_DrawCommPortraitA/B` differ only in this file name). The per-transmission tables (portraits, lines, markers, speakers) of all eleven loaders: [flow/e2-flight-screens §3.1–3.2](../spec/flow/e2-flight-screens.md) (RE-12).

**Ship switch screen** (`UI_ShipSwitchScreen`: E2 `0E96:9BEC`, E3 `0C5D:9A99`). The per-slot "selectable" flag and the map dots need only class ≠ FFh; E1 also required a guidance marker. Every live ship is shown, marked or not. CONFIRMED for the code, LIKELY that this flag is what limits the cursor.

**Ship library** (`UI_ShipLibrary`: E2 `0E96:7653`, E3 `0C5D:7337`). It draws the same nine `td.shp` sprites in the same places; only which entries are hidden changes (CONFIRMED, RE-12: the full text tables and the flight tests are in [flow/e2-flight-screens §3.3](../spec/flow/e2-flight-screens.md); C-0044):

| | E1 | E2 | E3 |
|---|---|---|---|
| Hatched (unknown) entries | left y B8h always; right y B6h always; left y 9Bh when flight < 4 | left B8h when flight < 15; right B6h when flight < 11 | none |
| Text blocks (4 lines each) | left 2Ch, 51h, 7Ch, and 9Bh when flight > 3; right 3Ch, 67h, 8Dh | left 2Ch..9Bh always, and BAh when flight > 14; right three always, and B3h when flight > 10 | all nine |

**Flight menu.** E2 is identical. E3 (`0C5D:7916`) re-applies the flight-16 tint after the closing fade-in (`0C5D:7EE6..7F63`): if flight = 16 and phase ≥ 5, every DAC entry is set from the flight palette as R′ = 63 − (63 − R) div 2, G′ = G div 2, B′ = 63 − (63 − B) div 2.

**Fades** (`Gfx_FadeInFlight` / `Gfx_FadeOutFlight`, E1 `0E8B:01F6` / `0426`):

- E2/E3 clear the hit flash once at entry; E1 clears it after every step. The end state is the same.
- Entry ranges per episode:

| | E1 | E2 | E3 |
|---|---|---|---|
| Fade-in | entries 1..255 | entries 1..255 | entries **0..255** |
| Fade-out | entries 0..255 | entries **1..255** | entries 0..255 |

## 7. Audio

**Music** (`Snd_PlayMusic`: E2 `0E96:07FB`, E3 `0C5D:07F3`). The behaviour is that of [music-cues.md](../spec/audio/music-cues.md) §4.1. The index table changed (CONFIRMED; music-cues Q3):

| N | E1 | E2 | E3 |
|---:|---|---|---|
| 1..13 | as E1 §3.1 | same | same |
| 14 | `td03` | — | — |
| 44 | — | — | `td03` |
| 45 | — | `td03` | `intro.cmf` |
| 69 | — | `intro.cmf` | `intro.cmf` |

Any other N behaves as in E1 (replays the freed buffer).

**Program-level cues.**

- **Main menu:** `menu` from `UI_LoadMainMenu` (E2 `0000:2ED7`, E3 `0000:2A12`) and after the campaign returns (E2 `0000:38C4`, E3 `0000:33F8`). E2 also plays it on entering the menu loop (`0000:3310`).
- **Menu items 6/7:** `intro.cmf` (69); see §8.3.
- **Game over:** `death` with looping off, then looping restored. E2 has three such sites (`0000:2DDE`, `2E1B`, `2E5E`, one per ending, §8.2); E3 has one (`0000:298A`).
- **E2 end sequence:** E2 also plays `death` **looping** at `0000:2D70`.

The per-mission songs are in the campaign tables (§8.2). Neither episode uses `climax`. Neither plays `low` before a flight, because the E1 pre-flight hint screen is gone; `low` comes only with the debrief.

**Sound effects:** identical (§3.1). The ten VOC names sit at the same code offsets `0655..06BA` of the effects unit in all three. The call sites use the same ids (0, 2, 9, 10, 100, 200 and class-coded ids). Call-site changes:

- the projectile sounds P4/P5 (§5.1);
- E2 `Msn_E2FlightEvents` (ids 9 and 100);
- the E3 hazard tile (id 9).

## 8. Front end and campaign

### 8.1 Flights and mission numbering (U-012)

| | E1 | E2 | E3 |
|---|---|---|---|
| Campaign blocks (`g_MissionNumber`) | 1..20 | 1..20 | 1..6, 8..19 (no block 7, no block 20) |
| Flights flown | 1..20, plus 21 inside block 19 (no mission file) | 1..20 | 1..19, plus 21 inside block 11 |
| Episode complete when (menu, after the increment) | `g_MissionNumber` = 21 | = 21 (`td2:0000:3B80`) | **= 20** (`td3:0000:369E`) |

E3 numbering (CONFIRMED). This resolves U-012.

- **Block 6** flies **F6**, shows its debrief and story, then the mission-7 title card. It then flies **F7**, debriefs, and sets `g_MissionNumber` := 7 (`td3:0000:171F`), so the menu moves on to 8.
- **Block 11** flies **F11**, then plays song 13 and dialogue 64, then flies **F21** (`m21.td3`) with no skip test, then shows one debrief.
- **Block 3** has no title card, and there is no block 20. That is why `td3.exe` has no "mission iii" or "mission xx" titles.
- **F21 is an ordinary flight.** It loads `m21.td3`, and `Msn_Setup`, mission logic and the message tables all have a case 21. Only its start view is special.
- **Quirks** (OB-027):
  - The kills of F6 and F11 are lost: `g_MissionKills` is reset at every flight start, and the paired flights do not add them up.
  - Both block-6 debriefs say mission 6.
  - Skipping the story after F6 jumps straight to F7.
  - If the player dies in F21 and accepts the retry, play restarts at **F11** (`0000:1CAC`).
  - The debrief after F21 is shown before the died/aborted tests.
  - E3 mission 19 has no debrief.

### 8.2 Per-mission sequence

The notation is that of [episode-program-flow.md](../spec/flow/episode-program-flow.md) §4.4:

| Code | Meaning |
|---|---|
| `Mn` | music cue n (§7) |
| `Tnn` | title card (`eX.mission.nn.title`) |
| `T(id)` | a card by exe-resource id |
| background abbreviation | a scene card on that `.scr` (§9.2) |
| `Dn` | dialogue scene n |
| `B` | M8 + debrief |

Every flight has the standard handling unless a row says otherwise: retry prompt on death, a decline leads to ending A, and an abort returns to the menu. The full per-mission traces, with caption resource ids, were extracted statically (CONFIRMED for the order of calls).

**E2 abbreviations:**

| Abbreviation | Background |
|---|---|
| bb | bb |
| brf | briefing |
| bun | bunker |
| bar | sbar |
| die | dieseche |
| elv | elevator |
| grn | greenblo |
| hdd | hydedead |
| heli | helihangar |
| hg, hg2 | hangar, hangar2 |
| hyd | hyde |
| in2 | intro2 |
| ipt, vpt | i-port, v-port |
| lab | tdlab |
| mar | marauder |
| sew | sewer |
| sho | shooting |
| shu | shuttle |
| sof | soffice |
| to2 | tdoffic2 |
| vel | velsroom |
| vid | vidcom |
| vvw | velvswlf |

| E2 | Before the flight | After the flight (after B) |
|---|---|---|
| 1 | M5 mar D1 T01 M10 lab D2 M4 sof D3 M3 hg D4 | M3 hg D5 M13 bun D6 |
| 2 | T02 M6 vel D7 M10 brf D8 M3 hg D130 | M3 hg D131 M11 bar D11 |
| 3 | M6 sho D12 T03 M10 brf D233 | M6; outcome 1: D14; outcome 0: D247, then retry prompt (a decline leads to ending A); M13 bun D15 |
| 4 | T04 M10 lab D16 M4 sof D17 M3 hg D18 | M3 hg D19 M13 sew D20 M4 sof D21 M10 bar D22 |
| 5 | M13 bun D23 T05 M10 sew D24 | M13 sew D25 M10 bun D26 M6 sew D27 |
| 6 | M13 sew D28 T06 M5 mar D29 M6 vid D30 M10 sew D31 | M10 bun D32 |
| 7 | T07 M13 bun D34 vvw D35 M10 elv D236 | M3 hg D36 M4 sof D37 M5 mar D38 |
| 8 | T08 M10 lab D39 M5 mar D40 M6 brf D41 M11 bar D42 M5 mar D46 grn M3 hg D47 | outcome 0: retry prompt; a decline decrements the mission and returns (replay later). Otherwise M11 heli D48 M6 D49 M10 to2 D50 |
| 9 | T09 M6 to2 D51 M10 sho D52 M4 to2 D53 M3 hg2 D54 T(c09.1) D55 | M3 hg2 D56 M5 hyd D57 |
| 10 | T10 M6 sho D58 M5 hyd D59 M4 to2 D60 M3 hg2 D61 | M10 to2 D62 M13 hyd D63 |
| 11 | T11 M11 bar D64 M3 hg2 D65 vid D66 | M3 hg2 D67 M5 hyd D68 |
| 12 | T12 M6 bar D69 M10 brf D70 M3 hg2 D71 | M4 to2 D72 |
| 13 | T13 M6 to2 D73 M11 bar D74 M4 to2 D75 M3 hg2 D76 | M4 to2 D77 M13 lab D78 |
| 14 | T14 M6 bar D79 M5 mar D80 M10 bar D81 M6 to2 D181 M3 hg2 D82 | M4 to2 D83 M6 hyd D84 |
| 15 | T15 M6 lab D85 M10 sho D86 T(c09.1) D87 M13 lab D187 M4 to2 D88 M3 hg2 D89 | M3 hg2 D90 M4 to2 D91 |
| 16 | T16 M5 mar D92 M10 lab D93 M4 to2 D94 | outcome 0: retry prompt; a decline leads to ending B. M3 hg2 D95 M5 hyd D96 |
| 17 | T17 M6 hyd D97 M4 to2 D98 M10 vpt D99 M4 to2 D100 M13 brf D101 | The flight has separate retry prompts for died and lost; lost + decline leads to ending C. After B: outcome 0 → retry prompt, a decline decrements the mission and returns. M5 hyd D102 M4 to2 D103 M5 mar D104 |
| 18 | T18 M10 to2 D105 M6 vid D106 M10 bar D107 M3 hg2 D108 | Flight as 17. M4 to2 D109 M10 vid D110 |
| 19 | T19 M5 mar D112 M4 to2 D113 M5 hyd D114 M3 hg2 D115 | M3 hg2 D116 M5 hyd D117 bb |
| 20 | T20 M4 to2 D119 ipt M5 mar D120 M10 to2 D121 M6 bar D122 | M13 shu D123 hdd D124 M5 mar D125 M12 die M5 mar D127, end card `e2.mission.20.cards[5]` |

E2 endings:

- **A** (`td2:0000:2DDE`): as E1, the player dossier with kills to date plus mission kills.
- **B** (`0000:2E1B`, mission 16): M12, then `intro2` with `e2.mission.20.cards[6]`/`[7]`.
- **C** (`0000:2E5E`, missions 17/18): `intro2` with `cards[8]`.

E2 plays 124 of the 125 scenes in `dialogu2.dat`; **scene 33 is never played**.

**E3 abbreviations:**

| Abbreviation | Background |
|---|---|
| cmd | command |
| cnv | convers |
| in2 | intro2 |
| ion | ionstat |
| kol | kolorvci |
| lab | tdlab |
| mar | marauder |
| mel | melting |
| mn1, mn2 | mining1, mining2 |
| mr2, mr3 | maraud2, maraud3 |
| sel | sel-pain |
| sh0 | shuttle0 |
| vhg | vhang |
| vid | vidcom |
| vlb | vlab |

"fail → R/replay" means: after B, outcome 0 brings up the retry prompt, and a decline decrements the mission and returns.

| E3 | Before the flight | After the flight |
|---|---|---|
| 1 | M6 sh0 D1 T(pre.3) D2 sh0 D3 sh0 D4 M5 mar D5 T01 M10 mn1 D6 M3 mn1 D7 | M6 mn1 D8 M4 cmd D201 |
| 2 | M6 cmd D202 T02 D11 M3 mn1 D12 | fail → R/replay; M3 mn1 D203 M6 lab D204 M5 mr2 D14 M10 sh0 D15 D16 |
| 3 | M13 cnv D17 M4 cmd D18 M10 cmd D19 (no title) | fail → R/replay; M3 mn1 D20 M13 cmd D21 D22 M6 lab D23 |
| 4 | T04 M10 lab D24 M6 cnv D124 T(c04.1) D25 | fail → R/replay; M3 mn1 D26 M5 mr2 D27 |
| 5 | M6 cnv D28 T05 M4 cmd D29 M13 cnv D30 M4 cmd D31 M6 cmd D32 M3 mn1 D33 | fail → R/replay; M5 mr2 D34 |
| 6 + 7 | M5 mr3 D35 T06 M4 cmd D234 M6 T(pre.10) D36 M3 mn1 D37, **F6**, B, M5 mr3 D38 kol D39 T07 D40 M13 T(pre.7) D41, **F7** | B, mission := 7, M10 mn2 D42 M6 lab D43 M5 kol D44 sel |
| 8 | M5 mr3 D46 T08 kol sel D48 M3 mn2 D47 | outcome 0: M3 mn2 D49, retry prompt; **a decline continues as a success**. M10 cmd D50 M5 kol D51 |
| 9 | T09 M5 kol D53 M3 mn2 D54 | fail → R/replay; M3 mn2 D55 M10 kol D56 |
| 10 | T10 M10 lab D57 M3 mn2 D58 | M3 mn2; outcome 1: D60; outcome 0: D59, retry prompt; a decline sets advance := 0 and aborted := 1 (the mission is not advanced, kills are not added) |
| 11 | T11 M5 kol D61 M4 cmd D62 M3 mn2 D63, **F11**, M13 D64, **F21** | B, M3 mn2 D65 |
| 12 | M5 kol D66 T12 M3 mn2 D67 | fail → R/replay; M3 mn2 D68 M10 cmd D168 |
| 13 | T13 M6 lab D69 M3 mn2 D70 | M13 ion D71 |
| 14 | T14 M10 kol D72 T(c01.5) D73 M13 vhg D74 | M3 mn2 D75 |
| 15 | M5 kol D76 T15 M4 cmd D77 M10 lab D78 M4 cmd D79 M6 vid D80 M3 mn2 D81 | M3 mn2 D82 |
| 16 | T16 M5 kol D83 M10 vid D84 M3 mn2 D85 | M6 lab D86 |
| 17 | T17 M5 kol D87 lab D88 M5 vlb D90 M10 vlb D91 | M13 mn2 D92 M5 kol D93 |
| 18 | T18 M10 vid D94 M5 kol D194 M13 vid D95 | M5 mr3 D96 mr3 D97 mr3 D197 mr3 D98 mr3 D99 in2 M11 mr2 D100 |
| 19 | M10 kol D101 M13 sh0 in2 D102 T19 M6 kol D103 mel D105 M13 vhg D106 | **no B**; M10 mn2 D107 M11 kol D108 kol D109, end card `e3.ending` |

E3 notes:

- **Endings:** only ending A. E3 never tests `g_MissionLost`; the flag is presumably folded into the outcome at flight end.
- **Dialogue:** E3 plays all 111 scenes.
- **Scene ids above 127** (E2 130, 131, 181, 187, 233, 236, 247; E3 168, 194, 197, 201–204, 234) are pushed as sign-extended bytes; the byte value is the id.
- **Decline-and-replay paths** (E2 8 and 17; E3 2, 3, 4, 5, 9, 12) leave `g_MissionAdvance` at 1, so the menu still adds the failed attempt's kills (OB-027, LIKELY).

### 8.3 Program start, menu and files

| Item | E1 | E2 / E3 |
|---|---|---|
| Switches | [episode-program-flow.md](../spec/flow/episode-program-flow.md) | Same, **plus `/?` or `?`**: prints a usage text (`e2.startup[0..3]`) and halts. The memory gate changes from MaxAvail < 291A0h to MemAvail < 2BF20h. The memory warning still names the E1 launcher (not ported, OB-002) |
| `tdstatus.dat` | read and written | **Neither read nor written.** `g_ResumeStatus` (E2 DS:07B2) is only read, so it is always 0. The script choice and joystick calibration therefore run at every start, and the menu starts at mission 0, cursor 0. Nothing carries over between episodes: E2/E3 start at mission 1 with 0 kills. U-005: they are presumably started directly (LIKELY) |
| `Exec`, `label.exe` | present (never ported) | **absent** |
| Menu items 0–5, 8 | E1 | as E1. E2 resets kills on New Game |
| Menu items 6, 7 | write `tdstatus.dat` and halt | Shown in-process: `aboutus.scr` (6) or `order.scr` (7) with music 69. A key, Esc or a timeout returns to the menu (30 s; E3 `order` 45 s) |
| Save files | `td.stt`/`td.sav`, `td.sss` | `td2.stt`/`td2.sav` and `td3.stt`/`td3.sav`; `td.sss` and `td.cfg` are shared |
| Missing dialogue file | notice screen | silently selects script 2. The script-choice screen loses one line (the `helpme.doc` pointer) |
| Menu free-memory readout | drawn at (10,10) in ink 0 (`0000:32D7..3305`) | removed |
| Program exit | text mode | Text mode 3, then the 262-byte crunched screen at DS:0264 is uncrunched to B800:0000: an end-of-program notice pointing to `catalog.exe` (`td2:0000:4E88`, `td3:0000:49C6`) |
| Unit init chain | 2404 has an init part | E3 has no 2404 init (EXECUTABLES §1.1) |

## 9. Scenes, text and dialogue (unit 051A / 04EC / 04A0)

### 9.1 Routines removed, moved or kept

| E1 routine | E2 / E3 | Conf. |
|---|---|---|
| `Inp_WaitKeyOrButton` `051A:1A40` | **Moved** to the joystick unit (`td2:1DE6:01ED`, `td3:1BB3:01ED`), same logic. Only caller: `Dlg_PlayScene` | CONFIRMED |
| `Txt_DrawStringOnce` `1DA1` | Removed; its only caller was the menu memory readout | CONFIRMED |
| `Scene_HintScreen` `2B26` | Removed: no pre-flight hint screens, no `secret.pic` | CONFIRMED |
| `Scene_Still` `4113` and the `td1-15`/`td1-18` stills | Removed: no `.pic` files in E2/E3 | CONFIRMED |
| `UI_MissingDialogueNotice` `56D5` | Removed (silent fallback) | CONFIRMED |
| `UI_AssignmentOrders` `62FD` and its rectangle helper `609D` | Removed | CONFIRMED |
| `UI_EnemyDossier` `6653` | Removed | CONFIRMED |
| `UI_PlayerDossier` `74B2` | Kept: `td2:04EC:712D`, `td3:04A0:5212`. Same layout | CONFIRMED |
| `UI_RetryPrompt` `6A33` | Kept: `td2:04EC:6E62`, `td3:04A0:4F47`. Taunt 5 has new text and its x position moves 46 → 50 | CONFIRMED |
| `UI_Debrief` `6FF2` | E2 identical. E3: a different failure-note string (`e3.debrief`) and a final wait of **15 s** instead of 10; tiers unchanged | CONFIRMED |
| `Scene_VultureCommand` `596F` | E2 `Scene_Hyde` (`04EC:5BA3`) uses its layout; no counterpart in E3 | CONFIRMED |

### 9.2 Scene cards

E2/E3 scene routines are named after their background in [td2.csv](symbols/td2.csv) / [td3.csv](symbols/td3.csv). The matcher paired them with E1 routines by position, which was wrong for most of them.

Captions use the `td.lt2` small font in three lines at y0, y0 + 10 and y0 + 20 ([text.md](../spec/render/text.md); the E1 cards are in text.md §4.5.1, RE-12). Every card plots its picture at x + 1 (OB-003). Shadow styles (black copies drawn before the ink; style l = (−1, 0) only):

| Style | Offsets |
|---|---|
| s1 | (−1, +1) |
| s3 | (−1, +1), (−1, 0), (0, +1) |
| o6 | s3 plus (+1, −1), (+1, 0), (0, −1) |
| o4 | (±1, 0), (0, ±1) |
| p | none |

"Ink e = (r, g, b)" means palette entry e is set to that 6-bit RGB.

| E2 `04EC:` | Background | Anchor, style | Ink | Wait (s) |
|---|---|---|---|---|
| 1D4A | hangar | (10,170) s1 | 210 = (14,63,63) | 10 |
| 1F49 | hangar2 | (10,170) s1 | 97 = (14,38,63) | 10 |
| 2146 | tdlab | (10,170) s3 | 109 = (10,10,63) | 10 |
| 23AF | marauder | as E1 ([text.md §4.5.1](../spec/render/text.md)): background 15 rows down; captions joined into one line (A + 4 spaces + B + 3 spaces + C + 3 spaces) at (10,10), style l | 255 = (63,5,5) | 10 |
| 25EF | briefing | (10,170) s3 | 250 = (10,10,63) | 10 |
| 2851 | helihangar | (180,20) p | 255 = (10,0,0) | 10 |
| 2A23 | intro2 | (50,90) p | 140 = (0,0,0) | 10 |
| 2BF0 | shooting | (10,170) p | 28 = (14,14,63) | 10 |
| 2DC0 | dieseche | (10,170) o6 | 174 = (63,0,63) | 15 |
| 30AF | soffice | (180,170) o6 | 74 = (14,50,63) | 10 |
| 33B1 | sewer | (180,170) o6 | 194 = (14,50,63) | 10 |
| 36B5 | shuttle | (10,10) o6 | 15 = (14,63,14) | 10 |
| 398E | bunker | (10,10) o6 | 197 = (40,40,63) | 10 |
| 3C69 | elevator | (10,10) o6 | 255 = (63,40,40) | 10 |
| 3F44 | greenblo | (120,170) o6 | 255 = (40,63,40) | 15 |
| 4232 | vidcom | (10,10) o6 | 255 = (40,40,63) | 10 |
| 4507 | bb | (10,10) o6 | 255 = (63,40,40) | 10 |
| 47E0 | v-port | (240,10) o6 | 255 = (40,63,40) | 10 |
| 4ACE | i-port | (240,10) o6 | 255 = (40,63,40) | 10 |
| 4DBE | hydedead | (240,10) o6 | 255 = (40,63,40) | 15 |
| 50AD | aboutus | no text | — | 30 |
| 5201 | order | no text | — | 30 |
| 5358 | tdoffic2 | (10,170) o6 | 4 = (14,50,63) | 10 |
| 5644 | sbar | (10,10) s1 | 90 = (48,38,63) | 10 |
| 583E | velsroom | (10,20) s3 | 12 = (40,40,63) | 10 |
| 5BA3 | hyde | x = 160 − Round(5 × length / 2) per line, y 5/15/25, o4 (E1 voffice layout 5 rows higher; C-0046) | 3 = (63,63,63) | 10 |
| 6127 | velvswlf | as hyde | 3, RGB not set (the picture's own entry 3) | 15 |

`velvswlf.scr` was missing from the E2 scene list in [EXECUTABLES.md §6](EXECUTABLES.md#6-episode-2-td2exe).

| E3 `04A0:` | Background | Anchor, style | Ink | Wait (s) |
|---|---|---|---|---|
| 1D6B | aboutus | no text | — | 30 |
| 1EBF | order | no text | — | **45** |
| 2013 | tdlab | as E2 | as E2 | 10 |
| 227C | marauder | as E1 | as E1 | as E1 |
| 24BB | maraud2 | (180,170) o6 | 255 = (63,5,5) | 10 |
| 27CC | maraud3 | one joined line at (5,2) p | 83 = (63,15,63) | 10 |
| 29AE | vidcom | as E2 | as E2 | 10 |
| 2C86 | vhang | as E1 | as E1 | as E1 |
| 2EE4 | intro2 | as E1 | as E1 | as E1 |
| 30B1 | shuttle0 | (10,170) o4 | 255 = (63,63,14) | 15 |
| 333D | vlab | as E1: (180,170) s3 | 32 | as E1 |
| 35A8 | convers | (10,10) p | 255 = (63,14,14) | 10 |
| 3775 | sel-pain | (120,180) p | 255 = (63,14,14) | 15 |
| 3944 | melting | (120,180) p | 255 = (63,14,14) | 15 |
| 3B14 | kolorvci | (180,170) o4 | 255 = (63,63,14) | 10 |
| 3DB2 | command | (10,5) p | 255 = (63,63,63) | 10 |
| 3F7E | mining1 | (10,170) o4 | 255 = (63,63,14) | 10 |
| 420D | ionstat | (10,170) o4 | 255 = (63,63,14) | 10 |
| 449C | mining2 | (10,170) p | 255 = (63,14,14) | 10 |

In `sel-pain` and `melting` the third caption line is drawn at y = 200, off screen (OB-027, LIKELY).

### 9.3 Other changes in the unit

- **Dialogue player** (`Dlg_PlayScene`: E2 `04EC:96A4`, E3 `04A0:7795`).
  - The face remaps of [dialogue-player.md §3.3](../spec/narrative/dialogue-player.md) are confirmed.
  - The text-entry allocator starts at **240** (initial F0h; E1 FAh) and wraps from 255 to 240, so entries cycle 240..254.
  - `Dlg_ShowLine` passes 64h instead of 5Ah as the row count of `Gfx_ScrollTextUp`, so the dialogue scroll region is rows 101..199 instead of 101..189. This matches the missing bottom prompt (CONFIRMED for the argument, LIKELY for the purpose).
  - `Dlg_LoadPortrait` loads `newfaces.td2` / `newfaces.td3`.
- **`Flow_WaitSecondsOrKey`:**
  - E2 is identical.
  - E3 (`04A0:0269`) flushes the BIOS keyboard buffer and clears both joystick button flags on entry, so typed-ahead keys no longer skip cards.
- **Save screens** (`Sav_RestoreScreen` and the save screen):
  - The file names change (§8.3).
  - E2 moves the keyboard flush to after drawing.
  - E3 flushes at the start and clears the joystick Y and button flags before the selection loop.
- **`Scene_MissionTitle`:** E2's CONST difference is a DGROUP palette address only.

## 10. Mission logic framework

The per-mission content is for the M10 mission specs ([mission-logic.md](../spec/sim/mission-logic.md) and [mission-setup.md](../spec/sim/mission-setup.md) describe E1). Structure:

- **Dispatch is by `g_FlightNumber`** in every episode: `Msn_UpdateA/B`, `Msn_Setup` and the message tables. The cases present are:

| Routine | E1 | E2 | E3 |
|---|---|---|---|
| `Msn_UpdateA` / `Msn_UpdateB` | `2404:1426` / `3521`: 1..9 / 10..20 | `2236:0ADB` / `240A`: 1..9 / 10..20 | `1FEB:0A7F` / `19AD`: 1..19, 21 |
| `Msn_Setup` | `0E8B:60D8`: 1..21 | `0E96:60F9`: 1..20 | `0C5D:6149`: 1..11, 21, 12..20 (20 is dead) |
| F5 target information (`Msn_LoadTargetInfo`, called once from `Msn_Setup`; flags, X, Y and five lines per entry; rev 2: was "message tables") | `1EDE:0E6A`: 1..13, 15..20 | `1E0B:0DCC`: 1..20 ([sim/e2-mission-setup §3.4](../spec/sim/e2-mission-setup.md)) | `1BD8:0DCF` (rev 3; `12A1` is the flight-3 test): 1..11, 21, 12..20 ([sim/e3-mission-setup §3.4](../spec/sim/e3-mission-setup.md)): 1..11, 21, 12..20 (20 is dead) |
| Per-tick event routine | `0E8B:9860` | `0E96:99B8` (17, 18) | `0C5D:959E` (§4.1) |
| Comm text routines | 9 (`2A15`) | 7 (`2555`) | 4 (`2215`) |

- **Ghidra artefacts.** Ghidra truncated E2 `Msn_UpdateB` at `2236:27E8`, so the code for flights 12–20 was not in its function body and its similarity score is meaningless. Rev 2: RE-9 completed it (`2236:240A..31DD`) with `tools/ghidra/DisassembleAt.java`; E2 mission logic is in [sim/e2-mission-logic](../spec/sim/e2-mission-logic.md). The "new" routines `1E0B:0DCC`/`11C2` and `1BD8:0DCF`/`0FEE`/`116A`/`411C` are fragments of the message-table routine. M10 needs those ranges disassembled (re-run `SetupBp7Program` after marking them).
- **Helpers:**
  - `Msn_DrawTickerGlyph`, `Msn_PostTicker` and `Msn_ExitAt` are identical.
  - The E1 helpers `2404:0616` (player touches) and `2404:07DB` (near) are gone.
  - `Msn_NearLive` survives as E2/E3 `:0616` with one change: slot 0 (the player) uses tile (`g_ViewTileX` + 5, `g_ViewTileY` + 3), computed in 16 bits, instead of the view origin. CONFIRMED.
  - `AI_SetTarget` (`34E6:3232`) is unchanged.
- **Globals:** the same set as E1 (phase, ticker, succeeded, comm flag, counters; td2/td3.csv). New are `g_MapSet` (§4.3) and the E3 flag DS:1EAD (U-202).
- **Size:** the mission-logic unit shrank from 6110h bytes (E1) to 31F0h (E2) and 22A0h (E3).
- **Comm resources.** RE-3's ids in [td2.toml](exe-resources/td2.toml)/[td3.toml](exe-resources/td3.toml) number the string runs, not the comms:
  - E2 comm 6 uses `e2.comms.06` and `.07`, and comm 7 uses `e2.comms.08`;
  - E3 comm 5 uses `e3.comms.04`.

## 11. Data tables

| Table | E1 | E2 | E3 | Notes |
|---|---|---|---|---|
| Configuration text screen (crunched, uncrunched by the 3A92 helper) | DS:0002..0262 | same | same | 3 bytes differ (DS:0240, 024C, 0259): the episode digit of the exe name in three example command lines |
| Exit notice screen (crunched, 262 bytes) | — | DS:0264..0369 | DS:0264..0369 (same bytes) | §8.3 |
| `k_TileRadarColour` (map tile → radar colour) | DS:0264, 71 entries used (tiles 0..70) | DS:036A, 80 entries (71..79 are 0); differs from E1 only at tiles 62..70 (E1 50h; E2 alternating 14h/1Ah) | DS:036A, its own contents (67 of entries 0..70 differ from E1) | Exe resources `e2.hud.radar_tile_colours`, `e3.hud.radar_tile_colours` |
| Alternate radar colours (map set 2) | — | DS:03BA, 80 entries; copied over DS:036A when `g_MapSet` = 2 | DS:03BA, a copy of E2's table, unreferenced | `e2.hud.radar_tile_colours_set2` |
| Heading ring | DS:02AC | DS:040A | DS:040A | byte-identical (+15Eh) |
| Class tables: max speed, armour, shields, accel | DS:02BE..02E4 | DS:041C..0442 | DS:041C..0442 | byte-identical. The E3 flight-16 runtime patch is in §4.1 |
| Shot table (17 muzzles per class) | DS:02E5 | DS:0443 | DS:0443 | byte-identical |
| Muzzle offsets | DS:035C (`flight.muzzle_offsets`) | DS:04BA | DS:04BA | byte-identical: use the E1 resource |
| Tile attributes | code fill (structures §8) | code fill, two sets | code fill | §4.4 |
| Music index table | code (`0E8B:0825..0963`) | code | code | §7 |
| Dialogue face remaps | `051A:92AF` | `04EC:96A4` | `04A0:7795` | [dialogue-player.md §3.3](../spec/narrative/dialogue-player.md) |
| Mission counts | 20 blocks, flights 1..21 | 20 blocks, flights 1..20 | 18 blocks, flights 1..19 + 21 | §8.1 |
| Map selection | — | `g_MapSet` DS:1302 | DS:12FE (always 1) + `g_FlightNumber` DS:31DD > 6 | §4.3 |

The table at DS:028A that E1 text code reads is not a DGROUP table. Every access is `ES:[DI+028A]` with ES:DI = the `td.lt2` font block, so it is byte 650 of the font: the opaque background colour of [text.md §3](../spec/render/text.md). It is the same in all episodes.

## 12. Original bugs and quirks found

New rows in [original-bugs.md](../spec/original-bugs.md): OB-023 (E2 infra-red toggle), OB-024 (E2 radar table never restored), OB-025 (E2 flight-16 outcome test), OB-026 (E2/E3 double kill in one tick), OB-027 (E3 campaign quirks and off-screen captions), OB-028 (E3 flight-16 class freeze). The E1 quirks OB-003..OB-022 carry over where their code is unchanged. These are:

- the scene and text unit routines (OB-003..OB-005, OB-007);
- path data (OB-008);
- sound (OB-009, OB-011, OB-012);
- Real48 (OB-010);
- AI (OB-013..OB-017);
- the radar and guidance quirks (OB-019, OB-020).

The E1 per-flight rules behind OB-021 and OB-022 do not exist in E2/E3. OB-018 does have an E2 instance (rev 2): E2 flight 8 sets `k_ClassMaxSpeed[3]` := 20h (`td2:0E96:65FD`), which persists.

## 13. Corrections to other documents

Found during M1-09 and not yet applied to the owning spec (U-207):

- **E1 mission 20 has no debrief:** `file0001:0000:2C9B` jumps straight to the story ([episode-program-flow.md §4.4](../spec/flow/episode-program-flow.md)).
- **E1 scene captions do not all sit at (10,170) in entry 210** ([text.md §4.5](../spec/render/text.md)). Examples: vlab at (180,170) entry 32, velsroom at (10,20), sbar at (10,10).
- **`docs/re/EXECUTABLES.md` §6** omits `velvswlf` (§9.2).

Applied here: [dialogue-player.md](../spec/narrative/dialogue-player.md) (E2/E3 allocator), [structures.md §8](structures.md#8-tile-attributes-u-009), [hardware-access.md](hardware-access.md), and the M1-09 questions in [real48.md](../spec/core/real48.md), [music-cues.md](../spec/audio/music-cues.md) and [sfx-policy.md](../spec/audio/sfx-policy.md).

### 13.1 Corrections in rev 2 (RE-9, M10-01)

- §4.3: the flight-7 armour store is at `td2:0E96:B98D` (`B986` is the test). §4.4: the "highest index" values are decimal (46h in `td.map`, 44h in `td2.map`).
- §8.2 E2 table (details in [flow/e2-program-flow §4.4](../spec/flow/e2-program-flow.md)): block 16's outcome-0 retry and ending B can never run (OB-025, OB-042); block 18 has no outcome rule after the debrief; ending C also plays song 12 with looping off; the ending music calls are at `0000:2DE0`/`2E1D`/`2E60`.
- §8.3: E1 also resets kills on New Game (`file0001:0000:38FF`), so that is not a delta; E2 menu items 0–5 also lose the item-3 highlight, DAC-255 and MemAvail rules (OB-041), the menu loop plays song 7 on entry, and item 3 calls `Gfx_InitUnchained` first. The `/?` text is 8 lines: four unmapped strings at file offset 0x826C, then `e2.startup[0..3]`.
- §10: `e2.flight_messages.02` has 20 strings, not 7; `e2.target_info` (113 strings) replaces `e2.locations` and `e2.target_info.01..06` in [td2.toml](exe-resources/td2.toml).

### 13.2 Corrections in rev 3 (RE-10, M10-02)

E3 per-flight content now lives in [sim/e3-mission-setup](../spec/sim/e3-mission-setup.md), [sim/e3-mission-logic](../spec/sim/e3-mission-logic.md), [sim/e3-mission-ships](../spec/sim/e3-mission-ships.md) and [flow/e3-program-flow](../spec/flow/e3-program-flow.md). Corrections to this document:

- §4.1, flight 12: the tick-6480 patch restores the `td4.map` bytes that `Msn_Setup` case 12 replaced with 26h; §4.2: no `td3.map` flight has hazard tiles. Flight 18: DS:1EAD is the F5 entry-21 flag (U-202). §4.3: flight-7 armour store `B915`. §6: E3 comm viewer rows in code order (8, 16, 19, 15).
- §4.1/§12 OB-028: `Msn_Setup` case 17 restores the class tables; the freeze lasts only until the next flight-17 setup (OB-028 corrected).
- §7: the ending's music call is at `0000:298C` (`298A` is the label); E3 never passes index 45 and starts the menu song once (from the loader).
- §8.1: `1CAC` is the F11 retry label (the call is at `1CAE`); a retry also replays M13 and D64. Aborting F21 shows the debrief and forces F11 again; aborting F7 replays block 6 from F6. Flight 7 therefore always starts in phase 100 (OB-043, U-270).
- §8.2 E3: block 1 begins with a skip test that `Main_CampaignLoop` no longer clears (OB-050); block 8's outcome-0 path, block 10's post-flight `mining2` card and D59 and block 11's D64 have no skip test. `g_MissionLost` is set only in flight 5; it is not folded into the outcome by code. Decline-and-replay (blocks 2, 3, 4, 5, 9, 12) is CONFIRMED; block 10's decline clears advance, block 8's continues as a success.
- §8.3 E3: the menu loader flushes the keyboard (keys typed after the script choice do not reach the menu); every timed wait flushes on entry; no song-7 call at menu-loop entry; no text mode between flights; `Crt.ClrScr` after the memory gate; the restore screen accepts missions 7 and 20, which have no block (OB-051).
- §10: `Msn_LoadTargetInfo` E3 is `1BD8:0DCF` with a 114-string pool (`e3.target_info`); `AI_UpdateShips` E3 is byte-identical to E2 apart from relocations and DGROUP −4.
- New original bugs: OB-043..OB-054 (E3).

## 14. Open questions

| ID | Question |
|---|---|
| U-200 | What sets E2 DS:31DE (`g_CutsceneRequest`) for flights 17/18? **Resolved (rev 2):** [sim/e2-mission-logic §4.7.2](../spec/sim/e2-mission-logic.md) |
| U-201 | Who reads the new E2/E3 ship-record flag +000h written by the `AI_UpdateShips` prologue? **Resolved (rev 2): nobody** ([sim/e2-mission-ships §4.2](../spec/sim/e2-mission-ships.md)) |
| U-202 | Meaning of E3 DS:1EAD. **Resolved (rev 3):** the flag of F5 target-information entry 21 ([sim/e3-mission-logic §4.9](../spec/sim/e3-mission-logic.md)) |
| U-203 | Is the E3 flight-16 magenta tint ever restored? **Resolved (rev 3, static):** DAC-only, removed by a white flash until the next Esc menu ([sim/e3-mission-logic §4.7.3](../spec/sim/e3-mission-logic.md)) |
| U-204 | Capture check of the E2/E3 double kill (OB-026) and of the palette-effect reordering (§4.1) |
| U-205 | E2/E3 first-tick behaviour without the setup `AI_ChooseWaypoint` loop. **Resolved for E2 (rev 2):** a null path step in tick 1, one-tick delay, no RNG change ([sim/e2-mission-ships §4.3](../spec/sim/e2-mission-ships.md), C-0015, C-0016) **E3 (rev 3):** the same, confirmed by C-0036 and C-0041 ([sim/e3-mission-ships §4.3](../spec/sim/e3-mission-ships.md)) |
| U-206 | Not analysed for E2/E3: the `UI_LoadMainMenu` highlight changes, the menu palette fades, `Scene_BackgroundStory`, and the "02 line uses its own entry" rule (dialogue-player §4.9) |
| U-207 | E1 spec corrections in §13 to be confirmed and applied by the owners |

## 15. Provenance

| Claim | Evidence | Confidence |
|---|---|---|
| Unit identity (§3.1) | `match-episodes.py` unit walk over every E1 instruction; raw-byte comparison of the rest; residual differences inspected one by one | CONFIRMED |
| Function classes (§2) | Instruction walk with relocation, flow, string and DS-map explanations | CONFIRMED (IDENT), LIKELY (pairing of CHANGED routines below 0.6 similarity; manual pairs are listed in the script) |
| Typed-constant layout (§11) | Byte alignment of the three DGROUP images; references from code | CONFIRMED |
| Flight, weapons, AI, HUD, audio, campaign deltas (§4–§10) | Static reading of the E1/E2/E3 listings around every changed run (`hunks-*.tsv`), by RE-7 and three delegated Archaeologist passes (flight; weapons/AI/HUD/audio; front end/narrative/data). Each row states its confidence | as stated |
| Map statistics (§4.4) | Counts over the `.map` files | CONFIRMED |

No dynamic captures of E2/E3 were made for M1-09. The first E2/E3 oracle scenarios belong in M10 (U-204).

## Appendix A. Every non-identical routine

This table lists every routine that is not IDENT in E2 or E3, plus the new routines, with a one-line description. IDENT routines are not listed (§2), and neither are the CONST rows of the shared units (DGROUP offsets only, §3.1). E2/E3 addresses are those of [td2.csv](symbols/td2.csv)/[td3.csv](symbols/td3.csv), after the hand corrections.

| E1 | Routine | E2 | E3 | Change |
|---|---|---|---|---|
| `0000:0000`, `0000:0124` | Menu palette fades (`Flow_MenuFadeOut` and its neighbour) | `0000:0000`, `00D2` | same | Changed (similarity 0.80); not analysed in M1-09 |
| `0000:0F06` | `Main_CampaignLoop` | `0000:0CC3` | `0000:0BC5` | Episode content: §8.1, §8.2 |
| `0000:2E88` | `UI_LoadMainMenu` | `0000:2EC0` | `0000:29E5` | Plays `menu`; no free-memory readout; highlight changes not analysed (U-206) |
| `0000:3381` | `UI_MainMenuLoop` | `0000:3304` | `0000:2E3F` | Items 6/7 shown in-process; no `tdstatus.dat`; own save names; episode ends at mission 21 (E3: 20) |
| `0000:4526` | `Main_Entry` | `0000:4365` | `0000:3E8A` | `/?` switch, MemAvail gate, no `tdstatus.dat` read, exit notice screen (§8.3) |
| `051A:1618` | `Dlg_LoadPortrait` | `04EC:176D` | `04A0:178D` | File name `newfaces.td2`/`.td3` |
| `051A:1A40` | `Inp_WaitKeyOrButton` | `1DE6:01ED` | `1BB3:01ED` | Moved to the joystick unit, same logic |
| `051A:1AA3` | `Flow_WaitSecondsOrKey` | identical | `04A0:0269` | E3 flushes keys and joystick buttons on entry |
| `051A:1BF7` | `Dlg_ShowLine` | `04EC:1B95` | `04A0:1BB5` | Scroll region rows 101..199 (row count 64h) |
| `051A:1DA1`, `2B26`, `4113`, `56D5`, `609D`, `62FD`, `6653` | String-once, hint screen, stills, missing-dialogue notice, assignment orders, enemy dossier | — | — | Removed (§9.1) |
| `051A:1E5A..4F5A` (scene cards) | `Scene_*`, `Flow_Scene*` | 27 routines | 19 routines | One routine per background, with its own caption anchor, shadow, ink and wait (§9.2) |
| `051A:527F` | `UI_ScriptChoice` | `04EC:689B` | `04A0:4980` | One line fewer; a missing dialogue file silently selects script 2 |
| `051A:5852` | `Scene_MissionTitle` | `04EC:5A89` | identical | DGROUP palette address only |
| `051A:596F` | `Scene_VultureCommand` | `04EC:5BA3` (`Scene_Hyde`) | — | Same layout, other background |
| `051A:5EE9`, `5FC7` | `Gfx_LoadJavelin`, `Gfx_LoadCity` | `04EC:6693`, `6B77` | `04A0:4778`, `4C5C` | Slightly shorter; same files |
| `051A:6A33` | `UI_RetryPrompt` | `04EC:6E62` | `04A0:4F47` | Taunt 5 text and x position |
| `051A:6FF2` | `UI_Debrief` | identical | `04A0:5F7E` | E3 failure-note string, 15 s wait |
| `051A:74B2` | `UI_PlayerDossier` | `04EC:712D` | `04A0:5212` | Same layout; local buffer size |
| `051A:76EA` | `Sav_RestoreScreen` | `04EC:7B20` | `04A0:5450` | File names; keyboard/joystick flush moved |
| `051A:8EAA` | `Scene_BackgroundStory` | `04EC:92EE` | `04A0:73DF` | One instruction fewer (similarity 0.999); not analysed in M1-09 |
| `051A:92AF` | `Dlg_PlayScene` | `04EC:96A4` | `04A0:7795` | Face remaps, no bottom prompt, allocator 240..254 (§9.3) |
| `0E8B:01F6`, `0426` | `Gfx_FadeInFlight`, `Gfx_FadeOutFlight` | `0E96:01F6`, `0426` | `0C5D:01F6`, `0422` | Entry ranges and hit-flash clear (§6) |
| `0E8B:07ED` | `Snd_PlayMusic` | `0E96:07FB` | `0C5D:07F3` | Index table (§7) |
| `0E8B:0984` | `Sim_ProjectilesAndHits` | `0E96:09A9` (+ `1874`) | `0C5D:09A7` | P1..P6 (§5.1) |
| `0E8B:51FE`, `530B` | `UI_DrawCommPortraitA/B` | `0E96:51CA`, `52D7` | `0C5D:51D7`, `52E4` | File name `td2.ccm`/`td3.ccm` |
| `0E8B:5435` | `UI_CommScreen` | `0E96:5401` | `0C5D:540E` | Dispatch to 7 / 4 comm text routines (§6) |
| `0E8B:56ED` | `Sim_EndSequenceStep` | identical | `0C5D:568F` | One dead store (§4.1 step 11) |
| `0E8B:60D8` | `Msn_Setup` | `0E96:60F9` | `0C5D:6149` | Per-flight content (§10) |
| `0E8B:7374` | `UI_ShipLibrary` | `0E96:7653` | `0C5D:7337` | Which entries are hidden (§6) |
| `0E8B:794C` | `UI_FlightMenu` | identical | `0C5D:7916` | Flight-16 tint re-applied (§6) |
| `0E8B:7FDF` | `UI_HudTickerAndIndicator` | `0E96:8330` | `0C5D:7FD0` | One jurisdiction warning (§6) |
| `0E8B:80D3` | `UI_CommViewer` | `0E96:83F9` | `0C5D:8099` | Per-episode (flight, phase) table (§6) |
| `0E8B:82DD` | `UI_DrawGuidanceLine` | `0E96:8587` | `0C5D:81C1` | Home points, 39h dots in IR, no flight-7 rule (§6) |
| `0E8B:9860` | `Msn_UpdateEvents` | — | — | Removed; replaced by `Msn_E2FlightEvents` / `Msn_E3TimedEvents` (§4.1) |
| `0E8B:A714` | `UI_ShipSwitchScreen` | `0E96:9BEC` | `0C5D:9A99` | Every live ship selectable (§6) |
| `0E8B:B1EC` | `Sim_PlayerSpecials` | inlined in `Sim_FlyMission` | inlined | E2 infra-red bug (OB-023) |
| `0E8B:B880` | `Sim_FlyMission` | `0E96:A76E` | `0C5D:A9CC` | §4 |
| — | `Gfx_HitFlashStep` (new) | `0E96:6D53` | `0C5D:A55B` | E1 inline block made a procedure (§4.1) |
| — | `Msn_E2FlightEvents`, `Msn_E3TimedEvents` (new) | `0E96:99B8` | `0C5D:959E` | §4.1 |
| `1E2A:006A` | `Snd_PlayEffect` | identical | `1B1E:006A` | DGROUP offset only |
| `1EDE:0E6A` | Message tables | `1E0B:1475` | `1BD8:0DCF` | Per-flight content (§10) |
| `2404:0616`, `07DB` | `Msn_PlayerTouches`, `Msn_Near` | — | — | Removed |
| `2404:06EE` | `Msn_NearLive` | `2236:0616` | `1FEB:0616` | Player tile = view + (5, 3) (§10) |
| `2404:1426`, `3521` | `Msn_UpdateA`, `Msn_UpdateB` | `2236:0ADB`, `240A` | `1FEB:0A7F`, `19AD` | Per-flight content (§10) |
| `2404:60FD` | `Unit2404_Init` | identical | — | No init part in E3 |
| `2A15:*` (9 routines) | Comm texts | `2555:*` (7) | `2215:*` (4) | Same structure, other texts (§10) |
| `3292:0011` | `AI_UpdateShips` | `2D26:0011` | `2935:0011` | Per-flight cases removed, alive-flag prologue (§5.2) |
| `3A7E:009D` | `Dos_Exec` | — | — | Removed |

## 16. Revision history

| Rev | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-11-02 | RE-7 (Archaeologist + Spec author) | Initial: matcher, unit identity, DS map, flight/weapons/AI/HUD/audio/front-end/narrative/mission-framework deltas, data tables |
| 2 | 2026-11-02 | RE-9 (M10-01) | §10 message-table row is `Msn_LoadTargetInfo` `1E0B:0DCC`; `Msn_UpdateB` completed; OB-018 E2 instance; U-200, U-201, U-205 resolved; corrections §13.1. E2 per-flight content now lives in sim/e2-mission-setup, sim/e2-mission-logic, sim/e2-mission-ships and flow/e2-program-flow |
| 3 | 2026-11-05 | RE-10 (M10-02) | E3 corrections (§13.2): flight-12 patch, hazard tiles on `td4.map` only (C-0041), U-202 and U-203 resolved, U-205 confirmed for E3 (C-0036, C-0041), comm viewer order, `Msn_LoadTargetInfo` `1BD8:0DCF`, OB-028 corrected, campaign details. E3 per-flight content now lives in sim/e3-mission-setup, sim/e3-mission-logic, sim/e3-mission-ships and flow/e3-program-flow |
