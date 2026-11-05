# Episode 3 per-flight setup (`td3` `Msn_Setup`, `0C5D:6149`): delta against E1 and E2

| | |
|---|---|
| **Spec ID** | `sim/e3-mission-setup` |
| **Revision** | 1 |
| **Status** | Draft |
| **Fidelity tier** | T1 (setup state, order of writes, RNG use) · T2 for the objective text, the F5 lines and the screen state during loading (presentation) |
| **Confidence** | CONFIRMED (static) for every table value: constants read from the instruction stream of `td3:0C5D:6149..6DFA`, `td3:0C5D:A9CC..BB1B` and `td3:1BD8:0DCF..4121`. The two per-flight routines were also executed from the raw exe bytes with a private 16-bit model of the instructions they use (§9), once per flight, and the store lists checked against the hand reading. Test vectors are computed from the shipped `m<n>.td3` files with these rules. Vectors 1 (flight 1) and 2 (flight 8) are **confirmed** against the last setup frame and ticks 1–2 of captures C-0036 and C-0041; the others remain static (U-285) |
| **Tasks** | M10-02 (feeds the E3 mission specs and the E3 first tick) |
| **Unknowns** | U-284, U-285 (vectors 1, 2 confirmed), U-286 (new, §8); U-205 (E2 resolution applies, §4.1); U-270 (flight 7 phase, [e3-mission-logic](e3-mission-logic.md)) |
| **Read with** | [e2-mission-setup.md](e2-mission-setup.md) rev 1 (E2), [mission-setup.md](mission-setup.md) rev 1 (E1), [mission-ships.md](mission-ships.md) §3.1/§4.1, [weapons.md](weapons.md) §3.2, [flight-screens.md](../flow/flight-screens.md) §3.4, [episode-deltas.md](../../re/episode-deltas.md) rev 2 §4.3, §4.4, §6, §10, §11, [e3-mission-logic](e3-mission-logic.md) rev 1 |

## 1. Summary

E3's `Sim_FlyMission` (`td3:0C5D:A9CC`) prepares a flight the way E2 does ([e2-mission-setup §4.1](e2-mission-setup.md)): it resets the ship records, loads `m<n>.td3` with the same six block reads and the same record set-up, sets the start view and calls `Msn_Setup` (`td3:0C5D:6149`), which calls the F5 routine `Msn_LoadTargetInfo` (`td3:1BD8:0DCF`) first and then runs one hand-written block for the flight. The differences from E2 are:

- **One map set** (`g_MapSet` := 1 always) but **two maps**: `td3.map` for flights 1–6, `td4.map` for flights 7 and up; always `td3.blk`; one tile-attribute table (41h–46h open) and the radar colours as they are.
- **Start views are final before `Msn_Setup`**; nothing overrides them later (E2 overrode them after the HUD was drawn).
- One block per flight in the dispatch order **1..11, 21, 12..20** (table §3.2). Flight 20's block is dead (E3 never flies flight 20). **Flight 7's block has no phase store** (OB-043), **flight 18's block fills the markers with 32h instead of clearing them** (OB-053), flight 17's block **restores the class tables** that flight 16 patches (OB-028), and flight 12's block **writes 27 map cells**.
- No palette-mode, late-draw or comm writes anywhere in setup: every E3 flight starts with palette mode 0, late draw 0 and no transmission waiting. No extra sprite files.
- Player state: class 2 in flight 12 starts with **10 missiles**; flight 7 starts with **5 armour** (as E2).
- The F5 routine has a second fixed point (**entry 22**) in flights 8, 10 and 11, writes no flag for entry 21 in **flight 9** (OB-054) and stores flag 0 for it in flight 18.
- Song cue 44 instead of E2's 45 (same song, `td03`).
- Like E2, no setup loop calls `AI_ChooseWaypoint`, and the only RNG use before the first tick is the 20 draws of `AI_ResetShipRecords`.

Everything not listed here is as in E2 (and, through it, E1).

## 2. Definitions and units

E3 addresses (DGROUP `32C5`; [episode-deltas §3.2](../../re/episode-deltas.md#32-dgroup-offset-map), [td3.csv](../../re/symbols/td3.csv)). "Record field +f of slot s" is DS:4356 + (s − 1) × 429h + f, as in E1 and E2.

**Slot-numbering trap.** The listing annotations of the per-slot arrays are 0-based: the annotation `g_SlotClass[7]` is DS:39A5, which is **slot 8** (DS:399D + 8). Every slot number in this spec is 1-based and was computed from the address.

| Term | E3 address | Type | Notes |
|---|---|---|---|
| Flight n | `g_FlightNumber` DS:31DD | u8 | 1..19 and 21 are flown (flight 20 never, [e3-program-flow](../flow/e3-program-flow.md) §4.3); flight n loads `m<n>.td3` (always) |
| Mission phase | `g_MissionPhase` DS:31DE | u8 | := 1 by every block except 7 (none) and 17 (100) |
| Ticker | `g_TickerPos` DS:31E0 | u16 | := 0 early in setup, := 1 by every block |
| Marker of slot s | `g_SlotMarker` DS:387B + s | u8 | 0, 32h or 39h; element 21 (DS:3890) is the home-point marker, 3Dh |
| Class of slot s | `g_SlotClass` DS:399D + s | u8 | FFh = no ship |
| Tile of slot s | `g_SlotTileX` DS:3A51 + s, `g_SlotTileY` DS:3A65 + s | u8 | path step 1 |
| Aggression | record +3F6h | u8 | 0 after `AI_ResetShipRecords` |
| Shields / armour | record +3F8h / +3FAh | i16 | k_ClassShields[file class] / 2 × k_ClassArmour[file class]; class FFh reads DS:052F = 34 and DS:0525 = 34, so shields 34 and armour 68 (as E1 and E2) |
| Player class | `g_PlayerClass` DS:197B | u8 | |
| Player armour, shields | `g_PlayerArmour` DS:38B2, `g_PlayerShields` DS:38B3 | i8 | armour is the byte **before** shields; set after `Msn_Setup` |
| Weapon, missiles, missile cooldown | DS:1BDC, DS:1BDD, DS:1BDE | u8 | set after `Msn_Setup` |
| Palette mode | `g_PaletteMode` DS:161E | u8 | 0 at every flight start (§3.3) |
| Late draw | `g_PlayerDrawLate` DS:161D | u8 | 0 at every flight start; E3 never sets it |
| Comm incoming | `g_CommIncoming` DS:31C0 | u8 | 0 at every flight start; no setup write |
| Map set | `g_MapSet` DS:12FE | u8 | := 1 always |
| Map | far pointer `g_Map` DS:4352 | u8[40401] | 201 × 201, cell (x, y) at x × 201 + y; reloaded at every flight start |
| Tile attributes | far pointer `g_TileAttr` DS:3BCE | u8[71] | 1 = solid, 2 = open; the block is 47h bytes, allocated once by the AI-unit initialisation (`td3:2B72:3866`) |
| Radar colours | `k_TileRadarColour` DS:036A | u8[80] | resource `e3.hud.radar_tile_colours`; E3 never copies DS:03BA over it |
| Class tables | max speed DS:041C, armour DS:0426, shields DS:0430, accel DS:043A | u8[10] | byte-identical to E1/E2 in the exe image: max speed 24, 28, 32, 28, 28, 28, 24, 28, 32, 0; armour 31, 16, 15, 20, 25, 25, 31, 25, 20, 0; shields 0, 0, 18, 20, 25, 20, 31, 25, 25, 0; accel 2, 4, 4, 4, 4, 4, 2, 4, 4, 0 (class 0..9) |
| View | tile X/Y `g_ViewTileX/Y` DS:395A/DS:395C, offsets `g_ViewSubX/Y` DS:395E/DS:3960 | i16 | player tile = view + (5, 3) |
| F5 entries | flag DS:1E98 + e, map X DS:1EB8 + e, map Y DS:1ED8 + e, line l of entry e at DS:1EF8 + 155e + 31l | u8, u8, u8, string[30] | E2 layout at −4; e = 0..31, l = 0..4; DS:1EAD is the flag of entry 21 (U-202, resolved: [e3-mission-logic](e3-mission-logic.md) §4.9) |
| Music buffer | `g_MusicBuffer` DS:11F2 | far pointer | nil = no song loaded |
| Key-down table | `g_KeyDown` DS:1980 | u8[601] | indices 0..600 |

## 3. Data

### 3.1 Map selection

| Flights | `g_MapSet` | Map | Tiles | Radar colours | Tile attributes |
|---|---:|---|---|---|---|
| 1–6 (`g_FlightNumber` ≤ 6) | 1 | `td3.map` | `td3.blk` | `k_TileRadarColour` as it is | §3.5 |
| 7–19, 21 (> 6) | 1 | `td4.map` | `td3.blk` | as it is | §3.5 (same table) |

`g_MapSet` := 1 is an unconditional store (`0C5D:AA71`). The code still tests it: the tile-attribute fill (`ABFC`) and the `td3.map` name assignment (`AD05`) run only when it is 1, and then `td4.map` replaces the name when the flight is above 6 (`AD1B..AD2C`). There is no set-2 branch any more, so the E2 radar copy (OB-024) cannot happen in E3.

### 3.2 Per-flight table (`Msn_Setup`)

Common part. Every block except 18 and 20 clears the markers of slots 1..20 to 0 before its own marker writes; block 18 writes 32h to all twenty instead (OB-053); block 20 has no marker writes. Every block except 20 sets marker element 21 := 3Dh after its own markers. Every block draws its objective with `Txt_FlightDrawStringOpaque(0, 735, text, ink)` (`0C5D:45C8`, ticker strip, VGA row 735) and then sets the ticker := 1. The objective is element k of `e3.objectives` (21 strings at `0C5D:59C8..6148`, file offset 15868h): k = n − 1 for flights 1..11, k = 11 for flight 21, k = n for flights 12..20. Ink is 37 for flights 1–7, 10, 11, 21 and 42 for flights 8, 9, 12–20.

Lists are in the order the code writes them; a later write to the same slot and field wins. Armour, shields and aggression are record fields of the slot. The "Start view" column is the final view tile (§4.1 step 9; no later override); view offsets are (32, 16) in every flight. "Markers" lists only non-zero values; slots not listed have marker 0 (except flight 18). The full store order of each block is in §4.2.

| Flight | Player class | Start view (tile) | Slot class overrides | Markers | Armour / shields | Aggression | Phase | Objective (ink) | Other |
|---:|---:|---|---|---|---|---|---:|---|---|
| 1 | 2 | (167, 66) | 2–4 := FFh; 11–14 := FFh | 2–4, 8, 9, 11–14 := 32h | | 2–14 := 200 | 1 | [0] (37) | slots 2–4 (file class 0) and 11–14 (file class 7) are hidden; mission logic gives them class 7 later ([e3-mission-logic](e3-mission-logic.md) §5.1) |
| 2 | 2 | (167, 66) | 15–17 := 7 | 2–7 := 32h | | 9–18 := 255 | 1 | [1] (37) | slot 15 is class 7 in the file already; 16, 17 are file class 6 |
| 3 | 2 | (167, 66) | | 2–4, 9–12 := 32h | | 6–14 := 255 | 1 | [2] (37) | |
| 4 | 4 | (167, 66) | | 5–7 := 32h | | 2–15 := 255 | 1 | [3] (37) | player class written after the aggression loop |
| 5 | 2 | (167, 66) | **3 := 2** | 2 := 39h; 5–7 := 32h | armour 2 := 900 | 3–12 := 250, then 5–12 := 150 | 1 | [4] (37) | the class store is to slot 3 (DS:39A0), file class 4; slot 2 keeps file class 0 (§5) |
| 6 | 2 | (167, 66) | | 2–10 := 39h | | 2–17 := 200 | 1 | [5] (37) | player class written after the objective |
| 7 | 2 | (185, 32) | | 2–10 := 39h | | 12–18 := 75 | **none** | [6] (37) | no phase store (OB-043); player class written after the objective; player armour 5 (§3.3) |
| 8 | 6 | (17, 164) | 11–20 := FFh | 1–3 := 39h | armour 1, 2, 3 := 100 | 4–10 := 255 | 1 | [7] (42) | slots 11–20 are empty in `m8.td3` already; the phase store is the last store of the block |
| 9 | 6 | (17, 164) | 10–20 := FFh | 1–3 := 32h | armour 1, 2, 3 := 100 | 4–9 := 255 | 1 | [8] (42) | as 8 (slots 10–20 empty in the file) |
| 10 | 6 | (17, 164) | 5 := FFh, then 4 := FFh | 5 := 39h; 7–10 := 32h | armour 5 := 100 | 3–13 := 255 | 1 | [9] (37) | the two class stores are the last stores of the block; slots 4 (file class 2) and 5 (file class 0) are hidden |
| 11 | 6 | (17, 164) | | none | | 3–15 := 255 | 1 | [10] (37) | |
| 21 | 6 | (160, 75) | | none | | 3–15 := 255 | 1 | [11] (37) | `m21.td3` is byte-identical to `m11.td3` |
| 12 | 2 | (17, 164) | | 1–6 := 32h | | 8–15 := 200 | 1 | [12] (42) | **27 map cells := 26h** (§4.2); missiles 10 (§3.3) |
| 13 | 7 | (17, 164) | | none | | 3–8 := 200 | 1 | [13] (42) | |
| 14 | 3 | (88, 76) | 6–11 := FFh | none | | 3–17 := 255 | 1 | [14] (42) | slots 6–11 (file classes 8, 8, 8, 7, 7, 7) are hidden; the timed events give them classes 8 and 7 at tick 25 |
| 15 | 2 | (17, 164) | | 15 := 32h | **shields 15 := 1750** | 4–15 := 255 | 1 | [15] (42) | OB-048 |
| 16 | 2 | (17, 164) | | 2–4 := 32h | | 6–14 := 255 | 1 | [16] (42) | player class written last |
| 17 | 3 | (88, 76) | | none | | 3–7 := 255 | **100** | [17] (42) | the class tables are rewritten first (§4.3) |
| 18 | 3 | (17, 164) | | **1–20 := 32h**, then 3–6 := 32h | | 3–9 := 255 | 1 | [18] (42) | no marker clear (OB-053) |
| 19 | 7 | (88, 76) | | 2 := 32h | armour 2 := 500 | 2 := 255 | 1 | [19] (42) | |
| (20) | 5 | (17, 164) | | no marker writes, element 21 not written | | 2–11 := 10; 12–15 := 220 | 1 | [20] (42) | dead block (never reached) |

Notes:

- No block writes a path, a slot tile, the comm flag, the palette mode, late draw or any record field other than +3F6h, +3F8h (flight 15 only) and +3FAh. No block calls anything but `Msn_LoadTargetInfo` (once, before the blocks) and the objective draw.
- E3 has no class-table **patch** in setup (E2 flight 8's OB-018 instance has no E3 counterpart); flight 17 writes the original values back (§4.3).
- Flight 12's map writes go to the live map buffer, which is read from `td4.map` at every flight start (`0C5D:B47E`, before `Msn_Setup`), so nothing carries over to a later flight.

### 3.3 Player state after `Msn_Setup`

| Item | Rule (`td3:0C5D:`) | Change from E2 |
|---|---|---|
| Primary weapon | only if class > 1: 2, 3, 8 → 1; 4, 5, 7 → 2; 6 → 3 (`B6BF..B718`); the weapon icon is drawn in the same branch. Class 0/1 keep the previous weapon (unreachable: no E3 flight uses them) | none |
| Missiles | := 0 and cooldown := 0; then, only if class > 1: 2 → 6; **if flight = 12 and class = 2 → 10**; 3 → 20; 4 → 4; 5 → 4; 6 → 6; 7 → 0; 8 → 0 (`B77A..B7ED`) | flight 12 rule new |
| Player armour | k_ClassArmour[player class]; then **flight 7: 5** (test `B90E`, store `B915`) | none (E2 has the same flight-7 rule) |
| Player shields | k_ClassShields[player class] (`B91A..B925`) | none |
| Palette mode | := 0 at `B0AB`; no later write in setup or in `Msn_Setup` | E2 set mode 1 again for flights 3 and 10: **E3 never starts in infra-red** |
| Late draw | := 0 at `AA62`; never set | E2 flight 8 set it |
| Comm incoming | := 0 at `A9DF`; never set in setup | E2 flight 11 set it |

Player class per flight: 1–3 → 2; 4 → 4; 5–7 → 2; 8–11 → 6; 21 → 6; 12 → 2; 13 → 7; 14 → 3; 15, 16 → 2; 17, 18 → 3; 19 → 7 (dead 20 → 5).

Resulting player start state (weapon, missiles, armour, shields): class 2 → 1, 6, 15, 18 (flight 7: armour 5; flight 12: missiles 10); class 3 → 1, 20, 20, 20; class 4 → 2, 4, 25, 25; class 6 → 3, 6, 31, 31; class 7 → 2, 0, 25, 25.

### 3.4 F5 target information (`Msn_LoadTargetInfo`, `td3:1BD8:0DCF`)

The routine has E2's structure ([e2-mission-setup §3.4](e2-mission-setup.md)): it clears the flags of entries 1..30, then the block for the flight writes the fixed points (entry 21, in flights 8, 10, 11 also entry 22) and five lines for some entries. Each line is copied from string k of the proposed resource `e3.target_info` with a maximum length of 30 (a longer string keeps its first 30 characters: k = 3, 4, 6, 7, 13, 15, 17–20, 22, 24–26, 28, 30, 31, 34, 37, 39, 49, 57, 61, 69, 82, 90, 110, 111). Three strings are all blanks: k = 2 (30 spaces, the blank line every block uses), k = 25 (31 spaces) and k = 74 (29 spaces).

`e3.target_info` is the run of **114** consecutive ShortStrings that starts at file offset 1F650h (`td3:1BD8:0000`, the routine's literal pool, which ends at `1BD8:0DCE`; the walk ends exactly at `0DCF`). Proposed entry for [td3.toml](../../re/exe-resources/td3.toml), replacing `e3.locations` and `e3.target_info.01`..`.07`:

```toml
[[resource]]
id = "e3.target_info"
kind = "pstring_seq"
offset = 0x1F650
count = 114
note = "F5 target-information lines (literal pool of Msn_LoadTargetInfo, td3:1BD8:0000..0DCE); indexed by sim/e3-mission-setup §3.4"
```

Until it exists, k maps to the current ids as follows (five strings have no id):

| k | Current id |
|---|---|
| 0–2 | `e3.locations[0..2]` (k = 2 is the blank line, not a location) |
| 3, 4 | `e3.target_info.01[0..1]` |
| 5 | `e3.target_info.02` |
| 6 | `e3.target_info.03` |
| 7, 8 | `e3.target_info.04[0..1]` |
| 12 | `e3.target_info.05` |
| 15–17 | `e3.target_info.06[0..2]` |
| 18–113 | `e3.target_info.07[0..95]` |
| 9, 10, 11, 13, 14 | none (needs the toml change, §8) |

k = 111..113 are used only by the dead flight-20 block.

Final state per flight (entry: k for lines l = 0..4; where the code writes an entry twice, the table shows the result; "–" = line not written):

| Flight | Entry: lines l = 0..4 | Fixed points (flag 1) |
|---:|---|---|
| 1 | 0: 3 4 5 2 2; 2..4: 11 7 8 9 10; 5, 6, 11..14: 12 7 8 9 10; 7..10: 6 7 8 9 10; 21: 0 1 2 2 2 | 21 at (172, 68) |
| 2 | 0: 3 4 5 2 2; 2..7: 13 7 8 14 2; 9..12: 15 7 8 16 2; 15..17: 17 7 8 2 2; 21: 0 1 2 2 2 | 21 at (172, 68) |
| 3 | 0: 3 4 5 2 2; 2..4: 18 7 8 19 2; 6, 7: 15 7 8 20 2; 9..12: 21 7 8 22 23; 21: 0 1 2 2 2 | 21 at (172, 68) |
| 4 | 0: 24 4 5 2 2; 2, 3: 24 7 8 25 2; 5..7: 18 7 8 30 2; 9..11: 6 7 8 26 27; 12, 13: 15 7 8 28 29; 21: 0 1 2 2 2 | 21 at (172, 68) |
| 5 | 0: 3 4 5 2 2; 2: 18 4 33 34 35; 3: 3 4 36 25 2; 5..11: 17 7 8 31 32; 21: 0 1 2 2 2 | 21 at (172, 68) |
| 6 | 0: 3 4 5 2 2; 2..5: 24 4 8 37 38; 6, 7: 3 4 8 37 38; 8: 17 4 8 37 38; 9, 10: 18 4 8 37 38; 13, 14: 17 7 8 39 40; 15..17: 15 7 8 39 40; 21: 0 1 2 2 2 | 21 at (172, 68) |
| 7 | 0: 3 4 5 2 2; 2..5: 24 4 8 37 43; 6, 7: 3 4 8 37 43; 8: 17 4 8 37 43; 9, 10: 18 4 8 37 43; 12..14, 19, 20: 6 7 8 39 40; 15..18: 15 7 8 39 40; 21: 41 42 2 2 2 | 21 at (22, 166) |
| 8 | 0: 15 4 5 2 2; 1: 49 4 52 50 51; 2: 49 4 53 50 51; 3: 49 4 54 50 51; 4..10: 17 7 8 55 56; 15, 19: 6 – – – –; 21: 41 42 2 2 2; 22: 44 45 46 47 48 | 21 at (22, 166); **22 at (108, 115)** |
| 9 | 0: 57 4 5 2 2; 1..3: 58 7 8 59 60; 4, 5, 8, 9: 61 7 8 62 63; 6, 7: 12 7 8 62 63; 21: 41 42 2 2 2 | **none**: entry 21 gets (22, 166) and its lines, but no flag store, so its flag keeps the 0 of the clear loop (OB-054) |
| 10 | 0: 69 4 5 2 2; 3: 3 4 73 74 2; 4: 3 4 75 74 2; 5: 18 4 70 71 72; 7, 8, 11, 13: 17 7 8 2 2; 9, 10, 12: 6 7 8 2 2; 21: 41 42 2 2 2; 22: 64 65 66 67 68 | 21 at (22, 166); **22 at (41, 58)** |
| 11 | 0: 15 4 5 2 2; 3..8: 17 7 8 2 2; 9..13: 80 7 8 2 2; 14, 15: 81 7 8 2 2; 21: 41 42 2 2 2; 22: 76 77 2 78 79 | 21 at (22, 166); **22 at (165, 77)** |
| 21 | as flight 11 (its own block, same values) | as flight 11 |
| 12 | 0: 82 4 5 2 2; 1..6: 58 83 8 84 85; 8..15: 6 7 8 86 63; 21: 41 42 2 2 2 | 21 at (22, 166) |
| 13 | 0: 17 4 5 2 2; 3..6, 9, 10: 17 7 8 2 2; 7, 8: 90 7 8 2 2; 11, 12: 18 7 8 2 2; 21: 87 2 2 88 89 | 21 at (41, 46) |
| 14 | 0: 90 7 5 2 2; 3..8, 15..17: 61 7 8 2 2; 9..14: 17 7 8 2 2; 21: 41 42 2 2 2 | 21 at (22, 166) |
| 15 | 0: 3 4 5 2 2; 1, 2: 58 7 8 91 92; 4..14: 80 7 8 93 2; 15: 15 7 94 95 2; 21: 41 42 2 2 2 | 21 at (22, 166) |
| 16 | 0: 3 4 5 2 2; 2..4: 58 7 8 2 2; 6: 24 4 96 2 2; 7: 24 4 97 2 2; 9..14: 12 7 8 2 2; 21: 41 42 2 2 2 | 21 at (22, 166) |
| 17 | 0: 90 7 5 2 2; 3..7: 17 7 99 100 101; 21: 98 2 2 2 2 | 21 at (22, 166) |
| 18 | 0: 90 7 5 2 2; 3..5, 9: 106 7 99 2 2; 6..8: 107 7 99 2 2; 21: 102 103 2 104 105 | none at the start: entry 21 at (28, 33) with its flag stored as **0**; `Msn_E3TimedEvents` sets it to 1 at tick 2160 ([e3-mission-logic](e3-mission-logic.md) §4.9) |
| 19 | 0: 17 7 5 2 2; 2: 6 7 108 109 110; 3, 4: 18 7 99 2 2; 21: 41 42 2 2 2 | 21 at (22, 166) |
| (20) | 0: 111 4 5 2 2; 2..5: 24 4 99 2 2; 6..8: 3 4 99 2 2; 10, 11: 111 4 99 2 2; 12..15: 17 7 99 112 113; 17, 18: 18 4 99 2 2; 19, 20: 49 4 99 2 2 | none (dead block, no fixed-point store) |

- Entries and lines not written keep what an earlier flight of the same program run left (zero-length strings after start-up), as in E1 and E2. Flight 8's entries 15 and 19 get only line 0 (both slots are empty and stay empty in flight 8, so they are never shown).
- The flags of entries 22..30 are cleared every flight; only flights 8, 10 and 11 set entry 22. Entry 31's flag and entry 0's flag are never written here; the F5 screen rewrites entries 0..20 when it opens ([e3-mission-logic](e3-mission-logic.md) §4.9).
- Every slot that is live after setup has five lines in its flight (checked for all 20 flown flights). Lines for slots that are empty at setup (§5) are for ships that mission logic adds later, or for nothing.
- Fixed point 21 equals the guidance home point (§3.7) in every flight except 6 (F5 point (172, 68), home (1, 110)), 9 (no flag) and 11 (F5 point the base (22, 166), home (166, 78); entry 22 at (165, 77) is next to the home point). Entry 22 of flights 8 and 10 lies inside the extra overlay boxes of §3.6 ((108..110, 113..115) and (41..42, 57..58)).
- Block order inside each flight block: flags and coordinates of the fixed points, then the lines of entry 21 (and 22), entry 0, the slot ranges, and last the single-entry line-0 overrides (§4.5).

### 3.5 Tile attributes (`Sim_FlyMission`, `td3:0C5D:ABFC..AD03`)

Written into the 71-byte table at every flight start, after the keyboard handler is installed and before the map file is opened. Indices are hexadecimal; the loops do not overlap.

| Code order (`td3:0C5D:`) | Writes |
|---|---|
| `AC06..AC23` | 00–0F := 1 |
| `AC25..AC43` | 10–16 := 2 |
| `AC45..AC63` | 17–21 := 1 |
| `AC65..AC83` | 22 := 2 |
| `AC85..ACA3` | 23–24 := 1 |
| `ACA5..ACC3` | 25–2C := 2 |
| `ACC5..ACE3` | 2D–40 := 1 |
| `ACE5..AD03` | **41–46 := 2** |

End state: solid 00–0F, 17–21, 23–24, 2D–40; open 10–16, 22, 25–2C, **41–46** (the hazard tiles are open, so ships and the player can enter them; their damage comes from the separate hazard test, [episode-deltas §4.2](../../re/episode-deltas.md#42-flight-model)). This confirms episode-deltas §4.4.

The fill writes exactly entries 00h..46h, the whole 71-byte block that the AI-unit initialisation allocates (`td3:2B72:3866`, GetMem(47h)); no entry is left from an earlier flight. Map statistics, counted over the 40,401 cells of each file with this end state (private script, numeric output only): `td3.map` uses 41 codes, highest 28h, 25.5% open (10,290 cells), no hazard cell; `td4.map` uses 69 codes, highest 46h, 34.3% open (13,843 cells), 37 hazard cells (41h..46h). Every cell of both maps has an attribute. Overlay codes 21h..24h occur in 4 cells of each map.

### 3.6 Overlay tiles (tick-order step 12, `td3:0C5D:C4E0..C83E`)

Skipped while late draw is set (`C4E0`; never in E3). For every cell of the 11 × 6 view window (columns viewX + 0..10, rows viewY + 0..5, the whole window as in E2; X and Y computed as 32-bit sums of view tile and window index), redraw the map tile over the ships when, tested in this order:

| Order | Rule | Address |
|---|---|---|
| 1 | tile = 21h, 22h, 23h or 24h | `C530`, `C569`, `C5A2`, `C5DB` |
| 2 | flight = 8 and (X 21..23 and Y 165..166, or X 108..110 and Y 113..115) | `C5E4..C6DF` |
| 3 | flight = 10 and (X 21..23 and Y 165..166, or X 41..42 and Y 57..58) | `C6E2..C7E4` |

The redraw is at `C7E6..C826`. This confirms episode-deltas §4.4. The box (21..23, 165..166) surrounds the base tile (22, 166).

### 3.7 Guidance home point (`UI_DrawGuidanceLine`, `td3:0C5D:81C5..8223`)

Computed at every call, in this order (a later rule wins):

```text
home := (172, 68)
if flight = 6:   home := (1, 110)
if flight > 6:   home := (22, 166)
if flight = 11:  home := (166, 78)
if flight = 13:  home := (41, 46)
if flight >= 14: home := (22, 166)
if flight = 18:  home := (28, 33)
```

| Flights | Home tile |
|---|---|
| 1–5 | (172, 68) |
| 6 | (1, 110) |
| 7–10, 12, 14–17, 19, 21 | (22, 166) |
| 11 | (166, 78) |
| 13 | (41, 46) |
| 18 | (28, 33) |

This confirms episode-deltas §6. No E3 home point depends on the view (E2 flights 8 and 20 did). Every reachable E3 block sets marker element 21 := 3Dh, so the dots use the home-marker colour when no live marked slot exists, as in E2.

## 4. Behaviour

### 4.1 Order of operations (`Sim_FlyMission` setup, `td3:0C5D:A9CC..BB1B`)

Steps follow [e2-mission-setup §4.1](e2-mission-setup.md); steps marked **E3** differ from E2, the rest is E2 behaviour at E3 addresses.

```text
 1. clear sound priority DS:1E2E and the byte after it;
    E3: died := 0 (DS:11ED) where E2 cleared its flight-18 debris counter; comm incoming := 0   (A9D0..A9DF)
    stop the music if WORX is installed (the song buffer is not freed)                         (A9E4..A9EB)
    AI_ResetShipRecords -> AI_ClearShipRecords (2B72:0000, AI unit identical to E1/E2):
        20 draws of Random(2), as E1                                                           (A9F0)
    kills, Ctrl-Break check, aborted := 0; flight number := argument; heap mark; joystick state,
    succeeded, lost, cutscene request, event counter, base-hit list 1..20, ion timer, DS:1758,
    late draw, over, end ticks := 0                                                            (A9F5..AA6C)
 2. E3: map set := 1, unconditionally; no radar copy                                           (AA71)
 3. fire requests := 0; allocate buffers; DS:17EE := 1; ticker := 0; edge hit := 0;
    unchained mode; key-down table 0..600 := 0; save and install the INT 09h handler           (AA76..ABF7)
 4. E3: tile attributes (3.5)                                                                  (ABFC..AD03)
 5. E3: map file name td3.map, or td4.map if flight > 6                                        (AD05..AD2C)
    load td.pal (flight palette DS:1300, 768 bytes), td.sc3, td.lt2, td.sc2, td.fac;
    td.fac pixels 26h -> 27h (4 frames of 28 x 28, as E2); td.shp and td.exp (sprite bank),
    td.win, td.wea, td.shr                                                                     (AD31..B099)
    flash frames, mission ticks (32-bit), palette mode := 0; draw page := 0; Crt_NoSound;
    flush the BIOS keyboard buffer (tail := head)                                              (B09E..B0CD)
    E3: no hit flash := 0 and no "all 256 DAC entries black" here (E2 B06D..B092; U-286)
 6. E3: no extra sprites in any flight (E2 loaded td.hel/td.h2 for flight 8)
 7. mission file m<n>.td3 and the ship records exactly as E2: header A (DS:3976), B (DS:39B2),
    classes (DS:399E), D (DS:398A), 20 bytes each; path X and Y, 10,000 bytes each;
    slot 2's FFh path bytes (steps 1..500) := 0 (no E3 mission file has any);
    shields := k_ClassShields[file class], armour := 2 x k_ClassArmour[file class];
    path copy into the records; waypoint and return point := path step 1;
    DS:3961 + s := 1; tile := path step 1; headings and target headings 6; sub-tile offsets 16;
    speeds 0                                                                                   (B0D0..B44F)
 8. player heading and target heading := 8; read the 201 x 201 map into g_Map; open td3.blk;
    player speeds 1..4 := 0; draw the tile set                                                 (B451..B591)
 9. E3 start view, final (a later rule wins)                                                   (B596..B5FB):
        view tile := (167, 66)
        if flight > 6:              view tile := (17, 164)
        if flight = 21:             view tile := (160, 75)
        if flight = 14, 17 or 19:   view tile := (88, 76)
        if flight = 7:              view tile := (185, 32)
    burst frame := 0; comm indicator phase := 0; view offsets (32, 16); draw page 1           (B5FC..B612)
    E3: key-down table 0..600 := 0 again                                                       (B617..B631)
    DS:1BEC..1BEF := 0; projectile slots 0..50 free (tile X := FFh); flip page                 (B633..B669)
10. Msn_Setup (4.2)                                                                            (B66E)
11. HUD frame (320 x 32 from the td.win buffer); weapon and its icon, missiles (3.3)           (B671..B7ED)
    split line; sound timers := 0; Snd_InstallTick                                             (B7F2..B84E)
    E3: no palette-mode store (E2 set 1 for flights 3 and 10 here)
    music (4.4)                                                                                (B853..B8E3)
    explosions 1..30 idle (frame 8)                                                            (B8E4..B8FE)
    player armour := k_ClassArmour[player class]; if flight = 7: 5;
    player shields := k_ClassShields[player class]                                             (B900..B925)
12. HUD lamp entries (8 x Pal_StoreEntry black, Pal_SetEntry(90h) black), missile lamp 26h and
    missile bar; fade in                                                                       (B928..BA91)
    E3: no infra-red palette after the fade (E2 BB0C..BBE4); no separate hit flash := 0
    (the fade clears it at entry, episode-deltas §6)
13. E3: no start-view overrides (E2 BBE9..BC36)
14. draw the 11 x 6 window; set the CRTC line compare; sample the DOS time                     (BA94..BB1B)
15. the tick loop starts (BB20). There is no "AI_ChooseWaypoint(s) for s = 1..20" (as E2)
```

Notes:

- Neither the setup part of `Sim_FlyMission` nor `Msn_Setup` nor `Msn_LoadTargetInfo` calls `Random` or `AI_ChooseWaypoint` (complete call list of `A9CC..BB1B`: only file, heap, string, graphics, palette, sound and DOS routines, `AI_ResetShipRecords` and `Msn_Setup`; `Msn_Setup` calls only `Msn_LoadTargetInfo` and `Txt_FlightDrawStringOpaque`; `Msn_LoadTargetInfo` calls only the stack check and the string store). The only RNG use before the first tick is the 20 draws of step 1.
- **`AI_UpdateShips` is E2's.** `td3:2935:0011..23C2` and `td2:2D26:0011..23C2` are both 9,138 bytes and end in the same `RETF 2`. Of the 971 differing bytes, all belong to 82 far-call segment words (E3 2B72h, 2EFAh, 311Ch for E2 2F63h, 32EBh, 350Dh) or to 807 DGROUP operand words that are exactly 4 lower in E3 (E2 operands 387Fh..968Fh); nothing else differs. The first waypoint choice therefore happens inside `AI_UpdateShips` when a ship first reaches its waypoint, and E2's resolution of U-205 ([e2-mission-ships §4.3](e2-mission-ships.md)) applies to E3 unchanged.
- The flight-17 class-table restore (§4.3) happens in step 10, before step 11 reads the class tables for the player.
- **What is in RAM at the start of tick 1:** every value in §3.2–§3.4 and §6 is written before the tick top `BB20`, and nothing between `Msn_Setup` and `BB20` writes slot classes, markers, record fields, phase, ticker, view or the F5 tables. The ship-record field +000h is written only by the `AI_UpdateShips` prologue in step 10 of tick 1.

### 4.2 `Msn_Setup` (`td3:0C5D:6149..6DFA`)

```text
Msn_LoadTargetInfo(flight)                         (4.5; 6149..6154)
for n in 1..11, 21, 12..20 (dispatch order):
    if flight = n: run block n                     (independent tests; no block writes the flight
                                                    number, so exactly one runs for 1..19, 21)
return                                             (6DF9..6DFA)
```

Block address ranges: 1 `6155..6248`, 2 `6249..62E8`, 3 `62E9..6388`, 4 `6389..6409`, 5 `640A..64BB`, 6 `64BC..653C`, 7 `653D..65B8`, 8 `65B9..666A`, 9 `666B..671C`, 10 `671D..67B5`, 11 `67B6..681A`, 21 `681B..687F`, 12 `6880..6A11`, 13 `6A12..6A76`, 14 `6A77..6AF7`, 15 `6AF8..6B67`, 16 `6B68..6BE8`, 17 `6BE9..6CA5`, 18 `6CA6..6D26`, 19 `6D27..6D96`, 20 `6D97..6DF8`.

Store order of each block ("mk" = marker, "cl" = class, "agg" = aggression, "obj" = objective draw followed by ticker := 1):

| Block | Stores in code order |
|---:|---|
| 1 | mk 1–20 := 0; mk 2–4, 8–9, 11–14 := 32h; mk21 := 3Dh; player class := 2; cl 2–4 := FFh; cl 11–14 := FFh; phase := 1; agg 2–14 := 200; obj |
| 2 | mk 1–20 := 0; mk 2–7 := 32h; mk21; player class := 2; agg 9–18 := 255; cl 15–17 := 7; phase := 1; obj |
| 3 | mk 1–20 := 0; mk 2–4, 9–12 := 32h; mk21; player class := 2; agg 6–14 := 255; phase := 1; obj |
| 4 | mk 1–20 := 0; mk 5–7 := 32h; mk21; agg 2–15 := 255; player class := 4; phase := 1; obj |
| 5 | mk 1–20 := 0; mk 2 := 39h; mk 5–7 := 32h; mk21; player class := 2; cl 3 := 2; armour 2 := 900; agg 3–12 := 250; agg 5–12 := 150; phase := 1; obj |
| 6 | mk 1–20 := 0; mk 2–10 := 39h; mk21; agg 2–17 := 200; phase := 1; obj; player class := 2 |
| 7 | mk 1–20 := 0; mk 2–10 := 39h; mk21; agg 12–18 := 75; obj; player class := 2 (**no phase**) |
| 8 | mk 1–20 := 0; mk 1–3 := 39h; mk21; armour 1, 2, 3 := 100; obj; player class := 6; agg 4–10 := 255; cl 11–20 := FFh; phase := 1 |
| 9 | mk 1–20 := 0; mk 1–3 := 32h; mk21; armour 1, 2, 3 := 100; obj; player class := 6; agg 4–9 := 255; cl 10–20 := FFh; phase := 1 |
| 10 | mk 1–20 := 0; mk 5 := 39h; mk 7–10 := 32h; mk21; agg 3–13 := 255; armour 5 := 100; phase := 1; obj; player class := 6; cl 5 := FFh; cl 4 := FFh |
| 11 | mk 1–20 := 0; mk21; agg 3–15 := 255; phase := 1; obj; player class := 6 |
| 21 | as 11 with objective [11] |
| 12 | mk 1–20 := 0; mk 1–6 := 32h; mk21; 27 map cells := 26h (below); agg 8–15 := 200; player class := 2; obj; phase := 1 |
| 13 | mk 1–20 := 0; mk21; agg 3–8 := 200; obj; player class := 7; phase := 1 |
| 14 | mk 1–20 := 0; mk21; obj; player class := 3; phase := 1; agg 3–17 := 255; cl 6–11 := FFh |
| 15 | mk 1–20 := 0; mk 15 := 32h; mk21; player class := 2; obj; phase := 1; agg 4–15 := 255; shields 15 := 1750 |
| 16 | mk 1–20 := 0; mk 2–4 := 32h; mk21; obj; phase := 1; agg 6–14 := 255; player class := 2 |
| 17 | class tables (4.3); mk 1–20 := 0; mk21; obj; player class := 3; phase := 100; agg 3–7 := 255 |
| 18 | **mk 1–20 := 32h**; mk 3–6 := 32h; mk21; phase := 1; player class := 3; obj; agg 3–9 := 255 |
| 19 | mk 1–20 := 0; mk 2 := 32h; mk21; obj; phase := 1; player class := 7; agg 2 := 255; armour 2 := 500 |
| 20 | phase := 1; obj; player class := 5; agg 2–11 := 10; agg 12–15 := 220 (dead) |

Flight 12's map cells, in code order, each := 26h (open, not a hazard), with the `td4.map` value they replace: (40, 121..123), (40, 131..133), (66, 128..130), (66, 121..123) were 44h, 45h, 46h (per column, top to bottom); (53..55, 137), (62..64, 114), (53..55, 114), (53..55, 118), (46..48, 118) were 41h, 42h, 43h (left to right). The other ten hazard cells of `td4.map`, (46..48, 109), (74, 77..79) and (91, 87..90), are not touched. `Msn_E3TimedEvents` writes the original values back at tick 6480 ([e3-mission-logic](e3-mission-logic.md) §4.6.3).

Differences from E2's structure: the dispatch puts 21 between 11 and 12 and keeps a dead block 20; block 7 has no phase store; block 18 has a 32h fill instead of the marker clear; block 20 has no marker writes; block 17 writes the class tables before its marker clear; E3 has no comm, palette-mode or late-draw stores and no class-table patch. As in E2, `Msn_LoadTargetInfo` is called first and every block repeats the marker writes, element 21 and the objective.

### 4.3 Class tables: flight 17 restores what flight 16 patches (OB-028)

`Msn_E3TimedEvents` (`td3:0C5D:9907..9946`) patches the typed constants in flight 16 at phase 3: max speed of classes 0..8 := 0, accel of classes 0..8 := 0, then max speed of class 2 := 20 and accel of class 2 := 4, then phase := 5. Class 9 is not written.

`Msn_Setup` block 17 (`td3:0C5D:6BF3..6C43`), first thing in the block:

```text
max speed[0] := 24
max speed[1..8] := 28
max speed[2] := 32;  max speed[6] := 24;  max speed[8] := 32
accel[0..8] := 4
accel[0] := 2;  accel[6] := 2
```

End state: max speed 24, 28, 32, 28, 28, 28, 24, 28, 32 and accel 2, 4, 4, 4, 4, 4, 2, 4, 4 for classes 0..8, which is exactly the exe image (§2); class 9 keeps its image value (0/0) because neither site writes it. These two sites are the only stores to the max-speed and accel tables in the flight unit (`0C5D`); armour and shield tables are never written in E3. So, in a program run, the patch is in effect from flight 16's phase 3 until the next flight-17 setup: a retry of flight 16, and any flight other than 17 started after a save is loaded or a new game is started in the same run, runs with the patched tables ([e3-mission-logic](e3-mission-logic.md) §6, U-275). Port policy follows OB-028 (preserve in Classic; the save-load path is the user's decision).

### 4.4 Flight song (`td3:0C5D:B853..B8E3`)

```text
r := flight mod 3
if r = 1: Snd_PlayMusic(1)      (td01)
if r = 2: Snd_PlayMusic(44)     (td03; E2 used cue 45, which is intro.cmf in E3)
if r = 0: Snd_PlayMusic(2)      (td02)
if the song buffer pointer DS:11F2 is still nil:
    if r = 2: Snd_PlayMusic(2)
    if r = 1: Snd_PlayMusic(44)
    if r = 0: Snd_PlayMusic(1)
```

Flights 1, 4, 7, 10, 13, 16, 19 → `td01`; 2, 5, 8, 11, 14, 17 → `td03`; 3, 6, 9, 12, 15, 18, 21 → `td02`. The fallback is E2's rule with cue 44 ([music-cues §4.1](../audio/music-cues.md), [episode-deltas §7](../../re/episode-deltas.md#7-audio)).

### 4.5 `Msn_LoadTargetInfo(flight)` (`td3:1BD8:0DCF..4121`)

```text
prologue, stack check                                         (0DCF..0DDC)
for e = 1..30: flag[e] := 0                                   (0DDD..0DF4)
for n in 1..11, 21, 12..20 (dispatch order):
    if flight = n:
        fixed points: flag, X, Y of entry 21 (flights 1-8, 10-17, 19, 21: flag 1;
            flight 18: flag 0; flight 9: X and Y only; flight 20: nothing),
            and of entry 22 (flights 8, 10, 11)
        for each (entry e, line l, k) of block n, in code order:
            line[e][l] := first 30 characters of e3.target_info[k]
return (RETF 2)                                               (411C..4121)
```

The dispatch test is "flight = n, else jump to the next test" at the start of each block. Flight 8's block writes entry 21's coordinates twice (the same values, `2107..210C` and `2170..2175`). Block address ranges (`1BD8:`): 1 `0DF5..0FED`, 2 `0FEE..12A0`, 3 `12A1..1566`, 4 `1567..18B8`, 5 `18B9..1B6B`, 6 `1B6C..1E2F`, 7 `1E30..20F3`, 8 `20F4..23DE`, 9 `23DF..2613`, 10 `2614..294C`, 11 `294D..2B87`, 21 `2B88..2D54`, 12 `2D55..2F68`, 13 `2F69..3129`, 14 `312A..32CA`, 15 `32CB..353D`, 16 `353E..380F`, 17 `3810..3984`, 18 `3985..3B25`, 19 `3B26..3CF9`, 20 `3CFA..411B`.

Ghidra misdecodes the start of the routine (it treats bytes after the stack-check call as code from the wrong offset), so `0DDA..0E33` and `1043..10CA` in the exported listing are not the real instructions; the table above was taken from the raw exe bytes (file offset = 14544 + 1BD8h × 16 + offset). The routine is longer than E2's (`1E0B:0DCC..429C`) and its pool has 114 strings instead of 113. Episode-deltas §10 gives `1BD8:12A1` as the start; that is block 3's test (correction, §8.1).

## 5. Edge cases and original bugs

| Quirk | Original | Port policy |
|---|---|---|
| **OB-053**: flight 18's marker loop stores 32h where every other block stores 0 | Block 18 (`td3:0C5D:6CAD..6CC7`) writes 32h to the markers of slots 1..20, then 32h again to slots 3..6. Every live ship of flight 18 (slots 3..9) starts marked: the brackets are drawn around all seven, the compass lamps count them and the guidance line goes to the lowest live marked slot (3) instead of slots 3..6 only. Empty slots 1, 2, 10..20 are marked too (no effect while they are FFh). Mission rule A clears markers 1..20 at tick 2161 ([e3-mission-logic](e3-mission-logic.md) §5.18). The second loop shows that slots 3..6 were meant; the first loop is the marker clear with the wrong value | preserve in Classic (HUD targets only: no flight-18 mission rule reads markers, [e3-mission-logic](e3-mission-logic.md) §5.18) |
| **OB-054**: flight 9's F5 block writes no flag for entry 21 | `Msn_LoadTargetInfo` block 9 (stores at `td3:1BD8:23E8` and `23ED`) stores the base point (22, 166) and five lines for entry 21, but not its flag, so the flag keeps the 0 of the clear loop and the F5 cursor never offers the base in flight 9. Flights 1–8, 10–17, 19 and 21 store 1; flight 18 stores 0 on purpose (set by the tick-2160 event) | preserve (presentation only; no simulation effect) |
| OB-043: flight 7 has no phase store | §3.2; the phase is whatever flight 6 or the previous attempt left (U-270) | as OB-043 |
| OB-028 (rev 2 correction): flight-16 class patch, restored by flight 17 | §4.3 | as OB-028 |
| OB-048: Kreel's 1750 shields | §3.2 flight 15; the first hit sets shields > 60 to 0 ([weapons](weapons.md) §4.3) | as OB-048 |
| Flight 5 retypes slot 3, not slot 2 | The class store is to DS:39A0 = slot 3 (file class 4 → 2); slot 2 (file class 0) keeps class 0 and gets marker 39h and armour 900. The F5 lines agree (entry 3's line 0 is k = 3, which the class-2 slots of flights 6, 7, 10 and the class-2 player of every class-2 flight but 12 have; entry 2's is k = 18, which the class-0 slots of flights 3–7, 13 and 19 have), so this is LIKELY intended. The flight-5 lost rule tests slot 2 ([e3-mission-logic](e3-mission-logic.md) §4.8, §5.5) | as read |
| Retyped slots keep file-class armour and shields | As E1/E2: flight 2 slots 16, 17 (6 → 7) start with armour 62 and shields 31, above the class-7 cap 25; flight 5 slot 3 (4 → 2) with armour 50 and shields 25, above the class-2 cap 18. Regeneration never lowers shields, so they keep the higher value until hit. Hidden slots keep their file-class values for the moment mission logic gives them a class: flight 1 slots 2–4 (file 0: armour 62, shields 0) and 11–14 (file 7) become class 7; flight 10 slot 4 (file 2) becomes class 2 at tick 10; flight 14 slots 6–11 get their file classes back at tick 25 | preserve |
| Writes to empty slots | Markers, armour, aggression and F5 lines are written for slots that are FFh after setup: markers flight 1 slots 2–4, 11–14; 6: 7, 8; 7: 4; 10: 5; 18: 1, 2, 10–20. Armour flight 10 slot 5. Aggression flight 1 slots 2–4, 7, 10–14; 2: 13, 14, 18; 3: 8, 13, 14; 4: 4, 8, 14, 15; 5: 4, 8, 12; 6: 7, 8, 11, 12; 7: 14, 17; 10: 4–6; 14: 6–11; 15: 7–9; 16: 8. F5 lines flight 1 entries 2–4, 7, 10–14; 5: 8; 6: 7, 8; 7: 4, 14, 17, 19, 20; 8: 15, 19; 10: 4, 5; 13: 9, 10; 14: 6–11; 15: 7–9. Mission logic gives some of these slots a class later (parts A and B); the values are then in effect | preserve |
| Flight 18 header byte | `m18.td3` header A of slot 11 is 76 (all other header bytes A, B, D of all 20 files are 0); never read (U-038) | ignore |
| Flight 20 | Dead block in both routines (never reached: no campaign block flies flight 20). If it ran: no marker writes (markers keep the previous flight's values, element 21 too), phase 1, player class 5 (4 missiles), F5 lines without any fixed point | not needed |
| Flight outside 1..21 | No block runs (unreachable) | not needed |
| Screen during loading | E3 does not black the DAC or clear the hit flash before the loads (E2 did); the fade-in still starts the visible flight (U-286) | presentation (T2) |

Side observation for the E2 owner (not E3 behaviour): E2 allocates the same 47h-byte tile-attribute block (`td2:2F63:3866`) but its fill writes up to index 51h (set 1: 47h–51h; set 2: up to 4Bh), 11 bytes past the block. [e2-mission-setup §3.5](e2-mission-setup.md) treats 47h–51h as table entries.

## 6. Test vectors

Computed from the shipped mission files with the rules above. Vectors 1 and 2 are **confirmed** against RAM at the last setup frame and at ticks 1–2 of C-0036 (flight 1, frames 693–699) and C-0041 (flight 8, frames 1069–1075): slot classes, tiles, armour, shields, aggression, markers (element 21 = 3Dh), player class, weapon, missiles, cooldown, armour and shields, view and offsets, phase, ticker, palette mode, late draw, comm, map set, class tables, radar table and the F5 flags and fixed points (entries 21 and 22; line lengths only, not the text). Vectors 3–14 remain static (U-285). Lists are per slot 1..20; "× n" repeats. Tiles are (X, Y) = path step 1 (file offsets 80 + (s − 1) × 501 and 10,080 + (s − 1) × 501). "Start of tick 1" means the state at the tick top `0C5D:BB20` of the first tick (§4.1, last note).

| # | Flight | Expected after setup (start of tick 1) | Evidence |
|---|---|---|---|
| 1 | 1 (`m1.td3`, `td3.map`) | classes FFh × 4, 7, 7, FFh, 8, 8, FFh × 11 (file: FFh, 0 × 3, 7, 7, FFh, 8, 8, FFh, 7 × 4, FFh × 6); tiles (0, 0), (85, 197), (85, 195), (85, 193), (85, 160), (85, 162), (0, 0), (80, 1), (78, 1), (0, 0), (1, 111), (3, 111), (5, 111), (7, 111), (0, 0) × 6; armour 68, 62 × 3, 50 × 2, 68, 40 × 2, 68, 50 × 4, 68 × 6; shields 34, 0 × 3, 25 × 2, 34, 25 × 2, 34, 25 × 4, 34 × 6; aggression 0, 200 × 13, 0 × 6; markers 0, 32h × 3, 0 × 3, 32h × 2, 0, 32h × 4, 0 × 6, element 21 = 3Dh; player class 2, weapon 1, missiles 6, cooldown 0, armour 15, shields 18; view (167, 66, 32, 16); late draw 0; phase 1; ticker 1; palette mode 0; comm incoming 0; map set 1; map `td3.map` unchanged; radar table unchanged; mission ticks 0; song cue 1; F5: flags 1..30 = 0 except 21 = 1, entry 21 at (172, 68), lines k = 0, 1, 2, 2, 2; home (172, 68) | **C-0036** (all listed fields equal) |
| 2 | 8 (`m8.td3`, `td4.map`) | classes 0 × 3, 7, 8, 8, 7, 8 × 3, FFh × 10 (as the file); tiles (22, 166), (22, 168), (24, 168), (102, 117), (50, 150), (102, 169), (103, 132), (48, 100), (95, 96), (99, 61), (0, 0) × 10; armour 100 × 3, 50, 40, 40, 50, 40 × 3, 68 × 10; shields 0 × 3, 25 × 7, 34 × 10; aggression 0 × 3, 255 × 7, 0 × 10; markers 39h × 3, 0 × 17, element 21 = 3Dh; player class 6, weapon 3, missiles 6, armour 31, shields 31; view (17, 164, 32, 16); late draw 0; phase 1; ticker 1; palette mode 0; map set 1; map `td4.map` unchanged (37 hazard cells); radar table unchanged; song cue 44; F5: flags 1..30 = 0 except 21 = 1 and 22 = 1, entry 21 at (22, 166), lines 41, 42, 2, 2, 2; entry 22 at (108, 115), lines 44..48; home (22, 166) | **C-0041** (all listed fields equal) |
| 3 | 9 (`m9.td3`, `td4.map`) | classes 0 × 3, 8, 8, 7, 7, 8, 8, FFh × 11; tiles (5, 94), (3, 94), (1, 94), (7, 96), (5, 96), (11, 95), (13, 95), (15, 95), (17, 95), (0, 0) × 11; armour 100 × 3, 40, 40, 50, 50, 40, 40, 68 × 11; shields 0 × 3, 25 × 6, 34 × 11; aggression 0 × 3, 255 × 6, 0 × 11; markers 32h × 3, 0 × 17, element 21 = 3Dh; player class 6, weapon 3, missiles 6, armour 31, shields 31; view (17, 164); phase 1; song cue 2; F5: **all flags 1..30 = 0**, entry 21 coordinates (22, 166), lines 41, 42, 2, 2, 2 (OB-054) | static |
| 4 | 12 (`m12.td3`, `td4.map`) | classes 0 × 6, FFh, 8 × 8, FFh × 5; tiles (102, 198), (102, 196), (102, 194), (102, 192), (102, 190), (102, 188), (0, 0), (101, 189), (103, 189), (101, 191), (103, 191), (101, 193), (103, 193), (101, 195), (103, 195), (0, 0) × 5; armour 62 × 6, 68, 40 × 8, 68 × 5; shields 0 × 6, 34, 25 × 8, 34 × 5; aggression 0 × 7, 200 × 8, 0 × 5; markers 32h × 6, 0 × 14, element 21 = 3Dh; player class 2, weapon 1, **missiles 10**, armour 15, shields 18; view (17, 164); phase 1; song cue 2; map cells of §4.2 = 26h (for example (40, 122) = 26h, file 45h; (54, 137) = 26h, file 42h); the ten other hazard cells unchanged; F5 entry 21 flag 1 at (22, 166) | static |
| 5 | 5 (`m5.td3`) | classes FFh, 0, **2**, FFh, 7 × 3, FFh, 7, 6, 6, FFh × 9; armour 68, 900, 50, 68, 50 × 3, 68, 50, 62, 62, 68 × 9; shields 34, 0, 25, 34, 25 × 3, 34, 25, 31, 31, 34 × 9; aggression 0, 0, 250, 250, 150 × 8, 0 × 8; markers 0, 39h, 0, 0, 32h × 3, 0 × 13; tiles (0, 0), (172, 70), (166, 66), …; player class 2; view (167, 66) | static |
| 6 | 7 (`m7.td3`) | player class 2, weapon 1, missiles 6, **armour 5**, shields 18; **phase = its value at entry** (100 after a successful flight 6; OB-043, U-270); view (185, 32); map `td4.map`; classes FFh, 4, 4, FFh, 4, 2, 2, 7, 0, 0, FFh, 8, 8, FFh, 6, 6, FFh, 6, FFh, FFh; aggression 0 × 11, 75 × 7, 0 × 2; markers 0, 39h × 9, 0 × 10; song cue 1; F5 entry 21 flag 1 at (22, 166) | static |
| 7 | 17 (`m17.td3`), after a flight 16 that reached phase 5 | max speed 24, 28, 32, 28, 28, 28, 24, 28, 32, 0 and accel 2, 4, 4, 4, 4, 4, 2, 4, 4, 0 (the image values); phase 100; player class 3, weapon 1, missiles 20, armour 20, shields 20; view (88, 76); classes FFh, FFh, 7 × 5, FFh × 13; aggression 0, 0, 255 × 5, 0 × 13; markers 0 × 20, element 21 = 3Dh | static |
| 8 | 18 (`m18.td3`) | markers **32h × 20**, element 21 = 3Dh; classes FFh, FFh, 7 × 3, 8 × 4, FFh × 11; aggression 0, 0, 255 × 7, 0 × 11; player class 3; phase 1; F5 entry 21 flag 0 at (28, 33), lines 102, 103, 2, 104, 105; home (28, 33) | static |
| 9 | 15 (`m15.td3`) | slot 15: class 6, armour 62, **shields 1750**, marker 32h, aggression 255; player class 2 | static |
| 10 | tile attributes (every flight) | 00h..0Fh → 1, 10h..16h → 2, 17h..21h → 1, 22h → 2, 23h, 24h → 1, 25h..2Ch → 2, 2Dh..40h → 1, 41h..46h → 2; open share 25.5% (`td3.map`) and 34.3% (`td4.map`) | static + map files |
| 11 | start view | flights 1–6 → (167, 66); 7 → (185, 32); 8–13, 15, 16, 18 → (17, 164); 14, 17, 19 → (88, 76); 21 → (160, 75); offsets (32, 16) | static |
| 12 | guidance home | flight 3 → (172, 68); 6 → (1, 110); 11 → (166, 78); 13 → (41, 46); 14 → (22, 166); 18 → (28, 33); 21 → (22, 166) | static |
| 13 | song | flights 1, 4, … 19 → cue 1; 2, 5, … 17 → cue 44; 3, 6, … 18, 21 → cue 2; if still nil: mod 3 = 2 → 2, 1 → 44, 0 → 1 | static |
| 14 | F5, flight 10 | flags 1..30 = 0 except 21 and 22 = 1; entry 21 at (22, 166), lines 41, 42, 2, 2, 2; entry 22 at (41, 58), lines 64..68; entry 5 lines 18, 4, 70, 71, 72 | static |

## 7. Presentation and audio hooks

- Objective: drawn into the ticker strip by every block; the ticker scrolls it from tick 1 ([hud](../render/hud.md) §4.7). Ink 37 or 42 per §3.2; text `e3.objectives[k]` (§3.2).
- Song: §4.4.
- Infra-red: never at the start of an E3 flight (§3.3).
- Comm: no E3 flight starts with a transmission waiting.
- F5 lines and fixed points: §3.4 (text from the exe, ADR-0003). Entry 22 is a second selectable point in flights 8, 10, 11; flight 9 offers no fixed point; flight 18's point appears at tick 2160.
- Loading screen: E3 skips E2's DAC blackout before the loads (§4.1 step 5); what the player sees before the fade-in is open (U-286).

## 8. Open questions

- [ ] **U-284** (new): F5 presentation of the E3 fixed points. Confirm with a capture that entry 22 is selectable and drawn in flights 8 (108, 115), 10 (41, 58) and 11 (165, 77), that flight 9 offers no fixed point (OB-054), and what the screen shows for flight 8's entries 15 and 19 if they ever became live (only line 0 written). Owner: [flow/flight-screens](../flow/flight-screens.md). P3.
- [ ] **U-285** (new): vectors 1 (flight 1, C-0036) and 2 (flight 8, C-0041) are confirmed (fields: slot arrays DS:399E.., DS:3A52.., DS:3A66.., DS:387C..DS:3890; ship records +3F6h/+3F8h/+3FAh; player DS:197B, DS:1BDC..1BDE, DS:38B2..38B3; view DS:395A..3961; phase, ticker; DS:1E98..1EF7 and the lines of entries 0..22). Still to confirm: vectors 3–9 with prepared saves ([e3-program-flow](../flow/e3-program-flow.md) §8). P2.
- [ ] **U-286** (new): E3 removed E2's "hit flash := 0; DAC entries 0..255 := black" before the file loads (`td2:0E96:B06D..B092`). Is anything visible between the previous screen and the flight fade-in (mode set, HUD and window drawing in the BIOS default palette)? Capture of a flight start. P3 (presentation).
- [ ] U-270 ([e3-mission-logic](e3-mission-logic.md) §5.7): flight 7's phase at the first tick.
- [ ] Resource map: `e3.target_info` (pstring_seq, file offset 1F650h, count 114) is needed in `td3.toml`; it replaces `e3.locations` and `e3.target_info.01`..`.07`, which cover 109 of the 114 strings (§3.4).

### 8.1 Corrections to [episode-deltas](../../re/episode-deltas.md) rev 2 (applied in rev 3)

| Where | Says | Code says |
|---|---|---|
| episode-deltas §4.3, player armour row | flight-7 store `td3:0C5D:B90E` | `B90E` is the test; the store is `B915` (as rev 2 corrected for E2) |
| episode-deltas §10 | E3 F5 routine `1BD8:12A1` | `1BD8:0DCF` (`12A1` is block 3's test); pool `1BD8:0000..0DCE`, 114 strings |
| episode-deltas §4.4, highest index | "td3.map 40, td4.map 70" | decimal: 28h and 46h (§3.5) |

## 9. Provenance

| Claim | Address | Method | Confidence |
|---|---|---|---|
| Per-flight blocks and store order (3.2, 4.2) | `td3:0C5D:6149..6DFA` | static: listing read store by store; the routine was also executed from the raw exe bytes once per flight (private 16-bit model of the few instruction forms it uses, unknown forms abort), and both store lists agree | CONFIRMED |
| Slot numbers of the class, marker and record stores | DS:399D + s, DS:387B + s, DS:4356 + (s − 1) × 429h | address arithmetic on the immediate operands | CONFIRMED |
| Objective strings | `td3:0C5D:59C8..6148`, 21 ShortStrings (walk ends at `6149`), file offset 15868h | static | CONFIRMED |
| Setup order, map choice, file loads, mission file, records, start view (4.1) | `td3:0C5D:A9CC..BB1B`; names at `A91F..A983` | static; compared with `td2:0E96:A76E..BCC8` | CONFIRMED |
| No `Random`, no `AI_ChooseWaypoint` before tick 1 | complete call list of `A9CC..BB1B`, `6149..6DFA`, `1BD8:0DCF..4121` | static | CONFIRMED |
| Player weapon, missiles, armour, shields; no palette-mode store | `td3:0C5D:B6BF..B7ED`, `B900..B925`; `B0AB` | static | CONFIRMED |
| Song | `td3:0C5D:B853..B8E3` | static | CONFIRMED |
| Class-table patch and restore (4.3) | `td3:0C5D:9907..9946`, `6BF3..6C43`; all stores to DS:041C..0443 in unit `0C5D` | static (operand search) | CONFIRMED |
| `Msn_LoadTargetInfo` and its table (3.4, 4.5) | `td3:1BD8:0DCF..4121`; literal pool `1BD8:0000..0DCE` (114 strings, walk ends at `0DCF`) | static: executed from the raw exe bytes once per flight with the same private model (string store recorded as destination and pool offset); block starts found by a byte scan for the flight compares | CONFIRMED |
| Tile attributes, table size | `td3:0C5D:ABFC..AD03`; allocation `td3:2B72:3866` (47h bytes) | static (listing; raw bytes for the allocation) | CONFIRMED |
| Overlay rules | `td3:0C5D:C4E0..C83E` | static | CONFIRMED |
| Home points | `td3:0C5D:81C5..8223` | static | CONFIRMED |
| Class tables and class-FFh bytes | `td3:32C5:041C..0443`, `0525`, `052F` (file bytes) | static (exe bytes; equal to E2) | CONFIRMED |
| `AI_UpdateShips` = E2 | `td3:2935:0011..23C2` vs `td2:2D26:0011..23C2` | raw byte comparison, every difference classified (far-call segments, DGROUP −4) | CONFIRMED |
| Map statistics, flight-12 cell values | `td3.map`, `td4.map` | asset decode (counts only) | CONFIRMED |
| Test vectors | `m1`, `m5`, `m7`, `m8`, `m9`, `m12`, `m15`, `m17`, `m18.td3` | computed (static): mission files decoded and `Msn_Setup` executed on that state; vectors 1 and 2 compared with C-0036 and C-0041 | CONFIRMED (1, 2); LIKELY (others, U-285) |

## 10. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-03 | RE-10d (Archaeologist + Spec author, draft for RE-10) and RE-10 (merge, checks, captures) | Initial: E3 setup order against E2, per-flight table 1..11, 21, 12..20, block store order, player state, `Msn_LoadTargetInfo` table and `e3.target_info` resource proposal, tile attributes, overlay tiles, home points, class-table restore, song, `AI_UpdateShips` identity check, OB-053, OB-054, U-284..U-286, test vectors, corrections to episode-deltas |
