# Core record layouts (`file0001.exe`)

Record layouts and tables of the flight engine (M1-08): the mission file as loaded, the 20 mission slots, the ship record, projectiles, explosions, the ship-class table (U-008), the heading encoding (U-003) and the tile attribute table (U-009). Variable names are in [globals.md](globals.md); addresses are DGROUP offsets (`DS:`) unless they carry a segment.

**Method.** Static reading of the flight setup (`0E8B:B880..CC7E`), the tick loop and their callees in a private Ghidra copy, checked against the C-0002 RAM dumps 1 s and 2 s into E1 mission 1 (DGROUP at segment 4FEBh). Pascal arrays are 1-based (an array at A is indexed with displacement A − 1).

## 1. Units and coordinates (U-002)

- **World position = tile + pixel offset.** Every position in flight is a pair (tile, offset) per axis: tile = map column or row (1..201), offset = pixels inside the tile. World pixel = `tile × 32 + offset` (offsets may exceed 31 temporarily; see below). The original stores tiles as u8 (slots, projectiles, explosions) or i16 (the view) and offsets as u8 or i16. There is no sub-pixel fraction anywhere in the player or ship positions.
- **The view** (`g_ViewTileX/Y` DS:3A36/3A38, `g_ViewSubX/Y` DS:3A3A/3A3C) is the camera. Its offsets are kept in 10h..30h (16..48): when an offset leaves that range the tile changes by one and the offset by 32. The visible playfield's top-left world pixel is `(tileX × 32 + subX − 16, tileY × 32 + subY − 16)`. Check: C-0002 frame 3543 has view (102, 104, 32, 16) → (3280, 3328), which matches the Verifier's tile match on frame 3500 (top-left tile (103, 104) at pixel offset (16, 0)).
- **The player** is not stored separately: it is drawn at the centre of the 320 × 160 playfield (screen (160, 80), per the Verifier), so the player's world position is the view position plus a constant. Collision tests use the map cells at view tile + (4..6, 2..4) ([flight-model](../spec/sim/flight-model.md) §4.3).
- **Map storage is column-major**: tile (x, y) is byte `x × 201 + y` of the 201 × 201 map (DS:442E far pointer). The tile window drawn each tick is `(viewTileX + i, viewTileY + j)` for i = 0..10, j = 0..5 (`0E8B:CD22`); the inner index is the second (y) one. This agrees with the Verifier's finding (FORMATS §5 on main, 2026-11-02).

## 2. Mission file (`m<n>.td`, 20,080 bytes) as loaded

`Sim_FlyMission` reads the file with six `BlockRead`s (`0E8B:C151..C1F0`):

| File offset | Size | Destination | Meaning |
|---:|---:|---|---|
| 0 | 20 | DS:3A52 `g_SlotHdrA[1..20]` | Per-slot byte A (zero in 59 of 60 files; U-038) |
| 20 | 20 | DS:3A8E `g_SlotHdrB[1..20]` | Per-slot byte B (zero in all files; U-038) |
| 40 | 20 | DS:3A7A `g_SlotClass[1..20]` | **Class code per slot** (§6); FFh = slot unused |
| 60 | 20 | DS:3A66 `g_SlotHdrD[1..20]` | Per-slot byte D (zero in all files; U-038) |
| 80 | 10,000 | heap `g_PathX` + 1 | X paths |
| 10,080 | 10,000 | heap `g_PathY` + 1 | Y paths |

**The path arrays are `array[1..20, 0..500] of byte` in memory (stride 501), but the file holds only the 10,000 bytes from element (1, 1) on.** Element (slot s, step k) of either array is therefore at

```text
file offset = base + (s − 1) × 501 + k − 1        base = 80 (X) or 10,080 (Y); s = 1..20, k = 0..500
```

as long as that is inside the 10,000-byte block. Consequences:

- Step k = 0 of slot s (s ≥ 2) is the byte just before the slot's run, at `base + (s − 1) × 501 − 1`. These are the "odd first bytes" of FORMATS §7 (U-015): editor data that the game never copies into the ship record (only k = 1..500 are used, `0E8B:C2B0..C320`). Element (1, 0) is not in the file at all.
- The last slot is truncated: slot 20 has file data only for k ≤ 481; k = 482..500 keep whatever the heap held (zero after a fresh allocation, in practice).
- Reading the file with a stride of 500 (as FORMATS §7 rev ≤ 2026-11-02 said) shifts slot s by s − 1 bytes. Because X and Y shift together the paths still look plausible, which is why this went unnoticed.
- After loading, every FFh in steps 1..500 of **slot 2** of both arrays is replaced by 0 (`0E8B:C218..C249`). Whether the loop was meant for every slot is unknown; preserve it.
- **Where the arrays live (RE-18, U-500).** `Sim_FlyMission` frees the music buffer, saves the heap top with `BP_Mark` into DS:16EA (`g_FlightHeapMark`, M) and allocates 17 buffers in a fixed order (`0E8B:B975..BA68`): `g_Font5` 975, the eight projectile arrays (51 or 102 bytes), `g_FacFrames` F50h, `g_WeaponIcons` F30h, `g_PathX` 2724h, `g_PathY` 2724h, `g_RotSc3` 3840h, `g_RotSc2` 3840h, `g_RotShr` 7D00h, `g_SpriteBank` 6C00h (BP7 rounds each to 8 bytes). So `g_PathX` = M + 9,328 and `g_PathY` = M + 19,352 in every flight of a run (C-0200, C-0300: M = 5C11:0000 at a direct start); `BP_Release` (`0E8B:F9DE`) gives the memory back at the end. Between flights the front end reuses it from M (music buffer, 975-byte font blocks, 16,000-byte portrait blocks). Flight 21 loads no file, so its arrays hold those leftovers: [mission-ships §4.1.1](../spec/sim/mission-ships.md). In a direct start the bytes the file never writes (element (1, 0), slot 20's steps 482..500) are 0 under DOSBox (fresh memory; C-0200, C-0300 flight 19).

Check: mission 1, slot 1 starts at (7Ch, 49h) and slot 2 at (7Dh, 49h); RAM `g_SlotTileX/Y` at +1 s are (7Ch, 4Bh) and (7Dh, 4Bh) and the ship records hold path step 1 = (7Ch, 49h) (§3).

`X` is the first array: the slot X tile (`g_SlotTileX`, fed from it) is the one the code pairs with the view X tile (`g_ViewTileX`, the map's major index) in distance tests and sound calls.

## 3. Ship record (`g_Ships`, DS:4432, 20 × 1065 bytes)

`record[s]` for s = 1..20 starts at `DS:4432 + (s − 1) × 0429h` (the code addresses fields as `s × 0429h + (4009h + field)`). The record ends at DS:9765, directly before the heap pointers at DS:9770.

| Field | Size | Meaning | Conf. | Evidence (M1 slot 1, +1 s / +2 s) |
|---:|---:|---|---|---|
| +000h | 1 | Dead flag (fixed-point mode) | CONFIRMED | 0 |
| +001h | 2 | Waypoint (x, y): the tile the ship flies to, at most one tile step from the previous one | CONFIRMED | (7C, 4C) / (7C, 51) |
| +003h | 2 | Previous waypoint (x, y) | CONFIRMED | (7C, 4B) / (7C, 50) |
| +005h..00Ah | 6 | AI: blocked-Y, blocked-X flags; i16 step X (+007h), step Y (+009h) ([mission-ships §3.1](../spec/sim/mission-ships.md)) | CONFIRMED | 0 |
| +00Bh | 2 | **Path cursor** (u16, index of the last path step reached; = "path step 0") | CONFIRMED | 4 / 9 |
| +00Dh + 2(k−1) | 2 × 500 | **Path copy**: (x, y) of steps k = 1..500 (from §2) | CONFIRMED | step 1 = (7C, 49) |
| +3F5h | 1 | Direction of the last waypoint step (2/4/6/8); facing of the wall follower | CONFIRMED | 2 / 2 |
| +3F6h | 1 | Aggression: fire-budget unit set per flight (`Msn_Setup`); FFh for every slot after the M+K+A+R chord | CONFIRMED | 0 (32h for slot 14) |
| +3F8h | 2 | Shields, initialised from `k_ClassShields[class]` | LIKELY | 0 (class 0) |
| +3FAh | 2 | Armour, initialised to `2 × k_ClassArmour[class]`, then overridden per mission | LIKELY | 3 (slot 12: 1000) |
| +3FCh..428h | | AI state: hold flag and schedule (+3FCh..+412h), attack mode +415h, attack counter +416h (u16), random detour hand +419h, detour steps +41Ah, direct mode +41Bh, in-line +41Ch, out-of-range +41Dh, target selector +41Fh, target position +420h/+421h, dead fixed-point fields +422h..+425h, return mode +428h. Full table: [mission-ships §3.1](../spec/sim/mission-ships.md) | CONFIRMED | +420h/+421h = player tile (6Bh, 6Bh) |
| +426h | 2 | Return point: the waypoint the ship last left while following its path | CONFIRMED | (7C, 4B) / (7C, 50) |

Mission ships advance one waypoint per tile reached; movement, AI and the record fields are specified in [sim/mission-ships.md](../spec/sim/mission-ships.md) rev 2 (U-039, U-045 resolved; C-0009).

## 4. Mission-slot arrays (DGROUP, index s = 1..20)

| Array | Base (element 1) | Type | Meaning | Conf. |
|---|---|---|---|---|
| `g_SlotClass` | DS:3A7A | u8 | Class code (§6). Per-mission overrides in `0E8B:60D8`; set to FFh when the ship is destroyed (`0E8B:12CB`) | CONFIRMED |
| `g_SlotHeading` | DS:3AA2 | u8 | Heading, keypad code (§7) | LIKELY |
| `g_SlotTargetHeading` | DS:3AB6 | u8 | Heading the ship turns toward | CONFIRMED |
| `g_SlotSubX` | DS:3ACA | i16 | Pixel offset X (C-0002: 10h) | LIKELY |
| `g_SlotSubY` | DS:3AF2 | i16 | Pixel offset Y (C-0002: 1Eh, changing) | LIKELY |
| `g_SlotTurnDir` | DS:3B1A | u8 | Turn direction 1 ccw / 2 cw | CONFIRMED |
| `g_SlotTileX` | DS:3B2E | u8 | Tile X | CONFIRMED |
| `g_SlotTileY` | DS:3B42 | u8 | Tile Y | CONFIRMED |
| `g_SlotSpeed` | DS:3B6A | u8[20][4] | Speed components up/down/left/right (element (s, k) at DS:3B6A + 4(s − 1) + k) | CONFIRMED |
| `g_SlotGoalX/Y` | DS:3BBA / DS:3BCE | u8 | Waypoint copy used by the thrust step | CONFIRMED |
| `g_SlotMarker` | DS:3958 | u8 | Target bracket 0 / 32h / 39h (set per flight) | CONFIRMED |
| `g_FireRequest` | DS:3BF6 | u8[20][9] | Fire request per slot and entry 1..9 (element (s, e) at DS:3BEC + 9s + e); the AI sets entries 2/4/6/8; consumed every second tick, then cleared | CONFIRMED |

## 5. Projectiles and explosions

**Projectiles:** 8 heap arrays allocated at flight start (`GetMem(51)` for byte arrays, `GetMem(102)` for word arrays), indexed 0..40 (41 slots; the remaining 10 bytes are unused). A slot is free when its tile X is FFh (all 51 tile-X bytes are set to FFh at setup). A new shot takes the **highest-numbered** free slot (the search runs 0..40 and keeps the last match).

| Array (far ptr at) | Type | Meaning when spawned by a mission ship (`0E8B:F316..F48F`) | Conf. |
|---|---|---|---|
| DS:3986 | u8 | Tile X (FFh = free) := ship tile X | LIKELY |
| DS:398A | u8 | Tile Y := ship tile Y | LIKELY |
| DS:3976 | i16 | Pixel offset X := muzzle dx + ship offset X − 16 | LIKELY |
| DS:397A | i16 | Pixel offset Y := muzzle dy + ship offset Y − 16 | LIKELY |
| DS:397E | u8 | Heading := ship heading | LIKELY |
| DS:3982 | u8 | Kind := shot-table value − 1 (0..3 guns, 4/5 missiles, 6 homing) | LIKELY |
| DS:396E | u8 | Owner slot | LIKELY |
| DS:3972 | u8 | Life counter (starts at 1, or 0 for missiles; the shot dies above 25); target slot for kind 6 | LIKELY |

Movement (one tile per tick), lifetime, map collision, hit tests and damage: [sim/weapons.md](../spec/sim/weapons.md).

**Explosions:** 30 slots, five `array[1..30] of u8` in DGROUP: frame DS:399A (8 = idle; all set to 8 at setup), tile X DS:39B8, tile Y DS:39D6, offset X DS:39F4, offset Y DS:3A12. Each tick an active explosion (frame < 8) is drawn with sprite 13h + frame if it is inside the 11 × 6 window, then frame += 1, skipping 7 (so frames 0..6 show, then idle). Started by `0E8B:0984`, `2404:1426` and `0E8B:56ED`.

## 6. Ship classes (U-008)

The slot class code from the mission file (and the player class `g_PlayerClass`) indexes four typed-constant tables directly (no −1): `k_ClassMaxSpeed` DS:02BE, `k_ClassArmour` DS:02C8, `k_ClassShields` DS:02D2, `k_ClassAccel` DS:02DC, plus the shot table DS:02E5 (17 entries per class). FFh = no ship.

| Code | Max speed (raw) | Max speed (px/tick) | Accel (per tick) | Armour | Shields | Uses in E1 | Name |
|---:|---:|---:|---:|---:|---:|---|---|
| 0 | 18h | 8 | 2 | 31 | 0 | 128 slots; player in flight 19 | truck (LIKELY) |
| 1 | 1Ch | 12 | 4 | 16 | 0 | 37 slots | transport (LIKELY) |
| 2 | 20h | 16 | 4 | 15 | 18 | 22 slots; player in flights 6–8, 11, 17, 18, 20 | Stiletto (LIKELY) |
| 3 | 1Ch | 12 | 4 | 20 | 20 | 29 slots; player in flights 12, 15, 21 (helicopter sprites loaded) | helicopter (HYPOTHESIS) |
| 4 | 1Ch | 12 | 4 | 25 | 25 | 48 slots; player in flights 1–5, 9, 10, 16 | Hornet (LIKELY) |
| 5 | 1Ch | 12 | 4 | 25 | 20 | 4 slots | Stingray (HYPOTHESIS) |
| 6 | 18h | 8 | 2 | 31 | 31 | none; player class of the J+D+W chord, which also raises its speed to 1Ch / 4 | special (HYPOTHESIS) |
| 7 | 1Ch | 12 | 4 | 25 | 25 | 243 slots (most common) | Vulture I (LIKELY) |
| 8 | 20h | 16 | 4 | 20 | 25 | 124 slots; player in flights 13, 14 | Vulture II (LIKELY) |
| 9 | 0 | 0 | 0 | 0 | 0 | none | unused |

"Max speed (px/tick)" = raw − 16 ([flight-model](../spec/sim/flight-model.md) §2). Armour and shields are the raw table values; the player uses them as they are (C-0002: class 4 → 25 / 25), mission ships get `2 × armour` (often overridden per mission, §3). Names are matched from: the class stat lines of the ship chart `0E8B:7374` (armour/shield pairs 2000/2500 and 1500/1800 match classes 8 and 2 exactly; 2500/2500 matches 7; truck and transport have 0 shields; the chart's speed order matches the raw speeds); the hint before flight 6 names the Stiletto (player class 2 there); the orders before flight 2 name a Hornet-class ship (player class 4); flights 12, 15 and 21 load `td.hel`/`td.h2`; the status panel shows the Vulture II text when the player switches to class 8 (`0E8B:A185`). The chart's numbers for Hornet and Stingray do not match the tables (they repeat the Stiletto line). Sprite selection per class is not mapped yet (U-042).

Player class per flight (`0E8B:60D8`): 1–5 → 4; 6–8 → 2; 9–10 → 4; 11 → 2; 12 → 3; 13–14 → 8; 15 → 3; 16 → 4; 17–18 → 2; 19 → 0; 20 → 2; 21 → 3.

## 7. Heading encoding (U-003)

Headings are **keypad codes**: the eight principal directions are the digits of the numeric keypad (8 up, 9 up-right, 6 right, 3 down-right, 2 down, 1 down-left, 4 left, 7 up-left) and the eight intermediate directions are the two neighbouring digits written as a decimal number. Clockwise from left:

| Order | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Code (dec) | 4 | 74 | 7 | 78 | 8 | 98 | 9 | 96 | 6 | 36 | 3 | 32 | 2 | 12 | 1 | 14 |
| Code (hex) | 04 | 4A | 07 | 4E | 08 | 62 | 09 | 60 | 06 | 24 | 03 | 20 | 02 | 0C | 01 | 0E |
| Direction | W | WNW | NW | NNW | N | NNE | NE | ENE | E | ESE | SE | SSE | S | SSW | SW | WSW |

The ring is stored at DS:02AD..02BC starting with 0Eh (WSW), with wrap entries 01h at DS:02AC and 0Eh at DS:02BD. One turn step moves to the previous entry (`g_PlayerTurnDir` = 1, counter-clockwise) or the next (= 2, clockwise). Each of the 16 codes has its own blit routine in `386E` (8 → `386E:024A`, 0E → `1193`, 0C → `172D`, 20 → `15C1`, 4E → `0D68`, 62 → `0EC0`, 24 → `1027`, 4A → `1302`, 60 → `145A`, 2 → `0C02`, 6 → `07BC`, 4 → `0926`, 7 → `0391`, 1 → `0A93`, 9 → `04E9`, 3 → `0650`), so the sprite heading is the logical heading: there is no finer angle and no sine table for the player. (Real48 trigonometry is used elsewhere, for example the guidance line `0E8B:82DD`.)

The muzzle table DS:035C is indexed with `heading × 34`, which is only meaningful for the single-digit codes; a ship firing while on a two-digit heading reads far outside the table (U-040).

## 8. Tile attributes (U-009)

`g_TileAttr` (far pointer DS:3CAA) is filled at flight start (`0E8B:BA6C..BB60`): attribute 2 = open (the ship can fly over), 1 = solid. Tiles above 46h are not initialised (heap contents; they do not occur in `td.map`, whose highest index is 70 = 46h).

| Tile indices | Attribute |
|---|---|
| 0–14 | 1 solid |
| 15–16 | 2 open |
| 17–48 | 1 solid |
| 49–51 | 2 open |
| 52–55 | 1 solid |
| 56–61 | 2 open |
| 62–68 | 1 solid |
| 69 | 2 open |
| 70 | 1 solid |

66% of `td.map` is open (the five most common tiles 49, 50, 16, 15, 61 are all open). Collision is per tile against fixed cells around the player, with a bounce response ([flight-model](../spec/sim/flight-model.md) §4.3). Tiles 42h, 44h, 45h, 46h are redrawn over ships each tick (overpasses; tick-order step 12). The table is the same for every E1 flight. E2 (two map sets) and E3 have their own tables and overlay codes, and E3 adds hazard tiles: [episode-deltas §4.4](episode-deltas.md#44-tile-attributes-and-overlay-tiles-u-009).

## 9. Provenance

| Claim | Address / capture | Method | Confidence |
|---|---|---|---|
| Mission file read order and destinations | `0E8B:C151..C1F0` | static | CONFIRMED |
| 501-byte path stride, k = 1..500 copied | `0E8B:C2C0..C3BF` (IMUL 1F5h), C-0002 ship records | static + capture | CONFIRMED |
| Slot-2 FFh → 0 fix-up | `0E8B:C218..C249` | static | LIKELY |
| Ship record size and base | stride 0429h in `0E8B:B880`, `0984`, `9860`, `2404`, `3292`, `34E6`; DS:442E map pointer precedes it | static + capture | LIKELY |
| Class tables and their meaning | DGROUP bytes DS:02BE..02E4; `0E8B:C94A..C963`, `0E8B:BB60..` | static + capture (player 25/25) | CONFIRMED (values), LIKELY (names) |
| Heading ring and per-heading blits | DS:02AC..02BD, `0E8B:CCE3`, `0E8B:D199..D624` | static + capture (heading 8 at rest) | CONFIRMED |
| Tile attributes | `0E8B:BA6C..`, C-0002 heap contents | static + capture | CONFIRMED |
| Projectile arrays | `0E8B:B978..B9F0` (allocation), `0E8B:F316..F48F` (spawn) | static | LIKELY |
| Explosion arrays | `0E8B:C93A`, `0E8B:D872..` | static | LIKELY |
| View-to-world relation | C-0002 RAM + Verifier tile match | capture | LIKELY |
