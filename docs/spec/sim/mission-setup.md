# Per-flight setup (`Msn_Setup`, `0E8B:60D8`)

| | |
|---|---|
| **Spec ID** | `sim/mission-setup` |
| **Revision** | 1 |
| **Status** | Draft |
| **Fidelity tier** | T1 |
| **Confidence** | CONFIRMED for the table values (constants read from the instruction stream of `0E8B:60D8` and `Sim_FlyMission`; flight 1 checked against capture C-0009). The meaning of markers and of the mission-phase values belongs to mission logic (RE-6) |
| **Tasks** | M8-01 (feeds M8-04, M8-05, M9) |
| **Unknowns** | U-038 (resolved), U-044 |

## 1. Summary

`Sim_FlyMission(flight)` loads the mission file and initialises the 20 ship slots from it ([mission-ships §4.1](mission-ships.md)), then calls `Msn_Setup`, which applies a hand-written block of overrides for the current flight number (1..21): the player's class, slot classes (ships removed or retyped), the target-bracket marker per slot, armour and "aggression" per ship record, a few class-table patches and flags, the objective text and the initial mission phase. `Sim_FlyMission` then sets the start view and palette mode from the flight number. The mission file's header bytes A, B and D are never read by anything (U-038).

## 2. Definitions

| Term | Where | Notes |
|---|---|---|
| Flight n | `g_FlightNumber` DS:32B9 | 1..21; flight n uses `m<n>.td` (flight 21 loads none) |
| Marker | `g_SlotMarker` DS:3958 − 1 + s | 0, 32h ("2") or 39h ("9"); target brackets drawn around the ship ([mission-ships §4.7](mission-ships.md)); read by mission logic and HUD (RE-6) |
| Aggression | ship record +3F6h | Fire-budget unit ([mission-ships §3.2](mission-ships.md)); 0 unless set here |
| Armour | ship record +3FAh (i16) | Default 2 × `k_ClassArmour[file class]` |
| Class tables | `k_ClassMaxSpeed` DS:02BE, `k_ClassShields` DS:02D2, `k_ClassAccel` DS:02DC | Typed constants; **patches here persist for the rest of the program run** |

## 3. Behaviour

### 3.1 Order of operations

```text
Msn_Setup:
    DS:396C := 3Dh                         (no reader found)
    marker[1..20] := 0
    1EDE:0E6A(flight)                      (per-flight message / comm tables: RE-6)
    the block for the current flight (table 3.2), in the order listed there
Sim_FlyMission, after Msn_Setup (0E8B:CBAA..CBD7, C879..C88C):
    if g_PlayerDrawLate != 0: view tile := (102, 100)
    if flight = 13 or 14:     view tile := (177, 153)
    flights 4 and 8: g_PaletteMode := 1 (infra-red view)
    ...  then AI_ChooseWaypoint for slots 1..20 (mission-ships 4.1 step 6)
```

The default start view is set earlier in `Sim_FlyMission` (`0E8B:C62B..C64D`): view tile (102, 104), view offsets (32, 16), draw page 1. Every flight block except 20 draws its objective text into off-screen VGA memory (`Txt_FlightDrawStringOpaque`) and sets the HUD ticker position `g_TickerPos` := 1 (start the objective ticker); every block sets `g_MissionPhase` := 1.

### 3.2 Per-flight table

"Slots" lists are in the order the code writes them; a later write wins. Armour and aggression are record fields of the slot. Values in decimal.

| Flight | Player class | Start view (tile) | Late draw | Slot class overrides | Markers | Armour | Aggression | Other |
|---:|---:|---|:-:|---|---|---|---|---|
| 1 | 4 | (102, 104) | | 3–8 := FFh; 14–20 := 7 | 1–11 := 32h | 1–17 := 3; 12–13 := 1000 | 8 := 100; 11 := 50; 14–20 := 50 | |
| 2 | 4 | (102, 104) | | 8–20 := FFh; 3–7 := FFh; 18–20 := 7 | 1–7 := 32h; 2 := 39h | 18–20 := 4; 3–7 := 5; 1 := 5 | 18–20 := 25; 1 := 100; 5 := 50; 6, 7 := 150 | |
| 3 | 4 | (102, 104) | | 1 := 4; 5–8 := FFh; 12 := FFh | 4 := 39h | 2–3 := 6; 6–7 := 6; 12 := 6 | 1 := 150; 2, 3 := 125; 6, 7, 12 := 225; 19, 20 := 50 | |
| 4 | 4 | (102, 104) | | 1 := 4; 4, 5, 7 := FFh; 8, 9 := 1 | 2–5, 7 := 32h | 2–5 := 6; 7 := 7 | 1, 2 := 210; 3 := 100; 4 := 110; 5 := 210; 7 := 150; 19, 20 := 50 | palette mode 1 |
| 5 | 4 | (102, 104) | | 2 := 7 | 3–5, 7 := 32h; 1 := 39h | 1 := 250 | 3, 4 := 100; 5 := 175; 7 := 75; 19, 20 := 50 | |
| 6 | 2 | (102, 104) | | | 7–9, 3, 5 := 32h | 3 := 50 | 3 := 255; 5 := 150; 7–9 := 75; 19, 20 := 50 | |
| 7 | 2 | (102, 104) | | 2–8 := FFh | 13–20 := 0; 1–12 := 32h | 1 := 50 | 13, 14 := 175 | slots 2–12 pinned every tick ([mission-ships §4.2.4](mission-ships.md)); `mine.td` sprites |
| 8 | 2 | (102, 104) | | 8, 9 := FFh; 3–6 := 7 | 15–18, 3–9, 12–13 := 32h | | 1 := 175; 3–6 := 75; 8, 9 := 200; 12, 13 := 50 | slot 12 offset Y := 31; palette mode 1 |
| 9 | 4 | (102, 104) | | | 1–10 := 32h | | 5–12 := 75; 13–18 := 150 | slot 18 path step 500 := (199, 121) |
| 10 | 4 | (102, 104) | | | 7–12 := 32h | 12 := 250 | 7–11 := 95; 12 := 180; 1 := 150 | |
| 11 | 2 | (102, 104) | | 7–9 := FFh; 1–3 := 2 | 1–3, 7 := 32h | 1–3 := 100; 7 := 150 | 1–3 := 195; 4, 5 := 25; 7–9 := 255; 17, 18 := 50 | |
| 12 | 3 | (102, 100) | yes | | 1–5 := 39h | | 7–13 := 115; 15, 16 := 55 | class 3: max speed := 20h, accel := 4; `td.hel`/`td.h2` sprites |
| 13 | 8 | (177, 153) | | 15–17 := 3 | 8–9, 11–13, 15–17 := 32h | | 15–18, 11–13 := 195; 8–9 := 150; 1–2 := 75 | |
| 14 | 8 | (177, 153) | | | 8–10, 12–13 := 32h | | 15–18, 11–13 := 195; 8–9 := 150; 1–6 := 75 | no F5 ship switch (tick-order step 29) |
| 15 | 3 | (102, 100) | yes | 8–12 := FFh | 1–5, 8–12 := 32h | | 3–12 := 125; 15 := 200; 16 := 125 | class 3: max speed := 20h, accel := 4; `td.hel`/`td.h2` |
| 16 | 4 | (102, 104) | | 8–13 := FFh | 1–3, 8–10 := 32h | | 4–6, 11–13 := 125; 17 := 200 | |
| 17 | 2 | (102, 104) | | 5–14 := FFh | 1, 5–6, 8–10, 12–14 := 32h | 5–14 := 4 | 3 := 150; 5–14 := 25 | |
| 18 | 2 | (102, 104) | | | 7, 10, 11, 15, 17, 18 := 32h | 3 := 250 | 1, 5, 6, 9, 13, 14 := 255 | |
| 19 | 0 | (102, 104) | | | | | 1–10 := 250 | DS:396C := 32h; per tick with phase > 1: player shields ≤ 10, slot 19 copied from DS:16EE.. ([mission-ships §4.2.4](mission-ships.md)) |
| 20 | 2 | (102, 104) | | 3 := 4; 4 := 5; 5 := 6 | 5 := 32h | 7, 8 := 250; 5 := 500 | 7, 8 := 250 | class 5 shields := 64; max speed of classes 0, 1, 3, 4, 5 := 11h (1 px/tick); slots 1–5 sink (mission-ships §4.2.4); `lander.td`; no objective text or ticker; player armour 64 in the bottom-right corner (globals §5) |
| 21 | 3 | (102, 100) | yes | | | | 7–13 := 115; 15, 16 := 55 | class 3: max speed := 20h, accel := 4; no mission file (slots keep the previous flight's data, U-044); ends after 25 ticks |

All flights: mission phase := 1. Player armour, shields and weapons are set from the (possibly patched) class tables after `Msn_Setup` ([weapons §3.2](weapons.md)).

### 3.3 Class-table patches persist

The writes to `k_ClassMaxSpeed`, `k_ClassAccel` and `k_ClassShields` change typed constants in DGROUP and are never undone. After flight 12, 15 or 21 has been flown in a program run, class 3 (helicopter) has maximum speed 20h for every later flight, including enemy class-3 ships in flight 13 and the player in later flights; after flight 20, classes 0, 1, 3, 4 and 5 are limited to 11h and class 5 has 64 shields until the program exits (only flight 21 follows in the campaign, but a retry or a saved game restarted in the same run is affected). The J+D+W cheat patches class 6 the same way (globals §5). OB-018.

## 4. Edge cases and original bugs

| Quirk | Original | Port policy |
|---|---|---|
| OB-018: class-table patches persist for the program run | See 3.3 | preserve in Classic: keep the class tables as session state, reset only at program start |
| Header bytes A, B, D | Loaded to DS:3A52 / 3A8E / 3A66 and never read | ignore |
| Flight 1 slots 9–11 | Start at map edges and are removed by mission logic in the first tick | mission logic (RE-6) |
| Armour of retyped slots | Armour and shields come from the file class (setup step 3) before the class override; e.g. flight 1 slots 14–17 (file class 1, now 7) start with 0 shields and regenerate to 25 | preserve |

## 5. Test vectors

| # | Flight | Expected after setup | Evidence |
|---|---|---|---|
| 1 | 1 | classes 0, 0, FFh × 6, 0, 0, 7, 1, 1, 7 × 7; armour 3 × 11, 1000, 1000, 3 × 4, 50 × 3; shields 0 × 7, 25, 0, 0, 25, 0 × 6, 25 × 3; aggression 0 × 7, 100, 0, 0, 50, 0, 0, 50 × 7; markers 32h × 11, 0 × 9; player class 4, view (102, 104, 32, 16), late draw 0, phase 1, palette mode 0 | C-0009 tick 0 |
| 2 | 12 | player class 3; `k_ClassMaxSpeed[3]` = 20h; view (102, 100); late draw 1 | static |

## 6. Open questions

- [ ] RE-6: meaning of markers 32h/39h for mission logic and the HUD; the role of `1EDE:0E6A`.
- [ ] U-044: flight 21's slots (no mission file is loaded, so the arrays hold the previous flight's state).

## 7. Provenance

| Claim | Address | Method | Confidence |
|---|---|---|---|
| Per-flight blocks | `0E8B:60D8..6EB0` | static (instruction stream) | CONFIRMED |
| Start view and palette mode | `0E8B:C62B..C64D`, `0E8B:CBAA..CBD7`, `0E8B:C879..C88C` | static | CONFIRMED |
| Header bytes never read | all segment listings: only the `BlockRead` destinations reference DS:3A52, 3A66, 3A8E | static | CONFIRMED |
| Flight 1 values | C-0009 | capture | CONFIRMED |

## 8. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-02 | RE-5 (Archaeologist + Spec author) | Initial: per-flight table for flights 1..21 |
