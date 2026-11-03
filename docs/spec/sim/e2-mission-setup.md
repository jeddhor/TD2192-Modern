# Episode 2 per-flight setup (`td2` `Msn_Setup`, `0E96:60F9`): delta against E1

| | |
|---|---|
| **Spec ID** | `sim/e2-mission-setup` |
| **Revision** | 1 |
| **Status** | Draft |
| **Fidelity tier** | T1 (setup state, order of writes, RNG use) · T2 for the objective text and F5 lines (presentation) |
| **Confidence** | CONFIRMED (static) for every table value: constants read from the instruction stream of `td2:0E96:60F9..6D52`, `td2:0E96:A76E..BCC8` and `td2:1E0B:0DCC..429C` (the last one decoded from the raw bytes, §9). Test vectors are computed from the shipped `m<n>.td2` files with these rules; no E2 capture exists yet (U-241) |
| **Tasks** | M10-01 (feeds M10 mission specs, mission-ships E2 first tick) |
| **Unknowns** | U-205 (resolved: [e2-mission-ships](e2-mission-ships.md)), U-240, U-241 (vectors 1 and 2 confirmed by C-0015/C-0016) |
| **Read with** | [mission-setup.md](mission-setup.md) rev 1 (E1), [mission-ships.md](mission-ships.md) §3.1/§4.1, [weapons.md](weapons.md) §3.2, [flight-screens.md](../flow/flight-screens.md) §3.4, [episode-deltas.md](../../re/episode-deltas.md) §3.2, §4.3, §4.4, §6, §10 |

## 1. Summary

E2's `Sim_FlyMission` (`td2:0E96:A76E`) prepares a flight the way E1 does ([mission-setup](mission-setup.md), [mission-ships §4.1](mission-ships.md)): it resets the ship records, loads `m<n>.td2` with the same six block reads and the same record set-up, sets the start-view defaults and then calls `Msn_Setup` (`td2:0E96:60F9`). The differences are:

- a **map set** chosen by flight number (`td.map`/`td.blk` for flights 1–8, `td2.map`/`td2.blk` for 9–20) with its own tile-attribute table and radar colours;
- one hand-written **block per flight 1..20** in `Msn_Setup` (table §3.2);
- a new F5 **target-information routine** `Msn_LoadTargetInfo` (`td2:1E0B:0DCC`) called first by `Msn_Setup`;
- other **start views**, infra-red flights 3 and 10, extra sprites only in flight 8, class 5 starts with 4 missiles, flight 7 starts with 5 armour, and a fallback **music** request;
- **no setup loop calling `AI_ChooseWaypoint`** for slots 1..20 (E1 `file0001:0E8B:CC63..CC7D`). The consequences for the first tick are specified elsewhere (U-205).

Everything not listed here is as in E1.

## 2. Definitions and units

E2 addresses (DGROUP `36B6`; [episode-deltas §3.2](../../re/episode-deltas.md#32-dgroup-offset-map), [td2.csv](../../re/symbols/td2.csv)). "Record field +f of slot s" is DS:435A + (s − 1) × 1065 + f, as in E1 ([mission-ships §3.1](mission-ships.md)).

| Term | E2 address | Type | Notes |
|---|---|---|---|
| Flight n | `g_FlightNumber` DS:31E1 | u8 | 1..20; flight n loads `m<n>.td2` (always) |
| Mission phase | `g_MissionPhase` DS:31E2 | u8 | := 1 by every block |
| Ticker | `g_TickerPos` DS:31E4 | u16 | := 1 by every block (objective ticker starts) |
| Marker of slot s | `g_SlotMarker` DS:387F + s | u8 | 0, 32h or 39h; element 21 (DS:3894) is the home point marker, 3Dh |
| Class of slot s | `g_SlotClass` DS:39A1 + s | u8 | FFh = no ship |
| Aggression | record +3F6h | u8 | 0 after `AI_ResetShipRecords`; fire-budget unit |
| Shields / armour | record +3F8h / +3FAh | i16 | k_ClassShields[file class] / 2 × k_ClassArmour[file class]; class FFh reads the bytes after the tables, DS:052F = 34 and DS:0525 = 34, so shields 34 and armour 68 (same values as E1) |
| Player class | `g_PlayerClass` DS:197F | u8 | |
| Player armour, shields | DS:38B6, DS:38B7 | i8 | set after `Msn_Setup` |
| Weapon, missiles | DS:1BE0, DS:1BE1 | u8 | set after `Msn_Setup` |
| Palette mode | `g_PaletteMode` DS:1622 | u8 | 1 = infra-red |
| Late draw | `g_PlayerDrawLate` DS:1621 | u8 | |
| Comm incoming | `g_CommIncoming` DS:31C4 | u8 | F6 transmission waiting |
| Map set | `g_MapSet` DS:1302 | u8 | 1 or 2 (new in E2) |
| Radar colours | `k_TileRadarColour` DS:036A, `k_TileRadarColour2` DS:03BA | u8[80] | resources `e2.hud.radar_tile_colours`, `e2.hud.radar_tile_colours_set2` |
| Tile attributes | far pointer DS:3BD2 | u8[] | 1 = solid, 2 = open |
| Class tables | max speed DS:041C, armour DS:0426, shields DS:0430, accel DS:043A | u8[10] | byte-identical to E1 ([structures §6](../../re/structures.md)) |
| View | tile X/Y DS:395E/DS:3960, offsets DS:3962/DS:3964 | i16 | |
| F5 entries | flag DS:1E9C + e, map X DS:1EBC + e, map Y DS:1EDC + e, line l of entry e at DS:1EFC + 155e + 31l | u8, u8, u8, string[30] | E1 layout ([flight-screens §3.4](../flow/flight-screens.md)) at −D8h; e = 0..31, l = 0..4 |

## 3. Data

### 3.1 Map sets

| `g_MapSet` | Flights | Map | Tiles | Radar colours | Tile attributes |
|---:|---|---|---|---|---|
| 1 | 1–8 (`g_FlightNumber` ≤ 8) | `td.map` | `td.blk` | `k_TileRadarColour` as it is | set 1 (§3.5) |
| 2 | 9–20 | `td2.map` | `td2.blk` | entries 0..79 of `k_TileRadarColour2` copied over `k_TileRadarColour`, never undone (OB-024) | set 2 (§3.5) |

### 3.2 Per-flight table (`Msn_Setup`)

Every block clears the markers of slots 1..20 to 0 before its own marker writes and sets marker element 21 := 3Dh after them. Every block also sets the mission phase := 1, draws the objective `e2.objectives[n − 1]` with `Txt_FlightDrawStringOpaque(0, 735, text, ink)` (ticker strip, VGA row 735; ink 37 for flights 1–7 and 9–11, ink 42 for flights 8 and 12–20) and sets the ticker := 1. Unlike E1 flight 20, every E2 flight has an objective.

Lists are in the order the code writes them; a later write to the same slot and field wins. Armour and aggression are record fields of the slot. The "Start view" column is the final view tile after `Sim_FlyMission`'s overrides (§4.1, step 13); view offsets are (32, 16) in every flight. "Markers" lists only non-zero values; slots not listed have marker 0.

| Flight | Player class | Start view (tile) | Late draw | Slot class overrides | Markers | Armour | Aggression | Other |
|---:|---:|---|:-:|---|---|---|---|---|
| 1 | 2 | (102, 104) | | | 6–13 := 32h; 1 := 32h | 1 := 100 | 3–13 := 150; 1 := 255 | |
| 2 | 2 | (102, 104) | | 4–8 := 3 | 4–8, 12–18 := 32h | | 1–18 := 155 | slot 8 is empty in `m2.td2` and becomes a class-3 ship (OB-035) |
| 3 | 2 | (102, 104) | | | 3–8 := 32h; 1 := 39h | 1 := 400 | 3 := 50; 4 := 75; 3 := 25; 5, 6 := 125; 7 := 55; 8 := 25 | palette mode := 1; the player class is written before the marker clear |
| 4 | 4 | (102, 104) | | 1–3 := 0; 8–11 := 7; 14–16 := 3 | 1–3 := 32h | | 8, 9 := 210; 10 := 100; 11 := 110; 14–16 := 210 | |
| 5 | 7 | (179, 33) | | | 1–3, 5, 6 := 32h | | 5–12 := 150 | |
| 6 | 7 | (179, 33) | | 13 := 7 | | | 2–11 := 200 | the player class is written after the objective |
| 7 | 3 | (120, 70) | | | | 2 := 75 | 3–13 := 195; 2 := 250 | player armour := 2 (dead store: §4.1 step 11 sets 5) |
| 8 | 3 | (102, 100) | yes | 4 := 4; 5–11 := FFh | 1–11 := 39h; 15–20 := 32h | 2, 3 := 200; 5, 6 := 125; 8–10 := 90 | 4, 7, 19, 11, 15, 16, 17 := 115; 18 := 55 | late draw := 1; then the class tables of the player class (3): max speed := 20h, accel := 4 (OB-018); `td.hel`/`td.h2` sprites (§4.1 step 6) |
| 9 | 4 | (87, 28) | | | 5 := 32h | 5 := 500 | 2, 3 := 200; 8–15 := 200 | |
| 10 | 2 | (87, 28) | | 14–16 := FFh | 14–16 := 32h; 2 := 39h | 2 := 500; 8 := 250 | 4–7, 15 := 195; 16 := 180 | palette mode := 1 |
| 11 | 5 | (87, 28) | | 6–14 := FFh | 2, 6, 11 := 32h | | 2–14 := 200 | comm incoming := 1 (the class and aggression writes come before the marker clear) |
| 12 | 5 | (87, 28) | | | 2–4 := 39h | | 8–13 := 200 | |
| 13 | 5 | (87, 28) | | | 4–6, 8–11, 13–15 := 32h | | 4–15 := 225 | |
| 14 | 2 | (87, 28) | | | 4–8, 10–12 := 32h | | 4–16 := 195; 2 := 255 | |
| 15 | 2 | (87, 28) | | | 2 := 32h | | 4–7 := 125; 2 := 200 | |
| 16 | 2 | (87, 28) | | | 2 := 32h | 2 := 500; 10 := 200 | 5–12 := 225 | |
| 17 | 2 | (87, 28) | | 12 := 6 | 2, 3 := 39h | 2 := 5 × armour(2); 3 := 5 × armour(3) | 5, 6 := 15; 8–14 := 125 | the × 5 multiplies the armour from the file class (16-bit; 32 → 160 in `m17.td2`) |
| 18 | 2 | (87, 28) | | 9–11 := FFh | 6–11 := 32h | | none (every slot keeps 0) | |
| 19 | 2 | (87, 28) | | | 2 := 32h | 2 := 1000 | 2–7 := 255 | |
| 20 | 5 | (87, 28) | | | 12–15 := 32h | | 2–11 := 10; 12–15 := 220 | |

All flights: mission phase := 1; marker element 21 := 3Dh; no path patches, no slot-position writes, no shield writes. The only class-table patch is flight 8's.

### 3.3 Player state after `Msn_Setup`

| Item | Rule | Change from E1 |
|---|---|---|
| Primary weapon | by player class, as [weapons §3.2](weapons.md) (2, 3, 8 → 1; 4, 5, 7 → 2; 6 → 3; 0, 1 unchanged) | none |
| Missiles | class 2 → 6, 3 → 20, 4 → 4, **5 → 4**, 6 → 6, 7 → 0, 8 → 0, 0/1 → 0; missile cooldown := 0 | class 5: E1 1 |
| Player armour | k_ClassArmour[player class]; then **flight 7: 5** | flight 7 rule new |
| Player shields | k_ClassShields[player class] | none |
| Palette mode | 0 before `Msn_Setup`; := 1 again after it for flights **3 and 10** | E1: flights 4 and 8 |

Player class per flight: 1–3 → 2; 4 → 4; 5–6 → 7; 7–8 → 3; 9 → 4; 10 → 2; 11–13 → 5; 14–19 → 2; 20 → 5.

### 3.4 F5 target information (`Msn_LoadTargetInfo`, `td2:1E0B:0DCC`)

The routine has E1's structure ([flight-screens §3.4](../flow/flight-screens.md)): it clears the flags of entries 1..30, then the block for the flight writes a fixed point (entry 21) and five lines for some entries. Each line is copied from string k of the resource `e2.target_info` with a maximum length of 30 (a longer string keeps its first 30 characters: k = 2, 3, 6, 10, 11, 15–17, 20, 21, 23–28, 30, 32–34, 39, 42, 44, 51–53, 57, 63, 70, 74, 75, 84, 100, 101). k = 1 is 30 spaces (a blank line).

`e2.target_info` is the run of **113** consecutive ShortStrings that starts at file offset 22230h (`td2:1E0B:0000`, the routine's literal pool, which ends at `1E0B:0DCB`). It does not exist yet as one id in [td2.toml](../../re/exe-resources/td2.toml); until it does, k maps to the existing ids as follows (nine strings have no id):

| k | Current id |
|---|---|
| 0–3 | `e2.locations[0..3]` |
| 4 | `e2.target_info.01` |
| 6 | `e2.target_info.02` |
| 10–12 | `e2.target_info.03[0..2]` |
| 15–17 | `e2.target_info.04[0..2]` |
| 20, 21 | `e2.target_info.05[0..1]` |
| 23–112 | `e2.target_info.06[0..89]` |
| 5, 7, 8, 9, 13, 14, 18, 19, 22 | none (needs the toml change, §8) |

Final state per flight (entry: k for lines l = 0..4; where the code writes an entry twice, the table shows the result):

| Flight | Entry: lines l = 0..4 | Fixed points (flag 1) |
|---:|---|---|
| 1 | 0: 2 3 4 1 1; 1: 2 3 5 1 1; 3: 6 3 7 8 1; 4: 6 3 9 8 1; 6..8: 10 11 12 13 14; 9..13: 15 11 12 13 14; 15, 16: 16 17 12 18 19; 21: 0 1 1 1 1 | 21 at (107, 104) |
| 2 | 0: 2 3 4 1 1; 1, 2: 2 3 12 1 1; 4..8: 20 21 12 22 1; 12..18: 15 23 12 22 1; 21: 0 1 1 1 1 | 21 at (107, 104) |
| 3 | 0: 2 3 4 1 1; 1: 24 25 12 1 1; 3..5, 7, 8: 15 21 12 26 1; 6: 10 21 12 26 1; 21: 0 1 1 1 1 | 21 at (107, 104) |
| 4 | 0: 6 3 4 1 1; 1..3: 27 23 12 26 1; 8..11: 15 23 12 28 29; 14..16: 20 21 12 30 1; 21: 0 1 1 1 1 | 21 at (107, 104) |
| 5 | 0: 32 23 4 1 1; 1..3: 27 21 12 33 1; 5..9: 20 21 12 34 35; 11: 32 23 36 30 1; 21: 31 1 1 1 1 | 21 at (184, 36) |
| 6 | 0: 32 23 4 1 1; 2..11: 10 11 12 39 40; 13: 32 23 41 30 1; 15, 16: 20 21 12 42 43; 21: 37 1 1 38 1 | 21 at (125, 73) |
| 7 | 0: 2 3 4 1 1; 1: 44 11 45 46 47; 2..12: 48 49 1 50 1; 13, 14: 10 11 45 51 1; 21: 0 1 1 1 1 | 21 at (107, 104) |
| 8 | 0: 52 3 4 1 1; 2, 3, 5, 6, 8..10: 53 3 12 54 55; 4, 7, 11: 56 3 12 54 55; 15, 19: 10 11 12 51 1; 16..18: 15 11 12 51 1; 21: 0 1 1 1 1 | 21 at (107, 104) |
| 9 | 0: 57 3 4 1 1; 2, 3: 58 3 12 1 1; 5: 53 11 12 59 60; 8..10, 12..14: 15 11 12 61 62; 11, 15: 10 11 12 61 62; 21: 0 1 1 1 1 | 21 at (92, 30) |
| 10 | 0: 63 3 4 1 1; 2: 16 3 64 65 66; 3: 2 3 67 68 1; 4, 6..14: 15 11 12 1 1; 5: 10 11 12 1 1; 15, 16: 53 11 12 69 1; 21: 0 1 1 1 1 | 21 at (92, 30) |
| 11 | 0: 70 3 4 1 1; 2, 6, 11: 73 11 12 71 72; 3: 10 11 12 71 72; 4, 5, 7..10, 12: 15 11 12 71 72; 21: 0 1 1 1 1 | 21 at (92, 30) |
| 12 | 0: 70 3 4 1 1; 2..4: 74 75 12 76 77; 8..13: 15 11 12 78 79; 21: 0 1 1 1 1 | 21 at (92, 30) |
| 13 | 0: 70 3 4 1 1; 2: 70 3 80 1 1; 4..8, 10..12, 14, 15: 15 11 12 81 82; 9, 13: 10 11 12 81 82; 21: 0 1 1 1 1 | 21 at (92, 30) |
| 14 | 0: 2 3 4 1 1; 2: 2 3 83 1 1; 4..8: 15 11 12 81 82; 10..13: 84 11 12 81 82; 21: 0 1 1 1 1 | 21 at (92, 30) |
| 15 | 0: 2 3 4 1 1; 2: 70 3 91 92 1; 4, 5: 85 11 12 86 62; 6, 7: 87 11 12 86 62; 10..12: 88 17 12 89 90; 21: 0 1 1 1 1 | 21 at (92, 30) |
| 16 | 0: 2 3 4 1 1; 2: 16 3 93 94 1; 5, 6: 87 11 12 96 97; 7: 85 11 12 96 97; 8..10: 95 11 12 96 97; 21: 0 1 1 1 1 | 21 at (92, 30) |
| 17 | 0: 2 3 4 1 1; 2..4: 16 3 45 98 99; 5: 6 3 100 98 99; 6: 6 3 101 98 99; 8, 9: 87 11 45 102 103; 10, 11, 14: 85 11 45 102 103; 12, 13: 95 11 45 102 103; 21: 0 1 1 1 1 | 21 at (92, 30) |
| 18 | 0: 2 3 4 1 1; 2: 6 3 104 1 1; 3: 6 3 105 1 1; 4: 6 3 106 1 1; 6..11: 85 11 45 102 107; 21: 0 1 1 1 1 | 21 at (92, 30) |
| 19 | 0: 2 3 4 1 1; 2: 10 11 108 109 110; 3..7: 15 11 45 1 1; 21: 0 1 1 1 1 | 21 at (92, 30) |
| 20 | 0: 70 3 4 1 1; 2..5: 6 3 45 1 1; 6..8: 2 3 45 1 1; 10, 11: 70 3 45 1 1; 12..15: 15 11 45 111 112; 17, 18: 16 3 45 1 1; 19, 20: 53 3 45 1 1 | none: entry 21 is not written, so its flag stays 0 |

- Entries and lines not written keep what an earlier flight of the same program run left (zero-length strings after start-up), as in E1. Entry 31 and the coordinates of entries 0..20 are never written here; the F5 screen takes slot positions from the slot arrays when it opens.
- Every slot that is live after setup has lines in its flight (checked for all 20 mission files). Several flights also write lines for slots that are empty at setup (for example flight 10, slots 9–16; flight 11, slots 5–12): these are ships that mission logic adds later (U-240).
- The E2 fixed point 21 is not the guidance home point (§3.7) in flights 5, 6 and 9–19; both are as read.
- Unlike E1, there is no other fixed point (E1 flight 19 had entry 22) and no flight without a block.

### 3.5 Tile attributes (`Sim_FlyMission`, `td2:0E96:A9D5..ACC0`)

Written into the table at DS:3BD2 at every flight start, before the map is read. Indices are hexadecimal; "→" lists the writes in code order, a later write wins.

| Map set | Writes in code order | End state: solid (1) | End state: open (2) |
|---|---|---|---|
| 1 (`A9D5..AAEE`) | 00–0E := 1; 0F–10 := 2; 11–30 := 1; 31–33 := 2; 34–37 := 1; 38–3D := 2; 3E–44 := 1; 45 := 2; 46 := 1; 47–51 := 2 | 00–0E, 11–30, 34–37, 3E–44, 46 | 0F–10, 31–33, 38–3D, 45, 47–51 |
| 2 (`AAF0..ACC0`) | 00–0A := 1; 0B–10 := 2; 11–1B := 1; 1C–1D, 1E–20, 21–22, 23–25, 26–29 := 2; 2A–2D := 1; 2E–2F := 2; 2E–32 := 2; 30 := 1; 33–3B := 1; 3A := 2; 3C–4B := 2 | 00–0A, 11–1B, 2A–2D, 30, 33–39, 3B | 0B–10, 1C–29, 2E–2F, 31–32, 3A, 3C–4B |

This confirms [episode-deltas §4.4](../../re/episode-deltas.md#44-tile-attributes-and-overlay-tiles-u-009). Set 1 is E1's table plus 47h–51h open. The highest tile in `td.map` is 46h and in `td2.map` 44h, so every cell has a defined attribute; 66.3% of `td.map` cells and 74.8% of `td2.map` cells are open with these tables. The table is allocated once, by the `2F63` unit initialisation (`2F63:386C`), so a set-2 flight leaves entries 4Ch–51h at whatever an earlier flight wrote; no `td2.map` cell uses them.

### 3.6 Overlay tiles (tick-order step 12, `td2:0E96:C684..C9C0`)

Skipped while late draw is set (`C67A`). For every cell of the 11 × 6 view window (columns viewX + 0..10, rows viewY + 0..5; E1 scanned only columns 3..6 and rows 1..4), redraw the map tile over the ships when:

| Map set | Rule |
|---|---|
| 1 (`C684..C836`) | tile ∈ {45h, 46h, 44h, 42h}, or the cell's X is 7Ah..7Fh and its Y is 47h (X and Y computed as 32-bit sums of view tile and window index) |
| 2 (`C839..C9C0`) | 33h ≤ tile ≤ 3Bh, or tile ∈ {1Fh, 20h, 1Eh} |

This confirms episode-deltas §4.4.

### 3.7 Guidance home point (`UI_DrawGuidanceLine`, `td2:0E96:858B..85E3`)

Computed at every call, in this order (a later rule wins):

```text
home := (107, 104)
if flight = 5:  home := (189, 37)
if flight = 6:  home := (125, 68)
if flight = 8:  home := (1, low byte of (viewY + 2))
if flight > 8:  home := (92, 28)
if flight = 20: home := (low byte of (viewX + 5), 0)        (straight up from the player)
```

| Flights | Home tile |
|---|---|
| 1–4, 7 | (107, 104) |
| 5 | (189, 37) |
| 6 | (125, 68) |
| 8 | (1, viewY + 2), every tick |
| 9–19 | (92, 28) |
| 20 | (viewX + 5, 0), every tick |

This confirms episode-deltas §6 (its "21" in the 9–19 row is the `flight > 8` test; E2 never flies 21). When no live marked slot exists, the line points home and the dot colours are chosen from marker element 21 (index set to 21 at `0E96:8D3B`, colour test at `0E96:90CB`); every E2 block sets that element to 3Dh. E1's flight-19 phase rule is gone.

## 4. Behaviour

### 4.1 Order of operations (`Sim_FlyMission` setup, `td2:0E96:A76E..BCC8`)

Steps marked **E2** differ from E1; the rest is E1 behaviour at E2 addresses.

```text
 1. clear: flight-18 event counter DS:11E8 (E2), sound priority, comm incoming
    stop the music if WORX is installed           (E2: no difficulty := 1, the song buffer is not freed)
    AI_ResetShipRecords                           (20 draws of Random(2), as E1)
    flight number := argument; kills, aborted, joystick, outcome flags, base-hit list, timers,
    late draw := 0
 2. E2: map set := 1 if flight <= 8 else 2                                   (A813..A821)
    E2: if map set = 2: k_TileRadarColour[0..79] := k_TileRadarColour2[0..79] (A826..A84D)
 3. clear fire requests; allocate buffers; ticker := 0; edge hit := 0; install the keyboard handler
 4. E2: tile attributes for the map set (3.5)                                (A9D5..ACC0)
 5. load td.pal, td.sc3, td.lt2, td.sc2, td.fac, td.shp, td.exp, td.win, td.wea, td.shr;
    flash frames, mission ticks, palette mode, hit flash := 0; all DAC entries black; draw page := 0
 6. E2: flight 8 only: td.hel into sprite bank +800h (800h bytes); td.h2 into +C80h of the
    three rotation buffers (C80h bytes each)                                 (B0B4..B197)
    (E1 loaded td.hel/td.h2 for flights 12, 15, 21, mine.td for 7, lander.td for 20)
 7. mission file m<n>.td2 and the ship records exactly as E1 (mission-ships 4.1 steps 2-4):
    six block reads; slot 2's FFh path bytes := 0; shields and armour from the file class;
    path copy; waypoint and return point := path[1]; tile := path[1]; headings 6; offsets 16;
    speeds 0                                                                 (B19C..B51B)
 8. player heading and target heading := 8; read the 201 x 201 map (td.map or td2.map);
    draw the tile set (td.blk or td2.blk); player speeds := 0                (B51D..B675)
 9. start-view defaults: view tile (102, 104), offsets (32, 16), draw page 1;
    projectile slots free                                                    (B67F..B6D7)
10. Msn_Setup (4.2)                                                          (B6E1)
11. HUD frame; weapon and missiles from the player class (3.3); split line;
    E2: palette mode := 1 if flight = 3 or 10                                (B85A..B872)
    sound timers; music (4.4); explosions idle;
    player armour := k_ClassArmour[player class]; E2: if flight = 7: player armour := 5
    player shields := k_ClassShields[player class]                           (B978..B99D)
12. HUD lamps and missile bar; fade in; infra-red palette if palette mode = 1; hit flash := 0
13. E2 start-view overrides, in this order (a later one wins)                (BBE9..BC36):
        if flight = 5 or 6:  view tile := (179, 33)
        if flight > 8:       view tile := (87, 28)
        if flight = 7:       view tile := (120, 70)
        if late draw:        view tile := (102, 100)
    (E1's "flights 13 and 14 -> (177, 153)" is gone)
14. draw the 11 x 6 window; set the CRTC; sample the DOS time                (BC3C..BCC3)
15. E2: the tick loop starts (BCC8). There is no "AI_ChooseWaypoint(s) for s = 1..20"
    here (E1 0E8B:CC63..CC7D sat between the time sample and the first tick)
```

Notes:

- The only `AI_ChooseWaypoint` call site in E2 is inside `AI_UpdateShips` (`td2:2D26:036C`), so each ship's first waypoint choice happens in the tick in which it first reaches its waypoint (U-205).
- Neither the setup part of `Sim_FlyMission` nor `Msn_Setup` calls `Random`. The only RNG use before the first tick is the 20 draws of `AI_ResetShipRecords` (step 1).
- The flight-8 class-table patch (§4.3) happens in step 10, before step 11 reads the class tables for the player; class 3's armour and shields are not patched.

### 4.2 `Msn_Setup(flight)` (`td2:0E96:60F9..6D52`)

```text
Msn_LoadTargetInfo(flight)                         (4.5)
for n = 1..20 in ascending order:
    if flight = n: run block n                     (3.2; the blocks are independent tests, so
                                                    exactly one runs for flights 1..20)
block n, common part (its position inside the block varies; see the table notes):
    marker[1..20] := 0
    ... the block's own writes (table 3.2, each column in code order) ...
    marker[21] := 3Dh
    phase := 1
    draw e2.objectives[n - 1] into the ticker strip (x 0, VGA row 735), ink 37 or 42
    ticker := 1
```

Block address ranges: 1 `6105..6198`, 2 `6199..6254`, 3 `6255..62ED`, 4 `62EE..63CA`, 5 `63CB..6458`, 6 `6459..64C2`, 7 `64C3..6537`, 8 `6538..6656`, 9 `6657..66E7`, 10 `66E8..679D`, 11 `679E..6835`, 12 `6836..68B6`, 13 `68B7..6972`, 14 `6973..6A17`, 15 `6A18..6A86`, 16 `6A87..6AFC`, 17 `6AFD..6BBF`, 18 `6BC0..6C3E`, 19 `6C3F..6CAE`, 20 `6CAF..6D50`.

Differences from E1's structure: E1 cleared the markers and set element 21 once before the blocks and called its message-table routine after that; E2 calls `Msn_LoadTargetInfo` first and repeats the marker clear and element 21 in every block. The end state is the same for flights 1..20. E2 has no flight-21 block, no `DS:396C := 32h` case (E1 flight 19) and no path patch (E1 flight 9).

The flight-8 class-table patch indexes the tables with the player class, which the block has just set to 3: `k_ClassMaxSpeed[3] := 20h` (was 1Ch), `k_ClassAccel[3] := 4` (already 4).

The flight-17 armour multiply reads the armour set in §4.1 step 7 (2 × k_ClassArmour[file class]) and stores 5 × that value, 16-bit.

### 4.3 Class-table patch persists (OB-018 in E2)

As in E1 ([mission-setup §3.3](mission-setup.md#33-class-table-patches-persist)), the flight-8 write to `k_ClassMaxSpeed[3]` changes a typed constant and is never undone. After flight 8 has been flown in a program run, every class-3 ship uses maximum speed 20h (16 px/tick instead of 12) for the rest of the run. At setup, E2 class 3 occurs in flights 2 (slots 4–8), 4 (slots 14–16), 5 (slots 5–9), 7 (the player) and 8 (the player); ships that mission logic adds later are not covered by this list. The campaign flies these before flight 8, so the effect needs a replay in the same run (a loaded save, a retry of an earlier mission after flight 8, or a new game). Port policy: OB-018's (preserve in Classic).

### 4.4 Flight song (`td2:0E96:B8CB..B959`)

```text
r := flight mod 3
if r = 1: Snd_PlayMusic(1)      (td01)
if r = 2: Snd_PlayMusic(45)     (td03; E1 cue 14)
if r = 0: Snd_PlayMusic(2)      (td02)
if the song buffer pointer DS:11F6 is still nil:
    if r = 2: Snd_PlayMusic(2)
    if r = 1: Snd_PlayMusic(45)
    if r = 0: Snd_PlayMusic(1)
```

Because E2 no longer frees the song at flight start, the pointer is nil here only when no song was ever loaded in this run or the last load failed; with music off both requests do nothing ([music-cues §4.1](../audio/music-cues.md)).

### 4.5 `Msn_LoadTargetInfo(flight)` (`td2:1E0B:0DCC..429C`)

```text
for e = 1..30: flag[e] := 0                                   (0DDA..0DF1)
for n = 1..20 in ascending order:
    if flight = n:
        if n <= 19: flag[21] := 1; mapX[21], mapY[21] := fixed point of n (table 3.4)
        for each (entry e, line l, k) of block n, in code order:
            line[e][l] := first 30 characters of e2.target_info[k]
```

Block address ranges (`1E0B:`): 1 `0DF2..11C1`, 2 `11C2..1474`, 3 `1475..165B`, 4 `165C..190E`, 5 `190F..1BC1`, 6 `1BC2..1E74`, 7 `1E75..2106`, 8 `2107..2379`, 9 `237A..2652`, 10 `2653..2937`, 11 `2938..2AF8`, 12 `2AF9..2D0C`, 13 `2D0D..2F06`, 14 `2F07..3179`, 15 `317A..348B`, 16 `348C..379D`, 17 `379E..3A55`, 18 `3A56..3CA2`, 19 `3CA3..3E76`, 20 `3E77..4298`. Ghidra's functions `1E0B:11C2` and `1E0B:1475` are the starts of blocks 2 and 3, not separate routines; block 1 is not in Ghidra's listing at all.

## 5. Edge cases and original bugs

| Quirk | Original | Port policy |
|---|---|---|
| OB-018 (E2 instance): flight 8's class-3 max-speed patch persists | §4.3 | preserve in Classic (session state), as OB-018 |
| OB-024: map set 2 radar colours never restored | §3.1 | as OB-024 |
| **OB-035**: flight 2's class loop 4–8 := 3 includes slot 8, which is empty in `m2.td2` (class FFh, path all zero) | Slot 8 becomes a class-3 ship at tile (0, 0) with armour 68 and shields 34 (the class-FFh table reads), marker 32h, aggression 155. In the first tick mission logic (steps 8–9) sees it as live and marked; `AI_UpdateShips` then finds waypoint = tile and calls `AI_ChooseWaypoint(8)` before the edge rule (tile X = 0) removes it ([mission-ships §4.2.3](mission-ships.md)). Never visible (far from the view) | preserve (it can change the first tick's AI calls and RNG order; RE-9 / U-205) |
| Retyped slots keep file-class armour and shields | As E1: flight 2 slots 4–7 (7 → 3) start with 25 shields, above the class-3 cap 20; regeneration never lowers shields, so they keep 25 until hit. Flight 8 slot 4 (2 → 4) starts with 30 armour and 18 shields (regenerates to 25). Flight 6 slot 13 (8 → 7) and flight 17 slot 12 (8 → 6, shields 25 → cap 31) likewise | preserve |
| Writes to empty slots | Markers, armour, aggression and F5 lines are written for slots that are FFh after setup (flight 1 slot 5; 2 slots 3, 9–11; 5 slots 10, 12; 6 slot 3; 8 slots 1, 5–11, 20; 10 slots 14–16; 11 slots 5–14; 13 slots 7, 12; 14 slots 9, 13–16; 16 slots 11, 12; 18 slots 9–11; 20 slots 8, 9, 11). Mission logic gives some of these slots a class later (for example `td2:2236:24C7`, `2236:266E`); the values are then in effect | preserve (U-240) |
| Flight 18 has no aggression write | Every ship keeps aggression 0: it never fires and an attack order ends at once ([mission-ships §3.1](mission-ships.md)). No E2 code outside `Msn_Setup` writes aggression (raw scan of units `0E96`, `2236`, `2D26`, `2F63`; the only other store is the reset to 0) | preserve |
| Flight 7 player armour := 2 in `Msn_Setup` | Dead store: §4.1 step 11 sets the class value and then 5 | ignore (no observable effect) |
| Flight 3 aggression of slot 3 written twice | 50, then 25; 25 holds | as the table |
| Marker element 21 | Every block sets 3Dh; no E2 code changes it later (no absolute-address writer outside `Msn_Setup`) | as the table |
| Flight 20 F5 | No fixed point; entry 21 keeps flight 19's lines but its flag is 0, so it is not offered | preserve |
| Flight outside 1..20 | No block runs; nothing in 3.2 is written (unreachable in E2) | not needed |

## 6. Test vectors

Computed from the shipped mission files with the rules above. Vectors 1 and 2 are **confirmed** against the last setup frame and tick 1 of captures C-0015 (flight 1) and C-0016 (flight 9): slot classes, tiles, armour, aggression, markers (element 21 = 3Dh), player class, armour and shields, view offsets, map set, radar table (unchanged / set-2 copy) and the view after the start-view override (tick 1); shields and the F5 tables were not compared. The others remain static (U-241). Lists are per slot 1..20; "× n" repeats. Tiles are (X, Y) = path step 1.

| # | Flight | Expected after setup (start of tick 1) | Evidence |
|---|---|---|---|
| 1 | 1 (`m1.td2`, map set 1) | classes 2, FFh, 4, 4, FFh, 8, 8, 8, 7 × 5, FFh, 1, 1, FFh × 4; tiles (20, 89), (0, 0), (21, 159), (163, 155), (0, 0), (145, 144), (40, 152), (72, 93), (109, 157), (127, 137), (23, 129), (91, 47), (30, 98), (0, 0), (115, 50), (138, 94), (0, 0) × 4; armour 100, 68, 50, 50, 68, 40, 40, 40, 50 × 5, 68, 32, 32, 68 × 4; shields 18, 34, 25, 25, 34, 25 × 8, 34, 0, 0, 34 × 4; aggression 255, 0, 150 × 11, 0 × 7; markers 32h, 0 × 4, 32h × 8, 0 × 7, element 21 = 3Dh; player class 2, weapon 1, missiles 6, armour 15, shields 18; view (102, 104, 32, 16); late draw 0; phase 1; ticker 1; palette mode 0; map set 1; radar table unchanged; song cue 1; F5 entry 21 flag 1 at (107, 104) | static (m1.td2 + §3) |
| 2 | 9 (`m9.td2`, map set 2) | classes FFh, 4, 4, FFh, 0, FFh, FFh, 7, 7, 7, 8, 7, 7, 7, 8, FFh × 5; tiles (0, 0), (103, 37), (94, 50), (0, 0), (1, 30), (0, 0), (0, 0), (52, 60), (25, 73), (74, 77), (110, 101), (127, 54), (148, 150), (151, 75), (75, 153), (0, 0) × 5; armour 68, 50, 50, 68, 500, 68, 68, 50, 50, 50, 40, 50, 50, 50, 40, 68 × 5; shields 34, 25, 25, 34, 0, 34, 34, 25 × 8, 34 × 5; aggression 0, 200, 200, 0 × 4, 200 × 8, 0 × 5; markers 0 × 4, 32h, 0 × 15, element 21 = 3Dh; player class 4, weapon 2, missiles 4, armour 25, shields 25; view (87, 28, 32, 16); late draw 0; palette mode 0; map set 2; `k_TileRadarColour` = `e2.hud.radar_tile_colours_set2`; tile attributes set 2; song cue 2; F5 entry 21 at (92, 30); guidance home (92, 28) | static (m9.td2 + §3) |
| 3 | 17 (`m17.td2`, map set 2) | classes FFh, 1, 1, FFh, 4, 4, FFh, 7, 7, 6, 6, 6, 8, 6, FFh × 6; armour 68, 160, 160, 68, 50, 50, 68, 50, 50, 62, 62, 40, 40, 62, 68 × 6; shields 34, 0, 0, 34, 25, 25, 34, 25, 25, 31, 31, 25, 25, 31, 34 × 6; aggression 0 × 4, 15, 15, 0, 125 × 7, 0 × 6; markers 0, 39h, 39h, 0 × 17; player class 2; view (87, 28) | static |
| 4 | 8 (`m8.td2`) | classes FFh, 0, 0, 4, FFh × 10, 8, 7, 7, 7, 8, FFh; armour 68, 200, 200, 30, 125, 125, 50, 90, 90, 90, 50, 68, 68, 68, 40, 50, 50, 50, 40, 68; shields 34, 0, 0, 18, 0, 0, 25, 0, 0, 0, 25, 34, 34, 34, 25 × 5, 34; aggression 0, 0, 0, 115, 0, 0, 115, 0, 0, 0, 115, 0, 0, 0, 115, 115, 115, 55, 115, 0; markers 39h × 11, 0 × 3, 32h × 6; player class 3, weapon 1, missiles 20, armour 20, shields 20; late draw 1; view (102, 100); `k_ClassMaxSpeed[3]` = 20h, `k_ClassAccel[3]` = 4 (and still so in any later flight of the run) | static |
| 5 | 7 | player class 3, armour 5 (not 2 and not 20), shields 20, missiles 20; slot 2 armour 75, aggression 250; view (120, 70) | static |
| 6 | 2 | slot 8: class 3, tile (0, 0), armour 68, shields 34, marker 32h, aggression 155 at setup; class FFh after tick 1's `AI_UpdateShips` (OB-035) | static |
| 7 | tile attributes | set 1: tile 45h → 2, 46h → 1, 47h..51h → 2; set 2: 2Eh, 2Fh → 2, 30h → 1, 31h, 32h → 2, 33h..39h → 1, 3Ah → 2, 3Bh → 1, 3Ch..4Bh → 2; open share 66.3% (`td.map`, set 1) and 74.8% (`td2.map`, set 2) | static + map files |
| 8 | guidance home | flight 5 → (189, 37); 6 → (125, 68); 8 with view Y 100 → (1, 102); 12 → (92, 28); 20 with view X 87 → (92, 0); 3 → (107, 104) | static |
| 9 | song | flights 1, 4, … 19 → cue 1; 2, 5, … 20 → cue 45; 3, 6, … 18 → cue 2; if still nil: mod 3 = 2 → 2, 1 → 45, 0 → 1 | static |
| 10 | F5, flight 1 | flags 1..30 = 0 except 21 = 1; entry 21 at (107, 104), lines k = 0, 1, 1, 1, 1; entry 0 lines 2, 3, 4, 1, 1; entries 6..8 lines 10, 11, 12, 13, 14 | static |

## 7. Presentation and audio hooks

- Objective: drawn into the ticker strip by every block; the ticker scrolls it from tick 1 ([hud](../render/hud.md) §4.7). Ink 37 or 42 per §3.2.
- Song: §4.4.
- Infra-red: palette mode 1 from the start in flights 3 and 10; the infra-red palette is applied after the fade-in (§4.1 step 12).
- Comm: flight 11 starts with a transmission waiting (`g_CommIncoming` := 1); the comm viewer's E2 table ([episode-deltas §6](../../re/episode-deltas.md#6-hud-and-flight-ui)) shows comm 4 for (flight 11, phase 1).
- F5 lines: §3.4 (text from the exe, ADR-0003).

## 8. Open questions

- [ ] U-240: which empty-at-setup slots that receive markers, armour, aggression or F5 lines (§5) does E2 mission logic give a class, and when? To be closed by the E2 mission specs (RE-9).
- [ ] U-241: vectors 1 (flight 1, C-0015) and 2 (flight 9, C-0016) are confirmed. Still to confirm: vectors 3, 4 and 6 with a RAM dump at the first tick of E2 flights 2, 8 and 17 (reach them with a prepared `TD2.SAV`, [e2-program-flow §8](../flow/e2-program-flow.md)) (slot arrays, ship records, class tables, DS:1E9C.., DS:1EFC..).
- [ ] U-205 (existing): first waypoint choice without the setup loop.
- [ ] Resource map: `e2.target_info` (pstring_seq, file offset 22230h, count 113) is needed in `td2.toml`; it would replace `e2.locations` and `e2.target_info.01`..`.06`, which cover 104 of the 113 strings (§3.4). Not a question about the game; listed so the dependency is not lost.

## 9. Provenance

| Claim | Address | Method | Confidence |
|---|---|---|---|
| Per-flight blocks (3.2) | `td2:0E96:60F9..6D52` (block ranges §4.2) | static (instruction stream, every store read; decoded mechanically and checked by hand) | CONFIRMED |
| Aggression writes only in `Msn_Setup` | units `0E96`, `2236`, `2D26`, `2F63` | static (raw byte scan for stores to record +3F6h: 50 stores, all in `60F9..6D52`, plus the reset in `2F63`) | CONFIRMED |
| Objective strings `e2.objectives[n − 1]` | `td2:0E96:59D7..60F8`, 20 consecutive ShortStrings, file offset 184B7h | static | CONFIRMED |
| Setup order, map set, radar copy, sprites, mission file, view defaults (4.1) | `td2:0E96:A76E..BCC8` | static | CONFIRMED |
| Absence of the setup `AI_ChooseWaypoint` loop | `td2:0E96:BCC3` (time sample) is followed directly by the tick top `BCC8`; the only call site is `td2:2D26:036C` | static | CONFIRMED |
| Weapons, missiles, palette mode, player armour/shields | `td2:0E96:B732..B84D`, `B85A..B872`, `B978..B99D` | static | CONFIRMED |
| Start view overrides | `td2:0E96:BBE9..BC36` | static | CONFIRMED |
| Song selection | `td2:0E96:B8CB..B959` | static | CONFIRMED |
| `Msn_LoadTargetInfo` and its table (3.4) | `td2:1E0B:0DCC..429C`; literal pool `1E0B:0000..0DCB` | static: decoded from the raw exe bytes (Ghidra misaligns `0DDA..11C1`); the decode ends exactly at the `RETF` at `429C` and walks all 113 pool strings | CONFIRMED |
| Tile attributes | `td2:0E96:A9D5..AAEE` (set 1), `AAF0..ACC0` (set 2) | static | CONFIRMED |
| Overlay rules | `td2:0E96:C684..C836` (set 1), `C839..C9C0` (set 2) | static | CONFIRMED |
| Home points | `td2:0E96:858B..85E3`; element 21 read via `8D3B`, `90CB` | static | CONFIRMED |
| Class tables and class-FFh bytes | `td2:36B6:041C..0443`, `0525`, `052F` (file bytes) | static (exe bytes; equal to E1) | CONFIRMED |
| Map statistics | `td.map`, `td2.map` | asset decode | CONFIRMED |
| Test vectors | `m1.td2`, `m2.td2`, `m7.td2`, `m8.td2`, `m9.td2`, `m17.td2` | computed (static); vectors 1, 2 checked against C-0015, C-0016 | CONFIRMED (1, 2); LIKELY (others, U-241) |

## 10. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-02 | RE-9a (Archaeologist + Spec author, for RE-9 M10-01) | Initial: E2 setup order, per-flight table 1..20, `Msn_LoadTargetInfo` table, tile attributes, overlay tiles, home points, test vectors |
