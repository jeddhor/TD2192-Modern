# Mission logic (Episode 3)

| | |
|---|---|
| **Spec ID** | `sim/e3-mission-logic` |
| **Revision** | 1 |
| **Status** | Draft |
| **Fidelity tier** | T1 (phase changes, spawns, removals, class-table and map patches, success, lost and outcome decide the result of a flight; rule order matters where two rules write the same ship) · T2 for the ticker text ([hud](../render/hud.md) §4.7) and for the flight-16 palette effects (presentation) |
| **Confidence** | Primitives, base tests, every rule of `td3:1FEB:0A8D..167F` (`Msn_UpdateA`: 81 calls, 64 stores) and `1FEB:19AD..229D` (`Msn_UpdateB`: 75 calls, 56 stores), every branch of `Msn_E3TimedEvents` (`0C5D:959E..9A6C`), start state, comm viewer, outcome tests and the flight-16 specials: CONFIRMED (code; listings checked gap-free). Gameplay consequences (who reaches which tile when, what frozen ships do, what the player sees): LIKELY. Capture C-0036 confirms the flight-1 start state and that no rule fires in its first ticks (§7 vector C1); C-0041 confirms the flight-8 start state and the hazard test (§7, [e3-mission-ships](e3-mission-ships.md) §4.4) |
| **Tasks** | M10-02 |
| **Unknowns** | U-270..U-279 (new, §8); U-202 and U-203 resolved (§4.9, §4.7.3); U-204, U-205, U-244 apply unchanged |


Read this spec as a **delta against the Episode 2 spec** [e2-mission-logic](e2-mission-logic.md), which is itself a delta against [mission-logic](mission-logic.md) (E1). Concepts, notation and primitives that are the same are referenced, not repeated. Setup values (classes, markers, armour, aggression, F5 tables) are owned by [e3-mission-setup](e3-mission-setup.md); this spec repeats only what the rules read. Addresses are `td3:<seg>:<off>` (image-relative, ADG §7.8); the `td3:` prefix is dropped where unambiguous. DGROUP is segment `32C5`; `DS:x` is an E3 DGROUP offset. Slot numbers are 1-based throughout (the listing annotations of the per-slot arrays are 0-based; §2).

## 1. Summary

The E3 framework is the E2 framework ([e2-mission-logic](e2-mission-logic.md) §1, §4) with E3 addresses: `Msn_UpdateA` (`1FEB:0A7F..1683`, flights 1–9) and `Msn_UpdateB` (`1FEB:19AD..229D`, flights 10, 11, 21, 12–19 in that order; E3 has no flight 20) run once per tick right after the player draw; only the block of the current flight runs; every block starts with its base test; and the primitives `Msn_ExitAt`, `Msn_NearLive`, `Msn_PostTicker` and `AI_SetTarget` are E2's (byte-identical code, §3.1). `Msn_NearLive` measures the player at (viewX + 5, viewY + 3), as in E2. A new per-tick routine, `Msn_E3TimedEvents` (`0C5D:959E`), drives the timers of flights 10, 12, 14, 18 and 21, the flight-5 lost rule and the flight-16 freeze (§4.6). The differences that matter:

1. **Few announced transmissions.** No flight 1–9 rule sets comm pending. Flight 8 has a silent transmission window (phase 2, OB-044); flights 15 and 19 announce theirs; in flight 16 the transmission is **forced** (step 15 sets the comm request every tick in phase 2).
2. **Flight 7 never sets its phase.** `Msn_Setup` case 7 has no phase store, so flight 7 starts in the phase flight 6 left, which is always 100 in campaign play (OB-043, §5.7).
3. **Timers.** Flight 10 (tick 10: phase 2 and slot 4 released; tick 300: phase 3; then the phase counts ticks 4 → 40), flight 14 (spawns at tick 25, attacks from tick 28, two silent removals), flight 18 (a 2160-tick wait, then the F5 map point appears, U-202 resolved), flight 21 (attacks from tick 11), flight 12 (the 27 hazard cells switched off by setup come back at tick 6480).
4. **Positional flights.** Flights 6, 11, 13, 14 and 21 succeed on reaching an area (11, 13, 14 and 21 without a phase test); flight 17 starts in phase 100 (setup), so it is "fly home".
5. **Flight 16.** After the forced transmission the event routine fades the DAC to a magenta tint and sets the maximum speed and acceleration of every class to 0 except class 2 (20 and 4): every mission ship stops; the player flies home slowly. `Msn_Setup` case 17 writes the original values back (OB-028 corrected, §6.3); the tint is never written to the flight palette (U-203 resolved, §4.7.3).
6. **Lost** is set only by the flight-5 rule (escort gone). **Outcome tests** exist for flights 2, 3, 4, 5, 8, 9, 10 and 12 (§4.8).
7. **No RNG** is drawn by `Msn_UpdateA`, `Msn_UpdateB` or `Msn_E3TimedEvents`.

## 2. Definitions and units

All terms of [e2-mission-logic](e2-mission-logic.md) §2 apply with these E3 addresses (each is the E2 address minus 4 unless marked):

| Term | Original (E3) | Notes |
|---|---|---|
| Class(*s*) | `g_SlotClass` DS:399D + *s*, u8 | FFh = not in play. **Trap:** the listing annotates DS:39A5 as `g_SlotClass[7]`; it is slot 8 (DS:399D + 8). Checked against `Msn_ExitAt` (`1FEB:060D`) and every rule below |
| Tile(*s*) | (`g_SlotTileX` DS:3A51 + *s*, `g_SlotTileY` DS:3A65 + *s*), u8 | Set from path step 1 at `0C5D:B385..B3C8`, before `Msn_Setup`; kept after removal |
| Phase | `g_MissionPhase` DS:31DE, u8 | Set to 1 by `Msn_Setup` (`0C5D:6149`) for flights 1–6, 8, 9. **Not set for flight 7** (§3.2, OB-043) |
| Ticker idle | `g_TickerPos` DS:31E0 = 0 (u16) | No flight 1–9 rule reads it. Every post sets it to 1 |
| Comm pending | `g_CommIncoming` DS:31C0, u8 | Cleared at flight start (`0C5D:A9DF`). **No flight 1–9 rule and no setup case 1–9 writes it** |
| Marker(*s*) | `g_SlotMarker` DS:387B + *s*, *s* = 1..21 | Element 21 (DS:3890) = home marker. Written only by setup in flights 1–9 |
| Succeeded | `g_MissionSucceeded` DS:31C2, u8 | Cleared at flight start (`0C5D:AA28`); recomputed by the base test every tick |
| Lost | `g_MissionLost` DS:11EC, u8 | Cleared at `0C5D:AA2D`. In flights 1–9 only `Msn_E3TimedEvents` sets it (flight 5, §4.6.6) |
| Over / End ticks | `g_MissionOver` DS:31DB / `g_EndTicks` DS:31DC | Not written by flights 1–9 |
| Outcome / Died | `g_MissionOutcome` DS:11EF / `g_PlayerDied` DS:11ED | Teardown (§4.8) |
| View tile, view offset | `g_ViewTileX/Y` DS:395A/395C (i16), `g_ViewSubX/Y` DS:395E/3960 (i16, 16..48) | |
| Rule loop counters | `g_MsnI` DS:31E2, `g_MsnJ` DS:31E4 (u16) | DS:31E4 has no name in `td3.csv` yet (proposed: `g_MsnJ`, = E2 DS:31E8). Flights 4, 6, 7 use it |
| Scratch "all gone" flags | DS:31C4 (flights 2, 3, 4, 6), DS:31C9 (flight 7) | `g_BaseHit` elements 1 and 6 (DS:31C3 + *s*), cleared for *s* = 1..20 at `0C5D:AA3C..AA56`; recomputed in the tick that uses them |
| Comm request, F6 key | `g_CommRequest` DS:1627; F6 key byte DS:19C0 | §4.5 |
| Ship record | `g_Ships` DS:4356 + (*s* − 1) × 429h | +001h/+002h waypoint X/Y (read by flight 8), +3F6h aggression, +3F8h shields, +3FAh armour (i16) |

| Tick count | `g_MissionTicks` DS:1638 (u32) | := 0 at `0C5D:B0A3..B0A8` (flight start), += 1 at `0C5D:BB20..BB30`, the first statement of every tick. Compared as 32-bit (high word DS:163A = 0 and low word = *n*) |
| Draw page | `g_DrawPage` DS:3954 | "Back page = 0" in the comm viewer test (E2 wording) is DS:3954 = 0 |
| Map buffer | far pointer `g_Map` DS:4352 | The 40,401-byte map file read unchanged at `0C5D:B47E` (before `Msn_Setup` at `B66E`); cell (*x*, *y*) at offset *x* × 201 + *y* (the hazard test `0C5D:D298..D2B1` indexes it the same way). Reloaded at every flight start |
| Flight palette | `g_FlightPalette` DS:1300, 768 bytes | `td.pal`, read at `0C5D:AD51..AD62` at every flight start; the flight-16 tint never writes it |
| Hit flash, flash frames | `g_HitFlash` DS:11E4 (u16), `g_FlashFrames` DS:1748 (u8) | [tick-order](tick-order.md) step 14; flash frames := 4 on a homing-missile kill or hit (`0C5D:0D22`, `1237`; [weapons](weapons.md) §4.3 steps 3–4) |
| Palette mode | `g_PaletteMode` DS:161E | Always 0 in E3: it is only ever set to 0 (`0C5D:793F`, `A3DA`, `B0AB`) except inside the Alt handler (`E486..E607`), which acts only in modes 1 and 2, so that handler is dead code |
| Class tables | `k_ClassMaxSpeed` DS:041C + c, `k_ClassAccel` DS:043A + c (c = 0..9) | Typed constants, initialised from the exe image at program start; patched by flight 16 (§4.6.4), restored by `Msn_Setup` case 17 |
| F5 target information | flag DS:1E98 + e, map X DS:1EB8 + e, map Y DS:1ED8 + e, line *l* of entry *e* at DS:1EF8 + 155e + 31l | Filled once per flight by `Msn_LoadTargetInfo` (`1BD8:0DCF`, from `Msn_Setup`). Entry 0 = player, 1..20 = slots (both rewritten by the F5 screen), **entry 21 = the fixed map point**; its flag is **DS:1EAD** (§4.9) |
| Ship record fields | record(*s*) at DS:4356 + (*s* − 1) × 429h | +3F6h aggression, +3F8h shields (i16), +3FAh armour (i16). DS:70EA = armour(11), DS:646F = armour(8), DS:57F4 = armour(5), DS:4B79 = armour(2), DS:818C = shields(15) |
| Unused flags | `g_CutsceneRequest` DS:31DA, `g_MsnEventCount` DS:31C1 | Cleared at flight start (`0C5D:AA32`, `AA37`); no E3 code sets or reads them in flight |

**Player positions.** As E2: `Msn_NearLive` with slot 0 uses (low byte of viewX + 5, low byte of viewY + 3) (`1FEB:0648..0657`, `067E..068D`); base tests use the view tile and, in some flights, viewSubY.

### 2.1 Notation

As [e2-mission-logic](e2-mission-logic.md) §3.2, with these points:

| Notation | Meaning |
|---|---|
| `post(M1[k])`, `post(M1[k], c = 42)` | `Msn_PostTicker(0, 735, M1[k], colour)` then ticker := 1; colour 37 unless written `c = 42`. Every post in flights 1–9 is followed by ticker := 1 |
| `M1[k]` | `e3.flight_messages.01`, element *k* (0-based), §3.4 |
| `engage a ⇄ b, r` | **one** range test `Msn_NearLive(a, b, r)`; if true, `AI_SetTarget(a, b)` then `AI_SetTarget(b, a)` (flight 4). Same effect as `engage a → b, r; engage b → a, r`, because `Msn_NearLive` has no side effects and `AI_SetTarget` changes no tile or class |
| `gone(s, …)` / `dead(s, …)` | every listed slot has class FFh (destroyed, left, hidden or empty) |
| `allGone(a..b)` | a scratch flag := 1, then := 0 if any slot of a..b has class ≠ FFh (flights 2, 3, 4, 6, 7) |
| `wp(s)` | ship record (*s*) +001h/+002h, the waypoint (X, Y) the ship is flying to ([mission-ships](mission-ships.md) §2) |
| player | slot 0 in `Msn_NearLive` |
| `@tick n: …` | done by `Msn_E3TimedEvents` when `g_MissionTicks` = *n*, i.e. at the end of tick *n* (new step after step 24). Rules of `Msn_UpdateB` see it in tick *n* + 1 |
| `home17` | view tile = (17, 164) and viewSubY ≤ 35 (the original tests viewX = 17, then viewSubY < 36, then viewY = 164; i16 compares). viewSubX is not tested |
| `view ∈ X × Y` | viewX ∈ X and viewY ∈ Y (separate exact i16 compares, any sub-tile offset) |
| `marker(s)` | `g_SlotMarker` DS:387B + *s* |

## 3. Data

### 3.1 Primitives (unit `1FEB`)

| Name | E3 address | Against E2 |
|---|---|---|
| `Msn_DrawTickerGlyph` | `1FEB:0000..0309` | Identical |
| `Msn_PostTicker` | `1FEB:030A..05DB` | Identical: `(X, Y: Integer; S: String; Colour: Byte)`, glyph routine `1FEB:0000` |
| `Msn_ExitAt` | `1FEB:05DC..0615` | Identical: `(Slot, X, Y: Byte)`; if tile(Slot) = (X, Y): class(Slot) := FFh. No class test |
| `Msn_NearLive` | `1FEB:0616..070E` | Identical: `(A, B, Range: Byte): Boolean`; player = slot 0 at (viewX + 5, viewY + 3); \|dx\| < Range and \|dy\| < Range (signed 16-bit differences of zero-extended bytes, strict); false if A or B (when non-zero) has class FFh; symmetric |
| `AI_SetTarget` | `2B72:3232` | Unit `2B72` is code-identical to E1 `34E6` |

Method: a byte compare of `td2:2236:0000..070E` with `td3:1FEB:0000..070E`. The only differing bytes are far-call segment words (relocations) and DGROUP offsets, each 4 lower in E3 (3A55 → 3A51, 3A69 → 3A65, 39A1 → 399D, 395E/3960 → 395A/395C, font pointer 9698 → 9694). CONFIRMED.

No message of `e3.flight_messages.01` contains a digit (byte scan), so the glyph routine's digit-advance quirk never shows in flights 1–9.

`Msn_UpdateB` calls `Msn_PostTicker` (11×), `Msn_ExitAt` (23×), `Msn_NearLive` (20×), `AI_SetTarget` (`2B72:3232`, 20×) and the stack check (1×), nothing else. Every post is `Msn_PostTicker(0, 735, M2[k], 42)` followed by ticker := 1 (the two alternative posts of flight 10 share one ticker store, `1FEB:1B4A`). No message of flights 10–21 uses colour 37.

`Msn_UpdateB` has no local frame (prologue `19AD..19B6`, epilogue `229C..229D`). It uses the loop counter `g_MsnI` (DS:31E2) only; `g_MsnJ` (DS:31E4) and the scratch flags of `Msn_UpdateA` are not used. Every block falls through to the next flight compare, and no block writes the flight number, so exactly one block runs per tick.

### 3.2 Per-flight start state that the rules depend on

#### Flights 1–9

Set by `m<n>.td3` (class bytes at file offset 40..59; path step 1 = start tile), the start view (`0C5D:B596..B60C`, before `Msn_Setup`, no later override) and `Msn_Setup` (`0C5D:6149`; blocks below). The setup spec is authoritative for armour, aggression and other record fields; this table lists what the rules read. Every case clears markers 1..20 first, sets the home marker (DS:3890) := 3Dh and draws `e3.objectives[n − 1]` at (0, 735) with `Txt_FlightDrawStringOpaque` (`0C5D:45C8`), then ticker := 1.

| Flight | Setup block | Map | Objective (colour) | Player class | Start view (offsets 32, 16) | Markers | Live at start (class) | Hidden by setup | Other setup writes |
|---:|---|---|---|---:|---|---|---|---|---|
| 1 | `6155..6248` | td3 | `e3.objectives[0]` (37) | 2 | (167, 66) | 2..4, 8, 9, 11..14 = 32h | 5, 6 (7); 8, 9 (8) | 2..4 (file class 0), 11..14 (file class 7) | aggression(2..14) := C8h |
| 2 | `6249..62E8` | td3 | `[1]` (37) | 2 | (167, 66) | 2..7 = 32h | 2..7 (0); 9..12 (6); 15..17 (7, setup; file 7, 6, 6) | — | aggression(9..18) := FFh |
| 3 | `62E9..6388` | td3 | `[2]` (37) | 2 | (167, 66) | 2..4, 9..12 = 32h | 2..4 (0); 6, 7 (6); 9..12 (7) | — | aggression(6..14) := FFh |
| 4 | `6389..6409` | td3 | `[3]` (37) | 4 | (167, 66) | 5..7 = 32h | 2, 3 (4); 5..7 (0); 9..11, 13 (8); 12 (6) | — | aggression(2..15) := FFh |
| 5 | `640A..64BB` | td3 | `[4]` (37) | 2 | (167, 66) | 2 = 39h; 5..7 = 32h | **2 (0, the escort)**; **3 (2, setup; file 4)**; 5..7, 9 (7); 10, 11 (6) | — | armour(2) := 900; aggression(3..12) := FAh, then aggression(5..12) := 96h |
| 6 | `64BC..653C` | td3 | `[5]` (37) | 2 (set after the objective) | (167, 66) | 2..10 = 39h | 2..5 (4); 6 (2); 9, 10 (0); 13, 14 (8); 15..17 (6) | — | aggression(2..17) := C8h |
| 7 | `653D..65B8` | td4 | `[6]` (37) | 2 (after the objective) | (185, 32) | 2..10 = 39h | 2, 3, 5 (4); 6, 7 (2); 8 (7); 9, 10 (0); 12, 13 (8); 15, 16, 18 (6) | — | aggression(12..18) := 4Bh; **no phase store**; player armour := 5 (`0C5D:B90E`, [episode-deltas](../../re/episode-deltas.md) §4.3) |
| 8 | `65B9..666A` | td4 | `[7]` (**42**) | 6 | (17, 164) | 1..3 = 39h | 1..3 (0); 4, 7 (7); 5, 6, 8..10 (8) | 11..20 (empty in the file: no effect) | armour(1..3) := 100; aggression(4..10) := FFh |
| 9 | `666B..671C` | td4 | `[8]` (**42**) | 6 | (17, 164) | 1..3 = 32h | 1..3 (0); 4, 5, 8, 9 (8); 6, 7 (7) | 10..20 (empty: no effect) | armour(1..3) := 100; aggression(4..9) := FFh |

Notes on the table:

- Empty slots (class FFh in the file): flight 1: 1, 7, 10, 15..20; 2: 1, 8, 13, 14, 18..20; 3: 1, 5, 8, 13..20; 4: 1, 4, 8, 14..20; 5: 1, 4, 8, 12..20; 6: 1, 7, 8, 11, 12, 18..20; 7: 1, 4, 11, 14, 17, 19, 20; 8: 11..20; 9: 10..20. Per-slot header bytes A, B, D are zero in all nine files.
- Armour and shields are filled from the **file** class at `0C5D:B242..B28D` (armour = 2 × `k_ClassArmour[class]`, DS:0426 + class), before `Msn_Setup`. A slot whose class is changed by setup or by a spawn keeps the record values of its file class unless setup overwrites them: flight 1 slots 2..4 spawn as class 7 with class-0 armour (62, not 50); flight 2 slots 16, 17 are class 7 with class-6 armour (62). LIKELY (setup spec authoritative).
- Home points (guidance target when no marked slot is live, `UI_DrawGuidanceLine` `0C5D:81C1`): flights 1–5 (172, 68); 6 (1, 110); 7–9 (22, 166) ([episode-deltas](../../re/episode-deltas.md) §6).
- Start player tile: (172, 69) for flights 1–6, (190, 35) for flight 7, (22, 167) for flights 8 and 9.

#### Flights 10–19 and 21

Set by `m<n>.td3` (class bytes 40..59, path step 1 = start tile), the start view (`0C5D:B596..B60C`, offsets (32, 16), no later override) and `Msn_Setup` (`0C5D:6149`; cases below). Every case clears markers 1..20, sets its own markers, sets the home marker DS:3890 := 3Dh and draws `e3.objectives[k]` at (0, 735) with `Txt_FlightDrawStringOpaque` (`0C5D:45C8`), then ticker := 1. All flights ≥ 7 fly `td4.map`. The setup spec is authoritative for armour, shields and aggression; this table lists what the rules and the event routine read.

| Flight | Setup block | Objective (colour) | Player class | Start view → player tile | Phase at start | Markers | Live at start (class) | Hidden by setup | Other setup writes |
|---:|---|---|---:|---|---:|---|---|---|---|
| 10 | `671D..67B5` | `e3.objectives[9]` (37) | 6 | (17, 164) → (22, 167) | 1 | 5 = 39h; 7..10 = 32h | 3 (2); 7, 8, 11, 13 (7); 9, 10, 12 (8) | **4** (file 2), **5** (file 0) | armour(5) := 100; aggression(3..13) := FFh |
| 11 | `67B6..681A` | `[10]` (37) | 6 | (17, 164) → (22, 167) | 1 | none | 3..8 (7); 9..13 (8); 14, 15 (3) | — | aggression(3..15) := FFh |
| 21 | `681B..687F` | `[11]` (37) | 6 | **(160, 75)** → (165, 78) | 1 | none | as flight 11 (`m21.td3` is byte-identical to `m11.td3`) | — | aggression(3..15) := FFh |
| 12 | `6880..6A11` | `[12]` (42) | 2 | (17, 164) → (22, 167) | 1 | 1..6 = 32h | 1..6 (0); 8..15 (8) | — | **27 map cells := 26h** (§4.6.3); aggression(8..15) := C8h; missiles := 10 (`0C5D:B797..B7A5`, class 2 in flight 12) |
| 13 | `6A12..6A76` | `[13]` (42) | 7 | (17, 164) → (22, 167) | 1 | none | 3..6 (7); 7, 8 (3); 11, 12 (0) | — | aggression(3..8) := C8h |
| 14 | `6A77..6AF7` | `[14]` (42) | 3 | **(88, 76)** → (93, 79) | 1 | none | 3..5 (8); 12..14 (7); 15..17 (8) | **6..11** (file 8, 8, 8, 7, 7, 7) | aggression(3..17) := FFh |
| 15 | `6AF8..6B67` | `[15]` (42) | 2 | (17, 164) → (22, 167) | 1 | 15 = 32h | 1, 2 (0); 4..6, 10..14 (8); **15 (6)** | — | aggression(4..15) := FFh; **shields(15) := 1750** |
| 16 | `6B68..6BE8` | `[16]` (42) | 2 | (17, 164) → (22, 167) | 1 | 2..4 = 32h | 2..4 (0); 6, 7 (4); 9..14 (7) | — | aggression(6..14) := FFh |
| 17 | `6BE9..6CA5` | `[17]` (42) | 3 | **(88, 76)** → (93, 79) | **100** | none | 3..7 (7) | — | **class tables restored** (§4.6.4); aggression(3..7) := FFh |
| 18 | `6CA6..6D26` | `[18]` (42) | 3 | (17, 164) → (22, 167) | 1 | **1..20 = 32h** (then 3..6 again) | 3..5 (7); 6..9 (8) | — | aggression(3..9) := FFh |
| 19 | `6D27..6D96` | `[19]` (42) | 7 | **(88, 76)** → (93, 79) | 1 | 2 = 32h | 2 (8); 3, 4 (0) | — | aggression(2) := FFh; armour(2) := 500 |

Notes:

- Empty slots (class FFh in the file): 10: 1, 2, 6, 14..20; 11/21: 1, 2, 16..20; 12: 7, 16..20; 13: 1, 2, 9, 10, 13..20; 14: 1, 2, 18..20; 15: 3, 7..9, 16..20; 16: 1, 5, 8, 15..20; 17: 1, 2, 8..20; 18: 1, 2, 10..20; 19: 1, 5..20. Per-slot header bytes A, B, D are zero in all eleven files except header byte A of slot 11 in `m18.td3` (76; the slot is empty and no mission rule reads the byte; U-038).
- `Msn_Setup` case 20 (`6D97..`) exists but is never reached.
- Home points (guidance target when no marked slot is live, `UI_DrawGuidanceLine` `0C5D:81C5..8223`, checked): flights 10, 12, 14..17, 19, 21 (22, 166); 11 (166, 78); 13 (41, 46); 18 (28, 33).
- Hazard tiles ([episode-deltas](../../re/episode-deltas.md) §4.2) hurt the player in flights 10, 11, 21, 12, 13, 15, 16 and 18, not in 14, 17 and 19 (`0C5D:D27A..D295`).

### 3.3 Paths the rules rely on

Paths the rules rely on (step *k* of slot *s* at file offset 80 + (*s* − 1) × 501 + *k* − 1 for X and 10,080 + … for Y; "loop" = the last step equals the first, so the ship patrols):

| Flight | Slot(s) | Path | Rule that uses it |
|---:|---|---|---|
| 1 | 2, 3, 4 | (85, 197 / 195 / 193) → (85, 3) → (85, 2) → (85, 1) | exit 2..6 @ (85, 1) |
| 1 | 5, 6 | (85, 160 / 162) north along x 85 to (85, 1) | exit 2..6 @ (85, 1) |
| 1 | 8 / 9 | (80, 1) → (80, 199) / (78, 1) → (78, 199) | exit 8 @ (80, 199), exit 9 @ (78, 199) |
| 1 | 11..14 | (1 / 3 / 5 / 7, 111) east along y 111 to (199, 111) | exit 11..14 @ (199, 111) |
| 2 | 2..7 | (83..85, 3..9) → (x, 197) → (x, 199) with x = 83, 85, 84, 85, 83, 84 | exit 2..12 @ (83..85, 199); outcome |
| 2 | 9, 11 / 10, 12 | south along x 83 / 85 to row 199 | exit 2..12 @ (83..85, 199) |
| 2 | 15 / 16, 17 | (79, 198) → (79, 1) / (79, 196 / 194) → (79, 2) | exit 15..17 @ (79, 2); slot 15 passes (79, 2) one step before its end and is removed there |
| 3 | 2..4 / 6, 7 | two-step paths (85, 1 / 3 / 5) → (85, 199) / (83, 2 / 4) → (83, 199) | exits; outcome |
| 3 | 9..12 | loops in four areas: x 19..82 / y 42..108; x 45..65 / y 120..156; x 81..166 / y 19..70 (starts at (166, 39), near the start); x 110..169 / y 120..146 | completion needs them destroyed |
| 4 | 5..7, 11 | (1 / 3 / 5 / 1, 111) east to (199, 111) (5..7 in two steps: (x, 111) → (195, 111) → … → (199, 111); 11 by a 357-step path) | exits; outcome (5..7) |
| 4 | 9 / 10 / 12, 13 | east along y 110 / 112 / 111 to beyond the map ((205, 110), (217, 112), (203, 111), (201, 111)) | exit 9 @ (199, 110), 10 @ (199, 112), 12, 13 @ (199, 111) |
| 4 | 2, 3 | long loops from the base area south to row 108/109 (slot 2: x 44..166, y 69..108; slot 3: x 109..178, y 56..109) | they meet the convoy's rows 109..112 on the south side of their loops |
| 5 | 2 | (172, 70) → (80, 70) → south to **(80, 201)**: the last two steps (80, 200), (80, 201) are off the map | arrival rule tests (80, 199) (U-272) |
| 5 | 3 | a 441-step loop from (166, 66) near the base down to the south edge (x 79..166, y 66..199), along the escort's route | `engage 3 → a, 20` |
| 5 | 5, 6, 7, 9 | short patrols: x 78..85 / y 66..94 (5, not closed); x 84..110 / y 70..88; x 85..166 / y 53..88; x 83..93 / y 146..182 | — |
| 5 | 10, 11 | (83, 197 / 199) north to (83, 1) | no rule |
| 6 | 2..5, 9, 10 | (160..166, 69 / 70) → (80, 69 / 70) → (80, 108) → (1, 108) | exit 2..10 @ (1, 108) |
| 6 | 6 | (170, 70) → (45, 70) → (45, 109) → **(1, 109)** → (103, 108) → (1, 108) | exit @ (1, 108) (it touches column 1 first, §6) |
| 6 | 13..17 | 13, 14: loops in x 65..80, y 85..108; 15..17: 4-tile loops in (80..83, 108..112), at the convoy's corner (80, 108) | — |
| 7 | 12, 13, 15, 16 | loops in x 87..125, y 49..111, on the convoy's route | — |
| 7 | 2, 3, 5 / 6..10 | from (184..194, 34..36) to (22, 166) / (21, 166) (slot 3 crosses (21, 166) first) | exit 2..10 @ (22, 166) and (21, 166) |
| 7 | 18 | (138, 90) → … → (1, 168) | exit 18 @ (1, 168) |
| 8 | 1, 2, 3 | round trip: from (22, 166) / (22, 168) / (24, 168) east to (102, 168), up to a stop where the steps alternate (108, 114) / (108, 115) many times, then back to (22, 166). **Slot 1 starts on (22, 166)** | waypoint test (108, 115); exit 1..3 @ (22, 166) (only in phase > 1); outcome |
| 8 | 4..10 | loops in x 44..138, y 61..177, between the base and the stop | — |
| 9 | 1, 2, 3 | (5 / 3 / 1, 94) → (102, 94) → (102, 177) → (168, 177) → (168, 199) | exit 1..6 @ (168, 199); outcome |
| 9 | 4, 5, 6 | from (7, 96), (5, 96), (11, 95), longer paths that also end at (168, 199) | exit 1..6 @ (168, 199) |
| 9 | 7 / 8, 9 | → (1, 169) / → (101, 199) | exit 7 @ (1, 169), 8, 9 @ (101, 199) |

Every exit tile used by a rule lies on the slot's waypoint walk (checked by walking each path one tile per step).

Step *k* of slot *s* is at file offset 80 + (*s* − 1) × 501 + *k* − 1 (X) and 10,080 + … (Y). A path ends before the first step with X = 0, after which the AI wraps back to step 2 ([mission-ships](mission-ships.md) §4.3: paths loop). "On the walk" = the tile lies on the one-tile-per-step waypoint walk between the steps (checked with a private walk script, numeric output only).

| Flight | Slot(s) | Path | Rule that uses it |
|---:|---|---|---|
| 10 | 3 | (22, 166) → … → (199, 106), 490 walk tiles | exit 3 @ (199, 106) (path end, east edge) |
| 10 | 4 | (22, 166) → … → (80, 1), 412 tiles | exit 4 @ (80, 1) (north edge) |
| 10 | 5 | closed loop from **(22, 166)** to a stop at (41, 57)/(41, 58) (the steps alternate between these two tiles several times around walk index 132) and back to (22, 166); 279 walk tiles; (22, 166) only at the first and the last step | exit 5 @ (22, 166) (only in phase > 38); outcome |
| 10 | 7..13 | short loops in x 38..75, y 36..168 | engagement |
| 11, 21 | 3..15 | short loops all over the map (x 43..157, y 36..177) | engagement |
| 12 | 1..6 | (102, 198 / 196 / 194 / 192 / 190 / 188) → (102, 95) → (93, 95) → north to **(93, 76)** | exit 1..6 @ (93, 76); completion; outcome |
| 12 | 8, 9 / 10, 11 / 12, 13 / 14, 15 | from (101..103, 189..195) to (1, 95) / (93, 76) / (1, 169) / (81, 1) | the matching exit rules (each tile is the path end and on the walk) |
| 13 | 11 / 12 | (150, 50) → (199, 107) / (47, 50) → (1, 18) | exit 11, exit 12 (path ends, map edges) |
| 14 | 6 / 7 / 9 / 10 | from (95, 83) / (95, 82) / (95, 79) / (89, 81), next to the start tile of the player (93, 79), to (1, 170) / (1, 168) / (1, 95) / (101, 199) | the matching exit rules |
| 14 | 8 / 11 | from (95, 80) / (85, 81) to (54, 116) / (87, 120); no exit | the tick-175 / tick-75 armour writes (§4.6.5) |
| 15 | 1 | (98, 1) → … → **(168, 199)** | exit 1 @ **(199, 168)**: not on the walk (OB-046) |
| 15 | 2 | (98, 3) → … → (199, 126) | exit 2 @ (199, 126) |
| 15 | 15; 10..14 | Kreel (15) and five class-8 ships loop in x 99..105, y 80..95 | rule A (range 55), engagement |
| 16 | 2, 3, 4 | (93, 78 / 80 / 82) → … → **(168, 199)** (225 / 223 / 221 walk tiles) | exit 2..4 @ **(199, 168)**: not on the walk (OB-046) |
| 16 | 6 / 7 | loops x 22..102, y 144..169 (starts at (22, 168)) / x 38..69, y 107..168 | the escort rules (OB-047) |
| 18 | 7 | loop x 20..32, y 33..58 starting at **(28, 33)**, the destination | — (it patrols the point) |
| 19 | 2 | Osyluth, loop x 63..87, y 96..112, from (75, 112); first within 25 tiles of the unmoved player tile (93, 79) at walk index 34, tile (69, 96) | rule A |
| 19 | 3 / 4 | (150, 152) → (102, 201), the last two steps off the map / (28, 33) → (81, 1), then steps with Y = 0, 255, 254, 253 | none: no rule reads slots 3, 4 |

### 3.4 Message resources

| Resource | Address | File offset | Count | Used by |
|---|---|---|---:|---|
| `e3.flight_messages.01` (`M1`) | `1FEB:070F..0A7E` | 0x23E8F..0x241FE | 10 | `Msn_UpdateA`: flights 1–3 post `M1[n − 1]`, flight 3 also `M1[3]`, flights 4–9 post `M1[n]` (table below) |
| `e3.flight_messages.02` | `1FEB:1684..19AC` | 0x24E04.. | 9 | `Msn_UpdateB` |
| `e3.objectives` | `0C5D:59C8..` | 0x15868.. | 21 (walked to 0C5D:6148) | `Msn_Setup`: flights 1–9 draw elements 0..8 |

The M1 run is 10 contiguous Pascal strings that end exactly at `Msn_UpdateA` (byte walk). Element addresses: 070F, 0767, 07BF, 0817, 086F, 08C6, 091E, 0975, 09CD, 0A25. No two are equal, and none mentions F6 or a transmission (byte scan).

| M1 element | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 |
|---|---|---|---|---|---|---|---|---|---|---|
| Posted by flight (rule) | 1 (C) | 2 (A) | 3 (A) | 3 (B) | 4 (A) | 5 (A) | 6 (A) | 7 (A) | 8 (A) | 9 (A) |
| Colour | 37 | 37 | 37 | 37 | 37 | 37 | 37 | 42 | 42 | 42 |

| M2 element | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
|---|---|---|---|---|---|---|---|---|---|
| Address | 1684 | 16DE | 1739 | 1794 | 17EE | 1846 | 189E | 18F9 | 1955 |
| Posted by flight (rule) | 10 (A) | 10 (D: truck destroyed) | 10 (D: truck home) | 12 (A) | 15 (A), 16 (A), 19 (A): the transmission announcement (asks for F6) | 15 (B) | 16 (B) | 18 (A) | 19 (B) |
| Colour | 42 | 42 | 42 | 42 | 42 | 42 | 42 | 42 | 42 |

Only M2[4] contains a digit (the "6" of F6), as in E2 (e2-mission-logic §4.7.1, "Text"); the digit-advance behaviour of the glyph routine ([hud](../render/hud.md) §4.7) applies to that one character exactly as in E2.

## 4. Behaviour

### 4.1 Where mission logic and the event routine run in a tick

Step numbers are the rows of [tick-order](tick-order.md) §3, as in [episode-deltas](../../re/episode-deltas.md) §4.1 (all E3 addresses `0C5D:`).

```text
step 1   tick count += 1 (BB20)
step 5   afterburner burst: flight 16 and phase >= 2 -> no new burst (BC37..BC43)            4.7.1
step 6   projectiles: a kill leaves the class byte (P3); a homing-missile hit or kill
         sets flash frames := 4 (0D22, 1237)
step 8   Msn_UpdateA (C4C3): the block of the current flight 1..9                         section 5
step 9   Msn_UpdateB (C4C8): the block of the current flight 10..19 or 21                    section 5
step 10  AI_UpdateShips (C4D1): pass 1 removes armour < 1 and exit-marker ships; pass 2 moves
step 14  palette effects: flash frames; flight 16 and phase >= 5 -> hit flash := 0
         (CC18..CC28); Gfx_HitFlashStep (CC2B)                                                4.7.1
step 15  flight 16 and phase = 2 -> comm request := 1 (CC3E..CC4C); then, if request = 1
         and draw page = 0: request := 0; UI_CommViewer (CC64)                                4.5
step 19  UI_DrawInstruments (CC7F): compass lamps, missile lamp, ticker, F6 indicator
step 24  Esc flight menu (D157..D165); re-tints in flight 16 with phase >= 5                  4.7.2
new      Msn_E3TimedEvents (D168): every tick                                                4.6
step 26  shield and armour checks (D16B..)
step 27  hazard tiles (D27A..D3CB), not in flights 14, 17, 19
step 28  F6 key -> request (D789..D797); F5 screen unless flight 16 and phase >= 5 (D79C..D7B1)
step 29  thrust: reads k_ClassMaxSpeed / k_ClassAccel of the player class (D7CC..)
step 33  leave the loop if aborted, end ticks = 35, lost or succeeded (EA1D..EA39)
teardown: outcome tests (EA52..EB99)                                                          4.8
```

Consequences:

- **Timed stores land at the end of tick *n*.** A phase store is seen by `Msn_UpdateB` in tick *n* + 1. A class store (flight 10 slot 4, flight 14 slots 6..11) makes the ship live from the AI pass of tick *n* + 1, where it first moves and is first drawn. An armour := 0 store (flight 14) removes the ship in pass 1 of the AI at tick *n* + 1. The flight-12 map patch is already seen by the hazard test (step 27) of tick *n*, and is first drawn at step 4 of tick *n* + 1. The flight-16 class patch already limits the player at step 29 of the same tick; mission ships from step 10 of tick *n* + 1.
- **Flight 16 chain in one tick:** rule A (step 9) sets phase 2 → step 15 forces the comm viewer (draw page 0) → phase 3 → the event routine (new step) fades, freezes and sets phase 5 → `Msn_UpdateB` posts M2[6] in the next tick (§5.16).
- As in E2 §4.1: kills are seen one tick late (P3), arrivals at the next step 8 or 9, and success decided at step 9 ends the loop at step 33 of the same tick. Exit rule before the completion test (same tick): flight 10 (exit 5 before D). Completion test before the exit rules (one tick later): flight 12. The exit rules of flights 15 and 16 that matter are inert (OB-046).

Where an exit rule and the "all gone" test of the same ships are in one block, their order decides whether the test sees the removal in the same tick:

| Exit rules before the completion test (same tick) | Completion test before the exit rules (one tick later) |
|---|---|
| flights 2, 3, 6, 7, 8 | flights 1, 4, 9 |

### 4.2 Shape of one flight block

As E2 §4.2, except:

- The base test is the **first rule of every block** (flights 1–9), and every base test requires phase = 100.
- No rule tests or depends on the ticker; no rule writes comm pending, markers, armour, lost or over.
- The "all gone" tests of flights 2, 3, 4, 6, 7 are computed with a scratch flag (§2.1 `allGone`); flights 1, 8, 9 test classes directly.
- Loops are `for` loops with constant inclusive bounds (bound read from the compare at each back edge). Flight 4, 6 and 7 nest two loops. No loop counts down.
- Flight 4 uses the one-test mutual form `engage a ⇄ b`.
- The base test is the first rule of every block; its form differs per flight (§4.3), and flights 11, 13, 14 and 21 do not test the phase.
- Loops are `for` loops with constant inclusive bounds (bound read from the compare at each back edge), all counting up, none nested, all on `g_MsnI`.
- Only one rule changes a class (flight 10 A: slot 5 := 0) and one changes markers (flight 18 A: 1..20 := 0). The spawns of flight 10 slot 4 and flight 14 slots 6..11 are done by the event routine.
- `engage a ⇄ b, r` (one range test, then `AI_SetTarget(a, b)`, `AI_SetTarget(b, a)`) appears in flight 10.
- Flight 16 has two rules that test a range to an escort and then order the attacker against the player (OB-047); they are written out in full.

**Spawning, hiding and removal.** As E2 §4.3, with these E3 points:

- **Spawns:** only flight 1 (A: slots 11..14 as class 7; B: slots 2..4 as class 7). The ships appear at their mission-file start tile (hidden ships are not moved), with the record values of their file class (§3.2).
- **Hide:** flight 5's arrival rule hides slot 2 (and leaves its tile on (80, 199)).
- **Exit rules** do not test the class (as E1/E2): an exit rule can fire for a hidden slot that stands on its exit tile. No flight 1–9 start tile is an exit tile of an active rule, except flight 8 slot 1, whose exit rule is only active in phase > 1.
- **"All gone"** never distinguishes destroyed, escaped and empty slots. Flights 2, 3, 4, 8, 9 separate them only through the outcome tests (§4.8).
- **No respawns** and no other class writes.
- **Timed spawns and removals** (flight 10 slot 4, flight 14 slots 6..11; flight 14 armour := 0 for slots 11 and 8) are done by the event routine (§4.6).

### 4.3 Base tests

`succeeded` is set when the condition holds and cleared otherwise, every tick, as the first rule of the block.

#### Flights 1–9

| Flight | `succeeded :=` | Phase condition | Position | Address |
|---:|---|---|---|---|
| 1, 2, 3, 4, 5 | viewX = 167 and viewY = 66 and subY ≤ 31 | phase = 100 | first | `0A97`, `0C18`, `0D58`, `0ED1`, `1093` |
| 6 | viewX = 6 and 101 ≤ viewY ≤ 114 (no sub-tile test) | phase = 100 | first | `1184` |
| 7, 8, 9 | viewX = 17 and subY ≤ 35 and viewY = 164 | phase = 100 | first | `12C0`, `13ED`, `15AA` |

All compares are signed 16-bit. The original tests subY < 32 and subY < 36. viewSubY ranges over 16..48, so "≤ 31" accepts the upper 16 offsets of the base cell and "≤ 35" the upper 20.

- Flights 1–5 start at view (167, 66) and flights 8 and 9 at (17, 164), both with subY = 16, so these flights start **inside their base cell** and end one tick after the phase reaches 100 if the player has not moved.
- Flight 6's base strip is at the west limit of the view (view X 6 is the smallest view X): the player must fly to the map's west edge in rows 101..114 (player tile (11, 104..117); home point (1, 110)).
- Flight 7 starts at (185, 32), far from its base (17, 164).
- The base test also runs during the end sequence (U-244, unchanged).

#### Flights 10–19 and 21

| Flight | `succeeded :=` | Phase condition | Address | Player tile on success |
|---:|---|---|---|---|
| 10, 12 | home17 | phase = 100 (tested last) | `19C1..19E5`, `1C49..1C6D` | (22, 167) |
| 15, 16, 17, 19 | home17 | phase = 100 (tested first) | `1EB0..1ED4`, `1F94..1FB8`, `20F1..2115`, `21EF..2213` | (22, 167) |
| 14, 21 | home17 | **none** | `1E1D..1E3A`, `1BE6..1C03` | (22, 167) |
| 11 | view ∈ {160, 161} × {75, 76} (cleared first, then four separate tests (160, 75), (161, 75), (161, 76), (160, 76)) | **none** | `1B5A..1BAA` | (165..166, 78..79); home point (166, 78) |
| 13 | view ∈ {36, 37} × {44, 45} (same form: (36, 44), (37, 44), (36, 45), (37, 45)) | **none** | `1D7A..1DC6` | (41..42, 47..48); home point (41, 46) is one row above |
| 18 | view = (23, 31), no sub-tile test | phase = 100 (tested first) | `2154..2170` | (28, 34); the home point and the F5 point (28, 33) are one row above |

All compares are signed 16-bit. viewSubY lies in 16..48, so home17 accepts the upper 20 sub-tile rows of the home cell (the same test as flights 7–9).

- Flights 10, 12, 15 and 16 start inside their base cell (view (17, 164), subY 16): an unmoved player ends them one tick after phase 100.
- Flights 14, 17 and 19 start at (88, 76), flight 21 at (160, 75), flights 11, 13 and 18 at (17, 164), all far from their base.
- **Flight 17 starts in phase 100** (setup), so its base test passes as soon as the player is back home; the flight has no other objective.
- **No phase test** in flights 11, 13, 14 and 21: reaching the base area ends them whatever the ships and the phase do. Flight 21 thus ends without its tick-10 phase change mattering.
- The base tests also run during the end sequence (U-244, unchanged).

### 4.4 Messages and transmissions

- Every post overwrites the ticker strip and restarts it (ticker := 1), whatever is scrolling, including the objective. No flight 1–9 rule waits for an idle ticker, so the M key and the jurisdiction warning delay nothing in flights 1–9.
- Each message is posted once: every posting rule also moves the phase out of its own condition.
- **No flight 1–9 rule sets comm pending**, so the F6 lamp and its sound 200 never start in flights 1–9 (comm pending is cleared at flight start, `0C5D:A9DF`). In E3 only `Msn_UpdateB` sets it (`1FEB:1F0D`, `1FF6`, `224C`).
- **No rule of flights 10–21 reads the ticker** (and none of flights 1–9). Every post overwrites the strip and restarts it (ticker := 1), even the objective text in its first ticks. The M key and the jurisdiction warning delay nothing.
- Each message is posted once: every posting rule moves the phase out of its own condition (flight 10 A/D, flight 12 A, 15 A/B, 16 A/B, 18 A, 19 A/B).
- **Comm pending** is set only by rule A of flights 15, 16 and 19 (after the post and the phase store) and cleared only by rule B of flights 15 and 19 (before the post), by the comm viewer on a match and at flight start. Unlike E2 (separate "target gone → comm pending := 0" rules every tick), the E3 clear sits inside B and fires once. No stale lamp is possible in flights 15 and 19: B always clears it when the target goes, and the viewer clears it when read.
- **Flight 16 lamp.** If A fires in a tick with draw page 0, the forced viewer runs at step 15 of the same tick and clears comm pending before the instruments (step 19): the F6 lamp never lights and sound 200 is not played. If A fires with draw page 1, the lamp is drawn at step 19 of that tick ([hud](../render/hud.md) §4.8; sound 200 when its animation phase is 0) and the viewer runs in the next tick.

### 4.5 Comm viewer (`UI_CommViewer` `0C5D:8099..8125`, step 15)

The whole E3 table, in code order:

| Code order | Flight | In phase | `UI_CommScreen(n)` | Comm text routine (episode-deltas §6) | Then phase |
|---:|---:|---:|---:|---|---:|
| 1 | 8 | 2 | 1 | `2215:0135` | 3 |
| 2 | 16 | 2 | 3 | `2215:0680` | 3 |
| 3 | 19 | 2 | 5 | `2215:093D` (`e3.comms.04`) | 3 |
| 4 | 15 | 2 | 2 | `2215:042A` | 3 |

On a match, in this order: F6 key byte DS:19C0 := 0; `UI_CommScreen(n)` (`0C5D:540E`; modal, no tick runs while it is open); phase := 3; comm pending := 0. The four tests run one after the other with no `else`, but each names another flight, so no row can chain into another. Comm pending is not tested. F6 in another phase only consumes the request. Neither `UI_CommScreen` nor the comm text unit `2215` writes the DAC or calls a fade (no palette call in `540E..568E` or in `2215`).

The request comes from the F6 key at step 28 (key byte DS:19C0 = 1 and not over, `D789..D797`) and, **in flight 16 with phase = 2 only, from step 15 itself** (`CC3E..CC4C`), every tick, without F6 and without a "not over" test. Matches episode-deltas §6 (verified; that table lists the rows in another order).

### 4.6 Per-flight event routine (`Msn_E3TimedEvents`, `td3:0C5D:959E..9A6C`)

#### 4.6.1 When it runs

One near call per tick, unconditional, at `0C5D:D168`: after the page toggle (`D144..D156`) and the Esc flight menu (`D157..D165`, tick-order steps 22–24), before the shield and armour checks (`D16B`, step 26). It therefore runs in every flight, on both pages, during the end sequence (mission over) and in a tick whose step 9 already set `succeeded` (the loop exit comes later, at step 33). A `lost` store ends the loop at step 33 of the same tick. The code ends with a near return at `9A6C`; the bytes `9A6D..9A98` are two string constants of `UI_ShipSwitchScreen`, not code. Prologue without stack check.

#### 4.6.2 State and behaviour

It keeps no state of its own. It reads the flight number, the 32-bit tick count, the phase and class(2); each tick condition is an equality, so it fires exactly once, in tick *n*. The loop counter is the global `g_I` (DS:1976), which is not live across the call.

```text
if flight = 18 and ticks = 2160:  phase := 2;  DS:1EAD := 1                (F5 point, 4.9)
if flight = 12 and ticks = 6480:  write the 27 map cells of 4.6.3
if flight = 16 and phase = 3:                                                 (no tick test)
    fade step 1: for e = 1..255: DAC(e) := half(e);     wait for retrace start
    fade step 2: for e = 1..255: DAC(e) := quarter(e);  wait for retrace start
    fade step 3: for e = 1..255: DAC(e) := (63, 0, 63); wait for retrace start
    fade step 4: for e = 1..255: DAC(e) := quarter(e);  wait for retrace start
    fade step 5: for e = 1..255: DAC(e) := half(e);     wait for retrace start
    for c = 0..8: k_ClassMaxSpeed[c] := 0
    for c = 0..8: k_ClassAccel[c] := 0
    k_ClassMaxSpeed[2] := 20;  k_ClassAccel[2] := 4
    phase := 5
if flight = 14 and ticks = 25:   for s = 6..8: class(s) := 8;  for s = 9..11: class(s) := 7
if flight = 14 and ticks = 27:   phase := 2
if flight = 14 and ticks = 75:   armour(11) := 0           (word DS:70EA)
if flight = 14 and ticks = 175:  armour(8) := 0            (word DS:646F)
if flight = 5 and phase < 100 and class(2) = FFh:  lost := 1                  (4.6.6)
if flight = 10 and ticks = 10:   phase := 2
if flight = 10 and ticks = 300:  phase := 3
if flight = 21 and ticks = 10:   phase := 2
if flight = 10 and ticks = 10:   class(4) := 2

with (R, G, B) = g_FlightPalette entry e (DS:1300 + 3e .. + 2), unsigned 16-bit arithmetic:
    half(e)    = (63 − (63 − R) div 2,  G div 2,  63 − (63 − B) div 2)
    quarter(e) = (63 − (63 − R) div 4,  G div 4,  63 − (63 − B) div 4)
DAC(e) := (r, g, b) is Gfx_SetDacEntry(e, r, g, b) (2EFA:1901); "wait for retrace start" is
Gfx_WaitRetraceStart (0C5D:0052)
```

Notes:

- **RNG:** none. The only calls are `Gfx_SetDacEntry` (5 loops of 255 calls) and `Gfx_WaitRetraceStart` (5 calls).
- Entry 0 is never written by the fade (the loops start at 1). `g_FlightPalette` is only read.
- The flight-16 branch has no tick condition. Phase 3 is only ever set in flight 16 by the comm viewer (step 15 of the same tick), so the branch runs once, in the tick the transmission is read, and its last store (phase 5) disarms it.
- The flight-10 tick-10 phase and class stores are separated by the tick-300 and flight-21 tests; the order has no effect.
- Per-routine count: 6 phase stores (flights 18, 16, 14, 10, 10, 21), 1 flag store, 27 map stores, 4 class-table stores (two loops and two single stores), 3 slot-class stores (two loops and one single store), 2 armour stores, 1 lost store.

#### 4.6.3 Flight 12: the hazard cells are switched off by setup and back on at tick 6480

Flight 12 flies `td4.map` (flight ≥ 7). The tick-6480 branch writes 27 bytes into the **live map buffer** (`g_Map`^, offset *x* × 201 + *y*). `Msn_Setup` case 12 (`0C5D:68C7..69CF`) wrote tile code **26h** (open, attribute 2, not a hazard) to **the same 27 offsets in the same order** at flight start, after the map was read. The tick-6480 values are exactly the bytes of `td4.map` at those cells (checked byte by byte). So:

- ticks 1..6480 (before the store): the 27 cells are plain tile 26h and harmless;
- from the store at the end of tick 6480 on: they are hazard tiles again, as in `td4.map` (attribute 2, open for the AI; they hurt the player through the step-27 hazard test, [episode-deltas](../../re/episode-deltas.md) §4.2, already in tick 6480); they are drawn as hazard tiles from tick 6481 and the radar shows their radar colour;
- the other 10 hazard cells of `td4.map`, (46..48, 109), (74, 77..79) and (91, 87..90), are never touched and are hazards for the whole flight.

| Code order | Cells (x, y) | Map offsets | Written at tick 6480 (= `td4.map`) | Written by setup case 12 |
|---:|---|---|---|---|
| 1–3 | (40, 121), (40, 122), (40, 123) | 1FE1..1FE3 | 44h, 45h, 46h | 26h |
| 4–6 | (40, 131), (40, 132), (40, 133) | 1FEB..1FED | 44h, 45h, 46h | 26h |
| 7–9 | (66, 128), (66, 129), (66, 130) | 3452..3454 | 44h, 45h, 46h | 26h |
| 10–12 | (66, 121), (66, 122), (66, 123) | 344B..344D | 44h, 45h, 46h | 26h |
| 13–15 | (53, 137), (54, 137), (55, 137) | 2A26, 2AEF, 2BB8 | 41h, 42h, 43h | 26h |
| 16–18 | (62, 114), (63, 114), (64, 114) | 3120, 31E9, 32B2 | 41h, 42h, 43h | 26h |
| 19–21 | (53, 114), (54, 114), (55, 114) | 2A0F, 2AD8, 2BA1 | 41h, 42h, 43h | 26h |
| 22–24 | (53, 118), (54, 118), (55, 118) | 2A13, 2ADC, 2BA5 | 41h, 42h, 43h | 26h |
| 25–27 | (46, 118), (47, 118), (48, 118) | 2494, 255D, 2626 | 41h, 42h, 43h | 26h |

The cells lie in x 40..66, y 114..137, away from the convoy route (x 93..102) and from the base; tick 6480 is long after the convoy normally arrives (its walks are 150..162 tiles). The patch matters only to a player who is still flying in that area after about 6480 ticks (roughly 5½ to 6 minutes, [timing](../core/timing.md)). Why the designers switch the cells off for the first 6480 ticks is unknown; the behaviour is CONFIRMED (code and bytes).

This corrects [episode-deltas](../../re/episode-deltas.md) §4.2 ("`td3.map` gets them from the flight-12 patch"): no E3 flight on `td3.map` (flights 1–6) ever has hazard tiles; flight 12 is on `td4.map`.

#### 4.6.4 Flight 16: magenta fade and class-table freeze (phase 3 → 5)

**Fade timeline** (all five steps run inside one call, in the tick the transmission was read; no game tick passes between them):

| Step | DAC entries | R′ | G′ | B′ | Then | Example (R, G, B) = (0, 0, 0) | Example (63, 40, 10) |
|---:|---|---|---|---|---|---|---|
| 1 | 1..255 | 63 − (63 − R) div 2 | G div 2 | 63 − (63 − B) div 2 | wait for retrace start | (32, 0, 32) | (63, 20, 37) |
| 2 | 1..255 | 63 − (63 − R) div 4 | G div 4 | 63 − (63 − B) div 4 | wait | (48, 0, 48) | (63, 10, 50) |
| 3 | 1..255 | 63 | 0 | 63 | wait | (63, 0, 63) | (63, 0, 63) |
| 4 | 1..255 | as step 2 | | | wait | (48, 0, 48) | (63, 10, 50) |
| 5 | 1..255 | as step 1 (final state: the half tint) | | | wait | (32, 0, 32) | (63, 20, 37) |

R, G, B are read from `g_FlightPalette` (`td.pal` as loaded, plus whatever `Pal_SetEntry` last stored there: the compass lamps, the missile lamp 26h and, when armour ≤ 6, entry 90h; [hud](../render/hud.md) §4.3–§4.5). Each step waits for the start of the next vertical retrace, so each step is on screen for about one video frame and the whole fade lasts about five frames (≈ 70 ms at 70 Hz) plus the DAC writing time (LIKELY). Entry 0 keeps the value it had (black in `td.pal`; see §4.7.3 for the exceptions).

**Class-table patch** (typed constants in DGROUP; the original values are the exe image, [episode-deltas](../../re/episode-deltas.md) §11):

| Class | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `k_ClassMaxSpeed` (DS:041C + c) before | 24 | 28 | 32 | 28 | 28 | 28 | 24 | 28 | 32 | 0 |
| after flight-16 phase 3 | 0 | 0 | **20** | 0 | 0 | 0 | 0 | 0 | 0 | 0 (not written) |
| `k_ClassAccel` (DS:043A + c) before | 2 | 4 | 4 | 4 | 4 | 4 | 2 | 4 | 4 | 0 |
| after flight-16 phase 3 | 0 | 0 | **4** | 0 | 0 | 0 | 0 | 0 | 0 | 0 (not written) |
| restored by `Msn_Setup` case 17 (`0C5D:6BF3..6C43`) | 24 / 2 | 28 / 4 | 32 / 4 | 28 / 4 | 28 / 4 | 28 / 4 | 24 / 2 | 28 / 4 | 32 / 4 | not written |

Armour and shield tables are not touched. In flight 16 no mission ship is class 2 (the convoy is class 0, the escorts class 4, the enemies class 7), so **every mission ship loses its acceleration and its speed limit drops to 0**, while the player (class 2) keeps acceleration 4 with a top speed of 20 instead of 32. How a moving ship and a player above the new limit behave is owned by [mission-ships](mission-ships.md) and [flight-model](flight-model.md) (U-274; expected: the ships stop and stay where they are, LIKELY). Mission ships keep turning and firing (attack mode does not depend on speed, LIKELY).

The restore in case 17 (max speed of class 0 := 24, then the loop 1..8 := 28, then classes 2, 6, 8 := 32, 24, 32; accel loop 0..8 := 4, then classes 0, 6 := 2) reproduces the original values exactly. Consequences for OB-028: §6.3.

#### 4.6.5 Flight 14 timeline

| Tick (end of) | Store | Seen |
|---:|---|---|
| 25 | class(6), class(7), class(8) := 8; class(9), class(10), class(11) := 7 (their file classes; setup hid them) | the six ships appear at their start tiles (85..95, 79..83), around the start tile of the player (93, 79), and move from the AI pass of tick 26 |
| 27 | phase := 2 | tick 28: the engagement loop (phase > 1) starts: slots 3..17 attack the player within 35 tiles |
| 75 | armour(11) := 0 | AI pass 1 of tick 76 removes slot 11 if it is live: no kill, no explosion, no sound (as E2 §5.11; mission-ships §4.2). Its path has no exit (it ends at (87, 120)) |
| 175 | armour(8) := 0 | slot 8 is removed the same way at tick 176 (path end (54, 116)) |

If the player hits slot 11 (or 8) between the store and its removal (at step 6 of tick 76 / 176), the hit follows [weapons](weapons.md) §4.3 with armour 0: it is destroyed with sound, explosions and a kill (LIKELY, U-278). The phase does not affect success in flight 14.

#### 4.6.6 Flight 5: the lost rule

```text
if flight = 5 and phase < 100 and class(2) = FFh: lost := 1
```

It runs every tick after the Esc menu, so it sees the AI pass (step 10) of the same tick: when the escort (slot 2) is destroyed or removed, the flight is lost **in the same tick** (loop exit at step 33). The arrival rule of §5.5 sets phase 100 in the same step-8 block as it hides slot 2, so an arrival never triggers it. Not owned here; listed because flight 5 cannot be understood without it.

### 4.7 Flight 16 across the flight loop

#### 4.7.1 Flight-16 cases in `Sim_FlyMission`

| Step | Address | Rule | Effect |
|---|---|---|---|
| 5 | `0C5D:BC37..BC43` | flight 16 and phase ≥ 2 → skip the burst start | No new afterburner burst from the announcement on (phases 2, 3, 5, 100). The skip goes to `BC75`: the burst key bytes are cleared (the press is consumed) and a burst already running continues (`BC7F..BC89`) |
| 14 | `0C5D:CC18..CC28` | flight 16 and phase ≥ 5 → hit flash := 0, before `Gfx_HitFlashStep` | Every block of `Gfx_HitFlashStep` (`0C5D:A55B`: `A55E`, `A6A4`, `A80B`) starts with "hit flash > 0", so it does nothing: no red hit flash (player hit, distance flash of a kill) and no end-of-flash palette restore |
| 15 | `0C5D:CC3E..CC4C` | flight 16 and phase = 2 → comm request := 1 | The transmission (screen 3) opens at the first step 15 with draw page 0, without F6 and even during the end sequence; phase 3 |
| new | `0C5D:96EE..9947` | phase = 3 → fade, freeze, phase := 5 | §4.6.4 |
| 24 | `0C5D:7EE6..7F63` (in `UI_FlightMenu`) | flight 16 and phase ≥ 5 → re-tint after the closing fade-in | §4.7.2 |
| 28 | `0C5D:D7A3..D7B1` | flight 16 and phase ≥ 5 → F5 screen not opened | No ship switch and no tactical map after the freeze (the player cannot move into a frozen ship) |

#### 4.7.2 Flight-menu tint

`UI_FlightMenu` (`0C5D:7916..7F77`) always starts with `Gfx_FadeOutFlight` (`791A`) and always leaves through the final `Gfx_FadeInFlight` (`7EE3`; one return at `7F76`, also on the abort path). Right after that fade-in, in flight 16 with phase ≥ 5, it sets **every DAC entry 0..255** to half(e) (the formula of fade step 1, read from `g_FlightPalette`), with no retrace wait. Both fades read `g_FlightPalette` (`Gfx_FadeOutFlight` copies it at `0C5D:0432..047C` in palette mode 0), so the menu fades out from the untinted colours, shows the menu, fades in to the untinted colours, and only then re-applies the tint. Unlike the event fade, the menu tint includes entry 0.

#### 4.7.3 U-203 resolved: is the tint ever removed, and what the player sees

The tint is written only to the DAC; `g_FlightPalette` keeps the untinted `td.pal` colours, and `td.pal` is read again at the next flight start, so **nothing carries over to later flights** (unlike the class tables, §6.3). Every DAC write path that is reachable in flight 16 after phase 5:

| Path | Address | Reachable after phase 5? | Effect on the tint |
|---|---|---|---|
| Hit flash (`Gfx_HitFlashStep`) | `0C5D:A55B..A91E` | runs every tick, but hit flash is 0 | none |
| White flash frames (homing-missile hit or kill, flash frames 4..1) | `0C5D:CA38..CC07` | yes: the player can still fire missiles at the frozen ships | frame 4: all 256 entries (63, 63, 63); frame 3: (63, 63, B); frame 2: (63, B, B); **frame 1: every entry := `g_FlightPalette` (palette mode 0), i.e. the untinted colours**. The tint is gone from then on, until the next Esc menu |
| Compass lamps, missile lamp, low-armour lamp (`Pal_SetEntry`) | `0C5D:871B..8784`, `9290`/`929D`, `8E18` | every tick (90h only while armour ≤ 6) | the 8 lamp entries (31h, C1h, 86h, 85h, 2Fh, 2Eh, 30h, 24h) and 26h are rewritten untinted at step 19 of every tick (90h when armour ≤ 6): the HUD lamps never show the tint after the tint tick |
| Esc flight menu (fades, help, ship library, joystick calibration) | `0C5D:7916..7F77`, `6E02..`, `7337..`, `A28E..` | yes (Esc, not over) | untinted while the menu fades; re-tinted (entries 0..255) on leaving (§4.7.2) |
| Comm viewer / comm screen | `0C5D:8099`, `540E` | the request is still accepted, but no row matches phase 5 or 100 | none (and the comm screen writes no DAC entry anyway) |
| F5 screen | `0C5D:9A99` | blocked (phase ≥ 5) | none |
| Alt (infra-red) toggle | `0C5D:E486..E607` | dead in E3 (palette mode is always 0) | none |
| Guidance home-lamp DAC write (palette mode 2) | `0C5D:8D76` | dead (palette mode 0) | none |
| End-of-flight fade-out | `0C5D:EA3C` → `0422` | once, at teardown | the fade starts from the untinted `g_FlightPalette`, so the first fade step already shows normal colours, darkened |
| The flight-16 fade itself | `0C5D:9702..98F6` | no (phase 5 disarms it) | — |

**What the player sees** (LIKELY until a capture, U-203 → capture check only):

1. The last convoy truck goes (rule A): the ticker starts M2[4] and, in the same or the next tick, comm screen 3 opens on its own.
2. When the screen closes, in the same tick: five quick frames, light magenta wash → stronger → the whole picture flat magenta (every colour index except 0, so only black pixels stay black) → stronger → light; the game then continues in the light magenta wash (half tint). The HUD lamps show their own colours again from the next tick.
3. Next tick: the ticker shows M2[6]. All mission ships stop; the player flies at most at speed 20; no afterburner, no F5, no red hit flash. The player flies back to (17, 164) to end the flight (phase 100).
4. The tint stays until the end of the flight, except: after a homing-missile hit on a ship the white flash ends in normal colours, which stay until the player opens and closes the Esc menu; after an Esc menu the tint returns and also covers colour index 0, so black areas turn dark magenta (32, 0, 32) (OB-049). The end-of-flight fade-out starts from normal colours.

### 4.8 End of flight and outcome (`0C5D:EA1D..EB99`)

Teardown order (E3): leave the loop (aborted, end ticks = 35, lost or succeeded); `Gfx_FadeOutFlight`; `g_PlayerDied` := (end ticks = 35); outcome := 1; the per-flight tests in code order; then INT 09h is restored and sound is shut down (`EB9B..`). Loop counter `g_I` (DS:1976).

```text
flight 2:  for s = 2..7: if tileY(s) = 199: outcome := 0
flight 3:  for s = 2..4: if tileY(s) = 199: outcome := 0
flight 4:  for s = 5..7: if tileX(s) = 199: outcome := 0
flight 5:  if tileY(2) ≠ 199: outcome := 0
flight 8:  outcome := 1; for s = 1..3: if tile(s) ≠ (22,166): outcome := 0
flight 9:  for s = 1..3: if tileY(s) = 199: outcome := 0
flight 10: if not (tileX(5) = 22 and tileY(5) = 166): outcome := 0            (EB4C..EB61)
flight 12: for s = 1..6: if tileY(s) = 76 and tileX(s) = 93: outcome := 0     (EB66..EB99)
```

There is **no outcome test for flights 1, 6, 7, 11, 13..19 and 21**: they always give outcome 1. The tests read tiles only (u8, kept after removal): a target destroyed **on** its exit row, column or tile counts as escaped. All rows match [episode-deltas](../../re/episode-deltas.md) §4.5 (verified).

| Flight | Meaning (LIKELY) | Campaign ([e3-program-flow](../flow/e3-program-flow.md) §4.4) |
|---:|---|---|
| 1, 6, 7 | Always 1 | — |
| 2 | 0 if any truck (2..7) reached the south edge | |
| 3 | 0 if any truck (2..4) reached the south edge | |
| 4 | 0 if any truck (5..7) reached the east edge | |
| 5 | 1 only if the escort (slot 2) reached row 199; its arrival leaves tile (80, 199). A flight lost through §4.6.6 also arrives here (E3 never tests `g_MissionLost` in the campaign, [episode-deltas](../../re/episode-deltas.md) §8.2) and gets outcome 0, because the escort was removed before row 199: block 5 then offers the retry. A kill of slot 2 on (80, 199) cannot make a lost flight with outcome 1: the arrival rule still sees it live at the next step 8 (§4.1) | |
| 8 | 1 only if all three trucks ended on (22, 166), i.e. all got home and were removed there | |
| 9 | 0 if any target truck (1..3) reached the south edge | |
| 10 | 1 only if the medical truck (slot 5) ended on (22, 166), i.e. it came home and was removed there by exit 5. A truck destroyed elsewhere gives 0; a truck destroyed exactly on (22, 166) counts as home | block 10: outcome 1 → D60; outcome 0 → D59 and a retry prompt |
| 12 | 0 if any target truck (1..6) ended on (93, 76), i.e. escaped through its exit; a truck destroyed exactly on (93, 76) counts as escaped | block 12: outcome 0 → retry prompt |
| 11, 13..19, 21 | Always 1 | — |

Verified against episode-deltas §4.5 (E3 row: "Flight 10: 0 unless slot 5 is at (22, 166). Flight 12: 0 if any slot 1..6 is at (93, 76)"): both rows match, and there are no other tests for flights 10–21. `g_MissionLost` is never set in flights 10–21 (its only E3 store is the flight-5 rule, `0C5D:99FD`), so these flights end only by success, death or abort.

### 4.9 U-202 resolved: E3 DS:1EAD is the F5 map point of flight 18

DS:1EAD is the **flag of entry 21 of the F5 target-information table** (DS:1E98 + 21). `Msn_LoadTargetInfo` (`1BD8:0DCF`, called once from `Msn_Setup`) fills entry 21, the fixed map point of the flight, for every flight. For flights 10–19 and 21 (block starts found by their flight compares):

| Flight | Block | Flag (DS:1EAD) | Point (X DS:1ECD, Y DS:1EED) |
|---:|---|---:|---|
| 10, 11, 21, 12, 14, 15, 16, 17, 19 | `1BD8:2614`, `294D`, `2B88`, `2D55`, `312A`, `32CB`, `353E`, `3810`, `3B26` | 1 | (22, 166), the T.D. base |
| 13 | `1BD8:2F69` | 1 | (41, 46), the destination (= home point) |
| **18** | `1BD8:3985` (stores at `398E`, `3993`, `3998`) | **0** | **(28, 33)**, the destination (= home point) |

The only reader of the flags is the F5 screen (`UI_ShipSwitchScreen` `0C5D:9A99`): it rewrites entries 0 (player) and 1..20 (slots: flag := class ≠ FFh) every time it opens, leaves entry 21 alone, and its cursor (DS:31BD) steps only through entries 0..30 whose flag is 1 (`9F7E..9FFB`); the selected entry is framed at its map position and its five information lines are shown (presentation LIKELY; [flow/flight-screens](../flow/flight-screens.md) owns it). So:

- **Flight 18, ticks 1..2160:** the point (28, 33) is not offered on the F5 map (flag 0).
- **End of tick 2160:** `Msn_E3TimedEvents` sets the flag to 1 (and phase := 2). From then on the F5 cursor can select the point (28, 33) and show its lines: the "coordinates" have been transmitted.
- **Tick 2161, rule A:** M2[7] sends the player to the map point; markers 1..20 are cleared, so the guidance line and the blue compass lamp point to the home point (28, 33); phase 100. The flight ends at view (23, 31) (player tile (28, 34)).

DS:1EAD has no simulation effect. The flag store in the event routine is unconditional on anything but flight and tick. U-202 is RESOLVED (static; the F5 drawing detail is LIKELY).

Flights 8, 10 and 11/21 also have a second fixed point, entry 22 (flag DS:1EAE): (108, 115), (41, 58) and (165, 77) ([e3-mission-setup](e3-mission-setup.md) §3.4). In flight 11 entry 21 is the base (22, 166) and entry 22 lies next to the destination (home point (166, 78)). Flight 9 stores entry 21's point and lines but no flag (OB-054).

## 5. Per-flight logic

Each section lists the block in original order. In flights 1–9 messages use colour 37 unless a step says `c = 42`; in flights 10–21 every post uses colour 42.

| Flight | Phases used | Transitions |
|---:|---|---|
| 1 | 1, 3, 5, 100 | 1 → 3: slot 8 **or** 9 gone (A, spawns 11..14); 3 → 5: slot 11 **or** 12 gone (B, spawns 2..4); 5 → 100: slots 8, 2, 11, 12 all gone (C) |
| 2 | 1, 100 | trucks 2..7 all gone |
| 3 | 1, 2, 100 | 1 → 2: trucks 2..4 all gone (A); 1 or 2 → 100: Vultures 9..12 all gone (B) |
| 4 | 1, 100 | trucks 5..7 all gone |
| 5 | 1, 100 | escort slot 2 on (80, 199) (A). Lost, with outcome 0, if slot 2 is gone first (§4.6.6) |
| 6 | 1, 100 | convoy 2..10 all gone |
| 7 | inherited (normally 100), 100 | convoy 2..10 all gone, only if the inherited phase is < 100 (OB-043) |
| 8 | 1, 2, 3, 4, 100 | 1 → 2: player within 15 of a live slot 4..10; 2 → 3: F6 (comm screen 1); 1, 2 or 3 → 4: any of 4..10 gone, or a truck's waypoint is (108, 115); any → 100: trucks 1..3 all gone |
| 9 | 1, 100 | trucks 1..3 all gone |
| 10 | 1, 2, 3, 4, 5..40, 100 | 1 → 2: @tick 10; 2 → 3: @tick 300; 3 → 4 → 5: A and B in tick 301 (truck released); 5..40: B, one per tick; 13..99 → 100: D (truck gone) |
| 11 | 1 | never changes; success is positional |
| 21 | 1, 2 | 1 → 2: @tick 10 (enables the attacks); success is positional |
| 12 | 1, 100 | trucks 1..6 all gone (A) |
| 13 | 1 | never changes; success is positional |
| 14 | 1, 2 | 1 → 2: @tick 27 (enables the attacks); success is positional |
| 15 | 1, 2, 3, 100 | 1 → 2: Kreel within 55 (A); 2 → 3: F6 (comm screen 2); 1..3 → 100: Kreel gone (B) |
| 16 | 1, 2, 3, 5, 100 | 1 → 2: trucks 2..4 gone (A); 2 → 3: forced comm screen 3 (step 15); 3 → 5: event routine (fade, freeze); 5 → 100: B |
| 17 | 100 | set by setup; never changes |
| 18 | 1, 2, 100 | 1 → 2: @tick 2160; 2 → 100: A in the next tick |
| 19 | 1, 2, 3, 100 | 1 → 2: Osyluth within 25 (A); 2 → 3: F6 (comm screen 5); 1..3 → 100: Osyluth gone (B) |

### 5.1 Flight 1 (`1FEB:0A8D..0C0D`)

Start: §3.3. Two class-8 ships (8, 9) cross the map from the north edge to the south edge on columns 80 and 78; two unmarked class-7 ships (5, 6) fly north on column 85. When the first of 8, 9 is gone, four class-7 ships (11..14) enter on row 111 from the west edge and fly to the east edge; when the first of 11, 12 is gone, three class-7 ships (2..4) enter on column 85 near the south edge and fly north. Markers 32h: 2..4, 8, 9, 11..14.

```text
succeeded := view = (167,66) and subY ≤ 31 and phase = 100
A: if (dead(8) or dead(9)) and phase < 3:
       phase := 3; spawn 11 as 7; spawn 12 as 7; spawn 13 as 7; spawn 14 as 7
B: if (dead(11) or dead(12)) and phase = 3:
       phase := 5; spawn 2 as 7; spawn 3 as 7; spawn 4 as 7
C: if phase > 2 and dead(8, 2, 11, 12) and phase < 100: phase := 100; post(M1[0])
for s = 5..14: engage s → player, 15
for s = 2..6:  exit s @ (85,1)
exit 8 @ (80,199)                      (written as a loop 8..8)
exit 9 @ (78,199)                      (loop 9..9)
for s = 11..14: exit s @ (199,111)
```

Notes:

- **What completes it:** slots 8, 2, 11 and 12 gone, in phase 5 (C). In phase 3 slot 2 is still hidden, but C cannot fire there: whenever 11 or 12 is gone in phase 3, B (which precedes C) spawns slot 2 first. Slots 3, 4, 9, 13 and 14 are marked targets but are not tested (OB-045).
- **Nothing has to be destroyed.** Every tested ship leaves through an exit rule, and leaving counts as gone. With no input the waves follow one another and the flight completes by itself; because the start view is inside the base cell, `succeeded` follows one tick after C (LIKELY; how long it takes depends on the ships' speeds).
- A, B, C precede the exit rules, so a wave advances one tick after the exit that removes its trigger ship (§4.1).
- B gives slots 2..4 class 7 although the file has class 0 there; they keep the class-0 armour and shields of setup (§3.2).
- Slots 7 and 10 are empty; their rules are inert. The player rules are the only engagements: no ship-against-ship rule.
- Outcome = 1. No lost rule.

### 5.2 Flight 2 (`1FEB:0C0E..0D4D`)

Start: §3.3. Six class-0 trucks (2..7, markers 32h) drive south on columns 83..85 from the north edge to row 199, escorted by four class-6 ships (9..12) on the same columns. Three class-7 ships (15..17) come north on column 79.

```text
succeeded := view = (167,66) and subY ≤ 31 and phase = 100
for s = 9..12:  engage s → player, 10
for s = 15..17: engage s → player, 20
for s = 15..17: exit s @ (79,2)
for s = 2..12:  exit s @ (83,199);  exit s @ (84,199);  exit s @ (85,199)
A: if allGone(2..7) and phase < 100: phase := 100; post(M1[1])
```

Notes:

- **Completes** when all six trucks are gone, destroyed or escaped (exits run first, so in the same tick as the last exit).
- **Fails the outcome** (§4.8) if any truck reached row 199 (it is removed there by the exit loop and its tile stays on row 199).
- Slot 15's path ends at (79, 1), but it passes (79, 2) first, so its exit rule removes it there (contrast E2 OB-036).
- Slot 8 is empty; its exit rules are inert.
- No ship-against-ship rule.

### 5.3 Flight 3 (`1FEB:0D4E..0EC6`)

Start: §3.3. Three class-0 trucks (2..4) drive straight south on column 85 from the north edge, with two class-6 escorts (6, 7) on column 83. Four class-7 Vultures (9..12) patrol closed loops in four areas of the map (§3.3). Markers 32h: 2..4, 9..12.

```text
succeeded := view = (167,66) and subY ≤ 31 and phase = 100
for s = 9..14: engage s → player, 15
for s = 6..7:  engage s → player, 5
for s = 2..4:  exit s @ (85,199)
for s = 6..7:  exit s @ (83,199)
A: if allGone(2..4) and phase < 2:   phase := 2;   post(M1[2])
B: if allGone(9..12) and phase < 100: phase := 100; post(M1[3])
```

Notes:

- **Completes** only when the four Vultures 9..12 are gone. Their paths are closed loops with no exit, so they must be destroyed. The trucks decide only the outcome: 0 if any of them reached row 199.
- A fires only from phase 1. If the Vultures are gone first, `M1[2]` is never posted.
- A and B can fire in the same tick (A then B; `M1[3]` overwrites `M1[2]`).
- Slots 13 and 14 are empty. Slot 11's loop (x 81..166, y 19..70) starts at (166, 39), close to the start: at tick 1 it is 30 rows from the player tile (172, 69), the nearest ship to the player in any flight 1–9 at tick 1.

### 5.4 Flight 4 (`1FEB:0EC7..1088`)

Start: §3.3; player class 4. Three class-0 trucks (5..7, markers 32h) drive east along row 111 from the west edge to (199, 111), escorted by 9..13 (class 8, and 12 class 6) on rows 110..112. Two unmarked class-4 ships (2, 3) patrol long loops from the base area south to rows 108/109.

```text
succeeded := view = (167,66) and subY ≤ 31 and phase = 100
for a = 2..3:
    for b = 9..13: engage a ⇄ b, 15
    for b = 5..7:  engage a → b, 20
for s = 9..11: engage s → player, 20
for s = 12..13: engage s → player, 10
A: if allGone(5..7) and phase < 100: phase := 100; post(M1[4])
for s = 5..7:   exit s @ (199,111)
for s = 11..13: exit s @ (199,111)
exit 9 @ (199,110)
exit 10 @ (199,112)
```

Notes:

- **Completes** when the three trucks are gone; A precedes the exits, so one tick after the last exit.
- **Fails the outcome** if any truck has tile X = 199 at teardown, whether the exit rule or the map-edge rule removed it.
- The friendly ships 2 and 3 fight the escorts and attack the trucks (the player's targets). Their loops reach rows 108/109, within range of the convoy on rows 109..112, so they can destroy trucks on their own, which completes the flight without an outcome penalty (LIKELY).
- Target order: for slots 9..13 the player rules come after `engage a ⇄ b`, so the player wins as the target when in range. For slots 2 and 3, the trucks (range 20) come after the escorts.
- The escort paths of 9, 10, 12 and 13 continue past column 199; their exit rules catch them in column 199 on their own rows, otherwise the map-edge rule removes them.

### 5.5 Flight 5 (`1FEB:1089..1179`)

Start: §3.3. Escort mission: slot 2 (class 0 as in the file, armour 900, marker 39h) flies from (172, 70), just below the start tile, west along row 70 and then south on column 80 to the south edge. Slot 3 (class 2 from setup, file class 4, unmarked) patrols a long loop from the base down the same corridor to the south edge. Slots 5..7 (class 7, markers 32h) and 9 (class 7) patrol short paths on the escort's route; 10 and 11 (class 6) fly north on column 83, next to it.

```text
succeeded := view = (167,66) and subY ≤ 31 and phase = 100
A: if tile(2) = (80,199) and class(2) ≠ FFh:
       hide 2; post(M1[5]); phase := 100
for a = 5..11:
    engage a → 3, 15
    engage a → player, 15
    engage a → 2, 10
    engage 3 → a, 20
```

Notes:

- **Completes** when slot 2 stands on (80, 199) at step 8 (the tick after it arrives). Row 199 has no map-edge removal, so it waits there until A hides it (U-272: its path continues to two off-map steps).
- **Lost** when slot 2 is gone in phase < 100 (§4.6.6), in the same tick as its removal.
- Target order: for a = 5..11 the last successful call wins, so slot 2 (range 10) beats the player (15), which beats slot 3 (15). Slot 3's final target is the highest-numbered attacker within 20.
- Slot 8 is empty; 10 and 11 have no exit rule (they reach row 1 and are removed by the map-edge rule or loop back).
- Outcome: 1 when slot 2 arrived (tile Y 199). A lost flight 5 ends the loop with outcome 0 (§4.8), which is how the E3 campaign sees the failure.

### 5.6 Flight 6 (`1FEB:117A..12B5`)

Start: §3.3. A friendly convoy (2..5 class 4, 6 class 2, 9 and 10 class 0; markers 39h; 7 and 8 empty) leaves the base area west along rows 69/70, turns south at column 80 and leaves at the west edge (1, 108). Enemies: 13, 14 (class 8) patrol in x 65..80, y 85..108; 15..17 (class 6) circle at the convoy's corner (80..83, 108..112).

```text
succeeded := viewX = 6 and 101 ≤ viewY ≤ 114 and phase = 100
for s = 2..10: exit s @ (1,108)
A: if allGone(2..10) and phase < 100: phase := 100; post(M1[6])
for a = 13..17:
    for b = 2..10: engage a → b, 5
    for b = 6..7:  engage b → a, 15
    engage a → player, 7
```

Notes:

- **Completes** when the whole convoy is gone, arrived or destroyed (exits run first, so in the same tick as the last exit). There is no outcome test: a destroyed convoy completes the flight as well as a delivered one.
- The player then has to reach the west map limit (§4.3).
- Slot 6 (class 2) is the only convoy ship that fights back (`engage 6 → a, 15`; slot 7 is empty). Its own route touches column 1 at (1, 109) before it turns back east; if the map-edge rule removes it there, it simply counts as gone (§6).
- The enemies attack convoy ships only within 5 tiles and the player within 7; the player rule comes last, so the player wins when in range.
- Outcome = 1.

### 5.7 Flight 7 (`1FEB:12B6..13E2`)

Start: §3.3; map `td4.map`; player class 2 with armour 5. A friendly convoy (2, 3, 5 class 4; 6, 7 class 2; 8 class 7; 9, 10 class 0; markers 39h; 4 empty) starts around the player (184..194, 34..36) and travels a long route to the home base (22, 166) / (21, 166). Enemies: 12, 13 (class 8) and 15, 16 (class 6) patrol on the convoy's route in x 87..125, y 49..111; 18 (class 6) flies from (138, 90) to the west edge (1, 168).

```text
succeeded := viewX = 17 and subY ≤ 35 and viewY = 164 and phase = 100
for s = 2..10: exit s @ (22,166);  exit s @ (21,166)
exit 18 @ (1,168)
for a = 12..18:
    for b = 2..10: engage a → b, 15
    engage a → player, 15
A: flag := 1; for s = 2..10: if class(s) ≠ FFh: flag := 0
   if phase > 99: flag := 0
   if flag = 1 and phase < 100: phase := 100; post(M1[7], c = 42)
```

Notes:

- **The phase is inherited (OB-043).** Setup case 7 has no phase store, and nothing else writes the phase between flights (§9). Block 6 flies flight 6 immediately before flight 7, and flight 6 can only succeed in phase 100. So in normal play flight 7 starts in phase 100:
  - the base test passes as soon as the player is back at (17, 164) with subY ≤ 35, whatever the convoy does;
  - A never fires (the flag is cleared by `phase > 99`), so `M1[7]` is never posted.
- If the inherited phase is below 100 (U-270), the flight behaves as designed: A fires in the same tick as the last convoy exit (exits run first), and the player then returns to (17, 164).
- The `phase > 99` test duplicates `phase < 100`.
- The player rule comes after the convoy targets, so the player wins when within 15.
- Outcome = 1 (no outcome test), so the convoy's fate never matters to the campaign.

### 5.8 Flight 8 (`1FEB:13E3..159F`)

Start: §3.3; player class 6; objective colour 42. Three class-0 trucks (1..3, armour 100, markers 39h) start at the home base (slot 1 on (22, 166) itself), drive to a stop at (108, 114..115), wait there (their path alternates between the two tiles), and drive back to (22, 166). Seven unmarked enemies (4, 7 class 7; 5, 6, 8..10 class 8; aggression FFh) patrol between the base and the stop. Comm viewer: phase 2 → screen 1, phase 3.

```text
succeeded := viewX = 17 and subY ≤ 35 and viewY = 164 and phase = 100
if phase > 1: for s = 1..3: exit s @ (22,166)
A: if dead(1, 2, 3) and phase < 100: phase := 100; post(M1[8], c = 42)
B: if phase = 1: for s = 4..10: if Msn_NearLive(s, player, 15): phase := 2
C: if phase < 4: for s = 4..10: if class(s) = FFh: phase := 4
if phase > 3:
    for a = 4..10:
        engage a → 1, 15;  engage a → 2, 15;  engage a → 3, 15
        engage a → player, 10
D: if phase < 4: for s = 1..3: if wp(s) = (108,115): phase := 4
```

Notes:

- **Phases 1–3: the enemies only patrol.** No rule orders an attack before phase 4, and the trucks' exit rules are off in phase 1 (slot 1 starts on its exit tile).
- **Phase 4 (the attack)** starts when the player kills (or anything removes) any of 4..10 (C), or when a truck's waypoint becomes (108, 115), i.e. when the convoy reaches its stop (D). D is the last rule, so the attack rules start in the next tick. D reads the ship record directly and has no class test.
- **Phase 2 is the transmission window, and it is silent (OB-044).** B moves to phase 2 when the player comes closer than 15 tiles (on both axes) to a live enemy in phase 1. Nothing sets comm pending and no message announces it. F6 in phase 2 shows screen 1 and sets phase 3. Phase 3 behaves like phase 2 for the rules (the exits are on, C and D still lead to 4).
- **Completes** when all three trucks are gone (A), in the same tick as the last exit. The **outcome** is 1 only if all three ended on (22, 166). A truck destroyed anywhere else makes the outcome 0 (block 8: retry prompt; a decline continues as a success, [episode-deltas](../../re/episode-deltas.md) §8.2).
- Since A precedes B, C and D, a phase of 100 is never lowered: C and D need phase < 4, B phase 1.
- From phase 4 on, including phase 100, the enemies keep attacking the player within 10 tiles.
- The setup hide of 11..20 has no effect (those slots are empty in `m8.td3`).

### 5.9 Flight 9 (`1FEB:15A0..167F`)

Start: §3.3; player class 6; objective colour 42. Three class-0 target trucks (1..3, armour 100, markers 32h) start at the west edge (1..5, 94) and drive east, south and east to the south edge at (168, 199). Escorts 4, 5 (class 8) and 6 (class 7) start next to them and also end at (168, 199). 7 (class 7) leaves at (1, 169). 8, 9 (class 8) leave at (101, 199). All six have aggression FFh.

```text
succeeded := viewX = 17 and subY ≤ 35 and viewY = 164 and phase = 100
A: if dead(1, 2, 3) and phase = 1: phase := 100; post(M1[9], c = 42)
for s = 1..6: exit s @ (168,199)
exit 7 @ (1,169)
exit 8 @ (101,199)
exit 9 @ (101,199)
for s = 4..9: engage s → player, 20
```

Notes:

- **Completes** when the three trucks are gone. A precedes the exits, so when the last truck arrives the exit removes it at the next step 8 and A fires one tick later (as E2 flight 9, E2 §7 vector 12). The start view is inside the base cell, so an unmoved player ends the flight one tick after that.
- **Fails the outcome** if any truck reached row 199 (escaped or destroyed there).
- The setup hide of 10..20 has no effect (empty slots).

### 5.10 Flight 10 (`1FEB:19B7..1B4F`): the medical truck

Start: §3.2. Slot 3 (class 2) leaves the home hangar (22, 166) at once for the east edge; slot 4 (class 2, hidden) is released at tick 10 for the north edge; both have no marker. Slot 5 is the medical truck (class 0, marker 39h, armour 100, hidden), released at tick 300 for a round trip from (22, 166) to a stop at (41, 57..58) and back. Slots 7..13 (class 7/8; 7..10 marked 32h) patrol on the way.

```text
succeeded := home17 and phase = 100
if phase > 1:
    for a = 7..13:
        engage a ⇄ 3, 40
        engage a ⇄ 4, 20
        engage a → player, 40
        engage a → 5, 20
A: if phase = 3: spawn 5 as 0; post(M2[0], c = 42); phase := 4
exit 3 @ (199,106)
exit 4 @ (80,1)
B: if phase > 3 and phase < 40: phase := phase + 1
C: if phase > 38: exit 5 @ (22,166)
D: if dead(5) and phase > 12 and phase < 100:
       phase := 100
       if tile(5) = (22,166): post(M2[2], c = 42)      (truck home)
       else:                  post(M2[1], c = 42)      (truck destroyed)
```

Timed events: @tick 10: phase := 2 and class(4) := 2; @tick 300: phase := 3 (§4.6.2).

Timeline with no input (tick *t* = value of the tick count):

| Tick | Event |
|---:|---|
| 1..10 | phase 1: no engagement; slot 3 leaves |
| 10 (end) | phase 2; slot 4 appears at (22, 166) |
| 11..300 | engagement loop active |
| 300 (end) | phase 3 |
| 301 | A: truck spawned at (22, 166), M2[0], phase 4; B: phase 5 |
| 302..336 | B: phase = *t* − 296 (6..40); stays 40 from tick 336 |
| ≥ 309 | D can fire (phase ≥ 13) |
| ≥ 335 | C active (phase ≥ 39): exit 5 removes the truck when it stands on (22, 166) |

Notes:

- **The phase is a counter.** Phases 4..40 are a 36-tick delay that keeps the home exit closed while the truck is still on its start tile (22, 166), which is also its exit tile, and keeps D quiet for the first 8 ticks after the spawn. The truck needs only a few ticks to leave its start tile (LIKELY, U-277).
- **What completes it:** the truck gone, either removed at home by C (then D posts M2[2] in the same block) or destroyed (D posts M2[1] one tick after the kill is seen, but not before tick 309). The player then returns to (17, 164). Nothing else is required; slots 3 and 4 may die or leave.
- **Outcome** (§4.8): 1 only if the truck ended on (22, 166). A truck destroyed while the phase is 5..12 completes the flight at tick 309.
- Target order for a = 7..13: the truck (range 20) beats the player (40), which beats slots 4 and 3. Slots 3 and 4 end with the highest-numbered attacker in range. The truck never fights back.
- `Sim_FlyMission` draws the overlay cells (21..23, 165..166) and (41..42, 57..58) over ships in flight 10 ([episode-deltas](../../re/episode-deltas.md) §4.4), so the truck disappears under the buildings at both ends of its trip.
- Comm: none. Slots 11..13 are unmarked.

### 5.11 Flight 11 (`1FEB:1B50..1BDE`): outbound trip to (165, 78)

Start: §3.2. Thirteen ships (3..15: classes 7, 8 and 3) patrol short loops all over the map; no markers; the guidance line points to the home point (166, 78).

```text
succeeded := 0
if view = (160,75): succeeded := 1
if view = (161,75): succeeded := 1
if view = (161,76): succeeded := 1
if view = (160,76): succeeded := 1
for a = 3..15: engage a → player, 40
```

Notes:

- **No phase script and no phase test:** the flight ends at the first tick whose step 9 finds the view in {160, 161} × {75, 76} (any sub-tile offset). The phase stays 1.
- The only rule is the attack order, with the large range 40 on both axes. At tick 1 slot 3 at (50, 150) is already in range of the player tile (22, 167) (|dx| 28, |dy| 17).
- Slots 14 and 15 (class 3) are attackers like the others.
- Outcome = 1. In block 11 flight 21 follows directly (episode-deltas §8.1); it starts at view (160, 75), inside this base area.

### 5.21 Flight 21 (`1FEB:1BDF..1C3E`): return trip

Start: §3.2 (same roster and paths as flight 11, `m21.td3` = `m11.td3`), start view (160, 75).

```text
succeeded := home17                               (no phase test)
if phase = 2: for a = 3..15: engage a → player, 40
```

Timed event: @tick 10: phase := 2.

Notes:

- The ships do not attack during ticks 1..10 (phase 1); from tick 11 every ship within 40 tiles on both axes is ordered against the player every tick. Slots 7 (150, 50) and 11 (147, 100) start within 40 tiles of the start tile (165, 78), so they are the first candidates at tick 11.
- The flight ends when the player is back home; the phase does not matter. Outcome = 1.

### 5.12 Flight 12 (`1FEB:1C3F..1D6F`): six target trucks heading north

Start: §3.2; player class 2 with 10 missiles. Six class-0 trucks (1..6, markers 32h) drive north from (102, 188..198) to their exit (93, 76). Eight class-8 escorts (8..15, aggression C8h) start beside them and leave at (1, 95), (93, 76), (1, 169) and (81, 1).

```text
succeeded := home17 and phase = 100
A: if phase < 100 and dead(1, 2, 3, 4, 5, 6): phase := 100; post(M2[3], c = 42)
for a = 8..15: engage a → player, 40
for s = 1..6: exit s @ (93,76)
exit 8 @ (1,95);    exit 9 @ (1,95);    exit 10 @ (93,76);  exit 11 @ (93,76)
exit 12 @ (1,169);  exit 13 @ (1,169);  exit 14 @ (81,1);   exit 15 @ (81,1)
```

Notes:

- **Completes** when all six trucks are gone, destroyed or escaped. A precedes the exits, so after the last exit A fires one tick later.
- **Outcome** 0 if any truck escaped through (93, 76) (block 12: retry prompt). The player must destroy all six before they reach (93, 76).
- The escorts attack only the player (range 40); nothing attacks the trucks.
- Event routine: the hazard cells of §4.6.3 are open tile 26h until the end of tick 6480.
- Slot 7 is empty; slots 16..20 have no rules.

### 5.13 Flight 13 (`1FEB:1D70..1E12`): trip to (41, 46)

Start: §3.2; player class 7; no markers; the guidance line points to (41, 46). Slots 3..8 (classes 7 and 3) patrol; slots 11 and 12 are unmarked class-0 trucks that leave at (199, 107) and (1, 18).

```text
succeeded := 0
if view = (36,44): succeeded := 1
if view = (37,44): succeeded := 1
if view = (36,45): succeeded := 1
if view = (37,45): succeeded := 1
for a = 3..8: engage a → player, 20
exit 11 @ (199,107)
exit 12 @ (1,18)
```

Notes: no phase script; the flight ends when the view reaches {36, 37} × {44, 45}. The trucks have no role. Outcome = 1.

### 5.14 Flight 14 (`1FEB:1E13..1EA5`): ambush on the way home

Start: §3.2; player class 3, start view (88, 76) (player tile (93, 79)); no markers. Slots 3..5 and 12..17 patrol; slots 6..11 are hidden by setup and start right next to the player.

```text
succeeded := home17                               (no phase test)
if phase > 1: for a = 3..17: engage a → player, 35
exit 6 @ (1,170);  exit 7 @ (1,168);  exit 9 @ (1,95);  exit 10 @ (101,199)
```

Timed events (§4.6.5): @tick 25: slots 6..8 := class 8, 9..11 := class 7; @tick 27: phase := 2; @tick 75: armour(11) := 0; @tick 175: armour(8) := 0.

Notes:

- No ship attacks before tick 28. At tick 25 six ships appear around the player; from tick 28 every live ship within 35 tiles is ordered against the player every tick.
- Slots 11 and 8 have no exit; the script removes them silently at ticks 76 and 176 respectively (AI pass 1, armour < 1).
- The flight ends when the player is back home, whatever the phase. Hazard tiles do not hurt in flight 14. Outcome = 1.

### 5.15 Flight 15 (`1FEB:1EA6..1F89`): Kreel

Start: §3.2; player class 2. Kreel is slot 15 (class 6, marker 32h, shields 1750) circling in x 99..105, y 80..95 with five class-8 ships (10..14); slots 4..6 (class 8) patrol near the base; trucks 1 and 2 (class 0, unmarked) cross the map. Comm viewer: phase 2 → screen 2, phase 3.

```text
succeeded := phase = 100 and home17
A: if Msn_NearLive(15, player, 55) and phase = 1:
       post(M2[4], c = 42); phase := 2; comm pending := 1
B: if dead(15) and phase < 100:
       comm pending := 0; post(M2[5], c = 42); phase := 100
for a = 4..15: engage a → player, 15
exit 1 @ (199,168)                                (inert: slot 1 ends at (168,199), OB-046)
exit 2 @ (199,126)
```

Notes:

- The announcement needs the player within 55 tiles of a live Kreel on both axes; it is posted once and does not wait for the ticker. The transmission (F6 in phase 2) is optional.
- **What completes it:** Kreel gone (destroyed; it has no exit), in phase 1, 2 or 3. If Kreel dies in phase 1 the announcement never appears.
- Kreel shields 1750 last one hit only (OB-048).
- Outcome = 1.

### 5.16 Flight 16 (`1FEB:1F8A..20E9`): convoy, forced transmission and freeze

Start: §3.2; player class 2. Trucks 2..4 (class 0, markers 32h) drive from the city centre (93, 78..82) to (168, 199). Escorts 6 and 7 (class 4, unmarked) patrol near the base; enemies 9..14 (class 7, aggression FFh) patrol in the city. Comm viewer: phase 2 → screen 3, phase 3 (forced, §4.5).

```text
succeeded := phase = 100 and home17
A: if dead(2, 3, 4) and phase = 1:
       post(M2[4], c = 42); phase := 2; comm pending := 1
B: if phase = 5: post(M2[6], c = 42); phase := 100
for a = 9..14:
    if Msn_NearLive(a, 6, 10): AI_SetTarget(a, player)          (OB-047)
    if Msn_NearLive(a, 7, 10): AI_SetTarget(a, player)          (OB-047)
    engage 6 → a, 15
    engage 7 → a, 15
    engage a → player, 15
exit 2 @ (199,168);  exit 3 @ (199,168);  exit 4 @ (199,168)    (inert, OB-046)
```

Sequence (draw page 0 in the tick of A; with draw page 1 everything from the comm screen on moves one tick later):

| When | What |
|---|---|
| tick *t*, step 9 | A: M2[4], phase 2, comm pending 1 |
| tick *t*, step 15 | forced request → comm screen 3 (modal) → phase 3, comm pending 0 |
| tick *t*, new step | fade (5 frames), class patch, phase 5 (§4.6.4) |
| tick *t* + 1, step 9 | B: M2[6], phase 100 |
| from tick *t* + 1 | hit flash suppressed, F5 blocked; afterburner already blocked since phase 2 |
| later | the player flies home (class 2, top speed 20); base test → success |

Notes:

- **What completes it:** all three trucks gone. Their exit tile is transposed (OB-046), so they cannot leave; at (168, 199) they turn back along their paths (LIKELY, U-276). In practice the player must destroy all three. The rest follows automatically; the transmission cannot be skipped.
- **Enemies never attack the escorts.** For an enemy *a*, the last successful order wins: the player whenever *a* is within 15 of the player, or within 10 of a live escort (OB-047). The escorts attack the highest-numbered enemy within 15.
- After phase 5 every mission ship is frozen (§4.6.4); they keep turning and firing at the player when ordered (LIKELY).
- Outcome = 1 (no test).

### 5.17 Flight 17 (`1FEB:20EA..2149`): home run

Start: §3.2; player class 3, start view (88, 76); **phase 100 from setup**; setup restores the class tables (§4.6.4).

```text
succeeded := phase = 100 and home17
for a = 3..7: engage a → player, 15
```

Notes: the flight ends as soon as the player is back at (17, 164) with subY ≤ 35. Slots 3..7 (class 7) patrol in x 38..87, y 112..156, between the start and the base; none is within 15 of the start tile at tick 1. No hazard damage in flight 17. Outcome = 1.

### 5.18 Flight 18 (`1FEB:214A..21E4`): timed wait for the map point

Start: §3.2; player class 3. All twenty markers are 32h; the guidance line therefore points to the lowest live slot (slot 3) and the compass lamps count all seven live ships (3..9). Slot 7 patrols the destination (28, 33).

```text
succeeded := phase = 100 and view = (23,31)        (no sub-tile test)
for a = 3..9: engage a → player, 35
A: if phase = 2:
       post(M2[7], c = 42)
       for s = 1..20: marker(s) := 0              (the home marker, element 21, stays 3Dh)
       phase := 100
```

Timed event: @tick 2160: phase := 2 and DS:1EAD := 1 (§4.9).

Notes:

- **A pure timer:** nothing the player does changes the phase. For 2160 ticks (about 2 minutes) the player survives the attacks (range 35; at tick 1 slot 3 is already in range); at tick 2161 A posts M2[7], clears the markers (guidance and blue compass lamp now point to (28, 33)) and sets phase 100; from tick 2162 reaching view (23, 31) ends the flight.
- The F5 point (28, 33) is selectable only from the end of tick 2160 (§4.9).
- Outcome = 1.

### 5.19 Flight 19 (`1FEB:21E5..229B`): Osyluth

Start: §3.2; player class 7, start view (88, 76). Osyluth is slot 2 (class 8, marker 32h, armour 500, aggression FFh), patrolling x 63..87, y 96..112. Slots 3 and 4 (class 0, unmarked) have no rules. Comm viewer: phase 2 → screen 5, phase 3.

```text
succeeded := phase = 100 and home17
A: if Msn_NearLive(2, player, 25) and phase = 1:
       post(M2[4], c = 42); phase := 2; comm pending := 1
engage 2 → player, 20
B: if dead(2) and phase < 100:
       comm pending := 0; post(M2[8], c = 42); phase := 100
```

Notes:

- No ticker test: A overwrites whatever scrolls. With an unmoved player (tile (93, 79)) A fires when Osyluth reaches the north-east part of its loop (first at walk index 34, tile (69, 96)), i.e. within its first lap (LIKELY).
- **Completes** when Osyluth is destroyed (it has no exit), with or without the transmission.
- Outcome = 1. E3 block 19 has no debrief (episode-deltas §8.1).

## 6. Edge cases and original bugs

### 6.1 Quirks

#### Flights 1–9

| Quirk | Original behaviour | Port policy |
|---|---|---|
| Kill seen one tick late | As E2 §6 (P3 removed in E3 too) | Preserve (T1) |
| Exit/completion order | Same-tick completion in flights 2, 3, 6, 7, 8; one tick later in 1, 4, 9 (§4.1) | Preserve |
| Escaped targets count as gone | Every "all gone" test; only the outcome tests of 2, 3, 4, 8, 9 tell escape from destruction | Preserve |
| Target destroyed on its exit row or column | Counts as escaped in the outcome (tile-only tests) | Preserve |
| Flights that start inside the base cell | 1–5, 8, 9 end one tick after phase 100 with no input | Preserve |
| Flight 1 waves and completion set | A and B advance on the first loss of a pair; C tests 8, 2, 11, 12 only (OB-045) | Preserve |
| Flight 1 spawn class vs record | Slots 2..4 spawn as class 7 with the armour and shields of file class 0 | Preserve (setup spec) |
| Flight 2 slot 15 exit | Exit tile (79, 2) is one step before the path end (79, 1); the ship is removed at (79, 2) | Preserve |
| Flight 3 truck message | `M1[2]` only from phase 1; skipped when the Vultures are gone first | Preserve |
| Flight 5 escort | Arrival needs the exact tile (80, 199) (U-272); a lost escort ends the flight in the same tick (§4.6.6) | Preserve |
| Flight 6 base at the map limit | View X must be 6 (the west limit), rows 101..114, no sub-tile test | Preserve |
| Flight 6 slot 6 route | Touches column 1 at (1, 109) before its exit (1, 108); the map-edge rule may remove it there; it counts as gone either way | Preserve |
| Flight 7 inherited phase | OB-043 | Preserve in Classic; fix-enhanced candidate |
| Flight 7 duplicate phase test | `phase > 99 → flag := 0` before `phase < 100` | Harmless |
| Flight 8 silent transmission | OB-044 | Preserve |
| Flight 8 waypoint trigger | Reads `wp(s)` of trucks 1..3 with no class test; record waypoints are cleared at flight start (`AI_ResetShipRecords`), so a stale value cannot carry over | Preserve |
| Flight 8 exits gated by phase | Slot 1 starts on its exit tile; the gate keeps it in play | Preserve |
| No ticker gates | Posts overwrite whatever scrolls, including the objective | Preserve |
| Ships starting on the first row or column | F1 slots 8, 9 (row 1), F2 slots 9, 10 (row 1), F3 slot 2 (row 1), F4 slots 5, 11 (column 1), F9 slot 3 (column 1); F1 slot 11 (column 1, hidden until phase 3) | U-273 |
| Shared counters and flags | DS:31E2, DS:31E4, DS:31C4, DS:31C9 are globals | Harmless; a port may use locals |

#### Flights 10–19 and 21

| Quirk | Original behaviour | Port policy |
|---|---|---|
| Timed events act at the end of the tick | Stores of `Msn_E3TimedEvents` reach `Msn_UpdateB` one tick later; the flight-12 map patch and the flight-16 class patch already act at steps 27/29 of the same tick | Preserve (T1) |
| Exact tick equality | Each timer fires only in tick *n* exactly (32-bit compare); pauses and menus do not advance the count | Preserve |
| Flight 10 phase counter | Phases 4..40 count ticks; exit 5 opens at phase 39 (tick 335), D waits for phase 13 (tick 309) | Preserve the values (saves, comm viewer and tests see them) |
| Flight 10 truck destroyed on (22, 166) | D posts "home" (M2[2]) and the outcome is 1 | Preserve |
| Flight 10 slot 4 released on the hangar tile | Slot 4 appears at (22, 166) at tick 10, where slot 3 started | Preserve |
| Flights 11, 13, 14, 21 positional | No phase test; reaching the base area ends the flight whatever the ships do | Preserve |
| Flight 17 starts in phase 100 | Setup sets phase 100; the flight is "fly home" | Preserve |
| Flight 12 hazard cells | Switched to tile 26h by setup, restored at tick 6480 (§4.6.3) | Preserve (T1: the hazard test reads the map) |
| Flight 14 silent removals | armour := 0 at ticks 75/175 removes slots 11/8 in the next AI pass without a kill (U-278) | Preserve |
| Flight 15/19 announcements | Not gated on the ticker; they cut off the scrolling text | Preserve |
| Flight 16 forced transmission | Screen 3 opens without F6, also during the end sequence (no "not over" test at step 15) | Preserve |
| Flight 16 lamp | No F6 lamp and no sound 200 when A fires in a draw-page-0 tick; one tick of lamp otherwise | Preserve |
| Flight 18 markers | All 20 markers 32h at start (also on empty slots, which the guidance ignores); all cleared by A | Preserve |
| Flight 18 timer | 2160 ticks of waiting, independent of the player | Preserve |
| Flight 19 slot 4 path | Steps with Y = 0, 255, 254, 253 after (81, 1); the map-edge rule removes the ship at row 1 (LIKELY); no rule reads it | Preserve; harmless |
| Hazard exclusions | No hazard damage in flights 14, 17, 19 | Preserve (episode-deltas §4.2) |
| Shared counter | `g_MsnI` (DS:31E2) and `g_I` (DS:1976) are globals | Harmless; a port may use locals |

### 6.2 Original bugs

Recorded in [original-bugs](../original-bugs.md):

- **OB-043** flight 7 inherits its phase (§5.7);
- **OB-044** flight 8 transmission is never announced (§5.8);
- **OB-045** flight 1 completes on four of nine marked ships (§5.1);
- **OB-046** flights 15/16 exit tile (199, 168) transposed from the path end (168, 199) (§5.15, §5.16);
- **OB-047** flight 16 enemies near an escort are ordered against the player (§5.16);
- **OB-048** flight 15 Kreel's 1750 shields absorb one hit (§5.15);
- **OB-049** flight 16 tint inconsistencies (§4.7.3);
- **OB-028** (rev 2, corrected): the flight-16 class freeze lasts until the next flight-17 setup (§6.3);
- **OB-053**, **OB-054** (setup): [e3-mission-setup](e3-mission-setup.md) §5.

### 6.3 Correction to OB-028 (flight-16 class freeze)

OB-028 says the flight-16 patch persists "for the rest of the program run" and that "later flights in the same run have frozen ships". The code says otherwise for the normal campaign order:

- `Msn_Setup` case 17 (`td3:0C5D:6BF3..6C43`) writes the original maximum speeds and accelerations of classes 0..8 back (§4.6.4). Flights 17, 18 and 19 after a normal flight 16 are **not** affected. No other store to the two tables exists in the exported E3 code (operand search of all exported listings: units `0000`, `04A0`, `0C5D`, `1BB3`, `1BD8`, `1FEB`, `2215`, `2935`). The AI unit `2B72` and the library units were not exported; they are not expected to write class tables (LIKELY).
- The patch does persist into every flight that starts **after flight 16 and before the next flight-17 setup** in the same program run:
  - a **retry of flight 16** after death (standard retry prompt) or a **resume** after an abort (menu → next mission = 16): the whole flight runs frozen from tick 1 (convoy, escorts and enemies cannot accelerate; the player, class 2, at top speed 20);
  - any flight other than 17 started after **loading a saved game** or a New Game in the same run (until a flight-17 setup runs): every mission ship and every player class except 2 has max speed 0 and accel 0. A player of class 3, 4, 6 or 7 (most E3 flights) could then not move at all, which blocks positional and base tests until the program is restarted (LIKELY; U-275).
- Proposed OB-028 text: "E3 flight 16 (phase 3): every class 0..8 gets max speed 0 and accel 0, then class 2 gets 20/4. `Msn_Setup` case 17 restores the original values, so the patch persists only until the next flight-17 setup: a retry or resume of flight 16 runs frozen from the start, and flights other than 17 started from a saved game or New Game in the same run are frozen (a non-class-2 player cannot move)." Policy: **preserve** for the flight-16 retry/resume (Classic); the user should decide whether the save-load path is a progression blocker (**fix-all** candidate) (MDG §6.3).

### 6.4 Corrections to [episode-deltas](../../re/episode-deltas.md) rev 2

| Section | Says | Correct (this draft) |
|---|---|---|
| §4.1 timed-event table, flight 12 | map patch "(hazard tiles, §4.2)" | Right cells and codes; the patch restores the `td4.map` bytes that `Msn_Setup` case 12 replaced with 26h (§4.6.3) |
| §4.1 timed-event table, flight 18 | "E3 DS:1EAD := 1 (meaning unknown, U-202)" | DS:1EAD = flag of F5 target-information entry 21; flight 18 starts with 0 at (28, 33) (§4.9) |
| §4.2 item 1 | "`td3.map` gets them from the flight-12 patch" | Flight 12 flies `td4.map`; no `td3.map` flight ever has hazard tiles |
| §6, E3 comm viewer row | (8,2), (15,2), (16,2), (19,2) | Same rows; code order (8,2), (16,2), (19,2), (15,2); flight 16's request is forced at step 15 |
| §10 table | `Msn_LoadTargetInfo` E3 `1BD8:12A1` | The routine starts at `1BD8:0DCF` (first flight compare `0DF5`); `12A1` is the flight-3 compare |
| §12 / OB-028 | frozen ships in later flights | §6.3 |

## 7. Test vectors

Static unless the evidence column names a capture. "Tick *t*" is the tick in which the stated cause happens. Player tile = (viewX + 5, viewY + 3).

### Flights 1–9

| # | Setup | Expected | Evidence |
|---|---|---|---|
| C1 | **E3 flight 1, no input, tick 1, step 8.** View (167, 66), offsets (32, 16) → player tile (172, 69). Live: 5 (85, 160) class 7, 6 (85, 162) class 7, 8 (80, 1) class 8, 9 (78, 1) class 8. Hidden: 2, 3, 4 at (85, 197 / 195 / 193), 11..14 at (1 / 3 / 5 / 7, 111). Empty: 1, 7, 10, 15..20 | **No rule of `Msn_UpdateA` fires.** `engage s → player, 15` (s = 5..14): live attackers 5, 6, 8, 9 have \|dx\| = 87, 87, 92, 94 (all ≥ 15); the hidden ones fail the class test. **Zero `AI_SetTarget` calls.** A, B, C: no (8 and 9 live, phase 1). Exits: no slot on its exit tile. succeeded 0, phase 1, no post, no class change. The same holds with E1's view-tile convention (\|dx\| ≥ 82). With no input this is expected to stay true for the whole flight (LIKELY): the paths keep slots 5, 6, 8, 9, 2..4 on columns 78..85 and slots 11..14 on row 111 (\|dy\| = 42), so only a long AI detour could bring one within 15 tiles | **C-0036**: confirmed for ticks 1–58 (phase 1, no target field written in any ship record, `RandSeed` unchanged at 05322D58h, no class change) |
| C2 | Tick 1 of every flight 1–9, no input (start tiles §3.2/§3.3, setup classes) | No engagement rule and no phase step fires in any flight. Closest pairs (distance − range on the larger axis): F3 slot 11 (166, 39) to the player, 30 − 15; F5 slot 3 to slot 7, 56 − 20; F7 slot 18 to slot 5, 54 − 15; F8 nearest enemy to the player (phase-2 test), slot 5 (50, 150), \|dx\| 28 ≥ 15 | static; flight 8 confirmed by **C-0041** (ticks 1–36 without input: phase 1, no target written, `RandSeed` unchanged) |
| 1 | `Msn_NearLive(5, 0, 15)`, view (167, 66) (player tile (172, 69)), tile(5) = (158, 69) / (157, 69) / (186, 83) / (186, 84), class 7 | true / false / true / false | static |
| 2 | Flight 1 base test: view (167, 66), subY 31, phase 100 / subY 32 / phase 5 | succeeded 1 / 0 / 0 | static |
| 3 | Flight 6 base test, phase 100: view (6, 101) / (6, 114) / (6, 100) / (6, 115) / (7, 108); any subY | 1 / 1 / 0 / 0 / 0 | static |
| 4 | Flight 7 or 9 base test: view (17, 164), subY 35, phase 100 / subY 36 / view (17, 165) | 1 / 0 / 0 | static |
| 5 | Flight 1: class(9) = FFh, class(8) = 8, phase 1 at step 8 | phase 3; class(11..14) = 7 at (1..7, 111); B does not fire this tick (11, 12 now live); C does not fire | static |
| 6 | Flight 1: phase 3, slot 12 destroyed in step 6 of tick *t* | *t* step 8: nothing (still live); *t* + 1 step 8: B: phase 5, class(2..4) = 7; C no (slot 2 live) | static |
| 7 | Flight 1: phase 5; classes of 8, 2, 11, 12 = FFh; 3, 4, 9, 13, 14 live | C: phase 100, `M1[0]` colour 37, ticker 1; succeeded 0 this tick, 1 next tick if view (167, 66) and subY ≤ 31 | static (OB-045) |
| 8 | Flight 5: tile(2) = (80, 199), class(2) = 0, phase 1 at step 8 | class(2) FFh, tile(2) stays (80, 199), `M1[5]` colour 37, phase 100; the timed-event step of the same tick does not set lost; teardown outcome 1 | static |
| 9 | Flight 5: slot 2 destroyed in step 6 of tick *t*, phase 1 | step 8: no change; step 10: class(2) FFh; timed-event step of *t*: lost := 1; the loop ends at step 33 of *t* | static |
| 10 | Flight 7 starting with phase 100 (after flight 6 succeeded); the player flies to view (17, 164), subY ≤ 35, with the convoy still on its way | succeeded 1 at the first step 8 at that position; `M1[7]` never posted; outcome 1 | static (OB-043, U-270) |
| 11 | Flight 7 with phase 1; the last convoy ship removed by `exit @ (21,166)` at step 8 of tick *t* | A in the same step: phase 100, `M1[7]` colour 42 | static |
| 12 | Flight 8: phase 1, player tile within 14 tiles of a live slot 6 on both axes | B: phase 2 (C: no slot 4..10 dead, so it stays 2); no lamp, no message. F6 then (back page 0, step 15): screen 1, phase 3, comm pending 0, F6 key byte 0 | static (OB-044) |
| 13 | Flight 8: phase 2 or 3, slot 5 destroyed at step 6 of *t* | *t* + 1 step 8: C sets phase 4; the attack rules (phase > 3) run in the same block | static |
| 14 | Flight 8: phase 1, `wp(2)` becomes (108, 115) in step 10 of *t* | *t* + 1 step 8: D sets phase 4 as the last rule; attack rules and exits active from *t* + 2 | static |
| 15 | Flight 8 teardown: tiles of 1..3 all (22, 166) / slot 2 destroyed at (60, 168) | outcome 1 / 0 | static |
| 16 | Flight 9: the last truck reaches (168, 199) in step 10 of *t*; player unmoved (view (17, 164), subY 16) | *t* + 1 step 8: exit hides it (phase 1); *t* + 2 step 8: A: phase 100, `M1[9]` colour 42; *t* + 3 step 8: succeeded 1; loop ends at step 33 of *t* + 3; outcome 0 | static |
| 17 | Flight 2: the last live truck removed by `exit @ (84,199)` at step 8 of *t* | A in the same step: phase 100, `M1[1]`; outcome 0 (Y 199) | static |
| 18 | Flight 3: Vultures 9..12 gone while trucks 2..4 live, phase 1 | B: phase 100, `M1[3]`; later truck removals do not post `M1[2]` | static |
| 19 | Flight 4: all trucks destroyed away from column 199 | phase 100 one step after the last kill is seen; outcome 1. One truck removed at (199, 111) → outcome 0 | static |
| 20 | Flight 6: convoy 2..10 all destroyed | phase 100 and `M1[6]`; outcome 1 | static |

### Flights 10–19 and 21

| # | Setup | Expected | Evidence |
|---|---|---|---|
| C1 | **Tick 1, no input, every flight 10–19, 21** (start tiles §3.3, setup classes and phases §3.2) | Range tests that hold at step 9: flight 11 `engage 3 → player, 40` (slot 3 at (50, 150), player tile (22, 167): \|dx\| 28, \|dy\| 17); flight 18 `engage 3 → player, 35` (same tiles). No other engagement, announcement or phase rule fires in flights 10–19, 21 (flights 10, 14, 21 are in phase 1 with gated loops; flight 16: no enemy within 10 of an escort or 15 of the player) | static; first E3 capture |
| 1 | Flight 10 base test: view (17, 164), subY 35, phase 100 / subY 36 / phase 40 | succeeded 1 / 0 / 0 | static |
| 2 | Flight 14 or 21 base test: view (17, 164), subY 16, phase 1 | succeeded 1 | static |
| 3 | Flight 11: view (161, 76), any subY, phase 1 / (162, 76) / (160, 77) | succeeded 1 / 0 / 0 | static |
| 4 | Flight 13: view (37, 45) / (38, 45) / (36, 43) | succeeded 1 / 0 / 0 | static |
| 5 | Flight 18: view (23, 31), subY 48, phase 100 / view (23, 31), phase 2 / view (23, 32), phase 100 | succeeded 1 / 0 / 0 | static |
| 6 | Flight 10, no input | End of tick 10: phase 2, class(4) = 2. Tick 11: first engagement step. End of tick 300: phase 3. Tick 301 step 9: class(5) = 0 at (22, 166), M2[0] colour 42, ticker 1, phase 5 at the end of the block (A then B). Phase 40 from tick 336 | static |
| 7 | Flight 10: truck reaches (22, 166) in step 10 of tick *t* ≥ 334 | Tick *t* + 1 step 9: C removes it (class FFh), D: phase 100, M2[2]; teardown outcome 1 | static |
| 8 | Flight 10: truck destroyed at (30, 100) in step 6 of tick *t* ≥ 308 | Tick *t* step 9: no change; step 10: class FFh; tick *t* + 1 step 9: D: phase 100, M2[1]; outcome 0 | static |
| 9 | Flight 10: truck destroyed in step 6 of tick 303 | D fires at tick 309 (phase 13 reached at the B rule of tick 309) | static |
| 10 | Flight 12: classes 1..6 all FFh at step 9, phase 1 | A: phase 100, M2[3]; succeeded 0 this tick | static |
| 11 | Flight 12 teardown: tile(4) = (93, 76) / all trucks destroyed away from (93, 76) | outcome 0 / 1 | static |
| 12 | Flight 12: map cell (40, 122) at tick 1 / after the event routine of tick 6480 | 26h / 45h; a player whose (viewX + 5, viewY + 2) is (40, 122) takes hazard damage from step 27 of tick 6480 on, not before | static |
| 13 | Flight 14, no input | End of tick 25: classes of 6..8 = 8, of 9..11 = 7. End of tick 27: phase 2. Tick 28: slots 3..17 within 35 of (93, 79) ordered against the player. End of tick 75: armour(11) = 0; tick 76 step 10: class(11) = FFh, no explosion. Same for slot 8 at ticks 175/176 | static |
| 14 | Flight 15: Kreel live at (102, 80), phase 1, view (42, 60) / (43, 60) (player tile (47, 63) / (48, 63)) | \|dx\| 55: no announcement / \|dx\| 54: M2[4], phase 2, comm pending 1 | static |
| 15 | Flight 15: phase 2, Kreel destroyed before F6 | Next step 9: B: comm pending 0, M2[5], phase 100; a later F6 only consumes the request | static |
| 16 | Flight 16: trucks 2..4 FFh at step 9 of tick *t*, phase 1, draw page 0 at step 15 | *t* step 9: M2[4], phase 2, comm pending 1; step 15: request 1 → screen 3, F6 key byte 0, phase 3, comm pending 0; step 19: no F6 lamp; new step: five DAC steps (§4.6.4), class tables patched, phase 5; *t* + 1 step 9: M2[6], phase 100; *t* + 1 step 14: hit flash := 0 | static |
| 17 | Same as 16 with draw page 1 at step 15 of *t* | *t*: F6 lamp drawn at step 19 (sound 200 if its animation phase is 0); *t* + 1: screen 3, fade, phase 5; *t* + 2: M2[6], phase 100 | static |
| 18 | Flight 16 fade, palette entry (0, 0, 0) / (63, 40, 10) | After steps 1..5: (32, 0, 32), (48, 0, 48), (63, 0, 63), (48, 0, 48), (32, 0, 32) / (63, 20, 37), (63, 10, 50), (63, 0, 63), (63, 10, 50), (63, 20, 37). DAC entry 0 unchanged | static |
| 19 | Flight 16 after phase 5: tables | `k_ClassMaxSpeed` = 0, 0, 20, 0, 0, 0, 0, 0, 0, 0; `k_ClassAccel` = 0, 0, 4, 0, 0, 0, 0, 0, 0, 0 | static |
| 20 | Flight 16 after phase 5: Esc menu opened and closed | DAC entries 0..255 = half(e) (entry 0 (0, 0, 0) → (32, 0, 32)) | static |
| 21 | Flight 16 after phase 5: the player kills a ship with a homing missile in tick *t* | Ticks *t* … *t* + 3 (flash frames 4..1): white, (63, 63, B), (63, B, B), then `g_FlightPalette` (untinted) in all 256 entries; stays untinted until the next Esc menu | static |
| 22 | Flight 16 after phase 5: F5 pressed / F6 pressed | F5: no screen / F6: request consumed, no comm screen | static |
| 23 | Flight 17 setup after flight 16 | Tables back to 24, 28, 32, 28, 28, 28, 24, 28, 32 and 2, 4, 4, 4, 4, 4, 2, 4, 4 (classes 0..8); phase 100; view (17, 164) with subY ≤ 35 ends the flight | static |
| 24 | Flight 16 retried after the player died in phase 100 | Tick 1: max speed 0 and accel 0 for classes 0, 1, 3..8; class 2: 20 / 4; no tint until phase 5 | static (U-275) |
| 25 | Flight 18, no input | End of tick 2160: phase 2, DS:1EAD = 1. Tick 2161 step 9: M2[7], markers 1..20 = 0 (marker 21 = 3Dh), ticker 1, phase 100. Tick 2162: succeeded if view = (23, 31) | static |
| 26 | Flight 19: Osyluth live at (69, 96) / (68, 96), phase 1, player tile (93, 79) | \|dx\| 24, \|dy\| 17: M2[4], phase 2, comm pending 1 / \|dx\| 25: nothing | static |
| 27 | Flight 19: phase 3, Osyluth destroyed in step 6 of *t* | *t* + 1 step 9: B: comm pending 0, M2[8], phase 100 | static |
| 28 | Teardown of flights 11, 13..19, 21 with any tiles | outcome 1 | static |
| 29 | `Msn_E3TimedEvents` in any flight other than 5, 10, 12, 14, 16, 18, 21 | No store, no call | static |

## 8. Open questions

Rows are in [UNKNOWNS](../../re/UNKNOWNS.md).

| ID | Question | Pri | Evidence so far | How to resolve | Milestone / task | Status |
|---|---|---|---|---|---|---|
| U-270 | E3 flight 7 starts in the inherited phase (OB-043). What is `g_MissionPhase` at the first tick of flight 7 on each campaign path: after flight 6 succeeded (100 expected), after flight 6 ended by death or abort, after a flight-7 retry? Is DS:31DE zero at program start? Does any path start flight 7 below 100? | P2 | Static: no phase store in `Msn_Setup` case 7. A byte scan of `td3.exe` finds phase writes only in `Msn_Setup` (every other case), `UI_CommViewer`, `Msn_E3TimedEvents`, `Msn_UpdateA` and `Msn_UpdateB` ([sim/e3-mission-logic §5.7](../spec/sim/e3-mission-logic.md)) | Flow owner reads block 6 (`td3:0000`); capture of flight 7 tick 1 (DS:31DE) after a played flight 6 | M10 | OPEN |
| U-271 | E3 flight 8: confirm in a capture that the F6 lamp never lights, that F6 pressed in phase 2 shows comm screen 1 (`2215:0135`) and sets phase 3, and that phase 2 is entered by proximity to slots 4..10 | P3 | Static (OB-044) | Capture | M10 | OPEN |
| U-272 | E3 flight 5: slot 2's path continues past the arrival tile (80, 199) to (80, 200) and (80, 201), off the map. Does the escort really stand on (80, 199) at a step 8, given no map-edge removal on row 199 and the AI's handling of off-map waypoints (detour, path wrap)? If it ends on another tile of row 199, the arrival rule never fires and the flight cannot be completed (only lost) | P2 | Static ([sim/e3-mission-logic §5.5](../spec/sim/e3-mission-logic.md); [mission-ships](../spec/sim/mission-ships.md) §4.2.3) | Mission-ships model run on `m5.td3`, then capture | M10 | OPEN |
| U-273 | E3 ships that start on the map's first row or column: flight 1 slots 8, 9 (row 1), flight 2 slots 9, 10 (row 1), flight 3 slot 2 (row 1), flight 4 slots 5, 11 (column 1), flight 9 slot 3 (column 1), and flight 1 slot 11 (column 1, spawned in phase 3). Are any of them removed by the map-edge rule in their first ticks (first waypoint without the setup `AI_ChooseWaypoint` loop, U-205)? In flight 1 such a removal would fire rule A at tick 2 | P3 | Static | Capture of flight 1 ticks 1–3 (slot classes and tiles) | M10 | OPEN |

Existing unknowns that apply unchanged: U-204 (kills seen one tick late), U-205 (first waypoint), U-244 (base test during the end sequence).

| ID | Question | Pri | Evidence so far | How to resolve | Milestone / task | Status |
|---|---|---|---|---|---|---|
| U-274 | E3 flight 16 after the class patch (max speed 0 and accel 0 for every class but 2; class 2: 20 / 4): what does a moving mission ship do (stop at once, coast, jitter), does it keep turning and firing, and how fast does a player above speed 20 slow down? | P2 | Static: [sim/e3-mission-logic §4.6.4](../spec/sim/e3-mission-logic.md); the tables are read by the player thrust (`td3:0C5D:D7CC..`) and by `AI_UpdateShips` | [flight-model](../spec/sim/flight-model.md) and [mission-ships](../spec/sim/mission-ships.md) owners; capture of flight 16 around the transmission | M10 | OPEN |
| U-275 | E3 OB-028 outside flight 16: confirm that `Msn_Setup` case 17 restores the tables, that a retry or resume of flight 16 runs frozen from tick 1, and what happens after loading a saved game or New Game in the same run (can a class-3/4/6/7 player move at all?) | P2 | Static: [sim/e3-mission-logic §6.3](../spec/sim/e3-mission-logic.md); no other table store in the exported code | Flow owner (menu load/new game paths); capture: play to flight 16 phase 5, die, retry; then load an early save | M10 | OPEN |
| U-276 | E3 OB-046: confirm that the flight-16 convoy trucks (and flight-15 truck 1) reach (168, 199), are never removed there, and drive back along their paths, so that flight 16 needs all three trucks destroyed | P2 | Static: exit tiles (199, 168) not on the walks; row 199 has no map-edge removal ([mission-ships](../spec/sim/mission-ships.md) §4.2.3) | Mission-ships model run on `m16.td3`; capture | M10 | OPEN |
| U-277 | E3 flight 10: confirm that the medical truck leaves its start and exit tile (22, 166) well before tick 335 (when its exit opens), how long its round trip to (41, 57..58) takes, and that it is removed on its return | P3 | Static: [sim/e3-mission-logic §5.10](../spec/sim/e3-mission-logic.md); first-waypoint delay U-205 | Mission-ships model run on `m10.td3`; capture | M10 | OPEN |
| U-278 | E3 flight 14: confirm that slots 11 and 8 vanish without explosion, sound or kill one tick after their armour is zeroed (ticks 76, 176), and what a player hit in the gap does | P3 | Static: [sim/e3-mission-logic §4.6.5](../spec/sim/e3-mission-logic.md); E2 e2-mission-logic §5.11 (same pass-1 removal) | Capture of flight 14 ticks 70–180 | M10 | OPEN |
| U-279 | E3 F5 target-information entry 21: confirm the presentation (flight 18: the point (28, 33) becomes selectable on the F5 screen only after tick 2160; what is drawn for it) and the second point (entry 22) of flights 8, 10, 11/21, and flight 9's missing flag (OB-054) (with U-284) | P3 | Static: [sim/e3-mission-logic §4.9](../spec/sim/e3-mission-logic.md); `UI_ShipSwitchScreen` `td3:0C5D:9A99`, `Msn_LoadTargetInfo` `td3:1BD8:0DCF` | [flow/flight-screens](../spec/flow/flight-screens.md) owner; capture of the F5 screen in flights 11 and 18 | M10 | OPEN |

Resolutions of existing rows:

| ID | Resolution |
|---|---|
| U-202 | **RESOLVED** (RE-10b, static): DS:1EAD is the flag of F5 target-information entry 21, the fixed map point. `Msn_LoadTargetInfo` sets it to 1 for every E3 flight 10–19, 21 except flight 18, where it starts at 0 with the point (28, 33); `Msn_E3TimedEvents` sets it at tick 2160, after which the F5 screen offers the point; the next tick rule A posts M2[7], clears the markers and sets phase 100. No simulation effect. [sim/e3-mission-logic §4.9](../spec/sim/e3-mission-logic.md) |
| U-203 | **RESOLVED** (RE-10b, static; appearance LIKELY): the tint is DAC-only and never written to `g_FlightPalette`; nothing removes it on purpose (hit flash suppressed, F5 blocked, Esc menu re-tints). It disappears after a white flash (homing-missile hit; frame 1 restores the untinted palette until the next Esc menu) and in the final fade-out; the HUD lamp entries are never tinted; after an Esc menu colour index 0 is tinted too (OB-049). Nothing carries over to later flights. [sim/e3-mission-logic §4.7.3](../spec/sim/e3-mission-logic.md). A flight-16 capture would confirm the appearance |

Existing unknowns that apply unchanged: U-204 (kills seen one tick late), U-205 (first waypoint, relevant to every ship released by a timer: flight 10 slots 4 and 5, flight 14 slots 6..11), U-244 (base test during the end sequence), U-038 (header byte A of `m18.td3` slot 11).

## 9. Provenance

| Claim | Address / evidence | Method | Confidence |
|---|---|---|---|
| Block ranges (dispatch compare to the next compare) | `td3:1FEB:0A8D..0C0D` (1), `0C0E..0D4D` (2), `0D4E..0EC6` (3), `0EC7..1088` (4), `1089..1179` (5), `117A..12B5` (6), `12B6..13E2` (7), `13E3..159F` (8), `15A0..167F` (9); prologue `0A7F..0A8C` (reserves a 2-byte local that is never used), epilogue `1680..1683` | Static: listing `range_1FEB_0000.asm` read in full, gap-free | CONFIRMED (code) |
| Every rule, compare and loop bound | Same ranges. 81 calls besides the stack check: 10 `Msn_PostTicker`, 23 `Msn_ExitAt`, 24 `Msn_NearLive`, 24 `AI_SetTarget`. Per flight (post / exit / near / set): 1: 1/4/1/1; 2: 1/4/2/2; 3: 2/2/2/2; 4: 1/4/4/5; 5: 1/0/4/4; 6: 1/1/3/3; 7: 1/3/2/2; 8: 1/1/5/4; 9: 1/4/1/1. 64 stores besides the loop counters DS:31E2/31E4: succeeded 18, phase 15, ticker 10, flag DS:31C4 10, flag DS:31C9 3, class 8 (flight 1 A: 4, B: 3; flight 5 hide: 1); nothing else (no marker, comm, armour, lost or over store; no `Random`) | Static; push order of every call read from the instructions | CONFIRMED (code) |
| Primitives identical to E2 | `td3:1FEB:0000..070E` vs `td2:2236:0000..070E` | Byte compare of the two exe images (differences only relocations and DGROUP −4) | CONFIRMED |
| Slot array bases (class DS:399D + s, tiles DS:3A51/3A65 + s, marker DS:387B + s, record DS:4356 + (s − 1) × 429h) | `Msn_ExitAt` `1FEB:05E6..0611`; `Msn_Setup` marker and aggression loops; record fields +001h/+002h at `1FEB:1574..1592`, +3F6h at `0C5D:6228`, +3FAh at `0C5D:6460`, `B284` | Static (address arithmetic) | CONFIRMED (code) |
| M1 indices and colours | Pushed offsets 070F..0A25 mapped onto the 10-element run; colours 25h / 2Ah | Static + byte walk (`td3.exe`, file offset = 14544 + seg × 16 + off) | CONFIRMED |
| Message runs, counts, no digits, no F6 mention | `1FEB:070F..0A7E` (10), `1FEB:1684..19AC` (9); `e3.objectives` 0C5D:59C8 (elements 0..8 for flights 1..9) | Byte walk and scan | CONFIRMED |
| Start state | `Msn_Setup` `0C5D:6149..671C`; start views `0C5D:B596..B60C`; slot tiles `B385..B3C8`; armour/shields from file class `B242..B28D`; `Msn_Setup` call `B66E`; mission files `m1.td3`..`m9.td3` | Static + file decode | CONFIRMED (code, bytes) |
| No phase store for flight 7 | `0C5D:653D..65B8`; byte scan of `td3.exe` for every write form to DS:31DE (55 immediate stores and 1 INC, all in `Msn_Setup`, `UI_CommViewer`, `Msn_E3TimedEvents`, `Msn_UpdateA`, `Msn_UpdateB`) | Static + byte scan | CONFIRMED (code); campaign consequence LIKELY |
| No comm-pending store in flights 1–9 | Stores to DS:31C0 only at `0C5D:80B9`, `80DB`, `80FD`, `811F`, `A9DF` (clears) and `1FEB:1F0D`, `1F20`, `1FF6`, `224C`, `227A` (`Msn_UpdateB`) | Static (all exported listings) | CONFIRMED (code) |
| Tick placement | `0C5D:C4C3`/`C4C8` (A/B), `C4D1` (AI), `CC3E..CC64` (comm request and viewer), `D168` (timed events), `D789..D797` (F6), `EA1D..EA39` (loop exit) | Static | CONFIRMED (code) |
| Comm viewer table | `0C5D:8099..8125` | Static | CONFIRMED (code) |
| Flight 5 lost rule | `0C5D:99E8..9A01` | Static | CONFIRMED (code) |
| Outcome tests 1–9 | `0C5D:EA52..EB4B` | Static | CONFIRMED (code) |
| Home points | `0C5D:81C5..81E7` | Static | CONFIRMED (code) |
| Paths and exit tiles on paths | `m1.td3`..`m9.td3` (stride 501), waypoint walk one tile per step | Asset decode (private script, numeric output only) | CONFIRMED (bytes); movement LIKELY |
| Gameplay consequences (who arrives where, what the player must do) | [mission-ships](mission-ships.md) §4.2.3 (edge removal), §4.3 (paths loop) | Spec + static | LIKELY |
| Block ranges (dispatch compare to the next compare) | `td3:1FEB:19B7..1B4F` (10), `1B50..1BDE` (11), `1BDF..1C3E` (21), `1C3F..1D6F` (12), `1D70..1E12` (13), `1E13..1EA5` (14), `1EA6..1F89` (15), `1F8A..20E9` (16), `20EA..2149` (17), `214A..21E4` (18), `21E5..229B` (19); prologue `19AD..19B6`, epilogue `229C..229D` | Static: listing `range_1FEB_0000.asm`, 878 instructions, checked gap-free | CONFIRMED (code) |
| Every rule, compare and loop bound | Same ranges. 75 calls: 1 stack check, 11 `Msn_PostTicker`, 23 `Msn_ExitAt`, 20 `Msn_NearLive`, 20 `AI_SetTarget`. Per flight (post / exit / near / set): 10: 3/3/4/6; 11: 0/0/1/1; 21: 0/0/1/1; 12: 1/9/1/1; 13: 0/2/1/1; 14: 0/4/1/1; 15: 2/2/2/1; 16: 2/3/5/5; 17: 0/0/1/1; 18: 1/0/1/1; 19: 2/0/2/1. 56 stores besides `g_MsnI`: succeeded 28, phase 11 (10 moves, 1 increment), ticker 10, comm pending 5, class 1 (flight 10), marker 1 (flight-18 loop). No ticker read, no `Random`, no lost/over/armour store | Static, private call/store counter (numbers only); push order of every call read from the instructions | CONFIRMED (code) |
| M2 indices, count, digits | Pushed offsets 1684..1955 mapped onto the 9-element run `1FEB:1684..19AC`; M1 offsets never pushed | Byte walk of `td3.exe` (file offset = 14544 + seg × 16 + off) | CONFIRMED |
| Objectives | Pushed offsets `0C5D:5CF2`, `5D4C`, `5DA7`, `5E02`, `5E60`, `5EBD`, `5F1A`, `5F77`, `5FD4`, `6031`, `608E` = elements 9..19 of the 21-element run `0C5D:59C8..6148` | Byte walk | CONFIRMED |
| Start state | `Msn_Setup` cases `0C5D:671D..6D96` (blocks in §3.2); start views `0C5D:B596..B60C`; missiles `B797..B7A5`; mission files `m10.td3`..`m19.td3`, `m21.td3` (classes, paths; `m11.td3` = `m21.td3` by SHA-256) | Static + file decode (private script, numeric output only) | CONFIRMED (code, bytes) |
| Paths and exit tiles on walks | §3.3; waypoint walk one tile per step | Asset decode | CONFIRMED (bytes); movement LIKELY |
| `Msn_E3TimedEvents` | `0C5D:959E..9A6C` (all 12 branches); `9A6D..9A98` are string constants; call `0C5D:D168`; tick count reset `B0A3..B0A8`, increment `BB20..BB30` | Static | CONFIRMED (code) |
| Flight-12 cells | `0C5D:95E0..96E8` (tick 6480) and `68C7..69CF` (setup, 26h): same 27 offsets in the same order; values equal `td4.map` bytes; map read at `B47E` before `Msn_Setup` (`B66E`); indexing `D298..D2B1` | Static + byte comparison with `td4.map` | CONFIRMED |
| Flight-16 fade and class patch | `0C5D:9702..98F6` (fade, 5 × `Gfx_SetDacEntry` loops, 5 × `Gfx_WaitRetraceStart`), `9907..9947` (tables, phase); original table values from the DGROUP image (DS:041C..0444); restore `6BF3..6C43` | Static + image bytes | CONFIRMED (code); appearance LIKELY |
| Flight-16 loop specials | `0C5D:BC37..BC43` (burst), `CC18..CC28` (hit flash), `CC3E..CC64` (forced request, viewer), `D7A3..D7B1` (F5), `7EE6..7F63` (menu tint); `Gfx_HitFlashStep` guards `A55E`, `A6A4`, `A80B` | Static | CONFIRMED (code) |
| DAC write paths (U-203) | All `Gfx_SetDacEntry` / `Pal_SetEntry` / fade call sites in unit `0C5D` (fades `01F6`, `0422`; menu and sub-screens `6E02..7F77`, `A28E..A55A`; lamps `871B..8784`, `8E18`, `9290`, `929D`; flash frames `CA38..CC07`; hit flash `A55B..A91E`; Alt `E486..E607`; teardown `EA3C`); no palette call in `UI_CommScreen` (`540E..568E`) or in unit `2215`; palette mode stores only `793F`, `A3DA`, `B0AB` (0), `E494`, `E52A` (inside the Alt handler) | Static (call-site scan of the exported listings) | CONFIRMED (code) |
| Comm viewer | `0C5D:8099..8125` | Static | CONFIRMED (code) |
| Outcome tests | `0C5D:EA52..EB99` (flight 10 `EB4C..EB61`, flight 12 `EB66..EB99`) | Static | CONFIRMED (code) |
| Home points | `0C5D:81C5..8223` | Static | CONFIRMED (code) |
| U-202 (entry 21) | `Msn_LoadTargetInfo` blocks `1BD8:2614`, `294D`, `2B88`, `2D55`, `2F69`, `312A`, `32CB`, `353E`, `3810`, `3985` (flag 0 at `398E`), `3B26`; readers `0C5D:9F8D`, `9FCC`, `9FF4` only | Static (operand scan of all exported listings) | CONFIRMED (code); F5 drawing LIKELY |
| Lost and unused flags | `g_MissionLost` stores `0C5D:99FD`, `AA2D` only; `g_CutsceneRequest`, `g_MsnEventCount` cleared at `AA32`, `AA37`, never set | Static (all exported listings) | CONFIRMED (code) |
| Kreel shields (OB-048) | `0C5D:6B62` (shields(15) := 1750); [weapons](weapons.md) §4.3 step 4; [mission-ships](mission-ships.md) §4.2 regeneration cap | Static + spec | LIKELY |

## 10. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-03 | RE-10a, RE-10b (Archaeologist + Spec author, drafts for RE-10); merged and checked by RE-10 | Initial: E3 architecture, primitives, base tests, comm viewer, outcome tests, flights 1–9 (RE-10a); flights 10, 11, 21, 12–19, `Msn_E3TimedEvents`, flight 16 across the loop, U-202 and U-203 resolved (RE-10b); OB-043..OB-049, OB-028 correction, U-270..U-279; capture vectors C-0036 (flight 1), C-0041 (flight 8) (RE-10) |
