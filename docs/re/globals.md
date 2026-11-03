# Global data map (`file0001.exe`)

Our names for the game's global variables in DGROUP (`file0001:3C43`, addressed as `DS:offset`), M1-07. Record layouts that these globals point to or form are in [structures.md](structures.md). Symbol rows for the names below are in [symbols/file0001.csv](symbols/file0001.csv).

**Method.** A scratch Ghidra listing of every code segment (private project copy, 2026-11-02) was scanned for every DGROUP operand (direct and `[reg + disp]`), giving reads, writes, widths and the functions that use each offset (925 distinct offsets). Meanings come from reading the users (static) and from the C-0002 RAM dumps taken 1 s and 2 s into mission 1 (runtime DGROUP = PSP 1398h + 10h + 3C43h = 4FEBh). Confidence per row: CONFIRMED (code + capture agree, or two code lines of evidence), LIKELY, HYPOTHESIS.

**Layout.** DS:0000..04B1 = the game's typed constants (initialised; tables of §1), DS:04B2..050F = System typed constants ([bp7-runtime-semantics §3.2](../spec/core/bp7-runtime-semantics.md)), DS:0502..9DF1 = uninitialised globals (zeroed at start-up), DS:9E00.. = stack. **Every BSS global starts at 0.** Pascal arrays are 1-based: an `array[1..n]` placed at address A is accessed as `[index + (A − 1)]`, so the code shows `A − 1` as the displacement (for example the slot class array is used as `[slot + 3A79h]` and lives at DS:3A7A..3A8D).

## 1. Typed constants (game tables, initialised from the file)

| Address | Type | Our name | Meaning | Conf. |
|---|---|---|---|---|
| DS:0000..0262 | text | `k_ConfigScreenText` | Text-mode configuration screen and help (attribute bytes interleaved). Game text: not reproduced | LIKELY |
| DS:0264..02AA | u8[0..70] | `k_TileRadarColour` | Radar colour of each map tile (resource `hud.radar_tile_colours`; [hud](../spec/render/hud.md) §3.4). The bytes earlier listed as `k_Class0295` (DS:0295..029F) and `k_Class02A2` (DS:02A2..02AA) are its entries for tiles 49..59 and 62..70 | CONFIRMED (RE-6: code + C-0002 radar) |
| DS:02AC..02BD | u8[18] | `k_HeadingRing` | The 16 headings in clockwise order with wrap entries: `01, [0E 04 4A 07 4E 08 62 09 60 06 24 03 20 02 0C 01], 0E` (structures §7) | CONFIRMED |
| DS:02BE + c | u8 | `k_ClassMaxSpeed` | Maximum speed component per class c (biased by 16, structures §6) | CONFIRMED |
| DS:02C8 + c | u8 | `k_ClassArmour` | Armour per class (player: as is; mission ships: × 2) | LIKELY |
| DS:02D2 + c | u8 | `k_ClassShields` | Shields per class (also the regeneration cap) | LIKELY |
| DS:02DC + c | u8 | `k_ClassAccel` | Thrust step per tick per class | CONFIRMED |
| DS:02E5 + 17c + m | u8 | `k_ClassShotTable` | Per class c, 17 muzzle entries m: non-zero = fire from muzzle m; value − 1 = shot kind | LIKELY |
| DS:035C + 34h + 2m | i8[2] | `k_MuzzleOffsets` | Muzzle (dx, dy) in pixels per heading h (keypad digit) and muzzle m = 1..17 | LIKELY |
| DS:0380..04B1 | u8[] | (rest of the muzzle table and other data) | Not decoded | — |

## 2. Front end and campaign

| Address | Type | Our name | Meaning | Conf. |
|---|---|---|---|---|
| DS:0502 | u16 | `g_SoundCard` | WORX card detection result (FFFFh = none; set to FFFFh when `g_NoSoundDriver`) | LIKELY |
| DS:060A..060C | u8 × 3 | `g_CfgEdit` | Working copies of music / SFX / controller on the configuration screen | LIKELY |
| DS:060D | u8 | `g_CfgCursor` | Configuration screen cursor (1..4) | LIKELY |
| DS:060E | u8 | `g_MenuExitItem` | Menu item that ended the menu | HYPOTHESIS |
| DS:060F | u8 | `g_GameOver` | Set by every ending of a mission block | LIKELY |
| DS:0610 | u8 | `g_MissionAdvance` | 1 = the mission block completed; the menu then advances the mission | LIKELY |
| DS:0752 | u8 | `g_ResumeStatus` | First byte of `tdstatus.dat` (0 none, 1 ordering info, 2 about) | CONFIRMED |
| DS:0753 | u8 | `g_MenuPrevCursor` | Previous menu cursor (for redraw) | LIKELY |
| DS:0754 | u8 | `g_MenuCursor` | Main-menu cursor 0..8 | CONFIRMED |
| DS:0756 | u8 | `g_MenuDirty` | Menu needs redraw | LIKELY |
| DS:0757 | u8 | `g_QuitConfirmed` | Quit answered with Y | LIKELY |
| DS:0758 / 0759 | u8 | `g_KeyChar` / `g_KeyScan` | Last `ReadKey` character and extended scan code (menus) | CONFIRMED |
| DS:075A | FileRec (128) | `g_File` | File variable shared by the main program (td.cfg, tdstatus.dat, palettes) | CONFIRMED |
| DS:07DA, 0AE0, 0AE2 | u16 | `g_FadeI`, `g_FadeJ`, `g_FadeK` | Loop counters of the menu palette fades | LIKELY |
| DS:07E0 | u8[768] | `g_MenuPalette` | `tdmen.pal` copy (passed to save/restore screens) | LIKELY |
| DS:0A80..0A8A | u8 | `g_MenuItemColour` | Highlight colour per menu item (3Fh lit, 10h dim, 0 hidden) | HYPOTHESIS |
| DS:0C28 | string | `g_TextBuf` | Scratch ShortString for screen text | LIKELY |
| DS:0D28 | u8 | `g_ScriptChoice` | 1 = `dialogue.dat`, 2 = `safedial.dat` | CONFIRMED |
| DS:0D29 | u8 | `g_MissionNumber` | 0 = no game, 1..20 next mission (saved, and in `tdstatus.dat`) | CONFIRMED |
| DS:0D2A | i16 | `g_KillsToDate` | Kills before the current mission | CONFIRMED |
| DS:12C6 | u8 | `g_CfgMusic` | 1 off, 2 on; 0 = show configuration screen | CONFIRMED |
| DS:12C7 | u8 | `g_CfgSfx` | 1 off, 2 Sound Blaster, 3 PC speaker (`S` chord toggles 1↔3 in flight) | LIKELY |
| DS:12C8 | u8 | `g_CfgController` | 1 keyboard, 2 joystick | CONFIRMED |
| DS:12CA | i16 | `g_MissionKills` | Kills in the current flight | CONFIRMED |
| DS:12CC | u8 | `g_MissionLost` | Mission failed by objective | LIKELY |
| DS:12CD | u8 | `g_PlayerDied` | Flight ended by the player's death | LIKELY |
| DS:12CE | u8 | `g_MissionAborted` | Player quit the flight (Esc menu) | LIKELY |
| DS:12CF | u8 | `g_MissionOutcome` | Mission-specific success flag (flow spec §3.4) | LIKELY |
| DS:12D0 | u16 | `g_SongHandle` | WORX handle of the playing song | LIKELY |
| DS:12D2 | farptr | `g_SongBuffer` | Loaded CMF data | CONFIRMED |
| DS:13DC | u8 | `g_NoSoundDriver` | `/nosound` or `/pc`: never call WORX | CONFIRMED |
| DS:1F4E | u8 | `g_SkipStory` | Esc pressed during a story screen | CONFIRMED |
| DS:1F56..1F5C | u16 × 4 | `g_JoyCal` | Joystick calibration (saved in `tdstatus.dat`) | LIKELY |
| DS:3946 | u16 | `g_MusicSize` | Size of `g_SongBuffer` (name from M1-02) | CONFIRMED |

## 3. Input

| Address | Type | Our name | Meaning | Conf. |
|---|---|---|---|---|
| DS:1A5C + sc | u8[600] | `g_KeyDown` | Key state by scan code sc (1 = down); E0-prefixed keys at +100h. Cleared at flight start and by modal screens | CONFIRMED |
| DS:1CB6 | u16 | `g_KeyExtOffset` | 100h after an E0 prefix | CONFIRMED (M1-03) |
| DS:1CB3 | u8 | `g_AnyKeyLatch` | Pause: set when any key byte becomes 1 | LIKELY |
| DS:1E30 | u8 | `g_IsrFlag` | Set by the keyboard ISR when DS:1AA1 (scan 45h, Num Lock) is down | HYPOTHESIS |
| DS:1F67 | i8 | `g_JoyX` | Joystick X: 1 = left, −1 (FFh) = right, 0 centre | LIKELY |
| DS:1F68 | i8 | `g_JoyY` | Joystick Y: 1 = up, −1 = down | LIKELY |
| DS:1F69 | u8 | `g_JoyButton2` | Second button: fire, like Space | LIKELY |
| DS:1F6A | u8 | `g_JoyButton1` | First button (afterburner burst, like Ins / keypad +) | LIKELY |

Flight key bytes read by the code (scan code set 1; `+E0` = the grey cursor-block key):

| Byte | Key | Use in flight |
|---|---|---|
| 1AA4 / 1BA4 | 48h keypad 8 / grey Up | Thrust up, steer |
| 1AAC / 1BAC | 50h keypad 2 / grey Down | Thrust down |
| 1AA7 / 1BA7 | 4Bh keypad 4 / grey Left | Thrust left |
| 1AA9 / 1BA9 | 4Dh keypad 6 / grey Right | Thrust right |
| 1AAA | 4Eh keypad + | Afterburner burst, classes 2 and 5 only (byte cleared after use: one burst per make code) |
| 1AAE | 52h Ins / keypad 0 | Afterburner burst (same) |
| 1A94 / 1B94 | 38h Alt / right Alt | Infra-red toggle (`0E8B:B1EC`; the IR notice says so, string `0E8B:B807`) |
| 1A95 | 39h Space | Fire: volley every second tick while held, or one missile per press ([weapons](../spec/sim/weapons.md)) |
| 1A6B | 0Fh Tab | Switch between primary weapon and missiles |
| 1A8E | 32h M | Replay the HUD ticker message |
| 1A5D | 01h Esc | Flight menu |
| 1A75 | 19h P | Pause |
| 1A9B | 3Fh F5 | Ship-switch screen (`0E8B:A714`, not in mission 14) |
| 1A9C | 40h F6 | Comm viewer |
| 1A7B | 1Fh S | Toggle SFX between off (1) and PC speaker (3) |

## 4. Flight state (player and view)

| Address | Type | Our name | Meaning | Conf. |
|---|---|---|---|---|
| DS:1703 | u8 | `g_PlayerDrawLate` | Draw the player after the mission ships (tick-order §2) | LIKELY |
| DS:1704 | u8 | `g_PaletteMode` | 0 normal, 1 infra-red (dimmed palette), 2 red-only | LIKELY |
| DS:1705 | u8 | `g_CommIndicatorPhase` | F6 indicator pulse phase 1..7 while a comm is incoming (`0E8B:7FDF`) | LIKELY |
| DS:1706 | u8 | `g_CommRequest` | F6 pressed; comm viewer opens on the next even page | LIKELY |
| DS:1708, 1710, 1718 | | (timing) | `g_TickPrevTime`, `g_TickNowTime`, `g_MissionTicks` (ticks since flight start, CONFIRMED) | see [timing](../spec/core/timing.md) |
| DS:16F6 | u8 | `g_PlayerSprite` | Sprite set used to draw the player (= class, or 2/3 alternating in late mode) | LIKELY |
| DS:16F8 | string[10] | `g_MissionFileName` | `m<n>.td` built at setup | CONFIRMED |
| DS:1828 | u8 | `g_FlashFrames` | Full-palette flash countdown (4 white … 1 restore) | LIKELY |
| DS:12C4 | u16 | `g_HitFlash` | Red hit-flash intensity, decays 2 or 3 per tick | LIKELY |
| DS:1836 / 1837 | u8 | `g_BurstFrame` / `g_BurstHeading` | Afterburner burst animation (classes 2 and 5), 0 = idle, 1..5; frames 2 and 3 set the speed in the burst heading | LIKELY |
| DS:194C | FileRec | `g_FlightFile` | File variable for flight loads | CONFIRMED |
| DS:1A52 / 1A54 | u16 | `g_I` / `g_J` | **Shared global loop counters** used by most game routines (a nested call clobbers the caller's counter) | CONFIRMED |
| DS:1A56 | u8 | `g_PlayerHeading` | Heading as keypad code (structures §7) | CONFIRMED |
| DS:1A57 | u8 | `g_PlayerClass` | Player ship class (per mission, structures §6) | CONFIRMED |
| DS:1A5A | u8 | `g_PlayerTurnDir` | 1 = counter-clockwise, 2 = clockwise | CONFIRMED |
| DS:1CB8 | u8 | `g_Weapon` | Selected weapon: 1..3 class volley, 4/5 missile kinds, 6 homing missile ([weapons §3.2](../spec/sim/weapons.md)) | LIKELY |
| DS:1CB9 | u8 | `g_Missiles` | Missiles left (HUD bar; the T+I+M chord sets 30) | LIKELY |
| DS:1CBA | u8 | `g_MissileCooldown` | 4 after a launch, −1 per tick (HUD colour) | LIKELY |
| DS:1CBB | u8 | `g_PlayerTargetHeading` | Heading requested by the steering keys | CONFIRMED |
| DS:1CC8..1CCB | u8 | `g_LastCrossing` | Direction of the last tile-boundary crossing of the view: 1CC8 left, 1CCA right, 1CC9 up, 1CCB down (1CC9 is also a scratch byte in the collision swap) | LIKELY |
| DS:1E2E | u16 | `g_EdgeHit` | Player touched the map limit (jurisdiction) | LIKELY |
| DS:32B7 | u8 | `g_MissionOver` | Flight is ending (death, success, timer) | LIKELY |
| DS:32B8 | u8 | `g_EndTicks` | Ticks since `g_MissionOver`; player hidden from 15; loop ends at 35 | LIKELY |
| DS:32B9 | u8 | `g_FlightNumber` | `Sim_FlyMission` argument (1..21) | CONFIRMED |
| DS:32BA | u8 | `g_MissionPhase` | Mission-script state: 1 at start, 100 = objectives done ([mission-logic](../spec/sim/mission-logic.md)) | CONFIRMED (RE-6, C-0010) |
| DS:32BC | u16 | `g_TickerPos` | HUD ticker scroll position 1..78 (0 idle); a new edge alert is raised only while it is 0 (`0E8B:7FDF`) | CONFIRMED (C-0010) |
| DS:32BE, 32C0, 32C2 | u16 | `g_MsnI`, `g_MsnJ`, `g_MsnK` | Loop counters of the mission rules (unit `2404`) | CONFIRMED |
| DS:329C | u8 | `g_CommIncoming` | Incoming transmission waiting (F6); raised and cleared by mission logic, cleared by the comm viewer | CONFIRMED (code) |
| DS:329D | u8 | `g_MsnEventCount` | Flight 17: base hits, then the destruction counter; flight 20: lander hits | LIKELY |
| DS:32A0..32B3 | u8[1..20] | `g_BaseHit` | Flight 17: slot already counted as reaching the base | LIKELY |
| DS:32B1 | u8 | `g_IonBomb` | Flight 18: bomb timer expired (mission lost) | LIKELY |
| DS:32B4 | u16 | `g_IonTimer` | Flight 18: ticks since the ion truck was found | LIKELY |
| DS:32B6 | u8 | `g_CutsceneRequest` | Flight 17: play the `td1-17.pic` cut-scene in `Msn_UpdateEvents` | LIKELY |
| DS:329E | u8 | `g_MissionSucceeded` | Ends the flight normally; re-evaluated every tick by the base test | CONFIRMED (code) |
| DS:3958..396C | u8[1..21] | `g_SlotMarker` | Guidance marker per slot (0 none, 32h target, 39h friend, 3Dh home); element 21 = home point | CONFIRMED (RE-6, C-0002) |
| DS:398E | i8 | `g_PlayerArmour` | Player armour; < 1 = destroyed | CONFIRMED (C-0002: 25 for class 4) |
| DS:398F | i8 | `g_PlayerShields` | Player shields; +1 per tick up to `k_ClassShields` | CONFIRMED (C-0002: 25) |
| DS:3995..3998 | u8 × 4 | `g_PlayerSpeed` | Speed components up, down, left, right (biased by 16) | CONFIRMED (code); C-0002: all 0 at rest |
| DS:3A30 | u8 | `g_DrawPage` | Back page 0/1, toggled every tick | CONFIRMED |
| DS:3A36 / 3A38 | i16 | `g_ViewTileX` / `g_ViewTileY` | Map tile of the view origin (x 6..183, y 7..190) | CONFIRMED |
| DS:3A3A / 3A3C | i16 | `g_ViewSubX` / `g_ViewSubY` | Pixel offset in that tile, kept in 10h..30h | CONFIRMED |

## 5. Cheat chords and hostile code (flight, `0E8B:F6B5..F7E0`)

Checked every tick after the player specials. A chord is several key bytes down at once. Recorded for completeness: the port implements none of the hostile ones (AGENTS.md rule 10) and the others only by decision.

| Keys held | Effect | Port |
|---|---|---|
| T + I + M | `g_PlayerArmour` := 64, `g_Missiles` := 30 | decision (D5/D9) |
| H + A + R + D | DS:13DE += 1 (difficulty/cheat level; also set to 8 by M+K+A+R) | decision |
| J + O + H + N | DS:13DE := 0 | decision |
| J + D + W | player class := 6 and **patches the class table** (max speed of class 6 := 1Ch, accel := 4) | decision |
| M + K + A + R | player class := 0, every slot's record byte +3F6h := FFh, DS:13DE := 8, armour 1, shields 0 | decision |
| E + V + L | `Dos_Exec` of the drive-relabel program with the pilot's name (strings `0E8B:B84F`/`B860`) | **never port** |
| P + L + A + Y | `Dos_Exec` of the relabel program with the title (strings `0E8B:B86A`/`B874`) | **never port** |
| (flight 20) view tile x > 170 and y > 170 | `g_PlayerArmour` := 64 | preserve (mission rule) |

## 6. Mission slots, projectiles and explosions (arrays)

Layouts and field meanings: [structures.md](structures.md) §2–§5.

| Address | Element | Our name | Conf. |
|---|---|---|---|
| DS:3A52..3A65 | u8[1..20] | `g_SlotHdrA` (mission file bytes 0..19) | CONFIRMED (load) |
| DS:3A66..3A79 | u8[1..20] | `g_SlotHdrD` (file bytes 60..79) | CONFIRMED (load) |
| DS:3A7A..3A8D | u8[1..20] | `g_SlotClass` (file bytes 40..59, overridden per mission; FFh = empty or destroyed) | CONFIRMED |
| DS:3A8E..3AA1 | u8[1..20] | `g_SlotHdrB` (file bytes 20..39) | CONFIRMED (load) |
| DS:3AA2..3AB5 | u8[1..20] | `g_SlotHeading` (keypad code) | LIKELY |
| DS:3958..396B | u8[1..20] | `g_SlotMarker` (target bracket 0 / 32h / 39h, [mission-setup](../spec/sim/mission-setup.md)) | CONFIRMED |
| DS:3A32 | farptr | `g_ShipTileAttr`: **never assigned** (nil); `3292:0011` reads the interrupt vector table through it (OB-013) | CONFIRMED |
| DS:3AB6..3AC9 | u8[1..20] | `g_SlotTargetHeading` | CONFIRMED |
| DS:3ACA..3AF1 | i16[1..20] | `g_SlotSubX` | LIKELY |
| DS:3AF2..3B19 | i16[1..20] | `g_SlotSubY` | LIKELY |
| DS:3B1A..3B2D | u8[1..20] | `g_SlotTurnDir` (1 ccw, 2 cw) | CONFIRMED |
| DS:3B2E..3B41 | u8[1..20] | `g_SlotTileX` | CONFIRMED |
| DS:3B42..3B55 | u8[1..20] | `g_SlotTileY` | CONFIRMED |
| DS:3B6A..3BB9 | u8[1..20][4] | `g_SlotSpeed` (up, down, left, right; biased by 16) | CONFIRMED |
| DS:3BBA..3BCD, 3BCE..3BE1 | u8[1..20] | `g_SlotGoalX`, `g_SlotGoalY` | CONFIRMED |
| DS:3BE2..3BEA | u8[1..9] | `g_TickFlags`: thrust flags of the ship being moved (3BE3 up, 3BE5 right, 3BE7 left, 3BE9 down); cleared after each ship and at tick start | CONFIRMED |
| DS:9766..976E | | `34E6` unit: player tile (9766/9767), 976A (1, unused), AI range 976B (3), detour step 976C/976E (i16) ([mission-ships §2](../spec/sim/mission-ships.md)) | CONFIRMED |
| DS:3BEC / 3BF0 | farptr | `g_PathX` / `g_PathY` (20 × 501 bytes each) | CONFIRMED |
| DS:3BF6..3CA9 | u8[1..20, 1..9] | `g_FireRequest` (shares the base DS:3BEC with the index arithmetic) | LIKELY |
| DS:3CAA | farptr | `g_TileAttr` (tile → 1 solid, 2 open) | CONFIRMED |
| DS:396E, 3972, 3976, 397A, 397E, 3982, 3986, 398A | farptr | Projectile arrays (structures §4) | LIKELY |
| DS:399A..39B7 etc. | u8[1..30] × 5 | Explosion arrays (structures §5) | LIKELY |
| DS:442E | farptr | `g_Map` (201 × 201, column-major) | CONFIRMED |
| DS:4432..9765 | record[1..20] | `g_Ships` (1065 bytes each, structures §3) | LIKELY |
| DS:9770, 9774, 9778, 977C, 9B82, 9B86, 18BE | farptr | Flight buffers: 9770 `td.lt2` (3B6h read), 9774 `td.sc3` (3840h), 9778 `td.sc2` (3840h), 977C `td.shr` (32000), 9B82 `td.shp` (5000h) + `td.exp` (1C00h at +5000h), 9B86 `td.fac` (F50h), 18BE `td.wea` (F30h). Mission extras overwrite parts: `mine.td` (flight 7, 400h at 9B82+C00h), `lander.td` (flight 20, 800h at 9B82 and C00h at 9B82+1000h), `td.hel` (flights 12/15/21, 800h at 9B82+800h), `td.h2` (same flights, C80h each at +C80h of 9778, 977C and 9774) | CONFIRMED (loads) |
| DS:13E6 | u8[768] | `g_FlightPalette` (`td.pal`, used for fades and flashes) | CONFIRMED |
| DS:9780..9A8F | u8[28×28] | `g_RadarBuf` (radar scratch, [hud](../spec/render/hud.md) §4.6) | CONFIRMED (C-0010) |

## 7. Sound-effect unit (`1E2A`)

| Address | Type | Meaning | Conf. |
|---|---|---|---|
| DS:1E34 | farptr | saved INT 1Ch vector (M1-03) | CONFIRMED |
| DS:1E38..1E3B, 1E40 | | SFX unit state | HYPOTHESIS |
| DS:1F0A | u8 | `g_SfxPriority` (M1-02): priority of the effect playing, 0 = idle; cleared when WORX reports idle | CONFIRMED |
| DS:1F0C..1F4C | | SFX channel table (6-byte entries) | HYPOTHESIS |

## 8. Notes for other tasks

- `0E8B:A714` (`UI_ShipSwitchScreen`, named `Inp_RestoreKeyboard` by M1-03 and renamed 2026-11-02) is the **F5 ship-switch screen**: it restores the BIOS keyboard handler for its own `ReadKey` loop, then shows the slot list with change-ship and return prompts.
- The shared loop counters `g_I`/`g_J` (DS:1A52/1A54) mean that routines are not re-entrant; a port must not "fix" any behaviour that depends on a callee leaving them changed (none found yet, but watch for it in M8).
