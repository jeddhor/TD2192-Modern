# Mission logic (Episode 1)

| | |
|---|---|
| **Spec ID** | `sim/mission-logic` |
| **Revision** | 1 |
| **Status** | Draft |
| **Fidelity tier** | T1 (phase changes, spawns, removals, success and failure decide the outcome of a flight; the order of the per-tick rules matters where two rules write the same ship) · T2 for the ticker text itself (render/hud.md) |
| **Confidence** | LIKELY overall (static: every rule below was read from the instruction stream, with the listing checked where the decompiler drops operands). Flights 1–3 CONFIRMED for the objective ticker start by capture (§7). The behaviour that `AI_SetTarget` triggers is RE-5's ([mission-ships](mission-ships.md)) |
| **Tasks** | M8-02 (feeds M8-05, M8-06, M8-07, M8-08, M8-10, M9) |
| **Unknowns** | U-031 (resolved for `2404`), U-180, U-181, U-182, U-183, U-184 |

## 1. Summary

Episode 1 has no mission scripts in data files: every flight's logic is code. Each tick, four places run mission rules: **`Msn_UpdateA`** (`2404:1426`, flights 1–9) and **`Msn_UpdateB`** (`2404:3521`, flights 10–20) right after the player ship is drawn (tick-order steps 8–9), **`Msn_UpdateEvents`** (`0E8B:9860`, step 18: ship respawns and scripted events), and the instruments routine (step 19), which holds one flight-7 rule. The F6 comm viewer (step 15) advances the phase when a pending transmission is read. A flight's rules are a flat list evaluated in a fixed order every tick: a **base test** (has the player returned to base after the objectives?), **exit rules** (a ship standing on a given tile leaves the game), **engagement rules** (when two ships, or a ship and the player, are within *r* tiles, one of them is ordered to attack the other), and a **phase script** keyed on the byte `g_MissionPhase` (1 at start, 100 = objectives done). Phase steps post a message to the HUD ticker, raise or drop the "incoming transmission" flag, spawn hidden ships (by giving them a class code) and set guidance markers. The flight ends normally when the base test passes; the end-of-flight code then computes a mission-specific outcome flag from where certain ships ended up.

## 2. Definitions and units

| Term | Original | Notes |
|---|---|---|
| Slot *s* | 1..20 | Mission ship slot ([structures](../../re/structures.md) §3–§4) |
| Class(*s*) | `g_SlotClass` DS:3A79 + *s*, u8 | FFh = not in play (never spawned, hidden, destroyed or left) |
| **Live** | class(*s*) ≠ FFh | |
| Tile(*s*) | (`g_SlotTileX` DS:3B2D + *s*, `g_SlotTileY` DS:3B41 + *s*), u8 | Kept after the ship dies or leaves |
| Phase | `g_MissionPhase` DS:32BA, u8 | Set to 1 by the per-flight setup (`Msn_Setup` `0E8B:60D8`); 100 = objectives complete |
| Ticker idle | `g_TickerPos` DS:32BC = 0 (u16) | A posted message sets it to 1; it returns to 0 after 78 page-0 ticks, i.e. about 156 ticks ([hud](../render/hud.md) §4.5) |
| Comm pending | `g_CommIncoming` DS:329C, u8 | 1 = a transmission waits for F6 (HUD indicator + sound, hud §4.6) |
| Marker(*s*) | `g_SlotMarker` DS:3957 + *s*, u8, *s* = 1..21 | Guidance colour class of slot *s*: 0 none, 32h target, 39h friend, 3Dh home. Element 21 (DS:396C) is the marker of the home point. Read by the guidance line and the compass lamps (hud §4.3) |
| Succeeded | `g_MissionSucceeded` DS:329E, u8 | Ends the flight at the end of the tick (tick-order step 35) |
| Lost | `g_MissionLost` DS:12CC, u8 | Ends the flight; mission failed (flow spec §4.5) |
| Over | `g_MissionOver` DS:32B7, u8 | Starts the end sequence; after 35 end ticks the flight ends with `g_PlayerDied` = 1 |
| Outcome | `g_MissionOutcome` DS:12CF, u8 | Computed at teardown (§4.8) |
| View tile | `g_ViewTileX/Y` DS:3A36/3A38, i16 | Map tile of the playfield's top-left corner |
| Mission loop counters | DS:32BE, DS:32C0, DS:32C2 (u16) | Globals shared by all rule loops (no locals). Nothing outside mission logic reads them |

**Where the player is.** The original uses three different "player tiles"; a port must keep each one where it is used:

| Used by | Player position |
|---|---|
| `Msn_NearLive` / `Msn_Near` (all engagement rules), base test | **view tile** (viewX, viewY), low bytes only |
| Respawn distance (`Msn_UpdateEvents`) | (viewX + 5, viewY + 2) |
| Guidance line, compass lamps, radar centre | (viewX + 5, viewY + 3) |

The ship is drawn at playfield pixel (144..175, 68..99), i.e. over tile (viewX + 4.5, viewY + 2.6) on average, so the first convention is about five tiles west and three tiles north of the ship.

## 3. Data

### 3.1 Primitives (unit `2404` and helpers)

| Name | Address | Pascal signature | Meaning |
|---|---|---|---|
| `Msn_ExitAt` | `2404:05DC` | `(Slot, X, Y: Byte)` | If tile(Slot) = (X, Y): class(Slot) := FFh. No liveness test, no kill credit |
| `Msn_NearLive` | `2404:06EE` | `(A, B, Range: Byte): Boolean` | Position of *k* = view tile if *k* = 0, else tile(*k*). True when \|xA − xB\| < Range **and** \|yA − yB\| < Range (signed 16-bit differences of the byte values, strict), and neither A nor B (when non-zero) has class FFh |
| `Msn_Near` | `2404:07DB` | same | As `Msn_NearLive` without the class tests (dead or hidden ships count at their last tile) |
| `Msn_PlayerTouches` | `2404:0616` | `(Slot): Boolean` | Pixel test of the ship against the player: ΔX = (tileX(Slot) − viewX) × 32 + subX(Slot) − (4 × Round(viewSubX / 4) + 128) and ΔY = (tileY(Slot) − viewY) × 32 + subY(Slot) − (viewSubY + 84); true when \|ΔX\| < 32 and \|ΔY\| < 32 and class(Slot) ≠ FFh. `Round` is the Real48 round of `viewSubX / 4.0` ([real48](../core/real48.md) §4.6), the X difference is a 32-bit signed value |
| `Msn_PostTicker` | `2404:030A` | `(X, Y: Integer; S: String; Colour: Byte)` | Draws S opaquely into the ticker strip (always X = 0, Y = 735) with the mission glyph routine `2404:0000`; render/hud §4.5 |
| `AI_SetTarget` | `34E6:3232` | `(Slot, Target: Byte)` | Ship record of Slot: +41Fh := Target (0 = player), +415h := 1, +41Bh := 1. What the ship then does is the AI's business ([mission-ships](mission-ships.md) §3.1, RE-5) |

### 3.2 Notation used in the flight sections

| Notation | Meaning |
|---|---|
| `post(id, c)` | `Msn_PostTicker(0, 735, text of id, c)` then ticker := 1. `c` is 37 unless written `c = 42` |
| `exit s @ (x, y)` | `Msn_ExitAt(s, x, y)` |
| `engage a → b, r` | `if Msn_NearLive(a, b, r) then AI_SetTarget(a, b)`; b = 0 is the player |
| `spawn s as c` | class(*s*) := *c* (the ship appears where it is; see §4.3) |
| `hide s` | class(*s*) := FFh |
| `go s` | ship record of *s*: +001h/+002h (current waypoint) := path step 2 (+00Fh/+010h) |
| `M1[k]`, `M2[k]` | resource `e1.flight_messages.01` / `e1.flight_messages.02`, element *k* (0-based) |
| `dead(s, …)` | every listed slot has class FFh |

### 3.3 Per-flight start state that the rules depend on

Set by `Msn_Setup` (`0E8B:60D8`, owned by RE-5; their spec is authoritative for slot classes, positions and armour). Listed here only so that the rules below can be checked.

| Flight | Objective (ticker at start) | Colour | Start phase | Markers at start | Hidden at start |
|---:|---|---:|---:|---|---|
| 1 | `e1.objectives[0]` | 37 | 1 | 1..11 = 32h | 3..8 |
| 2 | `e1.objectives[1]` | 37 | 1 | 1..7 = 32h, then 2 = 39h | 3..17 |
| 3 | `e1.objectives[2]` | 37 | 1 | 4 = 39h | 5..8, 12 |
| 4 | `e1.objectives[3]` | 37 | 1 | 2..5, 7 = 32h | 4, 5, 7 |
| 5 | `e1.objectives[4]` | 37 | 1 | 1 = 39h; 3..5, 7 = 32h | RE-5 |
| 6 | `e1.objectives[5]` | 37 | 1 | 3, 5, 7..9 = 32h | RE-5 |
| 7 | `e1.objectives[6]` | 37 | 1 | 1..12 = 32h | RE-5 |
| 8 | `e1.objectives[3]` (the infra-red text again) | 37 | 1 | 3..9, 12, 13, 15..18 = 32h | RE-5 |
| 9 | `e1.objectives[7]` | 37 | 1 | 1..10 = 32h | RE-5 |
| 10 | `e1.objectives[8]` | 37 | 1 | 7..12 = 32h | RE-5 |
| 11 | `e1.objectives[9]` | 37 | 1 | 1..3, 7 = 32h | RE-5 |
| 12 | `e1.objectives[10]` | 42 | 1 | 1..5 = 39h | RE-5 |
| 13 | `e1.objectives[11]` | 42 | 1 | 8, 9, 11..13, 15..17 = 32h | RE-5 |
| 14 | `e1.objectives[12]` (blank) | 42 | 1 | 8..10, 12, 13 = 32h | RE-5 |
| 15 | `e1.objectives[13]` | 42 | 1 | 1..5, 8..12 = 32h | RE-5 |
| 16 | `e1.objectives[14]` | 42 | 1 | 1..3, 8..10 = 32h | RE-5 |
| 17 | `e1.objectives[15]` | 42 | 1 | 1, 5, 6, 8..10, 12..14 = 32h | RE-5 |
| 18 | `e1.objectives[16]` | 42 | 1 | 7, 10, 11, 15, 17, 18 = 32h | RE-5 |
| 19 | `e1.objectives[17]` | 42 | 1 | none; home marker = 32h | RE-5 |
| 20 | none (no objective is drawn and the ticker is not started) | — | 1 | 5 = 32h | RE-5 |
| 21 | `e1.objectives[18]` | 42 | 1 | none | RE-5 |

Before the per-flight part, setup clears markers 1..20 and sets the home marker (element 21) to 3Dh; every flight except 20 then draws its objective into the ticker strip (x 0, VGA row 735) and starts the ticker (ticker := 1).

## 4. Behaviour

### 4.1 Where mission logic runs in a tick

Numbers are [tick-order](tick-order.md) steps.

```text
step 8   Msn_UpdateA      (2404:1426): only the block of the current flight (1..9) runs
step 9   Msn_UpdateB      (2404:3521): only the block of the current flight (10..20) runs
step 10  AI_UpdateShips   (moves live ships; ships spawned in steps 8-9 move this same tick)
step 15  comm viewer      (only when F6 was requested and the back page is 0): §4.6
step 18  Msn_UpdateEvents (0E8B:9860): respawns and scripted events, §4.7
step 19  instruments      (0E8B:8E87): guidance line, compass lamps, radar, ticker, indicator;
                          contains the flight-7 completion rule (§5.7)
step 35  leave the loop if aborted, end ticks = 35, lost, or succeeded
teardown outcome tests (§4.8)
```

Flight 21 has no rules in any of these routines; it ends by the 25-tick timer of tick-order step 1.

### 4.2 Shape of one flight block

Every block of `Msn_UpdateA`/`Msn_UpdateB` has the same shape, evaluated top to bottom once per tick:

```text
succeeded := (player is at the flight's base tile) and phase = 100      (flights 1..19; see 4.4)
the flight's rules, in the order given in section 5:
    exit rules, engagement rules and phase steps are interleaved; the order is part of the spec
```

A phase step is "if *condition* then *actions*". Several steps can fire in the same tick when an earlier step makes a later condition true; the sections below keep the original order so that this happens exactly as in the original. Some steps have an `else` branch that hides ships every tick until a phase is reached; this is how the original keeps later waves out of play (the mission file places them on the map from the start).

### 4.3 Spawning, hiding and removal

- **Spawning** gives a hidden slot a class code. The ship is at the tile and pixel offset it had when it was hidden (normally its mission-file start position, because hidden ships are not moved by `AI_UpdateShips`). Armour and shields are not reset by the spawn: they are whatever setup or earlier play left.
- **Hiding** and **exit rules** set class FFh. Neither counts as a kill (only `0E8B:0984` increments `g_MissionKills`, weapons §4.3). The tile is kept, which is what the teardown outcome tests read.
- **"All dead" tests** do not distinguish destroyed, escaped or never-spawned ships. Several flights advance when target ships *escape* through their exit tile, exactly as if the player had destroyed them.
- **Counting idiom.** Some flights count live ships as `n := 5 × (number of live slots in a..b)` and test `n = 0`; only the zero test matters.

### 4.4 Base test

`succeeded := (viewX, viewY) = (102, 104) and phase = 100`, re-evaluated (and so possibly cleared) every tick at the top of the block. (102, 104) is the view tile at the start of every flight except 12, 13 and 15, so the player has to fly back to within the 32 × 32-pixel cell of the start view. Exceptions: flights 12 and 15 accept viewX = 102 and viewY = 99..100 (the helicopter pad); flight 13 accepts viewX = 175..177 and viewY = 152..154 (the depot). Flight 20 has no base test; `Msn_UpdateEvents` sets `succeeded` (§5.20).

### 4.5 Messages and transmissions

- `post(id, c)` overwrites the ticker strip with the message and restarts the scroll (ticker := 1), even if a message is still scrolling. Phase steps that must not interrupt a message test "ticker idle" first.
- A transmission is announced by posting its "incoming transmission" message (colour 42) and setting comm pending := 1. Most flights repeat the announcement every time the ticker becomes idle while they stay in the announcing phase, so the message keeps coming back until the player presses F6.
- Comm pending is cleared by the comm viewer when the transmission is shown, and by flight rules when the sender dies.

### 4.6 Comm viewer (`UI_CommViewer` `0E8B:80D3`, tick-order step 15)

F6 sets the request `g_CommRequest` (DS:1706) every tick it is held, unless the mission is over. At step 15, when the request is set and the back page is 0, the request is cleared and the viewer checks this table. Only a matching (flight, phase) shows a transmission; otherwise the request is simply consumed.

| Flight | In phase | Shows (`UI_CommScreen(n)`, text `e1.comms.0n`) | Then phase := | Extra |
|---:|---:|---:|---:|---|
| 2 | 1 | 1 | 2 | |
| 5 | 2 | 2 | 3 | |
| 6 | 3 | 3 | 4 | |
| 8 | 4 | 4 | 5 | |
| 9 | 2 | 5 | 3 | |
| 10 | 2 | 6 | 3 | |
| 14 | 2 | 7 | 100 | markers: all 0, then slots 1..6 := 32h |
| 17 | 3 | 8 | 4 | |
| 18 | 2 | 9 | 3 | |

In every matching case the F6 key byte (DS:1A9C) is cleared and comm pending := 0. The transmission screen is modal: no ticks run while it is open.

### 4.7 `Msn_UpdateEvents` (`0E8B:9860`, tick-order step 18)

Despite its earlier name (`UI_UpdateStatusPanel`) this routine draws no HUD element. In this order it runs:

1. **Respawns.** For each listed slot that is dead (class FFh) and far from the player — \|viewX + 5 − tileX(s)\| > 15 **and** \|viewY + 2 − tileY(s)\| > 15 — give it a class and an armour value (record +3FAh, i16). The ship reappears at the tile where it died or left.

   | Flights | Slots (in order) | Class | Armour |
   |---|---|---:|---:|
   | 3, 4 | 19 | 7 | 1 |
   | 16 | 4, 5 | 8 | 15 |
   | 7 | 13, 14 | 7 | 10 |
   | 9 | 11, 12 | 7 | 10 |
   | 11 | 8, 9, then 17, 18 | 7 | 10 |
   | 1, 2, 5, 6 | 19, 20 | 7 | 1 |

2. Flight 17 base attack (§5.17), flight 20 lander (§5.20), flight 13 time limit (§5.13), flight 19 ship switch (§5.19), flight 18 bomb (§5.18), flight 17 base destruction (§5.17), and the flight-17 cut-scene (§5.17).

### 4.8 End of flight and outcome (`0E8B:F826..F990`)

```text
step 35: if aborted or end ticks = 35 or lost or succeeded: leave the tick loop
fade out; restore the keyboard handler
g_PlayerDied := (end ticks = 35)
outcome := 1
flight 3:  outcome := (tile(4) = (107, 106))
flight 5:  outcome := (tile(1) = (199, 122))
flight 12: outcome := any of tile(1) = (49,199), tile(2) = (50,199), tile(3) = (51,199),
                             tile(4) = (49,199), tile(5) = (51,199)
flight 16: outcome := any of tile(1) = (124,71), tile(2) = (125,71), tile(3) = (126,71),
                             tile(8) = (125,71), tile(9) = (125,71), tile(10) = (125,71)
flight 17: outcome := not lost
```

The tests read tiles only, so a ship destroyed elsewhere gives outcome 0 and a ship that reached its exit tile gives 1 even though it was removed there. How the flow uses `g_PlayerDied`, `g_MissionLost`, `g_MissionAborted` and the outcome is in [episode-program-flow](../flow/episode-program-flow.md) §3.4 and §4.3–4.5.

## 5. Per-flight logic

Each section lists the block in original order. Unless a step says otherwise, messages use colour 37.

### 5.1 Flight 1 (two trucks south, two north, convoy east, two more, a Vulture)

Start: objective `e1.objectives[0]`; player class 4; slots 3..8 hidden; markers 1..11 = 32h; phase 1. `Msn_UpdateEvents`: slots 19, 20 respawn (class 7, armour 1).

```text
succeeded := view = (102,104) and phase = 100
exit 1 @ (50,196);  exit 2 @ (51,197);  exit 3 @ (50,2);    exit 4 @ (51,2)
exit 5 @ (198,122); exit 6 @ (198,123); exit 7 @ (198,124); exit 8 @ (198,123)
exit 9 @ (198,122); exit 10 @ (198,123); exit 11 @ (50,1)
engage 8 → player, 20
engage 11 → player, 20
for s = 14..20: engage s → player, 10
A: if dead(1, 2) and phase = 1:
       phase := 2; spawn 3 as 0; spawn 4 as 0; post(M1[0])
B: if dead(3, 4, 1, 2) and phase < 3:
       post(M1[1]); phase := 3; spawn 5 as 0; spawn 6 as 0; spawn 7 as 0; spawn 8 as 7
   else if phase < 3: hide 5, 6, 7, 8
C: if dead(5, 6, 7, 8) and phase = 3:
       post(M1[2]); phase := 4; spawn 9 as 0; spawn 10 as 0
   else if phase < 4: hide 9, 10
D: if dead(9, 10) and phase = 4:
       if ticker idle: post(M1[3]); phase := 5; spawn 11 as 7
   else if phase < 5: hide 11
E: if dead(11) and phase = 5:
       post(M1[4]); phase := 100
```

Notes:
- In D the `else` belongs to the outer test: while slots 9 and 10 are dead but the ticker is busy, slot 11 is **not** hidden (it is already hidden from the earlier ticks).
- The trucks of A–C count as dealt with when they reach their exit tiles; the flight can be completed without a kill.
- Success (§4.4) ends the flight. Outcome = 1.

### 5.2 Flight 2 (Vulture recon from the west; transmission; convoy from the north; reinforcements from the south)

Start: objective `e1.objectives[1]`; player class 4; slots 3..7 and 8..17 hidden; markers 1..7 = 32h, slot 2 = 39h; phase 1. `Msn_UpdateEvents`: slots 19, 20 respawn (class 7, armour 1). Comm viewer: phase 1 → transmission 1, phase 2.

```text
succeeded := view = (102,104) and phase = 100
exit 1 @ (50,198); exit 2 @ (51,199); exit 3 @ (1,122); exit 4 @ (1,121)
exit 5 @ (2,124);  exit 6 @ (1,122);  exit 7 @ (1,124)
engage 1 → player, 30
engage 1 → 2, 70
engage 5 → player, 30
engage 6 → player, 60
engage 7 → player, 60
engage 18 → player, 10
engage 19 → player, 10
engage 20 → player, 10
A: if ticker idle and phase = 1: post(M1[5], c = 42); comm pending := 1
B: if dead(2): comm pending := 0
C: if dead(1, 2) and phase < 3:
       comm pending := 0; phase := 3; post(M1[6]); go 3; go 4; go 5
       spawn 3 as 0; spawn 4 as 0; spawn 5 as 7
   else if phase is neither 3 nor 4: hide 3, 4, 5
D: if dead(5) and phase = 3:
       phase := 4; post(M1[7]); go 6; go 7; spawn 6 as 7; spawn 7 as 7
   else if phase ≠ 4: hide 6, 7
E: if phase = 4 and dead(6, 7):
       post(M1[4]); phase := 100
```

Notes:
- While the phase is 1 the transmission is re-announced every time the ticker becomes idle. If slot 2 is dead, A sets and B clears the flag in the same tick, so neither the indicator nor its sound appears; the message still repeats.
- The flight does not need the transmission to be read: C only needs slots 1 and 2 gone.
- Once the phase is 100, C's `else` hides any convoy truck (slots 3, 4) that is still alive, and D's hides 6 and 7.
- Outcome = 1.

### 5.3 Flight 3 (rescue Montgomery; then a Vulture II, two Vultures, another Vulture II, a last Vulture)

Start: objective `e1.objectives[2]`; player class 4; slot 1 class 4 (an escort Hornet); slots 5..8 and 12 hidden; marker of slot 4 (Montgomery) = 39h, all other markers 0; phase 1. `Msn_UpdateEvents`: slot 19 respawns (class 7, armour 1).

```text
succeeded := view = (102,104) and phase = 100
for i = 2, 3:
    engage 1 → i, 20
    engage i → 1, 20
    engage i → player, 20
    engage i → 4, 20
engage 1 → 12, 20
engage 7 → player, 30
engage 6 → player, 30
engage 12 → 1, 50
engage 12 → player, 90
engage 19 → player, 10
exit 1 @ (107,106); exit 2 @ (7,122); exit 3 @ (1,124)
if tile(4) = (107,106) and class(4) ≠ FFh:
    hide 4; post(M1[8]); marker(2) := 32h; marker(3) := 32h
exit 5 @ (199,122); exit 6 @ (50,1); exit 7 @ (51,1); exit 8 @ (50,198); exit 12 @ (199,122)
if tileX(12) = 199 and 120 ≤ tileY(12) ≤ 125: hide 12
A: if phase = 1 and ticker idle:
       post(M1[9]); phase := 2; spawn 5 as 8; marker(5) := 32h
B: if phase = 2 and ticker idle and dead(4):
       post(M1[10]); phase := 3; spawn 6 as 7; spawn 7 as 7; marker(7) := 32h; marker(6) := 32h
C: if (dead(2) or dead(3)) and ticker idle and phase = 3:
       post(M1[11]); spawn 8 as 8; phase := 4; marker(8) := 32h
D: if dead(8) and ticker idle and phase = 4:
       post(M1[12]); spawn 12 as 7; phase := 5; marker(12) := 32h
E: if dead(12) and ticker idle and phase = 5:
       post(M1[13]); phase := 100
```

Notes:
- A fires as soon as the objective ticker of the flight start has finished (about 156 ticks), whatever the player does.
- B waits for Montgomery (slot 4) to be gone: arrived at (107, 106), or destroyed. The outcome (§4.8) tells the two apart by his tile: 1 only when he arrived. The flow then plays dialogue 13 or 12 ([episode-program-flow](../flow/episode-program-flow.md) §4.4).
- Slot 5 (the Vulture II of A) is never required.
- The guidance line follows Montgomery first (his marker is the only one set), then the lowest live marked slot.

### 5.4 Flight 4 (infra-red; two Vultures in the south-west; two Vulture IIs; a Vulture II and two transports)

Start: objective `e1.objectives[3]`; palette mode 1 (infra-red); player class 4; slot 1 class 4; slots 4, 5, 7 hidden; slots 8, 9 class 1; markers 2..5, 7 = 32h. `Msn_UpdateEvents`: slot 19 respawns (class 7, armour 1).

```text
succeeded := view = (102,104) and phase = 100
exit 2 @ (199,124); exit 3 @ (199,124); exit 4 @ (50,1); exit 5 @ (199,122); exit 7 @ (50,199)
if tile(6) = (84,135): slot 6's four speed bytes (DS:3B7E..3B81) := 16 (it stops)
engage 1 → 7, 50;  1 → 2, 50;  1 → 3, 60;  1 → 5, 20;  1 → 4, 30
engage 2 → 1, 30;  3 → 1, 30;  2 → player, 40;  3 → player, 40
if phase > 3: engage 4 → player, 30;  5 → player, 30
engage 7 → 1, 30;  7 → player, 60;  7 → 8, 30;  7 → 9, 30;  19 → player, 10;  20 → player, 10
A: if ticker idle and phase = 1: post(M1[14]); phase := 2
B: if phase = 3: phase := 4
C: if ticker idle and phase = 2 and dead(2, 3):
       post(M1[15]); spawn 4 as 8; spawn 5 as 8; phase := 3
D: if phase = 4 and dead(4, 5): post(M1[16]); spawn 7 as 8; spawn 8 as 1; spawn 9 as 1; phase := 5
E: if phase = 5 and dead(7): post(M1[17]); phase := 100
```

B precedes C, so phase 3 lasts exactly one tick (it becomes 4 at the next tick's B). Outcome = 1.

### 5.5 Flight 5 (escort Demeter; transmission; clean up)

Start: objective `e1.objectives[4]`; player class 4; marker 1 (Demeter) = 39h, markers 3..5, 7 = 32h. `Msn_UpdateEvents`: slots 19, 20 respawn (class 7, armour 1). Comm viewer: phase 2 → transmission 2, phase 3.

```text
succeeded := view = (102,104) and phase = 100
A: if tile(1) = (83,174) and class(1) ≠ FFh and phase = 1: phase := 2
engage 3 → 1, 40;  3 → player, 20;  4 → 1, 35;  4 → player, 15
engage 5 → 1, 40;  5 → player, 20;  7 → player, 20;  7 → 1, 10
exit 1 @ (199,122)
B: if dead(1) and tile(1) = (199,122) and phase < 4: post(M1[18], c = 42); phase := 4
C: if dead(1) and phase < 4: post(M1[19], c = 42); phase := 4
D: if phase = 4 and ticker idle and not dead(3, 4, 5, 7): post(M1[20], c = 42); phase := 5
E: if phase is 4 or 5 and ticker idle and dead(3, 4, 5, 7): post(M1[21], c = 42); phase := 100
F: if dead(1): comm pending := 0
G: if phase = 2 and ticker idle: post(M1[22], c = 42); comm pending := 1
```

B and C tell Demeter's safe exit from her destruction only by her final tile; the outcome (§4.8) uses the same test. G repeats the announcement while the phase is 2.

### 5.6 Flight 6 (Vulture II patrol in the west; transmission; three more Vultures)

Start: objective `e1.objectives[5]`; player class 2; markers 3, 5, 7..9 = 32h. `Msn_UpdateEvents`: slots 19, 20 respawn (class 7, armour 1). Comm viewer: phase 3 → transmission 3, phase 4.

```text
succeeded := view = (102,104) and phase = 100
engage 7 → player, 30;  8 → player, 40;  9 → player, 35;  19 → player, 10;  20 → player, 10
engage 5 → player, 40;  3 → player, 30
A: if Msn_NearLive(3, player, 40) and phase < 3 and ticker idle:
       post(M1[23], c = 42); comm pending := 1; phase := 3
B: if dead(3):
       comm pending := 0
       if dead(5) and ticker idle and phase < 5: phase := 5; post(M1[24], c = 42)
C: if dead(3, 5, 7, 8, 9) and phase < 100: post(M1[25], c = 42); phase := 100
```

### 5.7 Flight 7 (mines)

Start: objective `e1.objectives[6]`; player class 2; markers 1..12 = 32h; `mine.td` replaces sprite set 3 (structures §6). `Msn_UpdateEvents`: slots 13, 14 respawn (class 7, armour 10).

```text
succeeded := view = (102,104) and phase = 100
for m = 2..12:                                  (mines)
    if Msn_PlayerTouches(m):
        mission over := 1; end ticks := 1                     (the player is destroyed)
        five explosions at mine tile + (1,1), (0,0), (1,−1), (−1,1), (−1,−1), offset (16,16),
            each in the lowest idle explosion slot of 1..20 (frame 8), frame := 1; none idle: skipped
        armour(m) := 0; shields(m) := 0
for m = 2..8:  if Msn_Near(1, m, 1): class(m) := 3      (the layer, slot 1, arms mine m by
                                                        standing on its tile; dead or hidden counts)
exit 1 @ (199,122)
engage 13 → player, 50;  14 → player, 50
if phase < 3:
    phase := 1 + 5 × (number of dead slots among 1..12)       (byte arithmetic)
    phase := 2 if phase ≥ 60 else 1
A: if phase = 2 and ticker idle: post(M1[26], c = 42); phase := 100
```

All twelve of slots 1..12 must be gone for phase 2 (1 + 5 × 11 = 56 < 60). **Second completion path (instruments, tick-order step 19):** when no marked slot is live the guidance line points home, and if slots 2, 3 and 4 are dead the guidance routine sets phase := 100 directly (render/hud §4.3). Both paths need all marked slots 1..12 gone, so they fire in the same tick; the instruments path wins whenever the ticker is busy at that moment, and then the message `M1[26]` is never shown (U-180).

### 5.8 Flight 8 (infra-red; convoy and escort from the east; two Vulture IIs; transmission)

Start: objective `e1.objectives[3]` (the infra-red text); palette mode 1; player class 2; markers 3..9, 12, 13, 15..18 = 32h. Comm viewer: phase 4 → transmission 4, phase 5.

```text
succeeded := view = (102,104) and phase = 100
for s = 3..6: if class(s) ≠ FFh: class(s) := 7
A: if phase = 1 and ticker idle: phase := 2; post(M1[27], c = 42)
B: if dead(12, 13) and phase = 2: post(M1[28], c = 42); phase := 3; spawn 8 as 8; spawn 9 as 8
C: if dead(8, 9) and comm pending ≠ 0 and phase = 4: phase := 5
D: if dead(8, 9): comm pending := 0
E: if (Msn_NearLive(player, 8, 30) or Msn_NearLive(player, 9, 30)) and ticker idle and phase = 3:
       phase := 4; post(M1[29], c = 42); comm pending := 1
F: if dead(15, 16, 17, 18, 3, 4, 5, 6, 8, 9) and phase = 5: post(M1[30], c = 42); phase := 100
for s = 3..6: engage 1 → s, 20
engage 1 → 8, 10;  1 → 9, 10;  1 → 12, 20;  1 → 13, 20;  1 → 15, 30;  1 → 16, 30
for s = 3..6: engage s → player, 20;  s → 1, 15;  s → 12, 25;  s → 13, 25;  12 → s, 5;  13 → s, 5
exit 3 @ (50,1); exit 4 @ (51,1); exit 5 @ (49,1); exit 6 @ (51,1)
exit 8 @ (50,1); engage 8 → player, 35; engage 8 → 1, 10
exit 9 @ (51,1); engage 9 → player, 35; engage 9 → 1, 10
exit 12 @ (199,121); engage 12 → 1, 5; engage 12 → player, 5
exit 13 @ (199,124); engage 13 → player, 5; engage 13 → 1, 5
```

E only reacts to the player (the view tile) within 30 tiles of a live slot 8 or 9; if both die before that, C cannot fire (phase 3 ≠ 4) and F can never be reached: the flight cannot be completed (U-182).

### 5.9 Flight 9 (transmission; the radar trucks and their guards)

Start: objective `e1.objectives[7]`; player class 4; markers 1..10 = 32h. `Msn_UpdateEvents`: slots 11, 12 respawn (class 7, armour 10). Comm viewer: phase 2 → transmission 5, phase 3.

```text
succeeded := view = (102,104) and phase = 100
A: if phase = 1 and ticker idle: post(M1[31], c = 42); comm pending := 1; phase := 2
B: if dead(1..10) and phase > 1 and phase ≠ 100: comm pending := 0; phase := 100; post(M1[32], c = 42)
for a = 5..12: (for b = 13..18: engage a → b, 20); engage a → player, 15
for a = 5..12: for b = 13..18: engage b → a, 10
exit 13 @ (50,199);  engage 13 → 1, 30;  13 → 3, 30;  13 → player, 10
exit 14 @ (199,124); engage 14 → 1, 30;  14 → 2, 30;  14 → player, 10
exit 15 @ (199,122); engage 15 → 4, 30;  15 → player, 10
exit 16 @ (199,123); engage 16 → player, 15;  16 → 2, 30;  16 → 1, 30
exit 17 @ (199,124); engage 17 → 4, 30;  17 → player, 10;  17 → 12, 10
exit 18 @ (199,121); engage 18 → 3, 30;  18 → player, 25
```

Phase 2 is needed for B, so the flight completes only after the announcement was posted (it is posted as soon as the objective ticker ends).

### 5.10 Flight 10 (five Vultures and a Vulture II from the north; transmission)

Start: objective `e1.objectives[8]`; player class 4; markers 7..12 = 32h. Comm viewer: phase 2 → transmission 6, phase 3.

```text
succeeded := view = (102,104) and phase = 100
A: if phase = 1 and ticker idle: post(M2[0], c = 42); comm pending := 1; phase := 2
B: if dead(12): comm pending := 0
C: if dead(7..12) and phase < 100: phase := 100; post(M2[1], c = 42)
for s = 7..12: engage 1 → s, 25            (the range test is made with the pair (s, 1))
exit 7 @ (1,124)
for s = 7..11: engage s → 1, 15;  s → player, 20;  s → 3, 30;  s → 4, 30;  s → 5, 30;
               s → 14, 30;  s → 15, 30;  s → 16, 30
exit 8 @ (1,124); exit 9 @ (199,122); exit 10 @ (199,124); exit 11 @ (1,122); exit 12 @ (199,122)
if armour(12) > 50:
    engage 12 → 3, 10;  12 → 4, 10;  12 → 5, 10;  12 → 14, 25;  12 → 15, 25;  12 → 16, 25;
    engage 12 → 1, 10;  12 → player, 50
exit 3 @ (1,122); exit 4 @ (1,124); exit 5 @ (1,123); exit 14 @ (1,122); exit 15 @ (1,124); exit 16 @ (1,123)
```

### 5.11 Flight 11 (mercenaries from the west; Dispater)

Start: objective `e1.objectives[9]`; player class 2; markers 1..3, 7 = 32h. `Msn_UpdateEvents`: slots 8, 9, then 17, 18 respawn (class 7, armour 10).

```text
succeeded := view = (102,104) and phase = 100
if class(7) ≠ FFh: class(7) := 8;  if class(8) ≠ FFh: class(8) := 7;  if class(9) ≠ FFh: class(9) := 7
A: if dead(1, 2, 3) and phase = 1:
       phase := 2; post(M2[2], c = 42); spawn 7 as 8; spawn 8 as 7; spawn 9 as 7
B: if phase = 2 and ticker idle: phase := 100; post(M2[3], c = 42)
exit 1 @ (199,122)
for a = 1..3: (for b = 4, 5: engage a → b, 20); (for b = 11..15: engage a → b, 15)
engage 1 → player, 40; exit 2 @ (199,124); engage 2 → player, 40; exit 3 @ (199,123); engage 3 → player, 40
for a = 4, 5: for b = 1..3, 7..9, 17, 18: engage a → b, 10
exit 7 @ (199,122); engage 7 → 4, 15;  7 → 5, 15;  7 → player, 50
exit 8 @ (199,124); engage 8 → 11..15, 15 (in that order);  8 → 4, 20;  8 → 5, 20;  8 → player, 35
exit 9 @ (199,123); engage 9 → 4, 15;  9 → 5, 15;  9 → 11..15, 10;  9 → player, 40
for a = 17, 18: (for b = 11..15: engage a → b, 15); (for b = 4, 5: engage a → b, 15)
engage 17 → player, 15; engage 18 → player, 15
```

The respawn of slots 8, 9 (§4.7) gives class 7 and armour 10 while they are dead and far, also before A has spawned them (they start hidden, so they appear as soon as the player is far from their start tiles: U-183).

### 5.12 Flight 12 (helicopter; escort the convoy out of the south)

Start: objective `e1.objectives[10]` (colour 42); player class 3 (helicopter), drawn late; markers 1..5 = 39h. Base: viewX = 102, viewY = 99..100. Outcome: §4.8.

```text
succeeded := viewX = 102 and 98 < viewY < 101 and phase = 100
A: if dead(1..5) and phase = 1: phase := 100; post(M2[4], c = 42)
exit 7 @ (50,1);   engage 7 → 5, 4, 1, 2, 3 (in that order), 35
exit 8 @ (50,1);   engage 8 → 5, 1, 2, 3, 4, 35
exit 9 @ (50,1);   engage 9 → 15, 15;  9 → 16, 15;  9 → 5, 1, 2, 3, 4, 40
exit 10 @ (51,199); engage 10 → 5, 1, 2, 3, 4, 35
exit 11 @ (51,199); engage 11 → 5, 1, 2, 3, 4, 35
exit 12 @ (50,1);   engage 12 → 15, 10;  12 → 16, 10;  12 → 1, 2, 3, 4, 5, 25
exit 13 @ (51,1);   engage 13 → 15, 10;  13 → 16, 10;  13 → 1, 2, 3, 4, 5, 25
for s = 1..5: for x = 49..51: exit s @ (x,199)
exit 1 @ (49,199); exit 2 @ (50,199); exit 3 @ (51,199); exit 4 @ (49,199); exit 5 @ (51,199)
exit 15 @ (50,199); engage 15 → 7, 8, 9, 10, 11, 12, 13, 15
exit 16 @ (51,199); engage 16 → 7, 8, 9, 10, 11, 12, 13, 25
```

(`engage a → b1, b2, …, r` lists several rules with the same attacker and range, in order.) The convoy (slots 1..5) leaving at y = 199 or being destroyed both end phase 1; the outcome distinguishes them.

### 5.13 Flight 13 (Vulture II; three Hornets in the west; the depot)

Start: objective `e1.objectives[11]` (colour 42); player class 8; markers 8, 9, 11..13, 15..17 = 32h. Base: view 175..177 × 152..154 (home point (180, 155)).

```text
succeeded := 175 ≤ viewX ≤ 177 and 152 ≤ viewY ≤ 154 and phase = 100
A: if phase = 1 and dead(8, 9, 15, 16, 17): post(M2[5], c = 42); phase := 2
B: if phase = 2 and dead(8, 9, 11, 12, 13, 15, 16, 17): post(M2[6], c = 42); phase := 100
engage 1 → 8, 9, 15, 16, 17, 25;  2 → 8, 9, 15, 16, 17, 25
engage 8 → 1, 15;  8 → 2, 15;  8 → player, 30;  9 → 2, 15;  9 → 1, 15;  9 → player, 30
engage 11 → 1, 25;  11 → 2, 25;  11 → player, 30;  12 → 2, 15;  12 → 1, 15;  12 → player, 30
engage 13 → player, 35;  13 → 1, 20;  13 → 2, 20;  15 → player, 25;  16 → player, 30;  17 → player, 30
```

`Msn_UpdateEvents`: when `g_MissionTicks` equals exactly 15,000 (about 13.7 minutes): `e1.flight_events.01[0]` (colour 37) into the ticker, ticker := 1, mission over := 1. The end sequence then runs and the flight ends as a death (`g_PlayerDied` = 1, retry offered). The jurisdiction warning in flights 13 and 14 is `jurisdiction.warnings[1]`.

### 5.14 Flight 14 (Vulture II; the Stiletto; transmission)

Start: objective `e1.objectives[12]` (a blank line, colour 42); player class 8; markers 8..10, 12, 13 = 32h. F5 (ship switch) is disabled in this flight. Comm viewer: phase 2 → transmission 7, markers := 0 for 1..20 then 32h for 1..6, phase := 100.

```text
succeeded := view = (102,104) and phase = 100
A: if (Msn_NearLive(player, 12, 10) or Msn_NearLive(player, 13, 10) or dead(12) or dead(13))
      and phase = 1 and ticker idle:
       post(M2[7], c = 42); comm pending := 1; phase := 2
for a = 1..6: engage a → 8, 9, 10, 12, 13, 5;  then a → player with range 50 (a = 1, 3, 5, 6), 25 (a = 4),
              none for a = 2
for a = 8, 9, 10: engage a → 1, 2, 3, 4, 5, 6, 5
engage 12 → 1, 2, 3, 4, 6, 5, 5   (target 6 before 5)
engage 13 → 1, 2, 3, 4, 5, 6, 5
engage 8 → player, 35;  9 → player, 25;  10 → player, 30;  12 → player, 35;  13 → player, 30
```

Order within each attacker a = 1..6: targets 8, 9, 10, 12, 13 (range 5), then the player. The flight completes when the transmission is read (phase 100), then the base test.

### 5.15 Flight 15 (helicopter; two convoys)

Start: objective `e1.objectives[13]` (colour 42); player class 3, drawn late; markers 1..5, 8..12 = 32h. Base as flight 12.

```text
succeeded := viewX = 102 and 98 < viewY < 101 and phase = 100
A: if dead(1..5) and phase = 1:
       phase := 2; post(M2[8], c = 42); spawn 8 as 0; spawn 9 as 0; spawn 10 as 7; spawn 11 as 7; spawn 12 as 7
B: if dead(8..12) and phase = 2: phase := 100; post(M2[9], c = 42)
exit 1 @ (50,199); exit 2 @ (50,199); exit 8 @ (50,1); exit 9 @ (50,1)
exit 3 @ (1,122);   engage 3 → 15, 20;  3 → 16, 20
exit 4 @ (50,199);  engage 4 → 15, 25;  4 → 16, 25
exit 5 @ (50,199);  engage 5 → 15, 25;  5 → 16, 25
exit 10 @ (50,1);   engage 10 → 15, 20; 10 → 16, 20
exit 11 @ (50,1);   engage 11 → 15, 25; 11 → 16, 25
exit 12 @ (199,122); engage 12 → 15, 25; 12 → 16, 25
for a = 15, 16: engage a → 3, 4, 5, 10, 11, 12, 5;  a → 1, 2, 8, 9, 30
```

### 5.16 Flight 16 (convoy from the south; another at the east gate)

Start: objective `e1.objectives[14]` (colour 42); player class 4; markers 1..3, 8..10 = 32h. `Msn_UpdateEvents`: slots 4, 5 respawn (class 8, armour 15). Outcome: §4.8.

```text
succeeded := view = (102,104) and phase = 100
A: if dead(1, 2, 3) and phase = 1:
       spawn 8, 9, 10 as 0; spawn 11, 12 as 8; spawn 13 as 7; phase := 2; post(M2[10], c = 42)
B: if dead(8, 9, 10) and phase = 2: phase := 100; post(M2[11], c = 42)
exit 1 @ (124,71); exit 2 @ (125,71); exit 3 @ (126,71)
engage 4 → 17, 30;  4 → player, 30
exit 6 @ (199,122); engage 6 → 17, 30;  6 → player, 30
engage 5 → player, 30;  5 → 17, 30
exit 8 @ (125,71); exit 9 @ (125,71); exit 10 @ (125,71)
exit 11 @ (1,122);  engage 11 → player, 30; 11 → 17, 30
exit 12 @ (1,122);  engage 12 → 17, 30; 12 → player, 30
exit 13 @ (50,199); engage 13 → 17, 30; 13 → player, 30
engage 17 → 4, 5, 6, 11, 12, 13, 15;  17 → 1, 2, 3, 8, 9, 10, 30
```

### 5.17 Flight 17 (Amiel; the base under attack)

Start: objective `e1.objectives[15]` (colour 42); player class 2; markers 1, 5, 6, 8..10, 12..14 = 32h. Comm viewer: phase 3 → transmission 8, phase 4. Outcome := not lost.

```text
succeeded := view = (102,104) and phase = 100
A: if dead(1) and phase = 1: cut-scene request (DS:32B6) := 1; phase := 2
B: if phase = 2: phase := 3; post(M2[12], c = 42); comm pending := 1
C: if phase = 4: comm pending := 0; spawn 5 as 7; spawn 6 as 7; phase := 5
D: if phase = 5 and dead(5, 6): phase := 6; spawn 8 as 7; spawn 9 as 8; spawn 10 as 8
E: if phase = 6 and dead(8, 9, 10): phase := 7; spawn 12, 13, 14 as 8
F: if phase = 7 and dead(12, 13, 14): phase := 100; post(M2[13], c = 42)
exit 1 @ (50,1); exit 3 @ (51,1); engage 3 → 1, 15
exit 5 @ (199,122); exit 6 @ (199,124); engage 6 → player, 5
exit 8 @ (50,1); engage 8 → player, 5; exit 9 @ (50,1); exit 10 @ (199,124)
exit 12 @ (199,122); exit 13 @ (199,122); engage 13 → player, 5; exit 14 @ (199,122); engage 14 → player, 5
```

`Msn_UpdateEvents`, in order:

```text
if phase = 3 and ticker idle: ticker := 1                 (replays the announcement)
for s = 1..20:                                            (attack on the base)
    if hit(s) = 0 and tile(s) = (107,106) and count < 3:  (hit(s) = DS:329F + s; count = DS:329D; no class test)
        hit(s) := 1
        4 times: an explosion in the lowest idle slot of 1..20: offset (Random(48), Random(48)), tile (106,106), frame 1
        count += 1
... (flight 20, 13, 19, 18 events) ...
if count > 2:                                             (the base is destroyed)
    if count < 44: w := Random(4); if w ≠ 0: w times: explosion at offset (Random(48), Random(48)),
                                               tile (105 + Random(2), 105 + Random(2)), frame 1
    if count > 40: hit flash := 0
    count = 43 or 47: DAC := (63, b, b) for every entry (b = blue of the flight palette)
    count = 44 or 46: DAC := (63, 63, b)
    count = 45: DAC := white; map tiles (105..108, 104..107) := 49 (16 calls Random(1), each 0)
    count = 48: DAC := the flight palette; lost := 1
    count += 1
cut-scene (when DS:32B6 = 1): fade out; td1-17.pic (90 rows from screen row 67); text
    e1.flight_events.02[0..4] with delays (1,000 ms and 2,000 ms) and palette flashes; fade back; DS:32B6 := 0
```

The cut-scene uses `Crt.Delay`, i.e. real time; ticks stop while it runs. Three ships standing on tile (107, 106) (whether alive or not) start the destruction, which ends the flight as lost 46 ticks later.

### 5.18 Flight 18 (Leopold's bomb truck)

Start: objective `e1.objectives[16]` (colour 42); player class 2; markers 7, 10, 11, 15, 17, 18 = 32h. Comm viewer: phase 2 → transmission 9, phase 3. Retry and ending C: flow spec §4.4–4.5.

```text
succeeded := view = (102,104) and phase = 100
A: if dead(7, 10, 11, 15, 17): marker(3) := 32h
B: if Msn_NearLive(player, 3, 10) and phase = 1:
       marker(3) := 32h; markers 7..20 := 0; phase := 2; post(M2[14], c = 42); comm pending := 1; timer := 0
C: if dead(3): comm pending := 0
D: if phase is 2 or 3: timer += 1                       (timer = DS:32B4, u16)
E: bomb := (timer > 999)                                (DS:32B1)
F: if dead(3) and phase is 2 or 3: post(M2[15], c = 42); timer := 0; phase := 100
engage 1 → 3, 5, 6, 7, 9, 10, 11, 13, 14, 15, 15
exit 5 @ (50,199);  engage 5 → 1, 10;  5 → player, 10
exit 6 @ (199,122); engage 6 → 1, 10;  6 → player, 15
exit 9 @ (199,123); engage 9 → 1, 10;  9 → player, 15
exit 13 @ (50,199); engage 13 → 1, 15; 13 → player, 15
exit 14 @ (51,199); engage 14 → 1, 15; 14 → player, 15
exit 7 @ (50,1); exit 10 @ (50,199); exit 11 @ (199,123); exit 15 @ (50,199)
```

`Msn_UpdateEvents`: when the bomb flag is 1: hit flash := 0, DAC and flight palette all white, lost := 1. So the player has 1,000 ticks (about 55 s) from finding the truck to destroy it.

### 5.19 Flight 19 (the V.C.I. detonator; switch to the Vulture II)

Start: objective `e1.objectives[17]` (colour 42); player class 0 (truck); home marker = 32h; home point (123, 72) while phase < 100.

```text
succeeded := view = (102,104) and phase = 100
A: if phase = 2: home marker := 3Dh; post(M2[16], c = 42); phase := 100
B: if phase > 1: for s = 1..10: engage s → player, 25
```

`Msn_UpdateEvents`: if phase = 1 and viewY = 70 and 117 < viewX < 124 (the player has driven the truck to the V.C.I.): store the five F5-screen info lines (`e1.flight_events.01[1..4]`, the last twice) at DS:1FD4.., show `e1.flight_events.01[5]` on a cleared screen for 2,000 ms (real time), then: player class := 8; armour := 1; slot 19 := a class-0 ship (the truck left behind) at the player's position (tile (viewX + 5, viewY + 3), offsets = the view offsets; also stored as words at DS:16EE..16F5); phase := 2; viewY += 1. Tick-order step 10 adds the flight-19 rule "phase > 1: shields capped at 10" (tick-order §4).

### 5.20 Flight 20 (the lander)

Start: no objective; player class 2; marker 5 = 32h; slots 3, 4, 5 classes 4, 5, 6 and class tables patched (RE-5). No base test: only `Msn_UpdateEvents` sets the result. Mission ships: slots 1..5 drift down 2 px per tick ([mission-ships](mission-ships.md) §3); shots of slot 1 never hit ships ([weapons](weapons.md) §4.3). Sim: view x > 170 and view y > 170 → armour := 64 (tick-order step 34).

```text
Msn_UpdateB: engage 7 → player, 35;  8 → player, 35
Msn_UpdateEvents (count = DS:329D):
    if armour(5) > 50 and back page = 1 and Random(3) > 1:       (bombs from slot 1)
        three shots, each in the highest free projectile slot of 0..40 (none free: slot 40 is reused):
            life 1, owner 1, kind 2:
            1: tile(1), offset (subX(1), subY(1)), heading 2
            2: tile(1) + (1, 0), offset (subX(1) + 32, subY(1)), heading 2
            3: tile(1) + (1, 0), offset (subX(1), subY(1) − 10), heading Random(9)
    armour(1..4) := 200
    if armour(5) < 50:
        count += 1; armour(5) := 49
        for s = 1..5: in the lowest idle explosion slot of 1..20: sound 9 at tile(s); offset (Random(48), Random(48));
                      tile := tile(s); frame 1
    if count > 149: sound 100; succeeded := 1; hit flash := 0; DAC and flight palette all white
    if tileY(5) > 160: subY(1..5) += 2;   if tileY(5) > 170: subY(1..5) += 4
    if tileY(5) > 194: lost := 1 if count < 2 else succeeded := 1
```

The player must bring slot 5's armour below 50 at least twice before the lander reaches row 195. Retry and ending C: flow spec.

### 5.21 Flight 21

No mission rules. Objective `e1.objectives[18]` (colour 42); player class 3, drawn late. At tick 25 the tick loop sets mission over (tick-order step 1), so the flight ends through the end sequence: `g_PlayerDied` = 1 and outcome = 1 at teardown; the mission-19 block ignores both (flow spec §4.4). What the flight shows is U-044.

## 6. Edge cases and original bugs

| Quirk | Original behaviour | Port policy |
|---|---|---|
| Three player positions (§2) | Engagement ranges are measured from the view's top-left tile, about (−5, −3) tiles from the ship | Preserve (T1) |
| Escaped targets count as done | Exit rules and "all dead" tests treat leaving like destruction | Preserve |
| Respawn distance uses AND | A dead ship respawns only when the player is more than 15 tiles away on **both** axes | Preserve |
| Ticker interrupts | A rule that posts without testing "ticker idle" cuts the current message off | Preserve |
| Shared loop counters | Rule loops use globals DS:32BE/32C0/32C2 | Harmless (nothing else reads them); a port may use locals |

## 7. Test vectors

| # | Setup | Expected | Evidence |
|---|---|---|---|
| 1 | Flight 1, first tick | Phase 1; ticker restarted with `e1.objectives[0]`; slots 3..8 class FFh | static; C-0002 (`g_MissionPhase` = 1 in the +1 s and +2 s dumps) |
| 2 | `Msn_NearLive(8, 0, 20)` with view (102, 104), tile(8) = (121, 104), class(8) = 7 | true (\|121 − 102\| = 19 < 20 and 0 < 20) | static |
| 3 | `Msn_NearLive(8, 0, 20)` with tile(8) = (122, 104) | false (\|122 − 102\| = 20 is not < 20) | static |
| 4 | `Msn_NearLive(8, 0, 20)` with tile(8) = (110, 100) and class(8) = FFh | false (dead) | static |

## 8. Open questions

- [ ] U-180: the flight-7 completion shortcut in the instruments routine (§5.7) can skip the "all mines destroyed" message. Confirm in a capture.
- [ ] U-181: `AI_SetTarget` re-arms the target every tick while the range test holds; what the AI does when the target leaves the range (RE-5, U-045).

## 9. Provenance

| Claim | Address / capture | Method | Confidence |
|---|---|---|---|
| Blocks per flight, base test, exit/engage/phase rules | `2404:1426..2F2D`, `2404:3521..60FC` | static (decompiler + listing for every call's push order) | LIKELY |
| Primitive semantics | `2404:05DC`, `2404:06EE`, `2404:07DB`, `2404:0616`, `2404:030A`, `34E6:3232` | static | CONFIRMED (code) |
| Comm viewer table | `0E8B:80D3..8241` | static | LIKELY |
| Respawns and scripted events | `0E8B:9860..A713` | static | LIKELY |
| Outcome tests | `0E8B:F826..F990` | static (listing) | CONFIRMED (code) |
| Start state | `0E8B:60D8` (RE-5's) | static | LIKELY |
| Message ids | string addresses `2404:0898..13CF` and `2404:2F2E..34C2` = `e1.flight_messages.01/02` | resource map | CONFIRMED |

## 10. Revision history

| Rev | Date | Author (session/agent) | Change |
|---|---|---|---|
| 1 | 2026-11-02 | RE-6 (Archaeologist + Spec author) | Initial: architecture, primitives, flights 1–3 |
